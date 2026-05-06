// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use std::path::PathBuf;

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
    },
    /// Watch mode: keep one CommandContext alive, re-eval on file change.
    Watch {
        path: PathBuf,
        #[arg(long)]
        allow_eval: bool,
    },
    /// Drop the snippet-output sidecars for a document. CAS contents are kept.
    Clean { path: PathBuf },
}

fn main() -> Result<()> {
    // Must be the very first thing in main. evcxr re-enters this binary as the
    // host child or as a rustc wrapper depending on env vars; if we do anything
    // else first, that path breaks. See evcxr/src/runtime.rs runtime_hook().
    evcxr::runtime_hook();

    let _cli = Cli::parse();

    eprintln!(
        "evcxr-typst v{} — scaffolding only. Subcommands parse but are not yet \
         implemented (T-I01..T-I07 in docs/BACKLOG.md).",
        env!("CARGO_PKG_VERSION")
    );
    std::process::exit(2);
}
