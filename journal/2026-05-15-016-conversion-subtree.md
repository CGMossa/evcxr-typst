# `conversion` subtree (Conversion section)

**Date:** 2026-05-15
**Branch:** rbe/conversion-subtree
**Upstream source:** `.rust-by-example/src/conversion{.md,/from_into.md,/try_from_try_into.md,/string.md}` (snapshot 898f0ac)

## What I tried

Port the full Conversion subtree as one PR. Seven snippets across four chapters; lots of repeating-type redefinitions to validate evcxr's tolerance of stale impl blocks.

- `conversion.md`: section opener, prose only.
- `conversion/from_into.md`: one source-only str→String demo, three `rust-main` snippets (`from`, `into`, `interchangeable`), each redefining `struct Number` with different impl blocks.
- `conversion/try_from_try_into.md`: one `rust-main`, `EvenNumber`, only assertions (no stdout).
- `conversion/string.md`: three `rust-main` snippets — `Circle` + `Display`, parse with turbofish, `Circle` redefined + `FromStr` impl.

## What happened

All seven runnable snippets matched upstream verbatim. Notable: evcxr cleanly tolerated **two cascading redefinition cycles** in one PR (`Number` redefined 3×, `Circle` redefined 2×). The `From` impl from `from_into-from` left behind a stale impl when `Number` was redefined in `into` — evcxr didn't trip on it.

The `try_from_try_into` snippet has only assertions; manifest came back as `{"extensions":[],"v":1}` — second confirmation that PR #46's `_read-stdout` fix is doing its job.

## What I learned

evcxr's persistent context is more forgiving than I feared with cascading redefinitions in adjacent rust-main snippets. The risk model in `2026-05-15-008-custom-types-enum.md`'s journal was overcautious — type redefinition + stale impl blocks don't typically trigger `TypeRedefinedVariablesLost` unless a *live binding* references the prior type. For separate `rust-main` blocks where bindings stay function-local, the impl-block staleness is silently absorbed.

## Follow-ups

- None.
