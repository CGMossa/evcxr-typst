# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

Integration project gluing [evcxr](https://github.com/evcxr/evcxr) (Rust eval-context library) to [Typst](https://typst.app/). Goal: Rust snippets in a `.typ` document get evaluated and their outputs embedded. See `README.md` for the elevator pitch and `docs/ARCHITECTURE.md` for the design.

## Working with the evcxr source

evcxr is a **dependency**, not part of this repo. The local clone lives at `/Users/elea/Documents/GitHub/evcxr` and is treated as **read-only**.

- When you need to look at evcxr internals, read files at that absolute path. Add it as an extra Claude Code working directory if you'll need it for the session: `claude --add-dir /Users/elea/Documents/GitHub/evcxr`.
- That clone has its own `CLAUDE.md` summarizing evcxr's architecture — read it once before doing anything that touches the evcxr API. The big-picture map: `CommandContext` → `EvalContext` → `Module` → `ChildProcess`, plus the `runtime_hook()` re-entry trick (any binary that depends on evcxr must call it on startup) and the `EVCXR_BEGIN_CONTENT <mime>` MIME output protocol.
- Do **not** copy evcxr code into this repo. Depend on it via `path = "../evcxr/evcxr"` for local dev, or `evcxr = "<version>"` from crates.io once we settle on a published baseline (see `docs/DECISIONS.md` D-006).
- If something in evcxr's public API needs to change for our use case, propose a patch upstream rather than working around it here.

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

## What NOT to do without checking first

- Don't change the architectural shape (prequery vs. WASM plugin vs. embed-typst-as-lib) without writing a decision record and getting confirmation. The current direction is justified in `docs/DECISIONS.md` D-001.
- Don't add a "make typst sandboxing optional in a way that runs Rust by default on bare `typst compile`" path. Fallback rendering must be the default; arbitrary code execution must be opt-in via `--allow-eval` (D-004).
- Don't pull in a new heavy dep (a typst fork, a custom rust-analyzer, etc.) without a decision record.
