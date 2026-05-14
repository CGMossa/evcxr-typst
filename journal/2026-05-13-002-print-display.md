# Display formatting chapter

**Date:** 2026-05-13
**Branch:** codex/rbe-incremental
**Upstream source:** `.rust-by-example/src/hello/print/print_display.md` (snapshot 898f0ac)
**Resulting commits:** 14fe22b (chapter port), 1ba0906 (keep explanatory defs source-only)

## What I tried

Port the `Display` subchapter immediately after the `Debug` chapter, with `evcxr-typst watch --allow-eval --root . examples/rust-by-example/main.typ` already running. The goal was to keep the chapter faithful enough to upstream while using the new `rust-main(...)` path instead of visible synthetic calls.

## What happened

The first code block defines `Structure` plus a `fmt::Display` implementation and has no stdout. I first tried to make source-visible examples evaluate through the existing `rust(render: "source", ...)` idea. That surfaced the real issue: in one long evcxr session this block redefines `Structure`, which breaks the already-persisted `Deep(Structure)` from the previous `Debug` chapter because the new `Structure` no longer implements `Debug`.

The fix is chapter-level, not package-level: explanatory no-output blocks that define top-level items should be rendered as plain Rust source and not evaluated into the shared context. The runnable examples still evaluate.

The runnable `MinMax` / `Point2D` example uses `rust-main(...)` and produced the upstream output under watch. The chapter did not need any Rust source changes beyond preserving upstream comments and mdBook prose in Typst form.

## What I learned

The formatting chapters are now exercising a repeatable authoring convention:

- definitions-only examples that introduce top-level items: render plain source only, because evaluating them can pollute or break the shared evcxr context;
- runnable `fn main()` examples: render unchanged source with `rust-main(...)`;
- activities: keep expected-output raw blocks as prose, not evaluable snippets.

That convention is stable enough to use for the remaining formatted-print children unless a later chapter deliberately demonstrates compile failure.

## Follow-ups

- [x] Resolve the source-visible hidden-eval idea by not evaluating top-level explanatory definitions; the Display `Structure` collision proves this must stay source-only unless we add an explicit isolated-eval mode later.
