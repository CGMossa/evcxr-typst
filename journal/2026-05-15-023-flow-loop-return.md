# `flow_control/loop/return` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-loop-return
**Upstream source:** `.rust-by-example/src/flow_control/loop/return.md` (snapshot 898f0ac)

## What I tried

Port the loop/return chapter — the second sub-chapter under `loop`. Single snippet upstream: a counter loop that uses `break counter * 2;` to return a value from a `loop` expression, then `assert_eq!(result, 20)`. Used `rust-main(...)` since the snippet is wrapped in `fn main()`.

## What happened

Watch loop evaluated cleanly on save. The snippet emits no stdout (the `assert_eq!` passes silently), so the per-snippet manifest at `.evcxr-typst-cache/rbe-flow-loop-return.manifest.json` settled at `{"extensions":[],"v":1}` — no `.txt` sidecar was written, by design. Compared against `rbe-flow-loop-nested.manifest.json` from the previous chapter which has `extensions:["txt"]` because that snippet `println!`s. `typst compile --root . main.typ` exits 0; no `WARN evcxr_typst` lines.

## What I learned

First "silent eval" chapter in the rbe port. The `_manifest-exts(id)` / `_read-stdout(kind, id, ...)` path in `packages/evcxr/lib.typ` already handles `extensions: []` cleanly — `read-stdout` just emits nothing when there is no `txt` extension, so the chapter renders the code block followed by nothing, which is the right shape for this teaching ("`break value` returns from `loop`; the assertion is the test"). Worth pinning: an empty `extensions` array is the "ran, produced nothing" marker, distinct from an absent manifest (which is "didn't run / pre-eval state").

## Follow-ups

- [ ] Open: `flow_control/match.md` is the next first-level entry under flow_control after `for`. Several sub-chapters under match (destructuring, guard, binding). Pick up `match.md` itself next.
- [ ] Pin in a tutorial or test: "empty extensions == successful silent eval" — it's not explicitly documented anywhere as a contract, but `lib.typ` already relies on it. Worth a test under `crates/evcxr-typst/tests/` so a future schema change doesn't break the silent-eval path.
