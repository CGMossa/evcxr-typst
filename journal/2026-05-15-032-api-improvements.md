# lib.typ API improvements: id-as-label, auto-id gap, inert kwargs

**Date:** 2026-05-15
**Branch:** feat/id-as-label, feat/surface-auto-id-gap, feat/wire-inert-kwargs
**Upstream source:** n/a (internal API improvement)

## What I tried

Three loosely-related lib.typ design improvements surfaced from a user review
of `packages/evcxr/lib.typ`. Planned and implemented as three independent PRs
to minimize merge churn and enable focused review.

## PR 1 — id-as-label

Added Typst element labels `<id>` and `<id-out>` to rendered code blocks and
output blocks when an explicit `id:` is provided. Authors can now write
`@foo-bar` / `@foo-bar-out` from prose to cross-reference a snippet's code or
output.

Key design choices made:
- **Auto-derived IDs do not receive labels.** Blake3 hashes are opaque and
  unstable — labelling them would be misleading (the label disappears on any
  source edit).
- **`<id-out>` is only emitted when real evaluated output is present.** The
  `_index-available(id)` check gates this; placeholders are never labelled.
- **`rust-hidden` and `rust-data` emit no labels.** No visible content to
  reference.
- No schema change — labels are Typst-side only; the CLI never sees them.
- D-007's collision resolver gives uniqueness for free (explicit ID collisions
  are already a hard CLI error).

Implementation: two helpers `_code-label(id)` and `_out-label(id)` placed
after rendered content. The `_out-label` is embedded inside `_read-stdout`,
`_read-display`, and `_read-html` where real content is produced.

## PR 2 — surface the auto-id gap

The silent footgun: CLI evaluates snippets and writes sidecars keyed by
`blake3(src)[..12]`, but Typst cannot recompute blake3. Any snippet without
an explicit `id:` always falls through to the placeholder even after
`evcxr-typst run --allow-eval` — producing an all-placeholder PDF with no
signal to the author about what went wrong.

Options weighed: (a) pass computed ID back via `_index.json`, (b) make `id:`
required statically, (c) document-only. Chose a "b-soft" approach: keep the
`id: none` default (preserves D-004 fallback behavior) but emit a visible
orange warning box in read-mode when `id:` is missing from an output-rendering
function call. The warning names the function and shows an example fix.

Applies to: `rust`, `rust-main`, `rust-out`, `rust-display`, `rust-html`.
Exempt: `rust-hidden` and `rust-data` (no visible output block).

In fallback mode (bare `typst compile`), the warning is suppressed — the
normal placeholder appears, D-004 is fully preserved.

## PR 3 — wire inert kwargs

Four kwargs were accepted by the API but did nothing at render time:

**`render:` on `rust`/`rust-main` — now live.** Values `"source"` (code only),
`"output"` (output only), `"both"` (default). `auto` is treated as `"both"`.

**`caption:` on `rust`/`rust-main` — now live.** Wraps the visible blocks in
a Typst `figure(caption: ...)`. Useful for numbered figures in academic docs.

**`setup(default-render:)` — remains inert at render time.** Architectural
limitation: `lib.typ` cannot read its own `<evcxr-setup>` metadata markers at
render time without a `typst query` round-trip. The kwarg is forwarded to the
CLI via metadata (the CLI can use it for CLI-side defaults), but the Typst
rendering functions can't access it. Documented explicitly in the `setup()`
doc comment; per-call `render:` is the correct workaround.

**`setup(fallback:)` — marked "accepted, no effect".** The fallback rendering
shape is hardcoded in `fallback.typ`. The kwarg is kept in the signature for
forward-compatibility.

## What I learned

The `setup(default-render:)` → `rust(render: auto)` chain is architecturally
impossible in pure Typst without either `state` (which requires `context {}`
wrapping every call site, restricting usage contexts) or a CLI-side pre-pass
that embeds the resolved value into each snippet's metadata. Both have real
costs. The pragmatic fix — treat `auto` as `"both"` and document it — is the
right call for now. If document-level render control becomes a real need, a
Typst `state`-based approach or a `sys.inputs`-based override could work.

## Follow-ups

- [ ] PR 1 × PR 2 interaction: if both land, labels from PR 1 appear on the
  warning box from PR 2 when `id:` is missing — the label attaches to the
  warning block. This is odd but harmless; the warning box is temporary.
- [ ] `render: "output"` suppresses the code block but still shows a
  placeholder when no sidecar exists. That placeholder says the kind (e.g.
  "rust") which implies code — slightly misleading. Low priority.
- [ ] `caption:` on display functions (`rust-display`, `rust-html`) would also
  be useful. Not added in this pass since those functions don't have `caption:`
  in their current signatures (adding it is a signature change).
