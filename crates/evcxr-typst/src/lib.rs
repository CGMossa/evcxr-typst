// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.

//! evcxr-typst — embed Rust evaluation in Typst documents.
//!
//! This crate ships both the `evcxr-typst` CLI binary (the canonical
//! embedder) and a small library API for hosts that want to drive the same
//! discover → evaluate → write-sidecars loop programmatically: IDE servers,
//! custom test runners, mdBook plugins, CI integrations, future Jupyter-style
//! live servers.
//!
//! # Embedder contract
//!
//! [`evcxr::runtime_hook`] **must** be called as the very first thing in
//! `main()`, before any other code runs. evcxr re-enters the host binary as
//! a child process or as a rustc wrapper depending on env vars; if anything
//! else runs first, that path breaks silently. Library functions never call
//! `runtime_hook` themselves — it is the embedder's responsibility. See
//! `docs/DECISIONS.md` D-023.
//!
//! # Stability
//!
//! Pre-1.0: explicitly unstable. SemVer-minor bumps may break compile-time
//! API.
//!
//! # Example
//!
//! ```ignore
//! use evcxr_typst::{EvalOptions, Project};
//!
//! evcxr::runtime_hook();
//!
//! let mut project = Project::open("main.typ")?;
//! let report = project.evaluate(&mut EvalOptions::allow_eval())?;
//! for s in &report.snippets {
//!     println!("{}: {:?}", s.id, s.outcome);
//! }
//! # Ok::<(), evcxr_typst::Error>(())
//! ```

#![warn(missing_docs)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod cache;
mod discovery;
mod error_capture;
mod eval;
mod identity;
mod version_check;
mod watch;

/// Errors returned by the library.
///
/// Pre-1.0 the variant set is unstable; new kinds may appear in minor
/// releases. Match exhaustively at your own risk.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The requested operation has no body yet. T-L01 ships the API
    /// surface; T-I03 onward fills the bodies in. The argument names the
    /// method.
    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),

    /// I/O failure while discovering, watching, or writing sidecars.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// `typst query` failed, returned malformed JSON, or named an unknown
    /// snippet kind. The string is a one-line human-readable explanation.
    #[error("snippet discovery failed: {0}")]
    Discovery(String),

    /// Failure spawning or driving the embedded evcxr child process. The
    /// string is the underlying evcxr error rendered with `Display`.
    #[error("evcxr error: {0}")]
    Evcxr(String),

    /// The document requires a newer `evcxr-typst` than the one running
    /// (D-019 `min-cli` enforcement). The CLI maps this to exit code 2.
    #[error(
        "this document requires evcxr-typst >= {required}; you have {actual}\n\
         \tinstall a newer version: cargo install evcxr-typst"
    )]
    IncompatibleCliVersion {
        /// The `min-cli` version declared by the document.
        required: String,
        /// The version of the running CLI (`CARGO_PKG_VERSION`).
        actual: String,
    },
}

/// Kind-specific options carried by a [`Snippet`].
///
/// Parsed from the `options` bag in the `<evcxr-snippet>` / `<evcxr-dep>`
/// metadata payload. The variants mirror the subset of kwargs that affect
/// evaluation or sidecar selection — display-only kwargs (e.g. `caption`,
/// `render`) are irrelevant to the CLI and are not stored here.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub enum SnippetOptions {
    /// No kind-specific options (most snippet kinds).
    #[default]
    None,
    /// Options for `dep(...)` snippets.
    Dep {
        /// Crate specifier: plain name (`"serde"`) or TOML fragment
        /// (`"serde = { version = \"1\", features = [\"derive\"] }"`) when the
        /// field contains `=`.
        spec: String,
        /// Version requirement, e.g. `"1"` or `"^1.2"`. `None` → latest.
        version: Option<String>,
        /// Feature flags to enable.
        features: Vec<String>,
    },
    /// Options for `rust-display(...)` snippets.
    Display {
        /// Preferred output MIME type (`"image/png"`, `"image/svg+xml"`, …).
        /// `None` → default priority order.
        prefer: Option<String>,
    },
    /// Options for `rust-data(...)` snippets.
    Data {
        /// Desired deserialization format: `"json"` | `"cbor"` | `"auto"`.
        /// `None` / `"auto"` → sniff from available sidecars.
        format: Option<String>,
    },
}

/// A discovered Rust snippet within a Typst project.
///
/// Identity, ordering, and source text. The actual evaluation outcome lives
/// in [`SnippetResult`].
#[derive(Debug, Clone)]
pub struct Snippet {
    /// Resolved snippet ID after collision handling.
    /// See `docs/design/snippet-identity.md`.
    pub id: String,
    /// Which package function produced this snippet (`rust`, `rust-out`, …).
    pub kind: SnippetKind,
    /// `.typ` file the snippet was discovered in.
    pub file: PathBuf,
    /// Document order across the entire project (D-018: global order across
    /// the entry file and its imports).
    pub doc_order: usize,
    /// Verbatim Rust source captured from the metadata `src` field.
    /// For `Dep` snippets this is always empty (`""`); the eval loop builds
    /// the `:dep` directive from [`Snippet::options`] instead.
    pub src: String,
    /// Kind-specific options parsed from the metadata payload.
    pub options: SnippetOptions,
    /// Per-snippet timeout override from the `timeout:` kwarg (D-017).
    /// `None` → use the global `EvalOptions::snippet_timeout` (default 30 s).
    pub timeout_ms: Option<u64>,
}

/// The kind of snippet, mirroring the seven public package functions in
/// `docs/design/package-api.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SnippetKind {
    /// `rust(...)` — full evaluation with stdout + display output.
    Rust,
    /// `rust-out(...)` — stdout-only.
    RustOut,
    /// `rust-display(...)` — display rendering only.
    RustDisplay,
    /// `rust-hidden(...)` — eval with no rendered output.
    RustHidden,
    /// `rust-data(...)` — return value surfaced as Typst data.
    RustData,
    /// `rust-main(...)` — snippet contains `fn main()`; evaluator
    /// synthesises a `main()` call after definition (D-024).
    RustMain,
    /// `dep(...)` — dependency declaration.
    Dep,
    /// `setup(...)` — project-level configuration.
    Setup,
}

/// The outcome of evaluating a single snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SnippetOutcome {
    /// Evaluated successfully.
    Ok,
    /// Compile error; `<id>.error.json` was written (D-006 sidecar schema).
    CompileError,
    /// Panicked at runtime.
    RuntimePanic,
    /// Exceeded the configured timeout (D-009 / D-017).
    Timeout,
    /// `:dep` resolution failed before evaluation could start.
    DepResolutionError,
    /// `EvalOptions::deny()` was active; snippet not run.
    SkippedNoEval,
    /// Cache hit — sidecar reused, no re-eval (D-010).
    CacheHit,
}

/// Result of evaluating a single snippet.
#[derive(Debug, Clone)]
pub struct SnippetResult {
    /// Resolved snippet ID (matches a [`Snippet::id`] in the project).
    pub id: String,
    /// What happened.
    pub outcome: SnippetOutcome,
    /// Captured stdout, if any.
    pub stdout: String,
    /// Captured stderr, if any.
    pub stderr: String,
    /// Wall-clock time spent in this snippet (zero on cache hit).
    pub elapsed: Duration,
    /// Paths of every sidecar file written for this snippet (T-I04 MIME
    /// passthrough). Empty when no sidecars were written.
    pub mime_sidecars: Vec<PathBuf>,
}

/// A `[dependencies]` entry that successfully resolved during evaluation.
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    /// Crate name.
    pub name: String,
    /// Resolved version (after `:dep` evaluation).
    pub version: String,
}

/// Information about a `:dep` failure surfaced to callbacks.
/// See `docs/design/errors.md` § 1.d.
#[derive(Debug, Clone)]
pub struct DepError {
    /// Crate name that failed to resolve.
    pub name: String,
    /// Human-readable error message from cargo.
    pub message: String,
}

/// Project-level configuration overrides for [`Project::open_with_config`].
///
/// See D-018 (`evcxr-typst.toml`).
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct ProjectConfig {
    /// Path to an explicit `evcxr-typst.toml` (otherwise auto-discovered
    /// next to the entry file).
    pub config_path: Option<PathBuf>,
    /// Override the project root passed to `typst query` / `typst compile`.
    /// When `None`, the entry file's parent directory is used (matches
    /// Typst's own default).
    pub root: Option<PathBuf>,
}

impl ProjectConfig {
    /// Empty configuration (matches [`ProjectConfig::default`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the project root passed to `typst query` / `typst compile`.
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = Some(root.into());
        self
    }

    /// Point at an explicit `evcxr-typst.toml` (otherwise auto-discovered).
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }
}

/// Options controlling [`Project::evaluate`].
///
/// Built fluently; `EvalOptions::deny()` is the safe default and the
/// library equivalent of the CLI's `--allow-eval` requirement (D-004).
#[allow(dead_code)] // fields populated here; consumers via T-I03 onward.
pub struct EvalOptions {
    allow_eval: bool,
    snippet_timeout: Option<Duration>,
    callbacks: Option<Box<dyn EvalCallbacks>>,
    env_passthrough: Option<Vec<String>>,
}

impl std::fmt::Debug for EvalOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvalOptions")
            .field("allow_eval", &self.allow_eval)
            .field("snippet_timeout", &self.snippet_timeout)
            .field("callbacks", &self.callbacks.as_ref().map(|_| "<dyn>"))
            .field("env_passthrough", &self.env_passthrough)
            .finish()
    }
}

impl EvalOptions {
    /// Default deny: refuses to evaluate. All snippets resolve to
    /// [`SnippetOutcome::SkippedNoEval`]. Mirrors D-004.
    pub fn deny() -> Self {
        Self {
            allow_eval: false,
            snippet_timeout: None,
            callbacks: None,
            env_passthrough: None,
        }
    }

    /// Enable Rust evaluation. Equivalent to the CLI's `--allow-eval`.
    pub fn allow_eval() -> Self {
        Self {
            allow_eval: true,
            ..Self::deny()
        }
    }

    /// Override the per-snippet timeout. `None` disables the timeout
    /// entirely; `Some(d)` overrides the default (D-009: 30s; D-017).
    pub fn with_snippet_timeout(mut self, t: Option<Duration>) -> Self {
        self.snippet_timeout = t;
        self
    }

    /// Install lifecycle callbacks for snippet-start / snippet-finish /
    /// sidecar-written / dep-resolution events.
    pub fn with_callbacks(mut self, cb: Box<dyn EvalCallbacks>) -> Self {
        self.callbacks = Some(cb);
        self
    }

    /// Override the env-var allowlist passed through to the evcxr child.
    /// See `docs/design/cache.md` § "Env passthrough".
    pub fn with_env_passthrough(mut self, keys: Vec<String>) -> Self {
        self.env_passthrough = Some(keys);
        self
    }

    /// Whether Rust evaluation is allowed in this configuration.
    pub fn is_allowed(&self) -> bool {
        self.allow_eval
    }
}

impl Default for EvalOptions {
    fn default() -> Self {
        Self::deny()
    }
}

/// Lifecycle hooks invoked by [`Project::evaluate`] / [`Project::watch`].
///
/// All methods have a default empty implementation; consumers override only
/// the events they care about. Implementors must be `Send` so the watch
/// loop can call them from its own thread.
pub trait EvalCallbacks: Send {
    /// Called just before a snippet's evaluation starts.
    fn on_snippet_start(&mut self, _snippet: &Snippet) {}
    /// Called after a snippet finishes, with the resolved outcome.
    fn on_snippet_finish(&mut self, _snippet: &Snippet, _outcome: &SnippetOutcome) {}
    /// Called once per sidecar file written for the snippet. Fires once per
    /// file (e.g. once for `.png`, separately for `.txt`, separately for
    /// `.manifest.json`), not once per snippet.
    fn on_sidecar_written(&mut self, _snippet: &Snippet, _path: &Path) {}
    /// Called when `:dep` resolution begins for a crate.
    fn on_dep_resolution_start(&mut self, _crate_name: &str) {}
    /// Called when `:dep` resolution fails.
    fn on_dep_resolution_error(&mut self, _err: &DepError) {}
}

/// Options controlling [`Project::watch`].
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct WatchOptions {
    #[allow(dead_code)]
    eval: WatchEval,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) enum WatchEval {
    #[default]
    Deny,
    Allow,
}

impl WatchOptions {
    /// Default deny: file changes are observed but no evaluation occurs.
    pub fn deny() -> Self {
        Self {
            eval: WatchEval::Deny,
        }
    }

    /// Enable Rust evaluation in the watch loop.
    pub fn allow_eval() -> Self {
        Self {
            eval: WatchEval::Allow,
        }
    }
}

/// Handle to a running watch loop.
///
/// Dropping the handle is equivalent to calling [`WatchHandle::join`] and
/// discarding the `Result`; errors will be `tracing::warn!`-logged. Use
/// [`WatchHandle::join`] to handle errors explicitly.
pub struct WatchHandle {
    pub(crate) shutdown: crossbeam_channel::Sender<()>,
    pub(crate) thread: Option<std::thread::JoinHandle<Result<(), Error>>>,
}

impl WatchHandle {
    /// Block until the watch loop exits.
    ///
    /// Sends the shutdown signal then waits for the thread to finish.
    pub fn join(mut self) -> Result<(), Error> {
        let _ = self.shutdown.try_send(());
        self.thread
            .take()
            .expect("thread handle missing (already joined)")
            .join()
            .unwrap_or_else(|_| Err(Error::Evcxr("watch thread panicked".into())))
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = self.shutdown.try_send(());
            match thread.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("watch thread error on drop: {e}"),
                Err(_) => tracing::warn!("watch thread panicked on drop"),
            }
        }
    }
}

/// Aggregate result of [`Project::evaluate`].
#[derive(Debug, Clone, Default)]
pub struct EvaluationReport {
    /// Per-snippet results in document order.
    pub snippets: Vec<SnippetResult>,
    /// `dep()` declarations resolved during this run.
    pub deps_resolved: Vec<ResolvedDep>,
    /// Wall-clock time for the whole `evaluate()` call.
    pub elapsed: Duration,
    /// Snippets served from the output cache (D-010).
    pub cache_hits: usize,
    /// Snippets that re-evaluated.
    pub cache_misses: usize,
    /// Validation issues found on the deny-eval path: `(snippet_id, reason)`.
    pub validation_issues: Vec<(String, String)>,
}

/// A Typst document plus its discovered snippet set.
///
/// Single-entry-file scope per D-018; multi-entry-file projects are
/// deferred to v1.
pub struct Project {
    entry: PathBuf,
    root: PathBuf,
    #[allow(dead_code)] // honoured once D-018 config-file support lands.
    config: ProjectConfig,
    snippets: Vec<Snippet>,
}

impl Project {
    /// Open `entry` (a `.typ` file) and discover its snippets, including
    /// imported files (per D-018). Does not evaluate.
    pub fn open(entry: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_with_config(entry, ProjectConfig::default())
    }

    /// Same as [`Project::open`], plus an explicit configuration override
    /// (e.g. a custom `evcxr-typst.toml` path).
    pub fn open_with_config(entry: impl AsRef<Path>, config: ProjectConfig) -> Result<Self, Error> {
        let entry = entry.as_ref().to_path_buf();
        let root = config
            .root
            .clone()
            .unwrap_or_else(|| discovery::default_root_for(&entry));
        let result = discovery::discover(&entry, &root)?;

        // Enforce min-cli before evaluation (D-019). Checked here so library
        // callers get the same protection as CLI callers.
        if let Err((required, actual)) = version_check::check(result.min_cli.as_deref()) {
            return Err(Error::IncompatibleCliVersion { required, actual });
        }

        Ok(Self {
            entry,
            root,
            config,
            snippets: result.snippets,
        })
    }

    /// The project's entry `.typ` file.
    pub fn entry(&self) -> &Path {
        &self.entry
    }

    /// The project root passed to Typst (the `--root` argument).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The discovered snippet list, in global document order.
    pub fn snippets(&self) -> &[Snippet] {
        &self.snippets
    }

    /// Evaluate every snippet, write sidecars per the snippet-output cache
    /// (D-010), and return a structured report.
    pub fn evaluate(&mut self, options: &mut EvalOptions) -> Result<EvaluationReport, Error> {
        let start = Instant::now();
        let cache_dir = self.cache_dir();

        let (snippet_results, hits, misses, validation_issues) = if options.allow_eval {
            // WHY: Take callbacks out into a local so eval::run can borrow them
            // mutably. Leaving them in options.callbacks (behind &mut EvalOptions)
            // triggers a compiler invariance error because Box<dyn EvalCallbacks>
            // carries an implicit 'static bound and &mut references are invariant.
            let mut cb_box: Option<Box<dyn EvalCallbacks>> = options.callbacks.take();
            let cb: Option<&mut dyn EvalCallbacks> = match cb_box.as_mut() {
                Some(b) => Some(b.as_mut()),
                None => None,
            };
            let outcome = eval::run(&self.snippets, &cache_dir, cb, options.snippet_timeout)?;
            options.callbacks = cb_box;
            let h = outcome.cache_hits;
            let m = outcome.cache_misses;
            (outcome.results, h, m, Vec::new())
        } else {
            let outcome = eval::skip_all_with_cache(&self.snippets, &cache_dir)?;
            let h = outcome.cache_hits;
            let m = outcome.cache_misses;
            let issues = eval::validate_sidecars(&self.snippets, &outcome.results, &cache_dir);
            (outcome.results, h, m, issues)
        };

        // Write _index.json listing IDs with materialised sidecars so the
        // Typst package can guard json() calls on missing manifests (D-004).
        let available: Vec<&str> = snippet_results
            .iter()
            .filter(|r| matches!(r.outcome, SnippetOutcome::Ok | SnippetOutcome::CacheHit))
            .map(|r| r.id.as_str())
            .collect();
        eval::write_available_index(&cache_dir, &available)?;

        Ok(EvaluationReport {
            snippets: snippet_results,
            deps_resolved: Vec::new(),
            elapsed: start.elapsed(),
            cache_hits: hits,
            cache_misses: misses,
            validation_issues,
        })
    }

    /// Spawn the watch loop. The returned [`WatchHandle`] keeps the loop
    /// alive; drop or [`WatchHandle::join`] to stop it.
    pub fn watch(&mut self, options: &WatchOptions) -> Result<WatchHandle, Error> {
        watch::run(
            self.entry.clone(),
            self.root.clone(),
            self.snippets.clone(),
            options,
        )
    }

    /// Drop materialised sidecars for this project. CAS contents are
    /// preserved (D-010).
    pub fn clean_view(&self) -> Result<(), Error> {
        cache::clean_view(&self.cache_dir())
    }

    /// Run the cache GC: drop CAS entries not referenced by the current
    /// `v1/index.json`. Returns the number of CAS entries removed.
    pub fn gc(&self) -> Result<usize, Error> {
        cache::gc(&self.cache_dir())
    }

    fn cache_dir(&self) -> PathBuf {
        self.entry
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(eval::CACHE_DIRNAME)
    }

    /// Cache directory expressed as a Typst absolute path relative to
    /// [`Project::root`] (always starts with `/`). Suitable for passing
    /// through to `typst compile --input evcxr-cache=…` so the package's
    /// `read(…)` calls resolve from the project root regardless of which
    /// `.typ` file issued them.
    pub fn cache_dir_typst_path(&self) -> Result<String, Error> {
        let cache = self.cache_dir();
        let abs_cache = std::fs::canonicalize(&cache).or_else(|_| {
            std::fs::create_dir_all(&cache)?;
            std::fs::canonicalize(&cache)
        })?;
        let abs_root = std::fs::canonicalize(&self.root)?;
        let rel = abs_cache.strip_prefix(&abs_root).map_err(|_| {
            Error::Discovery(format!(
                "cache dir {} is not inside project root {}",
                abs_cache.display(),
                abs_root.display(),
            ))
        })?;
        let rel_str = rel
            .to_str()
            .ok_or_else(|| Error::Discovery(format!("non-UTF-8 cache path: {}", rel.display())))?;
        Ok(format!("/{}", rel_str.replace('\\', "/")))
    }
}
