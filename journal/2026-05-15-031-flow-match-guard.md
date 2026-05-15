# `flow_control/match/guard` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-match-guard
**Upstream source:** `.rust-by-example/src/flow_control/match/guard.md` (snapshot 898f0ac)

## What I tried

Port the match-guard chapter. Two snippets upstream:

- First snippet: `enum Temperature` with `Celsius`/`Fahrenheit` variants matched with `if` guards. Evaluated cleanly via `rust-main(...)` — for `Temperature::Celsius(35)`, the first arm fires.
- Second snippet: tagged `ignore,mdbook-runnable` upstream, deliberately non-exhaustive (catch-all commented out). Source-only per `examples/rust-by-example/CLAUDE.md` rule 1 — evaluating it would produce a compile-error box and lose the "guards don't make exhaustiveness checking work for you" pedagogy.

## What happened

Sidecar for the evaluable snippet matches upstream:

```
35C is above 30 Celsius
```

`{"extensions":["txt"],"v":1}`. No `WARN evcxr_typst` lines.

## What I learned

The match-guard chapter pattern (one evaluable + one deliberately-fails) matches the convention already used in `variable_bindings/{mut,scope,declare,freeze}.typ`'s error-demoing blocks. The split keeps the evaluable part teaching what *works* and the source-only part teaching what *the compiler rejects*. Easier than wrapping the failing block in a special "this should fail" affordance — which would also be wrong, because the lesson is about *exhaustiveness checking*, not about runtime panics, and there is no eval output to capture.

## Follow-ups

- [ ] Open: `flow_control/match/binding.md` is next — the last sub-chapter under `match`.
