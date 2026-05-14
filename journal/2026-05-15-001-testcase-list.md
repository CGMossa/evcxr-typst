# `print_display/testcase_list` chapter

**Date:** 2026-05-15
**Branch:** rbe/testcase-list
**Upstream source:** `.rust-by-example/src/hello/print/print_display/testcase_list.md` (snapshot 898f0ac)

## What I tried

Port the next chapter in upstream `SUMMARY.md` order after `print_display.md`: the testcase that implements `fmt::Display` for `struct List(Vec<i32>)` using `?` on `write!`. This is the first chapter to nest under `print_display/`, so the path layout becomes `hello/print/print_display.typ` (file) and `hello/print/print_display/testcase_list.typ` (file inside a directory of the same name) — Typst is fine with that coexistence, and the existing `hello/print.typ` / `hello/print/` pattern already proves it.

## What happened

Wrote the chapter as a single `evcxr.rust-main(...)` snippet (id `rbe-hello-print-display-testcase-list`) since upstream is one runnable `fn main()` example. The `rust,ignore` syntax-illustration block at the top is rendered as a plain `text`-language code fence, not an evcxr snippet — `?` on `write!` outside a function body wouldn't even parse and there's nothing to evaluate. No top-level item collisions with prior chapters: `List` is new; `Structure`, `MinMax`, `Point2D`, `Deep`, `Person` from earlier chapters stay untouched.

Bare `typst compile --root . examples/rust-by-example/main.typ` succeeds (PDF grows 211 → 237 KB, matching the new chapter content + placeholder box). Integration eval (`evcxr-typst run --allow-eval`) was **not run in this session** — the previous attempt during PR #39's review hung at 0% CPU for 41 minutes after the cache directory was cleaned. PR #40's `RUST_TEST_THREADS=1` config may make a fresh attempt cleaner, but I didn't burn the time here.

## What I learned

The naming-pattern coexistence (`X.typ` next to `X/`) extends one level deeper without surprises. Good — it means the rbe SUMMARY tree maps to the on-disk tree without renames.

## Follow-ups

- [ ] Run `evcxr-typst run --allow-eval --root . examples/rust-by-example/main.typ` from a fresh terminal to confirm the new snippet captures `[1, 2, 3]` to `rbe-hello-print-display-testcase-list.txt`. Deferred because the previous session's eval pipeline got stuck.
- [ ] The activity ("print the index too") is left as prose per the convention `examples/rust-by-example/CLAUDE.md` pins (activities aren't evaluable snippets unless we're showing the canonical answer).
