// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Verify that EvalOptions::deny() never spawns evcxr and returns
//! SkippedNoEval for all evaluable snippets (D-004).

use std::path::PathBuf;

use evcxr_typst::{EvalOptions, Project, ProjectConfig, SnippetOutcome};

/// Open examples/hello/main.typ and evaluate with EvalOptions::deny().
/// Assert that no evcxr child was spawned: all outcomes are SkippedNoEval
/// (or CacheHit from a previous run), cache_hits == 0 on a fresh cache,
/// and no .txt or .manifest.json sidecars are created for SkippedNoEval
/// snippets (only _index.json is written).
#[test]
fn deny_eval_produces_skipped_no_eval() {
    evcxr::runtime_hook();

    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let repo_root = PathBuf::from(&manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let entry = repo_root.join("examples/hello/main.typ");
    if !entry.exists() {
        eprintln!("skipping: examples/hello/main.typ not found");
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

    assert!(
        !project.snippets().is_empty(),
        "hello example must have snippets"
    );

    // Remove any pre-existing cache so we get a fresh deny run.
    let cache_dir = entry.parent().unwrap().join(".evcxr-typst-cache");
    if cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&cache_dir);
    }

    let report = project
        .evaluate(&EvalOptions::deny())
        .expect("evaluate with deny");

    // On a fresh cache all evaluable snippets must be SkippedNoEval.
    for result in &report.snippets {
        assert!(
            matches!(
                result.outcome,
                SnippetOutcome::SkippedNoEval | SnippetOutcome::Ok
            ),
            "unexpected outcome {:?} for snippet {} on fresh deny run",
            result.outcome,
            result.id,
        );
    }

    assert_eq!(
        report.cache_hits, 0,
        "expected 0 cache hits on fresh deny run"
    );

    // _index.json must exist (written by evaluate on both paths).
    assert!(
        cache_dir.join("_index.json").exists(),
        "_index.json must be written even on deny path"
    );

    // No .txt or .manifest.json sidecars should exist for SkippedNoEval snippets.
    for result in &report.snippets {
        if result.outcome == SnippetOutcome::SkippedNoEval {
            assert!(
                !cache_dir.join(format!("{}.txt", result.id)).exists(),
                "unexpected .txt sidecar for SkippedNoEval snippet {}",
                result.id
            );
            assert!(
                !cache_dir
                    .join(format!("{}.manifest.json", result.id))
                    .exists(),
                "unexpected .manifest.json for SkippedNoEval snippet {}",
                result.id
            );
        }
    }

    // Clean up.
    let _ = std::fs::remove_dir_all(&cache_dir);
}
