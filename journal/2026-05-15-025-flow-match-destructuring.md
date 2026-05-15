# `flow_control/match/destructuring` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-match-destructuring
**Upstream source:** `.rust-by-example/src/flow_control/match/destructuring.md` (snapshot 898f0ac)

## What I tried

Port the thin parent chapter under `match` that introduces the five destructuring sub-chapters (tuples / slices / enums / pointers / structures). No code snippets upstream — the chapter is intro paragraph + bullet list of sub-chapters + "See also" linking to the Rust Reference. Rendered as: `=== Destructuring` heading, the intro line, a Typst-native bulleted list mirroring the upstream bullets, and a `==== See also` subsection holding the Rust Reference link.

## What happened

`typst compile --root . main.typ` exits 0; watch loop continues to compile cleanly with no `WARN evcxr_typst` or `ERROR`. No per-snippet sidecars to inspect — the chapter has no `rust` / `rust-main` calls.

## What I learned

Three-level nesting (`flow_control/match/destructuring.typ`) now works the same way the `loop/{nested,return}` chapters did — relative import goes up four `..`s. A "thin parent" chapter without snippets is the smallest possible chapter port: heading + prose + maybe a link. The upstream Markdown bullet links to sub-chapter pages don't need to be reproduced as cross-document links — the document outline (via `#outline(depth: 2)` in `main.typ`) already surfaces the sub-chapters, and the bullets serve as a textual preview.

## Follow-ups

- [ ] Open: `flow_control/match/destructuring/destructure_tuple.md` is the first sub-chapter under destructuring. Pick up next.
