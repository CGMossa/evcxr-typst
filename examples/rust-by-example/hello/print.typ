// Adapted from rust-by-example/hello/print.md (see ../NOTICES.md).

#import "../../../packages/evcxr/lib.typ" as evcxr

== Formatted print

Printing is handled by a series of macros defined in `std::fmt`, some of which are:

- `format!`: write formatted text to `String`.
- `print!`: same as `format!` but the text is printed to the console (`io::stdout`).
- `println!`: same as `print!` but a newline is appended.
- `eprint!`: same as `print!` but the text is printed to the standard error (`io::stderr`).
- `eprintln!`: same as `eprint!` but a newline is appended.

All parse text in the same fashion. As a plus, Rust checks formatting correctness at compile time.

Upstream marks this snippet `ignore` because of a deliberate FIXME: `println!("My name is {0}, {1} {0}", "Bond")` is missing the `"James"` argument and will fail to compile as written. Per-chapter fidelity decision (option 2): show `fn main()` faithfully and append a synthetic `main();` call so the snippet evaluates; in the FIXME line, supply the missing `"James"` so the snippet compiles cleanly. The deliberately-commented `Structure(3)` line is preserved verbatim. See `journal/2026-05-10-001-print.md`.

#evcxr.rust(id: "rbe-hello-print", ```rust
fn main() {
    // In general, the `{}` will be automatically replaced with any
    // arguments. These will be stringified.
    println!("{} days", 31);

    // Positional arguments can be used. Specifying an integer inside `{}`
    // determines which additional argument will be replaced. Arguments start
    // at 0 immediately after the format string.
    println!("{0}, this is {1}. {1}, this is {0}", "Alice", "Bob");

    // As can named arguments.
    println!("{subject} {verb} {object}",
             object="the lazy dog",
             subject="the quick brown fox",
             verb="jumps over");

    // Different formatting can be invoked by specifying the format character
    // after a `:`.
    println!("Base 10:               {}",   69420); // 69420
    println!("Base 2 (binary):       {:b}", 69420); // 10000111100101100
    println!("Base 8 (octal):        {:o}", 69420); // 207454
    println!("Base 16 (hexadecimal): {:x}", 69420); // 10f2c

    // You can right-justify text with a specified width. This will
    // output "    1". (Four white spaces and a "1", for a total width of 5.)
    println!("{number:>5}", number=1);

    // You can pad numbers with extra zeroes,
    println!("{number:0>5}", number=1); // 00001
    // and left-adjust by flipping the sign. This will output "10000".
    println!("{number:0<5}", number=1); // 10000

    // You can use named arguments in the format specifier by appending a `$`.
    println!("{number:0>width$}", number=1, width=5);

    // Upstream FIXME: missing "James" argument. Supplied here so the
    // snippet compiles. The journal entry covers the deviation.
    println!("My name is {0}, {1} {0}", "Bond", "James");

    // Only types that implement fmt::Display can be formatted with `{}`. User-
    // defined types do not implement fmt::Display by default.

    #[allow(dead_code)] // disable `dead_code` which warn against unused module
    struct Structure(i32);

    // This will not compile because `Structure` does not implement
    // fmt::Display.
    // println!("This struct `{}` won't print...", Structure(3));
    // TODO ^ Try uncommenting this line

    // For Rust 1.58 and above, you can directly capture the argument from a
    // surrounding variable. Just like the above, this will output
    // "    1", 4 white spaces and a "1".
    let number: f64 = 1.0;
    let width: usize = 5;
    println!("{number:>width$}");
}
main();
```)

`std::fmt` contains many traits which govern the display of text. The base form of two important ones are:

- `fmt::Debug`: uses the `{:?}` marker. Formats text for debugging purposes.
- `fmt::Display`: uses the `{}` marker. Formats text in a more elegant, user-friendly fashion.

Here, we used `fmt::Display` because the standard library provides implementations for these types. To print text for custom types, more steps are required.

Implementing the `fmt::Display` trait automatically implements the `ToString` trait, which allows us to convert the type to `String`.

The `#[allow(dead_code)]` attribute applies only to the item that immediately follows it.

=== Activities

- Fix the issue in the above code (the `"Bond"` FIXME) so it runs without error. — Already fixed in this port; see the chapter header.
- Try uncommenting the line that attempts to format the `Structure` struct (the `TODO` line) and observe how the compile error renders.
- Add a `println!` call that prints `Pi is roughly 3.142` by controlling the number of decimal places. Hint: use `let pi = 3.141592` and consult the `std::fmt` documentation.

=== See also

`std::fmt`, macros, `struct`, traits, and `dead_code`.
