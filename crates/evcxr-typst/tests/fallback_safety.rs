// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! D-004 verification: bare `typst compile` must succeed without any
//! `--input` flags and without any sidecars on disk.

use std::process::Command;

/// Run `typst compile` on examples/hello/main.typ with no --input flags.
/// This simulates a reader compiling a document they received without
/// running evcxr-typst first. Should produce a PDF (placeholders only)
/// and exit 0.
#[test]
fn bare_typst_compile_succeeds() {
    // Locate the repo root: tests run from the crate directory.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let repo_root = std::path::Path::new(&manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let entry = repo_root.join("examples/hello/main.typ");
    if !entry.exists() {
        eprintln!("skipping: examples/hello/main.typ not found");
        return;
    }

    let out = Command::new("typst")
        .arg("compile")
        .arg("--root")
        .arg(repo_root)
        .arg("--format")
        .arg("pdf")
        .arg(&entry)
        .arg(repo_root.join("examples/hello/main-test-fallback.pdf"))
        .output();

    match out {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping: typst not in PATH");
        }
        Err(e) => panic!("failed to spawn typst: {e}"),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            assert!(
                o.status.success(),
                "bare typst compile failed (D-004 violated).\nstderr: {stderr}"
            );
            assert!(
                !stderr.to_lowercase().contains("error"),
                "bare typst compile produced errors (D-004 violated).\nstderr: {stderr}"
            );
            // Clean up the test artifact.
            let _ = std::fs::remove_file(repo_root.join("examples/hello/main-test-fallback.pdf"));
        }
    }
}
