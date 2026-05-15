# `flow_control/loop/nested` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-loop-nested
**Upstream source:** `.rust-by-example/src/flow_control/loop/nested.md` (snapshot 898f0ac)

## What I tried

Port the loop/nested chapter — first sub-chapter under `loop`. Single snippet upstream: a `'outer:` / `'inner:` labeled-loop demo where `break 'outer` exits both. Snippet is wrapped in `fn main()` and prefixed with `#![allow(unreachable_code, unused_labels)]` to silence the dead "this point will never be reached" line and the unused `'inner:` label. Used `rust-main(...)` and rendered the inner-attribute pass-through unchanged.

## What happened

Watch loop evaluated cleanly on save. Sidecar matches upstream exactly:

```
Entered the outer loop
Entered the inner loop
Exited the outer loop
```

No `WARN evcxr_typst` lines. `typst compile --root . main.typ` exits 0 (only the pre-existing `monospace` fallback warning, unrelated).

## What I learned

`#![allow(...)]` inner attributes pass through `rust-main` cleanly — the same pattern was already in use in `types/cast.typ`, `custom_types/structs.typ`, and the `custom_types/enum/*` chapters, so this just confirms the convention scales to flow-control snippets where the upstream attribute is load-bearing for pedagogical "unreachable on purpose" lines.

Also worth noting: this is the first chapter under a *third*-level path (`flow_control/loop/nested.typ`). Relative imports go up four `..`s (`../../../../packages/evcxr/lib.typ`), one per directory level. No new convention needed — just follow the path depth.

## Follow-ups

- [ ] Open: `flow_control/loop/return.md` is the natural pair (also a one-snippet sub-chapter under `loop`). Next iteration should pick it up.
