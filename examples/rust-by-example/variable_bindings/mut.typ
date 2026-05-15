// Adapted from rust-by-example/variable_bindings/mut.md (see ../NOTICES.md).

#import "../../../packages/evcxr/lib.typ" as evcxr

== Mutability

Variable bindings are immutable by default, but this can be overridden using the `mut` modifier.

Upstream tags this `ignore,mdbook-runnable` because the `_immutable_binding += 1;` line is a deliberate compile error — the chapter is teaching mutability. Rendered source-only to preserve the lesson.

```rust
fn main() {
    let _immutable_binding = 1;
    let mut mutable_binding = 1;

    println!("Before mutation: {}", mutable_binding);

    // Ok
    mutable_binding += 1;

    println!("After mutation: {}", mutable_binding);

    // Error! Cannot assign a new value to an immutable variable
    _immutable_binding += 1;
}
```

The compiler will throw a detailed diagnostic about mutability errors.
