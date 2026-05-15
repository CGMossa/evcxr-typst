// Fixture for crates/evcxr-typst/tests/no_stdout_sidecar.rs.
// Single `evcxr.rust(...)` snippet that defines a binding and prints nothing.
// The test hand-crafts a cache that says the snippet evaluated (manifest with
// `extensions: []`) and asserts bare `typst compile` in read mode succeeds.

#import "/packages/evcxr/lib.typ" as evcxr

#evcxr.rust(id: "no-stdout-snippet", ```rust
let x = 1;
```)
