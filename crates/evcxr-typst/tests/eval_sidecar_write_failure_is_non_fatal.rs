// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! When `write_mime_sidecars` fails mid-eval, the eval loop must continue and
//! the affected snippet must be omitted from `_index.json` so `lib.typ` falls
//! through to the placeholder for it (D-004) — not break the whole run.
//!
//! Mechanism: pre-create a *non-empty directory* at the path
//! `write_atomically` would rename its `<id>.tmp` file onto. `fs::rename`
//! over a non-empty directory fails with `ENOTEMPTY`, so the sidecar write
//! returns Err. With `?` propagation (the bug), `evaluate()` would return
//! Err and `_index.json` would never be written. After the fix, `evaluate()`
//! returns Ok, `_index.json` exists, and the obstructed snippet is absent
//! from its `available` list.

use std::fs;
use std::process::Command;

use evcxr_typst::{EvalOptions, Project, ProjectConfig, SnippetOutcome};

#[test]
fn evaluate_continues_when_sidecar_write_fails() {
    evcxr::runtime_hook();

    if Command::new("typst").arg("--version").output().is_err() {
        eprintln!("skipping: typst not in PATH");
        return;
    }

    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let repo_root = std::path::PathBuf::from(&manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let pkg_lib = repo_root.join("packages/evcxr/lib.typ");
    if !pkg_lib.exists() {
        eprintln!("skipping: {} not found", pkg_lib.display());
        return;
    }

    // Create the tempdir *inside* the repo root so Typst can resolve a
    // relative import to packages/evcxr/lib.typ under --root repo_root.
    // A tempdir outside repo_root would cause Typst to reroute absolute
    // import paths as <tmp-root>/… which doesn't exist (the silent-skip bug).
    let work = tempfile::TempDir::new_in(&repo_root).expect("tempdir in repo root");
    let entry = work.path().join("main.typ");

    // Compute relative path from the tempdir to packages/evcxr/lib.typ.
    // tempdir is one level below repo_root, so "../packages/evcxr/lib.typ".
    let rel_lib = "../packages/evcxr/lib.typ";

    fs::write(
        &entry,
        format!(
            "#import \"{rel_lib}\" as evcxr\n\
             #evcxr.setup()\n\
             #evcxr.rust(id: \"sidecar-block\", ```rust\nprintln!(\"hello\");\n```)\n",
        ),
    )
    .expect("write entry");

    // Pre-create the cache dir and obstruct the sidecar write target with a
    // non-empty directory so `fs::rename` fails with ENOTEMPTY.
    let cache_dir = work.path().join(".evcxr-typst-cache");
    fs::create_dir_all(&cache_dir).expect("mkdir cache");
    let blocker = cache_dir.join("sidecar-block.txt");
    fs::create_dir_all(&blocker).expect("mkdir blocker");
    fs::write(blocker.join("placeholder"), b"x").expect("non-empty marker");

    let project = Project::open_with_config(&entry, ProjectConfig::new().with_root(&repo_root));
    let mut project = match project {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: discovery failed (typst not in PATH?): {e}");
            return;
        }
    };

    let report = project
        .evaluate(&mut EvalOptions::allow_eval())
        .expect("evaluate must return Ok despite sidecar-write failure");

    let snippet = report
        .snippets
        .iter()
        .find(|r| r.id == "sidecar-block")
        .expect("sidecar-block snippet must be in the report");
    assert!(
        matches!(snippet.outcome, SnippetOutcome::Ok),
        "snippet evaluation itself should succeed; outcome was {:?}",
        snippet.outcome,
    );
    assert!(
        snippet.mime_sidecars.is_empty(),
        "no sidecars should be reported when the write failed",
    );

    let index_path = cache_dir.join("_index.json");
    assert!(
        index_path.exists(),
        "_index.json must be written even when a snippet's sidecar write failed",
    );
    let index = fs::read_to_string(&index_path).expect("read _index.json");
    assert!(
        !index.contains("sidecar-block"),
        "snippet whose sidecar write failed must be absent from _index.json; got {index:?}",
    );
}
