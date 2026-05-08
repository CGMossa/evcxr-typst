// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Snippet discovery via `typst query`.
//!
//! Phase 1 scope (T-I03): single entry file, no `#import` walking, no
//! `<evcxr-dep>` handling, no `loc.doc_order` field on the metadata payload.
//! Document order is the JSON array order returned by `typst query`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::{Error, Snippet, SnippetKind, identity};

#[derive(Debug, Deserialize)]
pub(crate) struct RawSnippet {
    pub kind: String,
    pub id: Option<String>,
    pub src: String,
}

pub(crate) fn discover(entry: &Path, root: &Path) -> Result<Vec<Snippet>, Error> {
    tracing::debug!(entry = %entry.display(), root = %root.display(), "discover");
    let raw = run_typst_query(entry, root)?;
    tracing::debug!(bytes = raw.len(), "typst query returned");
    let raws: Vec<RawSnippet> = serde_json::from_slice(&raw).map_err(|e| {
        Error::Discovery(format!(
            "failed to parse `typst query` JSON for {}: {e}",
            entry.display()
        ))
    })?;

    let default_ids: Vec<String> = raws.iter().map(|r| identity::default_id(&r.src)).collect();
    let resolved_defaults = identity::resolve_collisions(&default_ids);

    let mut snippets = Vec::with_capacity(raws.len());
    for (doc_order, (raw, default_resolved)) in raws.into_iter().zip(resolved_defaults).enumerate()
    {
        let kind = parse_kind(&raw.kind).ok_or_else(|| {
            Error::Discovery(format!(
                "unknown snippet kind {:?} from typst query",
                raw.kind
            ))
        })?;
        let id = raw.id.clone().unwrap_or(default_resolved);
        snippets.push(Snippet {
            id,
            kind,
            file: entry.to_path_buf(),
            doc_order,
            src: raw.src,
        });
    }
    Ok(snippets)
}

fn run_typst_query(entry: &Path, root: &Path) -> Result<Vec<u8>, Error> {
    let output = Command::new("typst")
        .arg("query")
        .arg("--root")
        .arg(root)
        .arg("--field")
        .arg("value")
        .arg(entry)
        .arg("<evcxr-snippet>")
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
