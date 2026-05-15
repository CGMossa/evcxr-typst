// Adapted from rust-by-example/flow_control/match/guard.md (see ../../NOTICES.md).

#import "../../../../packages/evcxr/lib.typ" as evcxr

=== Guards

A `match` _guard_ can be added to filter the arm.

#evcxr.rust-main(id: "rbe-flow-match-guard", ```rust
#[allow(dead_code)]
enum Temperature {
    Celsius(i32),
    Fahrenheit(i32),
}

fn main() {
    let temperature = Temperature::Celsius(35);
    // ^ TODO try different values for `temperature`

    match temperature {
        Temperature::Celsius(t) if t > 30 => println!("{}C is above 30 Celsius", t),
        // The `if condition` part ^ is a guard
        Temperature::Celsius(t) => println!("{}C is equal to or below 30 Celsius", t),

        Temperature::Fahrenheit(t) if t > 86 => println!("{}F is above 86 Fahrenheit", t),
        Temperature::Fahrenheit(t) => println!("{}F is equal to or below 86 Fahrenheit", t),
    }
}
```)

Note that the compiler won't take guard conditions into account when checking if all patterns are covered by the match expression. The block below is source-only — it deliberately fails to compile (the catch-all is commented out, leaving the match non-exhaustive) to motivate the rule, and evaluating it would replace that pedagogy with an error box.

```rust
fn main() {
    let number: u8 = 4;

    match number {
        i if i == 0 => println!("Zero"),
        i if i > 0 => println!("Greater than zero"),
        // _ => unreachable!("Should never happen."),
        // TODO ^ uncomment to fix compilation
    }
}
```

==== See also

#link("https://doc.rust-lang.org/rust-by-example/primitives/tuples.html")[Tuples], #link("https://doc.rust-lang.org/rust-by-example/custom_types/enum.html")[Enums].
