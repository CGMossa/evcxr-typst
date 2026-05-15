# `custom_types/enum/testcase_linked_list` chapter

**Date:** 2026-05-15
**Branch:** rbe/enum-linked-list
**Upstream source:** `.rust-by-example/src/custom_types/enum/testcase_linked_list.md` (snapshot 898f0ac)

## What I tried

Port the recursive-enum / `Box` linked-list testcase. Single `rust-main` snippet, ~70 lines. Upstream opens with `use crate::List::*;` which I expected might trip evcxr (no obvious `crate` namespace for snippets).

## What happened

Worked unchanged. evcxr's wrapping makes `crate::` resolve to the enclosing scope of the snippet, so `use crate::List::*;` brings `Cons` and `Nil` into scope just like upstream intends. Output matches verbatim:

```
linked list has length: 3
3, 2, 1, Nil
```

## What I learned

`use crate::X::*;` is portable into evcxr-snippet land. Good to know — future chapters that use that idiom (there are a few in the std section) don't need code changes.

## Follow-ups

- None.
