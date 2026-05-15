// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Pin lib.typ's `_read-stdout` against the no-stdout-but-evaluated case.
//!
//! eval.rs::write_mime_sidecars only writes `<id>.txt` when `plain_stdout`
//! is non-empty. A snippet that evaluates successfully but prints nothing
//! has no `.txt` sidecar — but its `<id>.manifest.json` still exists and
//! its id appears in `_index.json`. Before the fix, `_read-stdout`
//! unconditionally `read()`-ed the `.txt` path and Typst raised a hard
//! error, breaking D-004.
//!
//! Fixture: `tests/fixtures/no_stdout_main.typ`. The cache is hand-crafted
//! into a tempdir so no eval is involved.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn rust_snippet_with_no_stdout_does_not_break_compile() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_dir = PathBuf::from(&manifest);
    let repo_root = crate_dir.parent().unwrap().parent().unwrap().to_path_buf();
    let entry = crate_dir.join("tests/fixtures/no_stdout_main.typ");

    // The cache must live under --root because typst resolves paths
    // passed via sys.inputs (and read in lib.typ via json()/read()) relative
    // to the project root. `target/` is the conventional out-of-tree scratch
    // location inside the workspace.
    let cache_abs = repo_root.join("target/test-no-stdout-cache");
    let _ = fs::remove_dir_all(&cache_abs);
    fs::create_dir_all(&cache_abs).unwrap();
    fs::write(
        cache_abs.join("_index.json"),
        r#"{"v":1,"available":["no-stdout-snippet"]}"#,
    )
    .unwrap();
    fs::write(
        cache_abs.join("no-stdout-snippet.manifest.json"),
        r#"{"v":1,"extensions":[]}"#,
    )
    .unwrap();
    let cache_typst = "/target/test-no-stdout-cache";

    let pdf = std::env::temp_dir().join("evcxr-typst-test-no-stdout.pdf");
    let _ = fs::remove_file(&pdf);

    let out = Command::new("typst")
        .arg("compile")
        .arg("--root")
        .arg(&repo_root)
        .arg("--input")
        .arg("evcxr-mode=read")
        .arg("--input")
        .arg(format!("evcxr-cache={cache_typst}"))
        .arg(&entry)
        .arg(&pdf)
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
                "typst compile failed against a no-stdout snippet in read mode \
                 (manifest says snippet evaluated, just produced no stdout).\n\
                 stderr: {stderr}"
            );
            assert!(
                !stderr.to_lowercase().contains("error"),
                "typst compile emitted errors against a no-stdout snippet.\n\
                 stderr: {stderr}"
            );
            assert!(pdf.exists(), "expected PDF at {}", pdf.display());

            let _ = fs::remove_file(&pdf);
            let _ = fs::remove_dir_all(&cache_abs);
        }
    }
}
