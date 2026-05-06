#import "@local/evcxr:0.1.0": rust, rust-out, rust-hidden

= Persistent items: a struct across snippets

evcxr maintains a long-lived `CommandContext` for the duration of a run, so
items defined in one snippet stay defined for every snippet that follows ---
even if "follows" means "twelve pages later, after a chapter break". This
example exercises that across the document.

== Defining the type

We declare a small `User` record. The snippet has no observable output ---
the definition itself is the side effect. Using `rust-hidden` keeps the
rendered document tidy: the code is still shown, but no output box appears.

#rust-hidden(```rust
pub struct User {
    pub name: String,
    pub karma: i64,
}

impl User {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_owned(), karma: 0 }
    }
}
```)

== A digression

In a real document, plenty of prose would sit between definition and use.
Maybe a section on motivation, a figure or two, a footnote about the
borrow checker. The point is: none of that invalidates the type we just
declared --- evcxr's `committed_state` carries `User` forward.

== Using the type, much later

Now, far from the declaration, we instantiate a `User` and exercise the
`impl`. The compiler still knows what `User` is because the previous
snippet committed it.

#rust(```rust
let mut alice = User::new("alice");
alice.karma += 7;
println!("{} has {} karma", alice.name, alice.karma);
```)

This is one of the three cross-snippet composition examples in the
gallery. See also `c-module-across-snippets.typ` and `h-mini-report.typ`.
