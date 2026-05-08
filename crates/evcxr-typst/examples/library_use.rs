// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Canonical "use evcxr-typst as a library" example.
//!
//! Mirrors evcxr's own `evcxr/examples/example_eval.rs`. Runs the same
//! discover → evaluate path as the `evcxr-typst` CLI but from a regular
//! Rust program, demonstrating that hosts (IDE servers, custom test
//! runners, mdBook plugins, …) can drive the loop without going through
//! clap.
//!
//! Usage:
//!
//! ```sh
//! cargo run -p evcxr-typst --example library_use -- path/to/main.typ
//! ```
//!
//! Mirrors what `evcxr-typst run --allow-eval <path>` does end-to-end:
//! discover snippets, evaluate them through the library API, then shell out
//! to `typst compile` to render the document. Output is SVG (one file per
//! page) — text format chosen so the dev loop can inspect the rendered
//! result without a PDF viewer.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};
use evcxr_typst::{EvalOptions, Project, ProjectConfig};

fn main() -> anyhow::Result<()> {
    // Same contract as the binary: runtime_hook before anything else.
    evcxr::runtime_hook();

    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: library_use <main.typ> [<root>]");
    let root = args.next();

    let mut config = ProjectConfig::new();
    if let Some(r) = root {
        config = config.with_root(r);
    }

    let mut project = Project::open_with_config(&path, config)?;
    eprintln!(
        "discovered {} snippets in {}",
        project.snippets().len(),
        path
    );

    let report = project.evaluate(&EvalOptions::allow_eval())?;
    println!(
        "{} snippets evaluated, {} cache hits in {:?}",
        report.snippets.len(),
        report.cache_hits,
        report.elapsed
    );
    for s in &report.snippets {
        println!("  {} {:?}", s.id, s.outcome);
    }

    let cache_typst_path = project.cache_dir_typst_path()?;
    let entry = Path::new(&path);
    let pdf = output_path(entry, "pdf");
    let svg = output_path(entry, "svg");
    typst_compile(project.root(), entry, &cache_typst_path, "pdf", &pdf)?;
    typst_compile(project.root(), entry, &cache_typst_path, "svg", &svg)?;
    Ok(())
}

fn typst_compile(
    root: &Path,
    entry: &Path,
    cache_typst_path: &str,
    format: &str,
    output: &Path,
) -> anyhow::Result<()> {
    let status = Command::new("typst")
        .arg("compile")
        .arg("--root")
        .arg(root)
        .arg("--format")
        .arg(format)
        .arg("--input")
        .arg("evcxr-mode=read")
        .arg("--input")
        .arg(format!("evcxr-cache={cache_typst_path}"))
        .arg(entry)
        .arg(output)
        .status()
        .context("spawning `typst compile`")?;
    if !status.success() {
        bail!("`typst compile` ({format}) failed (status {status})");
    }
    Ok(())
}

fn output_path(entry: &Path, ext: &str) -> std::path::PathBuf {
    let stem = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let parent = entry.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{stem}.{ext}"))
}
