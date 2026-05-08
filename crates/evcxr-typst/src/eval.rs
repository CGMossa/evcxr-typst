// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Drive evcxr through a snippet sequence and write sidecars.
//!
//! T-I04 scope: captures `EvalOutputs.content_by_mime_type`, decodes binary
//! payloads (base64), writes per-MIME extension sidecars, and writes a
//! `<id>.manifest.json` listing every extension produced so that `lib.typ`
//! can probe safely without `try`/`catch`.
//!
//! `:dep` snippets are handled by emitting a `:dep` directive directly into
//! `CommandContext::execute` before any snippet that follows in document order.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use evcxr::CommandContext;
use serde_json::json;

use crate::{
    Error, EvalCallbacks, Snippet, SnippetKind, SnippetOptions, SnippetOutcome, SnippetResult,
};

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

        // Handle dep snippets: emit a `:dep` directive and continue.
        if snippet.kind == SnippetKind::Dep {
            let directive = format_dep_directive(snippet);
            tracing::debug!(id = %snippet.id, directive = %directive, "emitting :dep");
            drain_pending(&stdout_rx);
            drain_pending(&stderr_rx);
            let start = Instant::now();
            let exec_result = context.execute(&directive);
            let elapsed = start.elapsed();
            thread::sleep(Duration::from_millis(20));
            drain_pending(&stdout_rx);
            drain_pending(&stderr_rx);

            let outcome = match exec_result {
                Ok(_) => SnippetOutcome::Ok,
                Err(e) => {
                    tracing::warn!(id = %snippet.id, error = %e, ":dep resolution error");
                    SnippetOutcome::DepResolutionError
                }
            };
            let result = SnippetResult {
                id: snippet.id.clone(),
                outcome: outcome.clone(),
                stdout: String::new(),
                stderr: String::new(),
                elapsed,
                mime_sidecars: Vec::new(),
            };
            if let Some(cb) = cb_holder.as_deref_mut() {
                cb.on_snippet_finish(snippet, &outcome);
            }
            results.push(result);
            continue;
        }

        if !is_evaluable(snippet.kind) {
            let result = SnippetResult {
                id: snippet.id.clone(),
                outcome: SnippetOutcome::Ok,
                stdout: String::new(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
                mime_sidecars: Vec::new(),
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

        let (outcome, mime_map) = match exec_result {
            Ok(eval_outputs) => {
                tracing::debug!(
                    id = %snippet.id,
                    mime_types = ?eval_outputs.content_by_mime_type.keys().collect::<Vec<_>>(),
                    "eval succeeded"
                );
                (SnippetOutcome::Ok, eval_outputs.content_by_mime_type)
            }
            Err(evcxr::Error::CompilationErrors(errors)) => {
                tracing::warn!(id = %snippet.id, errors = ?errors, "compile error");
                (SnippetOutcome::CompileError, HashMap::new())
            }
            Err(e) => {
                tracing::warn!(id = %snippet.id, error = %e, "runtime error");
                (SnippetOutcome::RuntimePanic, HashMap::new())
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

        let mut mime_sidecars = Vec::new();
        if matches!(outcome, SnippetOutcome::Ok) {
            let written = write_mime_sidecars(cache_dir, &snippet.id, &mime_map, &stdout)?;
            for path in &written {
                if let Some(cb) = cb_holder.as_deref_mut() {
                    cb.on_sidecar_written(snippet, path);
                }
            }
            mime_sidecars = written;
        }

        let result = SnippetResult {
            id: snippet.id.clone(),
            outcome: outcome.clone(),
            stdout,
            stderr,
            elapsed,
            mime_sidecars,
        };
        if let Some(cb) = cb_holder.as_deref_mut() {
            cb.on_snippet_finish(snippet, &outcome);
        }
        results.push(result);
    }

    drop(context);
    let _ = stdout_drain.join();
    let _ = stderr_drain.join();

    Ok(EvalOutcome { results })
}

/// Write all sidecars for a single successful snippet. Returns paths of every
/// file written (including the manifest).
///
/// Policy: explicit `text/plain` MIME in `content_by_mime_type` wins over
/// forwarded stdout for the `.txt` sidecar. If both are present, the MIME
/// payload is written and the forwarded stdout is discarded (explicit wins).
/// If only `plain_stdout` is non-empty, it is written as the `.txt` sidecar.
fn write_mime_sidecars(
    cache_dir: &Path,
    id: &str,
    content_by_mime_type: &HashMap<String, String>,
    plain_stdout: &str,
) -> Result<Vec<PathBuf>, Error> {
    let mut written: Vec<PathBuf> = Vec::new();
    let mut extensions_emitted: Vec<String> = Vec::new();

    // Track whether an explicit text/plain MIME was emitted.
    let explicit_text_plain = content_by_mime_type.contains_key("text/plain");

    // Write the forwarded-stdout .txt only when no explicit text/plain MIME.
    if !explicit_text_plain && !plain_stdout.is_empty() {
        let path = cache_dir.join(format!("{id}.txt"));
        write_atomically(&path, plain_stdout.as_bytes())?;
        extensions_emitted.push("txt".to_owned());
        written.push(path);
    }

    for (mime, payload) in content_by_mime_type {
        let (ext, bytes) = mime_to_ext_and_bytes(mime, payload)?;

        let path = cache_dir.join(format!("{id}.{ext}"));
        write_atomically(&path, &bytes)?;
        if !extensions_emitted.contains(&ext) {
            extensions_emitted.push(ext.clone());
        }
        written.push(path.clone());

        // For unknown MIMEs, write a companion meta file so lib.typ can map
        // the extension back to the original MIME type.
        if is_unknown_mime(mime) {
            let meta_path = cache_dir.join(format!("{id}.meta.json"));
            let meta = json!({"mime": mime, "filename": format!("{id}.{ext}")});
            write_atomically(&meta_path, meta.to_string().as_bytes())?;
            written.push(meta_path);
        }
    }

    // Always write the manifest even when extensions_emitted is empty.
    // lib.typ reads the manifest before probing any extension file; this
    // guarantees that read() never hits a missing-file hard error because
    // the manifest itself always exists for any successfully evaluated snippet.
    let manifest_path = cache_dir.join(format!("{id}.manifest.json"));
    let manifest = json!({"v": 1, "extensions": extensions_emitted});
    write_atomically(&manifest_path, manifest.to_string().as_bytes())?;
    written.push(manifest_path);

    Ok(written)
}

fn mime_to_ext_and_bytes(mime: &str, payload: &str) -> Result<(String, Vec<u8>), Error> {
    match mime {
        "text/plain" => Ok(("txt".to_owned(), payload.as_bytes().to_vec())),
        "text/html" => Ok(("html".to_owned(), payload.as_bytes().to_vec())),
        "image/svg+xml" => Ok(("svg".to_owned(), payload.as_bytes().to_vec())),
        "application/json" => Ok(("json".to_owned(), payload.as_bytes().to_vec())),
        "image/png" => {
            let bytes = base64::decode(payload)
                .map_err(|e| Error::Evcxr(format!("base64 decode error for image/png: {e}")))?;
            Ok(("png".to_owned(), bytes))
        }
        "image/jpeg" => {
            let bytes = base64::decode(payload)
                .map_err(|e| Error::Evcxr(format!("base64 decode error for image/jpeg: {e}")))?;
            Ok(("jpg".to_owned(), bytes))
        }
        "application/cbor" => {
            let bytes = base64::decode(payload).map_err(|e| {
                Error::Evcxr(format!("base64 decode error for application/cbor: {e}"))
            })?;
            Ok(("cbor".to_owned(), bytes))
        }
        other => {
            // Derive extension from MIME subtype, strip structured-syntax suffix.
            let ext = other
                .split('/')
                .nth(1)
                .unwrap_or("bin")
                .split('+')
                .next()
                .unwrap_or("bin")
                .to_owned();
            // Try base64-decode; on failure use raw payload bytes.
            let bytes = base64::decode(payload).unwrap_or_else(|_| payload.as_bytes().to_vec());
            Ok((ext, bytes))
        }
    }
}

fn is_unknown_mime(mime: &str) -> bool {
    !matches!(
        mime,
        "text/plain"
            | "text/html"
            | "image/png"
            | "image/svg+xml"
            | "image/jpeg"
            | "application/json"
            | "application/cbor"
    )
}

/// Build the `:dep` directive string that `CommandContext::execute` accepts.
///
/// Grammar (from `command_context.rs`):
///   `:dep <name> [= <config>]`
/// where `<config>` is a TOML value (e.g. `"1"` or `{ version = "1",
/// features = ["derive"] }`). A name containing `=` is passed through
/// verbatim as a TOML fragment.
fn format_dep_directive(snippet: &Snippet) -> String {
    let SnippetOptions::Dep {
        spec,
        version,
        features,
    } = &snippet.options
    else {
        // Shouldn't happen, but be defensive.
        return format!(":dep {}", snippet.id);
    };

    // If spec contains '=', treat it as a TOML fragment and pass verbatim.
    if spec.contains('=') {
        return format!(":dep {spec}");
    }

    match (version.as_deref(), features.is_empty()) {
        // No version, no features → bare crate name (latest)
        (None, true) => format!(":dep {spec}"),
        // Version only
        (Some(v), true) => format!(":dep {spec} = \"{v}\""),
        // Features only
        (None, false) => {
            let feats: Vec<String> = features.iter().map(|f| format!("\"{f}\"")).collect();
            format!(":dep {spec} = {{ features = [{}] }}", feats.join(", "))
        }
        // Version + features
        (Some(v), false) => {
            let feats: Vec<String> = features.iter().map(|f| format!("\"{f}\"")).collect();
            format!(
                ":dep {spec} = {{ version = \"{v}\", features = [{}] }}",
                feats.join(", ")
            )
        }
    }
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
            outcome: if is_evaluable(s.kind) || s.kind == SnippetKind::Dep {
                SnippetOutcome::SkippedNoEval
            } else {
                SnippetOutcome::Ok
            },
            stdout: String::new(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
            mime_sidecars: Vec::new(),
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

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_dep_snippet(spec: &str, version: Option<&str>, features: Vec<&str>) -> Snippet {
        Snippet {
            id: "test".to_owned(),
            kind: SnippetKind::Dep,
            file: PathBuf::from("main.typ"),
            doc_order: 0,
            src: String::new(),
            options: SnippetOptions::Dep {
                spec: spec.to_owned(),
                version: version.map(|v| v.to_owned()),
                features: features.iter().map(|f| f.to_string()).collect(),
            },
        }
    }

    #[test]
    fn format_dep_bare() {
        let snippet = make_dep_snippet("image", None, vec![]);
        assert_eq!(format_dep_directive(&snippet), ":dep image");
    }

    #[test]
    fn format_dep_version_only() {
        let snippet = make_dep_snippet("image", Some("0.24"), vec![]);
        assert_eq!(format_dep_directive(&snippet), ":dep image = \"0.24\"");
    }

    #[test]
    fn format_dep_features_only() {
        let snippet = make_dep_snippet("image", None, vec!["jpeg"]);
        assert_eq!(
            format_dep_directive(&snippet),
            ":dep image = { features = [\"jpeg\"] }"
        );
    }

    #[test]
    fn format_dep_version_and_features() {
        let snippet = make_dep_snippet("image", Some("0.24"), vec!["jpeg", "png"]);
        assert_eq!(
            format_dep_directive(&snippet),
            ":dep image = { version = \"0.24\", features = [\"jpeg\", \"png\"] }"
        );
    }
}
