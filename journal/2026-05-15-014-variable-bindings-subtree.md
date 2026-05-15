# `variable_bindings/{mut,scope,declare,freeze}` chapters

**Date:** 2026-05-15
**Branch:** rbe/variable-bindings-subtree
**Upstream source:** `.rust-by-example/src/variable_bindings/{mut,scope,declare,freeze}.md` (snapshot 898f0ac)

## What I tried

Port the four children of `variable_bindings/` together — they're all tiny and three of them are `ignore,mdbook-runnable` source-only-by-pedagogy. Per-chapter PRs felt over-granular for content this uniform; bundled into one subtree PR.

- `mut.md`: source-only (`_immutable_binding += 1;` deliberate error)
- `scope.md`: two snippets — first source-only (`short_lived_binding` used outside its block), second runnable (shadowing demo — `rust-main`, id `rbe-variable-bindings-scope-shadow`)
- `declare.md`: source-only (read of uninitialized binding)
- `freeze.md`: source-only (write to frozen binding)

## What happened

`scope`'s second snippet evaluated cleanly via the watch loop, output verbatim:

```
before being shadowed: 1
shadowed in inner block: abc
outside inner block: 1
shadowed in outer block: 2
```

Source-only chapters compiled through the fallback path; the full book is at 679 KB now.

## What I learned

Subtree-PR is right when a section is uniform in shape (here: all source-only-by-default). Per-chapter PRs are the default; this is the exception, justified by the lack of per-chapter authoring decisions.

## Follow-ups

- The "fold the source-only-by-pedagogy convention into chapter CLAUDE.md" follow-up has stalled across four open journal entries now (`primitives.md`, `array.md`, `constants.md`, and this). Time to actually do it — captured as a separate task next.
