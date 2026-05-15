# rbe-autoloop protocol

Procedure for an autonomous Claude Code `/loop` that hand-ports `rust-by-example` chapters and, on a real evcxr-typst bug, switches to a four-role subagent fix loop. Read this before launching the loop, and re-read it on every iteration once the loop is running.

## Launching the loop

```
/loop Run one iteration of the rbe autoloop per docs/operations/rbe-autoloop.md.
Re-derive state every invocation; take exactly one action (port a chapter, open
a PR, squash-merge a green chapter PR, or enter/advance bug-fix mode); then stop.
Do not batch iterations.
```

No interval — the loop self-paces via `ScheduleWakeup`. One step per wakeup keeps blast radius small and lets the user interrupt cleanly.

## State derivation (run every iteration before deciding)

| Source | Command / read | What it tells us |
|---|---|---|
| Working tree | `git status --porcelain` | clean / dirty |
| Branch | `git branch --show-current` | on `main` or a feature/fix branch |
| Recent commits | `git log --oneline -5` | what we just landed |
| Open PRs | `gh pr list --author "@me" --state open --json number,headRefName,mergeable,statusCheckRollup` | what's pending |
| Watch loop | `pgrep -fl "evcxr-typst watch"` | alive / dead |
| Recent eval errors | `tail -200 /tmp/watch.log \| grep -aE "WARN evcxr_typst\|ERROR"` | new bug since last iteration |
| Last ported chapter | tail of `examples/rust-by-example/main.typ` (`#include` list) | where we are in SUMMARY |
| Next chapter | `.rust-by-example/src/SUMMARY.md` — first un-ported entry after the last include | where to go next |
| Invariants | auto-memory + `examples/rust-by-example/CLAUDE.md` | conventions to preserve |

State is fully re-derived each iteration; nothing is carried between wakeups in volatile memory. Persistent state lives in the repo (branches, PRs, journal) and `/tmp/watch.log`.

## Decision table (first match wins)

| Condition | Action this iteration | Stop after action |
|---|---|---|
| Watch loop is dead | Restart: `nohup ./target/debug/evcxr-typst watch --allow-eval --root . examples/rust-by-example/main.typ >/tmp/watch.log 2>&1 &` | yes |
| Watch log shows a fresh `WARN evcxr_typst::watch` that is **not** a known chapter-fidelity quirk (see classifier) | Enter BUG-FIX MODE step 1 | yes |
| On a feature branch, open chapter PR, all checks green, mergeable | `gh pr merge <n> --squash --delete-branch`; `git checkout main && git pull --rebase` | yes |
| On a feature branch, open chapter PR, checks pending | Stop, wait | yes |
| On a feature branch, open chapter PR, checks failed or merge conflict | Stop, surface to user | yes (exit loop) |
| On a `fix/*` branch, open PR, all checks green | **Stop and ask user for `go`** — bug-fix PRs are gated per `merge-gate` setting | yes |
| On `main`, clean tree, more chapters in SUMMARY | Port next chapter on a fresh `rbe/<slug>` branch; verify; commit; push; open PR | yes |
| On `main`, clean tree, SUMMARY exhausted | Stop, report completion | yes (exit loop) |
| Any other ambiguous state (uncommitted work + no branch context, half-staged hunks, etc.) | Stop, surface to user | yes (exit loop) |

## Chapter-port action (one iteration)

1. Branch: `git checkout -b rbe/<slug>` from `main`.
2. Write `examples/rust-by-example/<path>.typ` per `examples/rust-by-example/CLAUDE.md` conventions.
3. Add `#include` to `examples/rust-by-example/main.typ` in SUMMARY order.
4. Update README.md chapter table.
5. Wait for watch loop to pick up the new file (sleep 6–10s), then verify:
   - All `evcxr.rust-main` / `evcxr.rust` snippets produced sidecars under `.evcxr-typst-cache/<id>.txt`.
   - No new `WARN evcxr_typst::watch` lines in `/tmp/watch.log` *for this chapter's ids*.
   - `typst compile --root . examples/rust-by-example/main.typ` exits 0 (fallback path).
6. Write `journal/YYYY-MM-DD-NNN-<slug>.md` describing what was tried, what happened, what was learned, follow-ups.
7. Commit (HEREDOC body, no escaped backticks).
8. `git push -u origin rbe/<slug>`.
9. `gh pr create` with the established one-paragraph body style.

Next iteration's decision table picks up the open PR and squash-merges once checks are green.

### Real-bug vs chapter-fidelity classifier

Source-only and continue in-session:

| Symptom | Class | Treatment |
|---|---|---|
| `WARN ... Couldn't automatically determine type` on a top-level `let x = Some(...);` | chapter fidelity | Drop `evcxr.rust(...)`, render as `#raw(..., lang: "rust", block: true)`; note in chapter prose; journal it |
| `previously defined` redefinition collision with prior chapter | chapter fidelity | Same — source-only the offending block |
| Upstream block tagged `ignore,mdbook-runnable` containing a deliberate compile error | chapter fidelity | Source-only by convention (`examples/rust-by-example/CLAUDE.md` rule 1) |
| Typst-side parse error in our chapter `.typ` | author bug | Fix in-session, no escalation |

Enter BUG-FIX MODE:

| Symptom | Class |
|---|---|
| evcxr-typst panics / SIGABRT / SIGSEGV during eval | real bug |
| Watch loop fails to re-eval after a file change (no log line; no sidecar refresh; eval cycle stuck) | real bug |
| Sidecar schema error / missing field reported by `lib.typ` | real bug |
| CLI rejects a previously valid invocation | real bug |
| `cargo test -- --test-threads 1` newly fails on `main` | real bug |
| Cross-snippet eval state corrupts (a snippet's output depends on a sibling's stale binding from a prior run) | real bug |

## BUG-FIX MODE (spans multiple iterations)

Sequencing one role per iteration, with the orchestrator (the in-session Claude) being the only thing that pushes or opens PRs. Auto-memory flags subagent push-drift as a recurring problem — restate the "no push, no PR" constraint in every subagent prompt.

### Step 1 — Capture (orchestrator, one iteration)

- Branch off `main`: `git checkout -b fix/<slug>` (slug names the symptom, e.g. `watch-noop-runaway`).
- Write `journal/YYYY-MM-DD-NNN-<slug>.md`: bug symptom, reproduction, suspected area, any quick triage already done.
- Commit the journal entry. No push yet.

### Step 2 — Plan (Opus subagent, worktree-isolated)

- Spawn via `Agent` with `subagent_type: Plan`, `isolation: worktree`, `name: plan-<slug>`.
- Worktree shares `CARGO_TARGET_DIR` (per `feedback_subagent_target_dir`).
- Prompt: read the journal entry, the relevant source under `crates/evcxr-typst/`, and the failing reproduction. Produce a step-by-step plan in the worktree (no code edits). Identify critical files. **Do not push. Do not open a PR.**
- Plan agent returns its design as the tool result; orchestrator reads it and proceeds.

### Step 3 — Implement (Sonnet subagent, same worktree)

- Spawn via `Agent` with `subagent_type: general-purpose` or a dedicated implementer agent, `isolation: worktree` (same worktree path returned by the plan agent or a fresh one with the plan attached), `name: impl-<slug>`.
- Prompt: implement the plan. Run `cargo test -- --test-threads 1` (mandatory per `project_test_constraints`). No `rtk` for cargo runs. **Do not push. Do not open a PR.** Restate this twice.
- Implementer commits the fix on the worktree's branch and reports back.

### Step 4 — Review (Opus subagent — continue plan agent if alive)

- `SendMessage` to `plan-<slug>` if still resident; else spawn fresh `Agent` with `subagent_type: Plan`, `name: review-<slug>`.
- Prompt: review the implementation against the plan; flag drift, missed cases, test gaps. Produce a written review (no code edits). **Do not push. Do not open a PR.**

### Step 5 — Fix review nits (Sonnet subagent, optional)

- Skip if review surfaces nothing actionable.
- Otherwise spawn implementer agent against the worktree with the review notes attached. Same no-push restatement.

### Step 6 — Verify (orchestrator)

- In the worktree: `cargo test -- --test-threads 1`. Must pass.
- Pull the worktree branch into the main cwd. `cargo build -p evcxr-typst` (rebuilds the binary that the watch loop uses).
- Kill the current watch loop, restart it with the new binary, tail `/tmp/watch.log` for a clean startup.

### Step 7 — Propose (orchestrator)

- `git push -u origin fix/<slug>`.
- `gh pr create` — body should link the journal entry and summarize the plan→impl→review→fix→verify trace.

### Step 8 — Merge (gated)

- **Bug-fix PRs are gated** per the autoloop's merge policy (set during loop design): orchestrator stops and asks user for `go` before squash-merging, because the fix changes binary semantics that subsequent chapters depend on.
- On `go`: squash-merge, `git checkout main && git pull --rebase`, `cargo build -p evcxr-typst`, kill+restart watch.
- Resume chapter authoring on the next iteration.

## Merge-gate matrix

| PR class | Auto-merge on green? |
|---|---|
| Chapter port (`rbe/*`) | yes |
| Bug fix (`fix/*`) | no — ask user for `go` |
| Operations / docs (`ops/*`) | no — ask user for `go` (low traffic; the explicit signal is cheap) |
| Anything else | no — ask user |

## Stop conditions (loop exits, surface to user)

- Chapter PR checks fail
- Chapter PR has merge conflicts
- Bug-fix loop produces a still-failing test suite after step 5
- Same chapter/PR appears in 3+ consecutive iterations without progress (drift signal)
- All SUMMARY chapters ported
- Ambiguous state in the decision table

## Authorization scope

Launching the `/loop` once authorizes the orchestrator to:

- Open chapter PRs against this repo (origin: `CGMossa/evcxr-typst`)
- Squash-merge **chapter** PRs autonomously once checks are green
- `git pull --rebase` on `main` after each merge
- Restart the watch loop and rebuild the evcxr-typst binary
- Spawn subagents in worktrees for bug-fix mode

Launching does **not** authorize:

- Pushing to `upstream` (`evcxr/evcxr` or any other remote that isn't `origin`)
- Squash-merging bug-fix or operations PRs (these need an explicit `go`)
- Force-pushing
- `--no-verify` commits
- Skipping `cargo test -- --test-threads 1`
- Switching the `evcxr` path-dep without amending `D-006` / `D-025`

## Iteration budget hygiene

- One action per iteration. If a step needs more (e.g. writing several files for a chapter), still do it in one iteration but stop afterwards.
- If two consecutive iterations land on the same row of the decision table without state changing, that's drift — stop and surface.
- Cold rustc compiles per snippet are slow but not hung (auto-memory `feedback_eval_cold_compile_is_slow_not_hung`). Don't kill long-running evals.
