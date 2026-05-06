# CLAUDE.md — `crates/`

Cargo workspace member directory. Currently holds the single binary crate `evcxr-typst/`. Future crates (e.g. a separate library if we split the prequery scanner from the eval driver) go here as siblings, picked up by the workspace `members = ["crates/*"]` glob in the root `Cargo.toml`.

Per-crate guidance lives in each crate's own `CLAUDE.md`.

Don't put non-Cargo artifacts here. The Typst package and example documents live in `../packages/` and `../examples/` respectively.
