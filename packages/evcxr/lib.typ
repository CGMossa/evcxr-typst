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

// Phase 1 (T-I03): the CLI invokes
//   typst compile --input evcxr-mode=read --input evcxr-cache=<abs-path> ...
// after writing sidecars; everything else (bare `typst compile`, watch's
// pre-eval pass) leaves `evcxr-mode` unset and lands in the fallback branch
// so a missing sidecar never aborts the build (D-004). Auto-id lookup is a
// Phase 3 cache concern — until then, an unpinned snippet always renders
// the placeholder even after `evcxr-typst run` has populated the cache.
#let _evcxr-mode = sys.inputs.at("evcxr-mode", default: "fallback")
#let _evcxr-cache = sys.inputs.at("evcxr-cache", default: "")

// Whether sidecar reading is active.
#let _read-mode = _evcxr-mode == "read" and _evcxr-cache != ""

// Read the per-snippet manifest JSON.
// Returns the extensions array (e.g. ("png", "txt")) or () when absent.
// The manifest is written for every successfully evaluated snippet (T-I04),
// so in read-mode it is always present for snippets that ran ok.
#let _manifest-exts(id) = {
  if not _read-mode or id == none { return () }
  let path = _evcxr-cache + "/" + str(id) + ".manifest.json"
  json(path).at("extensions", default: ())
}

// Returns the parsed error JSON dict when the snippet has an error sidecar,
// or none when it ran successfully.
#let _check-error(id) = {
  if not _read-mode or id == none { return none }
  let exts = _manifest-exts(id)
  if not exts.contains("error") { return none }
  json(_evcxr-cache + "/" + str(id) + ".error.json")
}

#let _read-stdout(kind, id) = {
  if not _read-mode or id == none {
    return fallback.placeholder(kind, id)
  }
  let err = _check-error(id)
  if err != none { return error.evcxr-error-box(err) }
  let bytes = read(_evcxr-cache + "/" + str(id) + ".txt")
  raw(str(bytes), block: true)
}

#let _read-display(id, prefer: none) = {
  if not _read-mode or id == none {
    return fallback.placeholder("rust-display", id)
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
    // Default: raster images first, then vector, then html
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
    fallback.placeholder("rust-display", id)
  } else {
    result
  }
}

#let _read-html(id) = {
  if not _read-mode or id == none {
    return fallback.placeholder("rust-html", id)
  }
  let err = _check-error(id)
  if err != none { return error.evcxr-error-box(err) }
  let exts = _manifest-exts(id)
  if not exts.contains("html") {
    return fallback.placeholder("rust-html", id)
  }
  raw(str(read(_evcxr-cache + "/" + str(id) + ".html")), lang: "html")
}

#let _read-data(id, format: auto) = {
  if not _read-mode or id == none { return none }
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

#let rust(src, id: none, deps: (), render: auto, timeout: auto, caption: none) = {
  _emit-snippet("rust", src, id, deps, (
    render: render, timeout: timeout, caption: caption,
  ))
  raw(_src-text(src), lang: "rust", block: true)
  _read-stdout("rust", id)
}

#let rust-out(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-out", src, id, deps, (timeout: timeout))
  _read-stdout("rust-out", id)
}

#let rust-display(src, id: none, deps: (), prefer: auto, timeout: auto) = {
  _emit-snippet("rust-display", src, id, deps, (
    prefer: prefer, timeout: timeout,
  ))
  _read-display(id, prefer: prefer)
}

// rust-html renders the snippet's HTML output verbatim as a raw block.
// HTML frame rendering (typst html.frame) is intentionally deferred per T-I04.
#let rust-html(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-display", src, id, deps, (
    prefer: "text/html", timeout: timeout,
  ))
  _read-html(id)
}

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
