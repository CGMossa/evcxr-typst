// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Snippet discovery via `typst query`.
//!
//! Runs three `typst query` calls — one for `<evcxr-snippet>`, one for
//! `<evcxr-dep>`, and one for `<evcxr-min-cli>` — and merges the snippet
//! and dep results by `doc_order`. The `<evcxr-min-cli>` value is returned
//! separately so callers can enforce it before evaluation begins (D-019).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::{Error, Snippet, SnippetKind, SnippetOptions, identity};

#[derive(Debug, Deserialize)]
pub(crate) struct RawSnippet {
    pub kind: String,
    pub id: Option<String>,
    pub src: String,
    #[serde(default)]
    pub options: serde_json::Value,
    /// Document position emitted by the package counter. When absent, the
    /// CLI falls back to JSON-array order (backward-compat with old packages).
    pub loc: Option<ItemLoc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawDep {
    pub id: Option<String>,
    pub spec: String,
    pub version: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    pub loc: Option<ItemLoc>,
}

/// Location info emitted by the shared `_order` counter in `lib.typ`.
/// Shared between snippet and dep markers.
#[derive(Debug, Deserialize)]
pub(crate) struct ItemLoc {
    pub doc_order: usize,
}

/// Result of the discovery pass: the ordered snippet list plus the optional
/// `min-cli` string declared by the document's `setup()` call (D-019).
pub(crate) struct DiscoveryResult {
    pub snippets: Vec<Snippet>,
    /// Highest `min-cli` requirement found across all `<evcxr-min-cli>`
    /// markers. `None` when the document has no `setup(min-cli: ...)`.
    pub min_cli: Option<String>,
}

pub(crate) fn discover(entry: &Path, root: &Path) -> Result<DiscoveryResult, Error> {
    tracing::debug!(entry = %entry.display(), root = %root.display(), "discover");

    let snippet_bytes = run_typst_query(entry, root, "<evcxr-snippet>")?;
    tracing::debug!(
        bytes = snippet_bytes.len(),
        "typst query <evcxr-snippet> returned"
    );

    let dep_bytes = run_typst_query_optional(entry, root, "<evcxr-dep>")?;
    tracing::debug!(bytes = dep_bytes.len(), "typst query <evcxr-dep> returned");

    let min_cli = query_min_cli(entry, root)?;
    tracing::debug!(min_cli = ?min_cli, "typst query <evcxr-min-cli> returned");

    let raws: Vec<RawSnippet> = serde_json::from_slice(&snippet_bytes).map_err(|e| {
        Error::Discovery(format!(
            "failed to parse `typst query` JSON for {}: {e}",
            entry.display()
        ))
    })?;

    let raw_deps: Vec<RawDep> = serde_json::from_slice(&dep_bytes).unwrap_or_default();

    // Build snippets from the <evcxr-snippet> query.
    let default_ids: Vec<String> = raws.iter().map(|r| identity::default_id(&r.src)).collect();
    let resolved_defaults = identity::resolve_collisions(&default_ids);

    let mut snippets: Vec<Snippet> = Vec::with_capacity(raws.len() + raw_deps.len());

    for (json_order, (raw, default_resolved)) in raws.into_iter().zip(resolved_defaults).enumerate()
    {
        let kind = parse_kind(&raw.kind).ok_or_else(|| {
            Error::Discovery(format!(
                "unknown snippet kind {:?} from typst query",
                raw.kind
            ))
        })?;
        let id = raw.id.clone().unwrap_or(default_resolved);

        // Parse kind-specific options from the options bag.
        let options = parse_snippet_options(kind, &raw.options);
        let timeout_ms = parse_timeout_ms(&raw.options);

        // doc_order: prefer loc.doc_order from the shared counter (T-I04),
        // fall back to JSON-array order for backward-compat with old packages.
        // Both sources produce a stable ordering for single-file documents.
        let doc_order = raw.loc.as_ref().map(|l| l.doc_order).unwrap_or(json_order);

        snippets.push(Snippet {
            id,
            kind,
            file: entry.to_path_buf(),
            doc_order,
            src: raw.src,
            options,
            timeout_ms,
        });
    }

    // Build dep snippets from <evcxr-dep> and interleave by doc_order.
    // The package now emits loc.doc_order via the shared counter; when absent
    // (old packages), place deps before all snippets (conservative: better to
    // have deps resolve early than miss them).
    for (dep_idx, raw_dep) in raw_deps.into_iter().enumerate() {
        let dep_doc_order = raw_dep
            .loc
            .map(|l| l.doc_order)
            // Fallback: negative-order-equivalent by using 0-offset when no
            // loc is present. Use dep_idx to preserve relative dep ordering.
            .unwrap_or(dep_idx);

        let dep_id = raw_dep
            .id
            .unwrap_or_else(|| identity::default_id(&format!("dep:{}", raw_dep.spec)));

        snippets.push(Snippet {
            id: dep_id,
            kind: SnippetKind::Dep,
            file: entry.to_path_buf(),
            doc_order: dep_doc_order,
            src: String::new(),
            options: SnippetOptions::Dep {
                spec: raw_dep.spec,
                version: raw_dep.version,
                features: raw_dep.features,
            },
            timeout_ms: None,
        });
    }

    // Sort by doc_order so the eval loop sees everything in document order.
    snippets.sort_by_key(|s| s.doc_order);

    Ok(DiscoveryResult { snippets, min_cli })
}

/// Query `<evcxr-min-cli>` and return the highest requirement found.
///
/// The package emits `[#metadata("X.Y.Z")<evcxr-min-cli>]` when `setup()`
/// is called with `min-cli:`. Multiple nested imports may each emit one;
/// we take the highest so more-demanding transitive requirements win.
fn query_min_cli(entry: &Path, root: &Path) -> Result<Option<String>, Error> {
    let raw = run_typst_query_optional(entry, root, "<evcxr-min-cli>")?;
    if raw == b"[]" || raw.is_empty() {
        return Ok(None);
    }
    // The query returns a JSON array of the metadata values. Each value is a
    // plain string (e.g. ["0.1.0"]).
    let values: Vec<serde_json::Value> = serde_json::from_slice(&raw).unwrap_or_default();
    let mut best: Option<String> = None;
    for v in values {
        if let Some(s) = v.as_str() {
            // Take the lexicographically later semver string as the higher
            // requirement. For well-formed semver this is equivalent to a
            // numeric comparison — and if parsing fails we still get something
            // rather than nothing.
            let s = s.trim().to_owned();
            best = Some(match best.take() {
                None => s,
                Some(prev) => {
                    // Compare numerically when both parse; otherwise keep prev.
                    if is_version_higher(&s, &prev) {
                        s
                    } else {
                        prev
                    }
                }
            });
        }
    }
    Ok(best)
}

/// Returns true when `candidate` is numerically higher than `baseline`.
fn is_version_higher(candidate: &str, baseline: &str) -> bool {
    fn parse(s: &str) -> Option<(u64, u64, u64)> {
        let s = s.trim().trim_start_matches('v');
        let core = s.split(['-', '+']).next().unwrap_or(s);
        let mut p = core.splitn(3, '.');
        let major = p.next()?.parse::<u64>().ok()?;
        let minor = p.next()?.parse::<u64>().ok()?;
        let patch = p.next().unwrap_or("0").parse::<u64>().ok()?;
        Some((major, minor, patch))
    }

    match (parse(candidate), parse(baseline)) {
        (Some(c), Some(b)) => c > b,
        _ => false,
    }
}

fn run_typst_query(entry: &Path, root: &Path, selector: &str) -> Result<Vec<u8>, Error> {
    let output = Command::new("typst")
        .arg("query")
        .arg("--root")
        .arg(root)
        .arg("--field")
        .arg("value")
        .arg(entry)
        .arg(selector)
        .output()
        .map_err(|e| Error::Discovery(format!("failed to spawn `typst query`: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Discovery(format!(
            "`typst query` failed (status {}): {stderr}",
            output.status
        )));
    }
    Ok(output.stdout)
}

/// Like [`run_typst_query`] but treats a non-zero exit as an empty result
/// (`[]`). Used for the `<evcxr-dep>` query: documents that don't use
/// `dep(...)` simply have no metadata markers, and `typst query` may return
/// exit 1 with "no results" on some Typst versions.
fn run_typst_query_optional(entry: &Path, root: &Path, selector: &str) -> Result<Vec<u8>, Error> {
    let output = Command::new("typst")
        .arg("query")
        .arg("--root")
        .arg(root)
        .arg("--field")
        .arg("value")
        .arg(entry)
        .arg(selector)
        .output()
        .map_err(|e| Error::Discovery(format!("failed to spawn `typst query`: {e}")))?;

    if !output.status.success() {
        tracing::debug!(selector, "typst query returned non-zero; treating as empty");
        return Ok(b"[]".to_vec());
    }
    Ok(output.stdout)
}

fn parse_snippet_options(kind: SnippetKind, options: &serde_json::Value) -> SnippetOptions {
    match kind {
        SnippetKind::RustDisplay => {
            let prefer = options
                .get("prefer")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "auto")
                .map(str::to_owned);
            SnippetOptions::Display { prefer }
        }
        SnippetKind::RustData => {
            let format = options
                .get("format")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "auto")
                .map(str::to_owned);
            SnippetOptions::Data { format }
        }
        _ => SnippetOptions::None,
    }
}

/// Parse the optional `timeout:` kwarg from the options bag into milliseconds.
///
/// Typst sends `timeout: auto` (null), a duration dict, an integer (seconds),
/// or a string like "30s". We accept the common formats and ignore unknown
/// ones, falling back to `None` (use the global default).
fn parse_timeout_ms(options: &serde_json::Value) -> Option<u64> {
    let v = options.get("timeout")?;
    if v.is_null() {
        return None;
    }
    // Integer → seconds.
    if let Some(secs) = v.as_u64() {
        return Some(secs * 1000);
    }
    // String "30s", "5min", "1000ms", etc.
    if let Some(s) = v.as_str() {
        if s == "auto" || s == "none" {
            return None;
        }
        if let Some(ms) = s.strip_suffix("ms").and_then(|n| n.parse::<u64>().ok()) {
            return Some(ms);
        }
        if let Some(s2) = s.strip_suffix("min").and_then(|n| n.parse::<u64>().ok()) {
            return Some(s2 * 60 * 1000);
        }
        if let Some(s2) = s.strip_suffix('s').and_then(|n| n.parse::<u64>().ok()) {
            return Some(s2 * 1000);
        }
    }
    None
}

fn parse_kind(s: &str) -> Option<SnippetKind> {
    Some(match s {
        "rust" => SnippetKind::Rust,
        "rust-out" => SnippetKind::RustOut,
        "rust-display" => SnippetKind::RustDisplay,
        "rust-hidden" => SnippetKind::RustHidden,
        "rust-data" => SnippetKind::RustData,
        "rust-main" => SnippetKind::RustMain,
        "dep" => SnippetKind::Dep,
        "setup" => SnippetKind::Setup,
        _ => return None,
    })
}

pub(crate) fn default_root_for(entry: &Path) -> PathBuf {
    entry
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
