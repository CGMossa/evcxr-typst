# Phased plan

The roadmap. Each phase is a coherent slice that's worth landing on its own; phases roughly correspond to issue milestones. The actionable atoms live in `BACKLOG.md`; this file is the why-and-when narrative.

## Phase 0 — Planning (in progress)

This phase. Output: this docs/ tree. Enough to onboard a fresh agent and have them produce useful code on the first task.

**Done when:** README, ARCHITECTURE, PLAN, BACKLOG, DECISIONS exist and are internally consistent. Backlog has at least the Phase 1 tasks fully briefed.

## Phase 1 — End-to-end smoke test (no incremental, no caching)

The thinnest vertical slice that proves the architecture works.

**Scope:**
- A Typst package `packages/evcxr/` exposing one function: `rust(code)` that emits a `<evcxr-snippet>` metadata marker and `read`s a sidecar text file with the captured stdout.
- A Rust CLI `crates/evcxr-typst/` with one subcommand: `evcxr-typst run <main.typ>`. It:
  1. shells out to `typst query --field value <main.typ> '<evcxr-snippet>'`, getting JSON of snippets in document order;
  2. drives one `evcxr::CommandContext` through the snippets sequentially, capturing stdout via the existing `EvalContextOutputs` channels;
  3. writes one sidecar text file per snippet (keyed by stable id);
  4. shells out to `typst compile` twice — once for PDF (`<stem>.pdf`, the user-facing artifact) and once for SVG (`<stem>.svg`, for visual quick-look in a browser without a PDF viewer). Note that Typst's SVG renders glyphs as `<path>` references, not `<text>` elements, so the SVG is *not* a text record of what evaluated; for that, the agent / dev loop reads `.evcxr-typst-cache/<id>.txt`. Multi-page documents need `typst compile` invoked directly with a `{p}` template.
- An `examples/hello/` document with three snippets, where snippet 2 uses a `let` binding from snippet 1, demonstrating that evcxr's persistent state works.

**Out of scope here:** image/HTML MIME output, watch mode, cache, fallback rendering, error pretty-printing, multiple documents, anything Universe-related.

**Done when:** `cargo run -p evcxr-typst -- run examples/hello/main.typ` produces both a PDF and per-page SVG that contain the printed output of all three snippets, and snippet 2's output references state defined in snippet 1.

## Phase 2 — MIME passthrough + structured outputs

Plumb the rest of evcxr's `EVCXR_BEGIN_CONTENT <mime> ... EVCXR_END_CONTENT` protocol through to Typst.

**Scope:**
- Recognize `image/png`, `image/svg+xml`, `image/jpeg`, `text/html`, `text/plain`, `application/json` (and `application/cbor`).
- Write image MIME outputs as binary files (`<id>.png`, etc.) — base64 decode is already done by evcxr's protocol implementation; verify and adjust.
- Typst package gains: `rust-display(code)` (image-only), `rust-html(code)` (raw HTML), `rust-data(code)` returning Typst dictionaries via `cbor()` / `json()`.
- Add `:dep` support: a `dep(name, version)` Typst function that emits a `<evcxr-dep>` metadata marker; CLI flushes deps before each session.
- Use `runtimes/evcxr_image` from the evcxr workspace as the test producer for image MIME output.

**Done when:** an example doc renders an evcxr-generated PNG plot inline, and a small `cbor`-roundtripped dictionary is consumed by Typst as a real dictionary.

## Phase 3 — Caching + watch mode

The "interactive / progressive" part.

**Scope:**
- Per-snippet content-hash cache. Key = `blake3(src) ⊕ blake3(active :dep state) ⊕ snippet index`. Skip eval on cache hit, just re-emit the existing sidecar(s).
- Layer on top of evcxr's own `:cache` (rustc-output cache) — set it on by default with a sane size limit.
- `evcxr-typst watch <main.typ>`: long-lived `CommandContext`, `notify`-watch the source, run `typst watch` as a child process. On source change:
  1. re-query snippets; classify changes; produce new sidecars for affected snippets;
  2. let `typst watch` notice the sidecar mtime changes and rebuild incrementally.
- Honest behavior: editing a snippet in the middle of the document re-evaluates from scratch starting at that snippet (committed_state is forward-only). Document this clearly in the README. Compilation cache makes this much cheaper than it sounds.

**Done when:** editing a snippet in `examples/hello/main.typ` while `evcxr-typst watch` is running causes the PDF to update within a second or two without rebuilding the world from scratch.

## Phase 4 — Safety, polish, distribution

**Scope:**
- Fallback rendering: when `--input evcxr-fallback=true` is passed (or the sidecar is missing), the Typst package renders a placeholder box instead of erroring. This is what makes a document safe to compile with bare `typst compile` even though it embeds executable Rust.
- `--allow-eval` flag on the CLI is required to run snippets; otherwise `evcxr-typst run` refuses with a clear error pointing at this safety guarantee.
- Pretty error reporting: forward evcxr compilation errors (with source spans) into a sidecar that the Typst package surfaces as a styled error box with the offending snippet highlighted.
- Publish: Typst package goes to Universe; CLI goes to crates.io. Decide on versioning policy and minimum compatible CLI version (the package should error helpfully if the CLI is too old).

**Done when:** a user can `typst compile` a document by another author safely (placeholder boxes), then opt into evaluation with `evcxr-typst run --allow-eval`, and get nice errors when their snippet doesn't compile.

## Phase 5 (optional / later) — Editor story

- Pipe evcxr's tab completions through to a Typst LSP extension. Probably pricy for the value; revisit only if Phase 4 ships and people use it.
- Snapshot/restore in `EvalContext` so editing a middle snippet doesn't require re-eval-from-zero. Requires upstream evcxr changes; only worth it if Phase 3 measurements show this is the bottleneck.

## Side tracks (off main path)

Some valuable directions are designed but explicitly **not on the critical path**. They're documented under `docs/tracks/` with their own phased plans, and never block the main Phase 1–4 journey. Currently:

- **Semantic Typst** ([`tracks/semantic-typst.md`](tracks/semantic-typst.md)) — surface rust-analyzer's view of snippets (types, signatures, docs, diagnostics, refs) into the document, enabling literate programming with semantic awareness. Ships in four sub-phases S1–S4 (T-S01..T-S04 in `BACKLOG.md`); the first three are CLI-sidecar slices that piggy-back on the main-plan plumbing, S4 is the bigger WASM-plugin investment and is decision-gated. See D-020.
- **Rust-by-example port** ([`tracks/rust-by-example-port.md`](tracks/rust-by-example-port.md)) — port the upstream rust-by-example book (~198 chapters) to Typst documents under `examples/rust-by-example/`, evaluated through evcxr-typst. The flagship "real Rust at scale" demonstration. Ships in seven sub-phases B0–B6 (T-B00..T-B06 in `BACKLOG.md`); B0 is tooling, B1–B2 the v0 deliverable, B3–B6 expand later. See D-022.

If a main-plan task and a side-track task are both open, the main task wins.

## Non-goals (call them out so we don't drift)

- Not a replacement for Jupyter — if you want a notebook, use evcxr's Jupyter kernel directly.
- Not a sandbox. We don't try to make running Rust from a Typst doc safe by sandboxing the Rust; we make it *opt-in*.
- Not a Typst plugin (WASM) for evcxr itself. See `DECISIONS.md` D-001.
