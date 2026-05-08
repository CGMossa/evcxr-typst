// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Snippet discovery via `typst query`.
//!
//! Runs two `typst query` calls — one for `<evcxr-snippet>`, one for
//! `<evcxr-dep>` — and merges the results by `doc_order`. The merged
//! list is the global evaluation order for a single entry file.

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

pub(crate) fn discover(entry: &Path, root: &Path) -> Result<Vec<Snippet>, Error> {
    tracing::debug!(entry = %entry.display(), root = %root.display(), "discover");

    let snippet_bytes = run_typst_query(entry, root, "<evcxr-snippet>")?;
    tracing::debug!(
        bytes = snippet_bytes.len(),
        "typst query <evcxr-snippet> returned"
    );

    let dep_bytes = run_typst_query_optional(entry, root, "<evcxr-dep>")?;
    tracing::debug!(bytes = dep_bytes.len(), "typst query <evcxr-dep> returned");

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
        });
    }

    // Sort by doc_order so the eval loop sees everything in document order.
    snippets.sort_by_key(|s| s.doc_order);

    Ok(snippets)
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
