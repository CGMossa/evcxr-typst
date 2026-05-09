// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! Watch loop: notify watcher + change classification + re-eval. See `docs/design/watch-loop.md`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, select};
use evcxr::CommandContext;
use notify::{Event, RecursiveMode, Watcher};

use crate::cache::{self, CacheEnv};
use crate::discovery;
use crate::eval;
use crate::{Error, Snippet, SnippetKind, WatchEval, WatchHandle, WatchOptions};

const DEBOUNCE: Duration = Duration::from_millis(150);
const EVCXR_CACHE_MB: u32 = 500;

/// Spawn the watch loop in a background thread and return a `WatchHandle`.
pub(crate) fn run(
    entry: PathBuf,
    root: PathBuf,
    snippets: Vec<Snippet>,
    options: &WatchOptions,
) -> Result<WatchHandle, Error> {
    let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
    let allow_eval = matches!(options.eval, WatchEval::Allow);

    let handle =
        thread::spawn(move || watch_thread(entry, root, snippets, allow_eval, shutdown_rx));

    Ok(WatchHandle {
        shutdown: shutdown_tx,
        thread: handle,
    })
}

fn watch_thread(
    entry: PathBuf,
    root: PathBuf,
    initial_snippets: Vec<Snippet>,
    allow_eval: bool,
    shutdown_rx: crossbeam_channel::Receiver<()>,
) -> Result<(), Error> {
    let cache_dir = cache_dir_for(&entry);
    fs::create_dir_all(&cache_dir)?;
    cache::ensure_readme(&cache_dir)?;

    let (ctx_opt, stdout_rx, stderr_rx, _stdout_drain, _stderr_drain) = if allow_eval {
        let (mut ctx, outputs) =
            CommandContext::new().map_err(|e| Error::Evcxr(format!("CommandContext::new: {e}")))?;
        // Turn on evcxr's rustc artifact cache before any snippets run.
        let _ = ctx.execute(&format!(":cache {EVCXR_CACHE_MB}"));

        let (stdout_tx, stdout_rx) = mpsc::channel::<String>();
        let (stderr_tx, stderr_rx) = mpsc::channel::<String>();
        let sd = {
            let rx = outputs.stdout.clone();
            thread::spawn(move || {
                while let Ok(line) = rx.recv() {
                    if stdout_tx.send(line).is_err() {
                        break;
                    }
                }
            })
        };
        let ed = {
            let rx = outputs.stderr.clone();
            thread::spawn(move || {
                while let Ok(line) = rx.recv() {
                    if stderr_tx.send(line).is_err() {
                        break;
                    }
                }
            })
        };
        (
            Some(ctx),
            Some(stdout_rx),
            Some(stderr_rx),
            Some(sd),
            Some(ed),
        )
    } else {
        (None, None, None, None, None)
    };

    let mut ctx_opt = ctx_opt;

    // Spawn typst watch as a child process.
    let mut typst_child = spawn_typst_watch(&entry, &root, &cache_dir)?;

    // Set up notify watcher with a crossbeam channel.
    let (notify_tx, notify_rx) = bounded::<notify::Result<Event>>(64);
    let mut watcher = notify::recommended_watcher(move |ev| {
        let _ = notify_tx.send(ev);
    })
    .map_err(|e| Error::Evcxr(format!("notify watcher: {e}")))?;
    watcher
        .watch(&entry, RecursiveMode::NonRecursive)
        .map_err(|e| Error::Evcxr(format!("watch {}: {e}", entry.display())))?;
    if let Some(parent) = entry.parent().filter(|p| p != &Path::new("")) {
        let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
    }

    let env = CacheEnv::collect(&[]);
    let mut prev_snippets = initial_snippets;
    let mut backoff = Backoff::initial();
    let mut last_event_at: Option<Instant> = None;
    // D-011: track whether the previous cycle had a panic.
    let mut prev_had_panic = false;

    loop {
        let timeout_dur = match last_event_at {
            Some(t) => {
                let elapsed = t.elapsed();
                if elapsed >= DEBOUNCE {
                    Duration::ZERO
                } else {
                    DEBOUNCE - elapsed
                }
            }
            None => {
                // No pending events; use the backoff duration.
                backoff.current
            }
        };

        select! {
            recv(shutdown_rx) -> _ => break,
            recv(notify_rx) -> ev => {
                if let Ok(Ok(event)) = ev && is_relevant(&event, &entry, &cache_dir) {
                    last_event_at = Some(Instant::now());
                }
            }
            default(timeout_dur) => {
                if let Some(t) = last_event_at && t.elapsed() >= DEBOUNCE {
                    last_event_at = None;
                    match run_one_cycle(
                        &mut ctx_opt,
                        &entry,
                        &root,
                        &cache_dir,
                        &mut prev_snippets,
                        &env,
                        allow_eval,
                        &stdout_rx,
                        &stderr_rx,
                        &mut prev_had_panic,
                    ) {
                        Ok(()) => backoff.reset(),
                        Err(CycleError::QueryFailed(e)) => {
                            tracing::warn!("typst query failed: {e}");
                            backoff.bump();
                        }
                        Err(CycleError::Fatal(e)) => return Err(e),
                    }
                }
            }
        }
    }

    // Shutdown: terminate typst watch child, drop CommandContext.
    if let Some(child) = typst_child.as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
    drop(ctx_opt);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_one_cycle(
    ctx_opt: &mut Option<CommandContext>,
    entry: &Path,
    root: &Path,
    cache_dir: &Path,
    prev: &mut Vec<Snippet>,
    env: &CacheEnv,
    allow_eval: bool,
    stdout_rx: &Option<Receiver<String>>,
    stderr_rx: &Option<Receiver<String>>,
    prev_had_panic: &mut bool,
) -> Result<(), CycleError> {
    let curr =
        discovery::discover(entry, root).map_err(|e| CycleError::QueryFailed(e.to_string()))?;

    let plan = classify(prev, &curr, *prev_had_panic);
    tracing::debug!(plan = ?plan_name(&plan), "watch cycle plan");

    *prev_had_panic = false;

    match plan {
        Plan::Noop => {
            tracing::debug!("noop: no snippet changes");
        }
        Plan::AppendOnly { new_snippets } => {
            if allow_eval && let Some(ctx) = ctx_opt.as_mut() {
                for s in &new_snippets {
                    let had_panic = eval_one(ctx, s, cache_dir, env, prev, stdout_rx, stderr_rx)?;
                    if had_panic {
                        *prev_had_panic = true;
                    }
                }
            }
        }
        Plan::TruncateOnly { removed_ids } => {
            for id in &removed_ids {
                cache::drop_view_for_id(cache_dir, id);
            }
        }
        Plan::LeafEdit { snippet } => {
            if allow_eval && let Some(ctx) = ctx_opt.as_mut() {
                let had_panic =
                    eval_one(ctx, &snippet, cache_dir, env, prev, stdout_rx, stderr_rx)?;
                if had_panic {
                    *prev_had_panic = true;
                }
            }
        }
        Plan::ResetAndReplay {
            from_index,
            snippets_to_eval,
            removed_ids,
        } => {
            if allow_eval && let Some(ctx) = ctx_opt.as_mut() {
                let _ = ctx.execute(":clear");
            }
            for id in &removed_ids {
                cache::drop_view_for_id(cache_dir, id);
            }
            if allow_eval && let Some(ctx) = ctx_opt.as_mut() {
                let _ = from_index;
                for s in &snippets_to_eval {
                    let had_panic = eval_one(ctx, s, cache_dir, env, prev, stdout_rx, stderr_rx)?;
                    if had_panic {
                        *prev_had_panic = true;
                    }
                }
            }
        }
        Plan::IdRenamed { old_id, new_id } => {
            // Rename view files and update index.json — no eval needed.
            rename_view_files(cache_dir, &old_id, &new_id);
            let mut index = cache::read_index(cache_dir);
            if let Some(key) = index.remove(&old_id) {
                index.insert(new_id.clone(), key);
                let _ = cache::write_index(cache_dir, &index);
            }
            tracing::debug!("id renamed {} → {}", old_id, new_id);
        }
    }

    *prev = curr;

    // Keep _index.json current so lib.typ can distinguish available vs.
    // SkippedNoEval snippets on the deny-eval partial-cache path (D-004).
    let _ = eval::write_available_index_for_snippets(cache_dir, prev);

    Ok(())
}

fn eval_one(
    ctx: &mut CommandContext,
    snippet: &Snippet,
    cache_dir: &Path,
    env: &CacheEnv,
    all_snippets: &[Snippet],
    stdout_rx: &Option<Receiver<String>>,
    stderr_rx: &Option<Receiver<String>>,
) -> Result<bool, CycleError> {
    if snippet.kind == SnippetKind::Dep {
        let directive = eval::format_dep_directive(snippet);
        if let Some(rx) = stdout_rx {
            drain_rx(rx);
        }
        if let Some(rx) = stderr_rx {
            drain_rx(rx);
        }
        let _ = ctx.execute(&directive);
        thread::sleep(Duration::from_millis(20));
        if let Some(rx) = stdout_rx {
            drain_rx(rx);
        }
        if let Some(rx) = stderr_rx {
            drain_rx(rx);
        }
        return Ok(false);
    }
    if !eval::is_evaluable(snippet.kind) {
        return Ok(false);
    }

    // Build prior chain up to (but not including) this snippet's doc_order.
    let mut chain = cache::initial_chain();
    let mut active_deps: Vec<&Snippet> = Vec::new();
    for s in all_snippets {
        if s.doc_order >= snippet.doc_order {
            break;
        }
        chain = cache::advance_chain(&chain, &s.src);
        if s.kind == SnippetKind::Dep {
            active_deps.push(s);
        }
    }

    let key = cache::compute_key(snippet, &chain, &active_deps, env);

    // Check CAS before evaluating.
    if let Ok(cache::LookupResult::Hit) = cache::lookup(cache_dir, &key, &snippet.id) {
        tracing::debug!(id = %snippet.id, "watch cache hit");
        return Ok(false);
    }

    if let Some(rx) = stdout_rx {
        drain_rx(rx);
    }
    if let Some(rx) = stderr_rx {
        drain_rx(rx);
    }

    let start = std::time::Instant::now();
    let exec_result = ctx.execute(&snippet.src);
    let _elapsed = start.elapsed();

    thread::sleep(Duration::from_millis(20));
    let stdout = if let Some(rx) = stdout_rx {
        collect_rx(rx)
    } else {
        String::new()
    };

    let had_panic = matches!(&exec_result, Err(evcxr::Error::SubprocessTerminated(_)));

    match exec_result {
        Ok(eval_outputs) => {
            let _ = eval::write_mime_sidecars(
                cache_dir,
                &snippet.id,
                &eval_outputs.content_by_mime_type,
                &stdout,
            );
            let _ = cache::store(cache_dir, &key, &snippet.id);
            let mut index = cache::read_index(cache_dir);
            index.insert(snippet.id.clone(), key);
            let _ = cache::write_index(cache_dir, &index);
        }
        Err(e) => {
            tracing::warn!(id = %snippet.id, error = %e, "eval error in watch cycle");
        }
    }

    Ok(had_panic)
}

fn drain_rx(rx: &Receiver<String>) {
    while rx.try_recv().is_ok() {}
}

fn collect_rx(rx: &Receiver<String>) -> String {
    let mut out = String::new();
    while let Ok(line) = rx.try_recv() {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn rename_view_files(cache_dir: &Path, old_id: &str, new_id: &str) {
    let old_prefix = format!("{old_id}.");
    if let Ok(entries) = fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&old_prefix) {
                let suffix = &name_str[old_prefix.len()..];
                let new_name = format!("{new_id}.{suffix}");
                let _ = fs::rename(entry.path(), cache_dir.join(new_name));
            }
        }
    }
}

// ─── Change classification ───────────────────────────────────────────────────

#[derive(Debug)]
enum Plan {
    Noop,
    AppendOnly {
        new_snippets: Vec<Snippet>,
    },
    TruncateOnly {
        removed_ids: Vec<String>,
    },
    LeafEdit {
        snippet: Snippet,
    },
    ResetAndReplay {
        from_index: usize,
        snippets_to_eval: Vec<Snippet>,
        removed_ids: Vec<String>,
    },
    IdRenamed {
        old_id: String,
        new_id: String,
    },
}

fn plan_name(plan: &Plan) -> &'static str {
    match plan {
        Plan::Noop => "noop",
        Plan::AppendOnly { .. } => "append_only",
        Plan::TruncateOnly { .. } => "truncate_only",
        Plan::LeafEdit { .. } => "leaf_edit",
        Plan::ResetAndReplay { .. } => "reset_and_replay",
        Plan::IdRenamed { .. } => "id_renamed",
    }
}

/// Classify the difference between `prev` and `curr` snippet lists.
///
/// When `force_reset` is true (D-011 panic), LeafEdit is upgraded to
/// ResetAndReplay to ensure the context is clean after a prior panic.
fn classify(prev: &[Snippet], curr: &[Snippet], force_reset: bool) -> Plan {
    // Build prev id → index maps.
    let prev_by_id: HashMap<&str, usize> = prev
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let curr_by_id: HashMap<&str, usize> = curr
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    // Secondary index: (src_hash, doc_order) for IdRenamed detection.
    let _prev_by_src: HashMap<(u64, usize), usize> = prev
        .iter()
        .enumerate()
        .map(|(i, s)| ((fnv_hash(&s.src), s.doc_order), i))
        .collect();

    // Pair entries.
    let mut first_change: Option<usize> = None;
    let mut all_same = true;

    let max_len = prev.len().max(curr.len());
    for i in 0..max_len {
        let p = prev.get(i);
        let c = curr.get(i);
        let same = match (p, c) {
            (Some(ps), Some(cs)) => ps.id == cs.id && ps.src == cs.src,
            _ => false,
        };
        if !same {
            all_same = false;
            if first_change.is_none() {
                first_change = Some(i);
            }
        }
    }

    if all_same {
        return Plan::Noop;
    }

    let k = first_change.unwrap_or(0);

    // AppendOnly: prev matches up to k, all additions are at end, prev.len() == k.
    if k == prev.len() && curr.len() > k {
        // All remaining curr entries are new.
        return Plan::AppendOnly {
            new_snippets: curr[k..].to_vec(),
        };
    }

    // TruncateOnly: curr ends at k, all removed from prev end.
    if k == curr.len() && prev.len() > k {
        let removed_ids = prev[k..].iter().map(|s| s.id.clone()).collect();
        return Plan::TruncateOnly { removed_ids };
    }

    // Check for IdRenamed: single change where src+docorder matches.
    if prev.len() == curr.len() {
        let changes: Vec<usize> = (0..prev.len())
            .filter(|&i| !(prev[i].id == curr[i].id && prev[i].src == curr[i].src))
            .collect();
        if changes.len() == 1 {
            let i = changes[0];
            let ps = &prev[i];
            let cs = &curr[i];
            if ps.src == cs.src && ps.id != cs.id {
                return Plan::IdRenamed {
                    old_id: ps.id.clone(),
                    new_id: cs.id.clone(),
                };
            }
        }
    }

    // LeafEdit: only change is at the last position in both lists, and it's a leaf.
    let only_last = k == prev.len().saturating_sub(1) && k == curr.len().saturating_sub(1);

    if !force_reset && only_last && prev.len() == curr.len() {
        let ps = &prev[k];
        let cs = &curr[k];
        if ps.id == cs.id && ps.src != cs.src && is_leaf(cs) {
            return Plan::LeafEdit {
                snippet: cs.clone(),
            };
        }
    }

    // Catch-all: ResetAndReplay from k.
    let removed_ids: Vec<String> = prev_by_id
        .keys()
        .filter(|id| !curr_by_id.contains_key(*id))
        .map(|s| s.to_string())
        .collect();

    Plan::ResetAndReplay {
        from_index: k,
        snippets_to_eval: curr[k..].to_vec(),
        removed_ids,
    }
}

/// Returns `true` if a snippet is a "leaf": no items, no top-level `let`,
/// no evcxr commands. See `docs/design/watch-loop.md` § 5.
pub(crate) fn is_leaf(snippet: &Snippet) -> bool {
    let src = &snippet.src;

    // Dep snippets are never leaves.
    if snippet.kind == SnippetKind::Dep {
        return false;
    }

    // A snippet with a bare evcxr command is not a leaf.
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(':') {
            return false;
        }
    }

    // Try parsing as a file (works for snippets that are valid Rust items + stmts).
    if let Ok(file) = syn::parse_str::<syn::File>(src) {
        return check_leaf_items(&file.items) && check_leaf_stmts_from_file(&file.items);
    }

    // Try wrapping in a function and parsing as a Block.
    let wrapped = format!("fn _w() {{\n{src}\n}}");
    if let Ok(file) = syn::parse_str::<syn::File>(&wrapped)
        && let Some(syn::Item::Fn(f)) = file.items.first()
    {
        return check_leaf_stmts(&f.block.stmts);
    }

    // Parsing failed → classify as non-leaf (safe: forces ResetAndReplay).
    false
}

fn check_leaf_items(items: &[syn::Item]) -> bool {
    for item in items {
        match item {
            syn::Item::Fn(_)
            | syn::Item::Struct(_)
            | syn::Item::Enum(_)
            | syn::Item::Trait(_)
            | syn::Item::Impl(_)
            | syn::Item::Mod(_)
            | syn::Item::Use(_)
            | syn::Item::ExternCrate(_)
            | syn::Item::Type(_)
            | syn::Item::Const(_)
            | syn::Item::Static(_) => return false,
            // Macro invocations ending with `;` are statement-style (e.g. println!).
            // Those without a semicolon introduce definitions (e.g. macro_rules!).
            syn::Item::Macro(m) if m.semi_token.is_none() => return false,
            _ => {}
        }
    }
    true
}

fn check_leaf_stmts_from_file(items: &[syn::Item]) -> bool {
    // At file scope, statements appear as items — we already checked those.
    // There are no bare stmt-level items at file scope in syn.
    let _ = items;
    true
}

fn check_leaf_stmts(stmts: &[syn::Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            syn::Stmt::Local(_) => return false, // top-level let
            syn::Stmt::Item(item) if !check_leaf_items(std::slice::from_ref(item)) => {
                return false;
            }
            _ => {}
        }
    }
    true
}

/// Simple FNV-1a hash for the secondary index.
fn fnv_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ─── Backoff ─────────────────────────────────────────────────────────────────

struct Backoff {
    current: Duration,
}

impl Backoff {
    fn initial() -> Self {
        Self {
            current: Duration::from_millis(250),
        }
    }

    fn bump(&mut self) {
        self.current = (self.current * 2).min(Duration::from_secs(2));
    }

    fn reset(&mut self) {
        *self = Self::initial();
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn cache_dir_for(entry: &Path) -> PathBuf {
    entry
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(eval::CACHE_DIRNAME)
}

fn is_relevant(event: &Event, entry: &Path, cache_dir: &Path) -> bool {
    // Ignore events from the cache dir (our own sidecar writes).
    for path in &event.paths {
        if path.starts_with(cache_dir) {
            return false;
        }
        if path == entry || path.parent() == entry.parent() {
            return true;
        }
    }
    false
}

fn spawn_typst_watch(
    entry: &Path,
    root: &Path,
    cache_dir: &Path,
) -> Result<Option<std::process::Child>, Error> {
    // We need cache_typst_path for --input. Derive it as in cli.rs.
    let abs_cache = std::fs::canonicalize(cache_dir)
        .or_else(|_| {
            std::fs::create_dir_all(cache_dir)?;
            std::fs::canonicalize(cache_dir)
        })
        .map_err(Error::Io)?;
    let abs_root = std::fs::canonicalize(root).map_err(Error::Io)?;
    let rel = abs_cache.strip_prefix(&abs_root).map_err(|_| {
        Error::Discovery(format!(
            "cache dir {} not inside root {}",
            abs_cache.display(),
            abs_root.display()
        ))
    })?;
    let rel_str = rel
        .to_str()
        .ok_or_else(|| Error::Discovery("non-UTF-8 cache path".into()))?;
    let cache_typst_path = format!("/{}", rel_str.replace('\\', "/"));

    let mut cmd = std::process::Command::new("typst");
    cmd.arg("watch")
        .arg("--root")
        .arg(root)
        .arg("--input")
        .arg("evcxr-mode=read")
        .arg("--input")
        .arg(format!("evcxr-cache={cache_typst_path}"))
        .arg(entry);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let child = cmd
        .spawn()
        .map_err(|e| Error::Evcxr(format!("spawn typst watch: {e}")))?;
    Ok(Some(child))
}

// ─── Cycle errors ─────────────────────────────────────────────────────────────

enum CycleError {
    QueryFailed(String),
    Fatal(Error),
}

impl From<Error> for CycleError {
    fn from(e: Error) -> Self {
        CycleError::Fatal(e)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Snippet, SnippetKind, SnippetOptions};
    use std::path::PathBuf;

    fn make_snippet(id: &str, src: &str, doc_order: usize) -> Snippet {
        Snippet {
            id: id.to_owned(),
            kind: SnippetKind::Rust,
            file: PathBuf::from("main.typ"),
            doc_order,
            src: src.to_owned(),
            options: SnippetOptions::None,
            timeout_ms: None,
        }
    }

    #[test]
    fn test_classify_noop() {
        let s = make_snippet("a", "println!(\"hi\");", 0);
        let prev = [s.clone()];
        let curr = [s];
        let plan = classify(&prev, &curr, false);
        assert!(matches!(plan, Plan::Noop));
    }

    #[test]
    fn test_classify_append_only() {
        let s1 = make_snippet("a", "println!(\"hi\");", 0);
        let s2 = make_snippet("b", "println!(\"bye\");", 1);
        let prev = [s1.clone()];
        let curr = [s1, s2];
        let plan = classify(&prev, &curr, false);
        assert!(matches!(plan, Plan::AppendOnly { .. }));
    }

    #[test]
    fn test_classify_truncate_only() {
        let s1 = make_snippet("a", "println!(\"hi\");", 0);
        let s2 = make_snippet("b", "println!(\"bye\");", 1);
        let plan = classify(&[s1.clone(), s2], &[s1], false);
        assert!(matches!(plan, Plan::TruncateOnly { .. }));
    }

    #[test]
    fn test_classify_leaf_edit_last() {
        let s1 = make_snippet("a", "println!(\"hi\");", 0);
        let s2a = make_snippet("b", "println!(\"old\");", 1);
        let s2b = make_snippet("b", "println!(\"new\");", 1);
        let plan = classify(&[s1.clone(), s2a], &[s1, s2b], false);
        assert!(matches!(plan, Plan::LeafEdit { .. }));
    }

    #[test]
    fn test_classify_leaf_edit_non_leaf() {
        let s1 = make_snippet("a", "println!(\"hi\");", 0);
        let s2a = make_snippet("b", "println!(\"old\");", 1);
        // fn definition — not a leaf.
        let s2b = make_snippet("b", "fn helper() {} helper();", 1);
        let plan = classify(&[s1.clone(), s2a], &[s1, s2b], false);
        assert!(matches!(plan, Plan::ResetAndReplay { .. }));
    }

    #[test]
    fn test_classify_middle_edit() {
        let s1 = make_snippet("a", "println!(\"1\");", 0);
        let s2a = make_snippet("b", "println!(\"old\");", 1);
        let s2b = make_snippet("b", "println!(\"new\");", 1);
        let s3 = make_snippet("c", "println!(\"3\");", 2);
        let plan = classify(&[s1.clone(), s2a, s3.clone()], &[s1, s2b, s3], false);
        assert!(matches!(plan, Plan::ResetAndReplay { from_index: 1, .. }));
    }

    #[test]
    fn test_classify_id_renamed() {
        let s_old = make_snippet("old-id", "println!(\"hi\");", 0);
        let s_new = make_snippet("new-id", "println!(\"hi\");", 0);
        let plan = classify(&[s_old], &[s_new], false);
        assert!(matches!(plan, Plan::IdRenamed { .. }));
    }

    #[test]
    fn test_is_leaf_cases() {
        let cases: &[(&str, bool)] = &[
            ("println!(\"hi\");", true),
            ("let x = 5; println!(\"{x}\");", false), // top-level let
            ("{ let x = 5; println!(\"{x}\"); }", true), // let inside block
            ("fn helper() {} helper();", false),      // fn item
            ("for i in 0..3 { let j = i; println!(\"{j}\"); }", true), // let inside for
            (":dep regex = \"1\"", false),            // evcxr command
        ];

        for (src, expected) in cases {
            let s = make_snippet("x", src, 0);
            let got = is_leaf(&s);
            assert_eq!(got, *expected, "src={src:?} expected={expected} got={got}");
        }
    }
}
