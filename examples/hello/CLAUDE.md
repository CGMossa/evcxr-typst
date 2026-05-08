# CLAUDE.md — `examples/hello/`

The Phase 1 smoke test. Smallest possible end-to-end document: one snippet, one `println!`, no items, no `:dep`.

`evcxr-typst run --allow-eval main.typ` (T-I03 onward) renders both `main.pdf` and `main.svg` here. PDF is the user-facing artifact; SVG is for fast visual inspection in a browser. Note Typst SVG embeds glyphs as `<path>` references (not `<text>`), so to verify what was *evaluated* read `.evcxr-typst-cache/<id>.txt` instead.

This is *also* the regression target for the package's fallback path (D-004) — `typst compile main.typ` must always succeed here, regardless of CLI state.

If you change the strawman in `lib.typ`, update this example so the import line still works (`#import "../../packages/evcxr/lib.typ" as evcxr`). Once we publish to Universe, this and the other examples switch to `@preview/evcxr:X.Y.Z` in one coordinated PR.
