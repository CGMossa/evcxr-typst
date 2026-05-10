# How to write your first evcxr-typst document

This teaches: importing the `evcxr` Typst package, running an authoring watch loop, and iterating until your snippet compiles and produces the output you want. You need this when you're starting a new document, or porting an existing markdown / mdBook chapter.

The primary authoring loop is `evcxr-typst watch`. Use `evcxr-typst run` only for one-shot builds (CI, final renders).

## Minimal example

Save as `hello.typ` anywhere under your repo root:

```typ
#import "../../packages/evcxr/lib.typ" as evcxr

#evcxr.setup()

= Hello

#evcxr.rust(id: "hello-1", ```rust
println!("Hello World!");
```)
```

## Author with `watch`

In one terminal, from the repo root, start watch and leave it running:

```sh
cargo run -p evcxr-typst -- watch --allow-eval --root . path/to/hello.typ
```

Output will look like:

```
watch running; press Ctrl-C to stop.
watching path/to/hello.typ
writing to path/to/hello.pdf

[14:21:16] compiling ...
[14:21:16] compiled successfully in 41.62 ms
```

In a second terminal (or your editor), edit `hello.typ`. Each save:

1. evcxr-typst re-discovers the snippets, classifies the change (added / removed / leaf-modified / non-leaf), and re-evaluates only what's needed (D-003).
2. Cargo / rustc errors from the snippet appear in the watch terminal.
3. The captured stdout lands in `path/to/.evcxr-typst-cache/hello-1.txt`.
4. The Typst-watch child re-renders `path/to/hello.pdf`.

Iterate: write code, save, see error or output, fix, save. Stop with **Ctrl-C** — the CLI installs a SIGINT handler that asks the watch loop to shut down cleanly.

The first compile of a fresh snippet is slow (rustc + crate fetches). Subsequent compiles are seconds — evcxr's `:cache 500` (rustc artifact cache) and the per-snippet content-addressed cache (T-I05) are both on by default.

## Cold one-shot for CI

When you want a single PDF, no live loop:

```sh
# Bare Typst — placeholders only, no Rust evaluated. Always succeeds (D-004).
typst compile --root . path/to/hello.typ

# Evaluated end to end.
cargo run -p evcxr-typst -- run --allow-eval --root . path/to/hello.typ
```

`run` produces the same `<entry>.pdf` that watch does, then exits.

## Why it works

- `#import "../../packages/evcxr/lib.typ" as evcxr` brings the package's public functions into scope. Adjust the relative path to match where the package lives relative to your file. Until the package is published to Typst Universe (issue #17), this is how you depend on it.
- `#evcxr.setup()` emits a `<evcxr-setup>` metadata marker that the CLI reads via `typst query` to discover document-wide options. Call it once per entry file. (You don't need to call it from `#include`'d sub-files — see "Variations".)
- `#evcxr.rust(id: "hello-1", ```rust ... ```)` emits a `<evcxr-snippet>` metadata marker, then renders the source as a code block, then reads `<id>.txt` from the cache and renders it below. The `id:` is what the cache file is named — pick something stable.
- Bare `typst compile` works because the package gates every sidecar read on a `_index-available()` check (D-004 / T-I06). When sidecars don't exist, every `#evcxr.rust` falls through to a placeholder box.
- `evcxr-typst watch` spawns `typst watch` as a child for the rendering pass and runs its own evcxr-driving loop alongside, sharing one `CommandContext` across cycles so cross-snippet state persists.

## Variations

- **`fn main()` from upstream sources:** evcxr executes top-level statements; if you keep an upstream `fn main() { ... }` wrapper, evcxr will *define* `main` but never call it. For now, drop the wrapper. (Tracked: `T-B01` will add `rust-main(...)` for true fidelity.)
- **Multi-chapter document:** make each chapter its own `.typ` file. Each chapter file `#import`s the package independently — `#include` does not share scope with the entry file. `setup()` is called once in the entry file only. See `examples/rust-by-example/main.typ` for the pattern.
- **No source rendering, just stdout:** use `#evcxr.rust-out(...)` instead of `#evcxr.rust(...)`. Shows captured stdout but not the source code.
- **Hide the snippet entirely (e.g. setup helpers):** use `#evcxr.rust-hidden(...)`. Useful when you want a side-effect (defining a function, adding a `:dep`) without the visual block.
- **Pull a crate from crates.io:** `#evcxr.dep("regex", version: "1")` placed before any snippet that uses `regex::Regex` makes evcxr resolve and link it. The first dep snippet pays cargo-build cost; subsequent runs hit `:cache 500`.
- **Multi-page books and SVG:** the CLI also tries to render an `<entry>.svg` next to the PDF for visual quick-look. For multi-page documents (an `#outline()` plus body is enough to be multi-page), Typst rejects the single-file SVG path and the CLI prints `warning: SVG render skipped`. The PDF is unaffected. If you want per-page SVGs, run `typst compile` directly with a `{p}` template.

## See also

- `examples/hello/main.typ` — the smallest possible end-to-end document (no chapter structure).
- `examples/rust-by-example/main.typ` — multi-chapter book using the include-per-chapter pattern.
- `docs/design/package-api.md` — the full API surface (`rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data`, `dep`, `setup`).
- `docs/design/snippet-semantics.md` — how items, bindings, and modules persist across snippets.
- `docs/design/watch-loop.md` — change classification and re-eval policy.
- `docs/DECISIONS.md` D-003 (linear re-eval), D-004 (fallback by default), D-007 (snippet identity), D-013 (inline `dep`).
- `journal/2026-05-09-001-hello.md` and `journal/2026-05-09-002-watch-loop-exits-immediately.md` — working notes that motivated this tutorial.
