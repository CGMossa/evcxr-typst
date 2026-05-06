#import "@local/evcxr:0.1.0": rust, dep

= Pulling a real crate from crates.io

Anything you can `:dep` in evcxr's REPL, you can pull in here. The `dep`
function emits a `<evcxr-dep>` metadata marker; the CLI flushes the
collected deps into the evcxr context before evaluating any snippet.

The first run pays for compilation; the rustc artifact cache (evcxr's
own `:cache`, on by default in evcxr-typst) means subsequent runs are
near-instant.

#dep("regex", "1")

Once `regex` is loaded, every following snippet can `use` it as if it
were always there. We extract the version-tagged words from a string:

#rust(```rust
use regex::Regex;

let re = Regex::new(r"\bv\d+(?:\.\d+){0,2}\b").unwrap();
let text = "shipped v1.2 yesterday, planning v2 and v2.0.1 next";
let hits: Vec<&str> = re.find_iter(text).map(|m| m.as_str()).collect();

println!("{hits:?}");
```)

Worth flagging: external `#[macro_use]` crates do *not* work in evcxr
(see `COMMON.md` "Limitations"). For crates whose primary API is macros
(e.g. `lazy_static`'s `lazy_static!` macro, or `serde_derive`-style
proc-macros), you reach for the function-style alternative or accept the
limitation. Plain function/type APIs --- like `regex` here --- compose
perfectly.
