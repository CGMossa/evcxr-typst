# CLAUDE.md — `journal/`

A working log of the rust-by-example incremental port (track `track/rbe-incremental`). Each entry is one chapter's experience: what I expected, what actually happened, what I had to learn or work around. Distinct from `docs/` (design / spec) and `docs/tutorial/` (polished how-tos): the journal is *raw* and time-stamped.

## When to write an entry

Write one per chapter you port, even if nothing surprising happened ("nothing broke" is a finding). Also write one when you hit something off-chapter that's worth remembering — a CLI bug, a package-API gap, a Typst quirk.

## File naming

`YYYY-MM-DD-NNN-<slug>.md` where `NNN` is a per-day sequence (so two entries on the same day order cleanly). Slug is the chapter or topic.

Examples:
- `2026-05-09-001-hello.md`
- `2026-05-09-002-comments-and-formatted-print.md`
- `2026-05-10-001-fn-main-fidelity-decision.md`

## Entry shape

Loose template — break it freely when it doesn't fit.

```markdown
# <chapter or topic>

**Date:** 2026-05-09
**Branch:** track/rbe-incremental
**Upstream source:** .rust-by-example/src/<path>.md (or n/a)

## What I tried

(One paragraph. The intent.)

## What happened

(Verbatim error messages, surprising outputs, unexpected behaviours. Be specific.)

## What I learned

(The takeaway. Often this is the seed for a tutorial entry or a test.)

## Follow-ups

- [ ] Open: things to chase later, with enough context to pick up cold.
- [x] Done: things resolved within this entry, with a one-line resolution.
```

## What lives here vs. elsewhere

| If it's… | It goes in… |
|---|---|
| Raw experience, time-stamped, possibly messy | `journal/` (here) |
| A pattern that's now stable enough to teach | `docs/tutorial/` |
| A design choice or invariant of the tool | `docs/DECISIONS.md` (decision record) or `docs/design/` |
| A bug surfaced by porting | a test in `crates/evcxr-typst/tests/` referencing the journal entry |

The journal feeds the other three. Don't try to make journal entries polished — they exist precisely so that the polish can come later, with the rough edges still visible.

## Don't

- Don't edit old entries to "fix" historical findings. If a finding turned out to be wrong, write a *new* entry that supersedes it and link back.
- Don't put secrets, API keys, or anything that would be a problem if pushed to GitHub.
- Don't journal trivia ("ran `cargo build`, it worked"). Journal the bits that took thought.
