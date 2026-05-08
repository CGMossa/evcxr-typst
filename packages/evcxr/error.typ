// Copyright 2026 The evcxr-typst Authors.
// Licensed under MIT OR Apache-2.0.
//
// Typst-side error box rendering for `<id>.error.json` sidecars.
// Called from lib.typ via `_check-error(id)`; never imported by end users.
//
// Known limitation: caret underline uses `h(col * 0.6em)` as a column-width
// approximation for monospace fonts. This is accurate for ASCII-only snippet
// source; CJK or emoji characters before the span will cause visual drift.

#let _phase-label(phase) = if phase == "compile" {
  "rust error"
} else if phase == "runtime-panic" {
  "rust panic"
} else if phase == "dep-resolution" {
  "dep error"
} else if phase == "timeout" {
  "snippet timeout"
} else {
  "internal error"
}

#let _severity-color(sev) = if sev == "error" or sev == "panic" {
  red
} else if sev == "warning" {
  orange
} else {
  yellow
}

// Render a source excerpt with a caret underline at the primary span.
// Lines shown: line_start-1 context line and lines line_start..line_end.
#let _source-excerpt(src, primary) = {
  if src == "" or primary == none { return }
  let lines = src.split("\n")
  let ls = primary.at("line_start", default: 1)
  let le = primary.at("line_end", default: ls)
  let cs = primary.at("col_start", default: 1)
  let ce = primary.at("col_end", default: cs + 1)
  let label = primary.at("label", default: "")
  let ctx-start = calc.max(0, ls - 2)
  let ctx-end = calc.min(lines.len() - 1, le)

  for i in range(ctx-start, ctx-end + 1) {
    let lineno = ctx-start + i - ctx-start + 1 + ctx-start
    let linetext = if i < lines.len() { lines.at(i) } else { "" }
    [#str(lineno) #h(0.5em) #raw(linetext) \ ]
    if (lineno == ls) {
      // Caret underline: approximate column offset with 0.6em per character.
      // WHY: Typst has no character-width measurement API in pure Typst;
      // 0.6em is a reasonable approximation for typical monospace fonts.
      h((cs + 3) * 0.6em)
      [#"^" * calc.max(1, ce - cs) ]
      if label != "" [ *#label* ]
      linebreak()
    }
  }
}

/// Render a styled error box for an `<id>.error.json` dict.
///
/// Parameters:
///   `err`   — the parsed JSON dict from the sidecar
///   `theme` — "auto" | "light" | "dark" (currently unused; reserved for future)
#let evcxr-error-box(err, theme: "auto") = {
  let v = err.at("v", default: 1)
  // Unknown schema version: surface a minimal fallback (D-019).
  if v > 1 {
    block(
      stroke: 1pt + gray,
      fill: silver,
      inset: 6pt,
      radius: 2pt,
      width: 100%,
      [#set text(size: 0.8em); *evcxr-typst:* unknown error schema v#str(v)],
    )
    return
  }

  let phase = err.at("phase", default: "internal")
  let snippet-id = err.at("snippet_id", default: "")
  let errors = err.at("errors", default: ())
  let src = err.at("snippet_src", default: "")
  let first = if errors.len() > 0 { errors.at(0) } else { (:) }
  let sev = first.at("severity", default: "error")
  let border-color = _severity-color(sev)

  block(
    stroke: 2pt + border-color,
    fill: border-color.lighten(90%),
    inset: 0pt,
    radius: 2pt,
    width: 100%,
    {
      // Header bar
      block(
        fill: border-color,
        inset: (x: 8pt, y: 4pt),
        width: 100%,
        {
          set text(fill: white, size: 0.85em, font: ("DejaVu Sans Mono", "monospace"))
          let code = first.at("code", default: none)
          [*#_phase-label(phase)* · snippet #raw(snippet-id)
          #if code != none [ · #raw(code) ]]
        },
      )
      // Body
      block(
        inset: 8pt,
        {
          set text(size: 0.85em, font: ("DejaVu Sans Mono", "monospace"))
          // Main message
          let msg = first.at("message", default: "")
          [*#msg*]
          linebreak()
          // Source excerpt
          let pspan = first.at("primary_span", default: none)
          _source-excerpt(src, pspan)
          // Help messages
          for h in first.at("helps", default: ()) {
            let hmsg = h.at("message", default: "")
            if hmsg != "" {
              [*help:* #hmsg \ ]
            }
            let repl = h.at("suggested_replacement", default: none)
            if repl != none {
              raw(repl, lang: "rust", block: false)
              linebreak()
            }
          }
          // Cross-snippet attribution footer
          let cross = first
            .at("secondary_spans", default: ())
            .filter(s => s.at("is_cross_snippet", default: false))
          if cross.len() > 0 {
            let other = cross.at(0).at("snippet_id", default: "")
            [_see snippet #raw(other)_]
          }
          // evcxr hint
          let hint = first.at("evcxr_hint", default: none)
          if hint != none {
            linebreak()
            [_note: #hint_]
          }
        },
      )
    },
  )
}

/// Minimal note box for cross-snippet attribution (renders at the defining
/// snippet when another snippet's error references it).
#let evcxr-error-note(err) = {
  let first = err.at("errors", default: ((:),)).at(0, default: (:))
  let msg = first.at("message", default: "")
  block(
    stroke: 1pt + yellow,
    fill: yellow.lighten(85%),
    inset: 6pt,
    radius: 2pt,
    width: 100%,
    {
      set text(size: 0.8em, font: ("DejaVu Sans Mono", "monospace"))
      [*note:* #msg]
    },
  )
}
