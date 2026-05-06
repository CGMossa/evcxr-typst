# Watch loop & change classification

Detailed algorithm for `evcxr-typst watch <main.typ>`. Companion to `ARCHITECTURE.md` § "Watch loop", implements `D-003` (linear re-evaluation).

> **Reconciliation note.** `T-D04` (snippet identity, cache key) is being designed in parallel. At the time this doc was written, `docs/design/cache.md` and `docs/design/snippet-identity.md` did not yet exist. This doc therefore uses the working assumptions from `ARCHITECTURE.md` and `D-005`:
>
> - `id = explicit_id_or(blake3(src)[:12])` — collision-resolved by appending `loc.doc_order` if needed.
> - Cache key inputs include own source, ordered concatenation of prior snippet sources, active `:dep` state, evcxr version, rustc version, target triple.
>
> When `cache.md` lands, revisit "Step 4: classify" and "Step 5: re-eval plan" — the only thing the cache key actually affects here is whether classification is allowed to *skip* re-evaluating an unchanged-but-downstream-of-an-edit snippet.

---

## 1. High-level loop

State machine:

```
            ┌──────────────────────────────────────────────────────┐
            │                                                       ▼
   ┌─── [Idle] ──fs event──> [Debouncing] ──timer expires──> [Querying]
   │                              │                              │
   │                              └─more events─> reset timer    │
   │                                                            (typst query)
   │                                                              │
   │                                            ┌──parse error──┘
   │                                            ▼
   │                                       [QueryFailed]
   │                                       (backoff, log)
   │                                            │
   │                                            └──new fs event──> [Debouncing]
   │                                                              ▲
   │                                                              │
   └── [WriteSidecars] <── [ReEvaluating] <── [Classifying] <─────┘
            │                  │
            │                  └──eval error──> emit error sidecar, continue
            │
            └──> [Idle]
```

Pseudocode (Rust-flavored):

```rust
fn watch(main_typ: &Path, opts: &WatchOpts) -> Result<()> {
    install_signal_handler();                    // §7

    let mut ctx = CommandContext::new()?;        // long-lived
    apply_default_pragmas(&mut ctx, opts);       // :cache <MB>, :allow_static_linking, etc.

    let mut typst_child = if opts.spawn_typst {
        Some(spawn_typst_watch(main_typ)?)       // §2
    } else { None };

    let watcher = notify::recommended_watcher(...)?;
    watcher.watch(main_typ, RecursiveMode::NonRecursive)?;
    // Also watch the directory of main.typ to catch atomic-rename saves (§3).
    watcher.watch(main_typ.parent().unwrap(), RecursiveMode::NonRecursive)?;

    let mut prev_snippets: Vec<Snippet> = Vec::new();   // last successful query result
    let mut query_backoff = Backoff::initial();         // §6
    let mut last_event_at: Option<Instant> = None;
    const DEBOUNCE: Duration = Duration::from_millis(150);   // §3

    loop {
        select! {
            recv(shutdown_rx) -> _ => break,
            recv(watcher_rx, timeout = next_timer(&last_event_at, DEBOUNCE)) -> ev => {
                match ev {
                    Ok(event) if relevant(&event, main_typ) => {
                        last_event_at = Some(Instant::now());
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        // debounce window elapsed — fire one cycle
                        last_event_at = None;
                        match run_one_cycle(&mut ctx, main_typ, &mut prev_snippets, opts) {
                            Ok(()) => query_backoff.reset(),
                            Err(CycleError::QueryFailed(e)) => {
                                log::warn_once(&query_backoff, "typst query failed: {e}");
                                query_backoff.bump();    // delay next cycle, but do not block events
                            }
                            Err(CycleError::Fatal(e)) => return Err(e),
                        }
                    }
                    _ => {}  // ignore unrelated events
                }
            }
        }
    }

    shutdown(&mut ctx, typst_child)?;             // §7
    Ok(())
}

fn run_one_cycle(
    ctx: &mut CommandContext,
    main_typ: &Path,
    prev: &mut Vec<Snippet>,
    opts: &WatchOpts,
) -> Result<(), CycleError> {
    let curr = typst_query_snippets(main_typ).map_err(CycleError::QueryFailed)?;
    let plan = classify(prev, &curr);             // §4
    log_plan(&plan, opts.verbosity);              // §8

    match plan {
        Plan::Noop => {}
        Plan::AppendOnly { new_snippets } => {
            for s in new_snippets {
                eval_and_write(ctx, s)?;
            }
        }
        Plan::TruncateOnly { removed_ids } => {
            for id in removed_ids { drop_sidecars(id); }
        }
        Plan::LeafEdit { snippet } => {
            // Re-eval just this snippet. Its prior items/lets are *already*
            // committed in ctx; a leaf edit by definition introduces no new ones,
            // so we don't need to undo anything. (See §5.)
            eval_and_write(ctx, snippet)?;
        }
        Plan::ResetAndReplay { from_index, snippets_to_eval, removed_ids } => {
            ctx.execute(":clear")?;                // §“Reset operations” below
            for id in removed_ids { drop_sidecars(id); }
            for s in snippets_to_eval {
                eval_and_write(ctx, s)?;
            }
        }
    }

    *prev = curr;
    Ok(())
}
```

### Reset operations available in evcxr

(Confirmed by reading `evcxr/src/command_context.rs` and `evcxr/src/eval_context.rs`.)

| op | what it does | preserves | cost |
|---|---|---|---|
| `:clear` | `eval_context.clear()` → `committed_state = cleared_state(); restart_child_process()` | tmpdir, **rustc artifact cache**, config (linker, opt, sccache, `:cache` size) | spawn one fresh child process; **no dep recompilation** |
| `:restart` | `restart_child_process()` only | committed items/vars structurally, but vars are dropped on the child side | child respawn |
| drop `CommandContext`, build a new one | full reset | only the global `~/.cache/evcxr/` artifact cache | new tmpdir → recompiles dep crates from cache |

**For our purposes, `:clear` is the right tool for the reset case.** It's the cheapest reset that gives us a guaranteed-clean `committed_state`. The rustc artifact cache (`:cache`) lives at `~/.cache/evcxr/` (global, not tied to tmpdir or `EvalContext` instance) — so even a full `CommandContext::new()` benefits from previously compiled deps. `:clear` is strictly faster though because it also keeps the tmpdir's incremental compilation outputs.

#### Order-of-magnitude cost (`D-003` linear-reeval cost model)

For a document with N snippets, edit at position k (0-indexed), reset-and-replay re-evaluates snippets k..N. Per-snippet cost in watch mode with `:cache` warm:

| component | typical cost | notes |
|---|---|---|
| child process restart | 5–30 ms | spawning the runtime helper |
| codegen of one snippet's cdylib | 50–200 ms | dominated by linker on incremental rebuilds |
| dep recompilation | **0 ms** | served from `:cache` after first eval |
| variable transfer to child | <1 ms | only matters for snippets that introduce items/vars |

So for a 10-snippet doc edit at snippet 3, replay of 7 snippets is roughly **0.5–1.5 s** end-to-end. Linker dominates; `mold`/`lld` (`:linker mold`) materially helps. This is why D-003 is acceptable for v0.

---

## 2. Coordination with `typst watch`

**Decision: spawn `typst watch <main.typ>` as a child process** by default. Provide `--no-typst` to opt out (for users who run `typst watch` themselves in another terminal, or use the Typst LSP/Tinymist).

Why a child and not driving `typst compile` ourselves:

1. Typst's incremental compiler is *much* faster than cold `typst compile` for repeated edits, and that machinery is only exposed via `typst watch` today. Re-shelling `typst compile` per edit throws away its incremental state.
2. `typst watch` already watches the filesystem for changes to `read()`/`image()`/`json()`/etc. inputs — i.e. our sidecar files. Verified by inspection of typst-cli source: `typst watch` uses `notify` to watch all files actually read during a previous compile, including those reached via `read("…")`. So when we atomically replace `<id>.txt` or `<id>.png`, typst notices via the OS event and re-renders the affected pages. **mtime polling is not needed and would not help; it's event-driven on both sides.**
3. We don't have to reinvent diagnostic rendering, PDF/SVG export selection, font loading, etc.

How it works in practice:

```
evcxr-typst watch main.typ
├── (parent) notify watcher on main.typ ──> classify ──> evcxr ──> sidecar rewrite
└── (child)  typst watch main.typ        ──> notice .typ change & sidecar changes
                                              ──> re-render PDF
```

There is a benign feedback loop: editing `main.typ` triggers both us and `typst watch`. That's fine — typst will render once with stale sidecars (placeholder boxes if a snippet is brand new), then again after we finish writing. To avoid the user seeing a flicker for trivial edits, we could optionally pass `--input` markers or pause typst, but that's premature; ship the simple version.

> ⚠ **Confirms ARCHITECTURE.md** § "Watch loop": "`typst watch` runs as a child process; it notices our sidecar mtime changes and re-renders incrementally." Updated wording: it's `notify`-based event detection, not mtime polling.

Child process management:

- Spawn with stdout/stderr piped through to ours, prefixed `[typst]` in `-v` mode.
- Set process group / job object so SIGINT to us also reaches the child (§7).
- If the child dies unexpectedly, log it and **do not** auto-restart; that hides bugs. Exit with non-zero unless `--keep-running-without-typst`.

---

## 3. Debounce strategy

**Chosen window: 150 ms.**

Rationale:

- VSCode/Helix/Vim "atomic save" patterns: write `.typ.tmp`, fsync, rename over `.typ`. Often produces one `Create` (or `Modify`) event for the rename plus a `Remove` for the temp, all within a few ms. 150 ms easily covers this.
- Some editors (notably JetBrains, BBEdit) write twice: once for autosave, once for explicit save — but both within ~50–80 ms.
- Typing in the editor at autosave-on-keystroke (rare for `.typ` but possible) shouldn't fire eval per keystroke; 150 ms swallows quick bursts but still feels responsive (humans perceive >200 ms as "laggy").
- Lower bound: 100 ms (we tested mentally — risks splitting a single save into two cycles on slow disks).
- Upper bound: 250 ms (start to feel laggy on save-and-look-at-the-PDF workflow).

Implementation: trailing-edge debounce (reset timer on every event; fire when timer expires with no new events).

File events listened for (via `notify`):

- On the `main.typ` itself: `Modify(Data)`, `Modify(Metadata)` (some platforms), `Remove`, `Create` (atomic rename target).
- On the parent dir: `Create` events whose path == `main.typ` (catches atomic rename where the *file* watcher missed the create, which happens on Linux).

Filter out: events for sibling files we don't care about, our own sidecar writes (we know the cache dir; ignore events under it).

---

## 4. Change classification rules

Inputs: `prev: &[Snippet]`, `curr: &[Snippet]`. A `Snippet` has `{ id, src, doc_order, deps_active_at }`.

We compare the two sequences using `id` first, then `src`:

```rust
enum DiffEntry {
    Same(usize, usize),               // (prev_idx, curr_idx) same id, same src
    SrcChanged(usize, usize),         // same id, src differs
    Added(usize),                     // curr_idx, not in prev
    Removed(usize),                   // prev_idx, not in curr
    IdChanged{prev_idx: usize, curr_idx: usize},  // see below
}

fn classify(prev: &[Snippet], curr: &[Snippet]) -> Plan {
    // Step 1: pair by id (this is what doc_order-tied IDs vs explicit IDs both
    //         resolve to, see §"Snippet ID changed").
    let pairing = pair_by_id_then_position(prev, curr);

    // Step 2: find the FIRST changed index in document order.
    let first_change = pairing.first_change_index();   // None if everything matches

    let Some(k) = first_change else {
        return Plan::Noop;
    };

    // Step 3: special-case "append only" — everything before k matches, and
    // all changes are Added entries at indices >= k, AND prev.len() == k.
    if pairing.is_append_only(k) {
        return Plan::AppendOnly {
            new_snippets: curr[k..].to_vec(),
        };
    }

    // Step 4: special-case "truncation only" — everything before k matches,
    // all changes are Removed entries at prev indices >= k, AND curr.len() == k.
    if pairing.is_truncate_only(k) {
        return Plan::TruncateOnly {
            removed_ids: pairing.removed_after(k),
        };
    }

    // Step 5: leaf-edit — the change at k is a SrcChanged on a snippet that:
    //   - is the LAST snippet in both prev and curr (k == prev.len()-1 == curr.len()-1)
    //   - the modified version is a "leaf" (see §5)
    //   - no other diffs exist
    //   - deps_active_at unchanged (a :dep change is never a leaf)
    if pairing.only_change_is_at(k)
        && k == prev.len().saturating_sub(1)
        && k == curr.len().saturating_sub(1)
        && is_leaf(&curr[k])
        && curr[k].deps_active_at == prev[k].deps_active_at
    {
        return Plan::LeafEdit { snippet: curr[k].clone() };
    }

    // Step 6: catch-all — reset and replay from k.
    Plan::ResetAndReplay {
        from_index: k,
        snippets_to_eval: curr[k..].to_vec(),
        removed_ids: pairing.removed_ids_overall(),
    }
}
```

### Cases covered

| case | classification | result |
|---|---|---|
| Snippet added at end | `AppendOnly` | eval new snippet(s) |
| Snippet removed at end | `TruncateOnly` | drop sidecars |
| Last snippet modified, leaf | `LeafEdit` | re-eval just it |
| Last snippet modified, non-leaf | `ResetAndReplay { k=last }` | reset, eval one snippet (because `from_index..end` is one snippet) |
| Middle snippet modified | `ResetAndReplay { k=middle }` | reset, eval k..N |
| Snippet inserted in middle | `ResetAndReplay { k=middle }` | reset, eval k..N |
| Snippet removed from middle | `ResetAndReplay { k=middle }` | reset, eval k..N (drop removed sidecars) |
| Snippet reordered (no content change) | both old position and new position diff → `ResetAndReplay { k=min(old,new) }` | reset, eval from earliest moved-from index |
| `:dep` line changed in any snippet | non-leaf by definition; reset and replay from that snippet | |
| Nothing changed (cosmetic .typ edit) | `Noop` | log only |

### Snippet reordered

If src and id are unchanged but `doc_order` changes, a content-hash-based id will *match* across positions (good — we know "same code, different place"). The pairing step sees: prev had `s` at index 4, curr has `s` at index 7. Indices 4..7 in curr are different snippets. So `first_change` is index 4 → `ResetAndReplay` from 4. Correct: items defined by `s` now appear later, so anything that *was* between them and used to see `s`'s items no longer does. We cannot do better than reset-and-replay here without a snapshot mechanism.

### Snippet ID changed (user added explicit `id:`)

User edits `#rust(\`\`\` … \`\`\`)` to `#rust(id: "foo", \`\`\` … \`\`\`)`. The src is unchanged but the id flips from `blake3(src)[:12]` to `"foo"`. Naive pairing-by-id would see `Removed(old_id)` + `Added("foo")`, triggering reset.

**Resolution:** in the pairing step, also build a secondary index by `(src, doc_order)`. If a `Removed`/`Added` pair has matching `src` and adjacent `doc_order`, treat them as `IdRenamed` — same snippet, just a different name. The cache entry can be moved (rename `<old_id>.txt` → `<new_id>.txt`) and no eval is needed. Surface this in `-v` log.

Edge case: user changes id *and* edits src in the same save. Pairing falls back to `SrcChanged` if the position matches, else treat as remove+add (reset).

### Why `from_index` and not "first snippet that USES the changed item"

That requires Rust-level static analysis (does snippet 7 reference `Foo` defined in snippet 3?), which we explicitly opted out of in `D-003`. Linear replay from k is pessimistic but correct.

---

## 5. What "leaf" precisely means

A snippet is a **leaf** iff, after parsing it as Rust under evcxr's wrapping rules, it introduces nothing into `committed_state` that subsequent snippets can observe.

Operationally — and at the Rust syntax level — a snippet is a leaf iff **all of the following hold**:

1. It contains **no top-level items** of these kinds (top-level == direct children of the snippet body, not nested inside another item or block):
   - `fn`
   - `struct`, `enum`, `union`
   - `trait`
   - `impl` (inherent or trait)
   - `mod` (both `mod foo;` and `mod foo { … }`)
   - `use`, `extern crate`
   - `type` (alias)
   - `const`, `static`
   - `macro_rules!`

2. It contains **no top-level `let` statement**. A top-level `let` is one that appears as a direct statement of the snippet body. (`let` inside `{ … }` blocks, closures, or expression position does **not** disqualify, because evcxr only persists *top-level* `let` bindings — nested ones don't escape their lexical scope.)

3. It contains **no `:dep`, `:vars`, `:clear`, or any other `:command`** line. A `:dep` is a non-leaf because it changes `external_deps`, which all subsequent snippets see.

4. It does not assign to (or shadow) a previously-declared persistent variable. (e.g. if snippet 3 declared `let x = 1;` and our current snippet has top-level `x = 2;` — that would be caught by rule 2 if it's a `let`-shadow, but a bare assignment to an existing var also mutates committed state visible to later snippets. *Strictly* speaking, evcxr persists `let` bindings, and a bare `x = 2;` assignment doesn't redefine anything — it just runs. But to be safe, conservatively classify any snippet containing a top-level identifier on the LHS of `=` matching a known variable as non-leaf. This is rare; the conservative read costs us nothing.)

If all of (1)-(4) hold, the snippet is a leaf.

### Concrete examples

| snippet body | leaf? | why |
|---|---|---|
| `println!("hi");` | yes | only an expression statement |
| `let x = 5; println!("{x}");` | **no** | top-level `let` |
| `{ let x = 5; println!("{x}"); }` | yes | `let` inside a block expression — does not escape |
| `(0..10).for_each(\|i\| { let y = i*2; println!("{y}"); })` | yes | `let` is inside the closure body |
| `fn helper() {} helper();` | **no** | top-level `fn` |
| `for i in 0..3 { let j = i; println!("{j}"); }` | yes | `let` inside `for` body |
| `:dep regex = "1"` then `Regex::new(...)?;` | **no** | `:dep` |
| `dbg!(my_existing_var);` | yes | reads but doesn't redefine |
| `my_existing_var = 7;` | **no** (conservatively) | mutates a persistent var; rule (4) |

### Implementation note

Detection is done by parsing the snippet src with `syn::parse_file` (or `syn::Block` if we wrap it) and walking only top-level items. We do **not** semantic-analyze; pure syntax.

If `syn` parsing fails (snippet has a syntax error), classify as non-leaf — we'll let evcxr produce the error properly. Erring on the side of `ResetAndReplay` is safe; erring the other way (treating a non-leaf as leaf) would silently corrupt state.

---

## 6. Transient parse errors

`typst query` fails when the user is mid-edit and the doc is syntactically broken (unmatched brace, mid-typed `#` directive, etc.). This is the common case during typing.

Strategy:

- On query failure, do **not** mutate `prev_snippets` and do **not** touch the eval context.
- Log once per "error streak" at INFO. Do not log identical errors repeatedly.
- Apply exponential backoff for *automatic retries*: 250 ms → 500 ms → 1 s → 2 s, capped at 2 s. The backoff only delays a retry *if no new fs event arrives*; a new event resets the backoff and re-debounces normally. So in practice, the user will type, fix, save, and we'll evaluate immediately.
- After 5 consecutive query failures with no successful query in between, surface a sticky status line: `[evcxr-typst] typst query failing — see -v for details`.
- `typst watch` will independently render its own diagnostic into the PDF. We don't double-report.

```rust
struct Backoff { current: Duration, n_failures: u32 }
impl Backoff {
    fn initial() -> Self { Self { current: ms(250), n_failures: 0 } }
    fn bump(&mut self) { self.n_failures += 1; self.current = (self.current * 2).min(ms(2000)); }
    fn reset(&mut self) { *self = Self::initial(); }
}
```

---

## 7. Shutdown

Signals: handle SIGINT, SIGTERM, SIGHUP (Unix); Ctrl-C event (Windows).

```rust
fn install_signal_handler() -> Receiver<()> {
    let (tx, rx) = bounded(1);
    ctrlc::set_handler(move || { let _ = tx.try_send(()); }).unwrap();
    rx
}

fn shutdown(ctx: &mut CommandContext, typst_child: Option<TypstChild>) -> Result<()> {
    // 1. Stop typst child first — sending SIGTERM to its process group.
    if let Some(mut child) = typst_child {
        child.terminate(Duration::from_secs(2))?;     // SIGTERM, then SIGKILL after grace
    }
    // 2. Terminate evcxr's child process. Drop of CommandContext does this:
    //    eval_context's drop order is documented as "child_process before _tmpdir",
    //    so the runtime helper exits cleanly and the tmpdir is removed.
    drop(std::mem::replace(ctx, CommandContext::dummy()));
    // 3. Any in-flight sidecar writes? We never start a write we don't intend
    //    to atomically finish. See atomic-write below.
    Ok(())
}
```

### Atomic sidecar writes

Never write into `<id>.txt` directly. Always:

1. Write to `.evcxr-typst-cache/.tmp.<id>.<rand>.txt`.
2. `fsync` the temp file.
3. `rename` over `<id>.txt` (atomic on POSIX; `MOVEFILE_REPLACE_EXISTING` on Windows).

If we Ctrl-C between steps 1 and 3, the worst-case state on disk is a leftover `.tmp.…` file; the previous `<id>.txt` is intact. The Typst child sees no event for the temp file (we ignore events under the cache dir anyway).

Cleanup of stale `.tmp.*` files happens on next startup.

### Child cleanup invariant

Both children (typst-watch + evcxr's runtime helper) MUST be reaped. We rely on:

- `ctrlc` crate to catch Ctrl-C and *not* exit the process; we exit cleanly through `shutdown()`.
- `Drop` for `EvalContext` documented to terminate the child before dropping the tmpdir.
- For typst-watch, we wrap its `Child` in a struct whose `Drop` sends SIGTERM with a 2 s grace, then SIGKILL.

If we fall over via `panic!`, the OS reaps zombies but the tmpdir may leak. Acceptable v0.

---

## 8. Logging

Default verbosity (no flag): one line per cycle.

```
[14:02:03] reeval: 3 snippets (leaf-edit on snippet 7) in 412ms
[14:02:18] reeval: 5 snippets (reset+replay from snippet 4) in 2.1s
[14:03:01] noop:   no snippet changes (cosmetic edit)
```

`-v` adds:

- Per-snippet eval start/end with timing.
- Classification rationale ("snippet 7 is leaf because: no items, no top-level `let`").
- Sidecar paths written.
- `[typst]` prefixed forwarded child output.

`-vv` adds:

- Full diff of `prev` vs `curr` snippet lists.
- evcxr's stderr/stdout passed through.
- `:cache` stats per cycle.

Where: stdout for the human-watchable line; stderr for diagnostics. No log file by default. `--log-file <path>` flag enables structured JSON logging to file (one line per event), useful for "why was that re-eval slow" forensics.

---

## 9. Open questions

1. **Should we deduplicate sidecar writes when the new bytes equal the old?** **Resolved (D-016): yes, skip the rename when bytes are unchanged.** A leaf-edit that only touches whitespace inside the snippet re-evaluates and produces byte-identical sidecars; in interactive watch sessions this is a common case (autosave on a no-op edit) and an extra `typst watch` re-render per keystroke is felt by the user. Concretely: in the id-addressed view materialization step (`cache.md` § "Atomic-write strategy"), before the final `rename` over `<id>.<ext>`, `stat` the existing target; if its size and bytes match the staged file, drop the staged file and skip the rename. The CAS write itself (under `cas/<XX>/<full-key>/`) is **always** performed (its presence is what marks a cache key as having been computed); only the materialization to the live id-addressed view is conditional. This preserves cache.md's atomic-rename invariant — no torn writes, no half-replaced files — and adds at most one `stat` + one streaming byte-compare per snippet per cycle, dwarfed by evcxr execution cost.

2. **How do we handle multi-`.typ` projects (`#include`, `#import` of local files)?** **Resolved (D-018; see [`multi-file.md`](./multi-file.md))**. v0 supports single-entry-file projects with auto-discovered imports. After every successful `typst query` cycle, the CLI re-runs discovery (BFS over the entry file following local `#import`/`#include` via `typst-syntax` parsing, with an `evcxr-typst.toml` manifest as an opt-in override). The watch set is updated by diffing prev/curr discovered file sets and calling `watcher.watch` / `watcher.unwatch` on the delta — see `multi-file.md` § 7. Snippets across files are linearized into a single global order `(file_seq, doc_order_within_file)` that feeds the existing classification and `prior_chain_hash` machinery unchanged; an edit in any member file flows through the standard `prev`-vs-`curr` diff in § 4. The cache directory is rooted at the entry file's parent (the *workspace*); the CAS is shared across entry files in the same workspace, the `index.json`/materialized view is per entry. Multi-entry-file projects are deferred to v1; v0 users with multiple entries run two `evcxr-typst watch` invocations side-by-side, each with its own watch loop. **Open follow-up:** verify whether `typst query`'s `location()` exposes the source file of an imported-file metadata element — if yes, we can drop the AST-parsing step in discovery (Q1 in `multi-file.md` § 9).

3. *(Bonus.)* **Do we want a "force reset" hotkey?** During development of a `:dep`-heavy doc, it's plausible that the user wants to manually nuke and replay even when the doc didn't textually change (e.g. they edited a local crate that we `:dep`'d via path). Could be a SIGUSR1 or stdin command (`r<enter>`). Defer until users ask.

---

## Appendix: contradictions with other docs

- **Confirms** `ARCHITECTURE.md` § "Watch loop" on classification categories and reset policy.
- **Refines** the same section's "it notices our sidecar mtime changes" → it's notify-based event detection, not mtime polling. This is a clarification, not a contradiction.
- **Implements** `D-003` (linear re-eval). No deviation.
- **Depends on** `T-D04` outputs for the cache key formula — placeholder assumptions noted at top of file. Reconcile once `cache.md` and `snippet-identity.md` land.
