// Adapted from rust-by-example/flow_control/loop/nested.md (see ../../NOTICES.md).

#import "../../../../packages/evcxr/lib.typ" as evcxr

=== Nesting and labels

It's possible to `break` or `continue` outer loops when dealing with nested loops. In these cases, the loops must be annotated with some `'label`, and the label must be passed to the `break`/`continue` statement.

#evcxr.rust-main(id: "rbe-flow-loop-nested", ```rust
#![allow(unreachable_code, unused_labels)]

fn main() {
    'outer: loop {
        println!("Entered the outer loop");

        'inner: loop {
            println!("Entered the inner loop");

            // This would break only the inner loop
            //break;

            // This breaks the outer loop
            break 'outer;
        }

        println!("This point will never be reached");
    }

    println!("Exited the outer loop");
}
```)
