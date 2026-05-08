# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

Integration project gluing [evcxr](https://github.com/evcxr/evcxr) (Rust eval-context library) to [Typst](https://typst.app/). Goal: Rust snippets in a `.typ` document get evaluated and their outputs embedded. See `README.md` for the elevator pitch and `docs/ARCHITECTURE.md` for the design.

## Working with the evcxr source

evcxr is a **dependency**, not part of this repo. A local clone is kept inside the repo at `.evcxr/` (gitignored) and is treated as **read-only**. The `origin` remote on that clone is `CGMossa/evcxr` (the user's fork — see D-025); `upstream` is `evcxr/evcxr`. Both are checked out at `main` by default.

If `.evcxr/` (or the sibling `.prequery/` / `.typst-wasm-minimal-protocol/` reference checkouts) is missing on a fresh setup, run `just clone-refs` from the repo root.

- When you need to look at evcxr internals, read files under `.evcxr/`. The path-dep in `crates/evcxr-typst/Cargo.toml` resolves there too (`../../.evcxr/evcxr`).
- That clone has its own `CLAUDE.md` summarizing evcxr's architecture — read it once before doing anything that touches the evcxr API. The big-picture map: `CommandContext` → `EvalContext` → `Module` → `ChildProcess`, plus the `runtime_hook()` re-entry trick (any binary that depends on evcxr must call it on startup) and the `EVCXR_BEGIN_CONTENT <mime>` MIME output protocol.
- Do **not** copy evcxr code into this repo. During dev we depend on it via `path = "../../.evcxr/evcxr"` (from `crates/evcxr-typst/Cargo.toml`); once we settle on a published baseline we switch to `evcxr = "<version>"` from crates.io. See `docs/DECISIONS.md` D-006.
- If something in evcxr's public API needs to change for our use case, propose a patch upstream rather than working around it here.

### The `CGMossa/evcxr` fork (D-025)

There is a personal fork at <https://github.com/CGMossa/evcxr> with nine feature branches / PRs queued against the fork's own `main` (PRs #1–#9 — see `docs/DECISIONS.md` D-025 for the full table and rationale). Read that table before assuming a referenced evcxr feature is in upstream. Two are likely load-bearing for `evcxr-typst` features later: #8 (compiler warnings surfaced in `EvalOutputs.warnings`) and #5 (`:patch` for `[patch.crates-io]` deps).

The fork is a **staging ground**, not a substitute upstream. The in-repo `.evcxr/` checkout is still on `main` (read-only). Only switch the path-dep to a fork branch when a specific `evcxr-typst` task genuinely needs that change — and document the dependency switch in the task's commit. When a fork PR upstreams, amend D-025 to drop that row.

If you propose a *new* evcxr-side change while working on `evcxr-typst`, the workflow is: open a PR against `CGMossa/evcxr` `main` (not `evcxr/evcxr`) — the user has READ-only on upstream, and "just go" / "don't stop" is **not** authorization to publish to upstream. Add a row to D-025 for the new PR.

## Where to start a session

1. Read `docs/BACKLOG.md` and pick the top **open** task whose dependencies are met.
2. Read the "Reference reads" listed in that task before touching anything.
3. Each task has a "Done when" checklist — match it, don't expand scope.
4. When you finish a task: edit `docs/BACKLOG.md` to mark it `done` with a one-line summary and a link to the commit/PR. If the work shifted the design, log it in `docs/DECISIONS.md`.

## Repo layout

- `docs/` — plans, decisions, design specs. Source of truth.
- `crates/evcxr-typst/` — the Rust CLI. Has its own `CLAUDE.md`.
- `packages/evcxr/` — the Typst package. Has its own `CLAUDE.md`.
- `examples/` — end-to-end documents that exercise the integration. Has its own `CLAUDE.md`.

Each scaffolding directory has a local `CLAUDE.md` with directory-specific conventions and required reading. Read the local one before editing anything in that directory.

## Conventions

- Match evcxr's conventions where they apply: `cargo fmt --check` clean, dual MIT/Apache-2.0 license headers on new source files, `rustfmt.toml` should mirror evcxr's once we add one.
- Keep `docs/` source-of-truth current: PLAN, ARCHITECTURE, BACKLOG, DECISIONS. If you make a non-trivial design choice mid-task, append a decision record rather than burying it in a commit message.
- Default to no comments in code; one short line max when WHY is non-obvious. (Same rule the global `CLAUDE.md` already enforces.)
- This repo's git history is fresh — feel free to make small commits. Do **not** push to a remote without explicit instruction; no remote is configured by default.
- **`evcxr-typst run` renders both PDF and SVG** next to the entry file (`<stem>.pdf` and `<stem>.svg`). PDF is the user-facing artifact; SVG is for visual quick-look in a browser without a PDF viewer. Note that Typst SVG embeds glyphs as `<path>` references rather than `<text>` elements, so the SVG is **not** text-grep-able for snippet output — when an agent (or a script) needs to verify *what was evaluated*, read the textual sidecars at `<entry-parent>/.evcxr-typst-cache/<id>.txt` instead. The two together cover the dev loop: SVG for "did it lay out", sidecars for "did it evaluate". Multi-page documents will need `typst compile` invoked directly with a `{p}` template, since the single-file SVG path only works when the document fits one page.

## What NOT to do without checking first

- Don't change the architectural shape (prequery vs. WASM plugin vs. embed-typst-as-lib) without writing a decision record and getting confirmation. The current direction is justified in `docs/DECISIONS.md` D-001.
- Don't add a "make typst sandboxing optional in a way that runs Rust by default on bare `typst compile`" path. Fallback rendering must be the default; arbitrary code execution must be opt-in via `--allow-eval` (D-004).
- Don't pull in a new heavy dep (a typst fork, a custom rust-analyzer, etc.) without a decision record.
