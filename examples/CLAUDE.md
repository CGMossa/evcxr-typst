# CLAUDE.md — `examples/`

End-to-end Typst documents that exercise the integration. Each subdirectory is one self-contained example with its own `main.typ` and any sidecar assets.

`hello/` is the Phase 1 smoke test; `image/` exercises MIME passthrough (T-I04); `errors/` exercises pretty error rendering (T-I07). All three exist and run end-to-end. The full gallery design (`docs/design/examples/`) sketches the eight scenarios we want to ship; remaining examples land as later implementation tasks complete:

| Subdir         | Implements gallery scenario   | Status                       |
|----------------|-------------------------------|------------------------------|
| `hello/`       | `a-hello.typ`                 | done (T-I03)                 |
| `image/`       | `d-image-output.typ`          | done (T-I04)                 |
| `errors/`      | `g-error-case.typ`            | done (T-I07)                 |
| `struct/`      | `b-struct-across-snippets.typ`| not yet created              |
| `module/`      | `c-module-across-snippets.typ`| not yet created              |
| `crate-dep/`   | `e-cratesio-dep.typ`          | not yet created              |
| `async/`       | `f-async-tokio.typ`           | not yet created              |
| `mini-report/` | `h-mini-report.typ`           | not yet created              |

Don't pre-create empty subdirs; add them as their corresponding implementation tasks complete.

## `examples/rust-by-example/` (active, hand-port path)

Side track issue #20. The directory now exists and chapters are landing on `track/rbe-incremental` — currently `hello.typ` and `hello/comment.typ`, with the working log in `journal/`. See `examples/rust-by-example/CLAUDE.md` for the chapter-file invariants and `examples/rust-by-example/README.md` for the chapter index.

Two paths coexist for the rbe port; only one is active right now:

- **Hand-port** (active). `examples/rust-by-example/` contains hand-written chapters, one at a time, with the experience captured in `journal/`. See `docs/tracks/rust-by-example-port.md` § "How this differs" for the rationale (learning the tool through use; surfacing real bugs that the test suite missed — e.g. PR #27 and #28 both came out of this loop).
- **Mechanical porter** (designed, not yet started). `tools/rbe-port/` would deterministically convert `.md` → `.typ`. See `docs/tracks/rust-by-example-port.md` for the design. When/if this ships, the porter can backfill chapters not yet covered by hand or replace the directory wholesale.

License attribution for both paths lives in `examples/rust-by-example/NOTICES.md`.
