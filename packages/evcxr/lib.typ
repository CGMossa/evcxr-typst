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
// Status: scaffolding. Functions emit metadata markers and render fallback
// placeholders. Real sidecar consumption ships in T-I02 / T-I03.

#import "fallback.typ"

#let _v = 1

#let _src-text(src) = if type(src) == str { src } else { src.text }

#let _emit-snippet(kind, src, id, deps, options) = [#metadata((
  v: _v,
  kind: kind,
  id: id,
  src: _src-text(src),
  deps: deps,
  options: options,
))<evcxr-snippet>]

// Phase 1 (T-I03): the CLI invokes
//   typst compile --input evcxr-mode=read --input evcxr-cache=<abs-path> ...
// after writing sidecars; everything else (bare `typst compile`, watch's
// pre-eval pass) leaves `evcxr-mode` unset and lands in the fallback branch
// so a missing sidecar never aborts the build (D-004). Auto-id lookup is a
// Phase 3 cache concern — until then, an unpinned snippet always renders
// the placeholder even after `evcxr-typst run` has populated the cache.
#let _evcxr-mode = sys.inputs.at("evcxr-mode", default: "fallback")
#let _evcxr-cache = sys.inputs.at("evcxr-cache", default: "")

#let _read-stdout(kind, id) = {
  if id == none or _evcxr-mode != "read" or _evcxr-cache == "" {
    fallback.placeholder(kind, id)
  } else {
    let bytes = read(_evcxr-cache + "/" + str(id) + ".txt")
    raw(str(bytes), block: true)
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
  fallback.placeholder("rust-display", id)
}

#let rust-hidden(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-hidden", src, id, deps, (timeout: timeout))
  // renders nothing on purpose
}

#let rust-data(
  src, id: none, deps: (), format: auto, fallback: (:), timeout: auto,
) = {
  _emit-snippet("rust-data", src, id, deps, (
    format: format, timeout: timeout,
  ))
  fallback
}

#let dep(spec, version: none, features: (), id: none) = [#metadata((
  v: _v,
  kind: "dep",
  id: id,
  spec: spec,
  version: version,
  features: features,
))<evcxr-dep>]
