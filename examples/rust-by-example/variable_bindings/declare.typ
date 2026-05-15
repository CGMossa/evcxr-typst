// Adapted from rust-by-example/variable_bindings/declare.md (see ../NOTICES.md).

#import "../../../packages/evcxr/lib.typ" as evcxr

== Declare first

It is possible to declare variable bindings first and initialize them later, but all variable bindings must be initialized before they are used: the compiler forbids use of uninitialized variable bindings, as it would lead to undefined behavior.

It is not common to declare a variable binding and initialize it later in the function. It is more difficult for a reader to find the initialization when initialization is separated from declaration. It is common to declare and initialize a variable binding near where the variable will be used.

Upstream tags this `ignore,mdbook-runnable` because the `println!("another binding: {}", another_binding)` line reads an uninitialized binding — a deliberate compile error the chapter teaches. Rendered source-only.

```rust
fn main() {
    // Declare a variable binding
    let a_binding;

    {
        let x = 2;

        // Initialize the binding
        a_binding = x * x;
    }

    println!("a binding: {}", a_binding);

    let another_binding;

    // Error! Use of uninitialized binding
    println!("another binding: {}", another_binding);
    // FIXME ^ Comment out this line

    another_binding = 1;

    println!("another binding: {}", another_binding);
}
```
