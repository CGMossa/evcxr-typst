# Plan: watch picks up edits in entry-parent subdirectories

**Date:** 2026-05-09  
**Branch:** fix/watch-recursive  
**Symptom:** `evcxr-typst watch` never fires an eval cycle when a file included from the entry doc (e.g. `hello/comment.typ`) lives in a subdirectory of the entry's parent. Typst-watch sees the change and recompiles, but the evcxr sidecar is never updated.

---

## 1. Root cause

Two layers cooperate to produce the miss:

**Layer 1 — watcher scope (watch.rs:105-110).**  
`watch_thread` registers the notify watcher on exactly two paths:

```rust
watcher.watch(&entry, RecursiveMode::NonRecursive)           // line 106
watcher.watch(parent, RecursiveMode::NonRecursive)            // line 109
```

`NonRecursive` on `parent` means the OS delivers events only for files directly inside `parent/`, not for files in any subdirectory of `parent/`. A write to `parent/hello/comment.typ` produces no event on this watcher.

**Layer 2 — relevance filter (watch.rs:672-683).**  
Even if an event from a subdirectory path were somehow delivered (e.g. on a platform that collapses events), `is_relevant` would drop it:

```rust
if path == entry || path.parent() == entry.parent() {
    return true;
}
```

`path.parent()` for `examples/rust-by-example/hello/comment.typ` is `examples/rust-by-example/hello`, which is not equal to `examples/rust-by-example` (= `entry.parent()`). The predicate returns `false`.

Together: subdirectory files are both unwatched and un-recognisable if events did arrive.

---

## 2. Proposed fix — option (a): recursive watch on entry's parent

Switch `RecursiveMode::NonRecursive` → `RecursiveMode::Recursive` for the parent-directory watch, and broaden `is_relevant` to accept any path that is a descendant of the entry's parent while still excluding the cache dir.

**Justification over (b) and (c):**

- (a) is the smallest possible change: two lines in the watcher setup, four lines in `is_relevant`. No coupling to discovery output, no re-registration on each cycle.
- (b) requires storing the set of included files, re-registering watchers after every cycle (discovery output changes), handling removed files gracefully, and risks inotify watch-limit exhaustion on large projects.
- (c) adds hybrid complexity without meaningfully reducing (a)'s over-fire surface, because `is_relevant` already debounces via the 150 ms window and a full cycle is cheap (just `typst query`).

The main downside of (a) is noise from unrelated files under the entry's parent directory (e.g. `.git/`, generated artefacts). The cache dir exclusion already handles our own writes; `.git/` events will fire a `typst query` that returns the same snippets, producing a `Plan::Noop` — the only cost is one extra query per noisy event.  For real-world projects the entry's parent is typically the document root and its subtree is mostly `.typ` files anyway.

---

## 3. Concrete changes

### `crates/evcxr-typst/src/watch.rs`

**Change 1 — watch.rs:109**: switch to `RecursiveMode::Recursive`.

Current (lines 108-110):
```rust
if let Some(parent) = entry.parent().filter(|p| p != &Path::new("")) {
    let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
}
```

Replacement:
```rust
if let Some(parent) = entry.parent().filter(|p| p != &Path::new("")) {
    let _ = watcher.watch(parent, RecursiveMode::Recursive);
}
```

One-word change: `NonRecursive` → `Recursive`.

**Change 2 — watch.rs:672-683**: broaden `is_relevant` to accept any descendant of the entry's parent (excluding the cache dir, which is already handled).

Current:
```rust
fn is_relevant(event: &Event, entry: &Path, cache_dir: &Path) -> bool {
    for path in &event.paths {
        if path.starts_with(cache_dir) {
            return false;
        }
        if path == entry || path.parent() == entry.parent() {
            return true;
        }
    }
    false
}
```

Replacement:
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
        if path.starts_with(entry_parent) {
            return true;
        }
    }
    false
}
```

Key change: `path.parent() == entry.parent()` → `path.starts_with(entry_parent)`. The `continue` (not `return false`) on the cache-dir check is intentional: a single event can list multiple paths; only skip the cache-dir paths, not the whole event.

No other files need to change.

---

## 4. Test that pins it

Create `crates/evcxr-typst/tests/watch_subdir.rs`.  
This test does **not** use `CommandContext` (no eval), so `--test-threads 1` constraint applies only because all watch tests share the same process and the notify crate has global state on some platforms. Add the note to the test header anyway.

```
// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression test: watch fires an eval cycle when a file included from the
//! entry doc is edited in a subdirectory of the entry's parent.
//! Run with --test-threads 1 (notify global state, watch tests share process).
```

**Setup:** create a temp dir that lives inside the repo tree so the `typst query --root <repo>` invocation can resolve the local `lib.typ` import. Pattern from `min_cli_enforcement.rs`: derive `repo_root()` from `CARGO_MANIFEST_DIR`.

```
<tmp>/
  main.typ          # #import "/packages/evcxr/lib.typ": *
                    # #include "sub/chapter.typ"
  sub/
    chapter.typ     # #import "/packages/evcxr/lib.typ": *
                    # #evcxr.rust("println!(\"v1\");", id: "subdir-test")
```

**Spawn watch (no eval needed):** call `Project::watch` with `WatchOptions::default()` (eval denied). The watch loop still runs `run_one_cycle` → `discovery::discover` on each file event; the test only needs to observe that a cycle fires, not that any sidecar is written.

However, since `WatchOptions::default()` does not produce a sidecar, the observable signal is: after mutating `sub/chapter.typ`, the watch loop must emit at least one `tracing` debug line (not reliable in test), OR the test can assert via `_index.json`. On the deny-eval path, `run_one_cycle` still calls `eval::write_available_index_for_snippets` at line 280, which updates `_index.json`. So:

1. Spawn `Project::watch(&WatchOptions::default())` — returns `WatchHandle`.
2. Give the loop 300 ms to settle (one initial cycle will have written `_index.json`).
3. Mutate `sub/chapter.typ` (change snippet id or src text) to force a different snippet set.
4. Poll `_index.json` for up to 3 s with 100 ms sleeps; assert its contents change to reflect the new snippet id.
5. Call `handle.join()` (which sends shutdown and waits).

If `_index.json` does not change within 3 s on the patched code, the test fails. On unpatched code, the file event never reaches `is_relevant`, so `_index.json` never changes: this would correctly fail before the fix.

**Alternative observable (simpler):** if the above proves fragile, the test can instead directly call `is_relevant` (make it `pub(crate)` or test-only `#[cfg(test)]`) with a crafted `Event` whose path is in a subdirectory, assert it returns `true`. This is a pure unit test and does not require spawning the watch loop. The integration test above is preferred because it pins the end-to-end path, but if the spawning proves unreliable (timing, PATH), the unit test is the reliable fallback.

The test file should use `tempfile::TempDir` (already a dev-dependency if present, otherwise add to `[dev-dependencies]` in `crates/evcxr-typst/Cargo.toml`).

---

## 5. Risks / edge cases

**Cache dir loop.** The cache dir (`<parent>/.evcxr-typst-cache/`) is a descendant of `entry_parent`. The `path.starts_with(cache_dir)` check in `is_relevant` uses `continue` (skip this path), not `return false` (skip the whole event). If an event lists only cache-dir paths, no other path will match `path.starts_with(entry_parent)` and the function returns `false` — correct. If an event lists a mix of cache-dir and non-cache paths (unlikely but possible on some backends), the non-cache path will still be evaluated — also correct.

**`.git/` and other tool noise under entry's parent.** A recursive watch on a directory that contains `.git/` will receive events from every `git` operation. Each such event fires a `typst query` (one subprocess). On a busy repo this could increase CPU use, but `typst query` is fast (~40 ms), and events arrive only when the user is actively committing or pulling — not a steady-state cost. The `Plan::Noop` path (classification) is O(n snippets) with no eval; acceptable.

**`target/` if entry is inside the crate.** If the entry file lives under a crate, `target/` is a descendant of (or sibling to) the entry's parent. `target/` can produce thousands of events during `cargo build`. Mitigation: the typical usage of `evcxr-typst watch` puts the entry file in an `examples/` or `docs/` directory, not at the crate root. The design doc (watch-loop.md §3) already notes this as a known noise source. Documenting it as a known limitation is sufficient for this PR; a `--exclude` flag is out of scope.

**Symlinked included files.** If `sub/chapter.typ` is a symlink to a file outside `entry_parent`, editing the target file produces an event on the target's real path — which is outside `entry_parent`. `path.starts_with(entry_parent)` returns `false` → no cycle fires. This is the same behaviour as before the fix (symlinked files were also invisible to the old `NonRecursive` watcher) and is therefore not a regression. Document as a known limitation; resolving it would require tracking include paths from the discovery output (approach (b)).

**Linux inotify watch limits.** Option (a) registers exactly one `Recursive` watch on `entry_parent`. The kernel counts recursive watches as a single inotify instance consuming one watch descriptor per subdirectory traversed at registration time. For large trees (thousands of directories) this can hit `/proc/sys/fs/inotify/max_user_watches` (default 8192 on many distros). For typical `evcxr-typst` projects (a few dozen `.typ` files in a handful of subdirectories) this is not a concern. Option (b) would register one watch per included file, which is strictly fewer descriptors — but it comes with the re-registration complexity detailed in §2.

**macOS FSEvents coalescing.** On macOS, `notify` uses FSEvents, which can coalesce rapid writes into a single event with multiple paths. The updated `is_relevant` handles multi-path events correctly (loops over all paths).

---

## 6. Out of scope

- **`WatchHandle::join` rename** (`WatchHandle::stop_and_wait` or similar). Tracked as a follow-up in `journal/2026-05-09-002-watch-loop-exits-immediately.md`. Pure ergonomic/API change; requires a decision record and a separate PR. Do not touch in this fix.
- **`cargo metadata --lockfile-path` warnings.** Benign noise from `ra_ap_project_model`; pre-existing, unrelated to this bug. Tracked separately.
- **`--exclude` patterns for noisy directories** (`.git/`, `target/`). Useful feature, but depends on design decisions about configuration format. Out of scope for a surgical bug fix.
- **Symlink resolution for included files.** Would require coupling the watcher to discovery output (approach (b)); deferred.
