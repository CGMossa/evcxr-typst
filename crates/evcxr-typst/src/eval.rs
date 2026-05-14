// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Drive evcxr through a snippet sequence and write sidecars.
//!
//! T-I04 scope: captures `EvalOutputs.content_by_mime_type`, decodes binary
//! payloads (base64), writes per-MIME extension sidecars, and writes a
//! `<id>.manifest.json` listing every extension produced so that `lib.typ`
//! can probe safely without `try`/`catch`.
//!
//! T-I07 scope: adds error classification (compile / panic / timeout / dep /
//! internal) and writes `<id>.error.json` sidecars via `error_capture`.
//! Adds a watchdog-thread timeout (default 30 s, D-009) around each
//! `context.execute` call — sync, no tokio.
//!
//! `:dep` snippets are handled by emitting a `:dep` directive directly into
//! `CommandContext::execute` before any snippet that follows in document order.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use evcxr::CommandContext;
use serde_json::json;

use crate::cache::{self, CacheEnv};
use crate::{
    Error, EvalCallbacks, Snippet, SnippetKind, SnippetOptions, SnippetOutcome, SnippetResult,
    error_capture::{self, OffsetMap},
};

pub(crate) const CACHE_DIRNAME: &str = ".evcxr-typst-cache";

/// Default per-snippet timeout (D-009: 30 s). Can be overridden via
/// `EvalOptions::with_snippet_timeout` and per-snippet `timeout:` kwarg (D-017).
pub(crate) const DEFAULT_SNIPPET_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct EvalOutcome {
    pub results: Vec<SnippetResult>,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

/// Run the eval loop with an optional global timeout override.
///
/// `default_timeout`: `None` → use `DEFAULT_SNIPPET_TIMEOUT`;
///   `Some(Duration::ZERO)` → no timeout.
pub(crate) fn run(
    snippets: &[Snippet],
    cache_dir: &Path,
    callbacks: Option<&mut dyn EvalCallbacks>,
    default_timeout: Option<Duration>,
) -> Result<EvalOutcome, Error> {
    tracing::debug!(snippets = snippets.len(), "eval::run start");
    fs::create_dir_all(cache_dir)?;
    cache::ensure_readme(cache_dir)?;
    tracing::debug!(path = %cache_dir.display(), "cache dir ready");

    let env = CacheEnv::collect(&[]);
    let mut prior_chain = cache::initial_chain();
    let mut active_deps: Vec<&Snippet> = Vec::new();
    let mut index = cache::read_index(cache_dir);

    // SAFETY: single-threaded before CommandContext::new() spawns any child.
    // WHY: enables backtraces in the evcxr child on panic (D-009 error reporting).
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let (mut context, outputs) =
        CommandContext::new().map_err(|e| Error::Evcxr(format!("CommandContext::new: {e}")))?;
    tracing::debug!("CommandContext spawned");
    // Enable evcxr's rustc artifact cache (see cache.md § "Interaction with evcxr's :cache").
    let _ = context.execute(":cache 500");

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

    let global_timeout = default_timeout.unwrap_or(DEFAULT_SNIPPET_TIMEOUT);

    let mut results = Vec::with_capacity(snippets.len());
    let mut cb_holder = callbacks;
    let mut offset_map = OffsetMap::new();
    let mut prev_item_names: Vec<String> =
        context.defined_item_names().map(str::to_owned).collect();
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;

    for snippet in snippets {
        tracing::debug!(id = %snippet.id, kind = ?snippet.kind, "snippet start");
        if let Some(cb) = cb_holder.as_deref_mut() {
            cb.on_snippet_start(snippet);
        }

        // Handle dep snippets: emit a `:dep` directive and continue.
        if snippet.kind == SnippetKind::Dep {
            let directive = format_dep_directive(snippet);
            let dep_spec = if let SnippetOptions::Dep { spec, .. } = &snippet.options {
                spec.clone()
            } else {
                snippet.id.clone()
            };
            tracing::debug!(id = %snippet.id, directive = %directive, "emitting :dep");
            drain_pending(&stdout_rx);
            drain_pending(&stderr_rx);
            let start = Instant::now();
            let exec_result = context.execute(&directive);
            let elapsed = start.elapsed();
            thread::sleep(Duration::from_millis(20));
            let stdout = collect_pending(&stdout_rx);
            let stderr = collect_pending(&stderr_rx);

            // Track as active dep after successful resolution.
            if exec_result.is_ok() {
                active_deps.push(snippet);
            }

            let outcome = match exec_result {
                Ok(_) => SnippetOutcome::Ok,
                Err(e) => {
                    tracing::warn!(id = %snippet.id, error = %e, ":dep resolution error");
                    // Write a dep error sidecar.
                    let sidecar =
                        error_capture::classify_dep_error(&dep_spec, &stderr, &snippet.id);
                    if let Err(io) = error_capture::write_error_sidecar(cache_dir, &sidecar, false)
                    {
                        tracing::warn!(id = %snippet.id, error = %io, "failed to write dep error sidecar");
                    }
                    SnippetOutcome::DepResolutionError
                }
            };
            let result = SnippetResult {
                id: snippet.id.clone(),
                outcome: outcome.clone(),
                stdout,
                stderr,
                elapsed,
                mime_sidecars: Vec::new(),
            };
            if let Some(cb) = cb_holder.as_deref_mut() {
                cb.on_snippet_finish(snippet, &outcome);
            }
            results.push(result);
            // Advance chain with dep snippet (empty src — dep doesn't affect
            // code state but still advances the chain monotonically).
            prior_chain = cache::advance_chain(&prior_chain, &snippet.src);
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

        // Compute cache key and check for a hit.
        let cache_key = cache::compute_key(snippet, &prior_chain, &active_deps, &env);
        let cached_key_matches = index
            .get(&snippet.id)
            .map(|k| k == &cache_key)
            .unwrap_or(false);

        if cached_key_matches
            && let Ok(cache::LookupResult::Hit) = cache::lookup(cache_dir, &cache_key, &snippet.id)
        {
            tracing::debug!(id = %snippet.id, "cache hit");
            cache_hits += 1;
            let result = SnippetResult {
                id: snippet.id.clone(),
                outcome: SnippetOutcome::CacheHit,
                stdout: String::new(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
                mime_sidecars: Vec::new(),
            };
            if let Some(cb) = cb_holder.as_deref_mut() {
                cb.on_snippet_finish(snippet, &result.outcome);
            }
            results.push(result);
            prior_chain = cache::advance_chain(&prior_chain, &snippet.src);
            continue;
        }

        // Discard any leftover lines from before this snippet so we attribute
        // output correctly. The forwarder threads run continuously, so under
        // normal flow these channels are empty here, but a prior failed
        // snippet may have produced trailing noise.
        drain_pending(&stdout_rx);
        drain_pending(&stderr_rx);

        // Resolve the effective timeout for this snippet (D-017: per-snippet wins).
        let effective_timeout = snippet.timeout_ms.map(Duration::from_millis).or_else(|| {
            if global_timeout == Duration::ZERO {
                None
            } else {
                Some(global_timeout)
            }
        });

        tracing::debug!(id = %snippet.id, timeout_ms = ?effective_timeout.map(|d| d.as_millis()), "calling context.execute");
        let start = Instant::now();

        // Watchdog-thread timeout: spawn a thread that kills the child after
        // `effective_timeout` if the main thread hasn't already finished.
        // No tokio: we stay sync at the library boundary (D-023).
        // WHY two flags: `timed_out` is the stand-down signal (main → watchdog),
        // `watchdog_fired` is the kill-happened signal (watchdog → main).
        // We must NOT derive was_timeout from elapsed time: a slow machine
        // completing just under the threshold would misfire, and an actual
        // timeout where the child dies early for another reason would miss.
        let timed_out = Arc::new(AtomicBool::new(false));
        let watchdog_fired = Arc::new(AtomicBool::new(false));
        let watchdog = if let Some(timeout_dur) = effective_timeout {
            let stand_down = Arc::clone(&timed_out);
            let fired = Arc::clone(&watchdog_fired);
            let handle = context.process_handle();
            Some(thread::spawn(move || {
                thread::sleep(timeout_dur);
                if !stand_down.load(Ordering::Relaxed) {
                    fired.store(true, Ordering::Relaxed);
                    handle.lock().unwrap().kill().ok();
                }
            }))
        } else {
            None
        };

        let eval_src = source_for_execute(snippet);
        let exec_result = context.execute(eval_src.as_ref());

        // Signal the watchdog to stand down before joining.
        timed_out.store(true, Ordering::Relaxed);
        if let Some(wd) = watchdog {
            let _ = wd.join();
        }

        let elapsed = start.elapsed();
        tracing::debug!(id = %snippet.id, ok = exec_result.is_ok(), "context.execute returned");

        // Trailing settle: evcxr's wait-for-empty loop returns once its
        // own channel is drained, but the forwarder thread may still be
        // re-sending the last line on `stdout_rx` when execute() returns.
        thread::sleep(Duration::from_millis(20));
        let stdout = collect_pending(&stdout_rx);
        let stderr = collect_pending(&stderr_rx);

        let was_timeout = watchdog_fired.load(Ordering::Acquire);

        let (outcome, mime_map) = match exec_result {
            Ok(eval_outputs) => {
                tracing::debug!(
                    id = %snippet.id,
                    mime_types = ?eval_outputs.content_by_mime_type.keys().collect::<Vec<_>>(),
                    "eval succeeded"
                );
                // Update the offset map with newly defined items.
                let current_names: Vec<String> =
                    context.defined_item_names().map(str::to_owned).collect();
                let new_names = current_names
                    .iter()
                    .filter(|n| !prev_item_names.contains(n))
                    .cloned()
                    .collect::<Vec<_>>();
                offset_map.record_submission(&snippet.id, &snippet.src, new_names.iter());
                prev_item_names = current_names;

                (SnippetOutcome::Ok, eval_outputs.content_by_mime_type)
            }
            Err(evcxr::Error::CompilationErrors(ref errors)) => {
                tracing::warn!(id = %snippet.id, n = errors.len(), "compile error");
                let (sidecar, cross_snippets) = error_capture::classify_compile_error(
                    errors,
                    &snippet.id,
                    &snippet.src,
                    &offset_map,
                );
                if let Err(io) = error_capture::write_error_sidecar(cache_dir, &sidecar, false) {
                    tracing::warn!(id = %snippet.id, error = %io, "failed to write compile error sidecar");
                }
                // Write note stubs for any prior snippets referenced by this error (D-014).
                for (prior_id, prior_src) in &cross_snippets {
                    let note = error_capture::classify_cross_snippet_note(
                        prior_id,
                        prior_src,
                        &snippet.id,
                    );
                    if let Err(io) = error_capture::write_error_sidecar(cache_dir, &note, false) {
                        tracing::warn!(prior_id = %prior_id, error = %io, "failed to write cross-snippet note sidecar");
                    }
                }
                (SnippetOutcome::CompileError, HashMap::new())
            }
            Err(evcxr::Error::TypeRedefinedVariablesLost(ref vars)) => {
                tracing::warn!(id = %snippet.id, ?vars, "TypeRedefinedVariablesLost");
                let msg = format!(
                    "type redefinition caused variables to be lost: {}",
                    vars.join(", ")
                );
                let sidecar =
                    error_capture::classify_internal(&msg, "warning", &snippet.id, &snippet.src);
                if let Err(io) =
                    error_capture::write_error_sidecar(cache_dir, &sidecar, !stdout.is_empty())
                {
                    tracing::warn!(id = %snippet.id, error = %io, "failed to write internal sidecar");
                }
                // WHY: Ok, not RuntimePanic — the child process did NOT die;
                // evcxr just lost type-redefined bindings. RuntimePanic would
                // cause the T-I05 watch loop to reset CommandContext, which is
                // wrong here. The warning box is surfaced via the error sidecar.
                (SnippetOutcome::Ok, HashMap::new())
            }
            Err(evcxr::Error::SubprocessTerminated(ref msg)) => {
                tracing::warn!(id = %snippet.id, msg = %msg, timeout = was_timeout, "subprocess terminated");
                if was_timeout {
                    let dur = effective_timeout.unwrap_or(DEFAULT_SNIPPET_TIMEOUT);
                    let sidecar = error_capture::classify_timeout(
                        dur,
                        stdout.len(),
                        &snippet.id,
                        &snippet.src,
                    );
                    if let Err(io) =
                        error_capture::write_error_sidecar(cache_dir, &sidecar, !stdout.is_empty())
                    {
                        tracing::warn!(id = %snippet.id, error = %io, "failed to write timeout sidecar");
                    }
                    (SnippetOutcome::Timeout, HashMap::new())
                } else {
                    let sidecar =
                        error_capture::classify_panic(msg, &stderr, &snippet.id, &snippet.src);
                    let has_partial = !stdout.is_empty();
                    if let Err(io) =
                        error_capture::write_error_sidecar(cache_dir, &sidecar, has_partial)
                    {
                        tracing::warn!(id = %snippet.id, error = %io, "failed to write panic sidecar");
                    }
                    (SnippetOutcome::RuntimePanic, HashMap::new())
                }
            }
            Err(evcxr::Error::Message(ref msg)) => {
                tracing::warn!(id = %snippet.id, msg = %msg, "internal evcxr error");
                let sidecar =
                    error_capture::classify_internal(msg, "error", &snippet.id, &snippet.src);
                if let Err(io) = error_capture::write_error_sidecar(cache_dir, &sidecar, false) {
                    tracing::warn!(id = %snippet.id, error = %io, "failed to write internal sidecar");
                }
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
            // Mirror the panic/partial-stdout block below: warn, omit from _index.json.
            match write_mime_sidecars(cache_dir, &snippet.id, &mime_map, &stdout) {
                Ok(written) => {
                    for path in &written {
                        if let Some(cb) = cb_holder.as_deref_mut() {
                            cb.on_sidecar_written(snippet, path);
                        }
                    }
                    mime_sidecars = written;
                    let _ = cache::store(cache_dir, &cache_key, &snippet.id);
                    index.insert(snippet.id.clone(), cache_key.clone());
                }
                Err(e) => {
                    tracing::warn!(
                        id = %snippet.id,
                        error = %e,
                        "sidecar write failed; snippet absent from _index.json",
                    );
                }
            }
            cache_misses += 1;
        } else if matches!(
            outcome,
            SnippetOutcome::RuntimePanic | SnippetOutcome::Timeout
        ) && !stdout.is_empty()
        {
            // Write partial stdout sidecar for panic/timeout so rust-out shows it.
            let txt_path = cache_dir.join(format!("{}.txt", snippet.id));
            if let Err(e) = write_atomically(&txt_path, stdout.as_bytes()) {
                tracing::warn!(id = %snippet.id, error = %e, "failed to write partial stdout sidecar");
            }
            cache_misses += 1;
        } else {
            cache_misses += 1;
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
        prior_chain = cache::advance_chain(&prior_chain, &snippet.src);
    }

    drop(context);
    let _ = stdout_drain.join();
    let _ = stderr_drain.join();

    // Write the updated index atomically.
    let _ = cache::write_index(cache_dir, &index);

    Ok(EvalOutcome {
        results,
        cache_hits,
        cache_misses,
    })
}

/// Write all sidecars for a single successful snippet. Returns paths of every
/// file written (including the manifest).
///
/// Policy: explicit `text/plain` MIME in `content_by_mime_type` wins over
/// forwarded stdout for the `.txt` sidecar. If both are present, the MIME
/// payload is written and the forwarded stdout is discarded (explicit wins).
/// If only `plain_stdout` is non-empty, it is written as the `.txt` sidecar.
pub(crate) fn write_mime_sidecars(
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
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload)
                .map_err(|e| Error::Evcxr(format!("base64 decode error for image/png: {e}")))?;
            Ok(("png".to_owned(), bytes))
        }
        "image/jpeg" => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload)
                .map_err(|e| Error::Evcxr(format!("base64 decode error for image/jpeg: {e}")))?;
            Ok(("jpg".to_owned(), bytes))
        }
        "application/cbor" => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload)
                .map_err(|e| {
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
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload)
                .unwrap_or_else(|_| payload.as_bytes().to_vec());
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
pub(crate) fn format_dep_directive(snippet: &Snippet) -> String {
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

/// Like `skip_all` but checks the cache first; returns `CacheHit` for snippets
/// whose output is already in the CAS. Used on the deny-eval path so cached
/// results are still served without running evcxr.
pub(crate) fn skip_all_with_cache(
    snippets: &[Snippet],
    cache_dir: &Path,
) -> Result<EvalOutcome, Error> {
    if !cache_dir.exists() {
        return Ok(EvalOutcome {
            results: skip_all(snippets),
            cache_hits: 0,
            cache_misses: snippets.len(),
        });
    }
    let env = CacheEnv::collect(&[]);
    let mut prior_chain = cache::initial_chain();
    let mut active_deps: Vec<&Snippet> = Vec::new();
    let index = cache::read_index(cache_dir);
    let mut results = Vec::with_capacity(snippets.len());
    let mut cache_hits = 0usize;
    let mut cache_misses = 0usize;

    for snippet in snippets {
        if snippet.kind == SnippetKind::Dep {
            active_deps.push(snippet);
            results.push(SnippetResult {
                id: snippet.id.clone(),
                outcome: SnippetOutcome::SkippedNoEval,
                stdout: String::new(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
                mime_sidecars: Vec::new(),
            });
            prior_chain = cache::advance_chain(&prior_chain, &snippet.src);
            continue;
        }
        if !is_evaluable(snippet.kind) {
            results.push(SnippetResult {
                id: snippet.id.clone(),
                outcome: SnippetOutcome::Ok,
                stdout: String::new(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
                mime_sidecars: Vec::new(),
            });
            continue;
        }
        let key = cache::compute_key(snippet, &prior_chain, &active_deps, &env);
        let cached_key_matches = index.get(&snippet.id).map(|k| k == &key).unwrap_or(false);
        let outcome = if cached_key_matches
            && matches!(
                cache::lookup(cache_dir, &key, &snippet.id),
                Ok(cache::LookupResult::Hit)
            ) {
            cache_hits += 1;
            SnippetOutcome::CacheHit
        } else {
            cache_misses += 1;
            SnippetOutcome::SkippedNoEval
        };
        results.push(SnippetResult {
            id: snippet.id.clone(),
            outcome,
            stdout: String::new(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
            mime_sidecars: Vec::new(),
        });
        prior_chain = cache::advance_chain(&prior_chain, &snippet.src);
    }
    Ok(EvalOutcome {
        results,
        cache_hits,
        cache_misses,
    })
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

pub(crate) fn is_evaluable(kind: SnippetKind) -> bool {
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

/// Source submitted to evcxr for a snippet.
///
/// `rust-main(...)` keeps the rendered and metadata source faithful to the
/// user's `fn main() { ... }` block, but evcxr only defines that function.
/// The hidden trailing call is an execution detail, not part of the rendered
/// Typst source.
pub(crate) fn source_for_execute(snippet: &Snippet) -> std::borrow::Cow<'_, str> {
    if snippet.kind == SnippetKind::RustMain {
        std::borrow::Cow::Owned(format!("{}\nmain();", snippet.src))
    } else {
        std::borrow::Cow::Borrowed(&snippet.src)
    }
}

pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Write `_index.json` listing snippet IDs that have materialised sidecars.
/// The Typst package reads this once to decide which snippets can be read
/// vs. which should fall through to the placeholder (D-004, T-I06).
pub(crate) fn write_available_index(cache_dir: &Path, available_ids: &[&str]) -> Result<(), Error> {
    let ids_json = available_ids
        .iter()
        .map(|id| format!("\"{}\"", id.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!("{{\"v\":1,\"available\":[{ids_json}]}}");
    write_atomically(&cache_dir.join("_index.json"), json.as_bytes())
}

/// Derive the available-id list from filesystem state and write `_index.json`.
///
/// Used by the watch loop where results are accumulated incrementally rather
/// than returned as a batch. A snippet ID is "available" when its
/// `<id>.manifest.json` exists (written by `write_mime_sidecars`).
pub(crate) fn write_available_index_for_snippets(
    cache_dir: &Path,
    snippets: &[Snippet],
) -> Result<(), Error> {
    let available: Vec<&str> = snippets
        .iter()
        .filter(|s| cache_dir.join(format!("{}.manifest.json", s.id)).exists())
        .map(|s| s.id.as_str())
        .collect();
    write_available_index(cache_dir, &available)
}

/// Validate materialised sidecars for snippets that are expected to have them.
/// Returns pairs of `(snippet_id, reason)` for any malformed or missing sidecar.
pub(crate) fn validate_sidecars(
    snippets: &[Snippet],
    results: &[SnippetResult],
    cache_dir: &Path,
) -> Vec<(String, String)> {
    debug_assert_eq!(
        snippets.len(),
        results.len(),
        "validate_sidecars: snippets/results length mismatch"
    );
    let mut issues = Vec::new();
    for (snippet, result) in snippets.iter().zip(results.iter()) {
        if result.outcome != SnippetOutcome::CacheHit {
            continue;
        }
        let manifest_path = cache_dir.join(format!("{}.manifest.json", snippet.id));
        match fs::read_to_string(&manifest_path) {
            Err(_) => issues.push((
                snippet.id.clone(),
                "manifest.json missing for cache-hit snippet".to_owned(),
            )),
            Ok(txt) => match serde_json::from_str::<serde_json::Value>(&txt) {
                Err(_) => issues.push((
                    snippet.id.clone(),
                    "manifest.json is not valid JSON".to_owned(),
                )),
                Ok(v) => {
                    if v.get("v").and_then(|v| v.as_u64()) != Some(1) {
                        issues.push((snippet.id.clone(), "manifest.json has unknown v".to_owned()));
                    }
                }
            },
        }
    }
    issues
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
            timeout_ms: None,
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

    fn make_snippet(kind: SnippetKind, src: &str) -> Snippet {
        Snippet {
            id: "test".to_owned(),
            kind,
            file: PathBuf::from("main.typ"),
            doc_order: 0,
            src: src.to_owned(),
            options: SnippetOptions::None,
            timeout_ms: None,
        }
    }

    #[test]
    fn source_for_execute_rust_main_appends_main_call() {
        // D-022: rust-main keeps the rendered Typst source faithful to upstream
        // (`fn main() { ... }`) and the CLI synthesises a trailing `main();`
        // call only when handing the snippet to evcxr.
        let s = make_snippet(SnippetKind::RustMain, "fn main() {\n    println!(\"hi\");\n}");
        assert_eq!(
            source_for_execute(&s).as_ref(),
            "fn main() {\n    println!(\"hi\");\n}\nmain();"
        );
    }

    #[test]
    fn source_for_execute_other_kinds_pass_through() {
        for kind in [
            SnippetKind::Rust,
            SnippetKind::RustOut,
            SnippetKind::RustDisplay,
            SnippetKind::RustHidden,
            SnippetKind::RustData,
        ] {
            let s = make_snippet(kind, "let x = 1;");
            assert_eq!(source_for_execute(&s).as_ref(), "let x = 1;");
        }
    }
}
