// Adapted from rust-by-example/flow_control/match/destructuring/destructure_enum.md (see ../../../NOTICES.md).

#import "../../../../../packages/evcxr/lib.typ" as evcxr

==== enums

An `enum` is destructured similarly. Note: `Color` here is redefined once more (it has been a `struct` in `hello/print/fmt` and a small C-like `enum` in `custom_types/enum/c_like`); the variants here are the upstream ones for this chapter.

#evcxr.rust-main(id: "rbe-flow-match-destructure-enum", ```rust
// `allow` required to silence warnings because only
// one variant is used.
#[allow(dead_code)]
enum Color {
    // These 3 are specified solely by their name.
    Red,
    Blue,
    Green,
    // These likewise tie `u32` tuples to different names: color models.
    RGB(u32, u32, u32),
    HSV(u32, u32, u32),
    HSL(u32, u32, u32),
    CMY(u32, u32, u32),
    CMYK(u32, u32, u32, u32),
}

fn main() {
    let color = Color::RGB(122, 17, 40);
    // TODO ^ Try different variants for `color`

    println!("What color is it?");
    // An `enum` can be destructured using a `match`.
    match color {
        Color::Red   => println!("The color is Red!"),
        Color::Blue  => println!("The color is Blue!"),
        Color::Green => println!("The color is Green!"),
        Color::RGB(r, g, b) =>
            println!("Red: {}, green: {}, and blue: {}!", r, g, b),
        Color::HSV(h, s, v) =>
            println!("Hue: {}, saturation: {}, value: {}!", h, s, v),
        Color::HSL(h, s, l) =>
            println!("Hue: {}, saturation: {}, lightness: {}!", h, s, l),
        Color::CMY(c, m, y) =>
            println!("Cyan: {}, magenta: {}, yellow: {}!", c, m, y),
        Color::CMYK(c, m, y, k) =>
            println!("Cyan: {}, magenta: {}, yellow: {}, key (black): {}!",
                c, m, y, k),
        // Don't need another arm because all variants have been examined
    }
}
```)

===== See also

#link("https://doc.rust-lang.org/rust-by-example/attribute/unused.html")[`#[allow(...)]`], #link("https://en.wikipedia.org/wiki/Color_model")[color models], and #link("https://doc.rust-lang.org/rust-by-example/custom_types/enum.html")[`enum`].
