# `flow_control/match/destructuring/destructure_pointers` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-destructure-pointers
**Upstream source:** `.rust-by-example/src/flow_control/match/destructuring/destructure_pointers.md` (snapshot 898f0ac)

## What I tried

Port the fourth destructuring sub-chapter — pointers / `ref` / `ref mut`. Single snippet wrapped in `fn main()` covering the four cases: `&val` destructure-the-reference, `*reference` dereference-then-match, `ref r` create-a-ref-via-pattern, `ref mut m` create-a-mut-ref then `*m += 10` mutate-through-it. Used `rust-main(...)`. Upstream's `[The ref pattern](../../../scope/borrow/ref.md)` cross-link adapted to its rust-lang.org URL since `scope/borrow/ref.md` is unported.

## What happened

Sidecar matches upstream exactly:

```
Got a value via destructuring: 4
Got a value via dereferencing: 4
Got a reference to a value: 5
We added 10. `mut_value`: 16
```

`{"extensions":["txt"],"v":1}` in the manifest. No `WARN evcxr_typst` lines.

## What I learned

`ref` / `ref mut` patterns work cleanly through `rust-main` and evcxr. No surprises — the snippet is self-contained inside `fn main()` so no scope-leaking concerns. The `let ref _is_a_reference = 3;` form (binding-side `ref`) doesn't even emit a warning despite being unused, because it has the `_`-prefix that conventionally silences unused warnings.

## Follow-ups

- [ ] Open: `flow_control/match/destructuring/destructure_structures.md` is the last sub-chapter under destructuring.
