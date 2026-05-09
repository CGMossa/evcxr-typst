# Plan: backfill missing sidecars on watch startup (#30)

## 1. Root cause

`Project::open_with_config` runs `discovery::discover` once and stores the result as `initial_snippets`. `watch::run` passes them to `watch_thread`, which assigns them to `prev_snippets` at `crates/evcxr-typst/src/watch.rs:119`.

PR #32 added a startup cycle at `watch.rs:129–152`: it calls `run_one_cycle` once before entering the notify event loop. Inside `run_one_cycle` (line 220):

1. `discovery::discover` returns the same snippet list (nothing changed on disk) → `curr == initial_snippets`.
2. `classify(prev, &curr, false)` compares lists; every element has the same `id` and `src` → returns `Plan::Noop` (line 519).
3. The `Noop` arm (line 248) does nothing except log a debug message.
4. `*prev = curr` (line 308) leaves `prev_snippets` identical to `curr`.
5. `eval::write_available_index_for_snippets(cache_dir, prev)` (line 312) writes `_index.json`, but with no `<id>.manifest.json` files in an empty cache, `available` is empty: `{"v":1,"available":[]}`.

Subsequent edits also `Noop` if the snippet content didn't change (which on cold-cache startup, it hasn't — the user just started watch). Sidecars never get produced; loop is stuck.

Repro:

```sh
rm -rf examples/hello/.evcxr-typst-cache
evcxr-typst watch --allow-eval --root . examples/hello/main.typ
# wait for PDF; observe _index.json is empty, no *.manifest.json files
```

## 2. Proposed fix — option (a): sidecar-presence check inside `run_one_cycle`

After `classify` returns `Plan::Noop`, filter `curr` for evaluable snippets whose `<id>.manifest.json` is absent from `cache_dir`. Evaluate the missing ones as an implicit backfill before falling through to the `_index.json` write.

**Why (a) over alternatives:**

- **(a) Sidecar-presence check.** Work proportional to what's actually missing. Handles partial cache cleanly (only re-eval the absent ones; warm-CAS materializes the rest in `eval_one`'s lookup path). No new parameter through the call chain. Self-contained.
- **(b) Force full eval on startup cycle.** Re-evaluates snippets that already have valid sidecars. Wastes rustc invocations on every watch start.
- **(c) Detect missing-sidecar at the cache layer.** Couples cache and watch via a new `LookupResult::MissingSidecar` variant. Disproportionate for a watch-startup concern.

Pick **(a)**.

The `<id>.manifest.json` is the right sentinel: `eval.rs:771–781` already defines a snippet as "available" when its manifest exists, and that's the same file the Typst package reads to decide whether to embed output. Using the same sentinel keeps the watch-side check consistent with what `lib.typ` considers present.

## 3. Concrete changes

### `crates/evcxr-typst/src/cache.rs` — add `has_sidecar` helper

After `drop_view_for_id` (around line 417):

```rust
/// Return `true` when the id-addressed manifest sidecar exists in `cache_root`.
///
/// Used by the watch loop to detect snippets that need backfill on startup.
pub(crate) fn has_sidecar(cache_root: &Path, snippet_id: &str) -> bool {
    cache_root
        .join(format!("{snippet_id}.manifest.json"))
        .exists()
}
```

### `crates/evcxr-typst/src/watch.rs` — extend `Plan::Noop` arm

Replace the existing `Plan::Noop => { tracing::debug!("noop: no snippet changes"); }` with:

```rust
Plan::Noop => {
    if allow_eval && let Some(ctx) = ctx_opt.as_mut() {
        let missing: Vec<Snippet> = curr
            .iter()
            .filter(|s| eval::is_evaluable(s.kind) && !cache::has_sidecar(cache_dir, &s.id))
            .cloned()
            .collect();
        if !missing.is_empty() {
            tracing::debug!(count = missing.len(), "noop with missing sidecars — backfilling");
            for s in &missing {
                let had_panic =
                    eval_one(ctx, s, cache_dir, env, prev, stdout_rx, stderr_rx)?;
                if had_panic {
                    *prev_had_panic = true;
                }
            }
        } else {
            tracing::debug!("noop: no snippet changes");
        }
    } else {
        tracing::debug!("noop: no snippet changes");
    }
}
```

Notes:

- Filter `eval::is_evaluable(s.kind)` excludes `Dep` snippets (no manifest is ever produced for them).
- Deny-eval mode skips the backfill (no `CommandContext`), preserving existing behavior.
- `prev` is not yet updated at this point (that happens at line 308); `eval_one`'s prior-Merkle-chain build uses the same `prev` that `evcxr-typst run` would, so chain semantics are correct.
- After the loop falls through, `*prev = curr` and `write_available_index_for_snippets` run as normal; the index now includes the freshly backfilled IDs.

### `crates/evcxr-typst/CLAUDE.md` — note in watch.rs invariants section

Append a one-line note to invariant 4 (or as a new invariant 5) clarifying that on startup, missing manifests are backfilled by the Noop arm; first-run latency on a cold cache = N × rustc-compile-time (same as `run` subcommand).

## 4. Test that pins it — `tests/watch_sidecar_backfill.rs`

Outline (skip if `typst` not on PATH):

1. `repo_root()` from `CARGO_MANIFEST_DIR`.
2. Place fixtures under `target/watch-sidecar-backfill-test/`.
3. Write `main.typ` with two evaluable snippets (`id: "backfill-1"`, `id: "backfill-2"`).
4. `fs::remove_dir_all(base.join(".evcxr-typst-cache"))` to ensure cold cache.
5. `Project::open_with_config(&entry, ProjectConfig::new().with_root(&root))` — skip on error.
6. `project.watch(&WatchOptions::allow_eval())` — skip on error.
7. Poll `cache_dir.join("backfill-1.manifest.json")` and `backfill-2.manifest.json` for up to 30 s (eval is slow on first compile). 100 ms intervals.
8. Drop handle.
9. Assert both files exist.
10. Cleanup `base`.

**Red-before-green:** run on unpatched watch.rs first, confirm timeout (`backfill-1.manifest.json not written within 30s`); apply fix, confirm green.

This is the **first integration test to exercise allow-eval in watch mode** — the others (`watch_subdir`, `watch_subdir_relative`, `watch_no_noop_runaway`) all use `WatchOptions::deny()`. Tests `CommandContext::new()` + rustc end-to-end inside `Project::watch`. Requires `--test-threads=1`.

## 5. Risks and edge cases

### #29 noop-runaway interaction

After backfill writes `_index.json` with newly available IDs, typst-watch detects the change and recompiles, writing `main.pdf`. With #29's `is_source_extension` filter, the `.pdf` event is dropped → no further cycle. Backfill settles after one `_index.json` write. The two fixes compose correctly.

### Deny-eval mode

`if allow_eval && ...` guard skips backfill in deny mode. Correct: nothing to evaluate without an eval context. Existing deny-mode tests (`watch_subdir_relative.rs`, `watch_no_noop_runaway.rs`) unaffected.

### First-cycle latency

Startup cycle on cold cache = N × rustc-compile-time (same cost as `evcxr-typst run`). Subsequent watch restarts hit the warm CAS in `eval_one`'s lookup path; startup drops to ms. Document in CLAUDE.md.

### Partial cache

If `_index.json` exists but only some `.manifest.json` files were deleted, the filter finds exactly the missing ones; the warm CAS materializes the rest without rustc. Minimal-work; option (a) handles this naturally.

### `Dep` snippets

Filtered by `eval::is_evaluable(s.kind)` — they never produce manifests.

### `RustHidden` and other no-output kinds

`is_evaluable` returns true for `RustHidden`. Hidden snippets do produce `.manifest.json` (empty MIME list). Backfill includes them — correct, because `CommandContext` state must accumulate through all snippets in order.

## 6. Out of scope

- Combining with #29 (already merged at `480a4c4`).
- Adding a `--rebuild` / `--force` flag to force full re-eval.
- Cache GC behavior changes.
- Pre-existing `cargo metadata --lockfile-path` warnings.

## Implementation checklist

- [ ] Add `cache::has_sidecar(cache_root, snippet_id) -> bool` in `cache.rs`.
- [ ] Extend the `Plan::Noop` arm in `run_one_cycle` with the backfill check.
- [ ] Add `tests/watch_sidecar_backfill.rs` (allow-eval, 30 s timeout, skips if typst not on PATH).
- [ ] Add a one-line note to `crates/evcxr-typst/CLAUDE.md` invariant 4 (or a new invariant 5).
- [ ] `cargo fmt --check` clean.
- [ ] `cargo test -p evcxr-typst -- --test-threads=1` — all five watch tests green: `watch_subdir`, `watch_subdir_relative`, `watch_no_noop_runaway`, `watch_sidecar_backfill`, plus unit tests in `watch.rs`.
- [ ] Red-before-green: confirm test times out on unpatched code; passes after fix.
