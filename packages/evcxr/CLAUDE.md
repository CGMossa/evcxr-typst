# CLAUDE.md — `packages/evcxr/`

The Typst package. This is the user-facing surface — everything Typst writers ever see of evcxr-typst lives in `lib.typ` and `fallback.typ`.

## Status

Phases 1–4 complete (T-I02 through T-I08). All public functions emit `<evcxr-snippet>` / `<evcxr-dep>` metadata markers and read sidecars when run via the CLI. The `_index.json` guard (T-I06) ensures bare `typst compile` always succeeds with placeholder boxes. `error.typ` (T-I07) supplies error-box rendering for compile errors, runtime panics, dep failures, and timeouts. The `setup(min-cli: ...)` / `<evcxr-min-cli>` mechanism (D-019, D-026) is wired in `lib.typ`; CLI-side enforcement landed in T-I08. Package version is `0.1.0`.

## Critical invariants

- **D-004 — fallback by default.** Bare `typst compile` of any document using this package must succeed and produce a sensible PDF, even when no Rust has been evaluated. `lib.typ` must never produce a hard error from a missing sidecar; it falls back to the `placeholder()` from `fallback.typ`.
- **The package never executes Rust.** All evaluation is gated behind the CLI being explicitly run with `--allow-eval`.
- **The package only ever reads the id-addressed materialized view** of the cache (per D-010), not the CAS. The CLI is responsible for materializing the view.
- **Labels `<id>` / `<id-out>` are only emitted for explicitly-provided ids.** Auto-derived IDs (blake3 hashes) do not get labels. `<id-out>` is only emitted when real evaluated output is present — never on fallback placeholders. `rust-hidden` and `rust-data` emit no labels.

## Public API — pinned by decision records

| Function | Decision |
|---|---|
| `rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data` | D-012 |
| `rust-main` (CLI appends a hidden `main();` call; `options.auto-call: "main"` recorded additively per D-019) | D-022 |
| `dep` (inline-anywhere) | D-013 |
| `rust-data` failure shape (`fallback:` kwarg, returns `none` on snippet error) | D-015 |
| `timeout:` kwarg on all eval functions | D-017 |
| `setup(min-cli: ...)` and `<evcxr-min-cli>` marker | D-019 |
| Labels `<id>` / `<id-out>` on code and output blocks when explicit `id:` is provided | id-as-label |

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
