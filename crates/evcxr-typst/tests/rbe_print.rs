// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Regression target for the hand-written rust-by-example port chapter
//! `hello/print.typ` (formatted print).
//!
//! Pinned by `journal/2026-05-10-001-print.md`:
//! - the chapter discovers the `rbe-hello-print` snippet;
//! - evaluating with `--allow-eval` produces a `.txt` sidecar containing
//!   the canonical lines from upstream (positional args, base-N formatting,
//!   width/pad, the FIXED `Bond, James Bond` line, the implicit-named-arg
//!   final line);
//! - `_index.json` lists the snippet so `lib.typ`'s `_index-available`
//!   guard passes (D-004 + T-I06).
//!
//! Run with the mandatory `--test-threads 1` (CommandContext is process-global).

use std::path::PathBuf;

use evcxr_typst::{EvalOptions, Project, ProjectConfig, SnippetOutcome};

#[test]
fn rbe_print_chapter_evaluates_and_captures_formatted_output() {
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
        snippet_ids.contains(&"rbe-hello-print"),
        "expected snippet `rbe-hello-print` in the rbe book; got {snippet_ids:?}"
    );

    let cache_dir = entry.parent().unwrap().join(".evcxr-typst-cache");
    if cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&cache_dir);
    }

    let report = project
        .evaluate(&mut EvalOptions::allow_eval())
        .expect("evaluate with allow_eval");

    let print = report
        .snippets
        .iter()
        .find(|r| r.id == "rbe-hello-print")
        .expect("rbe-hello-print snippet must be in the report");
    assert!(
        matches!(
            print.outcome,
            SnippetOutcome::Ok | SnippetOutcome::CacheHit
        ),
        "rbe-hello-print snippet must succeed; got {:?}",
        print.outcome
    );

    let txt = cache_dir.join("rbe-hello-print.txt");
    let stdout = std::fs::read_to_string(&txt)
        .unwrap_or_else(|e| panic!("reading {}: {e}", txt.display()));

    for needle in [
        "31 days",
        "Alice, this is Bob. Bob, this is Alice",
        "the quick brown fox jumps over the lazy dog",
        "Base 10:               69420",
        "Base 2 (binary):       10000111100101100",
        "Base 8 (octal):        207454",
        "Base 16 (hexadecimal): 10f2c",
        "00001",
        "10000",
        "My name is Bond, James Bond",
    ] {
        assert!(
            stdout.contains(needle),
            "expected `{needle}` in {}; got {stdout:?}",
            txt.display()
        );
    }

    let index =
        std::fs::read_to_string(cache_dir.join("_index.json")).expect("read _index.json");
    assert!(
        index.contains("rbe-hello-print"),
        "_index.json must list `rbe-hello-print`; got {index:?}"
    );

    let _ = std::fs::remove_dir_all(&cache_dir);
}
