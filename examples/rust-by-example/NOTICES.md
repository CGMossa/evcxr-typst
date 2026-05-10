# Notices and attribution

The Typst documents in this directory are adapted from the upstream
[rust-by-example](https://github.com/rust-lang/rust-by-example) book.

> Portions of this work are derived from rust-by-example, copyright (c)
> 2014–present The Rust-by-Example Authors, dual-licensed MIT and
> Apache-2.0.

## Upstream snapshot

| Field | Value |
|---|---|
| Upstream repo | <https://github.com/rust-lang/rust-by-example> |
| Local checkout | `.rust-by-example/` (gitignored at the repo root) |
| Snapshot commit | `898f0ac1479223d332309e0fce88d44b39927d28` |
| Snapshot date | 2026-04-05 |

When upstream changes, bump the snapshot commit here in the same commit that re-syncs any chapter affected by the change.

## License compatibility

Both upstream rust-by-example and this repository (`evcxr-typst`) are dual-licensed MIT / Apache-2.0. Each ported chapter `.typ` file carries a header comment of the form:

```typ
// Adapted from rust-by-example/<src-path>.md (see NOTICES.md).
```

so that downstream readers can trace any chapter back to upstream.

## What this directory is *not*

This directory is not a redistribution of rust-by-example. It is a translation of a subset of its content into Typst form, evaluated end-to-end through `evcxr-typst`. For the original mdBook experience, run upstream directly. The local checkout at `.rust-by-example/` is reference-only.
