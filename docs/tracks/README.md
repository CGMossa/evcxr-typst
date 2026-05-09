# Side tracks

Tracks that are **off the main critical path**. They explore valuable directions, get the same level of design rigor as Phase 0 surfaces, and have their own backlog entries — but they do not block the main `PLAN.md` phases. Land them when there's bandwidth; never at the cost of the Phase 1–4 main journey.

Each track is one self-contained markdown file in this directory. Each declares its dependencies on the main track explicitly (which phase must ship first), what it adds, and what it does not promise.

## Active tracks

| Track | File | One-liner | Status |
|---|---|---|---|
| Semantic Typst (rust-analyzer integration) | [`semantic-typst.md`](semantic-typst.md) | Surface type names, signatures, docs, and diagnostics from rust-analyzer into `.typ` documents — literate programming with semantic awareness. | designed; tasks T-S01..T-S04 in `docs/BACKLOG.md` |
| Rust-by-example port | [`rust-by-example-port.md`](rust-by-example-port.md) | Port the upstream rust-by-example book (~198 chapters, dual MIT/Apache-2.0) to Typst documents in `examples/rust-by-example/`, evaluated end-to-end. The flagship "real Rust at scale" demonstration. | active hand-port on `track/rbe-incremental` (chapters `hello`, `hello/comment` landed); mechanical porter T-B00..T-B06 designed but not started. See `examples/rust-by-example/CLAUDE.md` and `journal/`. |

## Why a side-track concept

Some ideas are clearly worth doing and clearly not on the critical path. Putting them in the main `BACKLOG.md` queue invites accidental prioritisation and pretending the main journey is bigger than it is. Putting them in throwaway notes loses the design effort. A `tracks/` directory is the middle ground: design is preserved, ordering against the main plan is explicit, and a future agent or contributor can pick a track up cleanly without re-deriving the rationale.

## Adding a track

1. Write `docs/tracks/<name>.md`. Same internal shape as a main-plan PLAN+ARCHITECTURE+BACKLOG combined: vision, target UX, architecture options, phased plan, schema sketches, open questions.
2. Add a one-line entry in the table above.
3. Add `T-S<NN>` task entries in the **Side tracks** section of `docs/BACKLOG.md`.
4. Cross-reference: the track's main-plan dependency phase should mention the side track exists; the side track must declare which main phase it depends on.
