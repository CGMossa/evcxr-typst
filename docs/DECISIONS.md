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

**Decision (proposed):** while building Phase 1 we use `evcxr = { path = "../../.evcxr/evcxr" }` (resolved from `crates/evcxr-typst/Cargo.toml`), pointing at the in-repo `.evcxr/` checkout (gitignored; `origin = CGMossa/evcxr`, `upstream = evcxr/evcxr`). Before we cut a release we pin to a published evcxr version and document that as the minimum.

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

**Note (T-I04):** `loc.doc_order` is populated via a Typst `counter("evcxr-doc-order")` shared by `<evcxr-snippet>` and `<evcxr-dep>` markers in `packages/evcxr/lib.typ`. The counter increments at render-time evaluation order, which matches Typst-source order for normal flow but could diverge under a `show` rule that reorders content. For literate-programming documents this edge case is negligible; if it bites, the schema can later add an explicit `loc.source_line` field (additive per D-019, no `v` bump required).

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

---

## D-012 — Package API names: `rust` / `rust-out` / `rust-display` / `rust-hidden` / `rust-data` / `dep`; `rust(...)` defaults to `show: "both"`

**Status:** accepted · 2026-05-06

**Decision:**
- Primary verb: **`rust`**.
- Stdout-only: **`rust-out`**.
- Display-only: **`rust-display`** (with `prefer:` kwarg to pick among PNG/SVG/HTML/JPEG when a snippet emits multiple display artifacts).
- Evaluate-and-render-nothing: **`rust-hidden`**.
- Parsed-data return: **`rust-data`**.
- Cargo dependency: **`dep(name, version, ..)`** — positional `(name, version?)`, plus kwargs `features:`, `default-features:`, `git:`, `path:`, `package:`. A single positional string containing `=` outside leading whitespace is treated as a TOML fragment and passed verbatim.
- Default `show:` for `rust(...)` is `"both"` (source + output). Configurable per-document via `setup(default-show: ...)`.
- Hyphens, not underscores, throughout.

**Rationale:** Validated against all 8 `.typ` files in `docs/design/examples/`. `rust` reads naturally in flow and matches the language tag; `rust-out` is brief enough to live inline ("The answer is #rust-out(...)"); `rust-display` matches evcxr's `EVCXR_BEGIN_CONTENT` vocabulary and avoids colliding with Typst's `show` rule; `rust-hidden` describes the rendering rather than guessing intent (covers both setup-style and intentionally-suppressed cases); `show: "both"` matches Jupyter-cell convention and matches every `#rust(...)` use in the gallery without needing per-call overrides; `dep(name, version)` is the canonical form already used by the gallery (`#dep("regex", "1")`).

**Consequences:** ARCHITECTURE.md's `<evcxr-snippet>.kind` enum lists exactly these five kinds. `rust-html` is *not* a separate function in v0; HTML output is one of the artifacts surfaced by `rust-display` via `prefer: "html"`. The `dep()` API is mildly overloaded (two-arg positional, kwargs, TOML escape hatch) but each form serves a distinct case. (Per-snippet `timeout:` kwarg later resolved by D-017 — shipped in v0 on every eval function.)

**Reference:** `docs/design/package-api.md` § 7 (resolved) and § 2; gallery `docs/design/examples/`.

---

## D-013 — `dep()` calls remain inline-anywhere; the CLI pre-collects in document order

**Status:** accepted · 2026-05-06

**Decision:** `#dep(...)` may be placed anywhere in the document — at the head, immediately before its consumer, or interleaved through chapters. Document order determines visibility. The CLI pre-collects all `<evcxr-dep>` markers during the `typst query` pass, validates that no two calls disagree on version (snippet-semantics G5; otherwise hard error), and emits `:dep` directives in document order before the corresponding snippets.

**Rationale:** Restricting `dep()` to a top-of-document prelude was considered. Rejected: gallery `e-cratesio-dep.typ` co-locates `#dep("regex", "1")` immediately above its consumer for narrative flow, and a long document benefits from declaring deps adjacent to the chapter that uses them. The CLI already needs to pre-collect deps to detect version conflicts, so allowing inline placement costs nothing operationally.

**Consequences:** Authors can place deps wherever reads best. CLI complexity is unchanged from the strawman: a single pre-collection pass over the metadata query result. Conflicting versions across `dep()` calls are a hard error with both call sites named.

**Reference:** `docs/design/package-api.md` § 4.2; `docs/design/snippet-semantics.md` § Rules.3 (resolved Q1).

---

## D-014 — Snippet-id attribution via a parallel offset map on the `evcxr-typst` side, not an upstream patch

**Status:** accepted · 2026-05-06

**Decision:** Mapping rustc/evcxr error spans back to a Typst snippet id is done by `evcxr-typst` maintaining its own `OffsetMap` keyed by submission order, not by extending evcxr's `CodeKind::OriginalUserCode`. The map records, for each snippet fed to `CommandContext::execute`, the `snippet_id`, the exact `src` bytes submitted, and the byte range within the submission buffer. Cross-snippet item attribution adds a `committed_items: HashMap<ItemName, SnippetId>` rebuilt as we feed snippets, used to attribute spans that land in evcxr's regenerated `items_code()` for re-attached items back to the snippet that originally committed each item.

**Rationale:** evcxr's `CodeKind` is `pub(crate)`; extending it requires a non-trivial upstream API change with its own design and review cycle. Because `evcxr-typst` feeds exactly one snippet per `execute()` call, it already knows the snippet id at submission time and can map back without any evcxr-side change. The local map is strictly less precise than a `CodeKind::OriginalUserCode { snippet_id }` would be — cross-snippet item attribution leans on a name-based hash-match — but is sufficient for the rendering shapes specified in `errors.md` § 4.

**Consequences:** No upstream patch in v0. If span-fidelity for cross-snippet errors becomes a real problem in practice, revisit and propose the upstream patch then. No new `T-Uxx` upstream task is created at this time.

**Reference:** `docs/design/errors.md` § 3 and § 6 (the `OffsetMap` structure); `errors.md` § 8.1 (resolved).

---

## D-015 — `rust-data()` returns `none` on snippet error, `fallback:` when not yet evaluated

**Status:** accepted · 2026-05-06

**Decision:** `rust-data()` has three distinct return modes:
- **Success** — the parsed JSON/CBOR dict or array.
- **No sidecar yet** (CLI hasn't run, or `--allow-eval` was off) — returns the user-supplied `fallback:` value (default `(:)`). This is the under-D-004 fallback path; it lets bare `typst compile` produce a clean PDF without forcing every call site to handle an option type.
- **Snippet errored** (`<id>.error.json` present) — returns `none`, *and* a sibling error box is emitted at a sibling location.

**Rationale:** Considered three options: (a) always `none` on absence-or-error, (b) sentinel `(error: true, message: "...")` dict, (c) hard-fail (Typst panic). Sentinel dicts silently propagate corrupt data into downstream Typst layout — a doc author writing `#stats.mean` in flow text would see a degenerate value rendered as if it were real. Hard-fail violates `errors.md` § 0 (a Rust failure must not abort the Typst render). Returning `none` for genuine errors forces callers to acknowledge failure (`if stats != none { … } else { ... }`), distinct from the "not yet evaluated" case which still wants a sensible default. Validated against gallery `h-mini-report.typ` § 3, which uses `rust-data` to drive a Typst table — under this decision, an authoring-time bare `typst compile` shows the table populated with empty/zero cells from `fallback`, while a real failure during a run shows an error box plus a clearly-marked `none` in downstream code paths.

**Consequences:** Two failure modes for `rust-data` to handle, but they are distinguishable. Downstream Typst code that does `#stats.mean` on a `none` value fails with a clear Typst error, which is the correct behaviour — the original Rust failure already produced a visible error box.

**Reference:** `docs/design/package-api.md` § 2.5; `docs/design/errors.md` § 4 and § 8.4 (resolved).

**Amendment (T-I04, 2026-05-08):** the single-call form `#let stats = rust-data(...)` is incompatible with Typst's type system: a function body that emits `metadata(...)<evcxr-snippet>` content cannot also return a value (content evaluation absorbs the would-be return value). The canonical API is therefore split: `rust-data(id, src, ...)` emits the metadata marker (returns nothing renderable); `rust-data-read(id:, format:, fallback:)` reads the `.cbor`/`.json` sidecar and returns the dict. Authors call them in pairs at the same `id`. Other `read`-shaped helpers (`_read-display`, `_read-html`) gate on the per-snippet `<id>.manifest.json` produced by the CLI to know which extensions exist. Implementation lives in `packages/evcxr/lib.typ` and `crates/evcxr-typst/src/eval.rs`.

---

## D-016 — Skip sidecar rename when materialized bytes are unchanged

**Status:** accepted · 2026-05-06

**Decision:** During the id-addressed view materialization step (`cache.md` § "Atomic-write strategy"), if the live target file already exists and its bytes equal the staged file's, drop the staged file and skip the `rename`. The CAS write itself (`cas/<XX>/<full-key>/`) is **always** performed — its presence is what marks a cache key as having been computed. Only the materialization to the live id-addressed view is conditional.

**Rationale:** `typst watch` listens (via `notify`) for changes to files reached by `read()`/`image()`/`json()`/etc., including our sidecars. A no-op edit (cosmetic whitespace, unrelated paragraph rewrite) that re-evaluates a snippet to byte-identical output would otherwise trigger a `rename` event, a `typst watch` re-render, and a visible flicker for no reason. One `stat` + one streaming compare per snippet per cycle is dwarfed by evcxr execution cost; the visible-flicker reduction is real in interactive sessions. Reconciles cleanly with `cache.md`'s atomic-write strategy: CAS atomicity is preserved (always written), only the view-rename is conditional.

**Consequences:** The materialization path adds a "compare-then-rename" branch. If the comparison disagrees (bytes differ), the original atomic-rename path applies unchanged. CAS-by-key behaviour, GC, hardlink-vs-copy choice, and all other cache.md guarantees are unaffected.

**Reference:** `docs/design/watch-loop.md` § 9 (resolved Q1); `docs/design/cache.md` § "Atomic-write strategy" (updated).

---

## D-017 — Per-snippet `timeout:` kwarg ships in v0; SIGKILL-only cancellation; per-snippet wins over the global flag

**Status:** accepted · 2026-05-06

**Decision:**
- Every eval-emitting package function — `rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data` — accepts a `timeout:` kwarg in v0. `dep()` does **not** accept it.
- Accepted forms: `auto` (default; defer to `--snippet-timeout`), `none` (disable for this snippet), a Typst `duration`, a bare integer (interpreted as seconds), or a `<int>(ms|s|min|h)` string. The package validates and emits an integer-millisecond value (or `null`/`"none"`) into `<evcxr-snippet>.options.timeout_ms`.
- **Per-snippet wins.** When `timeout:` is anything other than `auto`, the global `--snippet-timeout` does not apply — neither floor nor ceiling. `--no-snippet-timeout` only sets the global default; per-snippet still overrides.
- Cancellation is **SIGKILL-only**. evcxr's `ChildProcess` exposes one stop mechanism (`process_handle().kill()`), per `evcxr/src/child_process.rs`; there is no clean cancel signal in either the IPC protocol or the host runtime. On expiry: SIGKILL the host child, evcxr returns `Error::SubprocessTerminated` on the next call, fresh child spawns for the next snippet. `let` bindings from earlier snippets are lost (per D-011); items (`fn`, `struct`, `impl`, `mod`, `use`) are recompiled from `committed_state` and survive. Same shape as D-009.
- The `tokio::time::timeout` wrapper covers `CommandContext::execute()` end-to-end, but inside that call evcxr runs `cargo build` synchronously in *our* thread before dispatching to the host child. SIGKILL on `process_handle()` does not stop a running cargo invocation, so a snippet wedged in cargo (procedural macro looping, etc.) overshoots `timeout:` by the cargo runtime. Documented as a known floor in `package-api.md` § 2.8; matches D-009's behaviour.
- `dep()` resolution does not run inside a timed `execute()`. The global flag covers `:dep` work; per-snippet `timeout:` is intentionally not exposed on `dep()` because there is no per-`dep()` cargo-cancellation primitive to drive it.

**Rationale:** D-009 deferred this kwarg pending clarity on evcxr's child-cancellation semantics. The clarity now: there is no clean cancel; only SIGKILL. That answer is *not* a blocker — it's identical to what the global timeout already does. No additional mechanism is needed; we just parameterise the duration we already pass to `tokio::time::timeout`. The ergonomic value (long benchmarks, async runs, intentional infinite loops) is real and present in the gallery (`f-async-tokio.typ`, `h-mini-report.typ` heavy snippets). Per-snippet wins (vs. minimum-wins or maximum-wins) is the rule that least surprises an author who explicitly wrote `timeout:` to override; it also matches how every other per-call kwarg overrides `setup()` defaults in this package.

**Consequences:** The package's `<evcxr-snippet>.options` schema gains an optional `timeout_ms` field. The CLI's per-snippet driver reads `options.timeout_ms` and passes it to the wrapping `tokio::time::timeout`; absent or `null` falls back to `--snippet-timeout`. The `dep()` sub-cargo race is documented but not solved; a future upstream primitive (e.g. cargo cancellation, or evcxr-side `:dep` cancellation) would tighten it. No upstream evcxr patch is required for v0.

**Reference:** D-009 (deferral); `evcxr/src/child_process.rs` (kill mechanism); `evcxr/src/eval_context.rs` (execute path); `docs/design/package-api.md` § 2.8; `docs/design/errors.md` § 1.e.

---

## D-018 — Multi-file project model: single entry file, auto-discovered imports, cache rooted at entry-file parent

**Status:** accepted · 2026-05-06

**Decision:**
- **Project model.** A project has exactly one **entry file** in v0 (the `.typ` passed to `evcxr-typst run`). The entry file's parent directory is the **workspace / project root**. **Member files** are the transitive set of local `.typ` files reached from the entry by following `#import`/`#include` of local-path string literals. `@preview/` and `@local/` package imports are not followed in v0 (their snippets are not evaluated by `evcxr-typst`).
- **Single vs multi-entry.** Single-entry-file only in v0. Users with multiple entry files (e.g. `paper.typ` + `slides.typ` sharing `lib.typ`) run two `evcxr-typst` invocations side-by-side; the CAS is shared automatically. Multi-entry as a first-class mode is deferred to v1; the on-disk layout is forward-compatible (per-entry `index.<stem>.json` + per-entry `views/<stem>/` materialized view).
- **Discovery.** On every cycle: BFS from the entry file, parsing each member's source with `typst-syntax` to collect `ModuleImport` / `ModuleInclude` targets. An optional `evcxr-typst.toml` `[project] files = [...]` at the workspace root overrides discovery (escape hatch for dynamic imports, hermetic CI).
- **Global snippet ordering.** Snippets are flattened into a single global order `(file_seq, doc_order_within_file)`, where `file_seq` is BFS encounter order. Diamond imports visit each file once; the first import claims the slot. This global order feeds `prior_chain_hash`, `:dep` activation order, and the metadata's `loc.doc_order`.
- **Cache scope.** The cache lives at `<workspace>/.evcxr-typst-cache/v1/`, where `<workspace>` is the entry file's parent directory. The CAS is shared across entry files in the same workspace and across documents (free dedup). The id-addressed view (`index.json` + materialized `<id>.<ext>`) is per entry file. With one entry file, the layout collapses to ARCHITECTURE.md's original `<id>.<ext>` flat shape.
- **`dep()` visibility.** Global, document-order: a `#dep` is visible to every snippet later in global order, regardless of file boundaries. Conflict detection (D-013) names file paths in the error message.
- **ID collision rule.** Project-wide, not per-file. Default-ID collisions get the occurrence-index suffix (D-007); explicit-ID collisions are a hard error citing both source files.
- **Watch set.** Union of all member files plus their parent directories, recomputed by diffing prev/curr discovered sets after each successful query.

**Rationale:** The entry-file-as-identity model avoids forcing a manifest on users while keeping discovery deterministic. Rooting the cache at the entry-file parent matches the natural Typst project layout (project = directory) and keeps `cache.md`'s "workspace level" claim honest. CAS sharing falls out of D-010 unchanged. Single-entry v0 is the smallest design that handles the bulk of real projects (one paper, one report, one slide deck) while leaving a clean v1 path for shared-library projects.

**Consequences:** Discovery costs an extra `typst-syntax` parse per member file per cycle. Cheap; absorbed by the cache. Users with imports we can't statically resolve fall back to the TOML override. The package side of v0 doesn't need to know which entry file it's being rendered for — there's only one. That simplification disappears in v1. Open: verify `typst query` location output to see if it already reports source-file paths (would simplify discovery; tracked in `multi-file.md` § 9 Q1).

**Reference:** `docs/design/multi-file.md`; `docs/design/cache.md` § "Cache layout on disk"; `docs/design/watch-loop.md` § 9 Q2 (resolved); D-013 (`dep()` ordering); D-007 (ID collisions).

---

## D-019 — Schema versioning policy: per-interface `v`, `min-cli` declared in `setup()`, side-by-side cache migrations

**Status:** accepted · 2026-05-06

**Decision:**

- Four independent `v` fields — `<evcxr-snippet>.v`, `<evcxr-dep>.v`, `<id>.error.json.v`, and the on-disk cache layout (`v1/`) — all currently at `1`. Each bumps **major-breaking-only**: rename / remove / type-change of an existing field. Adding optional fields, new enum variants in `kind`/`phase`, or new keys inside `options` does not bump.
- The CLI semver and the Typst package semver evolve independently of the four `v` fields.
- **Forward compatibility**: older readers ignore unknown additive fields (documented promise for `options` and equivalents). Unfamiliar `v` values are a hard error, not best-effort parsing.
- **Backward compatibility**: each `v: N` reader is required to also accept `v: N-1`. Older than that requires regenerating sidecars.
- **Min-CLI mechanism**: the Typst package declares `min-cli: "X.Y.Z"` as a kwarg on its `setup()` call. The package emits a top-level `<evcxr-min-cli>` metadata marker; the CLI reads it during `typst query` and exits with code `2` and a clear "upgrade evcxr-typst or pin the package" message if its own `CARGO_PKG_VERSION` is below the requirement. The package never tries to detect the CLI; it only declares.
- **Min-package mechanism**: none. Asymmetric on purpose — the CLI is what the user actively chose; the package is a transitive dependency. Missing-feature warnings are advisory at most.
- **Cache layout migration**: side-by-side. When the CLI bumps from `v1/` to `v2/`, it creates `v2/` and proceeds; `v1/` is preserved (effectively a cold cache for one run). `evcxr-typst clean --layout v1` removes a specific older layout; auto-deletion is rejected as a foot-gun. Downgrade-and-re-run continues to work.
- **Unknown-`v` rendering on the package side**: an `_evcxr-error-box` (per `errors.md` § 4) with header label `schema mismatch` and body advising the user to upgrade `@preview/evcxr` or downgrade `evcxr-typst`. The box replaces the snippet output, identical to other error boxes.

**Rationale:** The four versioned interfaces are independent in practice (a CLI release can change `<id>.error.json` without touching `<evcxr-snippet>`), so coupling them under one number would force needless re-renders. Major-breaking-only keeps `v` rare and meaningful — versioned interfaces with a chatty `v` field encourage readers to gate on exact match, which defeats forward compat. `min-cli` in `setup()` was preferred over a standalone `<evcxr-min-cli>` marker (forces every doc to add boilerplate) and over a runtime check function (easy to forget to call); folding it into the already-recommended `setup()` is the lightest viable surface. Side-by-side cache migration trades a one-time cold cache for never-corrupt downgrades and matches how `cargo` handles target-dir layout bumps. The asymmetric "no min-package" choice keeps the CLI authoritative; users curate the binary, not transitive packages.

**Consequences:** Each schema doc (`package-api.md` § 5, `errors.md` § 2, `cache.md` § "Cache layout") gains a one-line link to `schema-versioning.md` rather than restating the policy. The package picks up `evcxr.max-supported-error-v` as a constant for the unknown-`v` error message. CLIs supporting older packages must keep older `v: N-1` writers around for one major — bounded cost. Releases now have a checklist item: bump the relevant `v`, update `min-cli:` if applicable, log the cache migration in release notes.

**Reference:** `docs/design/schema-versioning.md` (canonical policy, all seven question areas covered).

---

## D-020 — Semantic features arrive via CLI sidecars first; WASM plugin is a deferred superset

**Status:** accepted · 2026-05-06

**Decision:** The "semantic Typst" feature set (`type-of`, `signature-of`, `kind-of`, `doc-of`, `items-table`, `ref`, `diagnostics-of`) ships first via CLI-emitted `<id>.semantic.cbor` sidecars consumed by the Typst package, exactly the same plumbing pattern as `rust-data` (D-015). The WASM-plugin path (`crates/evcxr-typst-analyzer/`) — analysed in `docs/design/wasm-plugin-analyzer.md` — is a strict superset that brings the same features to bare `typst compile` (no CLI run); it is **deferred** to side-track phase S4 and only revisited after S1–S3 have shipped and we have data on how often authors hit the "no CLI run yet, want semantic" case.

**Rationale:** evcxr's `CommandContext` already wraps a `RustAnalyzer` for its own type-inference and completion paths (`evcxr/src/rust_analyzer.rs`). The data needed for S1–S3 is largely already computed; we surface it. That's a small, low-risk slice that delivers most of the user-visible value of the track. The WASM-plugin path adds a multi-MB binary, fork maintenance, a stdlib summary build pipeline, and a fifth versioned interface — non-trivial cost for an incremental UX bump on a fallback path. Re-validating the cost/value once S1–S3 ship is honest; committing now would over-invest.

**Consequences:** The semantic-typst track is now formally a side track (`docs/tracks/semantic-typst.md`), with tasks T-S01..T-S04 in the side-tracks section of `BACKLOG.md`. T-D11 in the main backlog is rewritten to point at S4 specifically. The `<id>.semantic.cbor` sidecar is added as the fifth versioned interface in `docs/design/schema-versioning.md` when S1 ships.

**Reference:** `docs/tracks/semantic-typst.md`; `docs/design/wasm-plugin-analyzer.md`; D-015 (sidecar fallback model precedent); D-019 (schema versioning policy).

---

## D-021 — Rename `show:` kwarg to `render:` (amends D-012; Typst reserves `show`)

**Status:** accepted · 2026-05-06

**Decision:**
- The kwarg controlling what `rust(...)` displays in the rendered document is renamed `show:` → `render:`. Values unchanged: `auto` / `"source"` / `"output"` / `"both"`.
- The corresponding `setup()` document-wide default is renamed `default-show:` → `default-render:` for consistency (the standalone `default-show` identifier was not actually reserved, but mirroring the per-call kwarg name keeps the API memorable).
- The metadata-schema field at `<evcxr-snippet>.options.show` is renamed to `<evcxr-snippet>.options.render` to mirror the kwarg name.
- The `dep(..., show: ...)` kwarg (per package-api.md § 3, controlling whether a `dep` call renders a "depends on:" tag) is renamed `show:` → `render:` for the same reason. Boolean values unchanged.

**Rationale:** Typst rejects `show` as a function-parameter identifier — it's a reserved keyword (the `show` rule selector). Discovered while smoke-testing `typst compile examples/hello/main.typ` during T-H01 cleanup (T-H03 finding). Candidates considered: `display` (clashes with the `rust-display()` function name — same word, different scope, predicted to confuse readers), `output` (overlaps with the kwarg's *value* `"output"`), `mode` (too generic — doesn't say what's being moded), `show_` (Typst's mechanical workaround — ugly trailing underscore, not idiomatic), `view` (reads weirdly as `view: "both"`), `parts` (close, but doesn't say *what is rendered*). `render:` won: describes the action precisely, no Typst-keyword collision, no overlap with any package function name, parses naturally with every value (`render: "both"`, `render: "source"`, `render: auto`).

**Consequences:** `packages/evcxr/lib.typ` updated. `docs/design/package-api.md` updated wholesale (every `show:` references this kwarg, except where it already uses `default-show:` in setup which is also renamed). D-012's title and decision text reference `show: "both"`; left in place per the append-only DECISIONS convention — this entry is the amendment of record. The metadata-schema rename does not require a `<evcxr-snippet>.v` bump per D-019 (the schema isn't shipped; we're amending pre-1.0). Future schema bumps governed by D-019 normally.

**Reference:** D-012 (the original names decision); T-H03 in `BACKLOG.md` (the bug report); `packages/evcxr/lib.typ`.

---

## D-022 — Rust-by-example port lives in `examples/rust-by-example/` as a phased side track; `rust-main` package convenience added

**Status:** accepted · 2026-05-06

**Decision:**
- The rust-by-example port is a side track per `docs/tracks/README.md` policy: off main critical path, never blocks Phase 1–4. Designed in `docs/tracks/rust-by-example-port.md`.
- Output lives at `examples/rust-by-example/<chapter-path>.typ` mirroring upstream `SUMMARY.md` structure. A top-level `examples/rust-by-example/main.typ` `#include`s the per-chapter files in SUMMARY order. Multi-file project model per D-018 applies: one entry file, single workspace cache.
- A new package convenience `evcxr.rust-main(snippet, ..)` is introduced. It accepts a snippet whose source contains a `fn main() { … }` definition; the CLI defines everything in the snippet *and* synthesises a trailing `main();` invocation (recorded in `<evcxr-snippet>.options.auto-call = "main"`, not shown in the rendered source). The rendered Typst source is the unmodified rust-by-example snippet — faithful to upstream — and the captured stdout below it is the result of `main()` being called. Adding `auto-call` to `options` is additive per D-019; no schema-version bump.
- Conversion is performed by a workspace tool `tools/rbe-port/` (Rust binary using `pulldown-cmark` + `syn`). Deterministic: same input bytes → same output bytes. Drift detection via a `manifest.json` capturing the input commit SHA and per-file SHA-256.
- The `.rust-by-example/` source-mdBook checkout is not vendored into the repo. Stays gitignored. The porter reads from a configurable input path. Rejected alternatives: `git submodule` (transitive complexity), in-tree vendor (bloat + merge mess on upstream updates).
- License/attribution: rust-by-example is dual MIT/Apache-2.0, same as evcxr-typst; licenses are compatible. Required: `examples/rust-by-example/NOTICES.md` documents upstream license, repo, and the commit SHA the port is based on; each per-chapter `.typ` carries an auto-generated `// Adapted from rust-by-example/<src>.md` header; the top-level `main.typ` includes a banner pointing at `NOTICES.md`.
- Phasing: B0 (tooling + license scaffolding) → B1 (Hello / Primitives / Custom Types, ~15 chapters) → B2 (variable_bindings / types / conversion / expression / flow_control, ~30) → B3 (fn / mod / crates / cargo, ~15) → B4 (attribute / generics / scope / trait, ~25) → B5 (error, ~15) → B6 (std / std_misc / testing / unsafe / compatibility / meta, ~40). v0 deliverable = B0 + B1 + B2.

**Rationale:** Rust-by-example is the canonical "varied, idiomatic Rust prose-and-code" corpus. Successfully porting it validates the integration end-to-end against material we did not author — a much stronger demonstration than the eight-doc gallery in `docs/design/examples/`. The deterministic script-based porter (rather than hand-conversion) keeps the work tractable across 198 files and gives us a clean re-port story when upstream changes. `rust-main` is the smallest package change that lets us stay faithful to upstream's ubiquitous `fn main() { … }` framing without splitting every chapter into define-and-call snippets. The not-vendored, not-submoduled stance keeps repo hygiene clean while still being reproducible (manifest captures the SHA).

**Consequences:** New side-track tasks T-B00..T-B06 in `docs/BACKLOG.md`. New `tools/rbe-port/` workspace member. New `examples/rust-by-example/` output tree (gitignored or partially committed; phase choice). New `examples/rust-by-example/NOTICES.md` once B0 ships. Package gains `rust-main` once B0's spec lands; minor `lib.typ` addition. Schema's `options.auto-call` field added (additive). The `:cache` budget recommendation may need bumping to absorb the dep-heavy chapters in B6.

**Reference:** `docs/tracks/rust-by-example-port.md`; `docs/design/multi-file.md` (D-018); `docs/design/package-api.md` (where `rust-main` will be added); D-013 (`dep()` ordering); D-019 (schema versioning).

---

## D-023 — `evcxr-typst` ships a public library API; binary is a thin wrapper

**Status:** accepted · 2026-05-06

**Decision:**
- `crates/evcxr-typst/` carries both `lib.rs` (public library) and `main.rs` (CLI wrapper) in the same crate. One crate to crates.io named `evcxr-typst`. `cargo install evcxr-typst` ships the binary; `evcxr-typst = "X.Y"` in another crate's `Cargo.toml` ships the library.
- The library exposes `Project::open / evaluate / watch / clean_view`, `EvalOptions`, an `EvalCallbacks` trait for snippet-lifecycle hooks, and a typed `Error` enum (using `thiserror`, not `anyhow`).
- `runtime_hook()` is the embedder's responsibility — must be called first thing in `main()`. Library functions never call it. Documented prominently in the crate root and in every example.
- Library API is sync. Async at the library boundary is rejected for v0 — the watch loop uses `notify` + a thread; the eval boundary blocks; consumers spawn their own runtime if they want non-blocking. `tokio::time::timeout` per D-017 is implementation detail, not exposed.
- API is unstable pre-1.0. SemVer-minor bumps may break compile-time. Documented in the crate's docs.rs landing page.
- Re-evaluating a single snippet (`Project::evaluate_one`) is **deferred** to a real consumer asking — watch already does fine-grained re-eval internally; no public API surface yet.
- A separate `evcxr-typst-core` library crate (rejected): would force two crates to publish, version, and document. The single-crate path keeps shipping cheap; we revisit only if the binary's deps grow heavy enough to want isolation.

**Rationale:** the library API is approximately the binary minus runtime_hook + clap + eprintln-reporting. Structuring the crate that way from the start (i.e. before T-I03 lands real eval logic) avoids the trap where helpers accrete in private functions and never become `pub`. evcxr's own precedent is the `EvalContext` library + `evcxr_repl` / `evcxr_jupyter` binary embedders pattern, which scales — we follow it. Sync-by-default at the library boundary matches evcxr (`EvalContext::eval` is sync) and avoids forcing tokio on every consumer for negligible benefit.

**Consequences:** T-I01's scaffolding needs minor refactor (move clap parsing into a `cli` module, expose the eval pipeline via `lib.rs`) — handled as part of T-L01 below. `crates/evcxr-typst/examples/library_use.rs` ships as the canonical embedder example, mirroring evcxr's `example_eval.rs`. `thiserror` becomes a library dependency (small).

**Reference:** `docs/design/library-api.md`; `.evcxr/evcxr/examples/example_eval.rs` (precedent); D-004 (allow-eval safety surfaces in `EvalOptions::deny() / ::allow_eval()`); D-017 (timeout, hidden behind `EvalOptions::with_snippet_timeout`).

---

## D-024 — `tools/rbe-port/` uses a hand-written scanner + `syn` (not `pulldown-cmark`)

**Status:** accepted · 2026-05-06

**Decision:**
- The rust-by-example porter is implemented as a hand-written line/state-machine scanner for markdown, plus `syn` for accurate `fn main()` detection in fenced Rust blocks. **No** `pulldown-cmark` / `comrak` / `markdown-rs` dependency.
- Markdown features handled: fenced code blocks (the load-bearing case), headings, paragraphs, emphasis, inline links, mdBook ref-style links (`[text][key]` ... `[key]: url`), lists. Anything more exotic falls back to verbatim pass-through.
- Crate is workspace-isolated: `tools/rbe-port/Cargo.toml` ends with `[workspace]` to exclude itself from the parent `evcxr-typst` workspace, keeping tooling-only deps out of the main lockfile.
- Determinism is a contract: same input bytes → byte-identical output. Enforced via golden tests under `tools/rbe-port/tests/golden/<case>/{input.md,expected.typ}` with literal expected files (no snapshot library — diffs are eyeball-reviewed in PRs). Required golden cases listed in `docs/design/rbe-porter.md` § "Required golden cases".
- `--check` mode re-converts and diffs against on-disk output; CI runs this against a vendored input snapshot to detect drift.
- Manifest format (`<output-dir>/manifest.json`): captures `rbe_commit_sha`, `ported_at`, per-file SHA-256 of input + output, snippet-kind detected.
- Code-block tag mapping per `docs/design/rbe-porter.md` § "Code-block tag matrix": `rust`/`rust,editable` → snippet-detection; `rust,ignore`/`rust,no_run` → `options.skip-eval`; `rust,compile_fail` → `options.expected-error`; `rust,should_panic` → `options.expected-panic`; `text`/`bash`/`sh`/`toml` → Typst `#raw(block: true, ...)`. New `options` keys are additive per D-019 — no schema-version bump.
- Snippet-kind detection (`SyncMain`, `AsyncMain`, `AsyncRuntimeMain`, `Plain`, `MultipleMain`, `Unparseable`) maps to `rust-main` / `rust` per `rbe-porter.md` § "Snippet detection". `#[tokio::main]` snippets are emitted verbatim — evcxr's auto-tokio handles them at eval time.

**Rationale:** rust-by-example markdown is straightforward — fenced code blocks plus light prose syntax. A markdown AST library would force enumerating dozens of event types we never use, and the round-trip "events back to markdown then to typst" is its own bug surface. The scanner is estimated 200–400 lines of focused code. `syn` IS load-bearing for snippet detection (no regex hack survives `#[tokio::main] async fn main()`); we don't substitute it. Workspace isolation prevents tooling deps from polluting `evcxr-typst`'s lockfile, matching the pattern proposed for `crates/evcxr-typst-analyzer/` in `docs/design/wasm-plugin-analyzer.md` § "Mechanism". Literal golden files over snapshot testing trades convenience for review discipline — `INSTA_UPDATE=1` invites unreviewed drift.

**Consequences:** T-B01's "Done when" expands to include a working scanner + the 10 required golden cases. `pulldown-cmark` and friends remain documented escalation paths if the scanner proves insufficient on real chapters. New `options` keys (`skip-eval`, `expected-error`, `expected-panic`, `auto-call`, `auto-call-await`) get added to `docs/design/package-api.md` § 5.1 when T-B01 ships; CLI must honor them in T-I03 onward.

**Reference:** `docs/design/rbe-porter.md`; `docs/tracks/rust-by-example-port.md`; D-022 (track-level decision); D-019 (additive `options` policy).

---

## D-025 — Pending evcxr improvements live on a personal fork; switch to upstream when merged

**Status:** accepted · 2026-05-07

**Decision:** Patches and improvements to evcxr that arose from `evcxr-typst` work (or were drive-by while exploring it) are staged on the user's personal fork at <https://github.com/CGMossa/evcxr>, one feature branch + fork-PR per concern. They are **not** open against `evcxr/evcxr` upstream right now. When `evcxr-typst` development needs a not-yet-upstream change, depend on the appropriate fork branch (per D-006: still a path or git dep during dev, switch to crates.io once upstream releases include the change).

Open fork PRs (CGMossa/evcxr#1..#9), each on a dedicated branch:

| Fork PR | Upstream issue | What it adds |
|---|---|---|
| #1 | evcxr/evcxr#232 | preserve MIME-type parameters (`text/markdown; charset=utf-8`) — relevant to our display-output passthrough |
| #2 | evcxr/evcxr#393 | `--config <PATH>` flag on the REPL for a custom init file |
| #3 | evcxr/evcxr#376 | `:features` command — enable cargo features on the generated crate |
| #4 | evcxr/evcxr#280, #281 | Jupyter `kernel_info_reply` / `execute_reply` spec fixes (out-of-scope for us, but it's there) |
| #5 | evcxr/evcxr#370 | `:patch` command — `[patch.crates-io]` entries in the generated `Cargo.toml`. Useful for `evcxr-typst` users patching deps |
| #6 | evcxr/evcxr#374 | tab completion for `:doc <prefix>` (REPL-only) |
| #7 | evcxr/evcxr#188 | `--script <FILE>` + `--exit-after-script` flags on the REPL |
| #8 | evcxr/evcxr#238 | surface compiler warnings in REPL output. Relevant: `evcxr-typst` may want to render rustc warnings in fallback-eval output too — once this lands, `EvalOutputs.warnings` becomes a public Vec |
| #9 | evcxr/evcxr#428 | persist a tombstone for moved variables so bare reassignment in a later snippet rewrites to `let mut`. Affects D-011's "panics reset child" semantics tangentially — only the move-then-reassign case |

**Rationale:** The user has READ-only permission on `evcxr/evcxr`. A burst of eight fork-targeted-but-mistakenly-upstream PRs landed on the upstream queue and were closed; those branches were re-pushed to fork-PRs against `CGMossa/evcxr` `main` so we can iterate without bothering the maintainer. The fork-PR gate also lets us validate each change against `evcxr-typst`'s actual usage before proposing upstream.

**Consequences:**
- D-006's "path during dev" expands to "path against `../evcxr/` (which may be on a fork branch checkout) during dev". A library consumer of `evcxr-typst` does **not** see fork branches because we publish via crates.io once a baseline is picked.
- Two of these PRs are likely load-bearing for `evcxr-typst` features once the relevant tasks start: #8 (warnings) for fallback-eval pretty rendering (T-I07), and #5 (patch) as a user-facing convenience users may expect when they hit a transitive-dep bug (relevant to `dep()` ergonomics, T-D01/T-D03 already shipped — this is a follow-up consideration).
- When a fork PR upstreams, this entry should be amended (or superseded) to drop that row from the table. When all rows clear, supersede with "all merged upstream" and remove the table.
- We do **not** vendor or fork evcxr in this repo (D-002 still holds). The fork is a staging ground, not a substitute upstream.

**Reference:** D-002 (separate repo, evcxr is a dependency); D-006 (path during dev, crates.io for release); evcxr's CLAUDE.md "read-only reference workspace" framing; conversation-derived memory at `~/.claude/projects/-Users-elea-Documents-GitHub-evcxr/memory/feedback_iterate_on_fork_first.md`.
