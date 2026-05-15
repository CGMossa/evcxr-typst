// evcxr · embed Rust evaluation in Typst documents
//
// Public API per:
//   D-012  function names & default render mode (originally show:)
//   D-013  dep() inline-anywhere
//   D-015  rust-data() failure shape
//   D-017  per-snippet timeout: kwarg
//   D-019  schema versioning + min-cli mechanism
//   D-021  rename show: → render: (Typst reserves `show`)
// Spec lives at docs/design/package-api.md (in the source repo, not shipped).
//
// T-I04: MIME passthrough wired. rust-display reads the per-snippet manifest
// to know which extensions exist, then serves image() or raw() accordingly.
// rust-data reads .cbor or .json sidecars. rust-html renders HTML verbatim.
// The manifest (written for every successfully evaluated snippet) is the gate:
// lib.typ only calls read() on paths confirmed by the manifest, so missing
// files never trigger a hard Typst error (D-004 invariant preserved).
//
// id: kwarg is required for evaluated output to render. In read-mode, snippets
// without an explicit id: show a visible warning box instead of a placeholder.
// In fallback mode (bare typst compile), id: none is still silently accepted.

#import "fallback.typ"
#import "error.typ"

#let _v = 1

#let _src-text(src) = if type(src) == str { src } else { src.text }

// Global document-order counter shared across all evcxr items (snippets and
// deps). Incremented once per call so the CLI can interleave the two separate
// `typst query` results correctly, even without file-position info.
#let _order = counter("evcxr-doc-order")

#let _emit-snippet(kind, src, id, deps, options) = {
  _order.step()
  context [#metadata((
    v: _v,
    kind: kind,
    id: id,
    src: _src-text(src),
    deps: deps,
    options: options,
    loc: (doc_order: _order.get().first()),
  ))<evcxr-snippet>]
}

// The CLI invokes:
//   typst compile --input evcxr-mode=read --input evcxr-cache=<abs-path> ...
// after writing sidecars. Bare `typst compile` (no --input flags) leaves both
// unset and every _read-* helper falls through to the placeholder (D-004).
#let _evcxr-mode = sys.inputs.at("evcxr-mode", default: "fallback")
#let _evcxr-cache = sys.inputs.at("evcxr-cache", default: "")

// Whether sidecar reading is active.
#let _read-mode = _evcxr-mode == "read" and _evcxr-cache != ""

// Set of snippet IDs that have materialised sidecars this run (T-I06).
// The CLI writes _index.json {"v":1,"available":[...]} after every evaluate,
// listing only cache-hit and successfully-evaluated snippets. IDs absent from
// this list (SkippedNoEval) fall through to the placeholder, so a mixed
// cached/uncached run never calls json() on a missing manifest (D-004 fix).
#let _available-ids = if _read-mode {
  json(_evcxr-cache + "/_index.json").at("available", default: ())
} else {
  ()
}

#let _index-available(id) = id != none and _available-ids.contains(str(id))

// Read the per-snippet manifest JSON.
// Returns the extensions array or () when absent / not available this run.
#let _manifest-exts(id) = {
  if not _read-mode or id == none or not _index-available(id) { return () }
  let path = _evcxr-cache + "/" + str(id) + ".manifest.json"
  json(path).at("extensions", default: ())
}

// Returns the parsed error JSON dict when the snippet has an error sidecar.
#let _check-error(id) = {
  if not _read-mode or id == none or not _index-available(id) { return none }
  let exts = _manifest-exts(id)
  if not exts.contains("error") { return none }
  json(_evcxr-cache + "/" + str(id) + ".error.json")
}

// In read-mode, surface a visible warning when an output-rendering snippet
// is missing an explicit id: kwarg. In fallback mode, return none (silent).
//
// The auto-id gap: the CLI computes a default ID (blake3 hash) and writes
// sidecars under it, but Typst cannot recompute blake3, so id: none always
// falls through to the placeholder — even after evcxr-typst run has
// evaluated the snippet. This warning surfaces the gap at the exact call site.
//
// Applies to: rust, rust-main, rust-out, rust-display, rust-html.
// Does NOT apply to: rust-hidden, rust-data (they render nothing visible).
#let _require-id(kind) = {
  if not _read-mode { return none }
  block(
    stroke: 2pt + orange,
    fill: orange.lighten(88%),
    inset: 0pt,
    radius: 2pt,
    width: 100%,
    {
      block(
        fill: orange,
        inset: (x: 8pt, y: 4pt),
        width: 100%,
        {
          set text(fill: white, size: 0.85em, font: ("DejaVu Sans Mono", "monospace"))
          [*evcxr: missing #raw("id:") kwarg* · #kind]
        },
      )
      block(
        inset: 8pt,
        {
          set text(size: 0.85em, font: ("DejaVu Sans Mono", "monospace"))
          [This snippet is missing a required #raw("id:") keyword argument. \
           Without it, evcxr-typst cannot locate the evaluated sidecar, so \
           the output cannot be rendered even after running \
           #raw("evcxr-typst run --allow-eval"). \
           Add #raw("id:") to the call, for example:]
          linebreak()
          raw("#evcxr." + kind + "(id: \"my-id\", ```rust", lang: "typst", block: true)
        },
      )
    },
  )
}

#let _read-stdout(kind, id, src: none) = {
  if not _read-mode or id == none or not _index-available(id) {
    return fallback.placeholder(kind, id, src: src)
  }
  let err = _check-error(id)
  if err != none { return error.evcxr-error-box(err) }
  // A snippet that evaluates successfully but prints nothing has no .txt sidecar
  // (eval.rs::write_mime_sidecars only writes it when plain_stdout is non-empty).
  // Gate the read on the manifest so we never hit a missing-file hard error
  // (D-004 invariant), matching the rust-display / rust-html / rust-data path.
  let exts = _manifest-exts(id)
  if not exts.contains("txt") { return [] }
  let bytes = read(_evcxr-cache + "/" + str(id) + ".txt")
  raw(str(bytes), block: true)
}

#let _read-display(id, prefer: none, src: none) = {
  if not _read-mode or id == none or not _index-available(id) {
    return fallback.placeholder("rust-display", id, src: src)
  }
  let err = _check-error(id)
  if err != none { return error.evcxr-error-box(err) }
  let exts = _manifest-exts(id)

  // Priority order, honouring prefer:.
  let order = if prefer == "image/png" or prefer == "png" {
    ("png", "svg", "jpg", "html")
  } else if prefer == "image/svg+xml" or prefer == "svg" {
    ("svg", "png", "jpg", "html")
  } else if prefer == "image/jpeg" or prefer == "jpeg" or prefer == "jpg" {
    ("jpg", "png", "svg", "html")
  } else if prefer == "text/html" or prefer == "html" {
    ("html", "png", "svg", "jpg")
  } else {
    ("png", "svg", "jpg", "html")
  }

  let result = none
  for ext in order {
    if exts.contains(ext) and result == none {
      let path = _evcxr-cache + "/" + str(id) + "." + ext
      result = if ext == "html" {
        raw(str(read(path)), lang: "html")
      } else {
        image(path)
      }
    }
  }
  if result == none {
    fallback.placeholder("rust-display", id, src: src)
  } else {
    result
  }
}

#let _read-html(id, src: none) = {
  if not _read-mode or id == none or not _index-available(id) {
    return fallback.placeholder("rust-html", id, src: src)
  }
  let err = _check-error(id)
  if err != none { return error.evcxr-error-box(err) }
  let exts = _manifest-exts(id)
  if not exts.contains("html") {
    return fallback.placeholder("rust-html", id, src: src)
  }
  raw(str(read(_evcxr-cache + "/" + str(id) + ".html")), lang: "html")
}

#let _read-data(id, format: auto) = {
  if not _read-mode or id == none or not _index-available(id) { return none }
  let err = _check-error(id)
  if err != none { return none }
  let exts = _manifest-exts(id)

  // Priority: explicit format → auto-detect (cbor first, then json).
  let want-cbor = format == "cbor" or (format == auto and exts.contains("cbor"))
  let want-json = format == "json" or (format == auto and exts.contains("json"))

  if want-cbor and exts.contains("cbor") {
    cbor(_evcxr-cache + "/" + str(id) + ".cbor")
  } else if want-json and exts.contains("json") {
    json(_evcxr-cache + "/" + str(id) + ".json")
  } else {
    none
  }
}

#let setup(
  min-cli: none,
  default-render: "both",
  fallback: auto,
) = {
  [#metadata((
    v: _v,
    kind: "setup",
    min-cli: min-cli,
    default-render: default-render,
    fallback: fallback,
  ))<evcxr-setup>]
  if min-cli != none {
    [#metadata(min-cli)<evcxr-min-cli>]
  }
}

// rust: render a Rust snippet with its captured stdout below.
//
// The id: kwarg is required for the CLI to locate the evaluated sidecar.
// Omitting id: is silently OK in fallback mode (bare typst compile), but
// shows a visible warning box in read-mode (after evcxr-typst run).
#let rust(src, id: none, deps: (), render: auto, timeout: auto, caption: none) = {
  _emit-snippet("rust", src, id, deps, (
    render: render, timeout: timeout, caption: caption,
  ))
  raw(_src-text(src), lang: "rust", block: true)
  let warn = _require-id("rust")
  if warn != none { warn } else {
    _read-stdout("rust", id, src: _src-text(src))
  }
}

// rust-main: like rust, but the CLI appends a hidden `main();` call.
//
// The id: kwarg is required for the CLI to locate the evaluated sidecar.
#let rust-main(src, id: none, deps: (), render: auto, timeout: auto, caption: none) = {
  _emit-snippet("rust-main", src, id, deps, (
    render: render, timeout: timeout, caption: caption,
    auto-call: "main",
  ))
  raw(_src-text(src), lang: "rust", block: true)
  let warn = _require-id("rust-main")
  if warn != none { warn } else {
    _read-stdout("rust-main", id, src: _src-text(src))
  }
}

// rust-out: render only the captured stdout (no code block).
//
// The id: kwarg is required for the CLI to locate the evaluated sidecar.
#let rust-out(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-out", src, id, deps, (timeout: timeout))
  let warn = _require-id("rust-out")
  if warn != none { warn } else {
    _read-stdout("rust-out", id, src: _src-text(src))
  }
}

// rust-display: render the snippet's display output (image, SVG, or HTML).
//
// The id: kwarg is required for the CLI to locate the evaluated sidecar.
#let rust-display(src, id: none, deps: (), prefer: auto, timeout: auto) = {
  _emit-snippet("rust-display", src, id, deps, (
    prefer: prefer, timeout: timeout,
  ))
  let warn = _require-id("rust-display")
  if warn != none { warn } else {
    _read-display(id, prefer: prefer, src: _src-text(src))
  }
}

// rust-html renders the snippet's HTML output verbatim as a raw block.
// HTML frame rendering (typst html.frame) is intentionally deferred per T-I04.
//
// The id: kwarg is required for the CLI to locate the evaluated sidecar.
#let rust-html(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-display", src, id, deps, (
    prefer: "text/html", timeout: timeout,
  ))
  let warn = _require-id("rust-html")
  if warn != none { warn } else {
    _read-html(id, src: _src-text(src))
  }
}

// rust-hidden: evaluate without rendering anything.
// id: is optional — the CLI computes a default ID even without an explicit one.
// No warning is emitted since there is no visible output to locate.
#let rust-hidden(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-hidden", src, id, deps, (timeout: timeout))
  // renders nothing on purpose
}

// rust-data emits the snippet metadata marker and renders nothing visible.
// The snippet is evaluated by the CLI and its CBOR/JSON output is written to a
// sidecar. To consume the parsed value in Typst, call rust-data-read(id).
//
// Two-call pattern (required by Typst's type system — a function cannot both
// place metadata content in the document AND return a non-content dict value):
//   #evcxr.rust-data(id: "x", ```rust...```)        // emits marker, no visual
//   #let v = evcxr.rust-data-read(id: "x")          // returns dict / array
//
// id: is effectively required to use rust-data-read — rust-data-read(id: none)
// always returns the fallback: value. No warning box here since rust-data
// renders nothing itself; the symptom surfaces at rust-data-read's call site.
#let rust-data(
  src, id: none, deps: (), format: auto, timeout: auto,
) = {
  _emit-snippet("rust-data", src, id, deps, (
    format: format, timeout: timeout,
  ))
  // Error box here because rust-data-read returns a value, not content (D-015).
  let err = _check-error(id)
  if err != none { error.evcxr-error-box(err) }
}

// Read the evaluated sidecar for a rust-data snippet and return the parsed
// Typst value (dict or array). Returns `fallback` when in fallback mode or
// when no sidecar exists.
#let rust-data-read(id: none, format: auto, fallback: (:)) = {
  let result = _read-data(id, format: format)
  if result == none { fallback } else { result }
}

#let dep(spec, version: none, features: (), id: none) = {
  _order.step()
  context [#metadata((
    v: _v,
    kind: "dep",
    id: id,
    spec: spec,
    version: version,
    features: features,
    loc: (doc_order: _order.get().first()),
  ))<evcxr-dep>]
}
