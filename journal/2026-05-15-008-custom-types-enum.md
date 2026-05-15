# `custom_types/enum` chapter

**Date:** 2026-05-15
**Branch:** rbe/custom-types-enum
**Upstream source:** `.rust-by-example/src/custom_types/enum.md` (snapshot 898f0ac)

## What I tried

Three snippets in this chapter:

1. **WebEvent enum + inspect + main**: classic full-eval example — `rust-main`, id `rbe-custom-types-enum-webevent`.
2. **Operations type alias + main**: defines a `type Operations = …` alias, calls `main` which only does `let x = Operations::Add;`. No stdout. `rust-main`, id `rbe-custom-types-enum-alias` — used here partly to exercise PR #46's no-stdout sidecar fix in the wild.
3. **impl block with Self alias**: redefines `VeryVerboseEnumOfThingsToDoWithNumbers` (collides with snippet 2's persistent definition) and has no `fn main`. Source-only.

## What happened

All three behaved as expected via the watch loop.

Snippet 1 produced the canonical upstream output:
```
pressed 'x'.
pasted "my text".
clicked at x=20, y=80.
page loaded
page unloaded
```

Snippet 2's manifest came back as `{"extensions":[],"v":1}` — no `.txt` written because no stdout. `lib.typ`'s `_read-stdout` (post PR #46) noticed `txt` was absent in the manifest and returned `[]` (empty content) instead of erroring. The chapter renders the source plus literally nothing below it. This is **the first chapter to actually depend on the PR #46 fix.** Without it, the chapter would have broken with a "file not found" Typst error.

Snippet 3 is plain `#raw(..., lang: "rust")` per the chapter convention for evaluation-pointless definition-only blocks.

## What I learned

PR #46 was a load-bearing prerequisite, not just theoretical — the next chapter forward (Enums, this one) needed it. Good signal that fixing latent bugs proactively was the right move.

`rust-main` snippets with no stdout are a legitimate authoring pattern. The chapter shows the syntax + Self alias; the *running* of it is part of the contract the package promises ("yes, it evaluated; no, it didn't print"). The empty manifest is a positive signal, not an error.

## Follow-ups

- None new.
