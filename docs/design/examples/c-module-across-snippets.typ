#import "@local/evcxr:0.1.0": rust, rust-hidden

= Persistent modules and `use`

`mod` blocks and `use` statements persist across snippets exactly the same
way `struct` and `fn` do. evcxr merges `use`-trees across snippets
(see `evcxr/src/use_trees.rs`), so an import made early in the document
remains in scope for every later snippet.

== Defining a module

We define a tiny `geom` module with a free function. No output --- the
definition is the point.

#rust-hidden(```rust
mod geom {
    pub fn area_of_circle(r: f64) -> f64 {
        std::f64::consts::PI * r * r
    }

    pub fn area_of_square(side: f64) -> f64 {
        side * side
    }
}
```)

== Bringing items into scope

A separate snippet `use`s items from `geom`. The `use` is committed to
context state, so subsequent snippets get the unqualified names too.

#rust-hidden(```rust
use geom::{area_of_circle, area_of_square};
```)

== Using the module

Now we call the imported functions directly --- no `geom::` prefix needed,
because the previous `use` is still live.

#rust(```rust
let a = area_of_circle(2.0);
let b = area_of_square(3.0);
println!("circle: {a:.4}, square: {b:.1}");
```)

This document spans three snippets that compose: a `mod` definition, a
`use` import, and a final consumer. All three rely on evcxr's persistent
item table. (Cross-snippet composition example #2 of 3+.)
