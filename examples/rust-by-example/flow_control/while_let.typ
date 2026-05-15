// Adapted from rust-by-example/flow_control/while_let.md (see ../NOTICES.md).

#import "../../../packages/evcxr/lib.typ" as evcxr

== while let

Similar to `if let`, `while let` can make awkward `match` sequences more tolerable. Consider the following sequence that increments `i`. Source-only — upstream uses bare top-level statements, and evcxr's persistent context can't determine the type of `let mut optional = Some(0);` without an explicit annotation:

```rust
// Make `optional` of type `Option<i32>`
let mut optional = Some(0);

// Repeatedly try this test.
loop {
    match optional {
        // If `optional` destructures, evaluate the block.
        Some(i) => {
            if i > 9 {
                println!("Greater than 9, quit!");
                optional = None;
            } else {
                println!("`i` is `{:?}`. Try again.", i);
                optional = Some(i + 1);
            }
            // ^ Requires 3 indentations!
        },
        // Quit the loop when the destructure fails:
        _ => { break; }
        // ^ Why should this be required? There must be a better way!
    }
}
```

Using `while let` makes this sequence much nicer:

#evcxr.rust-main(id: "rbe-flow-while-let", ```rust
fn main() {
    // Make `optional` of type `Option<i32>`
    let mut optional = Some(0);

    // This reads: "while `let` destructures `optional` into
    // `Some(i)`, evaluate the block (`{}`). Else `break`.
    while let Some(i) = optional {
        if i > 9 {
            println!("Greater than 9, quit!");
            optional = None;
        } else {
            println!("`i` is `{:?}`. Try again.", i);
            optional = Some(i + 1);
        }
        // ^ Less rightward drift and doesn't require
        // explicitly handling the failing case.
    }
    // ^ `if let` had additional optional `else`/`else if`
    // clauses. `while let` does not have these.
}
```)
