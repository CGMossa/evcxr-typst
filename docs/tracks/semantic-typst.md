# Track: Semantic Typst

> Surface rust-analyzer's understanding of snippets — types, signatures, docs, diagnostics — into the `.typ` document, so technical writers can do literate programming with semantic awareness.

**Off main critical path.** Depends on the main Phase 1–3 CLI shipping first. Adds optional features; never blocks the main journey.

## Vision

A Typst writer puts a Rust snippet in their document. Then, in prose:

> Earlier I defined #evcxr.ref("xs"), a vector of #evcxr.type-of("xs"). The function #evcxr.signature-of("normalize") rescales it to unit length.

…and the rendered document shows the actual type (`Vec<f64>`), the actual signature (`fn normalize(xs: &mut [f64]) -> ()`), with the references hyperlinked back to their defining snippets. No copy-paste, no manual maintenance, no drift between code and prose. Edit the Rust, the prose updates with it.

This is the pitch: documents that are *automatically* semantically self-aware, beyond what `rust-out` or `rust-display` can do by capturing stdout/images. The information was already in evcxr's own embedded rust-analyzer (`/Users/elea/Documents/GitHub/evcxr/evcxr/src/rust_analyzer.rs`); we just expose it.

## Target UX — feature catalogue

Each row is a candidate Typst-package function. The third column says where the data comes from in the simplest implementation (CLI sidecar) and which phase ships it.

| Function | Renders | Source | Track phase |
|---|---|---|---|
| `evcxr.type-of(name)` | the Rust type of a binding, as inline `raw` | CLI sidecar `<id>.semantic.cbor` | S1 |
| `evcxr.signature-of(name)` | full `fn` signature, formatted | CLI sidecar | S1 |
| `evcxr.kind-of(name)` | `"struct"` / `"enum"` / `"fn"` / `"const"` / … | CLI sidecar | S1 |
| `evcxr.doc-of(name)` | the rustdoc comment, as Typst content (markdown→typst best-effort) | CLI sidecar | S2 |
| `evcxr.items-table(at: id, only-kinds: ...)` | a table of all items in scope at the named snippet | CLI sidecar | S2 |
| `evcxr.ref(name)` | a styled inline reference that hyperlinks to the snippet defining `name` | CLI sidecar (locations from the same query pass) | S2 |
| `evcxr.diagnostics-of(snippet-id)` | rust-analyzer-emitted diagnostics for a snippet, rendered as a styled block | CLI sidecar (from `evcxr-typst run`) | S3 |
| All of the above, working under bare `typst compile` (no CLI run) | same renderings | WASM plugin (`evcxr-typst-analyzer`) | S4 |

The package always degrades gracefully (D-004): missing semantic data → placeholder, never an error.

## Worked example

```typ
#import "@preview/evcxr:0.X.Y" as evcxr

#evcxr.setup()

#evcxr.rust-hidden(```rust
struct Sample { freq: f64, amp: f64 }
let samples: Vec<Sample> = (0..1000)
    .map(|i| Sample { freq: i as f64 * 0.01, amp: (i as f64).sin() })
    .collect();
```)

The `Sample` type — formally #evcxr.signature-of("Sample") — captures one
oscillator sample. We collect a #evcxr.type-of("samples") with 1000 entries.

#evcxr.rust(```rust
let max_amp = samples.iter().map(|s| s.amp).fold(f64::NEG_INFINITY, f64::max);
println!("{max_amp:.3}");
```)

Above, `max_amp` (a #evcxr.type-of("max_amp")) summarises the peak. The
fold uses `f64::max`, see signature: #evcxr.signature-of("f64::max").

#evcxr.items-table(only-kinds: ("struct", "fn"))
```

When `evcxr-typst run --allow-eval` has produced sidecars: every reference resolves to real values from the live evcxr session. When it hasn't: each `type-of`/`signature-of`/etc. renders a small placeholder (e.g. ⟨type of `xs`⟩) and the document still compiles.

## Architecture

Three options. Decision below.

### Option A — CLI sidecars only

The CLI already drives evcxr's `CommandContext`, which already wraps a `RustAnalyzer` (`evcxr/src/rust_analyzer.rs` — used today for type inference and tab completion). Extend the run loop: after evaluating each snippet, query the analyzer for the declared items and the bindings' types, serialise to CBOR, write `<id>.semantic.cbor` next to the existing sidecars.

The Typst package functions read from those sidecars exactly like `rust-data` does today (D-015 model: `none`/fallback when missing).

**Pros:** small. Slots into Phase 4 cleanly. Reuses an integration evcxr already has — no new dependency, no new build artifact, no new schema family to maintain. Works for every feature in the catalogue except *bare-`typst compile` fallback*.

**Cons:** does not work without `evcxr-typst run`. Authors editing a doc and previewing with bare `typst compile` see placeholders for type-of / ref / etc. — the "semantic awareness" only kicks in after evaluation.

### Option B — WASM plugin only

A separate `crates/evcxr-typst-analyzer/` cdylib using the same `ra_ap_*` crates evcxr pins (`0.0.307`, see `evcxr/Cargo.toml`), patched per the `cgmossa/rust-analyzer` `wasm` branch. Shipped as `analyzer.wasm` inside the Typst package. Functions call into the plugin during `typst compile`.

This is the architecture analysed in `docs/design/wasm-plugin-analyzer.md`. Its hard parts (multi-MB blob, fork maintenance, stdlib summary, items-summary schema) all apply. Net effect: every catalogue feature works at `typst compile` time without ever running the CLI.

**Pros:** fallback UX is dramatically better. Bare `typst compile` of a document produces a fully-annotated rendering for snippets that don't need third-party deps.

**Cons:** weeks of separate engineering. Doesn't reduce CLI scope. Diagnostic fidelity gap with rustc still applies. `:dep`-using snippets still go opaque past the syntax level.

### Option C — both, package prefers plugin and falls back to sidecar

Package functions try the plugin first; if the plugin reports "I don't have what you need" (e.g. snippet uses external deps, or items-summary not provided), fall back to the CLI sidecar; if the sidecar is missing, fall back to placeholder. Three-tier degradation.

**Pros:** strict superset of A. Best UX in every scenario.

**Cons:** strict superset of A's *cost* + strict superset of B's. Highest engineering bill. The package has to negotiate between two data sources, including version skew between them.

### Decision (proposed)

Start with **A** (CLI sidecars). Ship the catalogue's first six rows that way. Re-evaluate **B** (and via that, C) once A is shipped and we have measurements of how often authors hit the "bare `typst compile`, want semantic" case. Documented as **D-020** below.

## Phased plan (this track only)

| Track phase | Scope | Depends on main-plan phase |
|---|---|---|
| **S1** | CLI emits `<id>.semantic.cbor`. Package implements `type-of`, `signature-of`, `kind-of`. Reads from sidecar; placeholder fallback. | Main Phase 3 (`evcxr-typst run` end-to-end + sidecar plumbing) shipped. |
| **S2** | Add `doc-of`, `items-table`, `ref`. Same plumbing; richer schema. | S1. |
| **S3** | Add `diagnostics-of`. The CLI runs `RustAnalyzer::diagnostics` per snippet alongside `execute`; failures don't abort the run. | S1. |
| **S4** *(optional, bigger)* | WASM plugin (`evcxr-typst-analyzer`). Architecture and risks fully captured in `docs/design/wasm-plugin-analyzer.md`. | Main Phase 4 (safety + pretty errors) shipped, plus S3. |

Track work is interleaved with main work, never blocking. If main and S1 both have an open task, main wins.

## Sidecar schema sketch (S1)

```jsonc
// .evcxr-typst-cache/v1/views/<entry>/<id>.semantic.cbor — decoded
{
  "v": 1,
  "snippet_id": "a1b2c3d4e5f6",
  // Bindings introduced by this snippet *and* visible in committed_state
  // after evaluation. Keys are the binding name as the user wrote it.
  "bindings": {
    "xs":  { "ty": "Vec<u32>",        "kind": "let",   "is_mut": false },
    "max": { "ty": "u32",             "kind": "let",   "is_mut": false }
  },
  // Items declared by this snippet. Keys are the item name; same shape
  // for struct/enum/trait/fn/const/static/type alias/inline mod.
  "items": {
    "Sample":     { "kind": "struct", "signature": "struct Sample { freq: f64, amp: f64 }",
                    "doc": null, "span": { "start": 0, "end": 53 } },
    "normalize":  { "kind": "fn",     "signature": "fn normalize(xs: &mut [f64])",
                    "doc": "Rescale to unit length.", "span": {...} }
  },
  // Resolved type queries the package asked for via setup() metadata
  // (the package can't ask the CLI on-demand; it declares a wishlist
  // upfront and the CLI fills in what it can per snippet). Optional; if
  // the package doesn't pre-declare, we just dump the full bindings/items.
  "queries": {
    "type-of:max_amp": "f64",
    "signature-of:f64::max": "fn max(self, other: f64) -> f64"
  }
}
```

Schema is governed by D-019 (major-breaking-only bumps; this counts as a fifth `v` field in the project — note it in `docs/design/schema-versioning.md` § "tracked interfaces" when S1 ships).

## What this track explicitly does NOT do

- **Editor / LSP integration.** Tab-completion in IDEs is out of scope. evcxr already has it (`CommandContext::completions`); routing it through Typst's tooling is its own project and not pursued here.
- **Auto-rename / refactoring.** Out of scope.
- **Formatter / linter feedback.** Out of scope.
- **Interactive querying.** No "ask the analyzer mid-render" loop; everything is pre-computed by the CLI per run.
- **`:dep` crate analysis at WASM time.** When S4 lands, snippets using third-party crates still surface as opaque past the syntax level. We document the limit, we don't try to dissolve it.

## Risks specific to this track

1. **Reusing evcxr's `RustAnalyzer` from outside `EvalContext`.** Evcxr uses it for its own purposes; exposing it as a "give me items + types after each snippet" API may need an upstream patch. Cost depends on how internal the coupling is. Read `evcxr/src/rust_analyzer.rs` and `evcxr/src/eval_context.rs` (search for `analyzer.`) before estimating S1.
2. **Pretty-printing types.** Rust-analyzer renders types in its own canonical form; we may want to post-process (e.g. drop fully-qualified module paths for terseness, like evcxr's REPL does today). Defer to S2 — ship the canonical form first.
3. **Rustdoc → Typst content conversion.** `doc-of` returns Markdown-ish content from Rust doc comments. Best-effort conversion to Typst: punt on rustdoc-extension links (`[Foo]`-style intra-doc links) in S2, surface them as `raw` in v0.
4. **Items-summary cache invalidation.** The semantic sidecar depends on prior snippets the same way snippet output does (D-010 cache key). Ensure the cache key for `<id>.semantic.cbor` matches the one for `<id>.txt` etc. — prevents skew where prose says one type and the rendered output reflects another.

## Open questions

1. Should `evcxr.type-of("xs")` accept a snippet-id parameter to disambiguate when the same name is rebound in a later snippet? (Likely yes; default = "the most recent definition before this point in document order.")
2. Cross-snippet `ref` rendering — Typst hyperlinks need targets. Snippets currently don't emit anchors. Do we add a `<label>` per snippet?
3. Type-of works on bindings; what about expressions? Some users will want `evcxr.type-of-expr("xs.iter().map(...)")`. Requires a fresh analyser pass per query, which crosses into "interactive querying" we said we wouldn't do. Defer; revisit only if S2 ships and there's demand.

## References

- `docs/design/wasm-plugin-analyzer.md` — companion analysis for S4 specifically (the WASM-plugin path).
- `/Users/elea/Documents/GitHub/evcxr/evcxr/src/rust_analyzer.rs` — evcxr's existing RustAnalyzer integration. The data we want is largely already collected here for evcxr's own purposes.
- `/Users/elea/Documents/GitHub/evcxr/evcxr/src/command_context.rs` — `CommandContext::completions` is the closest existing surface to what we want; not exactly the right shape but a useful precedent.
- `docs/DECISIONS.md` D-001 (prequery), D-004 (fallback), D-010 (cache layout), D-019 (schema versioning) — all unchanged by this track.
