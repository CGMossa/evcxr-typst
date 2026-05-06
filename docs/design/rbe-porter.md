# `tools/rbe-port/` — deep design

> Companion to `docs/tracks/rust-by-example-port.md`. The track plan is the *what* and *why*; this doc is the *how*. Specifies the porter implementation in enough detail that T-B01 is agent-driveable.

## Crate structure

```
tools/rbe-port/
├── Cargo.toml                          its own [workspace] block — does NOT
│                                        join the parent workspace (so we can
│                                        carry tooling-only deps without
│                                        polluting evcxr-typst's dep graph)
├── src/
│   ├── main.rs                         clap CLI entrypoint
│   ├── lib.rs                          public library API for testing
│   ├── scan.rs                         line-walking markdown scanner
│   ├── snippet.rs                      syn-based fn-main detection
│   ├── emit.rs                         Typst output writer
│   ├── summary.rs                      SUMMARY.md → outline tree
│   ├── manifest.rs                     manifest.json (de)serialisation
│   └── cli.rs                          clap structs
└── tests/
    ├── golden.rs                       walks tests/golden/ comparing outputs
    └── golden/
        ├── 01-hello/
        │   ├── input.md
        │   └── expected.typ
        ├── 02-struct-with-helpers/
        │   ├── input.md
        │   └── expected.typ
        └── …
```

`tools/rbe-port` is workspace-isolated by appending `[workspace]` to its `Cargo.toml`. Same pattern as the WASM-spike crate would use (see `docs/design/wasm-plugin-analyzer.md` § "Mechanism"). Reasons: (a) tooling-only dependencies (`syn`, `regex`, `sha2`) shouldn't appear in `evcxr-typst`'s lockfile; (b) the porter is a one-time-per-port-run tool, not a runtime dependency.

## Why a hand-written scanner, not `pulldown-cmark`

rust-by-example markdown is straightforward: prose with fenced code blocks and a small tail of common inline syntax (links, emphasis, headings, lists). The transformations we need are mostly:

- Identify fenced code blocks (boundary regex; trivial)
- Pass prose through with a few sed-style rewrites (`# H` → `= H`, `**bold**` → `*bold*`, `*emph*` → `_emph_`, `[text](url)` → `#link("url", [text])`)
- Resolve mdBook ref-style links (a one-pass collection at the bottom of the file, then substitution)
- Recognise `### Activity`-style callout headings

A markdown AST is overkill for that. `pulldown-cmark` would force us to enumerate dozens of event variants we don't care about (raw HTML inlines, emphasis nesting, image syntax we don't use, etc.) and the round-trip "events back to markdown then to typst" is its own bug surface.

The hand-written scanner uses regex to find fences and a small line-by-line state machine for everything else. Estimated 200–400 lines including the rewrite table. **Escalate to `pulldown-cmark` (or `comrak`) only if the scanner proves insufficient**, captured as an open question below. So far on the rust-by-example corpus, no edge case I've seen needs an AST.

`syn`, on the other hand, **is** load-bearing: detecting `fn main()` cleanly across all the corner cases in § "Snippet detection" requires Rust syntax-tree access. No regex hack survives `#[tokio::main] async fn main()`.

## Dependencies

```toml
# tools/rbe-port/Cargo.toml
[package]
name = "rbe-port"
version = "0.0.1"
edition = "2024"
publish = false

[dependencies]
clap     = { version = "4", features = ["derive"] }
anyhow   = "1"
regex    = "1"
syn      = { version = "2", features = ["full", "extra-traits"] }
serde    = { version = "1", features = ["derive"] }
serde_json = "1"
sha2     = "0.10"        # SHA-256 for manifest
walkdir  = "2"           # tree walk during scan + check

[dev-dependencies]
pretty_assertions = "1"  # readable goldens diffs

[workspace]              # isolate from parent workspace (tooling deps)
```

Notably **not** depending on:

- `pulldown-cmark` / `comrak` / `markdown-rs` — see § "Why a hand-written scanner".
- `mdbook` — would force the porter to run as an mdBook preprocessor; ties us to mdBook's lifecycle. We just want to read `.md` files.
- `insta` — golden tests use literal `expected.typ` files (see § "Golden tests").

## CLI

```
rbe-port -i <input-dir> -o <output-dir> [--phase B1|B2|B3|B4|B5|B6|all] [--check]
```

- `-i, --input` — path to a rust-by-example checkout (the `src/` parent — i.e. the dir containing `book.toml` and `src/`). Required.
- `-o, --output` — destination, typically `examples/rust-by-example/`. Required.
- `--phase` — limits which SUMMARY subtree to port. `all` skips no chapters; `B1` etc. select per the track plan. Default: `all`.
- `--check` — does not write anything; re-converts and diffs against existing on-disk output. Exits non-zero on drift. Used in CI to detect "someone hand-edited the porter output."

Always emits `manifest.json` at `<output-dir>/manifest.json` reflecting the run.

## Conversion pipeline

```
input.md  ──┐
            │ scan.rs::scan_md
            ▼
       Vec<Block>            // Heading | Paragraph | Fence{lang, body, info} |
            │                //   List | Callout | Link | Raw{verbatim}
            │ emit.rs::emit
            ▼
       String (output.typ)
            │
            │ manifest.rs::record
            ▼
       manifest.json entry
```

`Block` is the porter's intermediate representation. It's flat, not nested — one entry per top-level markdown element. Lists are a single `Block::List` carrying their items; nested lists become a `BlockNested`. We do not chase markdown's full tree because we don't need to.

`scan_md`'s state machine:

```
state = Outside
for line in input:
    match (state, classify(line)):
        (Outside, Fence(lang, info))     → state = InsideFence(lang, info, [])
        (InsideFence(l, i, b), Fence(_)) → emit Block::Fence{l, i, b.join("\n")}; state = Outside
        (InsideFence(l, i, b), other)    → b.push(line)
        (Outside, Heading(level, text))  → emit Block::Heading{level, text}
        (Outside, blank line)            → emit Block::ParagraphBreak
        (Outside, prose)                 → buffer until next blank → Block::Paragraph
```

Inline rewrites (`*emph*` → `_emph_` etc.) happen at `emit` time, applied per-block to prose contents. The same emit pass also resolves cross-chapter links by consulting the SUMMARY tree to map `.md` → relative `.typ` path.

## Snippet detection (the meat)

For each `Block::Fence{lang: "rust"|"rust,editable"|...}`, run:

```rust
fn classify_snippet(rust_src: &str) -> SnippetKind {
    let parsed = syn::parse_file(rust_src);
    match parsed {
        Err(_) => SnippetKind::Unparseable,    // emit as plain rust(); the
                                                // CLI will surface the syntax
                                                // error at eval time
        Ok(file) => {
            let mains: Vec<_> = file.items.iter().filter(|item| matches!(
                item, syn::Item::Fn(f) if f.sig.ident == "main"
            )).collect();
            match mains.as_slice() {
                []           => SnippetKind::Plain,
                [m]          => classify_main(m),
                multiple     => SnippetKind::MultipleMain,  // warn + last wins
            }
        }
    }
}

fn classify_main(m: &syn::ItemFn) -> SnippetKind {
    let async_     = m.sig.asyncness.is_some();
    let attr_main  = m.attrs.iter().any(|a| a.path().is_ident("tokio::main")
                                          || a.path().is_ident("async_std::main"));
    match (async_, attr_main) {
        (true,  true)  => SnippetKind::AsyncRuntimeMain,
        (true,  false) => SnippetKind::AsyncMain,        // bare `async fn main`
                                                          // — needs runtime
        (false, _)     => SnippetKind::SyncMain,
    }
}
```

Mapping to package functions:

| `SnippetKind` | Becomes | `options.auto-call` | Notes |
|---|---|---|---|
| `Plain` | `rust(...)` | absent | No `main`; either expressions or item-only definitions. |
| `SyncMain` | `rust-main(...)` | `"main"` | The common case. |
| `AsyncRuntimeMain` | `rust-main(...)` | `"main"` plus `options.auto-call-await: true` | `#[tokio::main]` removes the attribute at port time and emits the body wrapped — actually leave that for evcxr (it auto-spins-up tokio per `COMMON.md`). Just emit `rust-main(...)` with the source verbatim and let evcxr's await detection kick in. |
| `AsyncMain` | `rust-main(...)` with a comment header noting "evcxr auto-tokio applies" | `"main"` | Same. |
| `MultipleMain` | `rust(...)` | absent | Warn at port time. We don't try to disambiguate — these are pathological in rust-by-example and likely indicate the chapter being mid-edit upstream. |
| `Unparseable` | `rust(...)` | absent | Pass through; CLI emits a real syntax error in its rendered box per T-I07. |

## Code-block tag matrix

The fence info-string after the language. rust-by-example uses these flags:

| Tag | Becomes | `options` flags set | Notes |
|---|---|---|---|
| `rust` | per snippet detection above | — | The default. |
| `rust,editable` | same | — | The `editable` flag is an mdBook Run-button hint; ignore. |
| `rust,ignore` | per snippet detection, plus `options.skip-eval = true` | `skip-eval` | Source rendered, no evaluation. CLI honors `skip-eval` and emits no sidecars beyond a stub recording the skip. |
| `rust,no_run` | same as `ignore` | `skip-eval` | rust-by-example treats them equivalently. |
| `rust,compile_fail` | `rust(...)` plus `options.expected-error = true` | `expected-error` | The CLI evaluates; if it errors, that's success — no error box. If it succeeds, that's the unexpected case — render an "expected error did not occur" warning. Implies T-I07 for the styling. |
| `rust,should_panic` | same, plus `options.expected-panic = true` | `expected-panic` | Same logic for runtime panics. |
| `rust,edition2018` / similar | warn; emit as `rust` | — | rust-by-example uses these on a few chapters. We always run on the workspace edition; surface the deviation in the porter log. |
| `bash`, `sh` | `#raw(block: true, lang: "bash", "...")` | n/a | Documentation-only; never executed. |
| `text` | `#raw(block: true, "...")` (no `lang`) | n/a | mdBook uses these for *expected* output blocks. We render verbatim. |
| `toml` | `#raw(block: true, lang: "toml", "...")` | n/a | Cargo chapter. |
| `console` / `output` | `#raw(block: true, "...")` | n/a | Treated as `text`. |

The new `options` keys (`skip-eval`, `expected-error`, `expected-panic`, `auto-call`, `auto-call-await`) are all additive per D-019 — no schema-version bump. They get added to `docs/design/package-api.md` § 5.1 when T-B01 ships.

## Cross-link resolution

mdBook supports two link styles:

```markdown
inline: [text](other.md)
ref:    See [text][some-key]   ...later in the file...   [some-key]: other.md
```

Both resolve at port time:

1. First pass: collect all `[key]: dest` definitions in the file. Build a `HashMap<String, String>`.
2. Second pass: emit. For each link:
   - If inline `[text](dest)` and `dest` ends in `.md`: rewrite to `#link(<chapter-label-of-other.typ>, [text])`.
   - If ref `[text][key]`: look up `key` in the map; rewrite as above.
   - If `dest` is an external URL: emit as `#link("url", [text])` unchanged.

Each ported chapter file emits a Typst label at its top heading: `= Chapter Title <chapter-other-typ>`. The chapter-label name is derived from the relative path: `mod/use.md` → `<chapter-mod-use-typ>`. Mechanical and stable.

## Determinism rules

Same input bytes → byte-identical output. Required for:

- Reproducible CI (`rbe-port --check` works only if outputs are stable).
- Sensible `git diff` when re-porting after upstream changes.

Concretely:

1. Always LF line endings. Convert CRLF input.
2. Always UTF-8, no BOM. Reject non-UTF-8 input with a clear error.
3. Always exactly one trailing newline.
4. Metadata fields in `<evcxr-snippet>` are emitted in a fixed order: `v, kind, id, src, deps, options`.
5. `options` keys within snippets are emitted in a fixed alphabetical order.
6. Quote style for Typst strings: double-quotes everywhere. No mixed.
7. Header comment in each output file is byte-identical: `// Adapted from rust-by-example/<src-relative>.md (see ../NOTICES.md)\n`. The `<src-relative>` is the input's path relative to the input dir's `src/`.

Property: running the porter twice with the same inputs produces zero `git diff`.

## Manifest format

`<output-dir>/manifest.json`:

```json
{
  "tool_version": "0.0.1",
  "rbe_commit_sha": "abc1234567...",   // captured via `git -C <input-dir> rev-parse HEAD`
  "ported_at": "2026-05-06T22:00:00Z",
  "phase": "B1",
  "files": {
    "hello.typ": {
      "input_path": "hello.md",
      "input_sha256": "...",
      "output_sha256": "...",
      "snippet_kinds": ["sync_main"]
    },
    "primitives/literals.typ": {
      "input_path": "primitives/literals.md",
      "input_sha256": "...",
      "output_sha256": "...",
      "snippet_kinds": ["sync_main", "plain"]
    }
  }
}
```

`--check` mode reads existing `manifest.json` (if present), computes a fresh one, diffs. Drift is reported per-file with a hint about which side changed (input vs output). CI runs `rbe-port --check` against a vendored input snapshot.

## Golden tests

Layout under `tools/rbe-port/tests/golden/`:

```
01-hello/
├── input.md
└── expected.typ
02-struct-no-main/
├── input.md
└── expected.typ
03-tokio-main/
├── input.md
└── expected.typ
…
```

Test harness (in `tools/rbe-port/tests/golden.rs`):

```rust
#[test]
fn all_goldens() {
    for case in walkdir::WalkDir::new("tests/golden").min_depth(1).max_depth(1) {
        let dir = case.unwrap().path().to_owned();
        let input = std::fs::read_to_string(dir.join("input.md")).unwrap();
        let expected = std::fs::read_to_string(dir.join("expected.typ")).unwrap();
        let actual = rbe_port::convert(&input).unwrap();
        pretty_assertions::assert_eq!(expected, actual,
            "golden mismatch in {}", dir.display());
    }
}
```

Why literal `expected.typ` files, not `insta` snapshots:

- The expected output is a small, stable artifact worth eyeballing in PR diffs. Snapshots tend to "drift quietly" — devs accept changes without reading them.
- Determinism is a contract; literal goldens make a `cargo test` failure obvious. Snapshots invite "just rerun with `INSTA_UPDATE=1`."
- One less dev-dep.

`rbe-port --golden` (or a small make/cargo target) regenerates expected.typ files in bulk after intentional changes. Change discipline: regeneration is a separate commit from the porter logic change, so review can see the output delta cleanly.

## Required golden cases (T-B01 must include all)

| Case | What it tests |
|---|---|
| `01-hello` | Plain `fn main()` with `println!`. |
| `02-struct-with-helpers-no-main` | Snippet defining types and helper fns; no `main`. Should emit `rust(...)`, not `rust-main`. |
| `03-tokio-main` | `#[tokio::main] async fn main()`. Should emit `rust-main(...)` with source verbatim. |
| `04-bare-async-main` | `async fn main()` without an attribute. Same as 03 (evcxr auto-tokio kicks in). |
| `05-rust-ignore-tag` | Fenced as `rust,ignore`. Should set `options.skip-eval`. |
| `06-rust-compile-fail` | Fenced as `rust,compile_fail`. Should set `options.expected-error`. |
| `07-multiple-main` | Two `fn main()` defs in one snippet. Should emit `rust(...)` with a porter warning. |
| `08-cross-md-link` | Prose with `[macros]: macros.md`. Should rewrite to `#link(<chapter-macros-typ>, [macros])`. |
| `09-text-output-block` | A `text` fence. Should emit `#raw(block: true, ...)` with no `lang`. |
| `10-chapter-with-bash-block` | A `bash` fence (cargo chapter). Should emit `#raw(block: true, lang: "bash", ...)`. |

Add cases as the corpus surfaces new patterns.

## Open questions

1. **`#[tokio::main]` attribute removal.** Should the porter strip the attribute at port time so the user-visible Rust is the inner `fn main()`, or leave it verbatim and let evcxr's auto-tokio do its thing? Recommendation: **leave verbatim**. Faithful to upstream; evcxr handles it. Test against a tokio-using chapter early in B6 to confirm.
2. **Activity / Exercise heading detection.** rust-by-example chapters often end with `### Activity` or `### Exercise` followed by a small task. Should the porter style these as a callout block? Recommendation: **yes, but as a v0.x polish** — start with plain heading rendering, add the callout style as a follow-up after measuring how often it'd fire.
3. **Comment-line preservation in Rust source.** The porter passes the snippet body verbatim to `_emit-snippet`. Comments survive. But what if a comment contains markdown-special chars (e.g. `// see [foo](bar.md)`)? The Typst raw block won't interpret it. Should be fine — `rust(```rust ... ```)` treats the body as raw. Verify with a golden.
4. **Should the porter validate against the package's metadata schema?** Tempting (would catch porter bugs early) but couples the porter to `evcxr-typst`'s release cadence. Recommendation: **no formal validation in porter** — let `typst compile --root . examples/rust-by-example/main.typ` be the integration test. Cheaper.
5. **mdBook preprocessor variants.** A few rust-by-example chapters use `{{#playground …}}` mdBook macros for the Run button. Recommendation: **drop them**; not meaningful in our render. Detect with a regex and elide.

## References

- `docs/tracks/rust-by-example-port.md` — the parent track doc; this file details its § "Tooling" section.
- `docs/DECISIONS.md` D-022 (track scope), D-019 (additive `options` keys don't bump schema), D-018 (multi-file model).
- `docs/design/package-api.md` § 5 — the metadata schema this porter emits against.
- `.rust-by-example/src/SUMMARY.md` — the canonical chapter ordering.
