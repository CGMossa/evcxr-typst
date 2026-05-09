# `.vscode/`

Editor convenience config for VSCode users. Non-load-bearing — every recipe here is a thin wrapper around a `cargo` or `evcxr-typst` invocation that works fine from any terminal.

## Available tasks

Open the command palette (`⌘⇧P` / `Ctrl⇧P`) → `Tasks: Run Task`:

| Task | What it does |
|---|---|
| `evcxr-typst: build` | `cargo build -p evcxr-typst --bin evcxr-typst`. Default build task. |
| `evcxr-typst: watch (rust-by-example)` | Start the rbe authoring loop on `examples/rust-by-example/main.typ`. Edits to any chapter file (including subdirs like `hello/comment.typ`) re-evaluate snippets and re-render the PDF. Stop with `Ctrl-C` in the terminal panel. |
| `evcxr-typst: watch (custom entry)` | Same, but prompts for the entry doc — pick any `.typ` you're authoring. |
| `evcxr-typst: run (one-shot, rust-by-example)` | One-shot evaluate + render. Useful for seeding sidecars before watch starts (workaround for issue #30 — watch doesn't backfill missing sidecars). |
| `evcxr-typst: clean (current example dir)` | Drop the sidecar view and GC unreferenced CAS entries. Prompts for the entry doc. |

Each watch task is single-instance (`instanceLimit: 1`) — running it again while a previous instance is alive will give you a focus-existing prompt instead of starting a second watcher.

## How the rbe authoring loop works

1. Build (or run-task: `evcxr-typst: watch (rust-by-example)` will build first via `dependsOn`).
2. The watcher prints `watch running; press Ctrl-C to stop.` once the typst-watch child is up.
3. Edit any `.typ` chapter file. The notify watcher (recursive on the entry's parent directory) sees it; the watch loop runs `discovery::discover` and classifies the change as `AppendOnly` / `LeafEdit` / `ResetAndReplay` / `Noop`. Re-eval happens for everything but `Noop`.
4. The terminal stream shows `compiled successfully in NNms` from typst-watch and `watch cycle plan plan="..."` debug lines from evcxr-typst (set `EVCXR_TYPST_LOG=evcxr_typst=debug` if you want to see them; the task default is `info`).
5. Sidecars land in `<entry-parent>/.evcxr-typst-cache/<id>.txt` (and `.png` / `.cbor` / etc. for non-text MIME types).

For a full design walkthrough see [`../docs/design/watch-loop.md`](../docs/design/watch-loop.md).

## Known watch quirks

- **Noop runaway after edits subside (#29).** typst-watch's `.pdf` rewrite trips `is_relevant` → `Noop` cycles fire every ~660 ms. Harmless but noisy in the terminal panel.
- **No missing-sidecar backfill at startup (#30).** If `_index.json` is missing, watch alone won't seed it; run `evcxr-typst: run (one-shot, ...)` once first.

Both are tracked as open issues; tasks here will pick up the fixes once they ship.
