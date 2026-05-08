// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Drive evcxr through a snippet sequence and write text sidecars.
//!
//! Phase 1 scope (T-I03): one `CommandContext` per `Project::evaluate` call,
//! snippets executed in document order, captured stdout written to
//! `<entry-parent>/.evcxr-typst-cache/<id>.txt`. No MIME passthrough, no
//! caching, no error sidecars; failures are reported via [`SnippetOutcome`].

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use evcxr::CommandContext;

use crate::{Error, EvalCallbacks, Snippet, SnippetKind, SnippetOutcome, SnippetResult};

pub(crate) const CACHE_DIRNAME: &str = ".evcxr-typst-cache";

pub(crate) struct EvalOutcome {
    pub results: Vec<SnippetResult>,
}

pub(crate) fn run(
    snippets: &[Snippet],
    cache_dir: &Path,
    callbacks: Option<&mut dyn EvalCallbacks>,
) -> Result<EvalOutcome, Error> {
    tracing::debug!(snippets = snippets.len(), "eval::run start");
    fs::create_dir_all(cache_dir)?;
    tracing::debug!(path = %cache_dir.display(), "cache dir ready");

    let (mut context, outputs) =
        CommandContext::new().map_err(|e| Error::Evcxr(format!("CommandContext::new: {e}")))?;
    tracing::debug!("CommandContext spawned");

    // evcxr's `EvalContext::try_run_statements` busy-waits for its internal
    // `stdout_sender` to drain before returning from `execute()`; if the
    // host has not consumed the channel by then, `execute()` deadlocks.
    // Forwarder threads keep evcxr's channel empty so `execute()` always
    // makes progress, re-publishing each line on a private mpsc channel
    // that the main thread drains per snippet.
    let (stdout_tx, stdout_rx) = mpsc::channel::<String>();
    let (stderr_tx, stderr_rx) = mpsc::channel::<String>();
    let stdout_drain = {
        let rx = outputs.stdout.clone();
        thread::spawn(move || {
            while let Ok(line) = rx.recv() {
                if stdout_tx.send(line).is_err() {
                    break;
                }
            }
        })
    };
    let stderr_drain = {
        let rx = outputs.stderr.clone();
        thread::spawn(move || {
            while let Ok(line) = rx.recv() {
                if stderr_tx.send(line).is_err() {
                    break;
                }
            }
        })
    };

    let mut results = Vec::with_capacity(snippets.len());
    let mut cb_holder = callbacks;

    for snippet in snippets {
        tracing::debug!(id = %snippet.id, kind = ?snippet.kind, "snippet start");
        if let Some(cb) = cb_holder.as_deref_mut() {
            cb.on_snippet_start(snippet);
        }

        if !is_evaluable(snippet.kind) {
            let result = SnippetResult {
                id: snippet.id.clone(),
                outcome: SnippetOutcome::Ok,
                stdout: String::new(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
            };
            if let Some(cb) = cb_holder.as_deref_mut() {
                cb.on_snippet_finish(snippet, &result.outcome);
            }
            results.push(result);
            continue;
        }

        // Discard any leftover lines from before this snippet so we attribute
        // output correctly. The forwarder threads run continuously, so under
        // normal flow these channels are empty here, but a prior failed
        // snippet may have produced trailing noise.
        drain_pending(&stdout_rx);
        drain_pending(&stderr_rx);

        tracing::debug!(id = %snippet.id, "calling context.execute");
        let start = Instant::now();
        let exec_result = context.execute(&snippet.src);
        let elapsed = start.elapsed();
        tracing::debug!(id = %snippet.id, ok = exec_result.is_ok(), "context.execute returned");

        // Trailing settle: evcxr's wait-for-empty loop returns once its
        // own channel is drained, but the forwarder thread may still be
        // re-sending the last line on `stdout_rx` when execute() returns.
        thread::sleep(Duration::from_millis(20));
        let stdout = collect_pending(&stdout_rx);
        let stderr = collect_pending(&stderr_rx);

        let outcome = match exec_result {
            Ok(_) => SnippetOutcome::Ok,
            Err(evcxr::Error::CompilationErrors(errors)) => {
                tracing::warn!(id = %snippet.id, errors = ?errors, "compile error");
                SnippetOutcome::CompileError
            }
            Err(e) => {
                tracing::warn!(id = %snippet.id, error = %e, "runtime error");
                SnippetOutcome::RuntimePanic
            }
        };
        tracing::debug!(
            id = %snippet.id,
            outcome = ?outcome,
            stdout_bytes = stdout.len(),
            stderr_bytes = stderr.len(),
            elapsed_ms = elapsed.as_millis() as u64,
            "snippet finished"
        );

        if matches!(outcome, SnippetOutcome::Ok) {
            let path = sidecar_path(cache_dir, &snippet.id);
            write_atomically(&path, stdout.as_bytes())?;
            if let Some(cb) = cb_holder.as_deref_mut() {
                cb.on_sidecar_written(snippet, &path);
            }
        }

        let result = SnippetResult {
            id: snippet.id.clone(),
            outcome,
            stdout,
            stderr,
            elapsed,
        };
        if let Some(cb) = cb_holder.as_deref_mut() {
            cb.on_snippet_finish(snippet, &result.outcome);
        }
        results.push(result);
    }

    drop(context);
    let _ = stdout_drain.join();
    let _ = stderr_drain.join();

    Ok(EvalOutcome { results })
}

fn drain_pending(rx: &Receiver<String>) {
    while rx.try_recv().is_ok() {}
}

fn collect_pending(rx: &Receiver<String>) -> String {
    let mut out = String::new();
    while let Ok(line) = rx.try_recv() {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

pub(crate) fn skip_all(snippets: &[Snippet]) -> Vec<SnippetResult> {
    snippets
        .iter()
        .map(|s| SnippetResult {
            id: s.id.clone(),
            outcome: if is_evaluable(s.kind) {
                SnippetOutcome::SkippedNoEval
            } else {
                SnippetOutcome::Ok
            },
            stdout: String::new(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        })
        .collect()
}

fn is_evaluable(kind: SnippetKind) -> bool {
    matches!(
        kind,
        SnippetKind::Rust
            | SnippetKind::RustOut
            | SnippetKind::RustDisplay
            | SnippetKind::RustHidden
            | SnippetKind::RustData
            | SnippetKind::RustMain
    )
}

fn sidecar_path(cache_dir: &Path, id: &str) -> PathBuf {
    cache_dir.join(format!("{id}.txt"))
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
