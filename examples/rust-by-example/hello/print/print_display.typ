// Adapted from rust-by-example/hello/print/print_display.md (see ../../NOTICES.md).

#import "../../../../packages/evcxr/lib.typ" as evcxr

=== Display

`fmt::Debug` hardly looks compact and clean, so it is often advantageous to customize the output appearance. This is done by manually implementing `fmt::Display`, which uses the `{}` print marker.

This first block is also rendered source-only. In a single long evcxr session it would redefine `Structure` from the previous `Debug` chapter and break the already-persisted `Deep(Structure)` example.

#let display-structure-src = ```rust
// Import (via `use`) the `fmt` module to make it available.
use std::fmt;

// Define a structure for which `fmt::Display` will be implemented. This is
// a tuple struct named `Structure` that contains an `i32`.
struct Structure(i32);

// To use the `{}` marker, the trait `fmt::Display` must be implemented
// manually for the type.
impl fmt::Display for Structure {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "{}", self.0)
    }
}
```

#raw(display-structure-src.text, lang: "rust", block: true)

`fmt::Display` may be cleaner than `fmt::Debug`, but this presents a problem for the standard library. How should ambiguous types be displayed? For example, if the standard library implemented a single style for all `Vec<T>`, what style should it be?

- `Vec<path>`: `/:/etc:/home/username:/bin` (split on `:`)
- `Vec<number>`: `1,2,3` (split on `,`)

No, because there is no ideal style for all types and the standard library does not presume to dictate one. `fmt::Display` is not implemented for `Vec<T>` or for any other generic containers. `fmt::Debug` must then be used for these generic cases.

This is not a problem though because for any new container type which is not generic, `fmt::Display` can be implemented.

#evcxr.rust-main(id: "rbe-hello-print-display-minmax", ```rust
use std::fmt; // Import `fmt`

// A structure holding two numbers. `Debug` will be derived so the results can
// be contrasted with `Display`.
#[derive(Debug)]
struct MinMax(i64, i64);

// Implement `Display` for `MinMax`.
impl fmt::Display for MinMax {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Use `self.number` to refer to each positional data point.
        write!(f, "({}, {})", self.0, self.1)
    }
}

// Define a structure where the fields are nameable for comparison.
#[derive(Debug)]
struct Point2D {
    x: f64,
    y: f64,
}

// Similarly, implement `Display` for `Point2D`.
impl fmt::Display for Point2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Customize so only `x` and `y` are denoted.
        write!(f, "x: {}, y: {}", self.x, self.y)
    }
}

fn main() {
    let minmax = MinMax(0, 14);

    println!("Compare structures:");
    println!("Display: {}", minmax);
    println!("Debug: {:?}", minmax);

    let big_range =   MinMax(-300, 300);
    let small_range = MinMax(-3, 3);

    println!("The big range is {big} and the small is {small}",
             small = small_range,
             big = big_range);

    let point = Point2D { x: 3.3, y: 7.2 };

    println!("Compare points:");
    println!("Display: {}", point);
    println!("Debug: {:?}", point);

    // The following line would not compile: both `Debug` and `Display`
    // were implemented, but `{:b}` requires `fmt::Binary` to be
    // implemented, which it hasn't been for `Point2D`.
    // println!("What does Point2D look like in binary: {:b}?", point);
}
```)

So, `fmt::Display` has been implemented but `fmt::Binary` has not, and therefore cannot be used. `std::fmt` has many such traits and each requires its own implementation.

==== Activity

After checking the output of the above example, use the `Point2D` struct as a guide to add a `Complex` struct to the example. When printed in the same way, the output should be:

```text
Display: 3.3 +7.2i
Debug: Complex { real: 3.3, imag: 7.2 }

Display: 4.7 -2.3i
Debug: Complex { real: 4.7, imag: -2.3 }
```

Bonus: Add a space after the `+`/`-` signs.

==== See also

`derive`, `std::fmt`, macros, `struct`, trait, and `use`.
