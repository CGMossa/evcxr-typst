# `custom_types.md` + `custom_types/structs` chapters

**Date:** 2026-05-15
**Branch:** rbe/custom-types-structs
**Upstream source:** `.rust-by-example/src/custom_types.md` and `custom_types/structs.md` (snapshot 898f0ac)

## What I tried

`custom_types.md` is a one-paragraph section opener with no code — straight prose port.

`custom_types/structs.md` is a single `rust-main` snippet exercising tuple structs, classic C structs, unit structs, struct update syntax, field destructuring, and tuple-struct destructuring.

The risk going in: this chapter defines `struct Person { name: String, age: u8 }`, which collides with `rbe-hello-print-debug-pretty`'s earlier definition `struct Person<'a> { name: &'a str, age: u8 }`. evcxr's `TypeRedefinedVariablesLost` path could trigger if any live binding references the old `Person`. None does (the prior chapter's `let peter` was scoped to `fn main()` of the rust-main snippet, not top-level). So I sent it.

## What happened

Watch picked up the change and evaluated cleanly:

```
Person { name: "Peter", age: 27 }
point coordinates: (5.2, 0.4)
second point: (10.3, 0.2)
pair contains 1 and 0.1
pair contains 1 and 0.1
```

Output matches upstream verbatim. evcxr silently accepted the type redefinition because no live binding depended on the prior `Person`. No `.error.json` sidecar produced.

## What I learned

Type redefinition is fine when prior live bindings don't reference the type. For rbe chapters, that holds as long as previous `fn main()` blocks (under `rust-main`) kept their `let`-bindings function-local. The earlier source-only convention for explanatory definitions (commit `1ba0906`) helps here: top-level types from the *evaluated* chapters are exactly the ones we knew about, and their reuse can be audited against `grep -rn "^struct \|^enum \|^fn " .rust-by-example/src` before porting each chapter.

## Follow-ups

- None new.
