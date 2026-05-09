# evcxr — Typst package

Embed evcxr-evaluated Rust snippets in Typst documents. The package emits metadata markers that the [`evcxr-typst`](https://github.com/CGMossa/evcxr-typst) CLI reads to drive a long-lived Rust evaluation context, then reads the resulting sidecars back at render time. Bare `typst compile` is always safe — snippets without sidecars render as placeholder boxes (no Rust ever executes from a Typst-only build).

## Getting started

Once published to Universe, this is the import line:

```typ
#import "@preview/evcxr:0.1.0" as evcxr
#evcxr.setup()

#evcxr.rust(```rust
println!("Hello from evcxr!");
```)
```

Until publication, vendor the package locally:

```typ
#import "../path/to/evcxr-typst/packages/evcxr/lib.typ" as evcxr
```

To actually evaluate the Rust:

```sh
evcxr-typst run --allow-eval --root . main.typ
```

`--allow-eval` is required — the CLI refuses to execute Rust without it.

## Public functions

| Function | Purpose |
|---|---|
| `setup(min-cli: ...)` | Boilerplate at top of doc; emits version-pin marker. |
| `rust(code, ...)` | Snippet that captures `println!` / `eprintln!` and renders source + output. |
| `rust-out(code, ...)` | Like `rust`, but only the captured output (no source). |
| `rust-display(code, ...)` | Snippet that emits a MIME-typed object via `evcxr_runtime::mime_type(...)`; the package embeds PNG/SVG/JPEG/etc. |
| `rust-hidden(code, ...)` | Runs the snippet for its side effects, renders nothing. |
| `rust-data(code, ...)` | Emits structured data (JSON, CBOR) consumable by `rust-data-read`. |
| `rust-data-read(id, fallback)` | Reads structured data from a previous snippet's sidecar. |
| `dep(name, version, ...)` | Adds a `[dependencies]` entry; can appear inline anywhere in the doc. |

See [`packages/evcxr/CLAUDE.md`](CLAUDE.md) for invariants and decision records, and the upstream [`docs/design/package-api.md`](https://github.com/CGMossa/evcxr-typst/blob/main/docs/design/package-api.md) for the full reference.

## Safety model

- **Bare `typst compile` never executes Rust.** `lib.typ` only emits metadata markers and reads sidecars; the package contains no I/O that triggers code execution.
- **Sidecars are reproducible.** Each snippet's id is a Blake3 hash of its source; the CLI is content-addressed.
- **Errors render visibly.** Compile errors, runtime panics, and missing-crate failures produce styled error boxes — the document renders, with the failure surfaced where it occurred.

## License

Dual-licensed under MIT or Apache-2.0. See the repository's `LICENSE-MIT` and `LICENSE-APACHE`.
