# Backlog

Agent-ready task queue. Pick the top **open** task whose dependencies are all `done`.

For each task: read the **Reference reads** before starting, satisfy the **Done when** checklist, then mark the task `done` here with a one-line summary and a commit/PR link.

Status legend: `open` · `in-progress` · `done` · `blocked` · `superseded`

---

## Phase 0 — design

> All six T-D0x tasks landed in commit 954e3a2 as parallel agent drafts and were reconciled into ARCHITECTURE.md / DECISIONS.md in a follow-up commit. They appear in **Done** at the bottom. The follow-up reconciliation tasks T-D07–T-D10 below cover the open questions that surfaced.

### T-D07 · Reconcile open questions left by Phase-0 drafts

- **Status:** open
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

- **Status:** open
- **Phase:** 0 (design follow-up)
- **Depends on:** T-D07
- **Reference reads:** `docs/DECISIONS.md` D-009; `docs/design/errors.md` § 1.e; `docs/design/package-api.md` § 6
- **Briefing:** D-009 deferred per-snippet timeout overrides because evcxr's child-cancellation semantics weren't clear. Read `evcxr/src/eval_context.rs` for what `execute` actually does on cancellation; decide whether `rust(..., timeout: 5min)` is shippable in v0 or stays deferred. Either way, document the decision and update the `errors.md` RECON-T-D03 flag.
- **Done when:** decision recorded as a new D-xxx entry; `errors.md` flag resolved; `package-api.md` § 6 updated accordingly.

### T-D09 · Multi-document and multi-file project layout

- **Status:** open
- **Phase:** 0 (design follow-up)
- **Depends on:** T-D07
- **Reference reads:** `docs/design/watch-loop.md` open Q2; `docs/design/cache.md` § "Cache layout on disk" (cache lives at workspace level)
- **Briefing:** A real Typst project rarely lives in one `.typ` file. Designing for `#import "chapter1.typ"` etc.: where does the cache live, how do snippets in `chapter1.typ` reach `dep()`s declared in `main.typ`, what's the watch-set, what's the run command (one `main.typ` is the entry, dependent files are auto-discovered)? Probably v0 supports a single entry file + auto-discovered imports, multi-entry-file projects deferred.
- **Output:** new `docs/design/multi-file.md`.
- **Done when:** the file exists; covers cache scope, watch-set discovery, dep visibility across files, entry-file selection on the CLI.

### T-D10 · Schema versioning policy

- **Status:** open
- **Phase:** 0 (design follow-up)
- **Depends on:** —
- **Reference reads:** ARCHITECTURE.md § "The metadata contract"; package-api.md § 5; errors.md § 2; cache.md § "Cache layout"
- **Briefing:** Three `v` fields exist in the wild: `<evcxr-snippet>.v`, `<evcxr-dep>.v`, `<id>.error.json.v`. Plus a CAS layout `v1/`. Document policy: when do we bump? what's backward-compat strategy? what's the minimum-CLI-version-required mechanism so a Typst package release can refuse an old CLI cleanly?
- **Output:** new `docs/design/schema-versioning.md` (~1 page).
- **Done when:** the file exists; covers all four version fields and the CLI/package compatibility check.

---

### T-D01 · Snippet semantics & dependency model

- **Status:** done · 954e3a2 · `docs/design/snippet-semantics.md`
- **Phase:** 0 (design)
- **Depends on:** —
- **Reference reads:**
  - `/Users/elea/Documents/GitHub/evcxr/COMMON.md` (whole file — variable persistence, `:dep`, references)
  - `/Users/elea/Documents/GitHub/evcxr/evcxr/src/eval_context.rs` (skim `ContextState`, item/var tracking)
  - `/Users/elea/Documents/GitHub/evcxr/evcxr/src/use_trees.rs` (how `use` statements are merged)
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
  - `/Users/elea/Documents/GitHub/evcxr/evcxr_jupyter/samples/evcxr_jupyter_tour.ipynb` (gold standard for "what kinds of things do people do in a Rust notebook")
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
  - `/Users/elea/Documents/GitHub/evcxr/.prequery/README.md` and `.prequery/src/` (look at the package API there; it's a good model)
  - `/Users/elea/Documents/GitHub/evcxr/.typst-wasm-minimal-protocol/examples/hello_rust/` (just for Typst-side ergonomics)
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
  - `/Users/elea/Documents/GitHub/evcxr/evcxr/src/command_context.rs` (what state-reset operations are available — `:clear`, etc.)
- **Briefing:** Detailed algorithm for `evcxr-typst watch`. Pseudocode for the change loop. Address: how do we tell `typst watch` (running as a child) about sidecar updates (mtime should suffice — verify), how do we debounce file events from multiple editors, what happens on transient parse errors in the `.typ` file, how do we shut down cleanly, what's logged where. Concrete rules for the change classification mentioned in ARCHITECTURE.md (added-at-end / removed-at-end / leaf-modified / non-leaf-modified). Define "leaf" precisely — does a snippet that only `println!`s but inside its body declares a `let` count as a leaf? (Answer: yes, because `let` inside a block doesn't escape.)
- **Output:** `docs/design/watch-loop.md` with pseudocode and the classification rules.
- **Done when:** the file exists; pseudocode is specific enough that someone implementing it doesn't have open design questions; classification rules cover the cases listed plus at least three I haven't anticipated.

---

### T-D06 · Error reporting & diagnostic plumbing

- **Status:** done · 954e3a2 · `docs/design/errors.md` · D-009 added (timeout 30s); D-011 added (panic resets state)
- **Phase:** 0 (design)
- **Depends on:** T-D03 (need to know how the package surfaces things)
- **Reference reads:**
  - `/Users/elea/Documents/GitHub/evcxr/evcxr/src/errors.rs` (compilation error structure, spans)
  - `/Users/elea/Documents/GitHub/evcxr/evcxr_repl/src/bin/evcxr.rs` (how the REPL renders errors with `ariadne`)
  - `docs/design/package-api.md` once T-D03 is done
- **Briefing:** Design how compilation/runtime errors from evcxr surface in the rendered Typst document. Cover: (1) compile error in a single snippet — what does the rendered box look like? (2) error in snippet A that surfaces only when snippet B uses item X (declared in A) — how do we attribute the error and where do we point? (3) panic at runtime — output partially captured? (4) `:dep` resolution failure — pre-snippet error, attached where? (5) snippet times out (do we even have a timeout?). Decide on the sidecar shape for errors and how the package displays them. Compare error rendering to `ariadne` (what evcxr's REPL uses) and decide if we mimic it or do something Typst-native.
- **Output:** `docs/design/errors.md`.
- **Done when:** the file exists; covers all five cases above; defines a concrete sidecar JSON schema for errors; sketches the rendered Typst output (markdown is fine, doesn't need to be a real `.typ` mock).

---

## Phase 1 — implementation

(These are placeholders; expand once Phase 0 is done.)

### T-I01 · Bootstrap `crates/evcxr-typst/` skeleton

- **Status:** blocked-on Phase 0
- **Phase:** 1
- **Depends on:** T-D03, T-D04
- **Done when:** crate compiles, has clap CLI shell, calls `evcxr::runtime_hook()` first thing in `main`.

### T-I02 · Bootstrap `packages/evcxr/` skeleton

- **Status:** blocked-on Phase 0
- **Phase:** 1
- **Depends on:** T-D03
- **Done when:** package has `typst.toml`, `lib.typ` with stub `rust()` function emitting metadata, `fallback.typ` returning placeholder.

### T-I03 · `evcxr-typst run` end-to-end smoke

- **Status:** blocked
- **Phase:** 1
- **Depends on:** T-I01, T-I02
- **Done when:** matches PLAN.md Phase 1 "Done when".

### T-I04 · MIME passthrough

- **Status:** blocked
- **Phase:** 2
- **Depends on:** T-I03

### T-I05 · `evcxr-typst watch` + caching

- **Status:** blocked
- **Phase:** 3
- **Depends on:** T-I04, T-D05

### T-I06 · Fallback safety + `--allow-eval`

- **Status:** blocked
- **Phase:** 4
- **Depends on:** T-I02, T-I05

### T-I07 · Pretty error rendering

- **Status:** blocked
- **Phase:** 4
- **Depends on:** T-D06, T-I04

---

## House-keeping

### T-H01 · License files

- **Status:** open
- **Phase:** any
- **Depends on:** —
- **Briefing:** Add `LICENSE-MIT` and `LICENSE-APACHE` matching evcxr's text. Reference them in `Cargo.toml` (`license = "MIT OR Apache-2.0"`) and in source-file headers when we start adding code.
- **Done when:** both files exist, copies match evcxr verbatim (modulo copyright year), `README.md` license section updated.

### T-H02 · `rustfmt.toml`

- **Status:** open
- **Phase:** any
- **Depends on:** T-I01
- **Briefing:** Mirror evcxr's `rustfmt.toml` so we stay style-aligned with upstream.

---

## Done

(Tasks above marked `done` retain their full briefing for posterity. Future "done" entries should keep the same shape: status line cites commit + output paths + any decision-record updates.)
