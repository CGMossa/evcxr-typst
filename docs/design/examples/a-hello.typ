#import "@local/evcxr:0.1.0": rust

= Hello, evcxr-typst

This is the smallest possible document that proves the integration is alive.
The single Rust snippet below is compiled by `evcxr` when you run
`evcxr-typst run --allow-eval a-hello.typ`. Its captured stdout is written
to a sidecar file and embedded back into the rendered PDF directly under
the snippet.

#rust(```rust
println!("Hello, world!");
```)

If you see "Hello, world!" in a styled output box above, the pipeline
works end-to-end: Typst's `query` found the snippet, the CLI evaluated it,
the sidecar landed on disk, and Typst's incremental rendering picked it up.

If you instead see a placeholder box reading "snippet not yet evaluated",
you compiled with bare `typst compile` --- which is by design (see
DECISIONS.md D-004). Re-run with the CLI and `--allow-eval`.
