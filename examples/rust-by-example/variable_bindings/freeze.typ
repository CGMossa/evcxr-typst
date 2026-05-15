// Adapted from rust-by-example/variable_bindings/freeze.md (see ../NOTICES.md).

#import "../../../packages/evcxr/lib.typ" as evcxr

== Freezing

When data is bound by the same name immutably, it also _freezes_. _Frozen_ data can't be modified until the immutable binding goes out of scope:

Upstream tags this `ignore,mdbook-runnable` because the `_mutable_integer = 50;` line inside the inner block is the deliberate compile error: the inner immutable shadowing has *frozen* `_mutable_integer` for that scope. Rendered source-only.

```rust
fn main() {
    let mut _mutable_integer = 7i32;

    {
        // Shadowing by immutable `_mutable_integer`
        let _mutable_integer = _mutable_integer;

        // Error! `_mutable_integer` is frozen in this scope
        _mutable_integer = 50;
        // FIXME ^ Comment out this line

        // `_mutable_integer` goes out of scope
    }

    // Ok! `_mutable_integer` is not frozen in this scope
    _mutable_integer = 3;
}
```
