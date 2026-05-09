// Adapted from rust-by-example/hello/comment.md (see ../NOTICES.md).

#import "../../../packages/evcxr/lib.typ" as evcxr

== Comments

Any program requires comments, and Rust supports a few different varieties:

=== Regular comments

Ignored by the compiler:

- *Line comments* start with `//` and continue to the end of the line.
- *Block comments* are enclosed in `/* ... */` and can span multiple lines.

=== Documentation comments

Parsed into HTML library documentation:

- `///` generates docs for the item that follows it.
- `//!` generates docs for the enclosing item (typically used at the top of a file or module).

The upstream chapter wraps the snippet below in `fn main() { ... }`. We drop the wrapper for the same reason as the hello chapter — evcxr executes top-level statements directly. See `journal/2026-05-09-001-hello.md` for the convention.

#evcxr.rust(id: "rbe-hello-comment", ```rust
// Line comments start with two slashes.
// Everything after the slashes is ignored by the compiler.

// Example: This line won't execute
// println!("Hello, world!");

// Try removing the slashes above and running the code again.

/*
 * Block comments are useful for temporarily disabling code.
 * They can also be nested: /* like this */ which makes it easy
 * to comment out large sections quickly.
 */

/*
Note: The asterisk column on the left is just for style -
it's not required by the language.
*/

// Block comments make it easy to toggle code on/off by adding
// or removing just one slash:

/* <- Add a '/' here to uncomment the entire block below

println!("Now");
println!("everything");
println!("executes!");
// Line comments inside remain unaffected

// */

// Block comments can also be used within expressions:
let x = 5 + /* 90 + */ 5;
println!("Is `x` 10 or 100? x = {}", x);
```)
