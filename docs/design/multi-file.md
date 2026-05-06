# Multi-document and multi-file project layout (T-D09)

> **Scope**: how `evcxr-typst` handles real Typst projects that span more than one `.typ` file via `#import`, `#include`, or local-package imports. Resolves the multi-file open question flagged in `watch-loop.md` § 9 Q2 and tightens the workspace-level claim in `cache.md` § "Cache layout on disk".
>
> **Status**: design (Phase 0 follow-up, T-D09). Decision recorded as **D-018**.

---

## 0. TL;DR

- **Single entry file** in v0. Dependent files are auto-discovered. Multi-entry-file projects are deferred to v1.
- **Cache scope** = the directory of the entry file (the *workspace*). One cache per entry file; co-located entry files in the same directory share one CAS but isolate by `index.json` keyed on entry-file relative path.
- **Discovery algorithm** = `typst query` once for snippets/deps, parse the same query result's source-location field for the file set, then parse each member file's `#import`/`#include` lines via `syn`-equivalent (the `typst-syntax` crate, or a regex backstop) to compute the closure. Re-run on every cycle.
- **Global snippet ordering** = files are flattened in **encounter order** within the entry file's import/include traversal (depth-first, source-first); inside each file, the existing `loc.doc_order` rule applies; `(file_seq, doc_order)` is the global tuple.
- **`dep()` visibility** = global, document-order. A `#dep` declared anywhere earlier in the flattened global order is visible to every later snippet, regardless of which file each lives in.
- **ID collision rule** = applies project-wide, not per-file. Default-vs-default collisions get the occurrence-index suffix; explicit-vs-explicit collisions are a hard error citing both files.
- **Watch-set** = recomputed after every successful `typst query` cycle as the union of all files reached during discovery.

---

## 1. Project model

### 1.1 The three roles

| Role | Definition | How many |
|---|---|---|
| **Entry file** | The `.typ` file passed to `evcxr-typst run <path>`. The CLI's argument. | exactly one in v0 |
| **Project root** | The directory containing the entry file. **This is the workspace.** | one |
| **Member file** | Any `.typ` file reachable from the entry file by following local `#import`/`#include` (transitively). The entry file itself counts as a member. | ≥ 1 |
| **Imported package** | A `@preview/foo:1.2.3` or `@local/bar:0.1.0` reference. Lives outside the workspace. | 0..n |

We deliberately do **not** introduce the term "project file" or a TOML manifest in v0. The entry file *is* the project's identity; everything else falls out of imports.

### 1.2 On-disk picture

```
~/papers/quarterly-report/             ← project root (== workspace)
├── main.typ                           ← entry file
├── chapters/
│   ├── intro.typ                      ← member (#import "chapters/intro.typ")
│   ├── results.typ                    ← member (#include "chapters/results.typ")
│   └── helpers.typ                    ← member (imported by results.typ)
├── lib/
│   └── plotting.typ                   ← member (imported by main.typ)
├── data.csv                           ← not a member (Typst data dep, not .typ)
├── .evcxr-typst-cache/                ← workspace-local, gitignored by default
│   └── v1/
│       ├── index.json                 ← keyed on entry-file rel-path: "main.typ"
│       └── cas/...
└── @local-package-cache/              ← outside our concern; Typst manages
```

The cache directory always lives next to the entry file: `entry.parent() / ".evcxr-typst-cache"`. If two entry files coexist in the same directory (`paper.typ` and `slides.typ` sharing `lib/`), the *cache* is shared (same CAS) but each has its own `index.json` view — see § 4.

---

## 2. Discovery algorithm

### 2.1 Choice: hybrid `typst query` + parse imports

Of the four candidates in the briefing:

- **(a) `typst query` only.** Insufficient: `typst query` returns metadata locations but does *not* (today) report the full set of files compiled. The `location()` value attached to each metadata element exposes the page/position only, not the source file path. We would still need to parse imports.
- **(b) Parse `#import`/`#include` ourselves.** Necessary anyway. Lightweight (string-level matching after stripping comments + raw blocks) — but on its own, won't catch dynamic imports computed by Typst code.
- **(c) Require `evcxr-typst.toml` with explicit file list.** Rejected as v0 default: it's a chore, gets out of date, and doesn't compose with how Typst authors actually work. Kept as a **fallback override**: if `evcxr-typst.toml` exists at the entry file's parent, its `[project] files = [...]` list overrides discovery and bypasses § 2.2 entirely. Useful for hermetic CI and for users with dynamic imports we can't statically resolve.
- **(d) Other.** Considered: invoke `typst compile --emit-deps`. Rejected because (i) the deps file format isn't stable as a public contract, (ii) it requires a successful compile (we want to handle in-progress edits), (iii) the snippet metadata query already costs us a full Typst parse — the same data is easier to get from the snippet metadata's `loc` if we can extend it (see open question Q1).

**Chosen v0 approach (b) augmented by (a)** — parse imports directly; cross-check against the file paths reported by `typst query` if available.

### 2.2 Pseudocode

```rust
fn discover_files(entry: &Path) -> Result<DiscoveredSet> {
    // 0. If user provided an evcxr-typst.toml, use its file list verbatim.
    if let Some(toml) = read_optional_manifest(entry.parent())? {
        return Ok(toml.into_set());
    }

    // 1. Standard discovery: BFS from entry, following local imports/includes.
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut order: Vec<PathBuf> = Vec::new();          // first-encounter order
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    queue.push_back(canonical(entry)?);

    while let Some(path) = queue.pop_front() {
        if !seen.insert(path.clone()) { continue; }
        order.push(path.clone());

        let src = std::fs::read_to_string(&path)?;
        for tgt in extract_local_typ_imports(&src, &path) {
            // tgt resolved relative to `path.parent()`; @preview/@local refs filtered out.
            if tgt.exists() && tgt.extension() == Some(OsStr::new("typ")) {
                queue.push_back(canonical(&tgt)?);
            }
        }
    }

    Ok(DiscoveredSet { entry: canonical(entry)?, files: order })
}

fn extract_local_typ_imports(src: &str, file: &Path) -> Vec<PathBuf> {
    // Use typst-syntax to parse to a SyntaxNode tree; walk for ModuleImport
    // and ModuleInclude nodes; collect their target string literals.
    // Filter:
    //   - "@preview/foo:..." → external package, skip
    //   - "@local/bar:..."   → external package (workspace package), skip
    //   - "./relative.typ"   → resolve against file.parent()
    //   - "/abs.typ"         → resolve against project root (entry.parent())
    //   - non-string targets (computed: `#import compute-path()`) → log + skip + flag
    ...
}
```

`extract_local_typ_imports` is a pure-function over the file's bytes, so it does **not** require a successful Typst compile and tolerates in-progress edits in *other* files (only the file being parsed needs to be syntactically valid enough for `typst-syntax` to produce a usable tree; on parse failure we fall back to a regex extractor and record a discovery warning).

**Why BFS, not DFS:** so that the *file order* is deterministic and matches a user's mental model of "main.typ pulls in these things, in this order." DFS would interleave deeply nested imports first, which is surprising. See § 3 for why this ordering choice matters for snippet ordering.

### 2.3 What about `@local/` packages?

A `@local/mypackage:0.1.0` import resolves to `~/.local/share/typst/packages/local/...` (Linux) or platform equivalents. We treat it the same as `@preview/`: **out of scope**, do not follow. Snippets defined inside a local package are not evaluated by `evcxr-typst` in v0. (If a user wants their helper snippets evaluated, they should `#import "lib/helpers.typ"` instead of packaging it.) Document this clearly in user-facing docs.

This is conservative; relaxing it later just means widening the BFS frontier to include resolved package paths — backward-compatible.

---

## 3. Global snippet ordering

### 3.1 The rule

**Snippets are ordered by `(file_seq, doc_order_within_file)`**, where:

- `file_seq` is the index of the snippet's file in the BFS-discovery order from § 2.2.
- `doc_order_within_file` is the position within that file as reported by `typst query` (the existing `loc.doc_order` semantics).

The CLI then numbers all snippets globally as `global_doc_order = 0, 1, 2, ...` after the sort. This `global_doc_order` is what feeds:
- The `prior_chain_hash` Merkle chain (`cache.md` § "Cache key — concrete formula").
- `:dep` emission ordering (D-013).
- The `loc.doc_order` field exposed in the metadata (now globally ordered).

### 3.2 Worked example with three files

```
main.typ:
    #import "chapters/intro.typ"
    #import "chapters/results.typ"
    #rust(```rust let totals = compute(); println!("{totals:?}"); ```)   // M1
    #rust(```rust render_summary(); ```)                                   // M2

chapters/intro.typ:
    #rust(```rust struct Total { v: u64 } ```)                             // I1
    #dep("serde", "1")                                                     // ID1
    #rust(```rust use serde::Serialize; ```)                               // I2

chapters/results.typ:
    #import "../lib/helpers.typ"
    #rust(```rust fn compute() -> Total { Total { v: 42 } } ```)           // R1
    #rust(```rust fn render_summary() { use_helper(); } ```)               // R2

lib/helpers.typ:
    #rust(```rust pub fn use_helper() {} ```)                              // H1
```

**Discovery order** (BFS from `main.typ`):
1. `main.typ`
2. `chapters/intro.typ`     (first import)
3. `chapters/results.typ`   (second import)
4. `lib/helpers.typ`        (transitive, from `results.typ`)

**Within-file `doc_order`** is what `typst query` returns for each file in isolation.

**Global order** (file_seq × within-file order):

| global_doc_order | snippet | rationale |
|---|---|---|
| 0 | M1 | main.typ, first snippet |
| 1 | M2 | main.typ, second |
| 2 | I1 | intro.typ, first |
| 3 | ID1 (dep) | intro.typ, dep marker |
| 4 | I2 | intro.typ, third |
| 5 | R1 | results.typ, first |
| 6 | R2 | results.typ, second |
| 7 | H1 | helpers.typ, first |

**Wait — but M1 *uses* `compute()` defined in R1.** Yes, and that's a problem under naive global ordering: M1 evaluates first (global 0), R1 evaluates later (global 5), so M1 fails with "undefined function `compute`" at evaluation time.

**Resolution.** We accept this. Two reasons:
1. Typst itself executes `main.typ` top-to-bottom; the `#import` statements at the top run *before* the `#rust(...)` calls below them. From Typst's point of view, `chapters/intro.typ` and `chapters/results.typ` evaluate (their metadata gets emitted) *before* the calls in `main.typ`. So `typst query` already reports M1's `loc.position` after the imports' positions in document-source order — meaning if we **read `loc.position` directly** for ordering, the ordering is correct: imports' bodies appear before the importing file's body.

   But `loc.position` from `typst query` returns a (page, x, y) tuple after layout, not a source-text offset. **This is the crux of the open question Q1 below.**

2. The pragmatic interim rule for v0: **document order = "BFS discovery × within-file source order"**, and tell the user. If they want `compute()` defined before `main.typ`'s usage, they put the definition above `main.typ`'s call by structuring the file: define helpers in `chapters/intro.typ` (file_seq = 2), use them in `chapters/results.typ` (file_seq = 3) or in `main.typ` only **after** the definition file gets evaluated.

   Concretely: a user-facing rule of thumb is "imported files' Rust snippets evaluate *before* the importing file's snippets, in import order." Match the worked example: rewriting `main.typ` to put its `#rust(...)` calls in a chapter file, with `main.typ` only handling structure, makes the ordering match intuition.

### 3.3 Diamond imports

What if `chapter1.typ` is imported into both `main.typ` and `appendix.typ`?

BFS visits each file **at most once** (the `seen` set). The first `#import` to reach `chapter1.typ` claims its position in `file_seq`. Subsequent imports from other files re-use that same `file_seq` — so chapter1's snippets evaluate exactly once, at that fixed slot.

This is the right behaviour: we want each snippet to evaluate exactly once per cycle, and `:dep`/`fn`/`struct` definitions to take effect once. Diamond imports are common when files share helpers.

**Cycle detection.** If `a.typ` imports `b.typ` and `b.typ` imports `a.typ`, BFS handles it (each visited once). If a *true* cyclic dependency would matter for evaluation order, that's already a Typst-side modelling problem we can't solve.

---

## 4. Cache scope

### 4.1 Where on disk

`<workspace>/.evcxr-typst-cache/v1/`, where `<workspace>` = the entry file's parent directory.

Concretely: `evcxr-typst run papers/quarterly-report/main.typ` writes its cache at `papers/quarterly-report/.evcxr-typst-cache/`. Even if `main.typ` imports `lib/foo.typ` from a sibling directory, the cache stays next to `main.typ`.

This is the same rule as `cache.md` § "Cache layout on disk" originally specified — we are formalizing "workspace level" as "entry file's parent directory".

### 4.2 What gets shared, what stays isolated

**Shared (across entry files, across documents) — the CAS.** The content-addressed store is keyed by the cache key (`cache.md` § "Cache key — concrete formula"), which doesn't include the entry-file path. Two entry files in the same workspace, or even two entry files in two different workspaces with the same snippet content + same prior-chain + same toolchain, will have the same cache key and share storage.

The CAS structure already supports this (D-010): `cas/<XX>/<full-cache-key>/`. Two `index.json` files pointing into the same CAS dir is fine; each entry-file holds its own materialized id-addressed view.

**Isolated (per entry file) — the `index.json` and id-addressed view.** Each entry file gets its own:
- `index.json` (named `index-<entry-rel-path-slug>.json` if more than one entry file exists in the workspace, else `index.json`)
- materialized `<id>.<ext>` sidecars that the Typst package reads at render time

Concretely, for two entry files `paper.typ` and `slides.typ` in the same directory:

```
.evcxr-typst-cache/
└── v1/
    ├── index.paper.json           ← only paper.typ's snippets
    ├── index.slides.json          ← only slides.typ's snippets
    ├── views/
    │   ├── paper/<id>.<ext>       ← materialized for paper.typ
    │   └── slides/<id>.<ext>      ← materialized for slides.typ
    └── cas/
        └── <XX>/<key>/...         ← shared
```

The Typst package side reads from `views/<entry-stem>/<id>.<ext>` based on which entry file is being compiled. The package needs to know its entry-file context — see open question Q2.

**One-entry-file simplification.** When there's only one entry file in the workspace (the v0 common case), drop the `views/<stem>/` layer and put materialized files directly at `.evcxr-typst-cache/v1/<id>.<ext>`. This is the simple, ARCHITECTURE.md-compatible layout. We only switch to per-stem subdirs when a *second* entry file is registered.

### 4.3 Two `.typ` files in the same directory wanting isolated caches

Use case: a tutorial repo with `tutorial-1.typ`, `tutorial-2.typ`, ..., each independent.

Today: cache key inputs include `prior_chain_hash`, which is computed over the snippets in the entry file's discovered set. Two unrelated tutorial files will naturally have different prior chains, so no actual snippet output collides. The CAS sharing is harmless and helpful (e.g. if both tutorials use `let x = 5; println!("{x}");` as their first snippet, that single CAS entry serves both).

If a user genuinely wants disconnected caches, two paths:
- Run with `--cache-dir <path>` to point at a different directory.
- Move the files into separate directories (the natural Typst-project structure).

### 4.4 Multi-file cache invalidation

When an imported file changes, every snippet whose `prior_chain_hash` includes that file's snippets is invalidated. This is the same mechanism as single-file: the chain hash flows through global ordering. No change to the cache-key formula is needed; we just need global ordering (§ 3) to compute it correctly.

---

## 5. `dep()` visibility across files

### 5.1 Rule

**A `#dep(...)` is visible to every snippet whose `global_doc_order` is greater than the dep's.** File boundaries do not bound dep visibility — the global order is the only thing that matters.

This is the natural extension of D-013 ("`dep()` calls remain inline-anywhere; the CLI pre-collects in document order") to the multi-file case: "document order" is now "global document order across all member files."

### 5.2 Worked example

Continuing § 3.2's three-file project:

| global_doc_order | element | active deps after this point |
|---|---|---|
| 0 | M1 (snippet) | (none) |
| 1 | M2 (snippet) | (none) |
| 2 | I1 (snippet) | (none) |
| 3 | **ID1 — `#dep("serde", "1")`** | serde 1.x |
| 4 | I2 (snippet) — `use serde::Serialize;` | serde 1.x |
| 5 | R1 (snippet) | serde 1.x |
| 6 | R2 (snippet) | serde 1.x |
| 7 | H1 (snippet) | serde 1.x |

I2 sees `serde` (good — that's why ID1 is *above* I2 in `intro.typ`). So do R1, R2, H1 — even though they're in different files, they appear later in global order. M1 and M2 do **not** see `serde` because they're in `main.typ` which is `file_seq=0`, while ID1 is in intro.typ `file_seq=2`.

### 5.3 Conflict detection

D-013's hard-error rule for conflicting versions extends as-is: a `#dep("regex", "1.10")` in one file and `#dep("regex", "1.11")` in another file is a hard error, the message naming both file paths and `loc.doc_order`s.

---

## 6. ID collision rule across files

**ID collisions are checked across the entire project, not per file.**

- Two default IDs colliding (same content hash, anywhere in the project) → occurrence-index suffix per `snippet-identity.md` § "Collision handling", in global-order.
- Two **explicit** IDs colliding (same `id:` string, anywhere) → **hard error**, naming both files + `doc_order` within each.

Worked: `chapter1.typ` has `#rust(id: "results", ...)` and `chapter2.typ` has `#rust(id: "results", ...)`. Both are reachable from `main.typ`. The CLI errors out:

```
Error: explicit snippet id "results" defined in two places:
  chapters/chapter1.typ (page 4, line 12)
  chapters/chapter2.typ (page 9, line 7)
Each explicit id must be unique within the project. Rename one.
```

This matches the rationale in D-007: explicit IDs are user choices; silently disambiguating them would hide bugs (two snippets meant to be different things accidentally given the same name, then the cache surfaces the wrong one).

---

## 7. Watch-set algorithm

### 7.1 The watch set

The watch set is the **union of all member files** plus their parent directories (for atomic-rename detection, per `watch-loop.md` § 3).

```
watch_set = { f for f in discovered_set.files }
            ∪ { f.parent() for f in discovered_set.files }
            ∪ { entry.parent() }                  // catch new files appearing
```

### 7.2 Recomputation

The watch loop already runs `typst query` on every cycle (`watch-loop.md` § 1). We piggy-back: after each successful query, re-run § 2.2's discovery. Diff the resulting file set against the current watch set:

```rust
fn update_watch_set(watcher: &mut Watcher, prev: &HashSet<PathBuf>, curr: &HashSet<PathBuf>) {
    for added in curr.difference(prev) {
        watcher.watch(added, NonRecursive)?;
        log::debug!("now watching {added:?} (newly imported)");
    }
    for removed in prev.difference(curr) {
        watcher.unwatch(removed)?;
        log::debug!("stopped watching {removed:?} (no longer imported)");
    }
}
```

This handles both directions:
- User adds `#import "chapters/new.typ"` to `main.typ`: next cycle's discovery picks up `new.typ`, watcher starts watching it. Future edits to `new.typ` will trigger cycles.
- User deletes `#import "chapters/old.typ"`: `old.typ` is no longer reached, the watcher stops watching it. Subsequent edits to `old.typ` are ignored (which is correct — it's no longer part of the project).

### 7.3 Re-classify trigger from imported-file edits

The classification logic (`watch-loop.md` § 4) operates on the snippet list returned by `typst query`. It doesn't care which file each snippet came from; it only cares about `(id, src, doc_order)`. So edits in any member file feed naturally through the same diff:

- Edit a file with no `<evcxr-snippet>` markers (e.g. a pure-prose chapter): `typst query` returns identical snippets → `Plan::Noop`.
- Edit a snippet inside `chapter2.typ`: `typst query` shows a `SrcChanged` for that snippet's id → classification proceeds normally.
- Add a new `#import` of a file with snippets: discovery picks up the new file; new snippets appear in `curr`; classified as Added → `Plan::ResetAndReplay` (because the inserts land in the middle of global order).
- Remove an `#import`: snippets vanish from `curr`; classified as Removed → reset and replay.

### 7.4 Watch-set bound

In pathological cases (a project importing thousands of files), the watcher inotify-budget could run out. We don't defend against this in v0 — surface a clear error from `notify` if it happens, and document that `evcxr-typst.toml` with an explicit smaller list is the workaround.

### 7.5 Cycles and infinite loops

A file edit that triggers a re-query that triggers a re-discovery that updates the watch-set: this is one cycle, not a loop. We only re-run discovery on successful queries; the watch-set update is idempotent. No risk of feedback.

---

## 8. Multi-entry-file projects

### 8.1 v0: single entry file

**Recommendation: single-entry-file in v0.** Multi-entry-file projects deferred to v1.

Reasons:
- Single-entry covers >90% of real-world cases (one paper, one report, one slide deck).
- Multi-entry introduces nontrivial complexity: which entry's `index.json` does the package read at render time? When `paper.typ` and `slides.typ` share `lib.typ`, do edits to `lib.typ` trigger re-eval of both? Do we run two CommandContexts in parallel, or one with state-bouncing between entry files?
- The v1 path (below) is clean and additive — we don't paint ourselves into a corner by punting.

### 8.2 v0 ergonomic workaround

A user with `paper.typ` + `slides.typ` sharing `lib.typ` can:
- Run `evcxr-typst run paper.typ` and `evcxr-typst run slides.typ` separately. Each gets its own watch loop, evcxr child, and `index.json` view in the shared `.evcxr-typst-cache/`.
- The CAS is shared (free dedup of identical snippets across entry files).
- A shell wrapper or `Justfile` can drive both. Document this pattern.

This is exactly the v0 cache-isolation story from § 4.2. So the multi-entry case is *operationally* supported via two CLI invocations; what's deferred is a single `evcxr-typst run --multi paper.typ slides.typ` mode.

### 8.3 v1 path

When we lift the v0 limitation:
- CLI accepts a list of entry files: `evcxr-typst run paper.typ slides.typ`, or an `evcxr-typst.toml` `[project] entries = [...]`.
- One discovery pass per entry file; the union of all member files is the watch set.
- One CommandContext per entry file (parallelism-friendly), or one shared with snippet-set switching (memory-friendly). Decide based on measurement.
- Per-entry `index.<stem>.json` (already designed in § 4.2 for the future-proofing).

Nothing in v0's design forecloses this path.

---

## 9. Open questions

These need testing or upstream clarification before we lock the implementation.

### Q1 — Does `typst query`'s `location()` field expose the source-file path of imported-file metadata?

**What we need to verify.** When `main.typ` `#import`s `chapter1.typ` and `chapter1.typ` contains a `metadata(...)<evcxr-snippet>` value, what does `typst query --field location main.typ '<evcxr-snippet>'` return for that element? Does the location include the source file path, only a (page, x, y) layout coordinate, or something else?

**Why it matters.** If `location()` already exposes the source file, we can skip § 2.2's `extract_local_typ_imports` parsing entirely — the query result *is* the file set. That would simplify discovery and tighten correctness (we'd be reading the same file set Typst itself compiles, including any dynamically-computed imports).

**Test plan when we have a CLI prototype.**
1. Construct a 2-file project with one `<evcxr-snippet>` in each file.
2. Run `typst query --field location main.typ '<evcxr-snippet>'`.
3. Inspect the JSON output for any source-file information.
4. Repeat with `--field` variants (`--field=value`, `--field=meta`, no `--field`) and with `typst query --one`.
5. Look at `typst-cli`'s `query` command source for what `location()` is willing to expose.

If the answer is "no source-file path", v0 stays as designed (§ 2.2). If "yes", revise § 2.2 to drop the AST-parsing step in favour of reading file paths from `location()`.

### Q2 — How does the Typst package side know which entry-file's `views/<stem>/` to read from?

**What we need to figure out.** For the multi-entry-file v1 path: when `lib.typ` is imported into both `paper.typ` and `slides.typ`, and the package's `rust(...)` function in `lib.typ` does `read("path/to/<id>.<ext>")`, that `read` path is computed at *Typst evaluation time*. There's no Typst built-in for "what's the entry file?" — `sys.inputs` is the closest thing (for `--input` CLI args).

**Candidate solutions.**
- The CLI passes `--input evcxr_entry_stem=paper` to `typst watch`/`typst compile`. The package reads `sys.inputs.evcxr_entry_stem` and prefixes the read path with `views/<stem>/`. Likely works; needs verification of how `setup()` plumbs this.
- The CLI materializes a fresh sidecar copy at `.evcxr-typst-cache/v1/<id>.<ext>` at the *start* of each entry-file's run, so the package's path resolution stays simple. Means the bytes are tied to the most-recently-run entry file; runs interleaved across entry files in the same workspace would see flapping. Worse than option 1.

Resolve as part of v1 design when we lift the multi-entry restriction. For v0 (single entry file in a workspace), the question is moot — the materialized view sits flat at `.evcxr-typst-cache/v1/<id>.<ext>`.

### Q3 (bonus) — Should an `evcxr-typst.toml` manifest exist in v0 as an opt-in override?

We've described it in § 2.1 as a v0-supported escape hatch. The question is whether the v0 implementation budget includes wiring it up, or whether we ship purely-discovered v0 and add the manifest in v1. Recommend: implement the `[project] files = [...]` reader in v0 (it's ~20 LOC). The `[project] entries = [...]` multi-entry mode is v1.

---

## Cross-references

- `docs/DECISIONS.md` D-013 (`dep()` inline-anywhere — extended here to cross-file) and D-018 (this design).
- `docs/design/cache.md` § "Cache layout on disk" — workspace level formalized as "entry file's parent directory" here.
- `docs/design/watch-loop.md` § 9 Q2 — resolved by this doc.
- `docs/design/snippet-identity.md` § "Collision handling" — collision semantics extended project-wide here.
- `docs/design/package-api.md` § 4.2, § 5 — `dep()` ordering and metadata schema; no changes required, but `loc.doc_order` is now a global field.
- ARCHITECTURE.md § "The pipeline" — will need a one-line note that the cache directory is rooted at the entry file's parent.
