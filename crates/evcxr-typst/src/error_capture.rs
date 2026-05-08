// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Capture, classify, and serialize evcxr errors into `<id>.error.json` sidecars.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::json;

use evcxr::CompilationError;

use crate::Error;

// ---------------------------------------------------------------------------
// Sidecar types — the `<id>.error.json` schema (errors.md § 2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ErrorSidecar {
    pub v: u32,
    pub snippet_id: String,
    pub phase: ErrorPhase,
    /// Full rendered terminal text from rustc/cargo (ANSI codes included).
    pub rendered_terminal: String,
    pub recorded_at: String,
    pub snippet_src: String,
    pub errors: Vec<ErrorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ErrorPhase {
    Compile,
    RuntimePanic,
    DepResolution,
    Timeout,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ErrorEntry {
    pub severity: String,
    pub code: Option<String>,
    pub message: String,
    pub primary_span: Option<SpanRef>,
    pub secondary_spans: Vec<SpanRef>,
    pub helps: Vec<HelpEntry>,
    pub evcxr_hint: Option<String>,
    pub panic: Option<PanicInfo>,
    pub timeout: Option<TimeoutInfo>,
    pub dep: Option<DepInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SpanRef {
    pub snippet_id: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub text: String,
    pub line_start: usize,
    pub col_start: usize,
    pub line_end: usize,
    pub col_end: usize,
    pub label: String,
    #[serde(default)]
    pub is_cross_snippet: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HelpEntry {
    pub message: String,
    pub suggested_replacement: Option<String>,
    pub span: Option<SpanRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PanicInfo {
    pub message: String,
    pub location: Option<String>,
    pub backtrace: Vec<String>,
    pub backtrace_truncated_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TimeoutInfo {
    pub duration_ms: u64,
    pub captured_stdout_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DepInfo {
    pub spec: String,
    pub cargo_stderr: String,
}

// ---------------------------------------------------------------------------
// Offset map — tracks which items were committed by which snippet (D-014)
// ---------------------------------------------------------------------------

#[allow(dead_code)] // fields read by the watch loop (T-I05)
pub(crate) struct SnippetSubmission {
    pub snippet_id: String,
    pub src: String,
}

pub(crate) struct OffsetMap {
    pub submissions: Vec<SnippetSubmission>,
    /// Reverse lookup: item name → snippet_id that last defined it.
    pub committed_items: HashMap<String, String>,
}

impl OffsetMap {
    pub(crate) fn new() -> Self {
        Self {
            submissions: Vec::new(),
            committed_items: HashMap::new(),
        }
    }

    /// Record a snippet as submitted and update the committed-items index from
    /// newly defined names after a successful execute.
    pub(crate) fn record_submission(
        &mut self,
        snippet_id: &str,
        src: &str,
        new_item_names: impl Iterator<Item = impl AsRef<str>>,
    ) {
        for name in new_item_names {
            self.committed_items
                .entry(name.as_ref().to_owned())
                .or_insert_with(|| snippet_id.to_owned());
        }
        self.submissions.push(SnippetSubmission {
            snippet_id: snippet_id.to_owned(),
            src: src.to_owned(),
        });
    }
}

// ---------------------------------------------------------------------------
// Byte-offset helper (mirrors evcxr/src/errors.rs::span_to_byte_range)
// WHY: evcxr's `Span` type is pub inside `errors.rs` but not re-exported from
// the crate root, so we cannot name it in our signatures. We replicate the
// offset formula and pass the four line/column integers directly.
// ---------------------------------------------------------------------------

/// Convert 1-based (line, col) pair to a byte offset within `source`.
/// Column is treated as a byte offset within the line (matching rustc JSON).
fn line_col_to_byte_offset(source: &str, line: usize, col: usize) -> usize {
    source
        .lines()
        .take(line - 1)
        .map(|x| x.len())
        .sum::<usize>()
        + col
        + line
        - 2
}

#[allow(clippy::too_many_arguments)]
fn lines_cols_to_span_ref(
    src: &str,
    snippet_id: &str,
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
    label: &str,
    is_cross_snippet: bool,
) -> Option<SpanRef> {
    let byte_start = line_col_to_byte_offset(src, start_line, start_col);
    let byte_end = line_col_to_byte_offset(src, end_line, end_col);
    if byte_start > src.len() || byte_end > src.len() || byte_start > byte_end {
        return None;
    }
    let text = src[byte_start..byte_end].to_owned();
    Some(SpanRef {
        snippet_id: snippet_id.to_owned(),
        byte_start,
        byte_end,
        text,
        line_start: start_line,
        col_start: start_col,
        line_end: end_line,
        col_end: end_col,
        label: label.to_owned(),
        is_cross_snippet,
    })
}

/// Resolve a span against the current snippet first, then prior submissions.
///
/// WHY: evcxr compiles a virtual file containing all committed items plus the
/// current snippet. Spans that fall outside the current snippet's line range
/// refer to a prior submission's code — this is the D-014 cross-snippet case.
/// Heuristic: cumulative line counts, assuming submissions are concatenated
/// in doc order. This will misattribute if evcxr inserts wrapper lines before
/// user code; acceptable for v0 per D-014.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_span(
    offset_map: &OffsetMap,
    current_snippet_id: &str,
    current_src: &str,
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
    label: &str,
) -> Option<SpanRef> {
    // Try current snippet first.
    if let Some(sr) = lines_cols_to_span_ref(
        current_src,
        current_snippet_id,
        start_line,
        start_col,
        end_line,
        end_col,
        label,
        false,
    ) {
        return Some(sr);
    }
    // Span is out-of-bounds for current snippet; search prior submissions
    // using cumulative line offsets.
    let mut cum: usize = 0;
    for sub in &offset_map.submissions {
        let sub_lines = sub.src.lines().count();
        if start_line > cum && start_line <= cum + sub_lines {
            let adj_start = start_line - cum;
            let adj_end = end_line.saturating_sub(cum).min(sub_lines);
            if let Some(mut sr) = lines_cols_to_span_ref(
                &sub.src,
                &sub.snippet_id,
                adj_start,
                start_col,
                adj_end,
                end_col,
                label,
                true,
            ) {
                sr.line_start = start_line;
                sr.line_end = end_line;
                return Some(sr);
            }
        }
        cum += sub_lines;
    }
    None
}

fn build_error_entry(
    ce: &CompilationError,
    current_snippet_id: &str,
    current_src: &str,
    offset_map: &OffsetMap,
) -> ErrorEntry {
    let primary_span = ce.primary_spanned_message().and_then(|sm| {
        sm.span.as_ref().and_then(|s| {
            resolve_span(
                offset_map,
                current_snippet_id,
                current_src,
                s.start_line,
                s.start_column,
                s.end_line,
                s.end_column,
                &sm.label,
            )
        })
    });

    let secondary_spans: Vec<SpanRef> = ce
        .spanned_messages()
        .iter()
        .filter(|sm| !sm.is_primary)
        .filter_map(|sm| {
            sm.span.as_ref().and_then(|s| {
                resolve_span(
                    offset_map,
                    current_snippet_id,
                    current_src,
                    s.start_line,
                    s.start_column,
                    s.end_line,
                    s.end_column,
                    &sm.label,
                )
            })
        })
        .collect();

    let helps: Vec<HelpEntry> = ce
        .help_spanned()
        .iter()
        .map(|sm| {
            let span = sm.span.as_ref().and_then(|s| {
                resolve_span(
                    offset_map,
                    current_snippet_id,
                    current_src,
                    s.start_line,
                    s.start_column,
                    s.end_line,
                    s.end_column,
                    &sm.label,
                )
            });
            HelpEntry {
                message: sm.label.clone(),
                suggested_replacement: None,
                span,
            }
        })
        .collect();

    ErrorEntry {
        severity: ce.level().to_owned(),
        code: ce.code().map(str::to_owned),
        message: ce.message(),
        primary_span,
        secondary_spans,
        helps,
        evcxr_hint: ce.evcxr_extra_hint().map(str::to_owned),
        panic: None,
        timeout: None,
        dep: None,
    }
}

// ---------------------------------------------------------------------------
// Public classification functions
// ---------------------------------------------------------------------------

/// Build a compile-error sidecar from a slice of evcxr `CompilationError`s.
///
/// Returns the sidecar plus a list of (snippet_id, src) pairs for any prior
/// snippets whose code was referenced by the error spans (D-014). The caller
/// is responsible for writing note stubs at those snippet IDs.
pub(crate) fn classify_compile_error(
    errors: &[CompilationError],
    snippet_id: &str,
    src: &str,
    offset_map: &OffsetMap,
) -> (ErrorSidecar, Vec<(String, String)>) {
    let rendered_terminal: String = errors
        .iter()
        .filter(|e| e.is_from_user_code())
        .map(|e| e.rendered())
        .collect::<Vec<_>>()
        .join("")
        .chars()
        .take(65536)
        .collect();

    let mut entries: Vec<ErrorEntry> = errors
        .iter()
        .filter(|e| e.is_from_user_code())
        .map(|ce| build_error_entry(ce, snippet_id, src, offset_map))
        .collect();

    // Fall back to all errors when none are from user code (e.g. macro
    // expansion into generated code) so the error box isn't empty.
    if entries.is_empty() {
        entries = errors
            .iter()
            .map(|ce| build_error_entry(ce, snippet_id, src, offset_map))
            .collect();
    }

    // Collect unique prior snippet IDs referenced by cross-snippet spans.
    let mut cross: Vec<(String, String)> = Vec::new();
    for entry in &entries {
        let all_spans = entry
            .primary_span
            .iter()
            .chain(entry.secondary_spans.iter());
        for span in all_spans {
            if span.is_cross_snippet {
                let already = cross.iter().any(|(id, _)| id == &span.snippet_id);
                if !already {
                    let prior_src = offset_map
                        .submissions
                        .iter()
                        .find(|s| s.snippet_id == span.snippet_id)
                        .map(|s| s.src.clone())
                        .unwrap_or_default();
                    cross.push((span.snippet_id.clone(), prior_src));
                }
            }
        }
    }

    (
        ErrorSidecar {
            v: 1,
            snippet_id: snippet_id.to_owned(),
            phase: ErrorPhase::Compile,
            rendered_terminal,
            recorded_at: rfc3339_now(),
            snippet_src: src.to_owned(),
            errors: entries,
        },
        cross,
    )
}

/// Build a note stub sidecar for a prior snippet A that is referenced by
/// a cross-snippet error in snippet B (D-014 § 3).
pub(crate) fn classify_cross_snippet_note(
    prior_snippet_id: &str,
    prior_src: &str,
    referencing_snippet_id: &str,
) -> ErrorSidecar {
    classify_internal(
        &format!(
            "this item is referenced from snippet {referencing_snippet_id} and is producing errors there"
        ),
        "note",
        prior_snippet_id,
        prior_src,
    )
}

/// Build a runtime-panic sidecar.
pub(crate) fn classify_panic(
    subprocess_msg: &str,
    stderr: &str,
    snippet_id: &str,
    src: &str,
) -> ErrorSidecar {
    let (panic_message, location) = parse_panic_message(subprocess_msg, stderr);
    let backtrace = parse_backtrace(stderr);
    let backtrace_truncated = backtrace.len().saturating_sub(8);
    let backtrace_frames = backtrace.into_iter().take(8).collect();

    let entry = ErrorEntry {
        severity: "error".to_owned(),
        code: None,
        message: panic_message.clone(),
        primary_span: None,
        secondary_spans: vec![],
        helps: vec![],
        evcxr_hint: None,
        panic: Some(PanicInfo {
            message: panic_message,
            location,
            backtrace: backtrace_frames,
            backtrace_truncated_count: backtrace_truncated,
        }),
        timeout: None,
        dep: None,
    };

    ErrorSidecar {
        v: 1,
        snippet_id: snippet_id.to_owned(),
        phase: ErrorPhase::RuntimePanic,
        rendered_terminal: format!("panicked: {subprocess_msg}"),
        recorded_at: rfc3339_now(),
        snippet_src: src.to_owned(),
        errors: vec![entry],
    }
}

/// Build a timeout sidecar.
pub(crate) fn classify_timeout(
    duration: Duration,
    captured_stdout_bytes: usize,
    snippet_id: &str,
    src: &str,
) -> ErrorSidecar {
    let duration_ms = duration.as_millis() as u64;
    let entry = ErrorEntry {
        severity: "error".to_owned(),
        code: None,
        message: format!("snippet timed out after {}s", duration.as_secs()),
        primary_span: None,
        secondary_spans: vec![],
        helps: vec![],
        evcxr_hint: None,
        panic: None,
        timeout: Some(TimeoutInfo {
            duration_ms,
            captured_stdout_bytes,
        }),
        dep: None,
    };
    ErrorSidecar {
        v: 1,
        snippet_id: snippet_id.to_owned(),
        phase: ErrorPhase::Timeout,
        rendered_terminal: format!("snippet `{snippet_id}` timed out after {duration_ms}ms"),
        recorded_at: rfc3339_now(),
        snippet_src: src.to_owned(),
        errors: vec![entry],
    }
}

/// Build a dep-resolution failure sidecar.
pub(crate) fn classify_dep_error(dep_spec: &str, stderr: &str, snippet_id: &str) -> ErrorSidecar {
    let entry = ErrorEntry {
        severity: "error".to_owned(),
        code: None,
        message: format!(":dep resolution failed for `{dep_spec}`"),
        primary_span: None,
        secondary_spans: vec![],
        helps: vec![],
        evcxr_hint: None,
        panic: None,
        timeout: None,
        dep: Some(DepInfo {
            spec: dep_spec.to_owned(),
            cargo_stderr: stderr.chars().take(65536).collect(),
        }),
    };
    ErrorSidecar {
        v: 1,
        snippet_id: snippet_id.to_owned(),
        phase: ErrorPhase::DepResolution,
        rendered_terminal: format!(":dep {dep_spec} failed: {stderr}"),
        recorded_at: rfc3339_now(),
        snippet_src: String::new(),
        errors: vec![entry],
    }
}

/// Build an internal / TypeRedefinedVariablesLost sidecar.
pub(crate) fn classify_internal(
    msg: &str,
    severity: &str,
    snippet_id: &str,
    src: &str,
) -> ErrorSidecar {
    ErrorSidecar {
        v: 1,
        snippet_id: snippet_id.to_owned(),
        phase: ErrorPhase::Internal,
        rendered_terminal: msg.to_owned(),
        recorded_at: rfc3339_now(),
        snippet_src: src.to_owned(),
        errors: vec![ErrorEntry {
            severity: severity.to_owned(),
            code: None,
            message: msg.to_owned(),
            primary_span: None,
            secondary_spans: vec![],
            helps: vec![],
            evcxr_hint: None,
            panic: None,
            timeout: None,
            dep: None,
        }],
    }
}

/// Write `<id>.error.json` and update `<id>.manifest.json`.
///
/// Sets `extensions: ["error"]` (or `["error", "txt"]` when
/// `has_partial_stdout` is true) so that lib.typ's `_check-error` helper
/// detects the error via the manifest without a separate file-exists call.
pub(crate) fn write_error_sidecar(
    cache_dir: &Path,
    sidecar: &ErrorSidecar,
    has_partial_stdout: bool,
) -> Result<(), Error> {
    let id = &sidecar.snippet_id;

    let error_path = cache_dir.join(format!("{id}.error.json"));
    let bytes = serde_json::to_vec_pretty(sidecar)
        .map_err(|e| Error::Evcxr(format!("serialize error sidecar: {e}")))?;
    write_atomically(&error_path, &bytes)?;

    let mut extensions = vec!["error".to_owned()];
    if has_partial_stdout {
        extensions.push("txt".to_owned());
    }
    let manifest_path = cache_dir.join(format!("{id}.manifest.json"));
    let manifest = json!({"v": 1, "extensions": extensions});
    write_atomically(&manifest_path, manifest.to_string().as_bytes())?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn parse_panic_message(subprocess_msg: &str, stderr: &str) -> (String, Option<String>) {
    let combined = if stderr.is_empty() {
        subprocess_msg.to_owned()
    } else {
        format!("{subprocess_msg}\n{stderr}")
    };

    for line in combined.lines() {
        if let Some(rest) = line.strip_prefix("thread '")
            && let Some(idx) = rest.find("' panicked at ")
        {
            let after = &rest[idx + "' panicked at ".len()..];
            if let Some((msg, loc)) = after.split_once(", ") {
                return (msg.trim_matches('\'').to_owned(), Some(loc.to_owned()));
            }
            return (after.to_owned(), None);
        }
    }
    (subprocess_msg.to_owned(), None)
}

fn parse_backtrace(stderr: &str) -> Vec<String> {
    let mut frames = Vec::new();
    let mut in_backtrace = false;
    for line in stderr.lines() {
        if line.contains("stack backtrace:") {
            in_backtrace = true;
            continue;
        }
        if in_backtrace {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(idx) = trimmed.find(':') {
                let before = &trimmed[..idx];
                if before.trim().parse::<usize>().is_ok() {
                    frames.push(trimmed[idx + 1..].trim().to_owned());
                }
            }
        }
    }
    frames
}

fn rfc3339_now() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days: [u64; 12] = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for md in month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    use std::fs;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_compile_error_round_trip() {
        let sidecar = ErrorSidecar {
            v: 1,
            snippet_id: "abc123".to_owned(),
            phase: ErrorPhase::Compile,
            rendered_terminal: "error[E0308]: mismatched types\n".to_owned(),
            recorded_at: "2026-05-08T00:00:00Z".to_owned(),
            snippet_src: "let x: u32 = \"hello\";".to_owned(),
            errors: vec![ErrorEntry {
                severity: "error".to_owned(),
                code: Some("E0308".to_owned()),
                message: "mismatched types".to_owned(),
                primary_span: None,
                secondary_spans: vec![],
                helps: vec![],
                evcxr_hint: None,
                panic: None,
                timeout: None,
                dep: None,
            }],
        };
        let json = serde_json::to_string(&sidecar).unwrap();
        let back: ErrorSidecar = serde_json::from_str(&json).unwrap();
        assert_eq!(back.snippet_id, "abc123");
        assert_eq!(back.errors[0].code.as_deref(), Some("E0308"));
    }

    #[test]
    fn test_dep_error_classification() {
        let s = classify_dep_error("tokio", "error: failed to fetch\n", "dep-id");
        assert!(matches!(s.phase, ErrorPhase::DepResolution));
        assert_eq!(s.errors[0].dep.as_ref().unwrap().spec, "tokio");
    }

    #[test]
    fn test_span_to_bytes_ascii() {
        let src = "let x = 1;\nlet y = 2;\n";
        // line 2, col 5 → 'y'; col 6 → after 'y'
        let byte_start = line_col_to_byte_offset(src, 2, 5);
        let byte_end = line_col_to_byte_offset(src, 2, 6);
        assert_eq!(&src[byte_start..byte_end], "y");
    }

    #[test]
    fn test_span_to_bytes_multibyte() {
        // 🦀 is 4 bytes; columns are byte offsets (matching evcxr's Span).
        // "let _ = '🦀';\n" is 16 bytes (15 non-newline + 1 newline).
        let src = "let _ = '\u{1F980}';\nlet y = 2;\n";
        // Line 2, col 5 should land on 'y' (byte 20).
        let byte_start = line_col_to_byte_offset(src, 2, 5);
        let byte_end = line_col_to_byte_offset(src, 2, 6);
        // Verify we land on a valid codepoint boundary and get the right char.
        assert!(
            src.is_char_boundary(byte_start),
            "byte_start not on char boundary"
        );
        assert!(
            src.is_char_boundary(byte_end),
            "byte_end not on char boundary"
        );
        assert_eq!(&src[byte_start..byte_end], "y");
    }

    #[test]
    fn test_resolve_span_current_snippet() {
        let om = OffsetMap::new();
        let src = "let x = 1;\nlet y = 2;\n";
        // Line 1, col 5 → 'x'
        let sr = resolve_span(&om, "snip", src, 1, 5, 1, 6, "").unwrap();
        assert!(!sr.is_cross_snippet);
        assert_eq!(sr.text, "x");
    }

    #[test]
    fn test_cross_snippet_stub_written() {
        // Snippet A defines two lines; in evcxr's combined file A occupies lines 1-2.
        // Snippet B has only 1 line, occupying line 3 in the combined file.
        // A span at line 2 (within A's range) is out of bounds for src_b (only 1 line),
        // so resolve_span must detect it as cross-snippet and attribute it to A.
        let mut om = OffsetMap::new();
        let src_a = "fn item() {}\nfn other() {}";
        let src_b = "use_item();";
        om.record_submission("a", src_a, std::iter::once("item"));
        // Span at line 2 is in A's second line ("fn other() {}"), out of bounds for src_b.
        let sr = resolve_span(&om, "b", src_b, 2, 4, 2, 9, "defined here").unwrap();
        assert!(sr.is_cross_snippet, "expected cross-snippet span");
        assert_eq!(sr.snippet_id, "a");

        // Verify that classify_cross_snippet_note produces the right sidecar.
        let tmp = TempDir::new().unwrap();
        let note = classify_cross_snippet_note("a", src_a, "b");
        write_error_sidecar(tmp.path(), &note, false).unwrap();
        assert!(tmp.path().join("a.error.json").exists());
        let note_json: serde_json::Value =
            serde_json::from_slice(&fs::read(tmp.path().join("a.error.json")).unwrap()).unwrap();
        assert_eq!(note_json["errors"][0]["severity"], "note");
        assert!(
            note_json["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains("snippet b")
        );
    }

    #[test]
    fn test_partial_stdout_on_panic() {
        let tmp = TempDir::new().unwrap();
        let sidecar = classify_panic("thread panicked", "", "p", "panic!(\"boom\");");
        write_error_sidecar(tmp.path(), &sidecar, true).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(tmp.path().join("p.manifest.json")).unwrap()).unwrap();
        let exts: Vec<&str> = manifest["extensions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(exts.contains(&"error"));
        assert!(exts.contains(&"txt"));
    }

    #[test]
    fn rfc3339_format_is_reasonable() {
        let s = rfc3339_now();
        assert_eq!(s.len(), 20);
        assert!(s.ends_with('Z'));
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        assert_eq!(&s[10..11], "T");
    }
}
