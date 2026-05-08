// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Binary-side clap layer. The library (`evcxr_typst`) is clap-free; this
//! module is the only place that knows about argument parsing. See D-023.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use evcxr_typst::{EvalOptions, Project, ProjectConfig, WatchOptions};

#[derive(Parser)]
#[command(
    name = "evcxr-typst",
    version,
    about = "Evaluate Rust snippets in Typst documents via evcxr"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// One-shot: discover snippets, evaluate them, render the document.
    Run {
        path: PathBuf,
        #[arg(long)]
        allow_eval: bool,
        /// Project root passed to `typst query` / `typst compile`. Defaults
        /// to the entry file's parent directory.
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Watch mode: keep one CommandContext alive, re-eval on file change.
    Watch {
        path: PathBuf,
        #[arg(long)]
        allow_eval: bool,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Drop the snippet-output sidecars for a document. CAS contents are kept.
    Clean {
        path: PathBuf,
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Run {
            path,
            allow_eval,
            root,
        } => {
            let mut project = open_project(&path, root)?;
            let opts = if allow_eval {
                EvalOptions::allow_eval()
            } else {
                EvalOptions::deny()
            };
            let report = project.evaluate(&opts)?;
            eprintln!(
                "{} snippets evaluated in {:?}",
                report.snippets.len(),
                report.elapsed
            );
            let cache_typst_path = project.cache_dir_typst_path()?;
            let pdf = output_path(&path, "pdf");
            let svg = output_path(&path, "svg");
            typst_compile(project.root(), &path, &cache_typst_path, "pdf", &pdf)?;
            typst_compile(project.root(), &path, &cache_typst_path, "svg", &svg)?;
            Ok(())
        }
        Cmd::Watch {
            path,
            allow_eval,
            root,
        } => {
            let mut project = open_project(&path, root)?;
            let opts = if allow_eval {
                WatchOptions::allow_eval()
            } else {
                WatchOptions::deny()
            };
            let handle = project.watch(&opts)?;
            handle.join()?;
            Ok(())
        }
        Cmd::Clean { path, root } => {
            let project = open_project(&path, root)?;
            project.clean_view()?;
            Ok(())
        }
    }
}

fn open_project(path: &Path, root: Option<PathBuf>) -> Result<Project> {
    let mut config = ProjectConfig::new();
    if let Some(r) = root {
        config = config.with_root(r);
    }
    Project::open_with_config(path, config).with_context(|| format!("opening {}", path.display()))
}

fn typst_compile(
    root: &Path,
    entry: &Path,
    cache_typst_path: &str,
    format: &str,
    output: &Path,
) -> Result<()> {
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
        .with_context(|| "spawning `typst compile`")?;
    if !status.success() {
        bail!("`typst compile` ({format}) failed (status {status})");
    }
    Ok(())
}

// We render both PDF (the user-facing artifact) and SVG. Typst's SVG embeds
// glyphs as `<path>` references (not `<text>`), so SVG is for visual
// inspection in a browser rather than text-grep; the textual record of each
// snippet's evaluated stdout lives in `.evcxr-typst-cache/<id>.txt`. For
// multi-page documents typst rejects a single-file SVG path; if that comes up,
// invoke `typst compile` directly with a `{p}` template.
fn output_path(entry: &Path, ext: &str) -> PathBuf {
    let stem = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let parent = entry.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{stem}.{ext}"))
}
