# CLAUDE.md — `examples/image/`

Smoke test for T-I04 (MIME passthrough). Two evidence paths in one document:

- **`rust-display` + `image/png` MIME.** Generates a 64×64 gradient with the `image` crate, ships it through `evcxr_runtime::mime_type("image/png").bytes(...)`, and lets the package embed the PNG sidecar in the PDF.
- **`rust-data` + `application/cbor` MIME.** Serializes a small dict via `ciborium`, ships it as CBOR, and reads it back with `rust-data-read` so the Typst document can format the values.

Together they pin the round-trip: a snippet emits a typed MIME blob, the CLI persists it as a sidecar at the right extension, and the package consumes it (image embed for binary types, structured-data deserialization for `application/cbor`/`application/json`).

## Critical invariants

- **Three `:dep` calls.** First `evcxr_runtime` (the bridge to evcxr's MIME protocol), then `image` and `ciborium`. Don't reorder — `evcxr_runtime` must be loaded before any snippet calls `mime_type(...)`.
- **Don't change the snippet IDs (`img-plot`, `cbor-stats`).** Sidecar fixture tests reference them.
- **Bare `typst compile` must succeed** (D-004). No-sidecar render falls back to placeholder boxes for the snippets and a sensible-fallback dict for `rust-data-read` (see the `fallback:` kwarg on line 58).

## Run

```sh
evcxr-typst run --allow-eval --root . examples/image/main.typ
```

The first run is slow (cargo fetches `evcxr_runtime`, `image`, `ciborium` and rustc-builds them). Subsequent runs hit the per-snippet CAS and the evcxr `:cache 500` artifact cache; both should complete in a few seconds.

Outputs of interest:
- `examples/image/.evcxr-typst-cache/img-plot.png` — valid PNG
- `examples/image/.evcxr-typst-cache/cbor-stats.cbor` — non-empty CBOR
- `examples/image/main.pdf` — rendered document with the image embedded and the CBOR-derived numbers in prose

See `docs/design/package-api.md` § MIME passthrough for the full mapping table and `docs/DECISIONS.md` D-014 / D-015 for the design rationale.
