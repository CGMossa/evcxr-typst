# `flow_control/match/destructuring/destructure_enum` chapter

**Date:** 2026-05-15
**Branch:** rbe/flow-control-destructure-enum
**Upstream source:** `.rust-by-example/src/flow_control/match/destructuring/destructure_enum.md` (snapshot 898f0ac)

## What I tried

Port the third destructuring sub-chapter — enum destructuring in match. Single snippet wrapped in `fn main()` that defines `enum Color` with eight variants (three unit-like + RGB / HSV / HSL / CMY tuple variants + CMYK 4-tuple), and matches on `Color::RGB(122, 17, 40)`. Used `rust-main(...)`. The chapter note in `examples/rust-by-example/custom_types/enum/c_like.typ:9` already flagged that this chapter would redefine `Color` once more — `Color` has been a `struct` in `hello/print/fmt.typ` and a small C-like `enum` in `c_like.typ`; here it acquires the full color-model variant set.

## What happened

Sidecar matches upstream exactly:

```
What color is it?
Red: 122, green: 17, and blue: 40!
```

No `WARN evcxr_typst` lines despite the third redefinition of `Color`. evcxr's redefinition tolerance (per `eval.rs::~340`, the "redefinition_warning" branch that returns `SnippetOutcome::Ok` rather than failing) absorbs the change silently.

## What I learned

Repeated redefinition of the same type-name across chapters works through evcxr without manual intervention. The eval loop already classifies "redefined" as `Ok` (not `RuntimePanic`) so the snippet evaluates and the new definition wins for subsequent snippets in the persistent context. The chapter-fidelity classifier in `docs/operations/rbe-autoloop.md` mentions "previously defined redefinition collision with prior chapter" as a *chapter-fidelity quirk* that should be source-only treated, but in practice — at least when the redefining snippet has `fn main()` and uses the new type internally rather than relying on prior bindings — the eval path handles it fine. Worth noting that the source-only treatment guidance applies when the *new* definition would silently change behavior of a *later* snippet that still imagines the old definition. Here, no subsequent chapter has yet been ported that re-uses `Color`, so the redefinition is benign.

## Follow-ups

- [ ] Open: `flow_control/match/destructuring/destructure_pointers.md` is next.
- [ ] Pin: a tutorial entry on "redefining types across chapters" — there's now a worked example (struct→enum→bigger enum) showing it's safe to redefine when each snippet defines what it needs.
