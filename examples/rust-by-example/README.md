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

Anything not yet listed is **todo**.

## License attribution

Adapted from rust-by-example, dual MIT / Apache-2.0. See [`NOTICES.md`](NOTICES.md) for the full statement and the upstream commit SHA the port is based on.
