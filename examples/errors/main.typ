// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.
//
// Smoke test for T-I07 pretty error rendering.
// Exercises three error classes: compile error, runtime panic, dep resolution.
// Run with: evcxr-typst run examples/errors/main.typ --allow-eval
// Expected: each snippet shows a styled error box; exit code non-zero.

#import "../../packages/evcxr/lib.typ" as evcxr

= Error rendering smoke test

== Compile error

The snippet below has a type mismatch — the CLI should write a `.error.json`
sidecar and lib.typ should render a styled error box.

#evcxr.rust(
  id: "e-compile",
  ```rust
  let x: i32 = "not a number";
  println!("{x}");
  ```,
)

== Runtime panic

#evcxr.rust-out(
  id: "e-panic",
  ```rust
  panic!("deliberate panic for T-I07 test");
  ```,
)

== Nonexistent crate dep

#evcxr.dep("nonexistent-crate-that-does-not-exist-abcxyz", version: "0.0.1", id: "e-dep")

#evcxr.rust-out(
  id: "e-dep-use",
  deps: ("e-dep",),
  ```rust
  println!("this line never runs");
  ```,
)
