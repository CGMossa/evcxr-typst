# Plan: `watch is_relevant` must compare canonical paths

Follow-up to PR #27 (commit `23d0a95`). The recursive watch lands events for files in subdirectories, but the comparison in `is_relevant` only matches when the user passes an absolute entry path on the CLI.

## 1. Root cause

`is_relevant()` at `crates/evcxr-typst/src/watch.rs:687` derives `entry_parent` from `entry.parent()`, then checks `path.starts_with(entry_parent)` for each notify event path. `Path::starts_with` is a literal component-prefix test — it does **not** resolve relative paths against the process working directory.

Notify (FSEvents on macOS, inotify on Linux) always delivers absolute paths. When the user runs `evcxr-typst watch examples/rust-by-example/main.typ` from the repo root, `entry` is the relative path `examples/rust-by-example/main.typ`, so `entry_parent` is the relative `examples/rust-by-example`. A literal prefix test of an absolute event path against a relative `entry_parent` can never match → `is_relevant` returns `false` on every event → no eval cycle ever fires.

The integration test `crates/evcxr-typst/tests/watch_subdir.rs` is vacuous for this regression: it constructs `entry` via `root.join("target/watch-subdir-test/main.typ")`, where `root` is derived from `CARGO_MANIFEST_DIR`; `entry` is therefore always absolute, and the relative-path code path is never exercised.

## 2. Proposed fix

**Option (a): canonicalize `entry` once at the top of `watch_thread`.** Replace `entry` with `std::fs::canonicalize(&entry)?` before any further use.

Rationale:

- Notify already delivers absolute canonical paths; matching them requires the comparison side to be canonical too.
- `entry` is used in three places inside `watch_thread`: `cache_dir_for(&entry)`, `watcher.watch(&entry, ...)` (twice — entry file plus parent dir), and as an argument to `run_one_cycle` which passes it to `is_relevant`. Canonicalizing once at the top fixes all three with no per-event syscall overhead.
- Options (b) "canonicalize inside `is_relevant`" and (c) "canonicalize both sides per call" require a `canonicalize` syscall on every notify event (dozens per second during a typst-watch compile burst), add error-handling surface, and leave it easy for a future caller to re-introduce the bug via a different code path.

The entry file must exist at this point — `discovery::discover` opened it during `Project::open_with_config` just before `watch::run` is called — so `canonicalize` will not fail on a missing file.

## 3. Concrete changes

### `crates/evcxr-typst/src/watch.rs`

One edit at the top of `watch_thread` (after the function signature, before `cache_dir_for`):

```rust
fn watch_thread(
    entry: PathBuf,
    root: PathBuf,
    initial_snippets: Vec<Snippet>,
    allow_eval: bool,
    shutdown_rx: crossbeam_channel::Receiver<()>,
) -> Result<(), Error> {
    // notify delivers absolute paths; canonicalize entry once so is_relevant
    // and watcher.watch see the same shape. Entry exists at this point —
    // discovery::discover opened it during Project::open_with_config.
    let entry = std::fs::canonicalize(&entry).map_err(Error::Io)?;
    let cache_dir = cache_dir_for(&entry);
    // ... rest unchanged
```

This propagates correctly to:

- `cache_dir_for(&entry)` — `entry.parent()` is absolute, so `cache_dir` is absolute. `spawn_typst_watch`'s existing `canonicalize(cache_dir)` becomes a no-op.
- `watcher.watch(&entry, RecursiveMode::NonRecursive)` — registers the absolute path. Required on Linux for inotify to deliver events for the file watch.
- `watcher.watch(parent, RecursiveMode::Recursive)` — `entry.parent()` is absolute; same requirement.
- `run_one_cycle(&entry, ...)` → `is_relevant(&event, &entry, &cache_dir)` — both sides absolute; `starts_with` matches.

`is_relevant`'s logic at lines 687–701 stays as-is; its assumptions are correct given absolute inputs.

No other files need edits. Verify (don't change):

- `cache_dir_for(&entry)` works with an absolute entry — yes, it joins `entry.parent()` with `eval::CACHE_DIRNAME`.
- `discovery::discover(&entry, &root)` inside `run_one_cycle` works with a canonical entry — yes, it passes `entry` to `typst query` as a filesystem path argument.
- `spawn_typst_watch` already canonicalizes `cache_dir` and `root` independently (see lines 715, 717).

## 4. Test that pins it

Add `crates/evcxr-typst/tests/watch_subdir_relative.rs` (keep `watch_subdir.rs` unchanged so its absolute-path coverage remains).

Header:

```rust
// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression: is_relevant must match notify's absolute event paths even when
//! the user passes a relative entry path.
//!
//! Run with `--test-threads=1`. `std::env::set_current_dir` is process-global
//! state; this test must not run concurrently with other watch tests.
```

Body outline:

1. Derive `repo_root()` from `CARGO_MANIFEST_DIR` (helper as in `watch_subdir.rs`).
2. Place fixtures inside `target/watch-relpath-test/` so `typst --root <repo>` resolves `/packages/evcxr/lib.typ`.
3. Write `target/watch-relpath-test/main.typ` (imports lib, includes `sub/chapter.typ`) and `target/watch-relpath-test/sub/chapter.typ` (an evcxr snippet with `id: "relpath-test"`).
4. `std::env::set_current_dir(&repo_root())` so the relative entry resolves correctly.
5. `entry = Path::new("target/watch-relpath-test/main.typ")` — relative path.
6. `Project::open_with_config(entry, ProjectConfig::new().with_root(&repo_root()))`.
7. `project.watch(&WatchOptions::deny())`. Wait up to 5 s for `_index.json` to appear in `target/watch-relpath-test/.evcxr-typst-cache/`.
8. Mutate `sub/chapter.typ` (change snippet id to `"relpath-test-v2"`) using the absolute path so the file write doesn't depend on cwd.
9. Poll `_index.json` for up to 3 s; assert it changes. On unpatched code `is_relevant` returns `false` for every event → cycle never runs → `_index.json` unchanged → test fails as expected. On patched code it updates.
10. Drop handle; remove `target/watch-relpath-test/`.

**Verify red-before-green:** write the test first, run `cargo test -p evcxr-typst --test watch_subdir_relative -- --test-threads=1` against the unpatched `watch_thread`, confirm it times out at step 9. Then apply the fix and confirm green.

`--test-threads=1` note: `set_current_dir` is process-global. Do not add `serial_test` as a new dep; document the requirement in the file header (existing `watch_subdir.rs` already carries the same note).

## 5. Risks and edge cases

- **`canonicalize` requires existence.** Entry exists at this point — see comment placed inline.
- **macOS `/var` ↔ `/private/var`.** `canonicalize` resolves `/var` to `/private/var`; FSEvents delivers `/private/var/...`. They match.
- **Symlinked entry file.** `canonicalize` follows the symlink; notify also reports the real file's path. They match. Edge case (notify delivering symlink path while canonicalize follows it) is too obscure to handle.
- **Windows long-path `\\?\` prefix.** `canonicalize` returns `\\?\`-prefixed paths; notify on Windows also uses extended paths. Should match. No Windows CI; flag with `// TODO(windows): verify \\?\ prefix` if relevant.
- **Relative `--root`.** `watch_thread`'s `root` may also be relative if the user passes `--root .`. `spawn_typst_watch` already canonicalizes it on its own line 715. `discovery::discover(entry, root)` accepts a relative root; typst query handles it. **Out of scope for this PR** — verify behavior is unchanged, do not expand scope.

## 6. Out of scope

- **Missing-sidecar backfill at startup** (`Plan::Noop` when `prev_snippets` matches discovery but cache is missing entries). Separate issue; will be filed as a follow-up.
- **`cargo metadata --lockfile-path` warnings.** Pre-existing noise from `ra_ap_project_model`.
- **`WatchHandle::join` rename.** API ergonomics; separate decision record.
- **Canonicalizing `root` inside `watch_thread`.** `spawn_typst_watch` already handles its own use; no broken behavior depends on it currently.
- **`--exclude` patterns** for noisy directories. Feature, not a bug fix.
