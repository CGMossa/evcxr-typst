// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

mod cli;

fn main() -> anyhow::Result<()> {
    // Must be the very first thing in main. evcxr re-enters this binary as
    // the host child or as a rustc wrapper depending on env vars; if we do
    // anything else first, that path breaks. See evcxr/src/runtime.rs.
    evcxr::runtime_hook();

    init_tracing();
    cli::run()
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt;

    // Off by default; opt-in via RUST_LOG / EVCXR_TYPST_LOG. Writing to
    // stderr keeps stdout clean for downstream consumers (e.g. piping a
    // future `--json` report).
    let filter = EnvFilter::try_from_env("EVCXR_TYPST_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(true)
        .try_init();
}
