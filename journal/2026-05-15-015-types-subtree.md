# `types` subtree (Types section)

**Date:** 2026-05-15
**Branch:** rbe/types-subtree
**Upstream source:** `.rust-by-example/src/types{.md,/cast.md,/literals.md,/inference.md,/alias.md}` (snapshot 898f0ac)

## What I tried

Port the whole Types subtree as one PR (per the new "subtree-PR when section shape is uniform" pattern). Five chapters:

- `types.md`: section opener, prose only.
- `types/cast.md`: source-only (two deliberate compile errors — implicit float→u8, float→char).
- `types/literals.md`: `rust-main`, id `rbe-types-literals` — `size_of_val` on suffixed/unsuffixed literals.
- `types/inference.md`: `rust-main`, id `rbe-types-inference` — vec-element-driven `Vec<u8>` inference, output `[5]`.
- `types/alias.md`: `rust-main`, id `rbe-types-alias` — top-level `type` aliases (`NanoSecond`, `Inch`, `U64` all = `u64`).

Verified the new `type` aliases don't collide with anything later in the book via grep.

## What happened

All three runnable snippets matched upstream output verbatim:

```
size of `x` in bytes: 1
size of `y` in bytes: 4
size of `z` in bytes: 4
size of `i` in bytes: 4
size of `f` in bytes: 8
```
```
[5]
```
```
5 nanoseconds + 2 inches = 7 unit?
```

## What I learned

The subtree-PR pattern is paying off — five chapters in one logical merge for a section that doesn't introduce new per-chapter authoring decisions.

## Follow-ups

- None.
