#import "@local/evcxr:0.1.0": rust

= When a snippet doesn't compile

A document in active development will sometimes contain Rust that doesn't
compile. We do not want this to abort the whole render; the Typst document
should still produce a PDF, with the broken snippet visibly flagged so the
author can find and fix it.

The snippet below has a deliberate type error: `String::push_str` wants
an `&str`, but we hand it a `String` (returned by `format!`) and forget
the `&`. evcxr's error reporting is borrowed almost wholesale from rustc
itself, so the diagnostic is the same one you'd get at the command line.

#rust(```rust
let mut s = String::new();
s.push_str(format!("answer: {}", 42));
```)

What you see in the rendered document, in place of an output box, is a
red-bordered error box containing:

- a one-line summary (`mismatched types`, `expected &str, found String`),
- the offending line with a caret pointing at the bad expression,
- rustc's `help:` suggestion if there is one (here: "consider borrowing"),
- a footer linking back to the snippet's stable id so you can grep for it.

The exact rendering is the subject of task T-D06. The contract for *this*
example is just: the rest of the document still renders, and the failure
is visually unmistakable.

== After the error

Crucially, snippets *after* the broken one are still evaluated. evcxr's
`CommandContext` rejects the failing input without committing it to state,
so a later snippet that doesn't depend on the broken one runs fine:

#rust(```rust
println!("the report continues");
```)

A snippet that *does* depend on broken state (e.g. it tries to use a
`struct` whose definition failed to compile) will itself error, and its
error message will point at both snippets via the snippet-id footer.
