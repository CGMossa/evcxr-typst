# Snippet-output cache

> **Scope**: when can the CLI skip re-evaluating a snippet because its prior output is still valid? This is the *snippet-output cache*, layered on top of evcxr's own `:cache` (the rustc artifact cache). The two caches are complementary, not competing.
>
> **Status**: design (Phase 0, T-D04). Companion to [`snippet-identity.md`](./snippet-identity.md).

## The two caches, plainly

| Cache | Owner | Granularity | What it stores | What it saves |
|---|---|---|---|---|
| **rustc artifact cache** (`:cache N`) | evcxr | per *rustc invocation* | dep crates' `.rmeta`, `.rlib`, `.so` files | rustc/linker time when the same crate is compiled with the same args |
| **snippet-output cache** (this doc) | evcxr-typst | per *snippet* | the snippet's sidecars (`<id>.txt`, `<id>.png`, …) | the entire evcxr round-trip when the snippet's *meaning* hasn't changed |

A snippet-output cache hit means: **don't even feed this snippet to evcxr**; the existing sidecars are still correct. A miss means: feed the snippet to evcxr; evcxr's `:cache` then determines whether *its* internal compilation can skip rebuilding deps.

## Cache key — concrete formula

```
cache_key(snippet, prior_chain, env) =
    blake3_hex(
        b"evcxr-typst-cache/v1\n" ||
        b"src=" || snippet.src_bytes || b"\n" ||
        b"prior=" || prior_chain_hash_bytes || b"\n" ||      # 32 raw bytes, see below
        b"deps=" || active_deps_canonical_bytes || b"\n" ||
        b"evcxr=" || env.evcxr_version_bytes || b"\n" ||
        b"rustc=" || env.rustc_version_string_bytes || b"\n" ||
        b"target=" || env.target_triple_bytes || b"\n" ||
        b"chan=" || env.passthrough_env_canonical_bytes || b"\n" ||
        b"sver=" || b"1\n"                                    # schema version of THIS layout
    )
```

`||` is byte concatenation. Newline separators prevent length-extension confusion across fields. `cache_key` is rendered as full hex (64 chars) — we are *not* truncating; collisions on the cache key would produce silently wrong output, while collisions on the snippet ID just produce ugly filenames.

`prior_chain_hash_bytes` is computed iteratively for snippet *i* in document order:

```
prior_chain_hash[0] = blake3(b"empty-chain-v1")
prior_chain_hash[i] = blake3(
    prior_chain_hash[i-1].as_bytes() ||
    b"\n" ||
    snippet[i-1].src_bytes
)
```

i.e. a Merkle-style chain over all previous snippets' sources. This is the *pessimistic-but-trivially-correct* answer to "did anything that this snippet might depend on change?".

### Justification per input

| Input | Why it must be in the key | Could we drop it? |
|---|---|---|
| `snippet.src_bytes` | obvious — different code, different output | no |
| `prior_chain_hash` | snippet B may consume `struct Foo` defined in snippet A; if A changes (even just to add a field), B's output may change | no, without parsing Rust (see § "Why pessimistic"). Punted to v1. |
| `active_deps_canonical_bytes` | a `:dep regex = "1.10"` → `"1.11"` change can change runtime behaviour of every snippet that uses regex; not part of `src` | no |
| `evcxr.version` | evcxr changes how it injects `main`, captures stdout, formats display; same source can produce different output across evcxr versions (especially around `Display`/`Debug` fallback) | no |
| `rustc.version` (`rustc -vV` full dump) | rustc bumps change `Debug` formatting, panic messages, optimizer-visible behaviour for some snippets; full `-vV` includes channel, host, commit hash | yes for trivial snippets, but we can't tell which trivially. Keep. |
| `target_triple` | cross-compiling changes pointer width, endianness, available cfg | yes if we forbid cross-compilation in v0. Keep — cheap, covers the case. |
| `passthrough_env` | snippets reading `env::var("LANG")` etc. produce env-dependent output. We canonicalize the *passlist* of env vars the user opts into (see § "Env passthrough"). | yes, by default — empty passlist is the default. Keep the field for forward-compat; it's the empty string in v0. |
| schema version `sver=1` | lets us bump the cache layout in future without colliding with old keys | yes, but adding it later requires invalidation; cheap insurance |

### What is *deliberately* excluded

- **Snippet ID** (`id`). The ID is a *name*, not part of the *meaning*. If the same `src` produces ID `abc` in one document and ID `def` in another (because of an explicit override), they should still cache-hit. The cache is keyed by content, looked up by content; the ID is only used for sidecar filenames and the on-disk index.
- **`doc_order`**. Folded into `prior_chain_hash` already. Including it separately would invalidate every cache entry below an inserted snippet, which is exactly the scenario we want the cache to handle gracefully.
- **`kind`** (`rust` vs `rust-out` vs …). Doesn't change evcxr execution; only changes how the package renders the existing sidecars.
- **Source file path of the `.typ`**. Not relevant to evaluation.
- **Wallclock time / mtime**. We want determinism.

### Pathological cases & how the formula handles them

| Case | Behaviour |
|---|---|
| Two byte-identical snippets in the same doc (`println!("ok")` × 2) | They *share a cache key*. They get separate IDs (and separate sidecar files) per [`snippet-identity.md`](./snippet-identity.md), but a hit on the first means the second can be **populated by copying** — same content, possibly different `prior_chain_hash` though, so usually it's a separate evaluation. The system handles both correctly because key includes `prior_chain_hash`. |
| Snippet moved with no edit, no surrounding edits | Same `src`, same `prior_chain_hash` (chain over earlier snippets unchanged), same key → hit. |
| Snippet edited, but produces identical output | Cache **miss** (we don't know the output is identical until we evaluate). Acceptable: caches key on inputs, not outputs; output-equality detection is out of scope. |
| evcxr version bumped | Every key changes → full document re-eval. Correct behaviour: evcxr behavioural changes are a real risk. The rustc artifact cache survives the bump (it's keyed differently) so the cost is dominated by linker time. |
| rustc bumped | Same as evcxr bump. |
| User adds whitespace to a snippet 3 of 50 | Snippets 3..=50 miss; snippets 1..=2 hit. (Whitespace busts the snippet's own ID and key; downstream snippets miss because their `prior_chain_hash` shifts.) |
| User adds a paragraph of *prose* between snippets 3 and 4 | All 50 hit. Prose is invisible to the cache. |
| User changes `:dep tokio = "1.30"` to `"1.40"` | Every snippet after the `:dep` directive misses (they all share `active_deps`). Snippets before the `:dep` line — if any — keep hitting. |

## Why pessimistic? (the `prior_chain_hash` question)

The "right" answer: hash, for snippet *i*, only the *items actually consumed* by *i*. That requires parsing snippet *i*, name-resolving against the union of all prior `committed_state.items_by_name`, and hashing the transitive closure. Approximate `evcxr/src/eval_context.rs:639` (`defined_item_names`) gives us the universe; mapping from a snippet's free names back to that universe is a Rust-aware analysis we don't want to write.

**v0 decision**: pessimistic chain hash. Correct, trivially implementable, and the dominant misses (someone edited an earlier snippet) are exactly the misses we want.

**Future option (v1+)**: precise dependency tracking. Would require either (a) `syn` parsing snippet-by-snippet to collect free names, or (b) a hook into evcxr that asks "which committed items did this evaluation consult?". Option (b) is the cleaner one and would be an upstream patch. Track in BACKLOG once Phase 1 lands.

## Env passthrough

The CLI accepts `--env-passthrough KEY` (repeatable) and `--env-passthrough-from-file PATH`. Only listed env vars are made visible to the evcxr child. The cache field `passthrough_env_canonical_bytes` is:

```
sorted_keys.map(|k| format!("{k}={}\n", env::var(k).unwrap_or_default())).join("")
```

Sorted, NUL-terminated would be safer; `\n` is fine because keys can't contain `\n` and we control the iteration. v0 default: empty passlist → empty string → cache key unaffected.

## Cache layout on disk

> ⚠ Refines `ARCHITECTURE.md` § "The pipeline" — diagram shows `.evcxr-typst-cache/<id>.{txt,png,…}`. We separate the **content-addressed store** (CAS) from the **id-addressed view**.

```
.evcxr-typst-cache/
├── v1/
│   ├── index.json                    # snippet-id → cache-key map for the current doc, atomic-write
│   ├── cas/
│   │   ├── 9f/
│   │   │   └── 9f3a4b…<full key>/    # one dir per cache_key
│   │   │       ├── meta.json         # {key, evcxr_version, rustc_version, target, deps_hash, schema, written_at}
│   │   │       ├── stdout.txt        # always present (may be empty)
│   │   │       ├── display/          # MIME blocks captured between EVCXR_BEGIN/END_CONTENT
│   │   │       │   ├── 0.png
│   │   │       │   ├── 0.mime        # "image/png"
│   │   │       │   ├── 1.html
│   │   │       │   └── 1.mime
│   │   │       └── status.json       # {exit: ok | compile_error | runtime_panic, ...}
│   │   └── …
│   └── tmp/                           # staging directory for atomic moves
└── README.txt                         # "managed by evcxr-typst, safe to delete"
```

**Why CAS-by-key, not by-id**:
- The same content (same key) computed in two documents can be shared.
- `clean` becomes "delete any CAS dir not referenced by any `index.json`" — straightforward GC.
- Renaming a snippet (changing its explicit ID) doesn't move bytes around — only `index.json` updates.
- The id-addressed sidecars that Typst's package consumes (`<doc-dir>/.evcxr-typst-cache/<id>.txt` style) are **hardlinks or copies** materialized from the CAS at the end of a CLI run. The Typst package only ever reads the materialized view; it has no awareness of the CAS.

The CAS sits at the *workspace* level (alongside the `.typ` source); we do not put it in `~/.cache` because it should be checked-in-or-gitignored alongside the project so CI builds are deterministic. Recommend `.gitignore`-by-default but document overriding for hermetic-CI users.

**Two-char fan-out** (`9f/9f3a4b…/`) avoids huge flat directories on FAT-family filesystems.

### Atomic-write strategy

For every cache miss:
1. Create `v1/tmp/<random-uuid>/`.
2. Write `meta.json`, `stdout.txt`, `display/*`, `status.json` into it.
3. `fsync` the directory.
4. `rename(2)` `tmp/<uuid>` → `cas/<XX>/<full-key>/`. POSIX `rename` is atomic on the same FS; on Windows we use `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`. If the target already exists (concurrent CLI), keep the existing one and discard ours — the bytes are equal by construction.

For the id-addressed view:
1. Stage in `tmp/view-<uuid>/<id>.<ext>`.
2. Hardlink (preferred) or copy from CAS.
3. `rename` over the live view location.

`index.json` is rewritten atomically (write to `tmp/index-<uuid>.json` → fsync → rename) at the end of each CLI run.

## Eviction

**v0**: never evict automatically. Document `evcxr-typst clean` and `evcxr-typst gc`.

- `clean` — drop the entire `.evcxr-typst-cache/` for the current workspace. Equivalent to `rm -rf`.
- `gc` — drop CAS entries not referenced by `index.json`. Useful after lots of snippet edits.

A `--cache-size-bytes N` option is *deferred*. The rustc artifact cache (evcxr's `:cache`) is the one with growth concerns (large `.rlib`s); the snippet-output cache stores small sidecars (text, modest images) and is unlikely to balloon for any realistic document.

## Interaction with evcxr's `:cache`

- **On by default**. The CLI emits `:cache 500` (500 MB) at session startup, before any user `:dep` directives. Rationale: rustc artifact caching is what makes "edit a snippet" cheap when the snippet-output cache misses; turning it off is almost always wrong.
- **Configurable**: `--evcxr-cache-mb N` overrides the budget. `--evcxr-cache-mb 0` disables it. Setting `0` is equivalent to evcxr's `:cache 0` semantics: cache-not-consulted, existing entries left intact.
- **Cache directory**: evcxr stores its cache in `dirs::cache_dir()/evcxr` per `evcxr/src/module/cache.rs:99-103`. We don't touch it; it's user-global, deliberately. Our snippet-output cache is workspace-local.
- **If the user sets `:cache 0` from inside a snippet**: we honour it. Their call.
- **`:clear_cache`**: not invoked by us. Document a `--evcxr-clear-cache` CLI flag for the rare case where the user wants both layers wiped.

## Cross-references

- [`snippet-identity.md`](./snippet-identity.md) — the ID layer.
- `docs/ARCHITECTURE.md` § "Caching" — superseded on disk-layout by this doc; otherwise consistent.
- `evcxr/src/module/cache.rs` — evcxr's own cache, concept-level reference.
- `COMMON.md` § "Caching" — user-facing description of `:cache N` and `:clear_cache`.
