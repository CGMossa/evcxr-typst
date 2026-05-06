# Example gallery (design)

These are **specification documents**, not runnable code. They show the
target user-facing API of the `packages/evcxr/` Typst package as it would
read in a finished document. Function names, signatures, and defaults are
strawmen here --- the package API itself is designed in task T-D03, and
naming choices flagged at the bottom of this file are explicitly handed
to that task to bikeshed.

The strawman API used throughout the gallery:

| Function | Purpose |
|---|---|
| `rust(code)` | Show the code AND its captured stdout. |
| `rust-out(code)` | Show only the captured stdout (hide the source). |
| `rust-display(code)` | Show only the display object (image / HTML / etc). |
| `rust-hidden(code)` | Evaluate but render nothing. Setup-style snippets. |
| `rust-data(code)` | Return a Typst dictionary from a JSON/CBOR-emitting snippet. |
| `dep(name, version)` | `:dep`-equivalent. Optional `features:` argument. |

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

## Naming choices for T-D03 to bikeshed

These calls were made in the strawman to keep the examples readable.
None are precious; T-D03 should reopen any of them.

- **`rust` vs `evcxr` vs `eval`** as the primary verb. `rust(code)` reads
  well in prose, but conflates "the language" with "this particular eval
  context". `evcxr(code)` is honest but awkward to a non-Rust reader.
- **`rust-display` vs `rust-image` vs `rust-figure`.** "Display" matches
  evcxr's vocabulary (`evcxr_display`) but is overloaded in the
  Typst/CSS world. "Image" is too narrow (HTML output also uses this).
- **`rust-data` vs `rust-value` vs `rust-json`.** `rust-data` reads well;
  it might suggest "a dataset" rather than "any structured value".
- **`dep` vs `cargo` vs `crate-dep`.** `dep` matches evcxr's `:dep`
  directive but is generic enough to clash mentally with Typst's own
  package imports. Consider whether `dep` should accept the same
  table-of-options as `:dep` does (`features`, `default-features`,
  `git`, `path`, `package`).
- **Hyphens vs underscores.** Typst conventions allow hyphens in
  function names (`raw-block`, `outline-entry` precedent), but Rust
  readers may instinctively read them as subtraction. T-D03 should
  pick one and apply consistently; the gallery uses hyphens throughout.
- **Implicit ID vs explicit ID argument.** None of the gallery examples
  pass an explicit `id:`. The package needs a syntax for it
  (`rust(code, id: "fib")`?) so authors can pin a snippet's identity
  across edits. T-D04 cares about this.
- **Dep emission timing.** `dep(...)` in the gallery is rendered as a
  bare top-level call. It might be cleaner if it were a `setup()`-style
  document-prelude block that visually doesn't render at all
  (vs. emitting a `<evcxr-dep>` metadata marker as side effect of being
  called inline).
