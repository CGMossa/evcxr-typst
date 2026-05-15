# `flow_control/match/destructuring/destructure_structures` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-destructure-structures
**Upstream source:** `.rust-by-example/src/flow_control/match/destructuring/destructure_structures.md` (snapshot 898f0ac)

## What I tried

Port the fifth and final destructuring sub-chapter — structs. Single snippet wrapped in `fn main()` that defines two local structs (`Foo` with a `(u32, u32)` tuple field and a `u32` field, and `Bar { foo: Foo }`), then exercises three matching styles: match-with-field-pattern, irrefutable `let Foo { x: x0, y: y0 } = faa` destructuring, and nested `let Bar { foo: Foo { x, y } } = bar` destructuring. Used `rust-main(...)`.

## What happened

Sidecar matches upstream exactly:

```
First of x is 1, b = 2,  y = 3 
Outside: x0 = (1, 2), y0 = 3
Nested: nested_x = (1, 2), nested_y = 3
```

(The trailing space after `y = {}` is upstream's formatting, preserved.) `{"extensions":["txt"],"v":1}` in the manifest. No `WARN evcxr_typst` lines.

## What I learned

Structs defined inside `fn main()` stay function-local under `rust-main` — the persistent evcxr context picks up only what escapes `main`. This is the same scope-leak avoidance documented in `examples/rust-by-example/CLAUDE.md` § "Fidelity vs. evcxr's evaluation model" for `let` bindings; the rule applies equally to local-item definitions (`struct`, `enum`, `fn`). Nested `let X { y: ... } = ...` destructuring with multiple levels of brace patterns works without quirks.

## Follow-ups

- [ ] Open: `flow_control/match/guard.md` is the next chapter (out of destructuring, on to match guards).
- [ ] Closes: destructuring set complete (`tuple` / `slice` / `enum` / `pointers` / `structures` — all five sub-chapters under `match/destructuring`).
