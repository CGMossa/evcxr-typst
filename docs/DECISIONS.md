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

**Status:** superseded by D-007 · 2026-05-06

**Decision (proposed):** `id = explicit_id_or(blake3(src)[:12])`. `loc.doc_order` is tracked separately for ordering. Whitespace/comment insensitivity in the hash is **not** in scope for v0 (a future tweak if it pays off).

**Rationale:** content hash gives stability across unrelated edits, which is what cache-hit rates care about. Explicit override gives the user a way to keep an ID stable when they're consciously editing a snippet.

**Consequences:** identical Rust source in two snippets collides on default ID — we either disambiguate by appending `loc.doc_order`, or document this. Open in `docs/design/snippet-identity.md`.

---

## D-006 — evcxr dependency: path during dev, crates.io once a baseline is picked

**Status:** proposed · 2026-05-06

**Decision (proposed):** while building Phase 1 we use `evcxr = { path = "../evcxr/evcxr" }`. Before we cut a release we pin to a published evcxr version and document that as the minimum.

**Rationale:** evcxr's API may need small adjustments (e.g. better hooks for capturing display output); easier to iterate against a local checkout. But shipping `evcxr-typst` to crates.io requires a published baseline.

**Consequences:** a one-time dependency change at release time. Document the required evcxr version in `crates/evcxr-typst/Cargo.toml` and the README.

---

## D-007 — Snippet ID = `blake3(src)` base32, 12 chars, with occurrence-index suffix on collision (supersedes D-005)

**Status:** accepted · 2026-05-06

**Decision:**
- Default ID = `base32_lower(blake3(snippet_src_bytes))[..12]`. RFC 4648 alphabet, no padding, lowercase.
- Explicit override via `id:` on the package call. Validation: `[a-z0-9_-]{1,64}`, no reserved prefix (`_`, `evcxr-`, or default-ID-shape).
- Collisions among default IDs disambiguated by occurrence-index suffix (`xyz`, `xyz-1`, `xyz-2`, …). Collisions among explicit IDs are a hard error. An explicit ID that collides with a default ID wins; the default-bearer gets the suffix.
- The ID is *only* a stable name. Toolchain/dependency identity lives in the cache key, not the ID. See `docs/design/cache.md`.
- Whitespace/comment normalization is **not** in scope for v0; raw-bytes hashing is the v0 behavior.

**Rationale:** BLAKE3 is fast/modern and `evcxr-typst` already pulls a hashing dep transitively. 12 base32 chars are filesystem-safe across macOS/Windows/Linux, case-insensitive-safe, and short enough to read in a directory listing while leaving 60 bits of entropy. Occurrence-index suffixing keeps duplicate-snippet suffixes stable across unrelated paragraph edits — `doc_order` would shift on every insertion.

**Consequences:** Whitespace-sensitive hashing means trivial reformatting busts the snippet-output cache (but not its neighbours; rustc artifact cache absorbs most of the cost). The CLI must run a one-pass collision-resolver after `typst query`, before evaluation.

---

## D-008 — File-based modules (`mod foo;`) are not supported; inline `mod foo { … }` is the canonical form

**Status:** accepted · 2026-05-06

**Decision:** Reject `mod foo;` (file-on-disk reference) at the CLI side with a clear error pointing the user at inline modules or `:dep`. Inline `mod foo { … }` works and composes across snippets normally.

**Rationale:** `mod foo;` resolves relative to evcxr's ephemeral `crate_dir()/src/`, which is a tmpdir, not the user's `.typ` file's directory. Even if we resolved paths to the document's directory, the file's contents wouldn't participate in snippet identity / cache invalidation, leading to silent staleness. Inline modules avoid all of this and cover the legitimate need.

**Consequences:** Users porting existing multi-file Rust projects must inline or pull as a `:dep`. Error message must be informative.

**Reference:** `docs/design/snippet-semantics.md` § "mod foo; (file-based modules)".

---

## D-009 — Snippet timeout = 30s default, configurable

**Status:** accepted · 2026-05-06

**Decision:** Each snippet's `CommandContext::execute` is wrapped in a `tokio::time::timeout` with a 30-second default. Configurable via `--snippet-timeout 60s` on the CLI (and `--no-snippet-timeout` to disable). Per-snippet override via Typst-side `rust(..., timeout: 5min)` is **deferred** (depends on T-D03's package API supporting that kwarg; flagged as RECON-T-D03 in `docs/design/errors.md`).

**Rationale:** Without a timeout, an infinite loop in any snippet hangs `evcxr-typst run` indefinitely with no signal. 30s is generous for normal interactive snippets and short enough that CI surfaces the problem quickly.

**Consequences:** On expiry: SIGKILL the host child, record a `phase: "timeout"` error in the snippet's `<id>.error.json`, evcxr respawns the child fresh — meaning all `let` bindings from earlier snippets are lost (see D-011). Users running long batch computations need to set the flag.

**Reference:** `docs/design/errors.md` § "1.e Timeout".

---

## D-010 — Snippet-output cache uses content-addressed storage with a separate id-addressed view

**Status:** accepted · 2026-05-06

**Decision:** Per-snippet output cache lives at `.evcxr-typst-cache/v1/`. Two layers: (a) a content-addressed store `cas/<XX>/<full-cache-key>/` keyed by the cache-key formula in `docs/design/cache.md`; (b) a materialized id-addressed view (hardlinks or copies) for the Typst package to read at render time. The package never sees the CAS.

The cache key formula (formal version in `docs/design/cache.md`) hashes: snippet src, prior-snippet Merkle chain, active deps, evcxr version, rustc version, target triple, allowlisted env vars.

The cache directory sits at the workspace level (alongside the `.typ` source), gitignored by default. CAS-by-key gives us free deduplication across documents, easy GC (`evcxr-typst clean` = drop CAS dirs not referenced by any `index.json`), and rename-stability (changing an explicit ID just rewrites `index.json`).

**Rationale:** ARCHITECTURE.md's original sketch (`<id>.{txt,png,…}`) conflated identity and validity. Splitting them is what lets a cache hit survive a snippet rename, and lets identical bytes computed in two different documents share storage.

**Consequences:** Implementation cost a bit higher (atomic-rename staging, two-char fan-out for FAT-family FSs, hardlink-or-copy materialization). We accept this; cache correctness is load-bearing for watch-mode UX.

**Reference:** `docs/design/cache.md` § "Cache layout on disk".

---

## D-011 — A snippet that panics resets evcxr's child; persisted `let` bindings are lost

**Status:** accepted · 2026-05-06

**Decision:** Document this prominently. A snippet that panics (or aborts, or segfaults, or hits the snippet timeout) kills the host child process; evcxr will respawn a fresh child for the next snippet. All `let`-bindings established before the offending snippet are gone from the new child's state. Item definitions (`fn`, `struct`, `impl`, `mod`, `use`) are recompiled from `committed_state` and survive the respawn — only runtime variable values are lost. Surface this with a banner across the rendered document run output and a per-affected-snippet sub-warning.

**Rationale:** This is evcxr's existing behavior, not something we can change. Pretending it doesn't happen would silently produce wrong output (a downstream snippet sees `let`s that don't actually exist).

**Consequences:** ARCHITECTURE.md's "Composition across snippets" table needed a caveat (added in the same commit as this entry). The watch loop's "leaf snippet modified" optimization must not apply if the previous run had any panic — replay from the panic point.

**Reference:** `docs/design/errors.md` § "1.c Runtime panic" and the contradiction flag at line 58.
