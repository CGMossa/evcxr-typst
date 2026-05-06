# evcxr-typst

Integration glue between [evcxr](https://github.com/evcxr/evcxr) (a Rust evaluation context — REPL + Jupyter kernel) and [Typst](https://typst.app/).

**Goal:** Rust snippets in a Typst document compile and their output (text, images, HTML, structured data) is embedded in the rendered PDF. Edits feel "live": evcxr keeps a long-running session so per-snippet incremental compilation is fast, and Typst's own incremental rendering picks up sidecar artifacts as they change.

## Status

Pre-implementation. This repo currently contains plans only. See [`docs/BACKLOG.md`](docs/BACKLOG.md) for the next actionable task.

## Repository layout (planned)

```
evcxr-typst/
├── docs/                    # plans, architecture, decisions, backlog ← here today
├── crates/
│   └── evcxr-typst/         # Rust CLI (Phase 1)
├── packages/
│   └── evcxr/               # Typst package (Phase 1)
└── examples/                # end-to-end sample documents (Phase 1+)
```

## How this repo relates to evcxr

The [evcxr](https://github.com/evcxr/evcxr) source is treated as a **read-only reference workspace**. Locally it lives at `/Users/elea/Documents/GitHub/evcxr`. We depend on its `evcxr` crate via path / git / crates.io — we do **not** vendor or fork it. Patches that need to land in evcxr proper are sent upstream.

Future Claude Code sessions: see [`CLAUDE.md`](CLAUDE.md).

## Documents

- [`docs/PLAN.md`](docs/PLAN.md) — phased roadmap.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — design rationale, MIME mapping, watch loop, why-not-WASM.
- [`docs/BACKLOG.md`](docs/BACKLOG.md) — agent-ready task queue. Pick the top open task and run.
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — ADR-lite log of design decisions.

## License

Will be dual MIT / Apache-2.0, matching evcxr. License files not yet added — see backlog.
