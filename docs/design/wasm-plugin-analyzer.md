# Architectural option: a Typst WASM plugin built on rust-analyzer

Exploratory design. Does **not** supersede the prequery architecture (D-001) — it is a candidate companion or follow-on.

> **Now subsumed by the Semantic Typst side track.** This document covers the WASM-plugin path specifically (option B / track phase **S4** in [`../tracks/semantic-typst.md`](../tracks/semantic-typst.md)). The semantic-typst track sets up cheaper non-WASM precursors (S1–S3) that ship the same user-facing features via CLI sidecars; S4 is the bigger investment that brings them to bare `typst compile`. Read the track's overview first; this file remains the canonical analysis of the plugin path itself.

## The seed

[`cgmossa/rust-analyzer`](https://github.com/CGMossa/rust-analyzer) `wasm` branch (commit `8a79b99`, AI disclosure attached) carries patches that get the rust-analyzer crate set compiling to WebAssembly. Typst since 0.8 supports WASM plugins via a tightly-scoped byte-in/byte-out protocol (`/Users/elea/Documents/GitHub/evcxr/.typst-wasm-minimal-protocol/`). Question: can those two combine to do part of evcxr-typst's job inside the Typst sandbox?

## What rust-analyzer-in-WASM actually buys us

Rust-analyzer is **not a compiler**. It's a static analyzer over Rust source. In a Typst plugin it can do, on the snippet text it's given:

- Lexing, parsing, syntax-error reporting with spans.
- Macro_rules! expansion (locally defined macros).
- Name resolution within the synthesized crate (snippet items + a bundled stdlib summary).
- Type inference, type-mismatch diagnostics.
- Completion candidates (probably out of scope for our plugin).

It **cannot** do, even compiled to WASM:

- Run rustc → no codegen, no machine code, no execution.
- Resolve `:dep` (no cargo, no network, no fs), so any snippet using a third-party crate is opaque past the syntax level.
- Run procedural macros or build scripts (those require executing compiled code).
- Capture stdout / images / any runtime output. There **is** no runtime.
- Auto-tokio for `await` (D's "async-await" feature) — that needs an actual runtime.

In short: the plugin can tell you whether a snippet *would* compile under specific assumptions, not what it *does* when run. **It is not a replacement for the prequery CLI.** The CLI stays load-bearing for everything that needs execution — which is the whole point of evcxr-typst.

## A realistic role: in-document pre-flight analysis

Where this *does* slot in cleanly: the **fallback path** (D-004). Today, `typst compile main.typ` without an `evcxr-typst run` step renders a generic placeholder box: "run the CLI to evaluate". A WASM rust-analyzer plugin lets us do better, while keeping every existing decision intact:

- Snippet has a syntax / type error using only stdlib? → render a real diagnostic box with spans, just like an editor would. No CLI required.
- Snippet uses a `:dep` crate? → render a placeholder that says "external deps required, run `evcxr-typst run`" — degraded but honest.
- Snippet looks fine and uses no deps? → render the source with a "ready to run" badge. After `evcxr-typst run`, the actual output replaces it.

Net effect: documents become *self-aware* under bare `typst compile`. The CLI keeps being the only thing that actually executes Rust. D-004 (fallback by default; eval is opt-in) survives unchanged.

## Concrete shape

A second package artifact, sibling to `crates/evcxr-typst/` and `packages/evcxr/`:

```
crates/
  evcxr-typst/             existing CLI (prequery)
  evcxr-typst-analyzer/    new — cdylib, builds to WASM
packages/
  evcxr/
    lib.typ                gains an optional `analyzer.wasm` plugin import
    analyzer.wasm          shipped alongside lib.typ in the package
    fallback.typ           upgraded to call into the plugin when present
```

The `evcxr-typst-analyzer` crate:

- Depends on the same `ra_ap_*` crates evcxr already pulls (`/Users/elea/Documents/GitHub/evcxr/evcxr/Cargo.toml` lists `ra_ap_ide`, `ra_ap_ide_db`, `ra_ap_hir`, `ra_ap_syntax`, `ra_ap_span`, `ra_ap_paths`, `ra_ap_base_db`, `ra_ap_vfs` — all at `0.0.307`).
- Uses the `cgmossa/rust-analyzer` wasm fork's patches to produce WASM-compatible builds of those crates.
- Bundles a precomputed "stdlib summary" so name/type resolution works without a sysroot. (Existing prior art: rust-analyzer-wasm playgrounds do this.)
- Exposes one or two `#[wasm_func]` entry points: `analyze(snippet_cbor) -> diagnostics_cbor` and possibly `merge_and_analyze(prior_items_cbor, snippet_cbor) -> diagnostics_cbor` for cross-snippet awareness.
- Communicates with the package via CBOR (per the wasm-minimal-protocol README), using the same metadata-schema contract (D-019).

The `packages/evcxr/lib.typ` changes minimally:

```typ
#let _analyzer = plugin("./analyzer.wasm")          // optional; gracefully missing
#let _diag(snippet, items) = cbor(_analyzer.analyze(cbor.encode((snippet: snippet, items: items))))
```

A new `fallback.typ` path consumes `_diag(...)` to render a real diagnostic box when available; otherwise the existing placeholder.

## Mechanism: how we'd actually depend on the fork

This is the part most likely to bite. The fork (`cgmossa/rust-analyzer` branch `wasm`, currently at commit `8a79b99`) carries patches to make `ra_ap_*` compile to `wasm32-unknown-unknown`. None of those crates are themselves the fork — they're published on crates.io by upstream rust-analyzer, and that's what evcxr already consumes (pinned to `0.0.307`).

To swap them for the fork's patched versions, Cargo's `[patch.crates-io]` is the right tool:

```toml
# crates/evcxr-typst-analyzer/Cargo.toml
[patch.crates-io]
ra_ap_ide      = { git = "https://github.com/CGMossa/rust-analyzer", rev = "8a79b99..." }
ra_ap_ide_db   = { git = "https://github.com/CGMossa/rust-analyzer", rev = "8a79b99..." }
ra_ap_hir      = { git = "https://github.com/CGMossa/rust-analyzer", rev = "8a79b99..." }
ra_ap_syntax   = { git = "https://github.com/CGMossa/rust-analyzer", rev = "8a79b99..." }
# … one entry per ra_ap_* crate the analyzer pulls transitively
```

`rev =` (a pinned commit), **not** `branch = "wasm"` — the latter resolves to whatever HEAD is at `cargo update` time, which is non-reproducible and a CI surprise machine. Pin the commit, bump deliberately.

### The workspace patch-leakage problem

`[patch.crates-io]` placed in our root `Cargo.toml` would apply to the *entire* dependency graph of the workspace, including the path-dep on `evcxr` (which has its own published-`ra_ap_*` consumers for type-inference and tab-completion). If the fork's API differs from upstream `0.0.307` at all, applying the patch at the workspace root breaks evcxr's own functionality for the whole project — just to support a sibling WASM build that nothing on the main path needs.

Mitigation: **isolate `crates/evcxr-typst-analyzer/` into its own Cargo workspace.** Append a `[workspace]` block to its `Cargo.toml`, exactly like the wasm-minimal-protocol example does (`/Users/elea/Documents/GitHub/evcxr/.typst-wasm-minimal-protocol/examples/hello_rust/Cargo.toml`):

```toml
# crates/evcxr-typst-analyzer/Cargo.toml — bottom
[workspace]    # excludes this crate from the parent workspace; the patch stays local
```

This keeps the patch's blast radius to the analyzer crate itself. The main `evcxr-typst` workspace continues to consume published `ra_ap_*` only (transitively via evcxr), unaffected.

Cost: the analyzer crate doesn't share the parent's `Cargo.lock`, target dir, or `[profile.*]` settings. That's fine — the main `evcxr-typst` build doesn't depend on it (it's only loaded by Typst at compile time as an opaque `.wasm` artifact). The build pipeline becomes a two-step shell: (1) `cargo build --release --target wasm32-unknown-unknown` from inside `crates/evcxr-typst-analyzer/`; (2) wasm-opt + wasi-stub if needed; (3) copy the artifact to `packages/evcxr/analyzer.wasm` for the package release.

### `0.0.x` versioning fragility

`ra_ap_*` releases at `0.0.X`. Cargo treats `0.0.A` and `0.0.B` as **fully incompatible** (not minor-compatible the way `0.X.Y`/`0.X.Z` would be). Implications:

- The fork must track evcxr's pinned `0.0.307` exactly. If evcxr bumps to `0.0.308` and we bump our path-dep to match, the fork has to rebase to `0.0.308` or we end up with two semver-incompatible `ra_ap_*` versions in the same dependency graph.
- Rebase cadence is therefore tied to evcxr's `ra_ap_*` bump cadence, which is in turn tied to upstream rust-analyzer's release cadence (~weekly).
- This is a real, ongoing maintenance burden, not a one-time cost. T-S04 must include "fork-rebase pipeline" as a sub-item before it's shippable.

### Existing prior-art reference

The Typst `wasm-minimal-protocol/examples/hello_rust/Cargo.toml` shows the exact isolation pattern we'd use, plus a release profile tuned for WASM size:

```toml
[profile.release]
lto = true
strip = true
opt-level = 'z'
codegen-units = 1
panic = 'abort'

[workspace]    # so that it is not included in the upper workspace
```

We'd match that profile and the workspace-isolation comment verbatim. Verified by inspecting `/Users/elea/Documents/GitHub/evcxr/.typst-wasm-minimal-protocol/examples/hello_rust/Cargo.toml`.

## Hard parts and risks

1. **WASM rust-analyzer is heavy.** The compiled cdylib is multi-MB after wasm-opt. A Typst package shipping a several-MB plugin is workable (Universe accepts it; users download once) but it isn't trivial.
2. **Cross-snippet item state.** The plugin is stateless per call. To analyze snippet 7 with knowledge of structs/fns/`use`s defined in 1–6, the package must pass an items-summary on every call. We have to define that summary's schema and keep it in sync with snippet evolution. Cost: another versioned interface per D-019.
3. **Stdlib summary maintenance.** Bundle a precomputed metadata blob for `core`/`std`/`alloc`. Has to be regenerated on rustc bumps. Needs a build step / CI artifact pipeline.
4. **Fork maintenance.** `cgmossa/rust-analyzer` `wasm` is, at this writing, a personal fork. Either upstream lands the patches (long path) or this project carries the maintenance burden of rebasing the fork on rust-analyzer master periodically. The patches commit (`8a79b99`) and AI disclosure are noted; non-trivial rebases will recur as upstream changes.
5. **Diagnostic fidelity gap.** Rust-analyzer's diagnostics don't perfectly match rustc's. Some errors only surface on real compile (borrowck edge cases, MIR-only diagnostics, codegen errors). Users may see "passes the analyzer, fails the CLI", which is a confusing UX unless we frame the analyzer's check as "preliminary / deps not resolved".
6. **Async + `:dep` opaque.** Big chunks of evcxr's UX (auto-tokio, third-party crates) are invisible to the plugin. Documents that lean on those see no analyzer benefit; the plugin needs a clean "I can't analyze this without the CLI" code path.
7. **Doesn't reduce CLI scope at all.** Every implementation phase from the existing PLAN still has to ship. This is *added* surface, not redirected effort.

## Why this is **not** the architecture for v0

- D-001 stands. The plugin doesn't do execution, so the CLI's prequery loop is still required for anything observable in the document.
- The CLI path is fully designed (D-001..D-019) and the scaffolding ships in `fa90905`. Pivoting now would discard ~all design work for a feature that isn't a substitute.
- Roughly six weeks of additional engineering (build pipeline, fork maintenance, stdlib summary, schema, plugin API, package integration, testing) for a UX that — while good — only fires in a fallback case.

## Where it slots in

If we still want it, the natural placement is a new **Phase 5** (or 6) after the CLI path is shipped and shipping value:

| Phase | Existing scope | New |
|---|---|---|
| 1 | smoke test (T-I01..I03) | — |
| 2 | MIME passthrough (T-I04) | — |
| 3 | watch + cache (T-I05) | — |
| 4 | safety + pretty errors (T-I06, T-I07) | — |
| **5 (new)** | (was: editor story / snapshot/restore) | **WASM analyzer plugin** |

In Phase 5 the plugin lands as a self-contained addition: new crate, new package payload, new optional lib.typ branch, new schema entry. It does not touch any of Phase 1–4's contracts. No existing decision record is invalidated; we'd add a new one (`D-020 — WASM analyzer plugin shipped as an optional pre-flight analyzer`) when we commit to building it.

## Decision

**No change to current architecture.** Track this doc as the canonical record of the option. Open a new task `T-D11` (design follow-up) only if we actually pull this forward into the active plan; otherwise it sits here as documented research.

## Open questions if we ever pull this forward

Answered in part by the Mechanism section above; the remainder:

1. **Does the fork build clean against `0.0.307`?** Resolved-by-spike: T-S04-spike (one engineering day, parse-only `ra_ap_syntax` only). Until that runs, every other answer here is conditional.
2. **Stdlib summary**: how big, how often regenerated, who owns the regeneration script? Prior art in `rust-analyzer/crates/intern` and the rust-analyzer-wasm browser playground may give us a starting point.
3. **Cross-snippet items summary schema.** Becomes a fifth `v` field per D-019 if we ship.
4. **Subset compilation.** Is there a meaningful subset of rust-analyzer we could compile (just `ra_ap_syntax` for parse-only diagnostics, dropping `ra_ap_hir`/`ra_ap_ide_db`)? Smaller blob, less coverage. The spike (T-S04-spike) starts with exactly this subset, so we get a measurement of the parse-only artifact size for free.
5. **Memory ceiling.** Does the wasm-minimal-protocol's WASM memory budget (Typst's per-call limits?) admit a multi-MB analyzer cleanly? The spike's hello-world tells us at least the lower bound.

## Reference

- Wasm protocol spec: `/Users/elea/Documents/GitHub/evcxr/.typst-wasm-minimal-protocol/README.md`, examples at `.typst-wasm-minimal-protocol/examples/hello_rust/`.
- Fork seed: `cgmossa/rust-analyzer` branch `wasm`, commit `8a79b99`.
- evcxr's existing rust-analyzer integration: `/Users/elea/Documents/GitHub/evcxr/evcxr/src/rust_analyzer.rs` (already uses `ra_ap_*` for type inference and tab completion in evcxr's own context).
- D-001 (why prequery) — unchanged by this analysis.
- D-004 (fallback by default) — strengthened by this option if implemented.
