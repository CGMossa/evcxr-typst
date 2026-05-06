#import "../../packages/evcxr/lib.typ" as evcxr

#evcxr.setup()

= Hello, evcxr-typst

This document evaluates a Rust snippet via evcxr and embeds the captured stdout
inline. Without `evcxr-typst run --allow-eval`, you'll see a placeholder box
where the output would appear.

#evcxr.rust(```rust
println!("Hello from Rust, in a Typst document!");
let answer = 6 * 7;
println!("answer = {answer}");
```)

The snippet defines no items and persists no bindings, so it would still work
correctly under the panic-resets-state caveat (D-011) — there's nothing
downstream to be affected.
