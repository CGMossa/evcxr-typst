# Library API for evcxr-typst

> The `evcxr-typst` CLI binary is one host. Other hosts (IDE servers, custom test runners, mdBook plugins, CI integrations, future Jupyter-style live servers) need to drive the same prequery loop programmatically. This doc designs the public Rust library API that lets them.

**Scope:** small. Most of the work is API hygiene — the public surface is what the binary uses; if it's clean enough to embed, it's clean enough to maintain. No new feature ships from this doc; we just expose what's already being built.

## Why now (and not later)

The CLI scaffolding shipped in `fa90905` has only a `main.rs`. T-I03 onwards will write the real eval loop. **If we know the library API matters before T-I03 starts, we structure the crate that way from the start** — `main.rs` becomes a thin caller of `lib.rs` functions. Splitting later is annoying because every internal helper has to be re-checked for "should this be `pub`?" If we don't decide now, the binary will accrete logic in private functions that will never become public.

The CLI's logical phases (discover → evaluate → write sidecars → render) map to library functions cleanly. There is no "binary-only" code beyond the runtime_hook, the clap parsing, and the `eprintln!` reporting. So the library API is approximately *the binary minus those three things*.

## Existing precedent

evcxr itself ships a library API used by the Jupyter kernel and the REPL — see `evcxr_jupyter/src/main.rs` and `evcxr_repl/src/repl.rs` for two embedders, and `/Users/elea/Documents/GitHub/evcxr/evcxr/examples/example_eval.rs` for the canonical "library use" example:

```rust
// abridged from evcxr/examples/example_eval.rs
let (mut context, outputs) = EvalContext::new()?;
context.eval("let mut s = String::new();")?;
context.eval(r#"s.push_str("Hello, World!");"#)?;
context.eval(r#"println!("{}", s);"#)?;
if let Ok(line) = outputs.stdout.recv() {
    println!("{line}");
}
```

Two patterns we mirror:

1. **Channels for output.** evcxr returns a sender/receiver pair so the host can pipe stdout/stderr wherever (UI, test harness, log aggregator). We do the same for snippet-evaluation events (snippet-started, snippet-finished, sidecar-written).
2. **`runtime_hook()` is the embedder's responsibility.** We don't call it from inside library functions — the host binary must call `evcxr::runtime_hook()` first thing in `main()`, just like `example_eval.rs` does. We document this prominently because forgetting it is a fork-bomb.

## Crate structure choice

Three options:

| Option | Pros | Cons |
|---|---|---|
| Binary-only (status quo) | Simplest. | No library users at all. |
| `lib.rs` + `main.rs` in one crate | One published artifact. The binary is a thin wrapper. | Binary's clap deps are in the library's dep graph. |
| Separate `evcxr-typst-core` library crate | Cleanest dep separation. | Two crates to publish, version, and document. |

**Decision (proposed, D-023):** Option 2 — `lib.rs` + `main.rs` in `crates/evcxr-typst/`. The library is what consumers depend on; the binary is a thin wrapper that calls into it. clap stays in the binary's `[dependencies]` only via a dedicated `cli` module; the library is clap-free. We ship one crate to crates.io named `evcxr-typst`; `cargo install evcxr-typst` gets the binary; `evcxr-typst = "X.Y"` in someone else's Cargo.toml gets the library.

The "two crates" option becomes attractive only if (a) we want the library to compile to platforms the binary doesn't (unlikely), or (b) the binary's deps grow heavy (clap is small; unlikely). Punt to v1.

## Public API surface (sketch)

Concrete function names are bikeshedable; the *shapes* below are load-bearing.

```rust
//! evcxr-typst — embed Rust evaluation in Typst documents.
//!
//! See the CLI binary `evcxr-typst` for the canonical embedder. To use this
//! crate as a library:
//!
//! ```no_run
//! use evcxr_typst::{Project, EvalOptions};
//!
//! evcxr::runtime_hook();   // mandatory, first thing in main()
//!
//! let mut project = Project::open("main.typ")?;
//! let report = project.evaluate(&EvalOptions::allow_eval())?;
//! for snippet in report.snippets {
//!     println!("{}: {:?}", snippet.id, snippet.outcome);
//! }
//! # Ok::<(), evcxr_typst::Error>(())
//! ```

pub struct Project { /* … */ }

impl Project {
    /// Discover snippets in `entry` (and its imports per D-018). Resolves the
    /// workspace root. Does not evaluate.
    pub fn open(entry: impl AsRef<Path>) -> Result<Self, Error>;

    /// Same, plus an explicit `evcxr-typst.toml` override path (D-018).
    pub fn open_with_config(entry: impl AsRef<Path>, config: ProjectConfig) -> Result<Self, Error>;

    /// The discovered snippet list, in global document order.
    pub fn snippets(&self) -> &[Snippet];

    /// Evaluate every snippet. Writes sidecars per the snippet-output cache
    /// (D-010). Returns a structured report.
    pub fn evaluate(&mut self, options: &EvalOptions) -> Result<EvaluationReport, Error>;

    /// Long-running variant. Returns a handle that drives the watch loop
    /// (D-018, watch-loop.md). Caller drives it via async or thread.
    pub fn watch(&mut self, options: &WatchOptions) -> Result<WatchHandle, Error>;

    /// Drop materialised sidecars (CAS preserved per D-010).
    pub fn clean_view(&self) -> Result<(), Error>;
}

pub struct EvalOptions { /* … */ }

impl EvalOptions {
    /// Default deny: refuses to evaluate. The library's equivalent of the CLI's
    /// `--allow-eval` requirement (D-004).
    pub fn deny() -> Self;
    pub fn allow_eval() -> Self;
    pub fn with_snippet_timeout(self, t: Option<Duration>) -> Self;
    pub fn with_callbacks(self, cb: Box<dyn EvalCallbacks>) -> Self;
    /// Override the env-var allowlist for the evcxr child (cache.md § "Env passthrough").
    pub fn with_env_passthrough(self, keys: Vec<String>) -> Self;
}

pub trait EvalCallbacks: Send {
    fn on_snippet_start(&mut self, _: &Snippet) {}
    fn on_snippet_finish(&mut self, _: &Snippet, _: &SnippetOutcome) {}
    fn on_sidecar_written(&mut self, _: &Snippet, _: &Path) {}
    fn on_dep_resolution_start(&mut self, _: &str) {}
    fn on_dep_resolution_error(&mut self, _: &DepError) {}
}

pub struct EvaluationReport {
    pub snippets: Vec<SnippetResult>,
    pub deps_resolved: Vec<ResolvedDep>,
    pub elapsed: Duration,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

pub enum SnippetOutcome {
    Ok,
    CompileError,        // <id>.error.json written
    RuntimePanic,
    Timeout,
    DepResolutionError,
    SkippedNoEval,       // EvalOptions::deny()
    CacheHit,
}
```

`Project` is single-document scope (D-018: single entry file in v0). Multi-entry projects are out of scope; deferred to v1 the same way the CLI defers them.

## Worked example: `examples/library_use.rs`

Ships in `crates/evcxr-typst/examples/library_use.rs`. Mirrors evcxr's own `example_eval.rs` precedent:

```rust
use evcxr_typst::{Project, EvalOptions};

fn main() -> anyhow::Result<()> {
    evcxr::runtime_hook();

    let mut project = Project::open(std::env::args().nth(1).expect("usage: library_use <main.typ>"))?;
    eprintln!("discovered {} snippets", project.snippets().len());

    let report = project.evaluate(&EvalOptions::allow_eval())?;
    println!("{} snippets evaluated, {} cache hits in {:?}",
             report.snippets.len(), report.cache_hits, report.elapsed);
    for s in &report.snippets {
        println!("  {} {:?}", s.id, s.outcome);
    }
    Ok(())
}
```

`cargo run -p evcxr-typst --example library_use -- main.typ` exercises the library path independently of the CLI.

## What stays internal (private to the crate)

- The cache-key formula (`docs/design/cache.md`). Internal; never serialised in a public type.
- The atomic-write strategy for sidecars. Internal.
- The watch loop's debounce interval (D-016). Internal — a constant, not a knob.
- The snippet-id collision resolver. Internal — the algorithm is a black-box exposed only as resolved IDs.
- The `<evcxr-snippet>` CBOR-encoding details. Internal — `Snippet` exposes typed fields, not bytes.

If a host needs any of those internals, that's a sign we missed a public-API need. Track the request, decide whether to expose.

## API stability

Pre-1.0: explicitly unstable. SemVer-minor bumps may break compile-time. We do not promise ABI/SemVer stability before `0.1.0`. Documented in `crates/evcxr-typst/README.md` (when written) and the crate's docs.rs landing page.

After 1.0: standard SemVer.

## Open questions

1. **Async vs sync at the lib boundary.** evcxr's `EvalContext` is sync. Our `Project::evaluate` could be sync (block until done) or async (return a `Future`). Recommendation: **sync** — most embedders run on a thread or worker, async would force tokio on every consumer for negligible benefit. The watch loop uses `notify` + a thread; no async. `tokio::time::timeout` per D-017 is implementation detail, not exposed.
2. **Error type.** `thiserror` for a typed `Error` enum, or `anyhow::Error` with context strings? Recommendation: **`thiserror` typed enum**. Library consumers want to match on error kinds (compile error vs cache error vs IO error); `anyhow` is ergonomic for the binary but lossy for downstreams.
3. **Re-evaluating a single snippet.** Library API today only exposes `evaluate()` (whole project). Should there be `evaluate_one(snippet_id)` for hosts that want fine-grained re-eval? Recommendation: **defer to v1**. The watch loop already does fine-grained re-eval internally; expose the API when a real consumer asks.
4. **Naming.** `Project` vs `Document` vs `Workspace` vs `Run`. `Project` reads best because (a) it scales to multi-entry-file v1, (b) matches Typst's own `world`/`project` terminology in the compiler API. Worth confirming during T-L01.
5. **Should `library_use.rs` ship in `crates/evcxr-typst/examples/` or in the workspace `examples/` directory?** Recommendation: **crates/-local**. The workspace `examples/` holds Typst documents, not Rust programs. Rust examples are crate-scoped per Cargo convention.

## References

- `crates/evcxr-typst/CLAUDE.md` — current crate guidance (will need updating once `lib.rs` lands).
- `/Users/elea/Documents/GitHub/evcxr/evcxr/examples/example_eval.rs` — the precedent.
- `docs/DECISIONS.md` D-004 (allow-eval safety), D-010 (cache layout), D-017 (timeout), D-018 (multi-file model — single-entry in v0).
- `docs/design/cache.md`, `docs/design/watch-loop.md` — implementation details that stay private.
