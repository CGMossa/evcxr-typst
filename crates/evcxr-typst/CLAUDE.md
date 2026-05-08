# CLAUDE.md — `crates/evcxr-typst/`

Both the Rust CLI (`evcxr-typst` binary; three subcommands `run`, `watch`, `clean`) and the public library API (`evcxr_typst::*`) that other hosts can embed. Single crate, two targets — see **D-023** and `docs/design/library-api.md`.

```
src/
  lib.rs   — public API surface (Project, EvalOptions, EvalCallbacks, …)
  main.rs  — 9-line binary entry: runtime_hook() then cli::run()
  cli.rs   — binary-only clap layer; library never sees clap
examples/
  library_use.rs — canonical embedder; mirrors evcxr's example_eval.rs
```

## Status

API surface landed (T-L01). Method bodies still stub to `Err(Error::NotImplemented(<method>))`. Real eval logic lands in **T-I03** onward; subsequent tasks must populate the library entry points (`Project::evaluate`, `Project::watch`, `Project::clean_view`) and keep `main.rs` / `cli.rs` thin — every code path goes through the library API.

## Critical invariants

- **`evcxr::runtime_hook()` must be the very first thing in `main`.** Before clap, before logging init, before anything. evcxr re-enters this binary as a host child process or rustc wrapper depending on env vars — if anything else runs first, that path breaks silently. See `.evcxr/evcxr/src/runtime.rs`. The library never calls `runtime_hook` itself — the embedder must (D-023). Both `main.rs` and `examples/library_use.rs` model the contract.
- **evcxr is a path dependency** (`../../.evcxr/evcxr`, from this Cargo.toml) per **D-006** / **D-025**. Don't switch to crates.io until the release pinning task explicitly says so.
- **Library is clap-free.** clap stays in `src/cli.rs` (binary-only). If you find yourself wanting `clap::Args` in `lib.rs`, push the parsed values across the boundary as plain types instead.
- **`#![warn(missing_docs)]` on `lib.rs`.** Every new `pub` item needs rustdoc; CI surfaces a warning otherwise. `cargo doc -p evcxr-typst --no-deps` should stay clean.

## Build / test

From the repo root:

```sh
cargo build -p evcxr-typst --all-targets   # lib + bin + library_use example
cargo run -p evcxr-typst -- --help
cargo run -p evcxr-typst --example library_use -- path/to/main.typ
cargo doc -p evcxr-typst --no-deps         # must stay missing-docs-warning-clean
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
