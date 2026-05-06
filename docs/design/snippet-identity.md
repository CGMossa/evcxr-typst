# Snippet identity

> **Scope**: how a Typst snippet acquires a stable identifier that the CLI uses to name sidecar files and look up the snippet-output cache. This document is the source of truth for the ID layer; the cache layer is in [`cache.md`](./cache.md).
>
> **Status**: design (Phase 0, T-D04). Supersedes the working assumption in `ARCHITECTURE.md` § "Snippet identity" only on points called out below; otherwise refines it.

## TL;DR

```
default_id = base32_lower(blake3(src_bytes))[..12]
final_id   = explicit_id_if_provided else default_id [+ "-" + doc_order_if_collision]
```

- Hash function: **BLAKE3** of the snippet source bytes (UTF-8, exactly as Typst handed them to us via the metadata `src` field).
- Length: **12 base32 chars** (≈ 60 bits). Encoding: `base32` lowercase, RFC 4648 alphabet without padding (`abcdefghijklmnopqrstuvwxyz234567`). Filesystem-safe on every target, case-insensitive (matters on macOS/Windows default FSes), and shorter than hex for the same entropy.
- 12 chars / 60 bits gives a 50% birthday-collision floor at ≈ 2^30 ≈ 1B distinct snippets — comfortably out of reach for any one document.
- "Real" collisions (byte-identical sources) are common in practice (`println!("ok")` twice) and are handled deterministically — see below.

## Inputs to the default ID — and what is *deliberately* excluded

| Input | Included? | Why / why not |
|---|---|---|
| Snippet source bytes (`src`) | **yes** | This is the whole point. |
| Whitespace, indentation, trailing newlines inside `src` | **yes** (v0) | Punting normalization avoids a Rust tokenizer dependency. Trade-off: re-indenting a snippet changes its ID and busts its cache entry. Documented as a known sharp edge. See "Whitespace sensitivity" below. |
| Comments inside `src` | **yes** | Same reason. |
| `kind` (`rust`, `rust-out`, `rust-display`, …) | **no** | Two snippets with the same source but different `kind` produce the same evcxr execution; only the *Typst-side rendering* differs. The kind is metadata for the package, not for evaluation, and the sidecar filenames already disambiguate by extension (`<id>.txt`, `<id>.png`, …). Including `kind` in the ID would cause unnecessary cache misses when a writer toggles `rust` ↔ `rust-out`. |
| Document path / filename | **no** | Would prevent two documents that legitimately share a snippet from sharing a cache entry. Path is captured in `loc` for diagnostics, not identity. |
| `doc_order` | **no, except as collision tiebreak** | Adding `doc_order` to every default ID would invalidate every cache entry below the insertion point on every paragraph add. That defeats the cache. See "Collision handling". |
| `:dep` state, evcxr version, rustc version, target triple | **no** | These belong in the **cache key**, not the ID. The ID names the snippet; the cache key decides whether a stored result is still valid for it. Mixing the two means renaming sidecars on every toolchain bump, which makes `clean` and `gc` harder and hides the toolchain dimension. |

## Whitespace sensitivity (v0 vs future)

**v0 decision**: hash raw bytes. Adding/removing a blank line, reformatting, or changing a comment changes the ID.

**Why this is acceptable**: the rustc artifact cache (evcxr `:cache`) absorbs most of the cost of a snippet-output cache miss. Cache misses on whitespace edits are annoying but cheap.

**Future option (not v0)**: a "normalize then hash" mode controlled by a package-level `setup(strict_id: false)`:
1. Parse `src` with `syn` (already a transitive dep of evcxr).
2. `prettyplease`-format the AST.
3. Strip line comments, keep doc comments (they affect compile output for some macros, e.g. `#[doc]`).
4. Hash the formatted bytes.

This is decidedly punted: the implementation is non-trivial (`syn` doesn't always round-trip macro-rules pleasantly), and the win is small.

## Collision handling

> ⚠ Refines `ARCHITECTURE.md` § "Snippet identity" — that section punts collision; here we resolve it.

A *real collision* — two snippets in one document with byte-identical source — is **not** an error. It happens routinely:

```typst
#rust(```rust println!("hello"); ```)   // section 1
… many pages …
#rust(```rust println!("hello"); ```)   // section 7
```

Both snippets must evaluate (their *outputs* might differ — the second one runs after intervening snippets that may have changed global state). They need distinct sidecar filenames.

**Algorithm** (executed by the CLI, not the package):

1. Compute `default_id = blake3(src)[..12]` for every snippet, in document order.
2. Walk the snippet list keeping a `seen: HashMap<id, count>`.
3. The first occurrence of an `id` keeps it bare (`abc123def456`).
4. Subsequent occurrences become `abc123def456-1`, `abc123def456-2`, … where the suffix is the **occurrence index**, not `doc_order`. This keeps suffixes stable when *unrelated* snippets are added between them.
5. Explicit IDs (see below) participate in the collision check just like default ones, but a collision among explicit IDs is a **hard error** (it's a user mistake, not a duplicate).

**Why occurrence index, not `doc_order`**: if you have two `println!("ok")` snippets and you insert a paragraph (and other snippets) between them, `doc_order` shifts but the occurrence index doesn't. That's the property the cache cares about.

**Pathological case**: a writer duplicates a snippet by copy-paste, then later edits one. The edited copy gets a new default ID immediately and stops colliding; the un-edited copy keeps the bare ID. No special handling.

## Explicit override

Users can pin an ID:

```typst
#rust(id: "warmup", ```rust let x = compute(); ```)
```

**Validation rules** (CLI-side, fail-fast at query time):

- **Allowed characters**: `[a-z0-9_-]`. ASCII only. No dots (collide with extensions), no slashes (paths), no spaces, no uppercase (case-insensitive FSes).
- **Length**: 1..=64 chars.
- **Reserved prefixes** (rejected with a clear error):
  - `_`  — reserved for CLI-internal sidecars (e.g. `_index.json`, `_meta.json`).
  - `evcxr-` — reserved for future package-internal use (e.g. error placeholders).
  - any string of 12 lowercase alphanumerics matching the default-ID shape — rejected to prevent a user pinning an ID that *looks* auto-generated and creating spooky cache entries.
- **Collisions among explicit IDs**: hard error, naming the two source locations.
- **A user explicit ID may collide with a default ID from elsewhere in the doc**: explicit wins, the default-ID-bearer gets the `-1` suffix. (Justification: the explicit ID was a deliberate user choice; we shouldn't move it.)

## Stability properties

Worked examples. "Stable" = ID unchanged.

| Edit | Default ID stability | Explicit ID stability |
|---|---|---|
| Add a paragraph of prose between snippets | stable | stable |
| Add a new snippet at the end | stable | stable |
| Add a new snippet *before* this one | stable | stable |
| Reorder two snippets (cut/paste) | stable | stable |
| Rename a local variable inside this snippet | **changes** | stable (id is pinned) |
| Reformat / re-indent inside this snippet | **changes** (v0) | stable |
| Add a comment inside this snippet | **changes** (v0) | stable |
| Edit an *earlier* snippet that this one consumes (e.g. struct definition) | stable | stable |
| Move snippet from page 2 to page 9 | stable | stable |
| Duplicate a snippet (copy-paste) | first keeps `xyz`, second becomes `xyz-1` | error if both have the same explicit ID |
| Delete the duplicate | the surviving one reverts to bare `xyz` | n/a |
| Bump rustc / evcxr / change `:dep` versions | stable (this is a *cache* concern) | stable |

The "stable across rustc bump" line is important: if rustc bumps invalidated IDs, sidecar filenames would change on every toolchain update, making the cache directory unreviewable. Toolchain changes invalidate cache **values**, not cache **keys' filenames**. See [`cache.md`](./cache.md).

## Decision: supersede D-005

D-005 is on the right track but underspecifies (a) hash encoding/length, (b) collision behavior, (c) override validation. We supersede it with D-007. Append the following to `docs/DECISIONS.md`:

```markdown
---

## D-007 — Snippet ID = blake3(src) base32, 12 chars, with occurrence-index suffix on collision (supersedes D-005)

**Status:** accepted · 2026-05-06

**Decision:**
- Default ID = `base32_lower(blake3(snippet_src_bytes))[..12]`. RFC 4648 alphabet, no padding, lowercase.
- Explicit override via `id:` on the package call. Validation: `[a-z0-9_-]{1,64}`, no reserved prefix (`_`, `evcxr-`, or default-ID-shape).
- Collisions among default IDs disambiguated by occurrence-index suffix (`xyz`, `xyz-1`, `xyz-2`, …). Collisions among explicit IDs are a hard error. An explicit ID that collides with a default ID wins; the default-bearer gets the suffix.
- The ID is *only* a stable name. Toolchain/dependency identity lives in the cache key, not the ID. See `docs/design/cache.md`.
- Whitespace/comment normalization is **not** in scope for v0; raw-bytes hashing is the v0 behavior. Future opt-in normalization mode is sketched in `docs/design/snippet-identity.md`.

**Rationale:**
- BLAKE3: fast, modern, well-supported, and `evcxr-typst` already pulls a hashing dep transitively.
- 12 base32 chars is filesystem-safe across macOS/Windows/Linux, case-insensitive-safe, and short enough to read in a directory listing while leaving 60 bits of entropy.
- Occurrence-index suffixing keeps duplicate-snippet suffixes stable across unrelated edits — `doc_order` would shift on every paragraph insertion.
- Reserved prefixes prevent foot-guns where a user pins a name that the system uses internally or that mimics auto-generated IDs.

**Consequences:**
- Whitespace-sensitive hashing means trivial reformatting busts the snippet-output cache for that snippet (but not its neighbours, and the rustc artifact cache absorbs most of the cost).
- The CLI must run a one-pass collision-resolver after `typst query`, before evaluation.
- Supersedes D-005.
```

## Cross-references

- [`cache.md`](./cache.md) — how the ID becomes part of file naming and how the *cache key* (a different thing) decides validity.
- `docs/ARCHITECTURE.md` § "Snippet identity", § "The metadata contract".
- `docs/DECISIONS.md` D-005 (superseded by D-007 above).
