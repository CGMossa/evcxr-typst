# `flow_control/match/destructuring/destructure_tuple` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-destructure-tuple
**Upstream source:** `.rust-by-example/src/flow_control/match/destructuring/destructure_tuple.md` (snapshot 898f0ac)

## What I tried

Port the first concrete sub-chapter under destructuring — tuple destructuring in match arms. Single snippet wrapped in `fn main()` showing `(0, y, z)` value-then-bind, `(1, ..)` / `(.., 2)` rest-ignoring, `(3, .., 4)` first-and-last with rest, and a `_` catch-all. Used `rust-main(...)`. Adapted upstream's cross-doc link to `../../../primitives/tuples.md` as an `https://doc.rust-lang.org/rust-by-example/...` external link, since we don't have a built mdBook locally and cross-doc Typst links would need a different convention.

## What happened

Sidecar matches upstream exactly (`(0, y, z)` arm fires for `(0, -2, 3)`):

```
Tell me about (0, -2, 3)
First is `0`, `y` is -2, and `z` is 3
```

`{"extensions":["txt"],"v":1}` in the manifest. No `WARN evcxr_typst` lines.

## What I learned

Four-level path nesting works fine — relative import goes up five `..`s (`../../../../../packages/evcxr/lib.typ`). This is the deepest chapter path so far in the rbe port. The convention scales straightforwardly: one `..` per directory depth, plus one to escape into the sibling `packages/` tree.

Cross-doc Markdown links (`[Tuples](../../../primitives/tuples.md)`) don't have a clean Typst-side analogue while chapters are still landing one at a time. Substituting the upstream rust-lang.org URL keeps the cross-reference functional without coupling chapter files to each other's filenames. If we later teach the porter (or this hand-port) to rewrite local `.md` links into Typst `#link(<other-chapter.typ>)` references that's a separate convention to invent — for now, external URL is simplest and survives chapter reorganization.

## Follow-ups

- [ ] Open: `flow_control/match/destructuring/destructure_slice.md` is next.
- [ ] Discuss: a Typst-side convention for cross-chapter links. The current "use upstream URL" works but means we link AWAY from the ported document instead of WITHIN it. Could be solved by mapping `(../../../primitives/tuples.md)` → a section reference at compile time, but only worth doing once the cross-link count is high enough to matter.
