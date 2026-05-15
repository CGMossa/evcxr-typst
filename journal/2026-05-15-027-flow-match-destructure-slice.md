# `flow_control/match/destructuring/destructure_slice` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-destructure-slice
**Upstream source:** `.rust-by-example/src/flow_control/match/destructuring/destructure_slice.md` (snapshot 898f0ac)

## What I tried

Port the second sub-chapter under destructuring — arrays and slices. Single snippet wrapped in `fn main()` showing five distinct array-pattern arms: `[0, second, third]` exact-prefix match, `[1, _, third]` ignore-with-underscore, `[-1, second, ..]` rest-ignore, `[3, second, tail @ ..]` rest-bind-with-`@`, `[first, middle @ .., last]` first-and-last with rest-bound-in-middle. For `array = [1, -2, 6]` the second arm fires. Used `rust-main(...)`.

## What happened

Sidecar matches upstream exactly:

```
array[0] = 1, array[2] = 6 and array[1] was ignored
```

`{"extensions":["txt"],"v":1}` in the manifest. No `WARN evcxr_typst` lines.

## What I learned

Same chapter shape as `destructure_tuple` — `rust-main` body with many arms, only one fires for the chosen input. The `@`-binding (`tail @ ..`, `middle @ ..`) compiles fine through evcxr's wrapping; nothing special needed.

Upstream's cross-doc link `[Binding](../binding.md)` would point at `flow_control/match/binding.md`, which is unported. Inlined the reference as prose ("`@` sigil — covered later in this chapter") rather than substituting a rust-lang.org URL like I did for the tuples link — this is a forward reference *within* the document, and Typst's `#outline` will surface `match/binding` once it lands. Worth noting as a soft convention: external upstream cross-link → URL substitution; internal forward reference → prose hint that the section will appear later.

## Follow-ups

- [ ] Open: `flow_control/match/destructuring/destructure_enum.md` is next.
