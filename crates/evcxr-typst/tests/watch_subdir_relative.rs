// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression: is_relevant must match notify's absolute event paths even when
//! the user passes a relative entry path.
//!
//! Two observation steps:
//!
//! 1. First cycle (entry-dir event): typst watch compiles main.typ and writes
//!    main.pdf → notify fires for main.pdf (in entry_parent) → is_relevant
//!    matches → cycle fires → _index.json is written.
//!
//! 2. Second cycle (subdir event): we mutate sub/chapter.typ (NOT included in
//!    main.typ, so typst does not recompile) → notify fires for
//!    <abs>/sub/chapter.typ → is_relevant checks path.starts_with(entry_parent):
//!      - Patched:   entry is canonicalized → entry_parent is absolute → matches
//!                   → cycle fires → _index.json mtime advances.
//!      - Unpatched: entry_parent is relative → absolute subdir path never
//!                   starts_with a relative prefix → no cycle → mtime stays
//!                   constant → assertion fails.
//!    This step pins the `path.starts_with(entry_parent)` code path for
//!    subdirectory files specifically — a regression that broke subdir delivery
//!    without breaking entry-dir delivery would be caught here.
//!
//! Run with `--test-threads=1`. `std::env::set_current_dir` is process-global
//! state; this test must not run concurrently with other watch tests.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use evcxr_typst::{Project, ProjectConfig, WatchOptions};

fn repo_root() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    std::path::Path::new(&manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Write bytes to a file, creating parent dirs as needed.
fn write_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut f = std::fs::File::create(path).expect("create file");
    f.write_all(content).expect("write file");
}

/// Poll `path` until its mtime differs from `before` or `deadline` elapses.
/// Returns `true` if mtime advanced, `false` on timeout.
fn poll_until_mtime_changes(path: &Path, before: SystemTime, deadline: Instant) -> bool {
    loop {
        if Instant::now() >= deadline {
            return false;
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                if mtime != before {
                    return true;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The watch loop fires an eval cycle when a relative entry path is given,
/// and fires a second cycle when a file in a subdirectory is mutated.
///
/// Setup:
///   target/watch-relpath-test/main.typ — plain Typst doc (no evcxr import,
///                                         no #include of the sub-file) so
///                                         typst watch compiles it without
///                                         needing `_index.json` and does NOT
///                                         recompile when sub/chapter.typ changes.
///   target/watch-relpath-test/sub/chapter.typ — plain sub-file; mutating it
///                                         generates a notify event for an
///                                         absolute subdir path without causing
///                                         typst to recompile. This isolates
///                                         the subdir-event path from the
///                                         entry-dir PDF-event path.
///
/// The tmp dir is inside `target/` so `typst --root <repo>` resolves package
/// imports correctly. `set_current_dir` is called so that the entry path
/// `target/watch-relpath-test/main.typ` is genuinely relative.
///
/// Observable (step 1): `_index.json` is written by every eval cycle.
/// Observable (step 2): `_index.json` mtime advances when the second cycle
/// fires (run_one_cycle always calls write_available_index_for_snippets at the
/// end, which rewrites the file unconditionally via write_atomically → rename).
#[test]
fn watch_fires_on_subdir_edit_relative_entry() {
    evcxr::runtime_hook();

    let root = repo_root();

    // set_current_dir so the entry path is genuinely relative.
    std::env::set_current_dir(&root).expect("set_current_dir to repo root");

    let base = root.join("target/watch-relpath-test");
    std::fs::create_dir_all(&base).expect("create base dir");

    // Absolute paths for file I/O; relative entry for the project.
    let entry_abs = base.join("main.typ");
    let sub_dir = base.join("sub");
    let cache_dir = base.join(".evcxr-typst-cache");

    // Clean up from any previous run.
    let _ = std::fs::remove_dir_all(&cache_dir);
    let _ = std::fs::remove_dir_all(&sub_dir);

    // Write a plain main.typ that typst can compile without `_index.json`.
    // Deliberately does NOT include sub/chapter.typ so that typst watch does
    // not recompile when the sub-file is mutated. This ensures the only notify
    // event from the sub-file mutation is for the subdir path itself, not for
    // a PDF rewrite — isolating the path.starts_with(entry_parent) code path
    // for subdirectory files.
    write_file(&entry_abs, b"= Relpath Regression Test\n\nPlain content.\n");

    // Write a sub-file that we can later mutate to fire a notify event for
    // a subdirectory path, exercising the path.starts_with(entry_parent) check
    // in is_relevant for subdir files specifically.
    let sub_file = sub_dir.join("chapter.typ");
    write_file(&sub_file, b"Plain sub-chapter v1.\n");

    // Relative entry — the key difference vs watch_subdir.rs.
    let entry_rel = Path::new("target/watch-relpath-test/main.typ");

    // Open project with relative entry.
    let mut project =
        match Project::open_with_config(entry_rel, ProjectConfig::new().with_root(&root)) {
            Err(e) => {
                eprintln!("skipping: discovery failed (typst not in PATH?): {e}");
                return;
            }
            Ok(p) => p,
        };

    // Spawn the watch loop (deny eval — we only need _index.json to be written).
    let handle = match project.watch(&WatchOptions::deny()) {
        Err(e) => {
            eprintln!("skipping: watch failed to start: {e}");
            return;
        }
        Ok(h) => h,
    };

    // ── Step 1: Wait for the first eval cycle to write _index.json (up to 8 s).
    //
    // Flow: typst watch compiles main.typ (succeeds — plain content, no evcxr
    // mode=read processing needed) → writes main.pdf → notify event fires for
    // main.pdf (absolute path) → is_relevant checks whether it starts_with
    // entry_parent:
    //   - Unpatched: entry_parent is relative → starts_with fails → no cycle.
    //   - Patched:   entry is canonicalized   → starts_with matches → cycle fires
    //                → run_one_cycle writes _index.json (even when empty).
    let index_path = cache_dir.join("_index.json");
    let step1_deadline = Instant::now() + Duration::from_secs(8);
    let index_mtime_after_step1 = loop {
        if Instant::now() >= step1_deadline {
            drop(handle);
            panic!(
                "_index.json was not written within 8 s (step 1); the watch cycle may not have \
                 fired because is_relevant is comparing a relative entry_parent against \
                 notify's absolute event paths (relative-path regression)"
            );
        }
        if let Ok(meta) = std::fs::metadata(&index_path) {
            if let Ok(mtime) = meta.modified() {
                break mtime;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    // ── Step 2: Mutate sub/chapter.typ and wait for a second cycle (up to 5 s).
    //
    // sub/chapter.typ is NOT included in main.typ, so typst watch does not
    // recompile. The only notify event is for the absolute subdir path
    // <base>/sub/chapter.typ. On patched code, is_relevant canonicalized
    // entry → entry_parent is <abs-base> → starts_with matches → cycle fires
    // → _index.json mtime advances. On unpatched, entry_parent is the relative
    // string "target/watch-relpath-test" → absolute subdir path never
    // starts_with that prefix → no cycle → mtime stays constant → test fails.
    write_file(&sub_file, b"Plain sub-chapter v2.\n");

    let step2_deadline = Instant::now() + Duration::from_secs(5);
    let subdir_cycle_fired =
        poll_until_mtime_changes(&index_path, index_mtime_after_step1, step2_deadline);

    // Shutdown before asserting so the thread is cleaned up.
    drop(handle);

    assert!(
        subdir_cycle_fired,
        "_index.json mtime did not advance within 5 s after mutating sub/chapter.typ \
         (step 2); the subdir notify event was not delivered to is_relevant, which \
         means path.starts_with(entry_parent) failed for a subdirectory path — \
         likely because entry_parent is still relative instead of canonicalized"
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&base);
}
