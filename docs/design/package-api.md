# Typst package API surface (T-D03)

Public API of `packages/evcxr/` — what users write inside their `.typ` documents.

> **Reconciliation needed.** T-D02's example gallery is not on disk yet (`docs/design/examples/` is empty). This design proceeds from the briefing's strawman primitives (`rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data`, `dep`) and the requirements in ARCHITECTURE.md and DECISIONS.md. The orchestrator should reconcile naming with the gallery once it lands; if a gallery example forces a different shape, that takes precedence and the contradictions become real ones to resolve.

> **Per-snippet `timeout:` resolved (D-017).** Shipped in v0 on every eval-emitting function (`rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data`); `dep()` does *not* accept it. Cancellation is SIGKILL-only; semantics match D-009 (host child dies, fresh child re-spawns, `let` bindings lost, items recompiled). See § 2.8 for the kwarg shape and § 6 for what stays deferred.

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
  timeout: auto,          // per-snippet override; see § 2.8
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
#let rust-out(src, id: none, deps: (), timeout: auto, ..) -> content
```

Evaluates the snippet, renders only the captured plain stdout. Source is recorded in metadata (so the CLI evaluates it) but not displayed.

```typ
The answer is #rust-out(```rust print!("{}", 6 * 7);```).
```

This is the inline-friendly form; the rendered output is `text/plain` content from `<id>.txt`, wrapped in nothing more than a default `raw` (so it composes inline). Caller can wrap it in `box`/`text` themselves.

### 2.3 `rust-display(src, ..)` — display object only

```typc
#let rust-display(src, id: none, deps: (), prefer: auto, timeout: auto, ..) -> content
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
#let rust-hidden(src, id: none, deps: (), timeout: auto) -> content
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
  fallback: (:),          // value to return when no sidecar yet exists (pre-CLI run)
  timeout: auto,          // per-snippet override; see § 2.8
) -> any
```

The odd one out: it does **not** return content. It returns a Typst value (dict / array) parsed from the `application/json` or `application/cbor` sidecar. Snippets are expected to emit `evcxr_runtime::mime_type("application/json", ...)` or similar.

```typ
#let stats = rust-data(```rust
  evcxr_json!({ "n": data.len(), "mean": mean(&data), "sd": sd(&data) })
```)

The dataset has #stats.n samples, mean #stats.mean.
```

Three return modes (resolved in D-015):

- **Success** — the parsed dict/array.
- **No sidecar yet** (CLI hasn't been run, or `--allow-eval` was off) — returns `fallback` (default `(:)`). Lets the document compile cleanly under bare `typst compile` per D-004 without forcing every call site to pattern-match an option type.
- **Snippet errored** (`<id>.error.json` present) — returns `none`, *and* a side-effect error box is emitted at a sibling location (see `errors.md` § 4 and D-015). Returning `none` here forces the caller to acknowledge a real failure (`if stats != none { … }`), distinct from the unevaluated case above; quietly returning a fake dict would silently propagate corrupt data into downstream Typst layout.

### 2.6 `dep(name, version, ..)` — declare a Cargo dependency

```typc
#let dep(
  name,                   // crate name, OR "name = …" TOML fragment if it contains '='
  version: none,          // version requirement, e.g. "1", "1.0", "^1.2"; optional positional too
  features: (),           // array of strings
  default-features: true,
  git: none,              // git URL
  path: none,             // local path (resolved relative to main.typ)
  package: none,          // rename: depend on `serde` but call it `s`
  id: none,
  show: false,            // by default deps render nothing
) -> content
```

Emits an `<evcxr-dep>` metadata marker at its document position. The CLI pre-collects all `<evcxr-dep>` markers, resolves them in document order, and emits a `:dep` directive into evcxr before any snippet that comes after the marker (see §4 on ordering and D-013 on inline-anywhere placement). Renders nothing by default; `show: true` renders a small "depends on: serde 1.0" tag for documentation-style writing.

`version` is positional too — `dep("regex", "1")` is the canonical form used throughout the gallery.

```typ
#dep("serde", features: ("derive",))
#dep("regex", "1")
#dep("plotters = \"0.3\"")            // TOML escape hatch (the '=' triggers it)

#rust(```rust
use serde::{Serialize, Deserialize};
// …
```)
```

Forms supported:

- `dep("serde")` → latest
- `dep("regex", "1")` → `regex = "1"`
- `dep("serde", features: ("derive",))`
- `dep("tokio", "1", features: ("full",))`
- `dep("mycrate", path: "./mycrate")` — `path` is canonicalized relative to the document
- `dep("plotters = \"0.3\"")` — full TOML fragment, passed through verbatim. Detected by the package because the single positional arg contains `=` outside leading whitespace.

Returning a **handle** (an opaque dict with the dep's id) is supported via `let s = dep("serde")`, so callers can reference deps explicitly via `deps: (s,)` on a snippet (see §4).

### 2.7 Helpers (small)

- `evcxr.version` — string, matches `typst.toml`. Exposed for fallback diagnostics.
- `evcxr.fallback` — Typst `state`, mirrors prequery's switch. Setting `evcxr.fallback.update(true)` (or `--input evcxr-fallback=true`) forces every snippet into placeholder mode regardless of sidecar presence. Useful when authoring new snippets without running the CLI.

### 2.8 `timeout:` kwarg — per-snippet override (D-017)

Every eval-emitting function (`rust`, `rust-out`, `rust-display`, `rust-hidden`, `rust-data`) accepts `timeout:`. **`dep()` does not** — `:dep` resolution does not run inside the timed `execute()` call (see "What `timeout:` does *not* cover" below).

**Accepted values.**

| Form | Meaning |
|---|---|
| `auto` *(default)* | Use the global timeout from `--snippet-timeout` (default 30s, per D-009). |
| `none` | Disable the timeout for *this* snippet only. The global flag is ignored. |
| `duration` | A Typst `duration` value, e.g. `5s`, `2min`, `1h + 30s`. |
| `<integer>` | Bare integer is interpreted as **seconds** (so `timeout: 60` = 60s). |
| `<string>` | Pattern `^([0-9]+)\s*(ms|s|min|h)$`, e.g. `"500ms"`, `"5min"`. Whitespace between digits and unit is allowed. |

The package validates the form at metadata-emission time (Typst `assert(...)`); invalid input fails `typst compile` with a useful message naming the snippet id.

The metadata marker carries the resolved value as **integer milliseconds** in `<evcxr-snippet>.options.timeout_ms` (or `null` for `auto`, the literal string `"none"` for explicit disable). The CLI consumes that.

**Example.**

```typ
#rust(timeout: 5min, ```rust
let mut acc = 0u64;
for i in 0..100_000_000 { acc = acc.wrapping_add(i); }
println!("{acc}");
```)

#rust-hidden(timeout: "500ms", ```rust
// Quick fixture; if it hangs we want to know fast.
let _ = std::fs::read_to_string("/etc/hostname").unwrap();
```)

#rust(timeout: none, ```rust
// User opts into a possibly-infinite computation.
loop { /* … */ }
```)
```

**Interaction with `--snippet-timeout`.**

Resolution rule: **per-snippet wins, full stop.** If `timeout:` is set to anything other than `auto`, the global flag does not apply — neither as a floor nor as a ceiling. Rationale: minimum-wins surprises authors who explicitly raise the limit; maximum-wins surprises the inverse. Single-rule override matches how every other per-call kwarg in the package interacts with `setup()` defaults.

`--no-snippet-timeout` on the CLI sets the global default to "none"; per-snippet `timeout:` still overrides it.

**What happens on expiry — same as D-009.**

Cancellation is **SIGKILL-only**. evcxr's `ChildProcess` exposes one stop mechanism (`process_handle().lock().unwrap().kill()`); there is no clean cancel signal in either the IPC protocol or the host child's runtime (confirmed by reading `evcxr/src/child_process.rs` and `evcxr/src/eval_context.rs`). On expiry, `evcxr-typst` SIGKILLs the host child, evcxr's next call returns `Error::SubprocessTerminated`, and a fresh child is spawned for the next snippet. All `let` bindings established before the timed-out snippet are lost (per D-011); items (`fn`, `struct`, `impl`, `mod`, `use`) are recompiled from `committed_state` and survive.

The error sidecar is `<id>.error.json` with `phase: "timeout"` and `errors[0].timeout = { duration_ms: <resolved>, captured_stdout_bytes: <n> }` (schema in `errors.md` § 2). Partial stdout captured up to the kill point is preserved as `<id>.txt` (`errors.md` § 1.e).

**What `timeout:` does *not* cover.**

The timer wraps `CommandContext::execute(<snippet src>)`. That call internally:

1. Adjusts state (cheap, in-memory).
2. Runs `cargo build` for the snippet's wrapper crate (in *our* thread, not the host child).
3. Loads the resulting cdylib via `libloading` and dispatches to the host child to run it.

A SIGKILL on `process_handle()` aborts step 3 only. It does **not** stop a running cargo invocation in step 2 — cargo is a synchronous child of the calling thread, and the `tokio::time::timeout` returns from the *await* but the cargo subprocess keeps running until it exits or the OS reaps it on `evcxr-typst` shutdown. Practical effect: a snippet that wedges in `cargo build` (e.g. a procedural macro that itself loops) will exceed `timeout:` by the cargo runtime. We accept this; it's a consequence of evcxr's architecture and matches D-009's behaviour exactly. Documenting it here so users with very tight timeouts (sub-second) know the floor is "however long cargo takes."

**Race with `:dep` resolution.** Per-snippet `timeout:` does *not* apply to `dep()` calls (no `timeout:` kwarg on `dep`). `dep()` produces `<evcxr-dep>` markers that the CLI realises into `:dep` directives **before** any `CommandContext::execute(<snippet>)` call. Those `:dep` directives ultimately drive `cargo build` inside an `execute()` call in step 2 above; the global `--snippet-timeout` covers them but not at sub-cargo granularity. If a `:dep` resolution is slow, the responsibility is on the global flag, not on a per-snippet kwarg that didn't fire yet. Documented as a known boundary.

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
| `rust-data` | No sidecar yet → returns `fallback:` value (default empty dict), no visible artifact. Errored snippet → returns `none` and emits a sibling error box (see D-015). |
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

`dep(...)` calls are **inline-anywhere** (D-013): a Typst author may place them at the document head, just before their consumer, or sprinkled through chapters. By default, **document order is the contract**: any `dep(...)` call appearing earlier in the document than a snippet is in scope for that snippet. The CLI sees deps and snippets interleaved in `loc.doc_order` and emits `:dep` directives in the right place.

Two `dep()` calls naming the same crate with conflicting versions are a **CLI-level error**, not a silent last-write-wins (per snippet-semantics G5). The error names both call sites.

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
- **Diagnostic-rich error rendering.** v0 gets a generic placeholder when a snippet errored; T-D06 designs the proper error UI and may add a `rust-error-style:` setup option.

---

## 7. Resolved naming and API choices (D-012)

Validated against the example gallery in `docs/design/examples/` (all 8 `.typ` files plus `index.md`). Final rulings:

1. **Primary verb: `rust`.** Instantly readable, matches the `` ```rust `` language tag, and reads naturally in flow ("If you see `Hello, world!`…"). `eval` collides with Typst's stdlib `eval`. `evcxr` is jargon. `rs` is too terse. The gallery uses `#rust(...)` throughout without strain.
2. **Stdout-only: `rust-out`.** Brief enough to live inline ("The answer is `#rust-out(...)`."), and the gallery already reads cleanly with it (`a-hello.typ`, `h-mini-report.typ` § 5). `rust-print` is misleading because we also capture `eprintln!`/`panic` text; `rust-stdout` is precise but ugly inline.
3. **Display-only: `rust-display`.** Matches evcxr's `EVCXR_BEGIN_CONTENT` vocabulary and Jupyter conventions. `rust-show` would clash semantically with Typst's `show` rule. `rust-render` is vague. Gallery `d-image-output.typ` and `h-mini-report.typ` § 4 read naturally.
4. **Evaluate-and-render-nothing: `rust-hidden`.** Describes the *rendering* (no visible output) rather than guessing intent. `rust-setup` would mislead when the snippet is intentionally suppressing visible side-effects (e.g. a fixture that already happened); `rust-quiet` is unclear to non-Jupyter users. Gallery `b-struct-across-snippets.typ`, `c-module-across-snippets.typ`, and `h-mini-report.typ` § 1 use it for setup, definition, and corpus blocks alike — `rust-hidden` covers all three.
5. **Default `show:` for `rust(...)`: `"both"` (source + output).** Matches Jupyter cell convention and matches every gallery use of `#rust(...)` (`a-hello.typ`, `b-…`, `e-cratesio-dep.typ`, `f-async-tokio.typ`, `g-error-case.typ`, `h-mini-report.typ` § 2). Output-only as the default would require explicit `show: "both"` on every tutorial snippet — backwards. Configurable via `setup(default-show: "output")` for docs-focused authors.
6. **`dep` API: positional `(name, version?)` plus kwargs, with TOML-fragment escape hatch.** Final shape:
   - `dep(name)` — latest.
   - `dep(name, version)` — pin (gallery's idiom: `#dep("regex", "1")`).
   - `dep(name, features: ("derive",))` and other kwargs (`default-features`, `git`, `path`, `package`).
   - `dep("name = ...")` — single string detected by the package as a TOML fragment (presence of `=` outside of leading whitespace), passed through verbatim to evcxr's `:dep`.
   The two-arg positional form is the canonical one in the gallery; kwargs cover the realistic surface; the TOML escape hatch survives for power users without forcing a separate `dep-toml` function. **`dep()` calls remain inline-anywhere**; the CLI pre-collects them in document order and errors on conflicting versions per snippet-semantics G5 (see D-013).
7. **Schema additions (`deps`, `options`) on `<evcxr-snippet>`.** Folded in. The architecture sketch already declared the schema "subject to change pre-1.0; pinned via a version field"; these additions are a strict superset, so `v` stays at `1`. ARCHITECTURE.md was updated in the same pass that added DECISIONS.md D-012. (No separate decision; bookkeeping only.)

Per-snippet `timeout:` kwarg shipped in v0 (D-017); see § 2.8.
