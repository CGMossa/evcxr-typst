// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression test: `is_relevant` must not accept typst-watch output files
//! (`.pdf`, `.svg`) next to the entry — they are not source edits and must not
//! trigger a Noop eval cycle.
//!
//! Root cause (issue #29): the document imports the evcxr package, so typst
//! watch monitors `_index.json` for changes. The runaway loop is:
//!
//!   1. typst watch compiles → writes `main.pdf` (next to entry)
//!   2. notify fires for `main.pdf` → `is_relevant` returns true (in entry_parent,
//!      not in cache_dir) → debounce → `run_one_cycle`
//!   3. No snippet changes → `Plan::Noop` → `_index.json` rewritten unconditionally
//!      (via write_atomically → rename, so mtime always advances)
//!   4. typst watch detects `_index.json` change (the document reads it via
//!      `json(_evcxr-cache + "/_index.json")`) → recompiles → writes `main.pdf`
//!      → back to step 1
//!
//! Fix: extension allowlist in `is_relevant` — only `.typ` and `.toml` fire cycles.
//! With the fix, step 2 rejects `.pdf` → no cycle → `_index.json` unchanged →
//! typst watch stays idle → loop stops.
//!
//! To bootstrap past the chicken-and-egg (`_index.json` must exist before typst
//! watch can compile the evcxr import, but the first cycle creates `_index.json`),
//! we pre-seed an empty `_index.json` so typst watch can compile immediately.
//!
//! Observable: count how many times `_index.json` mtime advances over a 3 s
//! window with no source edits.
//! - Unpatched: loop fires ~every 660 ms → count ≥ 1.
//! - Patched:   `.pdf` filtered → 0 advances.
//!
//! Run with `--test-threads=1` (notify global state; watch tests share process).

use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use evcxr_typst::Project;
use evcxr_typst::ProjectConfig;
use evcxr_typst::WatchOptions;

fn repo_root() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    Path::new(&manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn write_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut f = std::fs::File::create(path).expect("create file");
    f.write_all(content).expect("write file");
}

/// Count how many times `path`'s mtime advances during the observation window.
fn count_mtime_advances(path: &Path, window: Duration) -> usize {
    let deadline = Instant::now() + window;
    let mut count = 0usize;
    let mut last = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                if mtime != last {
                    count += 1;
                    last = mtime;
                }
            }
        }
    }
    count
}

/// Over a 3 s no-edit window, `_index.json` must not be rewritten.
///
/// The document imports the evcxr package (so typst-watch tracks `_index.json`
/// and recompiles when it changes), but has no snippets (so every cycle is
/// a Noop). We pre-seed `_index.json` so typst-watch can compile on startup.
///
/// Without the fix, `main.pdf` writes are accepted by `is_relevant`, triggering:
///   pdf write → is_relevant → noop → _index.json rewritten →
///   typst-watch detects _index.json change → recompiles → pdf write → …
/// With the fix, `.pdf` is filtered → no spurious cycles → count = 0.
#[test]
fn no_spurious_noop_cycles_during_idle_watch() {
    evcxr::runtime_hook();

    let root = repo_root();

    let base = root.join("target/watch-no-noop-test");
    std::fs::create_dir_all(&base).expect("create base dir");

    let entry = base.join("main.typ");
    let cache_dir = base.join(".evcxr-typst-cache");

    // Clean up from any previous run but keep a fresh state.
    let _ = std::fs::remove_dir_all(&cache_dir);
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");

    // Document imports the evcxr package (so typst-watch reads _index.json and
    // will recompile when it changes), but has no snippets (every cycle = Noop).
    write_file(
        &entry,
        b"#import \"/packages/evcxr/lib.typ\": *\n= No-noop Regression\n\nPlain content.\n",
    );

    // Pre-seed _index.json so typst-watch can compile the evcxr import on
    // startup (it reads _index.json synchronously during compilation).
    let index_path = cache_dir.join("_index.json");
    write_file(&index_path, b"{\"v\":1,\"available\":[]}");

    let mut project = match Project::open_with_config(&entry, ProjectConfig::new().with_root(&root))
    {
        Err(e) => {
            eprintln!("skipping: discovery failed (typst not in PATH?): {e}");
            return;
        }
        Ok(p) => p,
    };

    let handle = match project.watch(&WatchOptions::deny()) {
        Err(e) => {
            eprintln!("skipping: watch failed to start: {e}");
            return;
        }
        Ok(h) => h,
    };

    // Give typst-watch time to compile main.typ (it reads _index.json which
    // now exists) and write main.pdf. On unpatched code, that pdf write
    // triggers is_relevant → noop cycle → _index.json rewritten, starting
    // the runaway loop. Allow up to 4 s for the first recompile cycle.
    std::thread::sleep(Duration::from_secs(4));

    // Observe for 3 s with no source edits.
    // On unpatched code: pdf writes keep triggering Noop cycles, each of which
    // rewrites _index.json via write_atomically (rename → mtime always advances).
    // On patched code: .pdf filtered → no spurious cycles → count = 0.
    let advance_count = count_mtime_advances(&index_path, Duration::from_secs(3));

    drop(handle);

    assert_eq!(
        advance_count, 0,
        "_index.json mtime advanced {advance_count} time(s) during a 3 s idle \
         window — typst-watch output files (.pdf/.svg) are not being filtered \
         by is_relevant; see issue #29"
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&base);
}
