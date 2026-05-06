# Decisions

ADR-lite log. Append-only. Each entry: status (proposed | accepted | superseded), date, decision, rationale, consequences. New entries get appended to the bottom; do not edit older ones in place — supersede them with a new entry.

---

## D-001 — Use the prequery pattern, not a Typst WASM plugin

**Status:** accepted · 2026-05-06

**Decision:** evcxr is invoked from an external CLI that runs alongside `typst compile`/`typst watch`, communicating via on-disk sidecar files. We do **not** ship evcxr-as-a-Typst-plugin.

**Rationale:** Typst plugins are sandboxed WASM with no syscalls, no filesystem, no subprocess spawning. evcxr is fundamentally a subprocess manager (rustc/cargo wrapper plus a long-lived child process loading cdylibs via libloading). The two are architecturally incompatible. Prequery is the established pattern for "Typst document needs work done in the outside world" and matches our case exactly.

**Consequences:** users need to run a second tool (`evcxr-typst run`) in addition to `typst compile`. We get full host-system capabilities. Documents using the package remain renderable with bare `typst compile` thanks to fallback rendering (D-004).

---

## D-002 — Separate repository, evcxr as a dependency

**Status:** accepted · 2026-05-06

**Decision:** This work lives in its own repository (`evcxr-typst/`), not as a crate inside the evcxr workspace. evcxr is a dependency, treated as read-only upstream.

**Rationale:** Keeps evcxr's CI / dep graph / release cadence clean; lets us iterate on the integration without coupling. Patches that need to land in evcxr go upstream.

**Consequences:** local dev uses a path dependency to the evcxr clone; published builds will use crates.io. Need a clear "minimum supported evcxr version" once we ship.

---

## D-003 — Linear re-evaluation on middle-of-document edits, for v0

**Status:** accepted · 2026-05-06

**Decision:** When a snippet earlier than the last one changes, the CLI resets `CommandContext` and re-evaluates from the first changed snippet onward. We do **not** implement a snapshot/restore mechanism in v0.

**Rationale:** evcxr's `committed_state` is forward-only. Adding snapshot/restore is a non-trivial upstream change because state lives in the host child process, not just in evcxr's library state. Rustc artifact caching (`:cache`) makes re-eval much cheaper than it sounds — most of the cost of "re-eval from scratch" is paid in linker time, which the cache avoids.

**Consequences:** middle-edits feel slower than end-edits in watch mode. We measure before optimizing. If editing-in-the-middle becomes the dominant UX, revisit and propose snapshot/restore upstream in evcxr.

---

## D-004 — Fallback rendering by default; evaluation is opt-in

**Status:** accepted · 2026-05-06

**Decision:** A document using our Typst package must compile (with placeholder boxes) under bare `typst compile`. Actually executing Rust code requires `evcxr-typst run --allow-eval` (the `--allow-eval` flag is mandatory and not the default).

**Rationale:** A `.typ` file embedding executable Rust is a code-execution vector. We accept the convenience tradeoff but require explicit, informed opt-in. Mirrors `prequery`'s model.

**Consequences:** the package needs a fallback path that doesn't depend on sidecars existing. CLI is more verbose to invoke. Worth it.

---

## D-005 — Stable snippet IDs default to a content hash; explicit IDs override

**Status:** proposed · 2026-05-06

**Decision (proposed):** `id = explicit_id_or(blake3(src)[:12])`. `loc.doc_order` is tracked separately for ordering. Whitespace/comment insensitivity in the hash is **not** in scope for v0 (a future tweak if it pays off).

**Rationale:** content hash gives stability across unrelated edits, which is what cache-hit rates care about. Explicit override gives the user a way to keep an ID stable when they're consciously editing a snippet.

**Consequences:** identical Rust source in two snippets collides on default ID — we either disambiguate by appending `loc.doc_order`, or document this. Open in `docs/design/snippet-identity.md`.

---

## D-006 — evcxr dependency: path during dev, crates.io once a baseline is picked

**Status:** proposed · 2026-05-06

**Decision (proposed):** while building Phase 1 we use `evcxr = { path = "../evcxr/evcxr" }`. Before we cut a release we pin to a published evcxr version and document that as the minimum.

**Rationale:** evcxr's API may need small adjustments (e.g. better hooks for capturing display output); easier to iterate against a local checkout. But shipping `evcxr-typst` to crates.io requires a published baseline.

**Consequences:** a one-time dependency change at release time. Document the required evcxr version in `crates/evcxr-typst/Cargo.toml` and the README.
