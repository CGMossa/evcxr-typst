# Errors & diagnostic plumbing (T-D06)

How compile errors, runtime panics, and pipeline failures from `evcxr` reach the
reader of the rendered Typst document and the developer running
`evcxr-typst run|watch` in a terminal.

> **API names finalised (D-012).** This document uses the resolved names from
> `docs/design/package-api.md`: `rust()`, `rust-out()`, `rust-display()`,
> `rust-data()`, `rust-hidden()`, `dep()`. (`rust-html()` is *not* a
> separate function in v0 — HTML is one of the artifacts surfaced by
> `rust-display()` via its `prefer:` kwarg.) Per-snippet `timeout:` kwarg
> remains deferred under T-D08 (RECON-T-D03 → T-D08).

---

## 0. Goals

- A snippet that fails to compile / panics at runtime must **not** abort the
  Typst render. The PDF still builds, with the broken snippet flagged in place.
  (Already a baked-in expectation per `docs/design/examples/g-error-case.typ`.)
- A snippet failure must, by default, set the **CLI exit code to non-zero** so
  CI catches it (§ 7).
- The terminal output for the developer must be at least as informative as
  `cargo build`. We get this almost for free by reusing what evcxr's REPL
  already does.
- Span pointers must round-trip correctly: a rustc span that points into
  generated wrapper code is suppressed; a rustc span that points into snippet
  *A* while the error was produced compiling snippet *B* is shown at *B*'s call
  site **and** linked to *A*.

---

## 1. Error taxonomy

| # | Kind | When | Attribution | Doc render | Terminal |
|---|---|---|---|---|---|
| a | **Snippet compile error** | rustc rejects the snippet's own source | the snippet itself | error box at the snippet | ariadne report (see § 5) |
| b | **Cross-snippet compile error** | snippet B references item from A; rustc spans land in A | rendered at B; box names A as the secondary location | error box at B with "see snippet A (id=…)" footer | ariadne report with both spans |
| c | **Runtime panic** | snippet's compiled code panics at run time | the snippet | partial stdout (captured up to panic), then a panic box | ariadne-style box with panic message + first 8 backtrace frames (see § 1.c) |
| d | **`:dep` resolution failure** | cargo can't resolve / fetch / build a dep before any snippet runs | the `dep()` call that introduced the failing dep, or the *first* `dep()` if we can't tell | error box at that `dep()` site; if no `dep()` exists in the doc, surfaces as a top-of-document banner | one ariadne report per failing dep + cargo's stderr quoted verbatim |
| e | **Snippet timeout** | snippet runs > N seconds (see § 1.e) | the snippet | timeout box at the snippet, partial stdout | "snippet `<id>` timed out after Ns" + tail of captured stdout |
| f | **Internal evcxr error / child crash** | `Error::SubprocessTerminated`, `Error::TypeRedefinedVariablesLost`, libloading failure, evcxr panic | the snippet that triggered it (if known); else the whole run | minimal placeholder box: "snippet failed: internal error (see terminal)" — *no* internals leaked into PDF | full diagnostic to stderr; CLI exits non-zero |

### 1.c Runtime panics — what we capture

- evcxr surfaces panics by termination of the child or via a panic line on
  stderr. We capture the snippet's stdout up to the panic point and store it
  as `<id>.txt` (so a `rust-out()` call still shows partial output above the
  error box) plus a separate `<id>.error.json` with the panic kind.
- We set `RUST_BACKTRACE=1` in the child by default (configurable via
  `--no-backtrace`). The first 8 frames of the backtrace are stored; truncated
  frames go to the JSON `backtrace_truncated_count`.
- If the panic killed the child (`Error::SubprocessTerminated`), evcxr will
  spawn a fresh child for subsequent snippets. We treat that as a normal
  panic, and re-evaluate from the cleared state — i.e. all `let`-bindings
  established in earlier snippets are gone.
  > **⚠ Contradicts ARCHITECTURE.md § "Composition across snippets"** which
  > implicitly assumes `let` bindings persist; in the panic-kill case they
  > don't, and we must surface this to the user (a yellow banner across the
  > document run output, plus a per-affected-snippet sub-warning).

### 1.e Timeout — decision

**Decision: yes, with default 30s per snippet, configurable.** Rationale:
without a timeout, an infinite loop in any snippet hangs `evcxr-typst run`
indefinitely with no signal to the user. 30s is generous for normal snippets
and short enough that CI surfaces the problem.

Implementation: a `tokio::time::timeout` wrapper around the
`CommandContext::execute` call. On timeout we send SIGKILL to the host child
process (evcxr's `EvalContext` will report `SubprocessTerminated`), record a
timeout error, and continue with a fresh child.

Configurable via:

- CLI flag: `--snippet-timeout 60s` (or `--no-snippet-timeout` to disable)
- Per-snippet override via Typst-side `rust(..., timeout: 5min)` (RECON-T-D03)

---

## 2. Sidecar JSON schema for errors

Companion to the existing sidecar mapping in
`docs/ARCHITECTURE.md` § "MIME → Typst output mapping". Errors are the
**seventh MIME type** in that table — they live at `<id>.error.json` and the
Typst package treats their presence as authoritative regardless of whether
other sidecars (`<id>.txt`, `<id>.png`, …) also exist.

A snippet may have **0 or 1** `<id>.error.json` file. Multiple errors for a
single snippet are aggregated into the `errors[]` array within that one file.

```jsonc
{
  // schema version. Bump on breaking changes; the package emits a fallback
  // box with "unknown error schema vN" if it sees an unfamiliar version.
  "v": 1,

  // The snippet's stable id (matches <evcxr-snippet>.id in main.typ). Always
  // present; for kind "dep-resolution" with no associated dep() call site,
  // this is "" (empty) and the package renders a top-of-document banner.
  "snippet_id": "a1b2c3d4e5f6",

  // The one *user-facing* phase that failed for this snippet. Used by the
  // package to colour the box. One of:
  //   "compile" | "runtime-panic" | "dep-resolution" | "timeout" | "internal"
  "phase": "compile",

  // The original full rendered terminal text from rustc/cargo, including
  // ANSI colour codes if the upstream produced them. This is what the CLI
  // already prints to its own stderr — duplicated here so a downstream tool
  // (or the doc author after-the-fact) can read it without re-running. May
  // be empty for kind=internal.
  "rendered_terminal": "[1;31merror[E0308][0m: mismatched types\n  --> snippet:2:14\n   |\n …",

  // Wall-clock at which evcxr-typst recorded the error, ISO-8601 UTC. Used
  // by the watch loop to decide which sidecar is fresher.
  "recorded_at": "2026-05-06T14:22:09Z",

  // The snippet's source as the CLI saw it when the error occurred. Stored
  // verbatim so the Typst package can highlight spans without re-reading
  // main.typ (which might have moved on by the time the package renders).
  "snippet_src": "let mut s = String::new();\ns.push_str(format!(\"answer: {}\", 42));\n",

  // 1+ errors. For kind=compile, rustc may emit several distinct errors
  // for one snippet (e.g. one per missing import) — they all live here.
  // For kind=runtime-panic / timeout / internal there's exactly one.
  "errors": [
    {
      // Severity. Mirrors rustc's "level" field. Only "error" blocks the
      // snippet's output; "warning" is informational and the snippet still
      // produces its sidecars.
      "severity": "error",

      // rustc error code if known (E0308, E0382, …). null for non-rustc
      // kinds (panic, timeout, dep, internal). Used for the "?" link to
      // the rustc explanation (`rustc --explain E0308`).
      "code": "E0308",

      // The single-line summary, sanitized via evcxr's sanitize_message
      // (so `evcxr_variable_store` is rewritten to <end of input>).
      "message": "mismatched types",

      // The primary location. snippet_id may differ from the outer
      // snippet_id when this is a cross-snippet error: outer says "render
      // the box on snippet B"; primary says "the actual offending text
      // lives in snippet A".
      "primary_span": {
        "snippet_id": "a1b2c3d4e5f6",
        // Byte offsets into snippet_src above. Inclusive start, exclusive end.
        "byte_start": 39,
        "byte_end": 64,
        // Convenience copy of the substring. Redundant with byte_start/end
        // but the package can avoid a substring computation in pure Typst.
        "text": "format!(\"answer: {}\", 42)",
        // 1-based line/column for terminal pretty-printing. Derivable from
        // byte offsets but cheaper to store than recompute.
        "line_start": 2, "col_start": 12,
        "line_end": 2,   "col_end": 37,
        // Human label that ariadne would draw next to the span:
        // "expected `&str`, found `String`".
        "label": "expected `&str`, found `String`"
      },

      // Other spans that contribute context. May point into *other*
      // snippets — see § 3. Empty array if none.
      "secondary_spans": [
        {
          "snippet_id": "f0e1d2c3b4a5",     // a different snippet
          "byte_start": 7, "byte_end": 16,
          "text": "Foo<u32>",
          "line_start": 1, "col_start": 8,
          "line_end": 1,   "col_end": 17,
          "label": "originally defined here",
          // For cross-snippet refs, the package uses this to render
          // "see snippet f0e1d2c3b4a5" in the error footer.
          "is_cross_snippet": true
        }
      ],

      // help: messages (rustc's child diagnostics). Rendered as bullet
      // points beneath the primary span.
      "helps": [
        {
          "message": "consider borrowing here",
          // Optional rustc machine-applicable suggestion.
          "suggested_replacement": "&format!(\"answer: {}\", 42)",
          // Span the suggestion would replace. Absent for free-form helps.
          "span": {
            "snippet_id": "a1b2c3d4e5f6",
            "byte_start": 39, "byte_end": 64
          }
        }
      ],

      // evcxr-specific extra hint, from CompilationError::evcxr_extra_hint().
      // null when evcxr has nothing to add.
      "evcxr_hint": null,

      // For phase=runtime-panic only.
      "panic": null,
      // {
      //   "message": "index out of bounds: the len is 3 but the index is 5",
      //   "location": "snippet:3:5",         // file:line:col reported by panic
      //   "backtrace": [
      //     "core::panicking::panic_bounds_check",
      //     "<snippet>::run::h0a1b2c3"
      //   ],
      //   "backtrace_truncated_count": 12
      // },

      // For phase=timeout only.
      "timeout": null,
      // { "duration_ms": 30000, "captured_stdout_bytes": 1024 },

      // For phase=dep-resolution only.
      "dep": null
      // {
      //   "spec": "tokio = { version = \"1\", features = [\"full\"] }",
      //   "cargo_stderr": "error: failed to select a version for `tokio` …"
      // }
    }
  ]
}
```

### Field validity matrix

| field | compile | runtime-panic | dep-resolution | timeout | internal |
|---|---|---|---|---|---|
| `errors[].code` | yes (E0xxx) | null | null | null | null |
| `errors[].primary_span` | yes | yes (panic loc, may be null) | maybe (`dep()` site) | yes (whole snippet) | maybe |
| `errors[].secondary_spans` | maybe | rare | none | none | none |
| `errors[].helps` | maybe | none | sometimes (cargo hints) | none | none |
| `errors[].panic` | null | yes | null | null | null |
| `errors[].timeout` | null | null | null | yes | null |
| `errors[].dep` | null | null | yes | null | null |

---

## 3. Cross-snippet span attribution

**Decision.** When rustc's primary span lands in snippet *A* but the error was
produced while compiling snippet *B*:

1. The error sidecar is written under **B's** id (`<B>.error.json`). That is:
   the box appears at B's call site in the rendered document. Reason: B is the
   one the user just edited and is staring at. Rendering it at A would surprise
   the user, who didn't touch A.
2. Within that file, `errors[].primary_span.snippet_id` points to **A**. The
   Typst package renders the box at B's location (because we're reading
   `<B>.error.json`) but the box body shows the offending text from A and
   tags it with "snippet `a1b2c3d4e5f6`". A grey footer reads:
   "*originally defined in snippet `a1b2c3d4e5f6` (line N of main.typ)*".
3. We **also** write a tiny stub `<A>.error.json` with `phase="compile"` and
   a single error of severity `note` whose body says
   "this item is referenced from snippet `<B>` and is producing errors there".
   This makes A's render show a yellow note (not a red error), so the user
   gets a hint at the definition site without a duplicated red box.
4. The CLI's terminal output emits **one** ariadne report per error, with
   *both* spans (A's and B's) drawn against the union of A's and B's source.
   ariadne already supports multi-source reports.

How we know A vs B: evcxr's `CompilationError::is_from_user_code()` plus its
`code_origins: Vec<CodeKind>` already distinguish user code from generated
wrapper code. To map a user-code span back to a *snippet id* we maintain a
**parallel offset map** on the `evcxr-typst` side, not an upstream patch
(resolved in D-014). The offset map's structure is described in § 6 below.

---

## 4. Typst-side rendering

The package gains an internal helper, `_evcxr-error-box(error_json)`, that
every public function calls when its sidecar's `<id>.error.json` exists.
Function-level behaviour:

| Public function | When `<id>.error.json` exists |
|---|---|
| `rust(...)` (default; shows code + output) | Code block still rendered; **error box** replaces the output area. |
| `rust-out(...)` (output only) | **Error box** in place of output. |
| `rust-display(...)` (display objects only, e.g. images) | **Error box** instead of the image/etc. |
| `rust-data(...)` (returns dict/array to Typst) | **Returns `none`** *and* emits a side-effect error box at a sibling location. The doc author's downstream code must handle `none`. (Resolved in D-015; see `package-api.md` § 2.5.) |
| `rust-hidden(...)` (executes but produces no doc output) | **No box in the doc.** The CLI still records the error and exits non-zero, so CI catches it. The author can opt-in to surfacing via `rust-hidden(..., on-error: "show")`. |
| `dep("…")` | A failing `dep()` shows an error banner *at the call site* (since `dep()` normally produces no visible output). |

### Visual sketch (markdown approximation of the Typst box)

```
┌─ rust error · snippet a1b2c3d4e5f6 ──────────────── E0308 ─┐
│  mismatched types                                          │
│                                                            │
│  expected `&str`, found `String`                           │
│                                                            │
│  1 │ let mut s = String::new();                            │
│  2 │ s.push_str(format!("answer: {}", 42));                │
│    │            ^^^^^^^^^^^^^^^^^^^^^^^^^^                 │
│    │            expected `&str`, found `String`            │
│                                                            │
│  help: consider borrowing                                  │
│        s.push_str(&format!("answer: {}", 42));             │
│                                                            │
│  see snippet a1b2c3d4e5f6 (main.typ:17)                    │
└────────────────────────────────────────────────────────────┘
```

Style targets:

- Red 2px border, light-red 5% tint background, default text colour.
- Header strip in red; footer strip in mid-grey.
- Code rendered with the same font as the package's normal `rust()` code box,
  so the contrast is "this is the same code, but bad" not "this is a wholly
  different visual idiom".
- Carets / underlines drawn as Typst rules under the offending characters
  (computed from `byte_start`/`byte_end` mapped onto the rendered
  monospaced grid).
- Severity → border colour: error=red, warning=orange, note=yellow,
  panic/timeout/internal=red with a different header label (`panic`,
  `timeout`, `internal`).

A `theme: "auto" | "light" | "dark"` parameter passes through to the
`_evcxr-error-box` helper for users with dark-mode Typst themes.

A user-customizable hook is exposed: `evcxr.error-style.set(box: ..., header:
..., footer: ...)` — covered in T-D03's API surface (RECON-T-D03).

---

## 5. Terminal output (`evcxr-typst run|watch`)

**Decision.** Mimic evcxr's REPL — i.e. use ariadne — but with three
adjustments:

1. The "file name" in the ariadne report is `main.typ:<line>:<col>` of the
   Typst-level location of the snippet (resolved via `<evcxr-snippet>.loc`
   from the metadata query) **plus** `(snippet <id>)`. So the developer
   sees:

   ```
   error[E0308]: mismatched types
     ╭─[main.typ:17:1 (snippet a1b2c3d4e5f6):2:14]
     │
   2 │ s.push_str(format!("answer: {}", 42));
     │            ─────────────┬─────────────
     │                         ╰── expected `&str`, found `String`
     │
     ╰─ help: consider borrowing: `&format!("answer: {}", 42)`
   ```

2. Cross-snippet errors render with two source files in the ariadne `sources()`
   call — the offending snippet *and* the snippet referencing it — exactly as
   ariadne is designed for multi-file reports. Both show their `main.typ`
   location header.

3. Once per `evcxr-typst run` invocation, after the document is fully
   processed, a summary is emitted:

   ```
   evcxr-typst: 14 snippets processed; 2 errors, 1 warning, 11 ok.
     error  snippet a1b2c3d4e5f6 (main.typ:17:1) — E0308 mismatched types
     error  snippet 9988aabbccdd (main.typ:42:1) — runtime panic
     warning snippet 11223344 (main.typ:8:1) — unused variable `x`
   ```

In `watch` mode the same per-error and per-summary lines stream as snippets
are re-evaluated, prefixed with a timestamp.

We do **not** reimplement ariadne. We feed evcxr's existing
`CompilationError::build_report()` for compile-kind errors and synthesize
ariadne reports for our own kinds (panic, timeout, dep, internal) using the
sidecar JSON.

---

## 6. What evcxr already gives us, and what we synthesize

### Already in `evcxr/src/errors.rs` — reuse verbatim

| Type / fn | What it gives | How we use it |
|---|---|---|
| `CompilationError` | parsed rustc JSON | construct one error entry per `CompilationError` |
| `CompilationError::message()` | sanitized one-line | → `errors[].message` |
| `CompilationError::code()` | rustc code (E0308) | → `errors[].code` |
| `CompilationError::level()` | "error" / "warning" | → `errors[].severity` |
| `CompilationError::spanned_messages()` | primary + secondaries with `Span` | → primary_span + secondary_spans |
| `CompilationError::help_spanned()` + `::help()` | help children | → `errors[].helps` |
| `CompilationError::evcxr_extra_hint()` | E0597 etc. notes | → `errors[].evcxr_hint` |
| `CompilationError::rendered()` | full rustc terminal text incl. ANSI | → `rendered_terminal` |
| `CompilationError::is_from_user_code()` | filters out spans in generated wrapper | gate for whether we render at all |
| `CompilationError::primary_spanned_message()` | best-effort primary | → `errors[].primary_span` |
| `CompilationError::build_report(...)` + `Theme` | ariadne `Report` | terminal printing § 5 |
| `Span { start_line, start_column, end_line, end_column }` | 1-based line/col | converted to byte offsets via `span_to_byte_range()` (already in errors.rs) |
| `Error::SubprocessTerminated` | child died | → phase="runtime-panic" or "internal" |
| `Error::TypeRedefinedVariablesLost(vars)` | type swap dropped vars | rendered as a per-document warning banner (§ 1.f cousin) |

### What we synthesize (not in evcxr today)

- **Snippet-id tagging — parallel offset map (D-014).** evcxr has no
  concept of snippets and `CodeKind` is `pub(crate)`, so we don't extend
  it. Instead we keep a tiny in-memory map on the CLI side, populated as
  we feed snippets to `CommandContext::execute`:

  ```rust
  /// One entry per snippet ever fed to the current CommandContext, in
  /// the order it was fed. Cleared on `:clear` (watch-loop reset) and
  /// rebuilt as the linear replay re-feeds snippets.
  struct SnippetSubmission {
      snippet_id: String,        // <evcxr-snippet>.id
      src: String,               // exact bytes passed to execute()
      // Byte range *within the buffer we passed to execute()*. Because
      // we feed exactly one snippet per execute() call, this is always
      // (0, src.len()). Kept explicit for clarity / future flexibility.
      submitted_byte_start: usize,
      submitted_byte_end: usize,
  }

  struct OffsetMap {
      submissions: Vec<SnippetSubmission>,
      // Reverse lookup: a Span we get back from a CompilationError is
      // line/column-relative to the buffer we just submitted. Resolve
      // by converting line/col → byte offset (using the same
      // `span_to_byte_range` algorithm exposed in evcxr's errors.rs)
      // and matching against the most recently submitted snippet.
  }
  ```

  In v0 we only need the *current* submission to attribute compile errors
  raised by the just-submitted snippet (case § 1.a). Cross-snippet errors
  (§ 1.b — error in B about an item defined in A) currently surface with
  spans that point into evcxr's regenerated `items_code()` for the
  re-attached items, *not* into A's original src. To attribute those back
  to A, we additionally remember each snippet's contribution to
  `committed_state.items` keyed by `snippet_id`, and when we see a span
  in the wrapper-emitted items code we hash-match the source line back
  to the snippet that committed that item. Implementation detail in
  T-I07; the data structure is just `submissions` plus a
  `committed_items: HashMap<ItemName, SnippetId>` rebuilt from the same
  feed sequence.

  This avoids the upstream-patch coordination cost. It is strictly less
  precise than tagging `CodeKind::OriginalUserCode` directly — a future
  upstream contribution would simplify `committed_items` reconstruction —
  but it is sufficient for the rendering shapes in § 4. If span fidelity
  becomes a real problem in practice, revisit and propose the upstream
  patch then.

- **Byte offsets within snippet src.** evcxr's `Span` is line/column
  based; we convert to byte offsets using the same `span_to_byte_range`
  algorithm that `errors.rs` already has (we may want to expose it `pub` —
  tiny upstream patch, or copy the function).
- **Runtime panic capture** — distinct path from compile errors. We watch
  the child's stderr for `thread '<unnamed>' panicked at '…'` and parse
  out location + backtrace. Alternatively, evcxr's existing handling of
  `SubprocessTerminated` provides a fallback when the child died without a
  clean panic message.
- **Timeout enforcement** (§ 1.e) — fully ours.
- **Dep-resolution failure** — we already drive `:dep` ourselves; we capture
  cargo's stderr from the corresponding `CommandContext::execute` and tag
  it as phase="dep-resolution".
- **Cross-snippet attribution logic** (§ 3) — ours.
- **`<id>.error.json` sidecar writer** — ours.
- **Typst-side error box** (§ 4) — ours.
- **Per-run summary** (§ 5) — ours.

---

## 7. Run-mode vs watch-mode behaviour

**Run mode.** Recommended: **keep going on first error**, evaluate every
snippet, emit all error sidecars, exit with non-zero status if any snippet
errored.

```
evcxr-typst run main.typ
  → snippet 1 ok
  → snippet 2 ERR  (E0308)
  → snippet 3 ok        # uses no items from snippet 2
  → snippet 4 ERR  (uses item that 2 was trying to define)
  → snippet 5 ok
exit 1
```

Rationale:

1. The reader of the rendered PDF wants to see *all* current problems, not
   just the first one. Iterating "fix one error, re-run, see next error"
   wastes evcxr-typst startup cost (which is the rustc cache warm-up).
2. The doc still produces a PDF (the failing snippets show error boxes), so
   the "run" command produces useful output even on failure. Behaviour
   matches `cargo build` (continues collecting errors per crate) more than
   `cargo test --fail-fast` (stops on first).
3. Non-zero exit code makes CI treat any error as a build failure, which is
   what most users want.

Override: `evcxr-typst run --fail-fast` for the rare "stop at first error and
don't even run typst compile" scenario.

**Watch mode.** Always keep going. There's no "exit code" in watch mode; the
status is the most recent run's per-snippet outcomes, surfaced in:

- the rolling terminal output;
- the rendered PDF (which `typst watch` re-renders from sidecars);
- a single `.evcxr-typst-cache/_status.json` written after every run, that
  the user could tail or that an editor extension could surface.

Errors **never** kill the watch loop. The watch loop only exits on
SIGINT/SIGTERM, or on internal evcxr crashes that our retry budget can't
recover from (3 child re-spawns within 10s → bail with an explicit message).

---

## 8. Open questions

1. **(Cross-snippet attribution implementation: where does the snippet-id
   tag live?)** **Resolved (D-014):** parallel offset map on the
   `evcxr-typst` side, structure described in § 6 above. No upstream patch
   in v0; a future upstream change to `CodeKind::OriginalUserCode` would
   simplify cross-snippet item attribution but is not blocking.

2. **(Are there errors evcxr produces that aren't `CompilationError`s but
   still need to surface as snippet errors?)** Specifically
   `Error::TypeRedefinedVariablesLost(Vec<String>)` — when a snippet
   redefines a struct used elsewhere, evcxr drops bindings of that type and
   tells us which. Today the natural rendering is a per-document banner,
   but should it also flag every snippet that *had* such a binding? Probably
   yes for usability, but doing so requires us to track binding-type
   provenance that evcxr does not currently expose.

3. **(Panic + display-object interaction.)** A `rust-display()` snippet that
   prints a plot via the display protocol *and then* panics: do we render
   the (probably-incomplete) image alongside the error box, or hide it? My
   instinct says hide and only show the error, because partial images are
   misleading. But the analogous `rust-out()` *does* show partial stdout.
   Inconsistent — pick one. Recommend: show partial in both; user can
   distinguish via the box.

4. **(`rust-data()` failure mode.)** **Resolved (D-015):** option (a),
   return `none` on error and emit a sibling error box. The unevaluated
   case (no sidecar yet) returns the user-supplied `fallback:` value
   (default `(:)`) so `typst compile` works under D-004. Sentinel dicts
   silently propagate corrupt data into downstream layout; hard-fail
   defeats § 0. See `package-api.md` § 2.5 for the full three-way return.

5. **(`--allow-eval` + a snippet errors → what's the behaviour without
   `--allow-eval`?)** We don't run any Rust without `--allow-eval`, so
   compile errors cannot occur — all snippets render as fallback
   placeholders. But stale `<id>.error.json` files from a prior run could
   confuse the package. Decision: in non-`--allow-eval` mode the CLI still
   sweeps the cache directory and **deletes** stale error sidecars so the
   doc renders cleanly. (Cross-link to T-D04 cache invalidation.)

6. **(Severity threshold for non-zero exit.)** Errors → exit 1, obviously.
   Warnings → exit 0 by default, but `--werror` to upgrade. Sane?

---

## Summary of decisions

- Six error kinds: compile, cross-snippet compile, runtime-panic,
  dep-resolution, timeout, internal. (§ 1)
- Cross-snippet errors render the box at the *referencing* snippet, with the
  defining snippet linked. (§ 3)
- Timeout: yes, default 30s, configurable via CLI flag and per-snippet
  parameter. (§ 1.e)
- Sidecar at `<id>.error.json`, schema v1, errors[] array, byte-offset
  spans, severity per error, panic/timeout/dep sub-objects. (§ 2)
- Terminal: ariadne via evcxr's existing `build_report()`, with multi-source
  for cross-snippet and a per-run summary footer. (§ 5)
- Run mode: keep going, exit non-zero. Watch mode: keep going forever. (§ 7)
- Reuse all of `errors.rs`; snippet-id attribution via a parallel offset
  map maintained by `evcxr-typst` (D-014), no upstream patch in v0.
  (§ 6, § 8.1)
