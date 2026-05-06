// evcxr · embed Rust evaluation in Typst documents
//
// Public API per:
//   D-012  function names & default show:
//   D-013  dep() inline-anywhere
//   D-015  rust-data() failure shape
//   D-017  per-snippet timeout: kwarg
//   D-019  schema versioning + min-cli mechanism
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

#let setup(
  min-cli: none,
  default-show: "both",
  fallback: auto,
) = {
  [#metadata((
    v: _v,
    kind: "setup",
    min-cli: min-cli,
    default-show: default-show,
    fallback: fallback,
  ))<evcxr-setup>]
  if min-cli != none {
    [#metadata(min-cli)<evcxr-min-cli>]
  }
}

#let rust(src, id: none, deps: (), show: auto, timeout: auto, caption: none) = {
  _emit-snippet("rust", src, id, deps, (
    show: show, timeout: timeout, caption: caption,
  ))
  fallback.placeholder("rust", id)
}

#let rust-out(src, id: none, deps: (), timeout: auto) = {
  _emit-snippet("rust-out", src, id, deps, (timeout: timeout))
  fallback.placeholder("rust-out", id)
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
