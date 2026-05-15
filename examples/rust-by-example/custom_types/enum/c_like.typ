// Adapted from rust-by-example/custom_types/enum/c_like.md (see ../../NOTICES.md).

#import "../../../../packages/evcxr/lib.typ" as evcxr

=== C-like

`enum` can also be used as C-like enums.

Note: `Color` here redefines the `struct Color` from `hello/print/fmt` (now an `enum`). A later chapter (`flow_control/match/destructuring/destructure_enum.md`) will redefine it once more.

#evcxr.rust-main(id: "rbe-custom-types-enum-c-like", ```rust
// An attribute to hide warnings for unused code.
#![allow(dead_code)]

// enum with implicit discriminator (starts at 0)
enum Number {
    Zero,
    One,
    Two,
}

// enum with explicit discriminator
enum Color {
    Red = 0xff0000,
    Green = 0x00ff00,
    Blue = 0x0000ff,
}

fn main() {
    // `enums` can be cast as integers.
    println!("zero is {}", Number::Zero as i32);
    println!("one is {}", Number::One as i32);

    println!("roses are #{:06x}", Color::Red as u32);
    println!("violets are #{:06x}", Color::Blue as u32);
}
```)

==== See also

Casting.
