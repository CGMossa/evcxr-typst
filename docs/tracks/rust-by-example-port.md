# Track: Rust by Example, ported to Typst

> Port the [rust-by-example](https://github.com/rust-lang/rust-by-example) book (~198 chapters, dual MIT/Apache-2.0) to Typst documents in `examples/rust-by-example/`, evaluated end-to-end through `evcxr-typst`. Becomes the flagship demonstration that the integration handles real, varied Rust at scale.

**Off main critical path.** Depends on main Phase 1–3 shipping (T-I03 minimum, T-I04 for the chapters that emit images). Adds a substantial example set; never blocks the main journey.

## Why this track

The eight-doc gallery in `docs/design/examples/` proves the integration *can* render the feature catalogue. Rust-by-example proves it *does* render real, idiomatic, third-party Rust prose-and-code content faithfully. It exercises:

- Every persistent construct (`struct`, `enum`, `trait`, `impl`, `mod`, `use`, `fn`, `let`, `macro_rules!`) — see `docs/design/snippet-semantics.md`.
- Cross-snippet composition at chapter scope: a chapter introduces `Point` in §1, uses it in §3.
- Every MIME path: text-only chapters via `println!`; the `std` chapters that pull `regex`, `serde`, etc. via `:dep`; the formatting chapter's debug/display printing; the error chapter's deliberate compile failures.
- `:dep` ergonomics under D-013 (inline-anywhere with version conflicts surfaced).
- `async`/`await` in the std_misc and concurrency chapters (auto-tokio per evcxr's `COMMON.md`).
- Snippet timeout (D-009 / D-017) — some examples deliberately loop.
- The fallback path (D-004) — every chapter must compile under bare `typst compile` *and* render evaluated output after `evcxr-typst run --allow-eval`.

If we can render rust-by-example, we can render most things.

## Scope

The book has 198 markdown files (`.rust-by-example/src/**/*.md` mirroring `SUMMARY.md`). Full port is one Typst doc per chapter, organised under `examples/rust-by-example/<chapter-path>.typ`, with a top-level `main.typ` importing them all and reproducing the SUMMARY structure as a Typst outline.

Multi-file project model per D-018 applies: one entry file (`main.typ`), all per-chapter files reached via `#include`, single CAS shared across the workspace, single id-addressed view per render.

### Phasing

| Phase | Chapters | Lines roughly | Done when |
|---|---|---|---|
| **B0** | Tooling + license/attribution scaffolding (no chapters yet) | n/a | A `tools/rbe-port/` Rust binary converts one chapter end-to-end and is tested. License/attribution files in place. |
| **B1** | First three SUMMARY sections: `hello.md`, `primitives/*`, `custom_types/*` (~15 files) | ~600 | All 15 chapters render under `evcxr-typst run --allow-eval` with output matching rust-by-example's expected output. Hello-Primitives-Custom Types path readable as a Typst PDF. |
| **B2** | `variable_bindings/*`, `types/*`, `conversion/*`, `expression/*`, `flow_control/*` (~30 files) | ~1500 | Same render bar; `examples/rust-by-example/main.typ` builds top to bottom. |
| **B3** | `fn/*`, `mod/*`, `crates/*`, `cargo/*` (~15 files) | ~700 | Cross-snippet composition exercised heavily; the modules chapter is the acid test. |
| **B4** | `attribute/*`, `generics/*`, `scope/*`, `trait/*` (~25 files) | ~1200 | Trait composition; lifetime restrictions per snippet-semantics § "Variable-reference limitation" surface and are documented chapter-by-chapter. |
| **B5** | `error/*` (~15 files) | ~500 | The error chapter's deliberate compile failures render via T-I07's pretty error path. Documents how rust-by-example uses errors pedagogically. |
| **B6** | `std/*`, `std_misc/*`, `testing/*`, `unsafe/*`, `compatibility/*`, `meta/*` (~40 files) | ~2000 | The `:dep`-heavy chapters. Tests T-I04 MIME passthrough (the formatting chapters' Display output) and T-I05 watch (the std collections chapter's interactive feel). |

v0 ships **B0 + B1 + B2** as the side-track's first deliverable. Later phases land as bandwidth allows.

## Mapping: how a chapter is transformed

Each `.md` file becomes a `.typ` file. Translation rules:

### Markdown → Typst

| Markdown | Typst | Notes |
|---|---|---|
| `# Heading` | `= Heading` | Standard `=`/`==`/`===` for h1/h2/h3. |
| `*emphasis*` | `_emphasis_` | Same for double for bold. |
| `` `inline code` `` | `` `inline code` `` | Identical syntax. |
| `[text](url.md)` | `#link("url.typ", [text])` for cross-chapter, raw URL otherwise | The porter rewrites `.md`→`.typ` extensions for in-tree links. |
| Lists, tables, blockquotes | Idiomatic Typst equivalents | Standard. |
| Footnotes / `[ref]: url` reference-style links | Resolved inline | mdBook's link-bottoming pattern; flatten at port time. |

### Code blocks → evcxr functions

| mdBook tag | Becomes | Why |
|---|---|---|
| ` ```rust ` (no comma) | `#evcxr.rust(...)` with `render: "both"` | The standard executable example. |
| ` ```rust,editable ` | `#evcxr.rust(...)` | The `editable` flag is a mdBook Run-button hint; not meaningful for our render. |
| ` ```rust,ignore ` | `#evcxr.rust(..., render: "source")` | rust-by-example marks snippets it doesn't want auto-run. We mirror by suppressing evaluation: emit metadata but the porter sets a `skip-eval` flag the CLI honors. **Open question** for the porter: is `skip-eval` worth a kwarg, or do we use `rust-hidden` repurposed? Tracked in T-B01 below. |
| ` ```rust,no_run ` | Same as `ignore` | Same handling. |
| ` ```rust,compile_fail ` | `#evcxr.rust(...)` rendered with the *expected* compile error displayed via T-I07 | These are pedagogical — error must surface, not abort the document. Requires T-I07 (T-S03 territory). Defer to B5. |
| ` ```text ` | `#raw(block: true, ...)` with `lang: none` | mdBook uses these for *expected* output. We render verbatim; users see the example's intended output even if our `evcxr-typst run` produces the actual output below. |
| ` ```bash` / ` ```sh ` | `#raw(block: true, lang: "bash", ...)` | Documentation-only; never executed. |

### The `fn main()` problem

rust-by-example wraps almost every snippet in `fn main() { … }`. evcxr would happily *define* `main` but not invoke it; the body's `println!` would never run.

**Decision (proposed, tracked in `D-022`):** introduce a small package convenience `evcxr.rust-main(...)` that takes a snippet which contains a `fn main() { … }` definition and emits two CLI instructions:

1. Define everything in the snippet (including `main`).
2. Append a synthetic `main();` invocation — recorded in metadata as `options.auto-call = "main"`, **not** shown in the rendered source.

The rendered Typst source is the unmodified rust-by-example snippet (faithful to the upstream). The captured stdout below is the result of `main()` being called.

```typ
// Original rust-by-example hello.md becomes:
#evcxr.rust-main(```rust
fn main() {
    println!("Hello World!");
}
```)
```

`rust-main` is otherwise identical to `rust` in API surface (same kwargs, same metadata schema modulo the new `options.auto-call` field). Adding it does not require schema-version-bumping per D-019 (additive option). The decision record (`D-022`) and a one-page spec live in `docs/design/package-api.md` § "rust-main" once we ship.

For snippets that don't have `fn main()` (rare in rust-by-example but they exist — usually demonstrating a single expression), the porter falls back to plain `evcxr.rust(...)`.

### What gets dropped at port time

- "Click 'Run' above to see the expected output" prose — drop entirely.
- "Edit this code, then…" / "Ctrl+Enter" instructions — drop.
- Activity / Exercise prompts at chapter ends — keep, but render as a styled callout (the porter has a heuristic: "### Activity" or "### Exercise" headings).
- Cross-chapter mdBook permalinks (`[macros]: macros.md` style) — the porter resolves to relative `.typ` paths.

## Tooling: `tools/rbe-port/`

A Rust binary in the workspace that drives the conversion. Skeleton:

```
tools/rbe-port/
├── Cargo.toml                         workspace member
├── src/
│   ├── main.rs                        clap CLI: rbe-port <input-dir> <output-dir>
│   ├── md.rs                          markdown parsing (use `pulldown-cmark` 0.x)
│   ├── typst.rs                       Typst-content emission
│   ├── snippet.rs                     `fn main()` detection (use `syn`)
│   ├── summary.rs                     SUMMARY.md → outline tree
│   └── lib.rs
└── tests/
    └── golden/                        small md inputs + expected typ outputs
```

Inputs:
- A path to a rust-by-example checkout (the source mdBook).
- An output directory (typically `examples/rust-by-example/`).
- A `--phase B1|B2|…` flag selecting which SUMMARY subtree to port.

Output:
- One `.typ` per input `.md`, mirroring the directory structure.
- A top-level `main.typ` that `#include`s the per-chapter files in SUMMARY order.
- A `manifest.json` capturing input commit SHA + per-file SHA-256 — used for drift detection.

The tool is deterministic: `rbe-port -i .rust-by-example/ -o examples/rust-by-example/ --phase B1` produces byte-identical output across runs given the same input.

### Drift detection

When upstream rust-by-example updates, run `rbe-port --check`. It re-converts and diffs against on-disk `.typ`. Mismatches mean either the upstream changed (re-port) or we hand-edited a chapter (manual reconciliation needed). The `manifest.json` tells us which.

## License and attribution

rust-by-example is dual MIT/Apache-2.0. So is evcxr-typst. The licenses are compatible.

**Required (D-022 will codify):**
- `examples/rust-by-example/NOTICES.md` documenting upstream license, repo, and the commit SHA the port is based on. Includes the canonical "Portions of this work are derived from rust-by-example, copyright (c) 2014–present The Rust-by-Example Authors, dual-licensed MIT and Apache-2.0" attribution line.
- The top-level `examples/rust-by-example/main.typ` includes a generated banner pointing at `NOTICES.md`.
- Each per-chapter `.typ` carries a `// Adapted from rust-by-example/<src-path>.md (see ../NOTICES.md)` header. Auto-generated by the porter.

Not required but considered:
- `git submodule` reference to upstream — rejected; introduces transitive complexity. Plain manifest tracking is enough.
- Vendoring `.rust-by-example/` into our repo — rejected; bloats the repo and makes upstream updates a merge mess. The porter reads from a configurable path; the `.rust-by-example/` directory stays gitignored.

## Risks

1. **Snippet count.** ~198 chapters. Each one a snippet (or several). Rendering the full book end-to-end via `evcxr-typst run` will take meaningful wallclock time even with `:cache` warm. Watch mode helps but the cold first run is non-trivial. Measure on B1; budget accordingly for B6.
2. **`fn main()` detection edge cases.** Some rust-by-example snippets define `main` plus helpers; a few define helpers with no `main`; the closures chapter uses lambdas at top-level. The `rust-main` heuristic must handle each cleanly. Golden tests in `tools/rbe-port/tests/golden/` cover the matrix.
3. **`compile_fail` snippets.** The error chapter (B5) is built around demonstrating compile errors. Our T-I07 needs to render those gracefully — and the doc must STILL render the rest of itself (don't bail on first error). Verified by T-S03 timing.
4. **`:dep`-heavy chapters in B6.** Chapters that pull `serde`, `rand`, `regex` add cargo-build time per first-render. Not a correctness risk but an UX risk for the demo experience. The `:cache` setting (D-010 cache layout) absorbs this for subsequent renders.
5. **Cross-chapter linkage.** mdBook permalinks like `[macros]: macros.md` resolve at port time, but Typst hyperlinks need labels in the target file. Port logic must emit a `<label>` anchor at each chapter's heading and link with `#link(<label>, ...)`.
6. **Translations.** rust-by-example has `po/` (Portuguese, Japanese, Chinese, etc.) translations. Out of scope for this track. English only.

## Open questions

1. **Phase B1 size sanity check.** ~15 chapters → ~600 lines of Typst by my eye-estimate. Validate on the first three before committing to B2 sizing.
2. **`rust,ignore` / `rust,no_run` UX.** Current proposal: porter sets `options.skip-eval = true`. The CLI honors it (no eval, just emits source). Alternative: render via `evcxr.rust-hidden(...)` so they appear unevaluated naturally. The first is more faithful to rust-by-example's UX (rendered code, no output); the second is simpler. T-B01 decides.
3. **Should we ship a `rust-main` package function for everyone, or keep it porter-specific?** I lean toward shipping it publicly — it's a useful pattern beyond rust-by-example. But tracked as a question for the package-api owner.
4. **Pulldown-cmark version.** Latest 0.x has different API surface than 0.9. Pick at T-B00 implementation time; any working version is fine.
5. **Golden-test format.** Pure-text diff (`expected.typ` files) is simplest. Snapshot testing with `insta` is nicer for seeing changes. Pick at T-B00.

## References

- `.rust-by-example/` (local checkout, gitignored): the source material.
- `docs/design/multi-file.md`: multi-file project model (entry file = `examples/rust-by-example/main.typ`).
- `docs/design/snippet-semantics.md`: every chapter exercises this.
- `docs/design/package-api.md`: the API surface a `rust-main` addition would extend.
- `docs/DECISIONS.md` D-004 (fallback by default), D-010 (cache layout), D-018 (multi-file), D-019 (schema versioning).
- Upstream: <https://github.com/rust-lang/rust-by-example>, dual MIT/Apache-2.0.
