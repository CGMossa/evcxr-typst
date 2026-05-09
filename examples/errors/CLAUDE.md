# CLAUDE.md — `examples/errors/`

Smoke test for T-I07 (pretty error rendering). The doc deliberately exercises three error classes — compile error, runtime panic, dep-resolution failure — and asserts that each renders as a styled error box rather than aborting the document.

Standard fallback rule still holds: bare `typst compile main.typ` must succeed (D-004) — without `--allow-eval` the package renders placeholder boxes for snippets that have no sidecar yet, including no error sidecar.

## What this example pins

- `error_capture::ErrorSidecar` round-trips through the CLI sidecar format.
- `lib.typ` renders an error box when `.error.json` exists alongside the snippet's stdout sidecar.
- The CLI exits non-zero if any snippet errored (so CI / scripts can detect failure).
- Dep resolution failures surface in the same path (the `nonexistent-crate-…` line is the canary).

## Don't

- Don't change the snippet IDs (`e-compile`, `e-panic`, `e-dep`, `e-dep-use`) — they are referenced from sidecar fixture tests in `crates/evcxr-typst/tests/`.
- Don't make any of these snippets succeed. The point is that they fail visibly.
- Don't delete this directory when running `evcxr-typst clean --gc` cleanups elsewhere; the `.evcxr-typst-cache/` here is the smoke-test fixture.

## Run

```sh
evcxr-typst run --allow-eval --root . examples/errors/main.typ
```

Expected: PDF + SVG produced; each snippet section shows a styled error box; exit code non-zero. See `docs/design/errors.md` for the full error-rendering pipeline.
