# CLAUDE.md — `docs/`

Source of truth for plans, decisions, design, and the agent-ready task queue. Skim this index first; jump to the relevant file for detail.

## Top-level files

| File | What it is | When to read |
|---|---|---|
| `PLAN.md` | Phased roadmap (Phases 1–4) and current status. | Starting a session; checking what's shipped. |
| `ARCHITECTURE.md` | Big-picture design: pipeline, MIME mapping, watch loop, why-not-WASM. | First time touching the codebase. |
| `BACKLOG.md` | Agent-ready task queue. Each task has Done-when criteria and reference reads. | Picking up new work. |
| `DECISIONS.md` | ADR-lite log (D-001 … D-026 currently). | Before making a non-trivial design choice. |
| `INDEX.md` | (If present) navigation aid for cross-linked design docs. | When you don't know which design doc to read. |

## Subdirectories

- `design/` — per-area design specs (cache, watch loop, multi-file, package API, snippet semantics, errors, schema versioning, etc.). Has its own `CLAUDE.md` listing what each file covers.
- `tracks/` — off-main-critical-path side tracks: Semantic Typst (#19), Rust-by-example port (#20). Has its own `README.md`.
- `tutorial/` — task-oriented "how do I X" docs for end users of evcxr-typst. Has its own `CLAUDE.md` and `README.md`.

## Conventions

- **Edit source files, regenerate derived ones.** None of the in-repo docs here are auto-generated today; if that changes (e.g. an `INDEX.md` is built from frontmatter), the build step lives in a `justfile` recipe and will be called out at the top of the generated file.
- **Decision records are append-only.** When a design choice changes, write a *new* D-XXX referencing the old one rather than editing the old in place.
- **Don't proliferate top-level files.** Each new top-level `*.md` raises the cost of orientation. Prefer adding to an existing file or a `design/<area>.md` over a new top-level entry.
- **Cross-link liberally.** Tutorials link to design files; design files link to decision records; decision records link to the tasks that triggered them.
