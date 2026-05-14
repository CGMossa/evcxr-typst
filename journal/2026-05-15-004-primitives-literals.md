# `primitives/literals` chapter

**Date:** 2026-05-15
**Branch:** rbe/primitives-literals
**Upstream source:** `.rust-by-example/src/primitives/literals.md` (snapshot 898f0ac)

## What I tried

Port the first child of `primitives/`: literals and operators. Single `evcxr.rust-main(...)` snippet (id `rbe-primitives-literals`) — `fn main()` only, no top-level items, no name collisions with anything earlier in the book.

## What happened

Mechanical. Bare typst compile succeeded; PDF grew 317 → 343 KB.

## What I learned

Nothing new — this is now the third or fourth chapter using the same one-`rust-main`-snippet shape. The pattern is stable; surface for variation comes when chapters introduce top-level items, multiple blocks, or compile-fail blocks (the `mdbook-runnable` case from `primitives.typ`).

## Follow-ups

- [ ] Integration eval still pending across the chain of recent chapter PRs.
