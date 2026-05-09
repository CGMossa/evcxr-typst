# evcxr-typst

Integration glue between [evcxr](https://github.com/evcxr/evcxr) (a Rust evaluation context — REPL + Jupyter kernel) and [Typst](https://typst.app/). Rust snippets in a `.typ` document are evaluated and their output (text, images, structured data) is embedded in the rendered PDF. A watch mode keeps a live evcxr session open so edits re-evaluate in seconds.

## Status

Not yet published. The Universe package and crates.io publish are the remaining open work in Phase 4 (T-I08 in progress — see [`docs/PLAN.md`](docs/PLAN.md)). The library and CLI are functional for local use.

## Quick start

```sh
# Install the CLI (not yet on crates.io — build from source for now)
cargo install --path crates/evcxr-typst

# In your document (not yet on Universe — use a local import for now):
#import "packages/evcxr/lib.typ" as evcxr
#evcxr.setup()
#evcxr.rust(```rust println!("hello"); ```)

# Run once (evaluates snippets, then compiles PDF + SVG):
evcxr-typst run --allow-eval --root . main.typ

# Bare compile (safe — renders placeholder boxes without evaluating Rust):
typst compile --root . main.typ
```

## How it works

1. `typst query` extracts `<evcxr-snippet>` metadata markers from the document in order.
2. A long-lived `evcxr::CommandContext` drives snippets sequentially, capturing stdout and MIME display objects; output is written to sidecars under `.evcxr-typst-cache/`.
3. `typst compile` reads the sidecars at render time and embeds output; the Typst package renders placeholder boxes for any snippet not yet evaluated.

## Subcommands

| Subcommand | What it does |
|---|---|
| `evcxr-typst run [--allow-eval] [--root <dir>] <file>` | One-shot: discover, evaluate (if `--allow-eval`), compile PDF + SVG. |
| `evcxr-typst watch [--allow-eval] [--root <dir>] <file>` | Watch mode: keep one evcxr session alive, re-eval on file change, drive `typst watch`. |
| `evcxr-typst clean [--gc] [--root <dir>] <file>` | Drop sidecar view for a document; `--gc` also evicts unreferenced CAS entries. |

Run `evcxr-typst --help` or `evcxr-typst <subcommand> --help` for full flag documentation.

## Safety and `--allow-eval`

The CLI refuses to execute Rust snippets unless `--allow-eval` is passed explicitly. Bare `typst compile` of a document that uses this package is always safe: the package renders placeholder boxes when sidecars are absent. See [`docs/DECISIONS.md`](docs/DECISIONS.md) D-004.

## Caching and watch mode

Snippet output is cached by a Blake3 content-addressed store under `.evcxr-typst-cache/`. Unchanged snippets are skipped on re-run; edited snippets re-evaluate from the first changed snippet forward (evcxr's state is forward-only). Watch mode hooks into `notify` (recursive on the entry's parent dir, so subdirectories of multi-file documents are covered) and drives `typst watch` as a child process. The CLI accepts both relative and absolute entry paths; the entry is canonicalized at watch start so the comparison against notify's absolute event paths works either way. See [`docs/design/cache.md`](docs/design/cache.md) and [`docs/design/watch-loop.md`](docs/design/watch-loop.md).

VSCode users can drive the watch loop through the bundled `.vscode/tasks.json` — `Tasks: Run Task → evcxr-typst: watch (rust-by-example)` starts the rbe authoring session.

## Repository layout

```
evcxr-typst/
├── docs/                    # plans, architecture, decisions, backlog
├── crates/
│   └── evcxr-typst/         # Rust CLI and library
├── packages/
│   └── evcxr/               # Typst package
├── examples/
│   ├── hello/               # end-to-end smoke document
│   ├── image/               # MIME passthrough (T-I04)
│   ├── errors/              # pretty error rendering (T-I07)
│   └── rust-by-example/     # incremental hand-port of upstream rust-by-example (track/rbe-incremental)
├── journal/                 # working log of the rbe port
└── .vscode/                 # VSCode tasks (build/watch/run/clean)
```

## Documents

- [`docs/PLAN.md`](docs/PLAN.md) — phased roadmap and current status.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — design rationale, pipeline, MIME mapping, watch loop, why-not-WASM.
- [`docs/BACKLOG.md`](docs/BACKLOG.md) — agent-ready task queue.
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — ADR-lite log of design decisions.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT License ([`LICENSE-MIT`](LICENSE-MIT))

at your option.
