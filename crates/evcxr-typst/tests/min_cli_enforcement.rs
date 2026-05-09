// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Integration tests for D-019 `min-cli` enforcement.
//!
//! These tests write a temporary Typst document inside the repo tree (so
//! that `typst query --root <repo>` can resolve the local `lib.typ` import)
//! and verify that `Project::open` either succeeds (requirement met) or
//! returns `Error::IncompatibleCliVersion` (requirement not met).

use std::io::Write;
use std::path::PathBuf;

use evcxr_typst::{Error, Project, ProjectConfig};

fn repo_root() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    std::path::Path::new(&manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Write a minimal `.typ` file at `path` inside the repo tree. The import
/// uses a root-relative path so `typst query --root <repo>` resolves it
/// correctly regardless of where in the tree `path` lives.
fn write_min_cli_doc(path: &std::path::Path, min_cli: &str) {
    let doc = format!("#import \"/packages/evcxr/lib.typ\": *\n#setup(min-cli: \"{min_cli}\")\n");
    let mut f = std::fs::File::create(path).expect("create temp .typ");
    f.write_all(doc.as_bytes()).expect("write temp .typ");
}

/// Write a minimal `.typ` file with no `setup(min-cli:)`.
fn write_no_min_cli_doc(path: &std::path::Path) {
    let doc = "#import \"/packages/evcxr/lib.typ\": *\n= No min-cli\n";
    let mut f = std::fs::File::create(path).expect("create temp .typ");
    f.write_all(doc.as_bytes()).expect("write temp .typ");
}

/// Helper: open a project with the repo root as the typst root.
/// Returns `Ok(result)` or `Err(())` if typst is not in PATH.
fn try_open(doc_path: &std::path::Path) -> Result<Result<Project, Error>, ()> {
    let root = repo_root();
    match Project::open_with_config(doc_path, ProjectConfig::new().with_root(&root)) {
        Err(Error::Discovery(ref msg)) if msg.contains("failed to spawn") => {
            eprintln!("skipping: typst not in PATH");
            Err(())
        }
        other => Ok(other),
    }
}

/// RAII guard that removes a file on drop.
struct RemoveOnDrop(PathBuf);
impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A `min-cli` of "999.0.0" is always newer than the running binary.
/// `Project::open` must return `Error::IncompatibleCliVersion`.
#[test]
fn open_rejects_too_old_cli() {
    evcxr::runtime_hook();
    let root = repo_root();
    std::fs::create_dir_all(root.join("target")).unwrap();
    let doc_path = root.join("target/test-min-cli-reject.typ");
    write_min_cli_doc(&doc_path, "999.0.0");
    let _g = RemoveOnDrop(doc_path.clone());

    let result = match try_open(&doc_path) {
        Err(()) => return,
        Ok(r) => r,
    };

    match result {
        Err(Error::IncompatibleCliVersion { required, actual }) => {
            assert_eq!(required, "999.0.0");
            assert_eq!(actual, env!("CARGO_PKG_VERSION"));
        }
        other => panic!("expected IncompatibleCliVersion, got: {:?}", other.err()),
    }
}

/// A `min-cli` of "0.0.0" is always older than the running binary (at least
/// 0.1.0 after T-I08). `Project::open` must succeed.
#[test]
fn open_accepts_satisfied_min_cli() {
    evcxr::runtime_hook();
    let root = repo_root();
    std::fs::create_dir_all(root.join("target")).unwrap();
    let doc_path = root.join("target/test-min-cli-accept.typ");
    write_min_cli_doc(&doc_path, "0.0.0");
    let _g = RemoveOnDrop(doc_path.clone());

    let result = match try_open(&doc_path) {
        Err(()) => return,
        Ok(r) => r,
    };

    assert!(
        result.is_ok(),
        "expected Ok(Project), got Err: {:?}",
        result.err()
    );
}

/// No `setup(min-cli:)` at all — must always succeed.
#[test]
fn open_with_no_min_cli_succeeds() {
    evcxr::runtime_hook();
    let root = repo_root();
    std::fs::create_dir_all(root.join("target")).unwrap();
    let doc_path = root.join("target/test-no-min-cli.typ");
    write_no_min_cli_doc(&doc_path);
    let _g = RemoveOnDrop(doc_path.clone());

    let result = match try_open(&doc_path) {
        Err(()) => return,
        Ok(r) => r,
    };

    assert!(
        result.is_ok(),
        "expected Ok(Project) with no min-cli, got Err: {:?}",
        result.err()
    );
}

/// The exact current CLI version as the requirement — must succeed (equal is OK).
#[test]
fn open_accepts_exact_current_version() {
    evcxr::runtime_hook();
    let root = repo_root();
    std::fs::create_dir_all(root.join("target")).unwrap();
    let doc_path = root.join("target/test-min-cli-exact.typ");
    let current = env!("CARGO_PKG_VERSION");
    write_min_cli_doc(&doc_path, current);
    let _g = RemoveOnDrop(doc_path.clone());

    let result = match try_open(&doc_path) {
        Err(()) => return,
        Ok(r) => r,
    };

    assert!(
        result.is_ok(),
        "expected Ok(Project) for min-cli == current ({current}), got Err: {:?}",
        result.err()
    );
}
