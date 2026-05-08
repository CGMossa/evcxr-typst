# Backlog

Agent-ready task queue. Pick the top **open** task whose dependencies are all `done`.

For each task: read the **Reference reads** before starting, satisfy the **Done when** checklist, then mark the task `done` here with a one-line summary and a commit/PR link.

Status legend: `open` · `in-progress` · `done` · `blocked` · `superseded`

---

## Phase 0 — design

> All six T-D0x tasks landed in commit 954e3a2 as parallel agent drafts and were reconciled into ARCHITECTURE.md / DECISIONS.md in a follow-up commit. They appear in **Done** at the bottom. The follow-up reconciliation tasks T-D07–T-D10 below cover the open questions that surfaced.

### T-D07 · Reconcile open questions left by Phase-0 drafts

- **Status:** done · 038d2bc · D-012..D-016 added · `docs/design/{package-api,snippet-semantics,errors,watch-loop,cache,examples/index}.md` updated in place
- **Phase:** 0 (design follow-up)
- **Depends on:** —
- **Reference reads:** all of `docs/design/*.md` and `docs/DECISIONS.md` D-007..D-011
- **Briefing:** Resolve the explicit open questions and bikesheds left by the parallel Phase-0 drafts. Concrete punch list:
  - `package-api.md` § 7 bikesheds 1–6: pick names (`rust` vs `eval`, `rust-out` vs `rust-stdout`, `rust-display` vs `rust-show` vs `rust-render`, `rust-hidden` vs `rust-setup`, `rust(...)` default `show:`, `dep` overload). Read at least three gallery examples and check the chosen names read naturally in flow.
  - `snippet-semantics.md` open Q1: should `dep()` be document-level (a top-of-document prelude block) or remain inline-anywhere? Resolve in coordination with T-D03's chosen API.
  - `errors.md` open Q1: where does the snippet-id tag live (upstream patch to evcxr's `CodeKind::OriginalUserCode` vs. a parallel offset map maintained by `evcxr-typst`)? Decide; if upstream patch, add it as a separate evcxr-side task.
  - `errors.md` open Q2: `rust-data()` failure-return shape (`none` vs sentinel dict vs hard fail). Decide together with package-api `rust-data` semantics.
  - `watch-loop.md` open Q1: skip-sidecar-write when bytes are unchanged? Reconcile with cache.md.
  - `watch-loop.md` open Q2: multi-`.typ`-file projects with `#import` / `#include` — does `typst query` already report the imported-file set, or do we have to walk imports ourselves?
- **Output:** Updates to the relevant design files (in place) plus a new `docs/DECISIONS.md` entry per resolved question. Mark each bullet above with the resolution.
- **Done when:** every bullet has an explicit resolution; new decision entries added; design-file open-question sections updated to "resolved (D-xxx)".

### T-D08 · Decide on per-snippet `timeout:` kwarg in the package API

- **Status:** done · D-017 · `docs/design/package-api.md` (new § 2.8, signatures updated, deferred entry removed); `docs/design/errors.md` (§ 1.e expanded, RECON-T-D03 flags resolved); `docs/DECISIONS.md` (D-017 added)
- **Phase:** 0 (design follow-up)
- **Depends on:** T-D07
- **Reference reads:** `docs/DECISIONS.md` D-009; `docs/design/errors.md` § 1.e; `docs/design/package-api.md` § 6
- **Briefing:** D-009 deferred per-snippet timeout overrides because evcxr's child-cancellation semantics weren't clear. Read `evcxr/src/eval_context.rs` for what `execute` actually does on cancellation; decide whether `rust(..., timeout: 5min)` is shippable in v0 or stays deferred. Either way, document the decision and update the `errors.md` RECON-T-D03 flag.
- **Done when:** decision recorded as a new D-xxx entry; `errors.md` flag resolved; `package-api.md` § 6 updated accordingly.
- **Resolution:** Shipped in v0 (D-017). evcxr's `ChildProcess` only exposes SIGKILL; that's identical to what D-009's global timeout already uses, so per-snippet adds no new cancellation primitive — only a per-call duration. Kwarg accepts `auto`/`none`/`duration`/`<int seconds>`/`<int>(ms|s|min|h)`; per-snippet wins unconditionally over `--snippet-timeout`; applies to all five eval functions (not `dep()`); documented cargo-runtime floor and `:dep` race.

### T-D09 · Multi-document and multi-file project layout

- **Status:** done · `docs/design/multi-file.md` · D-018 added; `watch-loop.md` § 9 Q2 resolved
- **Phase:** 0 (design follow-up)
- **Depends on:** T-D07
- **Reference reads:** `docs/design/watch-loop.md` open Q2; `docs/design/cache.md` § "Cache layout on disk" (cache lives at workspace level)
- **Briefing:** A real Typst project rarely lives in one `.typ` file. Designing for `#import "chapter1.typ"` etc.: where does the cache live, how do snippets in `chapter1.typ` reach `dep()`s declared in `main.typ`, what's the watch-set, what's the run command (one `main.typ` is the entry, dependent files are auto-discovered)? Probably v0 supports a single entry file + auto-discovered imports, multi-entry-file projects deferred.
- **Output:** new `docs/design/multi-file.md`.
- **Done when:** the file exists; covers cache scope, watch-set discovery, dep visibility across files, entry-file selection on the CLI.
- **Resolution:** v0 = single entry file; cache rooted at entry-file parent (CAS shared across entry files in the same workspace, id-addressed view per entry); discovery = BFS from entry parsing local `#import`/`#include` via `typst-syntax`, with `evcxr-typst.toml` as an opt-in override; global snippet order is `(file_seq, doc_order_within_file)`; `dep()` visibility is global by document order; ID collisions are project-wide (default→suffix, explicit→hard error); watch set is the union of all member files, recomputed each cycle. See `docs/design/multi-file.md` and D-018.

### T-D11 · WASM analyzer plugin (folded into side-track S4)

- **Status:** superseded by **T-S04** in the Side tracks section · D-020
- **Phase:** n/a (off main path)
- **Notes:** Originally framed as a Phase 5 add-on to the main plan. After expanding the design we recognised it's part of a broader "semantic Typst" feature surface whose first three slices (type-of / signature-of / docs / items-table / refs / diagnostics) can ship via CLI sidecars without the WASM plugin investment. The plugin specifically remains the bigger fourth slice. See `docs/tracks/semantic-typst.md` and T-S04.

---

### T-D10 · Schema versioning policy

- **Status:** done · `docs/design/schema-versioning.md` · D-019
- **Phase:** 0 (design follow-up)
- **Depends on:** —
- **Reference reads:** ARCHITECTURE.md § "The metadata contract"; package-api.md § 5; errors.md § 2; cache.md § "Cache layout"
- **Briefing:** Three `v` fields exist in the wild: `<evcxr-snippet>.v`, `<evcxr-dep>.v`, `<id>.error.json.v`. Plus a CAS layout `v1/`. Document policy: when do we bump? what's backward-compat strategy? what's the minimum-CLI-version-required mechanism so a Typst package release can refuse an old CLI cleanly?
- **Output:** new `docs/design/schema-versioning.md` (~1 page).
- **Done when:** the file exists; covers all four version fields and the CLI/package compatibility check.
- **Resolution:** Major-breaking-only bumps for all four `v` fields (currently `1`); CLI and package semver track independently. Min-CLI declared via `min-cli: "X.Y.Z"` kwarg on `setup()`; CLI reads the resulting `<evcxr-min-cli>` marker during `typst query` and exits 2 if its own version is too old. No min-package check (asymmetric — CLI is authoritative). Cache migrations are side-by-side directories (`v1/` preserved when `v2/` lands). Unknown sidecar `v` renders as a `schema mismatch` error box.

---

### T-D01 · Snippet semantics & dependency model

- **Status:** done · 954e3a2 · `docs/design/snippet-semantics.md`
- **Phase:** 0 (design)
- **Depends on:** —
- **Reference reads:**
  - `.evcxr/COMMON.md` (whole file — variable persistence, `:dep`, references)
  - `.evcxr/evcxr/src/eval_context.rs` (skim `ContextState`, item/var tracking)
  - `.evcxr/evcxr/src/use_trees.rs` (how `use` statements are merged)
  - `docs/ARCHITECTURE.md` § "Composition across snippets" in this repo
- **Briefing:** Design how Rust constructs compose across Typst snippets. Specifically: `struct`/`enum`/`trait`/`impl`/`fn`/`mod`/`use`/`let` defined in snippet A and consumed in snippet B (where B may be many snippets later, on a different page). Document what evcxr already gives us for free (most of it) and where the gaps are. Cover at minimum: (1) the persistence behavior of each construct kind; (2) the variable-reference limitation and how we surface it to writers; (3) cross-snippet macros (`macro_rules!` works; external `#[macro_use]` does not — confirm); (4) what happens when a later snippet redefines an item; (5) `mod foo;` (file-based modules) — does that even make sense in this context, and if so how do paths resolve?
- **Output:** `docs/design/snippet-semantics.md` covering the matrix of constructs, the rules, examples of each, and a list of "things that look like they should work but don't" with a recommendation for each (error nicely vs. document vs. work around).
- **Done when:** the file exists, every Rust top-level construct is covered, and the doc cross-references at least three concrete examples slated for the gallery (T-D02).

---

### T-D02 · Example gallery design

- **Status:** done · 954e3a2 · `docs/design/examples/` (8 `.typ` + `index.md`)
- **Phase:** 0 (design)
- **Depends on:** —
- **Reference reads:**
  - `.evcxr/evcxr_jupyter/samples/evcxr_jupyter_tour.ipynb` (gold standard for "what kinds of things do people do in a Rust notebook")
  - `docs/ARCHITECTURE.md` (full)
  - `docs/PLAN.md` § Phase 1, Phase 2
- **Briefing:** Design the example gallery — concrete `.typ` documents that show off the integration. Each example should have a clear "this is the feature being shown" focus, and the set together should cover the spectrum from trivial to ambitious. Required scenarios: (a) hello world (println), (b) define a struct in one snippet, use it pages later, (c) define a module and use items from it, (d) generate a plot/image and embed it inline, (e) pull a `:dep` and use a real third-party crate, (f) async/await with tokio runtime, (g) an error case (compile error inside a snippet) — what does the rendered doc look like, (h) ambitious: a small "report" with five interleaved snippets where each builds on the last (a mini data-analysis with computed tables). For each: write the *intended* `.typ` source assuming the package and CLI exist, and a paragraph of prose explaining what it demonstrates.
- **Output:** `docs/design/examples/` directory with one `.typ` per scenario plus a `README.md` indexing them. The `.typ` files don't need to render; they're spec, not implementation.
- **Done when:** all eight required scenarios exist as `.typ` files, the README explains each, and at least three of them assume cross-snippet item composition (so they exercise T-D01's design).

---

### T-D03 · Typst package API surface

- **Status:** done · 954e3a2 · `docs/design/package-api.md` (open names bikeshed → T-D07)
- **Phase:** 0 (design)
- **Depends on:** T-D02 (need example syntax to validate naming)
- **Reference reads:**
  - `.prequery/README.md` and `.prequery/src/` (look at the package API there; it's a good model)
  - `.typst-wasm-minimal-protocol/examples/hello_rust/` (just for Typst-side ergonomics)
  - `docs/design/examples/` (output of T-D02 — the API has to make those examples readable)
  - Typst docs on `metadata`, `query`, `raw`, `image`, `cbor`, `json` (already mostly familiar)
- **Briefing:** Design the public API of the `packages/evcxr/` Typst package. Function names, signatures, defaults, output shape. Decide on: how do users pass code (raw blocks vs strings), how is an explicit ID specified, how does the package surface plain stdout vs display output vs both, what's the fallback rendering, what configuration is package-level (a `setup()` call?) vs per-call. Bikeshed naming a little — `rust()` vs `evcxr()` vs `eval()` matters because it's user-facing.
- **Output:** `docs/design/package-api.md` with: every public function, its signature, its semantics, an example. Plus a "deferred to v1" section for things we don't ship in v0 but want to design-for.
- **Done when:** every example in `docs/design/examples/` parses cleanly under the proposed API (i.e. you can read the example and the API doc agrees on what each call means). At least one round of "would this name confuse a non-Rust Typst user" sanity-check is in the doc.

---

### T-D04 · Snippet identity & cache key

- **Status:** done · 954e3a2 · `docs/design/snippet-identity.md`, `docs/design/cache.md` · D-005 superseded by D-007; D-010 added
- **Phase:** 0 (design)
- **Depends on:** —
- **Reference reads:**
  - `docs/ARCHITECTURE.md` § "Snippet identity", § "Caching"
  - `docs/DECISIONS.md` D-005 (proposed)
- **Briefing:** Pin down snippet identity and the cache-key formula. Address: (1) default ID = content hash, what hash and how long; (2) collision handling when two snippets are byte-identical (e.g. two `println!("hi")` calls in different sections); (3) cache key for snippet output — what's the full set of inputs that should invalidate it (own source, prior snippet sources for items they introduce, active deps, evcxr version, rustc version, target triple, env-vars passed through?); (4) how does the cache interact with evcxr's own `:cache`. Either confirm D-005 or supersede it.
- **Output:** `docs/design/snippet-identity.md` covering identity. `docs/design/cache.md` covering the output cache (separate file because cache is meaty enough to deserve it). Both cross-link.
- **Done when:** both files exist; either D-005 is upgraded to `accepted` in DECISIONS.md or a new entry supersedes it; the cache key is described as a concrete formula.

---

### T-D05 · Watch loop & change classification

- **Status:** done · 954e3a2 · `docs/design/watch-loop.md` (multi-file Q → T-D09; skip-on-unchanged Q → T-D07)
- **Phase:** 0 (design)
- **Depends on:** T-D04 (cache key feeds into the change classification)
- **Reference reads:**
  - `docs/ARCHITECTURE.md` § "Watch loop"
  - `docs/DECISIONS.md` D-003 (linear re-eval policy)
  - `.evcxr/evcxr/src/command_context.rs` (what state-reset operations are available — `:clear`, etc.)
- **Briefing:** Detailed algorithm for `evcxr-typst watch`. Pseudocode for the change loop. Address: how do we tell `typst watch` (running as a child) about sidecar updates (mtime should suffice — verify), how do we debounce file events from multiple editors, what happens on transient parse errors in the `.typ` file, how do we shut down cleanly, what's logged where. Concrete rules for the change classification mentioned in ARCHITECTURE.md (added-at-end / removed-at-end / leaf-modified / non-leaf-modified). Define "leaf" precisely — does a snippet that only `println!`s but inside its body declares a `let` count as a leaf? (Answer: yes, because `let` inside a block doesn't escape.)
- **Output:** `docs/design/watch-loop.md` with pseudocode and the classification rules.
- **Done when:** the file exists; pseudocode is specific enough that someone implementing it doesn't have open design questions; classification rules cover the cases listed plus at least three I haven't anticipated.

---

### T-D06 · Error reporting & diagnostic plumbing

- **Status:** done · 954e3a2 · `docs/design/errors.md` · D-009 added (timeout 30s); D-011 added (panic resets state)
- **Phase:** 0 (design)
- **Depends on:** T-D03 (need to know how the package surfaces things)
- **Reference reads:**
  - `.evcxr/evcxr/src/errors.rs` (compilation error structure, spans)
  - `.evcxr/evcxr_repl/src/bin/evcxr.rs` (how the REPL renders errors with `ariadne`)
  - `docs/design/package-api.md` once T-D03 is done
- **Briefing:** Design how compilation/runtime errors from evcxr surface in the rendered Typst document. Cover: (1) compile error in a single snippet — what does the rendered box look like? (2) error in snippet A that surfaces only when snippet B uses item X (declared in A) — how do we attribute the error and where do we point? (3) panic at runtime — output partially captured? (4) `:dep` resolution failure — pre-snippet error, attached where? (5) snippet times out (do we even have a timeout?). Decide on the sidecar shape for errors and how the package displays them. Compare error rendering to `ariadne` (what evcxr's REPL uses) and decide if we mimic it or do something Typst-native.
- **Output:** `docs/design/errors.md`.
- **Done when:** the file exists; covers all five cases above; defines a concrete sidecar JSON schema for errors; sketches the rendered Typst output (markdown is fine, doesn't need to be a real `.typ` mock).

---

## Phase 1 — implementation

(These are placeholders; expand once Phase 0 is done.)

### T-I01 · Bootstrap `crates/evcxr-typst/` skeleton

- **Status:** done · `crates/evcxr-typst/{Cargo.toml,src/main.rs,CLAUDE.md}`; root workspace `Cargo.toml`
- **Phase:** 1
- **Depends on:** T-D03, T-D04
- **Done when:** crate compiles, has clap CLI shell, calls `evcxr::runtime_hook()` first thing in `main`.
- **Resolution:** Skeleton committed. Clap subcommands `run`/`watch`/`clean` parse and exit 2 with "not yet implemented" — real bodies land per T-I03..T-I07. evcxr is a path dep (D-006). `runtime_hook()` is the first call in `main` per the evcxr re-entry contract.

### T-I02 · Bootstrap `packages/evcxr/` skeleton

- **Status:** done · `packages/evcxr/{typst.toml,lib.typ,fallback.typ,CLAUDE.md}`
- **Phase:** 1
- **Depends on:** T-D03
- **Done when:** package has `typst.toml`, `lib.typ` with stub `rust()` function emitting metadata, `fallback.typ` returning placeholder.
- **Resolution:** All seven public functions per D-012/D-013/D-015/D-017/D-019 stubbed. Each emits the resolved metadata schema and renders the `fallback.placeholder()` box. No sidecar reading yet — that lands in T-I03.

### T-L01 · Library API: split `lib.rs` + `main.rs` and expose the public surface

- **Status:** done · `crates/evcxr-typst/{Cargo.toml,src/lib.rs,src/main.rs,src/cli.rs,examples/library_use.rs}`
- **Phase:** 1 (precedes T-I03)
- **Depends on:** T-I01 done
- **Reference reads:** `docs/design/library-api.md` (full file); `docs/DECISIONS.md` D-023 (the decision); `.evcxr/evcxr/examples/example_eval.rs` (the precedent)
- **Briefing:** Refactor `crates/evcxr-typst/` to expose a public library API per D-023. Concrete steps:
  - Create `lib.rs` exporting `Project`, `EvalOptions`, `WatchOptions`, `WatchHandle`, `EvaluationReport`, `Snippet`, `SnippetOutcome`, `SnippetResult`, `EvalCallbacks` trait, `Error` (a `thiserror`-derived enum), and any supporting types per `docs/design/library-api.md` § "Public API surface".
  - Move clap parsing into a `cli` module under `main.rs` (or a dedicated `bin/` if cleaner). Library is clap-free.
  - Stub the methods returning `Err(Error::NotImplemented)`; the real bodies land in T-I03 onward. Each stub has the right signature and doc comment.
  - Add `crates/evcxr-typst/examples/library_use.rs` mirroring evcxr's `example_eval.rs` shape — calls `evcxr::runtime_hook()` first, then opens a project, then prints the snippet list. Will compile but `Project::evaluate()` returns `NotImplemented` until T-I03.
  - Add `thiserror = "1"` to `[dependencies]`.
- **Done when:** `cargo build -p evcxr-typst --all-targets` is clean (binary, library, and library_use example all build); `cargo doc -p evcxr-typst` produces docs without missing-doc warnings on public items; the `library_use` example compiles and runs (errors out cleanly with NotImplemented when `Project::evaluate` is called).
- **Resolution:** Public surface landed per D-023 / `docs/design/library-api.md`: `Project` (open / open_with_config / entry / snippets / evaluate / watch / clean_view), `EvalOptions` (deny / allow_eval / with_snippet_timeout / with_callbacks / with_env_passthrough / is_allowed), `EvalCallbacks` trait (5 lifecycle hooks), `WatchOptions`, `WatchHandle::join`, `EvaluationReport`, `Snippet` + `SnippetKind` (8 variants incl. `RustMain` for D-024), `SnippetResult`, `SnippetOutcome` (7 variants), `ResolvedDep`, `DepError`, `ProjectConfig`, and a `thiserror`-derived `Error { NotImplemented(&'static str), Io(#[from] io::Error) }`. All bodies are `Err(Error::NotImplemented(<method>))` stubs. `main.rs` is now a 9-line wrapper that calls `evcxr::runtime_hook()` then `cli::run()`; clap lives in `src/cli.rs`, the binary's only module — library is clap-free. Verified: `cargo build -p evcxr-typst --all-targets` clean, `cargo clippy --all-targets -D warnings` clean, `cargo doc --no-deps` clean (no missing-docs warnings), `cargo fmt --check` clean. `cargo run -p evcxr-typst --example library_use -- /tmp/dummy.typ` exits 1 with `Error: not yet implemented: Project::evaluate`. `cargo run -p evcxr-typst -- run /tmp/dummy.typ` does the same via the binary path.

### T-I03 · `evcxr-typst run` end-to-end smoke

- **Status:** done · `crates/evcxr-typst/src/{lib.rs,cli.rs,discovery.rs,eval.rs,identity.rs,main.rs}`, `crates/evcxr-typst/examples/library_use.rs`, `packages/evcxr/lib.typ`, `examples/hello/main.typ`
- **Phase:** 1
- **Depends on:** T-I01, T-I02, **T-L01** (real bodies populate the library API stubs)
- **Reference reads:** `docs/design/library-api.md` (the API to fill in); existing Phase 0 design docs (architecture, package-api, multi-file)
- **Done when:** matches PLAN.md Phase 1 "Done when". Plus: every code path runs through the library API; `main.rs` is a thin caller; the `library_use` example produces equivalent output to `evcxr-typst run`.
- **Resolution:** `Project::open` shells out to `typst query --field value <entry> '<evcxr-snippet>'`, parses the JSON, and resolves snippet IDs per D-007 (blake3 → base32-lower → 12 chars, with occurrence-index suffix on collision). `Project::evaluate` drives one `evcxr::CommandContext` through every evaluable snippet sequentially, captures stdout via two forwarder threads bridging evcxr's `crossbeam_channel::Receiver`s into private `std::sync::mpsc` channels (necessary because `EvalContext::try_run_statements` busy-waits for `stdout_sender.is_empty()` before returning — without a continuous drainer, the *first* snippet that prints anything deadlocks `execute()`), and writes one `<id>.txt` sidecar per `Ok` snippet at `<entry-parent>/.evcxr-typst-cache/`. Phase 1 limitation: the package's `rust(...)` only renders captured output when an explicit `id:` is pinned, gated on `--input evcxr-mode=read --input evcxr-cache=<typst-abs-path>` (both set by the CLI; bare `typst compile` falls through to the placeholder so D-004 still holds). The CLI's `Run` subcommand calls `Project::evaluate` then shells out to `typst compile` with those inputs and `--root` propagated; `examples/library_use.rs` mirrors the same flow through the library API. Three-snippet hello example (`examples/hello/main.typ`) round-trips: snippet 1 binds `let answer = 6 * 7`, snippet 2 prints `twice the answer = 84` (proving cross-snippet state via evcxr's `committed_state`), snippet 3 defines and calls `fn shout`. `cargo run -p evcxr-typst -- run examples/hello/main.typ --allow-eval --root .` and `cargo run -p evcxr-typst --example library_use -- examples/hello/main.typ .` both produce identical sidecars plus two render artifacts next to the entry file: `main.pdf` (the user-facing output) and `main.svg` (for visual quick-look in a browser without a PDF viewer — note Typst's SVG embeds glyphs as `<path>` references, not `<text>`, so the SVG isn't text-grep-able for snippet output; that lives in the `.txt` sidecars). Tracing/log control via `EVCXR_TYPST_LOG=evcxr_typst=debug` (falls back to `RUST_LOG`). Quality gates: `cargo build --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps`, and `cargo test --lib` all clean.

### T-I04 · MIME passthrough

- **Status:** done · `phase2/mime-passthrough` branch · `crates/evcxr-typst/src/{lib.rs,discovery.rs,eval.rs}`, `packages/evcxr/lib.typ`, `examples/image/main.typ`
- **Phase:** 2
- **Depends on:** T-I03 (done)
- **Resolution:** `eval.rs::write_mime_sidecars` captures `EvalOutputs.content_by_mime_type` after each `execute()` call, decodes binary payloads (base64 via the `base64 = "0.13"` dep matching `evcxr_runtime`'s pinned version), and writes per-MIME extension sidecars (`<id>.png`, `<id>.cbor`, `<id>.html`, `<id>.svg`, `<id>.json`, etc.). For unknown MIMEs, the subtype is used as the extension and a `<id>.meta.json` companion is written. Every successfully evaluated snippet also receives a `<id>.manifest.json` (schema `{"v":1,"extensions":[...]}`) listing available extensions; this manifest is always written even when the extensions list is empty, giving `lib.typ` a guaranteed read-path that never triggers a Typst hard error on a missing file (D-004 preserved). Policy: explicit `text/plain` MIME wins over forwarded stdout for the `.txt` sidecar; when both are present the MIME payload is used. `:dep` snippets (kind `Dep`) are handled in the eval loop before `is_evaluable`: the `:dep` directive string is built from `SnippetOptions::Dep` fields and sent via `CommandContext::execute`, which routes it through evcxr's `:dep` command parser. Discovery was extended to run a second `typst query` for `<evcxr-dep>` markers; both queries return results interleaved by the new shared `_order` counter (a Typst `counter("evcxr-doc-order")`) emitted by every `_emit-snippet` and `dep()` call in `lib.typ`. The counter gives a stable unified `doc_order` across the two separate query results without needing position metadata from Typst. `lib.typ` wires `rust-display` to read the manifest and serve the highest-priority available extension (PNG > SVG > JPG > HTML, overrideable via `prefer:`); `rust-html` renders `<id>.html` as `raw(..., lang: "html")`; `rust-data` emits the metadata marker (no visible output) and a companion `rust-data-read(id:, format:, fallback:)` function reads the CBOR/JSON sidecar and returns the parsed Typst value — the two-function split is required because Typst's code-block semantics cannot return both metadata content AND a non-content dict value from one function call (confirmed via empirical testing). Smoke test `examples/image/main.typ` exercises: `evcxr_runtime` + `image` + `ciborium` deps, a 64×64 gradient PNG plot via `rust-display`, and a CBOR-roundtripped `{"mean":42.0,"n":7}` dict consumed via `rust-data-read`. After `evcxr-typst run --allow-eval --root . examples/image/main.typ`, `img-plot.png` (valid PNG, `\x89PNG` magic) and `cbor-stats.cbor` (12 bytes, parses as the expected dict) are confirmed. All quality gates clean: `cargo build --all-targets`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps`, `cargo test -- --test-threads 1`. Deviation from briefing: `evcxr_image` path-dep cannot be referenced via `:dep` from crates.io, so bare `image` crate is used directly; `evcxr_runtime` must be added as an explicit `:dep` (it is not auto-injected by evcxr).

### T-I05 · `evcxr-typst watch` + caching

- **Status:** done · phase3/watch-and-cache
- **Phase:** 3
- **Depends on:** T-I04, T-D05
- **Resolution:** Implemented `cache.rs` (Blake3 CAS, Merkle chain, hardlink/copy materialisation, GC, index read/write, skip-if-unchanged per D-016) and `watch.rs` (notify v8 + crossbeam-channel 150 ms trailing-edge debounce, `classify` → `Plan` enum, `is_leaf` via syn parse, `run_one_cycle` for append/truncate/leaf/reset strategies, Backoff). `eval.rs` extended with cache hit/miss tracking and CAS store-after-eval. `lib.rs` wires `Project::watch` → `watch::run`, `Project::gc`. CLI gains `--gc` to `clean`. All quality gates clean: clippy -D warnings, fmt, doc, tests (22 pass).

### T-I06 · Fallback safety + `--allow-eval`

- **Status:** open
- **Phase:** 4
- **Depends on:** T-I02, T-I05

### T-I07 · Pretty error rendering

- **Status:** done · `phase4/pretty-errors` branch
- **Phase:** 4
- **Depends on:** T-D06 (done), T-I04 (done)
- **Resolution:** Five error phases (compile, runtime-panic, dep-resolution, timeout, internal) captured in `crates/evcxr-typst/src/error_capture.rs`: `ErrorSidecar` / `ErrorEntry` / `SpanRef` / `OffsetMap` (D-014 cross-snippet attribution) types, `classify_*` constructors, and `write_error_sidecar` which writes `<id>.error.json` and overwrites the manifest with `extensions: ["error"]` (or `["error","txt"]` for panic with partial stdout). `eval.rs` wires a watchdog thread (AtomicBool + `process_handle().lock().unwrap().kill()`) before each `context.execute` for timeouts (D-017 per-snippet wins over global; D-009 30s default). After each Ok result `OffsetMap::record_submission` diffs `defined_item_names()` to track item→snippet provenance for cross-snippet error attribution. `lib.rs` exposes `Snippet::timeout_ms`. `discovery.rs` parses `parse_timeout_ms` (accepts `"30s"`, `"5min"`, `"1000ms"`, integer). `cli.rs` counts non-Ok outcomes and `bail!`s non-zero. `packages/evcxr/error.typ` added: `evcxr-error-box` (header bar colored by severity, source excerpt with caret underline, help messages, cross-snippet footer, evcxr_hint; schema v>1 renders minimal fallback per D-019) and `evcxr-error-note` (cross-snippet stub box). `packages/evcxr/lib.typ` imports `error.typ`, adds `_check-error(id)` (reads manifest, returns parsed error JSON or none), and gates every `_read-*` helper behind it — error box takes precedence over normal output; `rust-data` also emits an error box inline since `rust-data-read` returns a value not content (D-015). Smoke test at `examples/errors/main.typ` exercises compile error, panic, and dep failure. Known limitation: caret underline uses `h(col * 0.6em)` approximation (documented in error.typ comment).

---

## House-keeping

### T-H01 · License files

- **Status:** done · LICENSE-MIT, LICENSE-APACHE at repo root, README.md updated.
- **Phase:** any
- **Depends on:** —
- **Briefing:** Add `LICENSE-MIT` and `LICENSE-APACHE` matching evcxr's text. Reference them in `Cargo.toml` (`license = "MIT OR Apache-2.0"`) and in source-file headers when we start adding code.
- **Done when:** both files exist, copies match evcxr verbatim (modulo copyright year), `README.md` license section updated.

### T-H02 · `rustfmt.toml`

- **Status:** done · `rustfmt.toml` at repo root, mirrors evcxr verbatim (edition 2024, `use_field_init_shorthand`).
- **Phase:** any
- **Depends on:** T-I01
- **Briefing:** Mirror evcxr's `rustfmt.toml` so we stay style-aligned with upstream.

### T-H03 · Rename `show:` kwarg in package API (Typst reserved word)

- **Status:** done · D-021 · `packages/evcxr/lib.typ` (kwarg renamed `show:` → `render:`, `default-show:` → `default-render:`); `docs/design/{package-api,examples/index}.md` (full rename sweep); `docs/DECISIONS.md` (D-021 added)
- **Phase:** any
- **Depends on:** —
- **Briefing:** `packages/evcxr/lib.typ` currently declares `#let rust(src, id: none, deps: (), show: auto, ...)`. Typst rejects this with "keyword `show` is not allowed as an identifier" — `show` is reserved (the rule selector). Discovered while smoke-testing `typst compile --root . examples/hello/main.typ` during T-H01/T-H04 cleanup. Need to (1) pick a non-reserved name, candidates: `display`, `output`, `mode`, `show_` (Typst's own suggestion); (2) update D-012 in `docs/DECISIONS.md` and `docs/design/package-api.md` to record the rename; (3) update `examples/hello/main.typ` and the `setup(default-show: ...)` kwarg accordingly (likely also reserved-adjacent but currently parses). Not blocking on the CLI side but blocks any actual `typst compile` of the hello example.
- **Done when:** `typst compile --root . examples/hello/main.typ` parses past the `lib.typ` import; design docs updated; decision recorded.
- **Resolution:** Renamed `show:` → `render:` (and `default-show:` → `default-render:` for symmetry). `render` won over `display`/`output`/`mode`/`show_` because it has no Typst reserved-keyword collision, no overlap with the `rust-display()` function name, no overlap with the kwarg's `"output"` value, and reads naturally with every value choice (`render: "both"`, `render: auto`). Metadata schema field correspondingly renamed `<evcxr-snippet>.options.show` → `.options.render`; not a versioned-schema bump because schema is pre-1.0 (D-019). D-021 amends D-012.

---

## Side tracks

> Off main critical path. See `docs/tracks/README.md` for the concept and `docs/tracks/semantic-typst.md` for the only current track. **Side-track tasks are interleaved with main work, never blocking.** If a main task and a side-track task are both open, the main task wins.

### T-S00 · Semantic Typst — track meta-doc

- **Status:** done · `docs/tracks/{README.md,semantic-typst.md}` · D-020
- **Track:** Semantic Typst
- **Depends on:** —
- **Resolution:** Track designed end-to-end; vision, target UX, three architecture options (CLI-sidecars / WASM-plugin / both), phased plan S1..S4, sidecar schema sketch, scope-explicit non-goals. D-020 records the "CLI sidecars first" policy. Scaffolding for picking up the work: implementation tasks T-S01..T-S04 below.

### T-S01 · CLI semantic sidecars: `type-of`, `signature-of`, `kind-of`

- **Status:** open
- **Track:** Semantic Typst
- **Depends on:** main-plan **T-I03** (sidecar plumbing must exist) · D-020
- **Reference reads:** `docs/tracks/semantic-typst.md` (whole file), `.evcxr/evcxr/src/rust_analyzer.rs` (the source of the data), `.evcxr/evcxr/src/eval_context.rs` (search for `analyzer.` to see how it's accessed today)
- **Briefing:** After each snippet the CLI evaluates, query the embedded `RustAnalyzer` for declared items and committed bindings; serialize to CBOR and write `<id>.semantic.cbor` per the schema sketched in `tracks/semantic-typst.md`. Add Typst-package functions `type-of(name)`, `signature-of(name)`, `kind-of(name)` reading the sidecar; `none`/placeholder fallback when missing (D-015 model). The `<id>.semantic.cbor` file becomes the project's fifth versioned interface — register it in `docs/design/schema-versioning.md` § "tracked interfaces" and start it at `v: 1`.
- **Done when:** the gallery `b-struct-across-snippets.typ` example renders inline `type-of` / `signature-of` references that resolve to actual rust-analyzer-emitted strings after `evcxr-typst run --allow-eval`. Bare `typst compile` of the same doc still succeeds, with placeholder boxes where references would resolve.

### T-S02 · Semantic sidecars: `doc-of`, `items-table`, `ref`

- **Status:** blocked
- **Track:** Semantic Typst
- **Depends on:** T-S01
- **Reference reads:** T-S01's outputs; `docs/tracks/semantic-typst.md` § "Risks" (rustdoc → Typst conversion footgun)
- **Briefing:** Extend the sidecar schema to carry rustdoc comments and item spans. Add `doc-of(name)` (best-effort markdown-ish content), `items-table(at: id, only-kinds: ...)` (a styled table of items in scope at the named snippet), `ref(name)` (an inline hyperlink to the snippet that defined `name` — needs snippets to emit Typst `<label>`s; T-S02 may need a small package-side mechanism for that). Schema version bumped if and only if this addition breaks T-S01 readers (it's purely additive, so no bump per D-019).
- **Done when:** a 200-word "narrative doc with semantic prose" example exists at `examples/semantic-narrative/main.typ` and renders with all three new functions populated.

### T-S03 · CLI-side rust-analyzer diagnostics sidecar

- **Status:** blocked
- **Track:** Semantic Typst
- **Depends on:** T-S01 · main-plan **T-I07** (pretty errors should land first so the rendering style is consistent)
- **Briefing:** Run `RustAnalyzer::diagnostics` per snippet alongside `execute`; serialize to a separate `<id>.diagnostics.cbor` (or extend the semantic sidecar — TBD; document the choice when implementing). Add `evcxr.diagnostics-of(snippet-id)` to the package, rendering using the same styling as T-I07's compile-error box. This complements but does not replace rustc errors: rust-analyzer surfaces some issues earlier, rustc surfaces others (especially borrowck and codegen edge cases) only at compile time; both can coexist for a single snippet, ranked together.
- **Done when:** snippet `g-error-case.typ` shows both an analyzer-level diagnostic and the rustc compile error in a stable order; the doc explains the difference in a one-line note above the box.

### T-S04-spike · WASM build mechanism — empirical proof of concept

- **Status:** open
- **Track:** Semantic Typst
- **Depends on:** — (independent of main path; can run any time)
- **Reference reads:**
  - `docs/design/wasm-plugin-analyzer.md` § "Mechanism: how we'd actually depend on the fork" (full story: patch-leakage, isolated `[workspace]`, pinned `rev`, `0.0.x` semver fragility)
  - `.typst-wasm-minimal-protocol/examples/hello_rust/Cargo.toml` (the canonical isolation pattern: bottom `[workspace]` block plus WASM-tuned `[profile.release]`)
  - `cgmossa/rust-analyzer` branch `wasm`, commit `8a79b99` — verify it exists, capture the actual full commit SHA for pinning
- **Briefing:** Hard time-box: **one engineering day**. Smallest possible WASM build that exercises the actual mechanism we'd use for T-S04:
  - Create `crates/evcxr-typst-analyzer/` with its own `Cargo.toml` ending in a `[workspace]` block (so the parent `evcxr-typst` workspace doesn't see this crate).
  - One `[patch.crates-io]` entry pointing at the fork, pinned by `rev =` (not `branch =`), for **just `ra_ap_syntax`** — nothing else. Parse-only, cheapest case.
  - Cdylib using `wasm-minimal-protocol`: one `#[wasm_func] fn parse(src_cbor: &[u8]) -> Vec<u8>` that decodes a snippet string from CBOR, runs it through `ra_ap_syntax`, returns parse errors as CBOR.
  - `[profile.release]` tuned for size per the upstream example. Build for `wasm32-unknown-unknown`. Run wasm-opt + wasi-stub if needed.
  - Smallest possible Typst doc that loads the plugin via `plugin("./analyzer.wasm")` and round-trips a sample snippet. Verify it actually works under `typst compile`.
- **Done when** (any resolves the gate):
  - **(a) Works clean.** Output: the `.wasm` artifact (committed for reference, gitignored from regular builds), a measurement of blob size (uncompressed and brotli-compressed), and a one-page `docs/design/wasm-spike-results.md` with what broke / what worked / what surprised. T-S04 unblocks for an explicit ship decision.
  - **(b) Hard blocker hit and not resolvable inside the time box.** E.g. the fork's commit doesn't compile against the published `ra_ap_syntax` API; `wasm-minimal-protocol` and `ra_ap_*` interact badly; blob size > 50 MB; missing wasi-stub coverage we can't quickly fill. Output: same one-page write-up describing exactly what blocks. T-S04 → `won't-do`.
  - **(c) Time-box expires inconclusively.** Same write-up, T-S04 stays `blocked-on-decision`, write-up captures what the next attempt would need.
- **Output (always):** `docs/design/wasm-spike-results.md` regardless of outcome.

### T-S04 · WASM analyzer plugin (the bigger one)

- **Status:** blocked — waiting on T-S04-spike outcome
- **Track:** Semantic Typst
- **Depends on:** T-S04-spike (must succeed, **(a)** outcome only) · T-S01..T-S03 shipped (to validate the user-facing surface) · main-plan **T-I06** shipped (to keep the plugin work parallel to a safe, shipping CLI baseline)
- **Reference reads:** `docs/design/wasm-plugin-analyzer.md` (full analysis, including § "Mechanism"), `docs/tracks/semantic-typst.md` § "Architecture options", and the spike's `wasm-spike-results.md`.
- **Briefing:** Build `crates/evcxr-typst-analyzer/` (already partially shaped by the spike) into a full Typst plugin cdylib, using the full `ra_ap_*` set patched per the fork. Bundle a precomputed stdlib summary. Define an items-summary input schema (a fifth versioned interface — register in `docs/design/schema-versioning.md` per D-019). Wire `packages/evcxr/lib.typ` to prefer the plugin when present and fall back to the CLI-sidecar path from T-S01..T-S03. Concrete sub-tasks (build pipeline including fork-rebase cadence playbook, stdlib bundle generation, plugin API, items-summary schema, package wiring, integration tests) get expanded once the spike returns success.
- **Done when:** explicit decision to ship → expanded into a sub-backlog of concrete implementation steps. Or explicit decision to drop → closed as `won't-do` and the `wasm-plugin-analyzer.md` doc retained as research-only.

---

### T-B00 · Rust-by-example port — track meta-doc

- **Status:** done · D-022 · `docs/tracks/rust-by-example-port.md`
- **Track:** Rust-by-example port
- **Depends on:** —
- **Resolution:** Track designed end-to-end: vision, mapping rules (md → typ, code-block tag → evcxr function), `fn main()` problem and `rust-main` resolution, deterministic porter design (`tools/rbe-port/`, `pulldown-cmark` + `syn`), license/attribution policy, B1..B6 phasing, drift detection via manifest, and 5 explicit open questions. D-022 records the policy.

### T-B01 · `tools/rbe-port/` skeleton + `rust-main` package addition

- **Status:** open
- **Track:** Rust-by-example port
- **Depends on:** main-plan **T-I02** shipped (the `evcxr` package must exist to extend it with `rust-main`)
- **Reference reads:**
  - `docs/design/rbe-porter.md` (the full implementation spec — the load-bearing read; covers crate structure, dep choices, the scanner state machine, snippet detection corner cases, code-block tag matrix, cross-link resolution, determinism rules, manifest format, and the 10 required golden cases)
  - `docs/tracks/rust-by-example-port.md` (parent track plan)
  - `docs/DECISIONS.md` D-022 (track scope), D-024 (porter mechanism choices, including the no-pulldown-cmark rationale), D-019 (additive `options` keys)
  - `docs/design/package-api.md` (the function set `rust-main` joins)
  - `.rust-by-example/src/hello.md` and `custom_types/structs.md` (the canonical inputs)
- **Briefing:** Two pieces in one task:
  1. **`tools/rbe-port/`**: new workspace member, Rust binary. clap CLI: `rbe-port <input-dir> <output-dir> [--phase B1|B2|…]`. Markdown parser via `pulldown-cmark`; Rust snippet detection via `syn` (specifically: detect `fn main() { … }` to know whether to emit `rust-main` vs `rust`); `summary.rs` parses upstream `SUMMARY.md` into a tree. Output: per-chapter `.typ` files mirroring the input directory structure, plus `manifest.json` capturing input commit SHA and per-file SHA-256. Determinism: same input bytes → byte-identical output. Golden tests under `tools/rbe-port/tests/golden/` for at least three inputs covering plain `rust`, `rust,editable`, and `rust,ignore`.
  2. **`rust-main` package addition**: extend `packages/evcxr/lib.typ` with `rust-main(src, ..)` mirroring `rust(...)`'s kwargs. Metadata emitted is `kind: "rust-main"` (a new `kind` variant — additive per D-019, no `v` bump) with `options.auto-call: "main"`. The CLI side (T-I03 onward) will know to evaluate the snippet and then synthesise a `main();` invocation; a stub of that contract goes into `docs/design/package-api.md` § 2 as the new `2.9` subsection.
- **Done when:** `tools/rbe-port/` builds clean and converts `.rust-by-example/src/hello.md` to a `.typ` file that uses `rust-main` and renders correctly under `typst compile --root . examples/rust-by-example/hello.typ`. Golden tests pass. `lib.typ` exports `rust-main`. `package-api.md` § 2.9 added.

### T-B02 · Phase B1 — port Hello / Primitives / Custom Types (~15 chapters)

- **Status:** blocked
- **Track:** Rust-by-example port
- **Depends on:** T-B01 shipped · main-plan **T-I03** shipped (so we can actually run-and-render to validate end-to-end)
- **Briefing:** Run `rbe-port --phase B1` against `.rust-by-example/`. Hand-review every output file once. Where the porter mis-translates, fix the porter (not the output) and re-run; the output is meant to be regenerable. Add `examples/rust-by-example/NOTICES.md` per D-022. Add the top-level `examples/rust-by-example/main.typ` `#include`-ing the chapter files in `SUMMARY.md` order. Verify each chapter renders under `evcxr-typst run --allow-eval` and the captured stdout matches the upstream's expected output (rust-by-example chapters have a known-good expected output for each example).
- **Done when:** all 15 B1 chapters render evaluated; `main.typ` builds top to bottom; `NOTICES.md` carries the upstream commit SHA; the regenerate-from-porter cycle is clean.

### T-B03 · Phase B2 — variable_bindings / types / conversion / expression / flow_control (~30 chapters)

- **Status:** blocked
- **Track:** Rust-by-example port
- **Depends on:** T-B02
- **Briefing:** Same shape as T-B02, larger. Validates `let`/scope/shadowing edge cases against snippet-semantics § "Variable-reference limitation" — expect at least one chapter to expose a workaround we need to document.
- **Done when:** B2 chapters render; any variable-reference workarounds documented in a per-chapter footnote.

### T-B04 · Phase B3 — fn / mod / crates / cargo (~15 chapters)

- **Status:** blocked
- **Track:** Rust-by-example port
- **Depends on:** T-B03
- **Briefing:** The modules chapter is the cross-snippet acid test (D-008: file-based `mod foo;` rejected — but rust-by-example uses inline `mod` consistently, so this should compose cleanly). The `crates/cargo` chapters demonstrate `:dep` integration; expect this to surface any `dep()` ergonomics issues.
- **Done when:** B3 chapters render; cross-snippet item composition holds across chapter boundaries.

### T-B05 · Phase B4 — attribute / generics / scope / trait (~25 chapters)

- **Status:** blocked
- **Track:** Rust-by-example port
- **Depends on:** T-B04
- **Briefing:** Generic and trait composition. Lifetime restrictions in the scope chapter will exercise persistence rules; document any chapter that needs the `Box::leak`-style workaround from snippet-semantics.

### T-B06 · Phase B5 + B6 — error + std/std_misc/testing/unsafe/compatibility/meta (~55 chapters)

- **Status:** blocked
- **Track:** Rust-by-example port
- **Depends on:** T-B05 · main-plan **T-I04** (MIME passthrough — the formatting chapter wants `Display` output; std chapters may want images) · main-plan **T-I07** (pretty errors — the error chapter is built around demonstrating compile failures gracefully)
- **Briefing:** Largest chunk; split into B5 and B6 sub-PRs if convenient. Tests T-I07 against the `compile_fail` snippets in `error/*` (the document must keep rendering past a failed snippet). Tests `:dep` cache absorption in `std/*` (the cache should make repeat `evcxr-typst run` cheap).
- **Done when:** B5 + B6 chapters render; full `examples/rust-by-example/main.typ` book builds end to end; any timeouts logged for follow-up.

---

## Done

(Tasks above marked `done` retain their full briefing for posterity. Future "done" entries should keep the same shape: status line cites commit + output paths + any decision-record updates.)
