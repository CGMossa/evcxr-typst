# Typst package API surface (T-D03)

Public API of `packages/evcxr/` — what users write inside their `.typ` documents.

> **Reconciliation needed.** T-D02's example gallery is not on disk yet (`docs/design/examples/` is empty). This design proceeds from the briefing's strawman primitives (`rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data`, `dep`) and the requirements in ARCHITECTURE.md and DECISIONS.md. The orchestrator should reconcile naming with the gallery once it lands; if a gallery example forces a different shape, that takes precedence and the contradictions become real ones to resolve.

---

## 1. Top-level design choices

### 1.1 Code is passed as raw blocks, not strings

```typ
#rust(```rust
let x = 2 + 2;
println!("{x}");
```)
```

Not:

```typ
#rust("let x = 2 + 2;\nprintln!(\"{x}\");")  // rejected
```

**Why.** Editors syntax-highlight the inside of fenced raw blocks; users can copy-paste from existing Rust files without escaping; `raw` content carries the language tag we re-use for fallback display. Strings would also work but lose all editor support — the trade-off is real but the gallery (and the prequery / typst-doc precedent) lean clearly toward raw.

The package extracts the underlying source via `src.text` when given a `raw` element. As an escape hatch, a string is also accepted (so users can build snippets programmatically), but the docstring discourages it.

### 1.2 Configuration lives in a Typst `state`, set via `setup()`

Per-call kwargs cover the common case (`id:`, `deps:`, `show:`, `caption:`). Document-wide knobs (cache directory hint, default placeholder style, default show mode) live in a state variable and are seeded by an optional `setup()` call at document top:

```typ
#import "@preview/evcxr:0.1.0" as evcxr
#evcxr.setup(
  show-source: true,
  source-style: (fill: luma(245), inset: 6pt, radius: 3pt),
  placeholder-style: (stroke: 0.5pt + red, inset: 4pt),
  default-show: "both",  // "source" | "output" | "both" | "output-only"
)
```

`setup()` is **optional**. Defaults are sensible. There is no global mutation outside the state — calling `setup` twice replaces, doesn't merge (predictable; users who want partial overrides pass kwargs at call sites).

**Why state, not module-level constants.** Typst packages are immutable once imported; a module-level binding can't be reconfigured by the consumer. State is the idiomatic Typst answer for "package config a user can tweak."

### 1.3 Naming convention

- Public functions are kebab-case to match Typst stdlib idiom (`raw`, `box`, `place`) and prequery (`prequery.image`).
- All Rust-related functions share the `rust-` prefix. This is **deliberate** even though the package import is already namespaced (`evcxr.rust(...)` vs `evcxr.rust-out(...)`): we want unqualified `import "@preview/evcxr": *` to remain unambiguous, and the prefix telegraphs "this evaluates Rust" to readers who didn't see the import.
- `dep` is **not** prefixed (`evcxr.dep`, not `evcxr.rust-dep`) — it's distinct enough in shape (no body) that a separate name reads better, and it pairs intuitively with Cargo's `dep` mental model.
- The package itself is named `evcxr` (matches the upstream tool, matches the CLI binary).

---

## 2. Function reference

All functions take an optional `id: none` and `deps: ()` (covered in §4). They emit a `<evcxr-snippet>` (or `<evcxr-dep>`) metadata marker per ARCHITECTURE.md § "The metadata contract".

### 2.1 `rust(src, ..)` — the kitchen sink

```typc
#let rust(
  src,                    // raw block (preferred) or string
  id: none,               // override auto ID
  deps: (),               // array of dep handles or label strings
  show: auto,             // "source" | "output" | "both" | "output-only" | auto
  caption: none,          // figure caption; if set, wraps in figure
  source-lang: "rust",    // for the rendered source block
) -> content
```

Renders the source (syntax-highlighted) **and** the captured stdout/display output below it. The default. Equivalent to a Jupyter cell rendered with both code and result visible.

```typ
#rust(```rust
let xs: Vec<i32> = (1..=5).collect();
println!("sum = {}", xs.iter().sum::<i32>());
```)
```

### 2.2 `rust-out(src, ..)` — output only, source hidden

```typc
#let rust-out(src, id: none, deps: (), ..) -> content
```

Evaluates the snippet, renders only the captured plain stdout. Source is recorded in metadata (so the CLI evaluates it) but not displayed.

```typ
The answer is #rust-out(```rust print!("{}", 6 * 7);```).
```

This is the inline-friendly form; the rendered output is `text/plain` content from `<id>.txt`, wrapped in nothing more than a default `raw` (so it composes inline). Caller can wrap it in `box`/`text` themselves.

### 2.3 `rust-display(src, ..)` — display object only

```typc
#let rust-display(src, id: none, deps: (), prefer: auto, ..) -> content
```

Evaluates the snippet and renders only the highest-priority display artifact — the thing emitted via evcxr's `EVCXR_BEGIN_CONTENT` protocol (image, html, …), not plain stdout. If multiple display artifacts were emitted, `prefer:` picks one (`"png"`, `"svg"`, `"html"`, `"jpeg"`); `auto` follows the priority order in §5.

```typ
#figure(
  rust-display(```rust
    let plot = plotters_make_chart();
    evcxr_image_pic(plot)
  ```),
  caption: [Quarterly revenue],
)
```

If no display artifact was produced (snippet only printed text), this renders the placeholder (§3) annotated "no display output."

### 2.4 `rust-hidden(src, ..)` — execute, render nothing

```typc
#let rust-hidden(src, id: none, deps: ()) -> content
```

Used for setup snippets: define a struct, import a module, run side-effecting fixtures. Emits the metadata marker (so the CLI evaluates it) but produces no visible content. Returns `none`-equivalent content; safe to use at top-level or inside `#{ … }`.

```typ
#rust-hidden(```rust
#[derive(Debug)]
struct Sample { x: f64, y: f64 }

fn make_data() -> Vec<Sample> { /* … */ }
```)
```

### 2.5 `rust-data(src, ..)` — return parsed data, not content

```typc
#let rust-data(
  src,
  id: none,
  deps: (),
  format: auto,           // "json" | "cbor" | auto (sniff)
  fallback: (:),          // value to return when sidecar is missing
) -> any
```

The odd one out: it does **not** return content. It returns a Typst value (dict / array) parsed from the `application/json` or `application/cbor` sidecar. Snippets are expected to emit `evcxr_runtime::mime_type("application/json", ...)` or similar.

```typ
#let stats = rust-data(```rust
  evcxr_json!({ "n": data.len(), "mean": mean(&data), "sd": sd(&data) })
```)

The dataset has #stats.n samples, mean #stats.mean.
```

In fallback mode (no sidecar), returns `fallback` (default empty dict) — using `none` as the default would force every call site to handle the option type, which is hostile.

### 2.6 `dep(spec, ..)` — declare a Cargo dependency

```typc
#let dep(
  spec,                   // string: crate name, or "name = ..." TOML fragment
  features: (),           // array of strings
  id: none,
  show: false,            // by default deps render nothing
) -> content
```

Emits an `<evcxr-dep>` metadata marker. The CLI translates this into evcxr's `:dep` directive before any snippet that references it (see §4 on ordering). Renders nothing by default; `show: true` renders a small "depends on: serde 1.0" tag for documentation-style writing.

```typ
#dep("serde", features: ("derive",))
#dep("plotters = \"0.3\"")

#rust(```rust
use serde::{Serialize, Deserialize};
// …
```)
```

Single argument forms supported:

- `dep("serde")` → latest
- `dep("serde", features: ("derive",))`
- `dep("serde = \"1.0\"")` — full TOML fragment, passed through verbatim

Returning a **handle** (an opaque dict with the dep's id) is supported via `let s = dep("serde")`, so callers can reference deps explicitly via `deps: (s,)` on a snippet (see §4).

### 2.7 Helpers (small)

- `evcxr.version` — string, matches `typst.toml`. Exposed for fallback diagnostics.
- `evcxr.fallback` — Typst `state`, mirrors prequery's switch. Setting `evcxr.fallback.update(true)` (or `--input evcxr-fallback=true`) forces every snippet into placeholder mode regardless of sidecar presence. Useful when authoring new snippets without running the CLI.

---

## 3. Fallback behavior

Per D-004, `typst compile main.typ` without running the CLI must succeed and produce a placeholder-bearing PDF. Fallback fires when (a) the sidecar file the function expects is missing, or (b) `evcxr.fallback` state is `true`.

### 3.1 What renders

| Function | Fallback rendering |
|---|---|
| `rust` | Source block + a placeholder box where the output would be. |
| `rust-out` | A single placeholder box with text "(rust output not yet evaluated)". |
| `rust-display` | A placeholder box sized to a default 4cm × 3cm with a Unicode picture-frame glyph (U+1F5BC) and the snippet id. |
| `rust-hidden` | Nothing (same as success). |
| `rust-data` | Returns the `fallback:` value (default empty dict). No visible artifact. |
| `dep` | Nothing by default; with `show: true`, "depends on: <spec>" tag (no fallback distinction needed). |

### 3.2 Placeholder box anatomy

A single function `_placeholder(kind, id, src, reason)` (private, in `fallback.typ`) produces:

```
┌──────────────────────────────────────────────┐
│ evcxr · <kind> · <id-prefix>           ⚠     │
│ <reason: "not evaluated" | "missing sidecar">│
│                                              │
│ <truncated source preview, ~3 lines, raw>    │
└──────────────────────────────────────────────┘
```

- Stroke `0.5pt + orange` by default.
- Inset 6pt.
- The source preview helps an author who's writing snippets without yet running the CLI; they can still see what's where.
- For `rust-display` we also reserve a default 4cm × 3cm so paginated layout doesn't reflow when the image lands.

### 3.3 User styling

Override via `setup(placeholder-style: (..))` (whole-document) or by passing `placeholder-style:` at call site. `placeholder-style` accepts the same dict shape that `box`/`rect` accept (`fill`, `stroke`, `inset`, `radius`, `width`, `height`).

For deep customization, `setup(placeholder: my-fn)` accepts a function `(kind, id, src, reason) => content` that fully replaces the renderer.

### 3.4 What the placeholder shows

- `kind` (e.g. `rust-out`)
- `id` (the snippet id, truncated to 8 chars unless full)
- `reason` (`"sidecar missing"`, `"fallback mode forced"`, `"display artifact missing"`)
- A truncated raw preview of the source (first ~120 chars, single-line collapse)

Never shows: full source by default (it's already in the doc above), filesystem paths (security), or the full id (visual noise).

---

## 4. Identity & deps

### 4.1 Overriding the auto ID

Every function accepts `id: none` (default = auto). Auto = `blake3(src)[:12]` per D-005.

```typ
#rust(id: "intro-loop", ```rust
for i in 0..3 { println!("{i}"); }
```)
```

When `id` is supplied, that's the verbatim id used for the sidecar filename (`<.evcxr-typst-cache>/intro-loop.txt`). The package validates: `id` must match `^[a-zA-Z_][a-zA-Z0-9_-]{0,63}$` and is asserted at compile time (Typst `assert(...)`). Invalid ids fail `typst compile` with a useful message.

### 4.2 `deps:` — explicit dep ordering

By default, **document order is the contract**: any `dep(...)` call appearing earlier in the document than a snippet is in scope for that snippet. The CLI sees deps and snippets interleaved in `loc.doc_order` and emits `:dep` directives in the right place.

`deps:` is the explicit-override form for two cases:

1. The user wants to reference a dep that, for layout reasons, appears *after* the snippet in the document. Authoring a dep at the top of the page next to its consumer reads better than at the file top.
2. The user wants belt-and-suspenders documentation of intent.

```typ
#let serde-dep = dep("serde", features: ("derive",), show: false)

// … many pages later …

#rust(deps: (serde-dep,), ```rust
#[derive(Serialize)]
struct Pt { x: f64, y: f64 }
```)
```

`deps:` accepts: dep handles (returned from `dep(...)`), label strings (e.g. `"my-dep-id"`), or a mix. The CLI resolves them to ids and ensures the corresponding `:dep` directive is active before this snippet runs.

**Ordering enforcement.** The package itself does not (cannot) enforce order — Typst's evaluation is single-pass top-to-bottom and doesn't run code. The CLI is the enforcement point: it reads `loc.doc_order` from the metadata, sorts deps to come before any snippet that mentions them in `deps:`, and otherwise honors document order. A `dep` that appears after a snippet that does **not** list it in `deps:` is treated as not-yet-active for that snippet — and if the snippet fails to compile, the error message names the candidate dep.

### 4.3 Collisions

Two snippets with byte-identical source produce the same auto id. v0 disambiguates by appending `-{doc_order}` to the second occurrence; the CLI handles this transparently. Documented as a known wart; users who care should pass explicit `id:`. (See D-005, T-D04.)

---

## 5. Metadata contract

Each call emits exactly one `metadata((...))<evcxr-snippet>` (or `<evcxr-dep>`) value at its location. Cross-references ARCHITECTURE.md § "The metadata contract".

### 5.1 `<evcxr-snippet>` schema

```json
{
  "v": 1,
  "id": "intro-loop",
  "kind": "rust" | "rust-out" | "rust-display" | "rust-hidden" | "rust-data",
  "src": "for i in 0..3 { println!(\"{i}\"); }",
  "deps": ["serde-derive-abc", "plotters-def"],
  "options": {
    "prefer": "png",
    "format": "json"
  },
  "loc": { "doc_order": 7 }
}
```

Fields:

- `v` — schema version. Bumped when this schema changes incompatibly.
- `id` — final id (after auto vs explicit resolution).
- `kind` — which package function emitted this.
- `src` — verbatim Rust source (post-`raw.text` extraction).
- `deps` — array of explicit dep ids from `deps:` kwarg. Empty when none. Implicit document-order deps are **not** listed here; the CLI computes those itself from the `<evcxr-dep>` markers.
- `options` — bag of kind-specific kwargs that affect evaluation or rendering (`prefer` for `rust-display`, `format` for `rust-data`). Forward-compatible: unknown keys are ignored by older CLI versions.
- `loc.doc_order` — captured by the CLI from `typst query`'s position info, not by the package. Listed here for completeness.

> ⚠ Slight extension to ARCHITECTURE.md § "The metadata contract": this design adds `deps` and `options` fields not present in the architecture sketch. They're additive and the architecture doc explicitly says the schema is "subject to change pre-1.0; pinned via a version field." Bumping `v` is unnecessary for a strict superset, but flag for review.

### 5.2 `<evcxr-dep>` schema

```json
{
  "v": 1,
  "id": "serde-derive-abc",
  "spec": "serde",
  "features": ["derive"],
  "loc": { "doc_order": 3 }
}
```

`spec` is whatever the user passed; the CLI re-formats it into a `:dep` directive.

---

## 6. Deferred to v1 (or later)

- **Tab-completion / signature help forwarding.** evcxr supports completion; routing it through Typst's tooling story is a separate problem. v0 ships no editor integration.
- **Snapshot / restore across edits.** D-003 already defers this; the API doesn't expose any of it. No `rust-checkpoint()`, no `rust-rewind()`. Add later as a kwarg `snapshot: true` if/when the underlying mechanism exists.
- **Cross-document caching.** Sidecars are per-document. Sharing a cache directory across multiple `.typ` files is a CLI concern; the package API doesn't model it.
- **Inline expressions inside Rust source.** No `#{some_typst_expr}` interpolation into Rust. Users compose at the Typst level (call `rust-data`, splice the dict). Adding interpolation would require Typst-side template expansion before metadata emission, which fights Typst's evaluation model.
- **Streaming long output.** No `rust-stream()` for snippets that take minutes and the user wants partial progress. v0 is batch-only.
- **Capture by binding rather than by sidecar.** A would-be `let x = rust(...)` returning the snippet's last expression value (à la a notebook cell). Neat, but requires exotic round-tripping; `rust-data` covers the common need.
- **Per-snippet timeout kwarg.** `timeout: 30s`. Defer until we know whether evcxr's child supports clean cancellation mid-eval (T-D06 territory).
- **Diagnostic-rich error rendering.** v0 gets a generic placeholder when a snippet errored; T-D06 designs the proper error UI and may add a `rust-error-style:` setup option.

---

## 7. Open questions / bikeshed list

These are genuinely arguable; flagging for human decision rather than picking and pretending it's obvious:

1. **`rust` vs `eval` vs `rs` vs `evcxr` as the primary verb.** I picked `rust` because it's instantly readable to Typst users who don't know what evcxr is, and it matches the language tag in `` ```rust ``` ``. `evcxr` is technically more correct (we could swap engines). `eval` is too generic (Typst already has `eval`, collision is bad). `rs` is too terse. **Strong opinion, weakly held.** If the gallery comes back with `evcxr-out` reading better than `rust-out`, swap.
2. **`rust-out` vs `rust-print` vs `rust-stdout`.** Strawman has `rust-out`. `rust-print` reads well for `print!`/`println!` but is misleading when the snippet emits via `eprintln!` or panic-output that we capture. `rust-stdout` is most precise but ugly. Picked `rust-out` for brevity; flagging.
3. **`rust-display` vs `rust-show` vs `rust-render`.** "Display" matches Jupyter/IPython terminology and evcxr's own `EVCXR_BEGIN_CONTENT` semantics. `show` collides with Typst's `show` rule. `render` is fine but vague. Picked `display`.
4. **`rust-hidden` vs `rust-setup` vs `rust-quiet`.** Strawman has `rust-hidden`. `rust-setup` reads better for the dominant use case ("define a struct for later") but is misleading when the snippet has visible side effects intentionally suppressed. `rust-quiet` matches Jupyter's `;` suffix convention. Picked `rust-hidden` for now — it describes the rendering, not the intent.
5. **Should `rust(...)` default to showing source AND output, or just output?** I picked both (matches Jupyter cell default; matches how the gallery `h-mini-report.typ` will probably want to read). A docs-focused user might prefer output-only as default and explicit `rust(...show: "both")` for tutorials. Configurable via `setup(default-show: ...)`.
6. **`dep` API: positional `spec` string vs structured kwargs.** I support both (`dep("serde")`, `dep("serde = \"1.0\"")`, `dep("serde", features: (...))`). Some readers will find that overloaded. Splitting into `dep-crate(name, version: ..)` vs `dep-toml(spec)` is cleaner but verbose.
7. **Schema additions (`deps`, `options`) vs strict adherence to ARCHITECTURE.md's documented schema.** Flagged in §5.1. Either fold them in (update ARCHITECTURE.md) or strip them and have the CLI inspect `<evcxr-dep>` order for dep linkage.
