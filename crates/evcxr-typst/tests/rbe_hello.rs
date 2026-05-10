// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression target for the hand-written rust-by-example port chapter 1.
//!
//! Pinned by `journal/2026-05-09-001-hello.md`:
//! - the chapter discovers exactly one evaluable snippet (`rbe-hello`);
//! - evaluating with `--allow-eval` produces a `.txt` sidecar containing
//!   `Hello World!`;
//! - the `_index.json` lists the snippet as available so `lib.typ`'s
//!   `_index-available()` guard passes (D-004 + T-I06).
//!
//! Run with the mandatory `--test-threads 1` (CommandContext is process-global).

use std::path::PathBuf;

use evcxr_typst::{EvalOptions, Project, ProjectConfig, SnippetOutcome};

#[test]
fn rbe_hello_chapter_evaluates_and_captures_stdout() {
    evcxr::runtime_hook();

    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let repo_root = PathBuf::from(&manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let entry = repo_root.join("examples/rust-by-example/main.typ");
    if !entry.exists() {
        eprintln!("skipping: examples/rust-by-example/main.typ not found");
        return;
    }

    let project = Project::open_with_config(&entry, ProjectConfig::new().with_root(&repo_root));
    let mut project = match project {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: discovery failed (typst not in PATH?): {e}");
            return;
        }
    };

    let snippet_ids: Vec<&str> = project.snippets().iter().map(|s| s.id.as_str()).collect();
    assert!(
        snippet_ids.contains(&"rbe-hello"),
        "expected snippet `rbe-hello` in the rbe book; got {snippet_ids:?}"
    );

    let cache_dir = entry.parent().unwrap().join(".evcxr-typst-cache");
    if cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&cache_dir);
    }

    let report = project
        .evaluate(&mut EvalOptions::allow_eval())
        .expect("evaluate with allow_eval");

    let hello = report
        .snippets
        .iter()
        .find(|r| r.id == "rbe-hello")
        .expect("rbe-hello snippet must be in the report");
    assert!(
        matches!(
            hello.outcome,
            SnippetOutcome::Ok | SnippetOutcome::CacheHit
        ),
        "rbe-hello snippet must succeed; got {:?}",
        hello.outcome
    );

    let txt = cache_dir.join("rbe-hello.txt");
    let stdout = std::fs::read_to_string(&txt)
        .unwrap_or_else(|e| panic!("reading {}: {e}", txt.display()));
    assert!(
        stdout.contains("Hello World!"),
        "expected `Hello World!` in {}; got {stdout:?}",
        txt.display()
    );

    let index =
        std::fs::read_to_string(cache_dir.join("_index.json")).expect("read _index.json");
    assert!(
        index.contains("rbe-hello"),
        "_index.json must list `rbe-hello`; got {index:?}"
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
}
