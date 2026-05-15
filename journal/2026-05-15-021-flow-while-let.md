# `flow_control/while_let` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-while-let
**Upstream source:** `.rust-by-example/src/flow_control/while_let.md` (snapshot 898f0ac)

## What I tried

Port the while-let chapter. Two snippets upstream:

- Awkward `loop { match optional { … } }` sequence — bare top-level statements, no `fn main`.
- `while let Some(i) = optional { … }` solution — inside `fn main`.

Source-only for the first (same evcxr top-level inference issue we hit in `if_let` — `let mut optional = Some(0);` at the persistent scope), `rust-main` for the second.

## What happened

Watch loop evaluated the `while-let` block on file save. Sidecar matches upstream exactly:

```
`i` is `0`. Try again.
…
`i` is `9`. Try again.
Greater than 9, quit!
```

The awkward-loop block is rendered source-only with an inline note, identical convention to `if_let`'s opening block.

## What I learned

Confirms the pattern from yesterday's `if_let` finding: any upstream snippet that opens with `let mut? optional = Some(…);` at top level needs either an explicit `: Option<T>` annotation, a `fn main` wrapper, or the source-only treatment. Source-only stays the simplest answer when the block's pedagogical role is comparative.

## Follow-ups

- [ ] Open: this is the second chapter to trip on the same evcxr top-level inference pattern. Worth a fourth bullet in `examples/rust-by-example/CLAUDE.md` § "When to render a block as source-only" — defer until the pattern repeats once more, or until match.md (which also opens with bare `Option` statements upstream).
