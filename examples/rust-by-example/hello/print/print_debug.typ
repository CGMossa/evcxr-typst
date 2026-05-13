// Adapted from rust-by-example/hello/print/print_debug.md (see ../../NOTICES.md).

#import "../../../../packages/evcxr/lib.typ" as evcxr

=== Debug

All types which want to use `std::fmt` formatting traits require an implementation to be printable. Automatic implementations are only provided for types such as those in the standard library. All others must be manually implemented somehow.

The `fmt::Debug` trait makes this very straightforward. All types can derive, automatically creating the `fmt::Debug` implementation. This is not true for `fmt::Display`, which must be manually implemented.

This first block defines two structures and has no stdout of its own; a successful run materializes only its manifest sidecar.

#let debug-derive-src = ```rust
// This structure cannot be printed either with `fmt::Display` or
// with `fmt::Debug`.
struct UnPrintable(i32);

// The `derive` attribute automatically creates the implementation
// required to make this `struct` printable with `fmt::Debug`.
#[derive(Debug)]
struct DebugPrintable(i32);
```

#raw(debug-derive-src.text, lang: "rust", block: true)
#evcxr.rust-hidden(debug-derive-src, id: "rbe-hello-print-debug-derive")

All standard-library types are automatically printable with `{:?}` too.

The upstream chapter wraps the snippets below in `fn main() { ... }`. We keep the wrapper and use `evcxr.rust-main(...)` so evcxr invokes `main()` while local bindings stay scoped to the function body. See `journal/2026-05-13-001-print-debug.md`.

#evcxr.rust-main(id: "rbe-hello-print-debug-std", ```rust
// Derive the `fmt::Debug` implementation for `Structure`. `Structure`
// is a structure which contains a single `i32`.
#[derive(Debug)]
struct Structure(i32);

// Put a `Structure` inside of the structure `Deep`. Make it printable
// also.
#[derive(Debug)]
struct Deep(Structure);

fn main() {
    // Printing with `{:?}` is similar to with `{}`.
    println!("{:?} months in a year.", 12);
    println!("{1:?} {0:?} is the {actor:?} name.",
             "Slater",
             "Christian",
             actor="actor's");

    // `Structure` is printable!
    println!("Now {:?} will print!", Structure(3));

    // The problem with `derive` is there is no control over how
    // the results look. What if I want this to just show a `7`?
    println!("Now {:?} will print!", Deep(Structure(7)));
}
```)

So `fmt::Debug` definitely makes this printable but sacrifices some elegance. Rust also provides "pretty printing" with `{:#?}`.

#evcxr.rust-main(id: "rbe-hello-print-debug-pretty", ```rust
#[derive(Debug)]
struct Person<'a> {
    name: &'a str,
    age: u8
}

fn main() {
    let name = "Peter";
    let age = 27;
    let peter = Person { name, age };

    // Pretty print
    println!("{:#?}", peter);
}
```)

One can manually implement `fmt::Display` to control the display.

==== See also

Attributes, `derive`, `std::fmt`, and `struct`.
