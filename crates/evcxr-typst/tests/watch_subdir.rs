// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression test: watch fires an eval cycle when a file included from the
//! entry doc is edited in a subdirectory of the entry's parent.
//!
//! Run with `--test-threads 1` (notify global state; watch tests share process).

use std::io::Write;
use std::path::PathBuf;
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
fn write_file(path: &std::path::Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut f = std::fs::File::create(path).expect("create file");
    f.write_all(content).expect("write file");
}

/// Poll `path` until its contents differ from `initial` or `deadline` elapses.
/// Returns the new contents if changed, `None` on timeout.
fn poll_until_changed(path: &std::path::Path, initial: &str, deadline: Instant) -> Option<String> {
    loop {
        if Instant::now() >= deadline {
            return None;
        }
        if let Ok(contents) = std::fs::read_to_string(path) {
            if contents != initial {
                return Some(contents);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The watch loop fires a re-eval cycle when a file in a subdirectory of the
/// entry's parent is modified.
///
/// Setup:
///   <tmp>/main.typ         — entry doc; imports and includes sub/chapter.typ
///   <tmp>/sub/chapter.typ  — contains an evcxr snippet
///
/// The tmp dir is created inside `target/` so that `typst query --root <repo>`
/// resolves the `/packages/evcxr/lib.typ` import correctly.
///
/// After the initial watch cycle writes `_index.json`, we mutate
/// `sub/chapter.typ` and assert that `_index.json` is updated within 3 s.
/// On unpatched code the event never reaches `is_relevant`, so the file
/// would not change — correctly failing the test.
#[test]
fn watch_fires_on_subdir_edit() {
    evcxr::runtime_hook();

    let root = repo_root();

    // Place the temp dir inside target/ so the repo root covers it.
    let base = root.join("target/watch-subdir-test");
    std::fs::create_dir_all(&base).expect("create base dir");

    let entry = base.join("main.typ");
    let sub_dir = base.join("sub");
    let chapter = sub_dir.join("chapter.typ");
    let cache_dir = base.join(".evcxr-typst-cache");

    // Clean up from any previous run.
    let _ = std::fs::remove_dir_all(&cache_dir);
    let _ = std::fs::remove_dir_all(&sub_dir);

    // Write main.typ — imports the evcxr package and includes the sub chapter.
    write_file(
        &entry,
        b"#import \"/packages/evcxr/lib.typ\": *\n#include \"sub/chapter.typ\"\n",
    );

    // Write sub/chapter.typ with an initial snippet.
    write_file(
        &chapter,
        b"#import \"/packages/evcxr/lib.typ\": *\n#evcxr.rust(\"println!(\\\"v1\\\");\", id: \"subdir-test\")\n",
    );

    // Open the project; skip if typst is not in PATH.
    let mut project = match Project::open_with_config(&entry, ProjectConfig::new().with_root(&root))
    {
        Err(e) => {
            eprintln!("skipping: discovery failed (typst not in PATH?): {e}");
            return;
        }
        Ok(p) => p,
    };

    // Spawn the watch loop (deny eval — we only need _index.json to update).
    let handle = match project.watch(&WatchOptions::deny()) {
        Err(e) => {
            eprintln!("skipping: watch failed to start: {e}");
            return;
        }
        Ok(h) => h,
    };

    // Wait for the initial cycle to write _index.json (up to 5 s).
    let index_path = cache_dir.join("_index.json");
    let init_deadline = Instant::now() + Duration::from_secs(5);
    let initial_index = loop {
        if Instant::now() >= init_deadline {
            drop(handle);
            panic!(
                "_index.json not written within 5 s of watch start; \
                 typst query may have failed or the cache dir is wrong"
            );
        }
        if let Ok(c) = std::fs::read_to_string(&index_path) {
            break c;
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    // Mutate sub/chapter.typ to change the snippet id — forces a different
    // discovery result and therefore a new _index.json.
    write_file(
        &chapter,
        b"#import \"/packages/evcxr/lib.typ\": *\n#evcxr.rust(\"println!(\\\"v2\\\");\", id: \"subdir-test-v2\")\n",
    );

    // Poll for _index.json to change within 3 s.
    let deadline = Instant::now() + Duration::from_secs(3);
    let updated = poll_until_changed(&index_path, &initial_index, deadline);

    // Shutdown the watch loop before asserting so the thread is cleaned up.
    drop(handle);

    assert!(
        updated.is_some(),
        "_index.json did not change within 3 s after editing sub/chapter.typ; \
         the watch loop may not be receiving events from subdirectories"
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&base);
}
