# CLAUDE.md — `packages/evcxr/`

The Typst package. This is the user-facing surface — everything Typst writers ever see of evcxr-typst lives in `lib.typ` and `fallback.typ`.

## Status

Scaffolding only. Functions emit `<evcxr-snippet>` / `<evcxr-dep>` / `<evcxr-setup>` metadata markers (so `typst query` works) and render fallback placeholder boxes. They do **not** yet read sidecars — that lands in **T-I02 / T-I03**.

## Critical invariants

- **D-004 — fallback by default.** Bare `typst compile` of any document using this package must succeed and produce a sensible PDF, even when no Rust has been evaluated. `lib.typ` must never produce a hard error from a missing sidecar; it falls back to the `placeholder()` from `fallback.typ`.
- **The package never executes Rust.** All evaluation is gated behind the CLI being explicitly run with `--allow-eval`.
- **The package only ever reads the id-addressed materialized view** of the cache (per D-010), not the CAS. The CLI is responsible for materializing the view.

## Public API — pinned by decision records

| Function | Decision |
|---|---|
| `rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data` | D-012 |
| `dep` (inline-anywhere) | D-013 |
| `rust-data` failure shape (`fallback:` kwarg, returns `none` on snippet error) | D-015 |
| `timeout:` kwarg on all eval functions | D-017 |
| `setup(min-cli: ...)` and `<evcxr-min-cli>` marker | D-019 |

Don't add functions or kwargs without a decision record. Don't rename existing ones — the names are an external contract once we publish to Universe.

## Schema

The `metadata((...))<evcxr-snippet>` payload schema is documented in `../../docs/design/package-api.md` § 5 and `../../docs/design/schema-versioning.md`. Bumping any `v` field is a major-breaking change governed by D-019.

## Testing

Local-import the package from `../../examples/hello/main.typ`:

```typ
#import "../../packages/evcxr/lib.typ" as evcxr
```

Once we publish, examples switch to `@preview/evcxr:X.Y.Z`. Don't change the example imports until publication.

## What does NOT belong here

- Any Rust code (that's `crates/evcxr-typst/`).
- Test fixtures or example documents (that's `examples/`).
- Logic that depends on sidecar contents being evcxr-version-specific — keep version awareness behind the schema-versioning policy in D-019.
