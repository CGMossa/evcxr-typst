# `variable_bindings` chapter (top-level)

**Date:** 2026-05-15
**Branch:** rbe/variable-bindings
**Upstream source:** `.rust-by-example/src/variable_bindings.md` (snapshot 898f0ac)

## What I tried

Port the Variable Bindings section opener — first chapter of D-022's Phase B2. Single `rust-main` snippet, no top-level items.

## What happened

Watch eval matched upstream:

```
An integer: 1
A boolean: true
Meet the unit value: ()
```

Upstream's `noisy_unused_variable` line is in the snippet as-is; rustc/evcxr emit a warning to stderr (not captured in the `.txt` sidecar), exactly as upstream documents.

## Follow-ups

- None.
