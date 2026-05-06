# CLAUDE.md — `examples/`

End-to-end Typst documents that exercise the integration. Each subdirectory is one self-contained example with its own `main.typ` and any sidecar assets.

`hello/` is the Phase 1 smoke test. The full gallery design (`docs/design/examples/`) sketches the eight scenarios we want to ship; flesh them out as their corresponding implementation tasks land:

| Subdir         | Implements gallery scenario | Unblocked by |
|----------------|------------------------------|---------------|
| `hello/`       | `a-hello.typ`                 | T-I03 |
| `struct/`      | `b-struct-across-snippets.typ`| T-I03 |
| `module/`      | `c-module-across-snippets.typ`| T-I03 |
| `image/`       | `d-image-output.typ`          | T-I04 (MIME passthrough) |
| `crate-dep/`   | `e-cratesio-dep.typ`          | T-I03 |
| `async/`       | `f-async-tokio.typ`           | T-I03 |
| `error/`       | `g-error-case.typ`            | T-I07 (pretty errors) |
| `mini-report/` | `h-mini-report.typ`           | T-I03 then exercised by T-I05 (watch) |

Don't pre-create empty subdirs; add them as their corresponding implementation tasks become real.
