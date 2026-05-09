# CLAUDE.md — `docs/design/`

Per-area design specs. Each file describes one mechanism — how it works, why it's shaped that way, what the alternatives were, and what's open. Distinct from `docs/DECISIONS.md` (which is the *log* of choices) and `docs/tutorial/` (which is task-oriented for writers).

## Index

| File | Covers |
|---|---|
| `cache.md` | Blake3 CAS, materialised view, cache directory layout, GC. (D-010.) |
| `errors.md` | Compile/runtime/dep error capture pipeline; `error_capture::ErrorSidecar` schema; `lib.typ` error-box rendering. (D-007 .. D-008.) |
| `library-api.md` | Public Rust API surface (`Project`, `EvalOptions`, `EvalCallbacks`); how an embedder consumes the crate without clap. (D-023.) |
| `multi-file.md` | Discovery across `#include`, snippet-id resolution, single-cache-per-project model. (D-018.) |
| `package-api.md` | Typst-side public functions (`rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data`, `dep`, `setup`); metadata schema; sidecar layout. |
| `rbe-porter.md` | Design for the deterministic mechanical rust-by-example porter (`tools/rbe-port/`). Currently designed but not built; the active rbe path is the hand-port (see `docs/tracks/rust-by-example-port.md` § "How this differs"). |
| `schema-versioning.md` | How `<evcxr-snippet>` payloads version, the `setup(min-cli: …)` mechanism, the IncompatibleCliVersion contract. (D-019, D-026.) |
| `snippet-identity.md` | Blake3 → base32 id derivation, occurrence-index collision suffix, why ids are content-derived. |
| `snippet-semantics.md` | What evcxr does and doesn't preserve across snippets (items vs. expressions, `let` shadowing, panic recovery). |
| `wasm-plugin-analyzer.md` | Future direction: replacing the prequery scan with a Typst WASM plugin and/or rust-analyzer. Not on the critical path; designed for the Semantic Typst track (#19). |
| `watch-loop.md` | Notify watcher topology, change classification (Append/LeafEdit/Reset), debounce, plan execution. As of PR #27 + #28, the watch is recursive on `entry.parent()` and `entry` is canonicalized at watch start. |

## When to add a new file here

A new `design/<area>.md` is justified when:

1. The area is a self-contained mechanism that takes more than one paragraph to describe correctly.
2. Cross-references from at least two existing files would benefit (otherwise inline it).
3. There is a reasonable chance someone will want to *change* it later — design files anchor that conversation.

If your area doesn't meet those bars, add a section to an existing file or write a decision record (`docs/DECISIONS.md`) instead.
