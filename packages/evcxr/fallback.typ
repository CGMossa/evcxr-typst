// Placeholder rendering when no sidecar exists.
// D-004: bare `typst compile` must succeed and produce a sensible PDF
// without any Rust having been evaluated.

#let placeholder(kind, id) = block(
  fill: rgb("fffae6"),
  stroke: 0.5pt + rgb("ffaa00"),
  inset: 6pt,
  radius: 2pt,
  width: 100%,
  [
    #set text(font: ("DejaVu Sans Mono", "monospace"), size: 0.8em)
    *evcxr-typst placeholder* · #kind · id: #if id == none { [_auto_] } else { raw(str(id)) } \
    Run `evcxr-typst run --allow-eval <doc>.typ` to evaluate the snippet.
  ],
)
