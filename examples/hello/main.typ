#import "../../packages/evcxr/lib.typ" as evcxr

#evcxr.setup()

= Hello, evcxr-typst

This document evaluates three Rust snippets via evcxr and embeds their captured
stdout inline. Without `evcxr-typst run --allow-eval`, you'll see placeholder
boxes where the outputs would appear.

== Snippet 1 — establish a binding

#evcxr.rust(id: "hello-1", ```rust
let answer = 6 * 7;
println!("answer = {answer}");
```)

== Snippet 2 — reuse the binding from snippet 1

evcxr keeps `answer` alive across snippets, so this one prints something
derived from it without re-declaring the value.

#evcxr.rust(id: "hello-2", ```rust
println!("twice the answer = {}", answer * 2);
```)

== Snippet 3 — define an item, call it

This snippet defines a function and invokes it. Items committed by earlier
snippets remain in scope for later ones (D-001 / snippet-semantics § "Items").

#evcxr.rust(id: "hello-3", ```rust
fn shout(s: &str) -> String { s.to_uppercase() + "!" }
println!("{}", shout("hello, world"));
```)
