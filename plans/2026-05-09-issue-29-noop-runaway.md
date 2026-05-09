# Plan: filter typst-watch output rewrites from is_relevant (#29)

## 1. Root cause

After a real edit triggers a successful `reset_and_replay` cycle, `watch cycle plan plan="noop"` fires continuously at the typst-watch render cadence (~660 ms).

Call chain:

1. `spawn_typst_watch` (`crates/evcxr-typst/src/watch.rs:707`) spawns `typst watch <entry>` with no `--format` or `[OUTPUT]`. Typst defaults to writing `<stem>.pdf` next to the entry, i.e. inside `entry.parent()`.
2. That PDF write fires a notify event for an absolute path like `/path/to/project/main.pdf`.
3. `is_relevant` (line 691):
   - `path.starts_with(cache_dir)` → false (`main.pdf` is not under `.evcxr-typst-cache/`)
   - `path.starts_with(entry_parent)` → **true**
   So `is_relevant` returns true.
4. The debounce fires `run_one_cycle`. `typst query` returns the same snippets. `classify` → `Plan::Noop`.
5. `run_one_cycle` writes `_index.json` unconditionally at the end. That write is in the cache dir → filtered → does not loop.
6. Typst-watch's internal poll/recompile timer (~660 ms) keeps re-rendering, regenerating `main.pdf`, restarting the loop.

`EVCXR_TYPST_LOG=evcxr_typst=trace` shows `is_relevant` accepting events for `.pdf` paths inside `entry_parent`.

Future formats produced by `typst watch` (e.g. `--format html` in 0.14+) would reproduce the same bug.

## 2. Proposed fix — option (a): extension allowlist

Only accept events for paths whose extension is `.typ` or `.toml`. Drop all other extensions (`.pdf`, `.svg`, `.html`, `.png`, …).

Justification:

- **(a) Extension allowlist.** Answers "what is a relevant source edit for evcxr-typst?" semantically: the user is editing source files. `.typ` covers entry, included chapters, imported files. `.toml` covers `typst.toml` (font/dep entries warrant a re-discover). Narrowest, most semantic.
- **(b) Output-suffix denylist.** Smaller change, but maintenance debt: typst keeps adding output formats; forgetting one re-opens the bug.
- **(c) Track known typst-watch outputs** by deriving `<entry-stem>.pdf` and `<entry-stem>.svg`. Most coupled — breaks if typst changes its default output naming.
- **(d) Throttle/dedupe at discovery layer.** Hides the symptom, doesn't fix the cause; the unnecessary `typst query` invocations still happen.

Pick **(a)**.

## 3. Concrete changes

### `crates/evcxr-typst/src/watch.rs`

Add `is_source_extension` helper next to `is_relevant`; add the guard to `is_relevant`:

```rust
fn is_relevant(event: &Event, entry: &Path, cache_dir: &Path) -> bool {
    let entry_parent = match entry.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(p) => p,
        None => return false,
    };
    for path in &event.paths {
        if path.starts_with(cache_dir) {
            continue;
        }
        if path.starts_with(entry_parent) && is_source_extension(path) {
            return true;
        }
    }
    false
}

/// Source-file extensions that warrant an evcxr-typst eval cycle.
///
/// `.typ`  — Typst source files (entry, included chapters, imports).
/// `.toml` — typst.toml manifest (fonts, deps).
///
/// Output formats produced by `typst watch` (`.pdf`, `.svg`, `.html`, `.png`)
/// are intentionally excluded so a typst re-render does not retrigger our
/// eval loop. See issue #29.
fn is_source_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("typ" | "toml")
    )
}
```

The cache-dir exclusion stays at the same precedence (filtered before the extension check).

### Existing unit test — `is_relevant_accepts_subdir_path` (`watch.rs:903`)

Extend with three new assertions:

- `.pdf` sibling of entry → not relevant
- `.svg` sibling of entry → not relevant
- `typst.toml` sibling of entry → relevant

## 4. Test that pins it

New file `crates/evcxr-typst/tests/watch_no_noop_runaway.rs`:

- Skip if `typst` is not on PATH.
- Set up a temp project under `target/watch-no-noop-test/` with a plain Typst document (no evcxr snippets, but typst-watch will still write `main.pdf` next to it).
- Open the project and call `Project::watch(&WatchOptions::deny())`.
- Wait up to 8 s for `_index.json` to appear (the first cycle fires on the initial backoff timer, not from a notify event — so this works regardless of the fix).
- Capture the initial `_index.json` mtime.
- Over a 3 s observation window with no source edits, count how many times `_index.json` mtime advances.
- Assert count = 0.

On unpatched code: typst-watch's PDF write triggers `is_relevant` → noop cycle → `_index.json` rewritten → mtime advances → count ≥ 1 → test fails.
On patched code: `.pdf` filtered → no spurious cycle → mtime unchanged → count = 0 → test passes.

**Red-before-green:** run against unpatched watch.rs first, confirm count ≥ 1. Then apply fix, confirm count = 0.

## 5. Risks and edge cases

- **Real `.typ` edits still fire cycles.** Allowlist includes `.typ`. No regression for primary use case.
- **`typst.toml` edits.** Included on purpose: font/dep changes warrant a re-discover.
- **Other `.toml` files** (e.g. `Cargo.toml` if a Rust project happens to live next to the Typst project). Would fire a cycle. Tolerable in practice; if it becomes a problem, narrow to `path.file_name() == Some("typst.toml")`. Premature for v0.
- **`Cargo.lock` and Rust artifacts inside the cache.** Already filtered by `starts_with(cache_dir)`.
- **`tests/watch_subdir.rs`** mutates a `.typ` file. Still fires cycles. No regression.
- **`tests/watch_subdir_relative.rs`** also mutates a `.typ` file. The first `_index.json` write in that test comes from the initial backoff timer (`run_one_cycle` writes `_index.json` unconditionally on every cycle, regardless of trigger). Step 1 unaffected by the PDF filter. Step 2 (subdir mutation) sends a `.typ` event — still passes.
- **HTML output (`typst 0.14+ --format html`).** `.html` is not in the allowlist — correct, it should be filtered.

## 6. Out of scope

- Issue #30 (missing-sidecar backfill at startup).
- Cargo-metadata `--lockfile-path` warnings.
- `WatchHandle::join` rename.
- Combining with #30 (separate fix even though the file is the same).

## Implementation checklist

- [ ] Add `is_source_extension` helper in `watch.rs`.
- [ ] Extend `is_relevant` with the new guard.
- [ ] Extend the existing `is_relevant_accepts_subdir_path` unit test.
- [ ] Add `tests/watch_no_noop_runaway.rs`.
- [ ] `cargo fmt --check`.
- [ ] `cargo test -p evcxr-typst -- --test-threads=1` all green.
- [ ] Red-before-green: capture failure on unpatched code, confirm green after fix.
