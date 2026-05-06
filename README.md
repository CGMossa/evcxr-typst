# evcxr-typst

Integration glue between [evcxr](https://github.com/evcxr/evcxr) (a Rust evaluation context — REPL + Jupyter kernel) and [Typst](https://typst.app/).

**Goal:** Rust snippets in a Typst document compile and their output (text, images, HTML, structured data) is embedded in the rendered PDF. Edits feel "live": evcxr keeps a long-running session so per-snippet incremental compilation is fast, and Typst's own incremental rendering picks up sidecar artifacts as they change.

## Status

Phase 0 design is complete (architecture, decisions, snippet semantics, error model, cache, watch-loop, multi-file projects, schema versioning — see [`docs/design/`](docs/design/)). Phase 1 scaffolding has shipped: a Rust CLI skeleton (`crates/evcxr-typst/`), a Typst package skeleton (`packages/evcxr/`), and a hello-world example. The next actionable task is **T-I03** — wiring the end-to-end smoke test that connects the CLI to the package via sidecar files. See [`docs/BACKLOG.md`](docs/BACKLOG.md).

## Quick start

```sh
cargo build -p evcxr-typst
cargo run -p evcxr-typst -- --help    # prints "scaffolding only" and exits 2
```

## Repository layout

```
evcxr-typst/
├── docs/                    # plans, architecture, decisions, backlog
├── crates/
│   └── evcxr-typst/         # Rust CLI (Phase 1 scaffolding)
├── packages/
│   └── evcxr/               # Typst package (Phase 1 scaffolding)
└── examples/
    └── hello/               # end-to-end smoke document
```

## How this repo relates to evcxr

The [evcxr](https://github.com/evcxr/evcxr) source is treated as a **read-only reference workspace**. We depend on its `evcxr` crate via path-dep during development (per [D-006](docs/DECISIONS.md)) and will switch to crates.io for releases. We do **not** vendor or fork it. Patches that need to land in evcxr proper are sent upstream.

Future Claude Code sessions: see [`CLAUDE.md`](CLAUDE.md).

## Documents

- [`docs/PLAN.md`](docs/PLAN.md) — phased roadmap.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — design rationale, MIME mapping, watch loop, why-not-WASM.
- [`docs/BACKLOG.md`](docs/BACKLOG.md) — agent-ready task queue. Pick the top open task and run.
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — ADR-lite log of design decisions.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT License ([`LICENSE-MIT`](LICENSE-MIT))

at your option.
