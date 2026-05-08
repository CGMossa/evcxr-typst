// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Snippet-output cache: content-addressed store (CAS) + id-addressed view.
//!
//! See `docs/design/cache.md` for the full layout and key formula.
//! The CAS lives at `.evcxr-typst-cache/v1/cas/<XX>/<64-hex>/`; the
//! id-addressed view is materialised as hardlinks (or copies on cross-fs) at
//! `.evcxr-typst-cache/<id>.<ext>`. Typst only ever reads the view; it has no
//! awareness of the CAS.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use blake3::Hasher;
use serde_json::json;

use crate::Snippet;

const CAS_SUBDIR: &str = "v1/cas";
const TMP_SUBDIR: &str = "v1/tmp";
const INDEX_PATH: &str = "v1/index.json";
const README_CONTENT: &str = "Managed by evcxr-typst. Safe to delete the whole directory.\n";

/// Pre-computed environment inputs shared across all snippet keys in one run.
pub(crate) struct CacheEnv {
    pub evcxr_version: String,
    pub rustc_version: String,
    pub target_triple: String,
    /// Sorted KEY=VALUE\n lines per cache.md § "Env passthrough".
    pub passthrough: String,
}

impl CacheEnv {
    /// Populate from the current process environment, capturing rustc version once.
    pub(crate) fn collect(env_passthrough_keys: &[String]) -> Self {
        let (rustc_version, target_triple) = probe_rustc();
        let mut kv_pairs: Vec<String> = env_passthrough_keys
            .iter()
            .map(|k| {
                let v = std::env::var(k).unwrap_or_default();
                format!("{k}={v}\n")
            })
            .collect();
        kv_pairs.sort();
        let passthrough = kv_pairs.join("");

        Self {
            evcxr_version: env!("CARGO_PKG_VERSION").to_owned(),
            rustc_version,
            target_triple,
            passthrough,
        }
    }
}

fn probe_rustc() -> (String, String) {
    let out = std::process::Command::new("rustc").arg("-vV").output().ok();
    let Some(out) = out else {
        return ("unknown".to_owned(), "unknown".to_owned());
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let version = text
        .lines()
        .find(|l| l.starts_with("rustc "))
        .unwrap_or("rustc unknown")
        .to_owned();
    let target = text
        .lines()
        .find(|l| l.starts_with("host: "))
        .and_then(|l| l.strip_prefix("host: "))
        .unwrap_or("unknown")
        .to_owned();
    (version, target)
}

/// Compute the 64-hex cache key for a snippet.
///
/// `prior_chain` is a running Merkle chain over all earlier snippets' sources.
/// `active_deps` is all Dep snippets that appear at-or-before this snippet in doc
/// order, serialised canonically so the dep set is part of the key.
pub(crate) fn compute_key(
    snippet: &Snippet,
    prior_chain: &[u8; 32],
    active_deps: &[&Snippet],
    env: &CacheEnv,
) -> String {
    let mut h = Hasher::new();
    h.update(b"evcxr-typst-cache/v1\n");
    h.update(b"src=");
    h.update(snippet.src.as_bytes());
    h.update(b"\n");
    h.update(b"prior=");
    h.update(prior_chain);
    h.update(b"\n");
    h.update(b"deps=");
    h.update(deps_canonical(active_deps).as_bytes());
    h.update(b"\n");
    h.update(b"evcxr=");
    h.update(env.evcxr_version.as_bytes());
    h.update(b"\n");
    h.update(b"rustc=");
    h.update(env.rustc_version.as_bytes());
    h.update(b"\n");
    h.update(b"target=");
    h.update(env.target_triple.as_bytes());
    h.update(b"\n");
    h.update(b"chan=");
    h.update(env.passthrough.as_bytes());
    h.update(b"\n");
    h.update(b"sver=1\n");
    h.finalize().to_hex().to_string()
}

/// Advance the Merkle prior-chain by one snippet's source.
pub(crate) fn advance_chain(prior: &[u8; 32], src: &str) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(prior);
    h.update(b"\n");
    h.update(src.as_bytes());
    *h.finalize().as_bytes()
}

/// Initial prior-chain value (before any snippet).
pub(crate) fn initial_chain() -> [u8; 32] {
    *blake3::hash(b"empty-chain-v1").as_bytes()
}

fn deps_canonical(active_deps: &[&Snippet]) -> String {
    use crate::SnippetOptions;
    let mut parts: Vec<String> = active_deps
        .iter()
        .filter_map(|s| {
            if let SnippetOptions::Dep {
                spec,
                version,
                features,
            } = &s.options
            {
                let v = version.as_deref().unwrap_or("");
                let f = features.join(",");
                Some(format!("{spec}@{v}[{f}]"))
            } else {
                None
            }
        })
        .collect();
    parts.sort();
    parts.join("\n")
}

// ─── CAS paths ───────────────────────────────────────────────────────────────

fn cas_dir(cache_root: &Path, key: &str) -> PathBuf {
    let prefix = &key[..2];
    cache_root.join(CAS_SUBDIR).join(prefix).join(key)
}

fn tmp_dir(cache_root: &Path) -> PathBuf {
    cache_root.join(TMP_SUBDIR)
}

// ─── Cache entry on disk ─────────────────────────────────────────────────────

/// Result of a cache lookup.
pub(crate) enum LookupResult {
    /// Cache hit — CAS entry found, view materialised.
    Hit,
    /// Cache miss — no CAS entry for this key.
    Miss,
}

/// Look up `key` in the CAS. On hit, materialise the id-addressed view files
/// in `cache_root` (hardlinks or copies) and return `Hit`.
pub(crate) fn lookup(
    cache_root: &Path,
    key: &str,
    _snippet_id: &str,
) -> Result<LookupResult, crate::Error> {
    let cas = cas_dir(cache_root, key);
    if !cas.is_dir() {
        return Ok(LookupResult::Miss);
    }
    // Materialise each file in the CAS entry as the id-addressed view.
    materialise_view(cache_root, &cas)?;
    Ok(LookupResult::Hit)
}

/// Store the id-addressed view files into the CAS under `key`, then
/// materialise them back in the view location.
///
/// Files to store: every `<id>.*` file currently in `cache_root`.
pub(crate) fn store(cache_root: &Path, key: &str, snippet_id: &str) -> Result<(), crate::Error> {
    let tmp_staging = tmp_dir(cache_root).join(format!(
        "cas-{}-{}",
        &key[..8],
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&tmp_staging)?;

    // Collect all view files for this snippet_id.
    let prefix = format!("{snippet_id}.");
    let entries = fs::read_dir(cache_root)?;
    let mut found_any = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&prefix) && entry.path().is_file() {
            let dst = tmp_staging.join(name_str.as_ref());
            fs::copy(entry.path(), &dst)?;
            found_any = true;
        }
    }

    // Write the meta file.
    let meta = json!({"key": key, "schema": 1});
    fs::write(tmp_staging.join("meta.json"), meta.to_string().as_bytes())?;

    let cas = cas_dir(cache_root, key);
    if found_any || !cas.is_dir() {
        // Move staging dir to CAS location (atomic on POSIX same-FS).
        fs::create_dir_all(cas.parent().unwrap())?;
        if cas.is_dir() {
            // Another process beat us; keep existing — bytes identical by construction.
            fs::remove_dir_all(&tmp_staging).ok();
        } else {
            fs::rename(&tmp_staging, &cas)?;
        }
    } else {
        fs::remove_dir_all(&tmp_staging).ok();
    }

    Ok(())
}

/// Create hardlinks (or copies on cross-fs) from the CAS entry into the
/// id-addressed view location, applying D-016 skip-if-unchanged.
fn materialise_view(cache_root: &Path, cas: &Path) -> Result<(), crate::Error> {
    let entries = fs::read_dir(cas)?;
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        // Skip meta.json; it's internal to the CAS.
        if fname_str == "meta.json" {
            continue;
        }
        // Map CAS filename → view filename. CAS files are stored as-is.
        let view_path = cache_root.join(fname_str.as_ref());
        let cas_path = entry.path();

        if !cas_path.is_file() {
            continue;
        }

        // D-016: skip-if-unchanged to avoid spurious notify events.
        if view_path.exists() && files_identical(&view_path, &cas_path)? {
            continue;
        }

        // Try hardlink; fall back to copy on cross-device error (EXDEV).
        let link_result = fs::hard_link(&cas_path, &view_path);
        match link_result {
            Ok(()) => {}
            Err(e) if is_cross_device(&e) => {
                fs::copy(&cas_path, &view_path)?;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                // View file exists (not byte-identical due to concurrent write).
                // Overwrite via tmp rename.
                let tmp = view_path.with_extension("cas_tmp");
                fs::copy(&cas_path, &tmp)?;
                fs::rename(&tmp, &view_path)?;
            }
            Err(e) => return Err(crate::Error::Io(e)),
        }
    }
    Ok(())
}

fn files_identical(a: &Path, b: &Path) -> Result<bool, crate::Error> {
    let meta_a = fs::metadata(a)?;
    let meta_b = fs::metadata(b)?;
    if meta_a.len() != meta_b.len() {
        return Ok(false);
    }
    // Same size: byte-compare.
    let bytes_a = fs::read(a)?;
    let bytes_b = fs::read(b)?;
    Ok(bytes_a == bytes_b)
}

fn is_cross_device(e: &io::Error) -> bool {
    // Rust 1.75+ exposes ErrorKind::CrossesDevices, but we also check raw
    // OS error for platforms/versions where that kind is not yet mapped.
    if e.kind() == io::ErrorKind::CrossesDevices {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::io::RawFd;
        let _ = RawFd::MAX; // just to use the import
        if let Some(code) = e.raw_os_error() {
            // EXDEV = 18 on Linux/macOS
            return code == 18;
        }
    }
    false
}

// ─── Index ────────────────────────────────────────────────────────────────────

/// Read the `v1/index.json` mapping `snippet_id → cache_key`. Returns empty
/// map when the file doesn't exist (cold start).
pub(crate) fn read_index(cache_root: &Path) -> HashMap<String, String> {
    let path = cache_root.join(INDEX_PATH);
    let Ok(bytes) = fs::read(&path) else {
        return HashMap::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Atomically write the `v1/index.json` mapping `snippet_id → cache_key`.
pub(crate) fn write_index(
    cache_root: &Path,
    index: &HashMap<String, String>,
) -> Result<(), crate::Error> {
    let tmp_dir_path = tmp_dir(cache_root);
    fs::create_dir_all(&tmp_dir_path)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp = tmp_dir_path.join(format!("index-{nanos}.json"));
    let bytes = serde_json::to_vec(index)
        .map_err(|e| crate::Error::Discovery(format!("failed to serialize index.json: {e}")))?;
    fs::write(&tmp, &bytes)?;
    let dest = cache_root.join(INDEX_PATH);
    fs::create_dir_all(dest.parent().unwrap())?;
    rename_with_retry(&tmp, &dest)?;
    Ok(())
}

fn rename_with_retry(src: &Path, dst: &Path) -> Result<(), crate::Error> {
    // On Windows, rename over an existing open file may fail; retry up to 3×.
    for attempt in 0..3 {
        match fs::rename(src, dst) {
            Ok(()) => return Ok(()),
            Err(_) if attempt < 2 => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => return Err(crate::Error::Io(e)),
        }
    }
    Ok(())
}

// ─── GC ──────────────────────────────────────────────────────────────────────

/// Drop CAS entries not referenced by `v1/index.json`, and clean up `v1/tmp/`.
pub(crate) fn gc(cache_root: &Path) -> Result<usize, crate::Error> {
    let index = read_index(cache_root);
    let referenced: std::collections::HashSet<String> = index.into_values().collect();

    let cas_root = cache_root.join(CAS_SUBDIR);
    if !cas_root.is_dir() {
        return Ok(0);
    }

    let mut removed = 0usize;
    // Walk cas/<XX>/<full-key>/
    for prefix_entry in fs::read_dir(&cas_root)?.flatten() {
        let prefix_path = prefix_entry.path();
        if !prefix_path.is_dir() {
            continue;
        }
        for key_entry in fs::read_dir(&prefix_path)?.flatten() {
            let key_path = key_entry.path();
            if !key_path.is_dir() {
                continue;
            }
            let key_name = key_entry.file_name();
            let key_str = key_name.to_string_lossy();
            if !referenced.contains(key_str.as_ref()) {
                fs::remove_dir_all(&key_path)?;
                removed += 1;
            }
        }
        // Remove empty prefix dir.
        let _ = fs::remove_dir(&prefix_path);
    }

    // Clean up v1/tmp/.
    let tmp = tmp_dir(cache_root);
    if tmp.is_dir() {
        for entry in fs::read_dir(&tmp)?.flatten() {
            fs::remove_dir_all(entry.path()).ok();
            fs::remove_file(entry.path()).ok();
        }
    }

    Ok(removed)
}

// ─── View cleanup ─────────────────────────────────────────────────────────────

/// Remove id-addressed view files for a specific snippet from the cache root.
/// Does NOT touch the CAS.
pub(crate) fn drop_view_for_id(cache_root: &Path, snippet_id: &str) {
    let prefix = format!("{snippet_id}.");
    if let Ok(entries) = fs::read_dir(cache_root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with(&prefix) && entry.path().is_file() {
                fs::remove_file(entry.path()).ok();
            }
        }
    }
}

/// Clean the id-addressed view: remove all files in `cache_root` that are NOT
/// under `v1/`. Preserves `v1/cas/` and `v1/tmp/`.
pub(crate) fn clean_view(cache_root: &Path) -> Result<(), crate::Error> {
    if !cache_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(cache_root)?.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Keep the v1/ subtree (contains CAS, index, tmp).
        if name_str == "v1" || name_str == "README.txt" {
            continue;
        }
        let path = entry.path();
        if path.is_file() {
            fs::remove_file(&path)?;
        } else if path.is_dir() {
            fs::remove_dir_all(&path)?;
        }
    }
    // Also remove just the index.json inside v1/ (the view index is stale
    // after a clean, but the CAS is preserved).
    let index = cache_root.join(INDEX_PATH);
    if index.exists() {
        fs::remove_file(&index)?;
    }
    Ok(())
}

/// Ensure the cache root has a README.txt.
pub(crate) fn ensure_readme(cache_root: &Path) -> Result<(), crate::Error> {
    let readme = cache_root.join("README.txt");
    if !readme.exists() {
        fs::write(&readme, README_CONTENT)?;
    }
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Snippet, SnippetKind, SnippetOptions};
    use std::path::PathBuf;

    fn make_snippet(src: &str, doc_order: usize) -> Snippet {
        Snippet {
            id: crate::identity::default_id(src),
            kind: SnippetKind::Rust,
            file: PathBuf::from("main.typ"),
            doc_order,
            src: src.to_owned(),
            options: SnippetOptions::None,
        }
    }

    fn make_env() -> CacheEnv {
        CacheEnv {
            evcxr_version: "0.1.0".to_owned(),
            rustc_version: "rustc 1.80.0".to_owned(),
            target_triple: "x86_64-unknown-linux-gnu".to_owned(),
            passthrough: String::new(),
        }
    }

    #[test]
    fn test_key_is_deterministic() {
        let s = make_snippet("println!(\"hi\");", 0);
        let chain = initial_chain();
        let env = make_env();
        let k1 = compute_key(&s, &chain, &[], &env);
        let k2 = compute_key(&s, &chain, &[], &env);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_key_changes_on_src_change() {
        let s1 = make_snippet("println!(\"hi\");", 0);
        let s2 = make_snippet("println!(\"bye\");", 0);
        let chain = initial_chain();
        let env = make_env();
        assert_ne!(
            compute_key(&s1, &chain, &[], &env),
            compute_key(&s2, &chain, &[], &env)
        );
    }

    #[test]
    fn test_key_changes_on_prior_chain() {
        let s = make_snippet("println!(\"hi\");", 0);
        let chain1 = initial_chain();
        let chain2 = advance_chain(&chain1, "prior snippet src");
        let env = make_env();
        assert_ne!(
            compute_key(&s, &chain1, &[], &env),
            compute_key(&s, &chain2, &[], &env)
        );
    }

    #[test]
    fn test_key_changes_on_rustc_version() {
        let s = make_snippet("println!(\"hi\");", 0);
        let chain = initial_chain();
        let env1 = make_env();
        let env2 = CacheEnv {
            rustc_version: "rustc 1.81.0".to_owned(),
            ..make_env()
        };
        assert_ne!(
            compute_key(&s, &chain, &[], &env1),
            compute_key(&s, &chain, &[], &env2)
        );
    }

    #[test]
    fn test_key_excludes_snippet_id() {
        // Same src, different explicit IDs → same key.
        let chain = initial_chain();
        let env = make_env();
        let mut s1 = make_snippet("println!(\"hi\");", 0);
        let mut s2 = s1.clone();
        s1.id = "foo".to_owned();
        s2.id = "bar".to_owned();
        assert_eq!(
            compute_key(&s1, &chain, &[], &env),
            compute_key(&s2, &chain, &[], &env)
        );
    }

    #[test]
    fn test_hardlink_fallback_to_copy() {
        // We can't easily create cross-device scenarios, but we can test that
        // materialise_view writes files via copy when hard_link would fail.
        let dir = tempfile::tempdir().unwrap();
        let cas = dir.path().join("cas");
        fs::create_dir_all(&cas).unwrap();
        fs::write(cas.join("snippet.txt"), b"hello").unwrap();

        let view_root = dir.path().join("view");
        fs::create_dir_all(&view_root).unwrap();

        // materialise_view copies from cas to view_root.
        materialise_view(&view_root, &cas).unwrap();
        assert_eq!(fs::read(view_root.join("snippet.txt")).unwrap(), b"hello");
    }
}
