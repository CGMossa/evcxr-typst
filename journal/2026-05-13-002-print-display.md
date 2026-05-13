# Display formatting chapter

**Date:** 2026-05-13
**Branch:** codex/rbe-incremental
**Upstream source:** `.rust-by-example/src/hello/print/print_display.md` (snapshot 898f0ac)
**Resulting commit:** TBD

## What I tried

Port the `Display` subchapter immediately after the `Debug` chapter, with `evcxr-typst watch --allow-eval --root . examples/rust-by-example/main.typ` already running. The goal was to keep the chapter faithful enough to upstream while using the new `rust-main(...)` path instead of visible synthetic calls.

## What happened

The first code block defines `Structure` plus a `fmt::Display` implementation and has no stdout. It uses the same raw-source plus `rust-hidden(...)` pattern as the definitions-only block in the `Debug` chapter. That pattern matters here because `Structure` intentionally shadows a same-named type introduced in the prior debug chapter; the run order stays explicit, and no fake output box is rendered for a block that never prints.

The runnable `MinMax` / `Point2D` example uses `rust-main(...)` and produced the upstream output under watch. The chapter did not need any Rust source changes beyond preserving upstream comments and mdBook prose in Typst form.

## What I learned

The formatting chapters are now exercising a repeatable authoring convention:

- definitions-only examples: render raw source, evaluate with `rust-hidden(...)`;
- runnable `fn main()` examples: render unchanged source with `rust-main(...)`;
- activities: keep expected-output raw blocks as prose, not evaluable snippets.

That convention is stable enough to use for the remaining formatted-print children unless a later chapter deliberately demonstrates compile failure.

## Follow-ups

- [ ] Consider a small package helper for source-visible hidden evaluation, since `#raw(src.text, ...)` plus `#evcxr.rust-hidden(src, ...)` is now repeated.
