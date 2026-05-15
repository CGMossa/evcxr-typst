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
// render: / caption: kwargs are now live in rust() and rust-main():
//   render: "source"  — code block only; output block suppressed
//   render: "output"  — output block only; code block suppressed
//   render: "both"    — both (default)
//   render: auto      — treated as "both" (setup(default-render:) is not
//                        accessible at Typst render time; see comment on setup)
//   caption:          — wraps code + output in a Typst figure with the caption
//
// setup(default-render:) is forwarded to the CLI via <evcxr-setup> metadata
// but is not interpreted at Typst render time. Use render: per-call to control
// render mode per snippet.
//
// setup(fallback:) is accepted for forward-compatibility but has no effect.

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

// Resolve render: kwarg to an effective mode string.
// render: auto is treated as "both" — setup(default-render:) is forwarded
// to the CLI via metadata but is not accessible at Typst render time.
#let _effective-render(render) = {
  if render == "source" { "source" }
  else if render == "output" { "output" }
  else { "both" }
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

// setup: document-level configuration.
//
// min-cli: "X.Y.Z" — require at least this CLI version. The CLI reads the
//   resulting <evcxr-min-cli> marker and exits 2 if it is too old (D-019).
//
// default-render: "both" | "source" | "output" — forwarded to the CLI via
//   <evcxr-setup> metadata. NOT interpreted at Typst render time (lib.typ
//   cannot read its own metadata markers at render time). Use render: per-call
//   to control which blocks a specific snippet renders.
//
// fallback: accepted for forward-compatibility; currently has no effect.
//   The fallback rendering shape is controlled by fallback.typ, not this kwarg.
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

// rust: render a Rust snippet with (optionally) its captured stdout.
//
// render: controls which blocks appear:
//   "both"   — code block + output block (default when render: is auto or "both")
//   "source" — code block only; output suppressed
//   "output" — output block only; code block suppressed
// Note: setup(default-render:) is not applied here; auto is treated as "both".
//
// caption: wraps both visible blocks in a Typst figure with the given caption.
#let rust(src, id: none, deps: (), render: auto, timeout: auto, caption: none) = {
  _emit-snippet("rust", src, id, deps, (
    render: render, timeout: timeout, caption: caption,
  ))
  let eff = _effective-render(render)
  let body = {
    if eff == "source" or eff == "both" {
      raw(_src-text(src), lang: "rust", block: true)
    }
    if eff == "output" or eff == "both" {
      _read-stdout("rust", id, src: _src-text(src))
    }
  }
  if caption != none {
    figure(body, caption: caption)
  } else {
    body
  }
}

// rust-main: like rust, but the CLI appends a hidden `main();` call.
//
// render: and caption: have the same semantics as in rust().
#let rust-main(src, id: none, deps: (), render: auto, timeout: auto, caption: none) = {
  _emit-snippet("rust-main", src, id, deps, (
    render: render, timeout: timeout, caption: caption,
    auto-call: "main",
  ))
  let eff = _effective-render(render)
  let body = {
    if eff == "source" or eff == "both" {
      raw(_src-text(src), lang: "rust", block: true)
    }
    if eff == "output" or eff == "both" {
      _read-stdout("rust-main", id, src: _src-text(src))
    }
  }
  if caption != none {
    figure(body, caption: caption)
  } else {
    body
  }
}

// rust-out: render only the captured stdout (no code block).
// render: and caption: are not on this function's signature — it is always
// output-only by design. Use rust() with render: "output" for a more
// flexible alternative.
#let rust-out(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-out", src, id, deps, (timeout: timeout))
  _read-stdout("rust-out", id, src: _src-text(src))
}

// rust-display: render the snippet's display output (image, SVG, or HTML).
#let rust-display(src, id: none, deps: (), prefer: auto, timeout: auto) = {
  _emit-snippet("rust-display", src, id, deps, (
    prefer: prefer, timeout: timeout,
  ))
  _read-display(id, prefer: prefer, src: _src-text(src))
}

// rust-html renders the snippet's HTML output verbatim as a raw block.
// HTML frame rendering (typst html.frame) is intentionally deferred per T-I04.
#let rust-html(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-display", src, id, deps, (
    prefer: "text/html", timeout: timeout,
  ))
  _read-html(id, src: _src-text(src))
}

// rust-hidden: evaluate without rendering anything.
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
