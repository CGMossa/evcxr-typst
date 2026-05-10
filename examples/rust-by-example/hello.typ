// Adapted from rust-by-example/hello.md (see NOTICES.md).

// Each chapter imports the package independently because `#include` does not
// share scope with the entry file (see journal/2026-05-09-001-hello.md).
#import "../../packages/evcxr/lib.typ" as evcxr

= Hello World

This is the source code of the traditional Hello World program.

Upstream wraps the body in `fn main() { ... }`; we drop the wrapper here because evcxr executes top-level statements directly. The visible-source fidelity gap is tracked by T-B01 (`rust-main`); see `journal/2026-05-09-001-hello.md` for the decision.

#evcxr.rust(id: "rbe-hello", ```rust
// This is a comment, and is ignored by the compiler.

// Print text to the console.
println!("Hello World!");
```)

`println!` is a _macro_ that prints text to the console.

A binary can be generated using the Rust compiler `rustc`. (Not relevant in evcxr's eval-context model — we're running statements, not building a binary — but the upstream chapter shows it for context.)

```bash
$ rustc hello.rs
```

`rustc` would produce a `hello` binary that can be executed.

```bash
$ ./hello
Hello World!
```

== Activity

Upstream invites you to add a second `println!`. Try it: edit the snippet above and re-run `evcxr-typst run --allow-eval`. The expected output becomes:

```text
Hello World!
I'm a Rustacean!
```
