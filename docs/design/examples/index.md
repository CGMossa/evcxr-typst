# Example gallery (design)

These are **specification documents**, not runnable code. They show the
target user-facing API of the `packages/evcxr/` Typst package as it would
read in a finished document. Names are now finalised (D-012, D-013); the
authoritative reference is `docs/design/package-api.md`.

API summary used throughout the gallery:

| Function | Purpose |
|---|---|
| `rust(code)` | Show the code AND its captured stdout (default `render: "both"`). |
| `rust-out(code)` | Show only the captured stdout (hide the source). |
| `rust-display(code)` | Show only the display object (image / HTML / etc), with `prefer:` to pick among multiple artifacts. |
| `rust-hidden(code)` | Evaluate but render nothing. Setup / definition / fixture snippets. |
| `rust-data(code)` | Return a Typst dictionary from a JSON/CBOR-emitting snippet. Returns `none` on snippet error; returns `fallback:` (default `(:)`) when not yet evaluated. |
| `dep(name, version, ..)` | `:dep`-equivalent. `version` is positional; `features:`, `git:`, `path:`, `package:`, `default-features:` are kwargs. A single `"name = …"` string is treated as a TOML fragment. |

## The gallery

| File | One-line description | Feature |
|---|---|---|
| `a-hello.typ` | The minimum viable document --- one `println!`. | End-to-end smoke. |
| `b-struct-across-snippets.typ` | Define a `struct` early, instantiate it pages later. | Persistent items (struct/impl). **Cross-snippet.** |
| `c-module-across-snippets.typ` | Define a `mod`, `use` items from it, call them. | Persistent items (mod/use). **Cross-snippet.** |
| `d-image-output.typ` | Generate a PNG with `evcxr_image` and embed inline. | MIME passthrough (`image/png`). |
| `e-cratesio-dep.typ` | Pull `regex` from crates.io and use it. | `dep` / `:dep` plumbing. |
| `f-async-tokio.typ` | Plain `.await` in a snippet --- evcxr auto-spins tokio. | Async/await detection. |
| `g-error-case.typ` | Deliberate compile error; document still renders. | Error fallback. |
| `h-mini-report.typ` | Five interleaved snippets: data → stats → table → chart → prose. | The whole pipeline. **Cross-snippet (multiple).** |

## Cross-snippet composition examples

The brief required at least three. Four scenarios in the gallery exercise
cross-snippet item composition (T-D01's territory):

- `b-struct-across-snippets.typ` --- `struct` + `impl` defined early, used late.
- `c-module-across-snippets.typ` --- `mod` definition, then `use`, then a consumer call: three snippets, all linked.
- `g-error-case.typ` --- mostly an error-rendering example, but the trailing snippet implicitly depends on context state surviving the failure.
- `h-mini-report.typ` --- the ambitious one: five snippets, with later ones consuming bindings (`RAIN`, `stats`) and items (`Summary`, `summary()`) committed by earlier ones. Snippet 5 also illustrates that the dependency graph is *not* a chain --- it skips snippets 3 and 4 --- which is a load-bearing detail for the cache key in T-D04.

## Naming choices — resolved (D-012, D-013)

The names used throughout the gallery are now the final v0 API. Recap:

- Primary verb: **`rust`**.
- Output-only inline: **`rust-out`**.
- Display-only (image / HTML / etc.): **`rust-display`**, with a
  `prefer:` kwarg to pick among multiple display artifacts.
- Evaluate-and-render-nothing: **`rust-hidden`**.
- Return parsed JSON/CBOR as a Typst dict: **`rust-data`**.
- Cargo dependency: **`dep(name, version?, …)`**, accepting the same
  table-of-options as evcxr's `:dep` (`features`, `default-features`,
  `git`, `path`, `package`), plus a TOML-fragment escape hatch
  (`dep("serde = \"1.0\"")`).
- Hyphens, not underscores, throughout — matches Typst stdlib idiom
  (`raw-block`, `outline-entry`).
- Default `render:` for `rust(...)` is `"both"` (source + output);
  configurable via `setup(default-render: ...)`.
- `dep(...)` calls remain inline-anywhere (D-013); the CLI
  pre-collects them in document order and errors on conflicting
  versions per snippet-semantics G5.
- Explicit `id:` argument is supported on every function for stable
  identities across edits.

See `docs/design/package-api.md` for the full function reference.
