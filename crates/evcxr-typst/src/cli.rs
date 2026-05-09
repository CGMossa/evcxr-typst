// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Binary-side clap layer. The library (`evcxr_typst`) is clap-free; this
//! module is the only place that knows about argument parsing. See D-023.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use evcxr_typst::{
    Error as LibError, EvalOptions, Project, ProjectConfig, SnippetOutcome, WatchOptions,
};

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
    ///
    /// With `--gc`: additionally evict CAS entries not referenced by the
    /// current run's index.
    Clean {
        path: PathBuf,
        #[arg(long)]
        root: Option<PathBuf>,
        /// Run the cache GC after cleaning the view (drops unreferenced CAS entries).
        #[arg(long)]
        gc: bool,
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
            let mut opts = if allow_eval {
                EvalOptions::allow_eval()
            } else {
                EvalOptions::deny()
            };
            let report = project.evaluate(&mut opts)?;
            let error_count = report
                .snippets
                .iter()
                .filter(|s| {
                    !matches!(
                        s.outcome,
                        SnippetOutcome::Ok
                            | SnippetOutcome::SkippedNoEval
                            | SnippetOutcome::CacheHit
                    )
                })
                .count();

            if !allow_eval {
                // Informational summary on the deny-eval path. Exits 0 (D-004).
                let skipped = report
                    .snippets
                    .iter()
                    .filter(|s| s.outcome == SnippetOutcome::SkippedNoEval)
                    .count();
                let nag = if skipped > 0 {
                    " — run with `evcxr-typst run --allow-eval` to evaluate"
                } else {
                    ""
                };
                eprintln!(
                    "{} snippets: {} cached, {} need eval{}",
                    report.snippets.len(),
                    report.cache_hits,
                    skipped,
                    nag,
                );
                if !report.validation_issues.is_empty() {
                    eprintln!(
                        "  {} malformed sidecar(s): {}",
                        report.validation_issues.len(),
                        report
                            .validation_issues
                            .iter()
                            .map(|(id, r)| format!("{id}: {r}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            } else {
                eprintln!(
                    "{} snippets evaluated in {:?}; {} errors",
                    report.snippets.len(),
                    report.elapsed,
                    error_count
                );
            }

            let cache_typst_path = project.cache_dir_typst_path()?;
            let pdf = output_path(&path, "pdf");
            let svg = output_path(&path, "svg");
            typst_compile(project.root(), &path, &cache_typst_path, "pdf", &pdf)?;
            // Multi-page documents reject a single-file SVG output; rather than
            // bailing the whole run, downgrade SVG failure to a warning. The
            // PDF is the user-facing artifact; SVG is the visual quick-look.
            // For multi-page docs the user can run `typst compile` directly
            // with a `{p}` template to get per-page SVGs.
            if let Err(e) = typst_compile(project.root(), &path, &cache_typst_path, "svg", &svg) {
                eprintln!(
                    "warning: SVG render skipped ({e}). PDF at {} is unaffected.",
                    pdf.display()
                );
            }
            if error_count > 0 {
                bail!("{error_count} snippet(s) failed (see error sidecars and SVG for details)");
            }
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
            // Block until Ctrl-C; only then ask the watch loop to shut down.
            // WatchHandle::join sends the shutdown signal first, so calling it
            // unconditionally here would exit after the first cycle.
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            ctrlc::set_handler(move || {
                let _ = tx.send(());
            })
            .map_err(|e| anyhow::anyhow!("installing Ctrl-C handler: {e}"))?;
            eprintln!("watch running; press Ctrl-C to stop.");
            let _ = rx.recv();
            handle.join()?;
            Ok(())
        }
        Cmd::Clean { path, root, gc } => {
            let project = open_project(&path, root)?;
            project.clean_view()?;
            if gc {
                let removed = project.gc()?;
                eprintln!("GC: removed {removed} unused CAS entries");
            }
            Ok(())
        }
    }
}

fn open_project(path: &Path, root: Option<PathBuf>) -> Result<Project> {
    let mut config = ProjectConfig::new();
    if let Some(r) = root {
        config = config.with_root(r);
    }
    Project::open_with_config(path, config).map_err(|e| {
        // Map IncompatibleCliVersion → exit 2 per D-019.
        if let LibError::IncompatibleCliVersion {
            ref required,
            ref actual,
        } = e
        {
            eprintln!("error: this document requires evcxr-typst >= {required}; you have {actual}");
            eprintln!("\tinstall a newer version: cargo install evcxr-typst");
            std::process::exit(2);
        }
        anyhow::Error::from(e).context(format!("opening {}", path.display()))
    })
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
