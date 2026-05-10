// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression test: watch must backfill missing sidecars on cold-cache startup.
//!
//! Root cause (issue #30): `Project::open_with_config` runs `discover` once and
//! stores the result as `initial_snippets`. `watch_thread` assigns them to
//! `prev_snippets`. The startup cycle then calls `run_one_cycle`, which runs
//! `discover` again (nothing changed on disk), so `curr == prev`. `classify`
//! returns `Plan::Noop`. The `Noop` arm did nothing. The index was written with
//! `available: []`. Subsequent cycles also returned `Noop` (no source change).
//! Result: cold-cache watch never produced sidecars.
//!
//! Fix: in the `Plan::Noop` arm, filter `curr` for evaluable snippets whose
//! `<id>.manifest.json` is absent, and evaluate them as an implicit backfill.
//!
//! Observable: after the startup cycle completes, both `backfill-1.manifest.json`
//! and `backfill-2.manifest.json` must exist in the cache dir.
//!
//! Poll timeout is 120 s to tolerate first cold-compile latency; warm CAS hits
//! materialise in milliseconds.
//!
//! Run with `--test-threads=1` (notify global state; watch tests share process).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use evcxr_typst::{Project, ProjectConfig, WatchOptions};

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
    std::fs::write(path, content).expect("write file");
}

/// Poll until `path` exists or `deadline` passes. Returns `true` if the file appeared.
fn poll_until_exists(path: &Path, deadline: Instant) -> bool {
    loop {
        if path.exists() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// On a cold cache, `watch --allow-eval` must backfill both snippet sidecars
/// without requiring a source-file edit to trigger the first cycle.
///
/// Poll timeout is 120 s: first cold compile can take 60–90 s depending on the
/// machine and sccache state. Warm CAS hits materialise in milliseconds.
#[test]
fn watch_backfills_sidecars_on_cold_cache() {
    evcxr::runtime_hook();

    let root = repo_root();
    let base = root.join("target/watch-sidecar-backfill-test");
    std::fs::create_dir_all(&base).expect("create base dir");

    let entry = base.join("main.typ");
    let cache_dir = base.join(".evcxr-typst-cache");

    // Ensure cold cache.
    let _ = std::fs::remove_dir_all(&cache_dir);

    // Two evaluable snippets with explicit IDs.
    // `import *` exports `rust` directly (not `evcxr.rust`).
    write_file(
        &entry,
        b"#import \"/packages/evcxr/lib.typ\": *\n\
          #rust(\"println!(\\\"backfill-1\\\");\", id: \"backfill-1\")\n\
          #rust(\"println!(\\\"backfill-2\\\");\", id: \"backfill-2\")\n",
    );

    let mut project = match Project::open_with_config(&entry, ProjectConfig::new().with_root(&root))
    {
        Err(e) => {
            eprintln!("skipping: discovery failed (typst not in PATH?): {e}");
            return;
        }
        Ok(p) => p,
    };

    let handle = match project.watch(&WatchOptions::allow_eval()) {
        Err(e) => {
            eprintln!("skipping: watch failed to start: {e}");
            return;
        }
        Ok(h) => h,
    };

    // 120 s timeout: first cold compile can take 60–90 s.
    let deadline = Instant::now() + Duration::from_secs(120);

    let manifest1 = cache_dir.join("backfill-1.manifest.json");
    let manifest2 = cache_dir.join("backfill-2.manifest.json");

    let ok1 = poll_until_exists(&manifest1, deadline);
    let ok2 = poll_until_exists(&manifest2, deadline);

    drop(handle);

    assert!(
        ok1,
        "backfill-1.manifest.json not written within 60 s; \
         watch did not backfill sidecars on cold-cache startup (issue #30)"
    );
    assert!(
        ok2,
        "backfill-2.manifest.json not written within 60 s; \
         watch did not backfill sidecars on cold-cache startup (issue #30)"
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&base);
}
