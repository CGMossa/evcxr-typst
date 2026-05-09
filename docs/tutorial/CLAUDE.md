# CLAUDE.md — `docs/tutorial/`

Task-oriented "how do I X with evcxr-typst" documents. Distinct from `docs/design/` (which is design-of-the-tool) and `journal/` (which is raw working notes). A tutorial here teaches one specific thing a writer would actually want to do.

## When to add a tutorial

A tutorial is justified when:

1. A pattern has appeared in two or more `journal/` entries.
2. Or it answers a question that the existing `docs/design/*.md` files don't answer for *writers* (vs. for tool authors).
3. Or it captures a workaround that will outlive a single chapter (e.g. "how to share state across chapters when one chapter forgets to declare an item public").

Don't add a tutorial just because something is interesting. The cost of a stale tutorial is high — it misleads new writers. Either it's load-bearing for someone using evcxr-typst, or it stays in the journal.

## Tutorial shape

Each file is one task. Loose template:

```markdown
# How to <do thing>

(One sentence: what this teaches. One sentence: when you need it.)

## Minimal example

```typ
// Smallest possible doc that demonstrates the pattern.
```

## Why it works

(One short paragraph. Cite the relevant decision record or design doc if there is one.)

## Variations

- "If you also need <X>, do <Y>."
- "If you don't need <Z>, you can drop <W>."

## See also

- `docs/design/<…>.md` for the underlying mechanism.
- `journal/<…>.md` for the experience that motivated this tutorial.
```

## File naming

Plain kebab-case, descriptive: `first-chapter.md`, `cross-snippet-state.md`, `using-deps.md`. No date prefix — tutorials are evergreen.

## Don't

- Don't duplicate `docs/design/` content. Link to it.
- Don't write tutorials about future / proposed features. Tutorials describe what *works today*.
- Don't include long explanatory prose. Reader is task-driven; show, don't lecture.
