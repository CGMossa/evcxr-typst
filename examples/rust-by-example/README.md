# rust-by-example, ported to Typst (incremental)

A hand-written, incremental port of upstream [rust-by-example](https://github.com/rust-lang/rust-by-example) to Typst documents that evaluate end-to-end through `evcxr-typst`.

This is **not** the deterministic mechanical port described in issue #20 (`tools/rbe-port/`). Chapters here are written by hand, one at a time, so that porting them is itself an exercise in using the tool — and the experience of doing so is captured in `journal/` and distilled into `docs/tutorial/`. See `CLAUDE.md` for the rationale and the invariants chapter files must hold.

## Render the book

From the repo root:

```sh
# Bare Typst (placeholders only, no Rust evaluated).
typst compile --root . examples/rust-by-example/main.typ

# Evaluated end-to-end.
cargo run -p evcxr-typst -- run --allow-eval --root . examples/rust-by-example/main.typ
```

The first run will be slow (cargo / rustc warming up plus crate fetches if any chapter pulls a `:dep`). evcxr's `:cache 500` (already on per `eval.rs`) and the per-snippet CAS (T-I05) make subsequent runs incremental.

## Chapters

Mirrors upstream's `SUMMARY.md` ordering. Add a new row when you port a chapter; remove it from "todo" if it was listed there.

| Upstream path | This repo | Status |
|---|---|---|
| `hello.md` | [`hello.typ`](hello.typ) | ported (2026-05-09) |
| `hello/comment.md` | [`hello/comment.typ`](hello/comment.typ) | ported (2026-05-09) |
| `hello/print.md` | [`hello/print.typ`](hello/print.typ) | ported (2026-05-10) |
| `hello/print/print_debug.md` | [`hello/print/print_debug.typ`](hello/print/print_debug.typ) | ported (2026-05-13) |
| `hello/print/print_display.md` | [`hello/print/print_display.typ`](hello/print/print_display.typ) | ported (2026-05-13) |
| `hello/print/print_display/testcase_list.md` | [`hello/print/print_display/testcase_list.typ`](hello/print/print_display/testcase_list.typ) | ported (2026-05-15) |
| `hello/print/fmt.md` | [`hello/print/fmt.typ`](hello/print/fmt.typ) | ported (2026-05-15) |
| `primitives.md` | [`primitives.typ`](primitives.typ) | ported source-only (2026-05-15) |
| `primitives/literals.md` | [`primitives/literals.typ`](primitives/literals.typ) | ported (2026-05-15) |
| `primitives/tuples.md` | [`primitives/tuples.typ`](primitives/tuples.typ) | ported (2026-05-15) |
| `primitives/array.md` | [`primitives/array.typ`](primitives/array.typ) | ported (2026-05-15) |
| `custom_types.md` | [`custom_types.typ`](custom_types.typ) | ported (2026-05-15) |
| `custom_types/structs.md` | [`custom_types/structs.typ`](custom_types/structs.typ) | ported (2026-05-15) |
| `custom_types/enum.md` | [`custom_types/enum.typ`](custom_types/enum.typ) | ported (2026-05-15) |
| `custom_types/enum/enum_use.md` | [`custom_types/enum/enum_use.typ`](custom_types/enum/enum_use.typ) | ported (2026-05-15) |
| `custom_types/enum/c_like.md` | [`custom_types/enum/c_like.typ`](custom_types/enum/c_like.typ) | ported (2026-05-15) |
| `custom_types/enum/testcase_linked_list.md` | [`custom_types/enum/testcase_linked_list.typ`](custom_types/enum/testcase_linked_list.typ) | ported (2026-05-15) |
| `custom_types/constants.md` | [`custom_types/constants.typ`](custom_types/constants.typ) | ported source-only (2026-05-15) |
| `variable_bindings.md` | [`variable_bindings.typ`](variable_bindings.typ) | ported (2026-05-15) |
| `variable_bindings/mut.md` | [`variable_bindings/mut.typ`](variable_bindings/mut.typ) | ported source-only (2026-05-15) |
| `variable_bindings/scope.md` | [`variable_bindings/scope.typ`](variable_bindings/scope.typ) | ported (2026-05-15) |
| `variable_bindings/declare.md` | [`variable_bindings/declare.typ`](variable_bindings/declare.typ) | ported source-only (2026-05-15) |
| `variable_bindings/freeze.md` | [`variable_bindings/freeze.typ`](variable_bindings/freeze.typ) | ported source-only (2026-05-15) |
| `types.md` | [`types.typ`](types.typ) | ported (2026-05-15) |
| `types/cast.md` | [`types/cast.typ`](types/cast.typ) | ported source-only (2026-05-15) |
| `types/literals.md` | [`types/literals.typ`](types/literals.typ) | ported (2026-05-15) |
| `types/inference.md` | [`types/inference.typ`](types/inference.typ) | ported (2026-05-15) |
| `types/alias.md` | [`types/alias.typ`](types/alias.typ) | ported (2026-05-15) |
| `conversion.md` | [`conversion.typ`](conversion.typ) | ported (2026-05-15) |
| `conversion/from_into.md` | [`conversion/from_into.typ`](conversion/from_into.typ) | ported (2026-05-15) |
| `conversion/try_from_try_into.md` | [`conversion/try_from_try_into.typ`](conversion/try_from_try_into.typ) | ported (2026-05-15) |
| `conversion/string.md` | [`conversion/string.typ`](conversion/string.typ) | ported (2026-05-15) |
| `expression.md` | [`expression.typ`](expression.typ) | ported (2026-05-15) |

Anything not yet listed is **todo**.

## License attribution

Adapted from rust-by-example, dual MIT / Apache-2.0. See [`NOTICES.md`](NOTICES.md) for the full statement and the upstream commit SHA the port is based on.
