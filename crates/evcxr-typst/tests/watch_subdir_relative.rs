// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression: is_relevant must match notify's absolute event paths even when
//! the user passes a relative entry path.
//!
//! Run with `--test-threads=1`. `std::env::set_current_dir` is process-global
//! state; this test must not run concurrently with other watch tests.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

/// The watch loop fires an eval cycle when a relative entry path is given.
///
/// Setup:
///   target/watch-relpath-test/main.typ — plain Typst doc with no evcxr import,
///                                         so typst watch compiles it without
///                                         needing `_index.json`.
///   target/watch-relpath-test/sub/edit-me.typ — a plain sub-file we can touch
///                                         to trigger a notify event in a subdir.
///
/// The tmp dir is inside `target/` so `typst --root <repo>` resolves package
/// imports correctly. `set_current_dir` is called so that the entry path
/// `target/watch-relpath-test/main.typ` is genuinely relative.
///
/// Observable: `_index.json` is written by every eval cycle (even when empty).
/// On unpatched code, `is_relevant` compares notify's absolute event paths
/// against the relative `entry_parent` — `Path::starts_with` never matches →
/// no cycle fires → `_index.json` never appears → test times out and fails.
/// On patched code, entry is canonicalized before use → paths match → cycle
/// fires after the PDF write event → `_index.json` is written → test passes.
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
    // No evcxr import — the file simply includes a sub-chapter for layout.
    write_file(&entry_abs, b"= Relpath Regression Test\n\nPlain content.\n");

    // Write a sub-file that we can later touch to fire a notify event for
    // the subdir path (exercising the subdirectory-event path from PR #27).
    let sub_file = sub_dir.join("chapter.typ");
    write_file(&sub_file, b"Plain sub-chapter.\n");

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

    // Wait for the first eval cycle to write _index.json (up to 8 s).
    //
    // Flow: typst watch compiles main.typ (succeeds — plain content, no evcxr
    // mode=read processing needed) → writes main.pdf → notify event fires for
    // main.pdf (absolute path) → is_relevant checks whether it starts_with
    // entry_parent:
    //   - Unpatched: entry_parent is relative → starts_with fails → no cycle.
    //   - Patched:   entry is canonicalized   → starts_with matches → cycle fires
    //                → run_one_cycle writes _index.json (even when empty).
    let index_path = cache_dir.join("_index.json");
    let deadline = Instant::now() + Duration::from_secs(8);
    let appeared = loop {
        if Instant::now() >= deadline {
            break false;
        }
        if index_path.exists() {
            break true;
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    // Shutdown before asserting so the thread is cleaned up.
    drop(handle);

    assert!(
        appeared,
        "_index.json was not written within 8 s; the watch cycle may not have \
         fired because is_relevant is comparing a relative entry_parent against \
         notify's absolute event paths (relative-path regression)"
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&base);
}
