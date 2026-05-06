# CLAUDE.md — `crates/evcxr-typst/`

The Rust CLI. Single binary, three subcommands (`run`, `watch`, `clean`), drives the whole prequery loop.

## Status

Scaffolding only. `main.rs` parses CLI args and exits with an "unimplemented" message. Real implementation is queued as **T-I01..T-I07** in `../../docs/BACKLOG.md`.

## Critical invariants

- **`evcxr::runtime_hook()` must be the very first thing in `main`.** Before clap, before logging init, before anything. evcxr re-enters this binary as a host child process or rustc wrapper depending on env vars — if anything else runs first, that path breaks silently. See `/Users/elea/Documents/GitHub/evcxr/evcxr/src/runtime.rs`.
- **evcxr is a path dependency** (`../../../evcxr/evcxr`) per **D-006**. Don't switch to crates.io until the release pinning task explicitly says so.

## Build / test

From the repo root:

```sh
cargo build -p evcxr-typst
cargo run -p evcxr-typst -- --help
cargo test -p evcxr-typst
```

When tests start touching `EvalContext`, follow evcxr's CI rules: `cargo test -- --test-threads 1`. Multiple `EvalContext`s in parallel don't work (it's a global-process limitation, not flakiness).

## Design references — read before coding

The big picture lives in `../../docs/ARCHITECTURE.md`. The detail you need depends on what you're touching:

| Touching… | Read first |
|---|---|
| Snippet discovery / `typst query` | `docs/design/multi-file.md`, `docs/design/snippet-identity.md` |
| The eval loop / `CommandContext` driving | `docs/design/snippet-semantics.md`, `docs/DECISIONS.md` D-003, D-009, D-011, D-017 |
| Cache | `docs/design/cache.md`, `docs/DECISIONS.md` D-010 |
| Watch mode | `docs/design/watch-loop.md` |
| Error reporting | `docs/design/errors.md` |
| Sidecar / metadata schema | `docs/design/package-api.md` § 5, `docs/design/schema-versioning.md` |

(All paths above are relative to the repo root, two levels up from this file.)

## Conventions

- Match evcxr's style: dual MIT/Apache-2.0 headers on new files; rustfmt clean; clippy clean on `x86_64-unknown-linux-gnu`.
- No comments explaining what the code does — names should. Comments are for *why* a non-obvious thing is the way it is. (Same rule as evcxr's own `CLAUDE.md`.)

## What does NOT belong here

- The Typst package — that's `../../packages/evcxr/`.
- Examples / fixtures — `../../examples/`.
- Anything that re-implements logic already in evcxr (e.g. parsing Rust spans). If you need it, expose it from evcxr upstream rather than mirroring.
