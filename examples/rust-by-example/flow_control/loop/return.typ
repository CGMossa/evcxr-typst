// Adapted from rust-by-example/flow_control/loop/return.md (see ../../NOTICES.md).

#import "../../../../packages/evcxr/lib.typ" as evcxr

=== Returning from loops

One of the uses of a `loop` is to retry an operation until it succeeds. If the operation returns a value though, you might need to pass it to the rest of the code: put it after the `break`, and it will be returned by the `loop` expression.

#evcxr.rust-main(id: "rbe-flow-loop-return", ```rust
fn main() {
    let mut counter = 0;

    let result = loop {
        counter += 1;

        if counter == 10 {
            break counter * 2;
        }
    };

    assert_eq!(result, 20);
}
```)
