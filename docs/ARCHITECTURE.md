# Architecture

Companion to `PLAN.md`. PLAN says *what we build and when*; this says *why it's shaped this way and how the pieces fit*.

## The pipeline

```
            evcxr-typst (CLI, this repo)
            ┌──────────────────────────────────────────────┐
main.typ ── │  1. typst query   → snippet list (JSON)      │
            │  2. evcxr::CommandContext, drive top→bottom  │
            │     (:dep persists, :cache=on, vars persist) │
            │  3. write sidecars (stdout, images, cbor…)   │── .evcxr-typst-cache/<id>.{txt,png,cbor,…}
            │  4. typst compile (→ PDF + SVG)              │
            │     | typst watch                            │
            └──────────────────────────────────────────────┘
                                │
                                ▼
            packages/evcxr/lib.typ (Typst package, this repo)
            ┌──────────────────────────────────────────────┐
            │ #rust(```rust …```) emits <evcxr-snippet>    │
            │   metadata + reads sidecar at render time    │
            └──────────────────────────────────────────────┘
```

Two artifacts in this repo: a Rust **CLI** and a Typst **package**. They are coupled by the schema of the `<evcxr-snippet>` metadata marker and the sidecar file layout — that schema is the public contract between the two pieces and should be versioned as such.

## Why prequery, not a WASM plugin

Typst plugins are sandboxed WebAssembly: pure byte-in / byte-out functions, no syscalls, no filesystem, no network, no spawning processes. evcxr's job is essentially "manage subprocesses": it spawns rustc/cargo, dlopens cdylibs via `libloading`, and runs a long-lived host child process so Rust variables persist across snippets. Approximately none of evcxr can run inside a Typst plugin. See `DECISIONS.md` D-001.

What's left is the prequery model: an external preprocessor does the real work, writes results to disk, and Typst — in its sandbox — just reads them. This is exactly what the upstream `prequery` package was built for, just with "evaluate Rust code" in place of "download an image".

## The metadata contract

The Typst package emits, at the location of each snippet, a `metadata((...))<evcxr-snippet>` value (and similar for `<evcxr-dep>`). Schema (subject to change pre-1.0; pinned via a version field):

```json
{
  "v": 1,
  "id": "<stable id>",
  "kind": "rust" | "rust-out" | "rust-display" | "rust-hidden" | "rust-data",
  "src": "<the rust source>",
  "deps": ["<explicit-dep-id>", "..."],
  "options": { "prefer": "image/png", "format": "auto", "...": "..." },
  "loc": { "doc_order": 7 }
}
```

`deps` lists explicit dep ids passed via the `deps:` kwarg — implicit document-order deps are not in the metadata; the CLI infers them from `<evcxr-dep>` markers in document order. `options` is a forward-compatible bag of kind-specific kwargs; older CLIs ignore unknown keys. Exact schema and field semantics live in `docs/design/package-api.md` § 5.

A parallel `<evcxr-dep>` marker carries dep specifications (see `docs/design/package-api.md` § 5.2).

`typst query --field value <doc> '<evcxr-snippet>'` returns these in document order, with their physical location. The CLI consumes that, drives evcxr, and writes sidecars keyed by `id`.

## Snippet identity

This is one of the more subtle design points; details live in `docs/design/snippet-identity.md` (Phase 0 design task). The constraints:

- Stable across edits to *unrelated* snippets, so adding a paragraph doesn't invalidate every cache entry below it.
- Stable across edits to whitespace / comments inside the snippet itself when feasible (open question — we may not bother).
- Cheap to compute from the Typst source alone (no AST analysis of Rust code).
- Easy to override with an explicit `id:` argument when the user wants reproducible naming.

Working assumption for v0: `id = explicit_id_or(blake3(src)[:12])`. Document order is captured separately as `loc.doc_order`. Re-evaluation order is `loc.doc_order` (see "Composition across snippets" below).

## Composition across snippets

evcxr already supports the natural composition story for Rust REPLs: each evaluation can introduce items (structs, enums, traits, impls, fns, modules, `use`s) and bindings that subsequent evaluations see. The key design exercise is mapping that to Typst snippets. Detail in `docs/design/snippet-semantics.md`.

Quick map of what falls out of evcxr today:

| Rust construct | Persists across snippets? | Notes |
|---|---|---|
| `let` bindings | yes, **unless** an intervening snippet panics — see D-011 | Can't reference previous bindings (borrow-checker / `'static`); see evcxr's `COMMON.md` "References". |
| `fn` definitions | yes | Stored in `committed_state.items`; survives child respawn (rebuilt from items). |
| `struct`/`enum`/`trait`/`impl` | yes | Same. `impl` blocks attach to the previously-named item — see snippet-semantics.md § Rules.1. |
| `mod foo { … }` (inline) | yes | The canonical form — see D-008. |
| `mod foo;` (file-based) | **rejected** — see D-008 | Resolves to evcxr's tmpdir, not the document's directory. Use inline modules or `:dep`. |
| `use foo::bar;` | yes | Use-trees are merged across snippets; see `evcxr/src/use_trees.rs`. |
| Macros | importing from external crates: **no** (documented limitation). Local `macro_rules!`: yes. |
| Lifetimes / borrows across snippets | restricted | Persisted vars must be `'static`-ish. Workarounds: scope-limit, `Box::leak`. |

This is critical UX: a writer should be able to "define a struct in one snippet, use it three pages later" — and that has to Just Work.

The full construct matrix and rules live in `docs/design/snippet-semantics.md`.

## MIME → Typst output mapping

evcxr's display protocol is line-based: code emits `EVCXR_BEGIN_CONTENT <mime>\n<payload>\nEVCXR_END_CONTENT` to stdout. We capture all such blocks per snippet.

| MIME type | Sidecar file | Typst rendering |
|---|---|---|
| `text/plain` (default stdout) | `<id>.txt` | `raw(read("…"))` inside a styled box |
| `text/html` | `<id>.html` | `html.frame(read("…"))` (Typst 0.13+ HTML export) or fall back to verbatim |
| `image/png` | `<id>.png` | `image("…")` |
| `image/svg+xml` | `<id>.svg` | `image("…")` |
| `image/jpeg` | `<id>.jpg` | `image("…")` |
| `application/json` | `<id>.json` | `json("…")` returns dict/array |
| `application/cbor` | `<id>.cbor` | `cbor("…")` returns dict/array |
| (anything else) | `<id>.<ext>` + `<id>.meta.json` | raw box with mime stamped on |
| **errors** (compile/panic/timeout/dep-resolution) | `<id>.error.json` | styled error box; takes precedence over other sidecars. Schema: `docs/design/errors.md` § 2. |

Stdout that is *not* wrapped in BEGIN/END is the snippet's plain text output. A snippet that produces both display objects and plain stdout writes both `<id>.txt` and `<id>.png` (etc.), and the Typst package decides which to surface based on the called function (`rust` vs `rust-out` vs `rust-display`).

## Caching

Two layers, both essential, both already partly built elsewhere:

1. **rustc artifact cache** — evcxr already has this (`:cache <MB>` directive). We turn it on by default with a sane budget. This is what makes "edit one snippet" cheap — the dep crates are already compiled.
2. **Snippet output cache** — ours to build. Cache key hashes snippet src + a Merkle chain over prior snippets + active deps + evcxr/rustc/target/env. Storage uses content-addressed CAS at `.evcxr-typst-cache/v1/cas/<XX>/<full-key>/` with a separate id-addressed view (hardlinks) that the Typst package reads at render time. The package never sees the CAS. CAS-by-key gives us free dedup across documents and easy GC. Detail and exact formula in `docs/design/cache.md`; locked in by D-010.

## Watch loop

Long-lived `CommandContext`. Watch the `.typ` file with `notify`. On change:

1. Re-query snippets. Diff against the previous list (id + src).
2. Classify each diff:
   - **Added at the end** → just evaluate the new ones.
   - **Removed at the end** → drop sidecars; nothing to re-eval.
   - **Modified leaf** (no items, no `:dep`, no `let`) → re-eval just this snippet.
   - **Modified non-leaf or anything earlier than the end** → reset the `CommandContext` and re-eval from the first changed snippet onward. Rustc cache makes this cheap in practice.
3. `typst watch` runs as a child process; it notices our sidecar mtime changes and re-renders incrementally.

The "just reset on middle-edit" choice is honest and simple. Snapshot/restore inside evcxr would be the principled fix, but that's a substantial upstream change and we should measure first. Detail in `docs/design/watch-loop.md`.

## Render outputs

`evcxr-typst run` shells out to `typst compile` **twice**: once for `<stem>.pdf` (the user-facing artifact) and once for `<stem>.svg` next to the entry file. PDF is what authors actually distribute; SVG is for the dev loop — opening fast in a browser, easy to diff visually. Typst's SVG embeds glyphs as `<path>` references rather than `<text>` elements, so the SVG is not text-grep-able for snippet output; for that, agents and scripts read the textual sidecars at `.evcxr-typst-cache/<id>.txt`. The two together cover "did it lay out" (SVG) and "did it evaluate" (sidecars). Multi-page documents need `typst compile` invoked directly with a `{p}` template.

## Fallback / safety

`typst compile main.typ` of a document that uses our package, **without** running our CLI, must be safe and must produce sensible output (placeholder boxes where evaluated output would go), in whichever format the user requests. Concretely:

- The Typst package detects missing sidecar files and renders a placeholder.
- The CLI requires `--allow-eval` to actually execute Rust. Otherwise it does query + sidecar-validity-check only.
- The package never invokes Rust. All execution is gated by the CLI being explicitly run.

This is the same safety model as upstream `prequery`. See D-004.

## Where things live in the source tree

```
crates/evcxr-typst/
  src/
    main.rs              # CLI entry, calls evcxr::runtime_hook() FIRST (mandatory)
    cli.rs               # clap config: run, watch, clean, query
    discover.rs          # shells out to `typst query`, parses snippet JSON
    session.rs           # owns CommandContext, drives snippets, captures output
    sidecar.rs           # MIME → file mapping, atomic writes
    cache.rs             # snippet-output cache
    watch.rs             # notify + change classification + typst watch wrapper
packages/evcxr/
  typst.toml             # package manifest
  lib.typ                # rust(), rust-out(), rust-display(), dep(), …
  fallback.typ           # placeholder rendering when sidecars are missing
examples/
  hello/                 # the Phase 1 smoke test
  gallery/               # showcase docs (one per scenario)
```
