# CLAUDE.md — `examples/hello/`

The Phase 1 smoke test. Smallest possible end-to-end document: one snippet, one `println!`, no items, no `:dep`.

When `evcxr-typst run --allow-eval main.typ` is implemented (T-I03), the rendered PDF should contain the captured stdout from the snippet. Until then, the document compiles to a placeholder box.

This is *also* the regression target for the package's fallback path (D-004) — `typst compile main.typ` must always succeed here, regardless of CLI state.

If you change the strawman in `lib.typ`, update this example so the import line still works (`#import "../../packages/evcxr/lib.typ" as evcxr`). Once we publish to Universe, this and the other examples switch to `@preview/evcxr:X.Y.Z` in one coordinated PR.
