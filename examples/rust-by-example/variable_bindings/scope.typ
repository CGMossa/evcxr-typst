// Adapted from rust-by-example/variable_bindings/scope.md (see ../NOTICES.md).

#import "../../../packages/evcxr/lib.typ" as evcxr

== Scope and Shadowing

Variable bindings have a scope, and are constrained to live in a _block_. A block is a collection of statements enclosed by braces `{}`.

Upstream tags this first block `ignore,mdbook-runnable` because the `println!("outer short: …", short_lived_binding)` line is a deliberate "binding doesn't exist in this scope" compile error. Rendered source-only to preserve the lesson.

```rust
fn main() {
    // This binding lives in the main function
    let long_lived_binding = 1;

    // This is a block, and has a smaller scope than the main function
    {
        // This binding only exists in this block
        let short_lived_binding = 2;

        println!("inner short: {}", short_lived_binding);
    }
    // End of the block

    // Error! `short_lived_binding` doesn't exist in this scope
    println!("outer short: {}", short_lived_binding);
    // FIXME ^ Comment out this line

    println!("outer long: {}", long_lived_binding);
}
```

Also, variable shadowing is allowed.

#evcxr.rust-main(id: "rbe-variable-bindings-scope-shadow", ```rust
fn main() {
    let shadowed_binding = 1;

    {
        println!("before being shadowed: {}", shadowed_binding);

        // This binding *shadows* the outer one
        let shadowed_binding = "abc";

        println!("shadowed in inner block: {}", shadowed_binding);
    }
    println!("outside inner block: {}", shadowed_binding);

    // This binding *shadows* the previous binding
    let shadowed_binding = 2;
    println!("shadowed in outer block: {}", shadowed_binding);
}
```)
