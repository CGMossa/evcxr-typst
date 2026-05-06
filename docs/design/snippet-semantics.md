# Snippet semantics & dependency model

How Rust constructs in Typst snippets compose across one long-running
`evcxr::CommandContext`. This is mostly a description of behavior evcxr
already gives us; the value-add is mapping that to per-snippet UX and
calling out the gaps.

Reference reads (links are to local checkout):

- `evcxr/COMMON.md` — variable persistence, `:dep`, the references limitation.
- `evcxr/evcxr/src/eval_context.rs` — `ContextState` (lines ~1198–1237),
  item commit logic (~1854–1943), `add_dep` (~1386–1402),
  `items_code()` / `analysis_code()` (~1517–1583).
- `evcxr/evcxr/src/use_trees.rs` — how `use` is exploded to per-name
  `Import::{Named, Unnamed}` so later snippets can shadow individual names.
- `evcxr/evcxr/src/code_block.rs` — segmentation of a single evaluation
  (items vs statements vs final expression) and `OriginalUserCode` vs
  `OtherUserCode` (committed-but-still-emitted) distinction.

Mental model: every snippet evaluation is wrapped as a fresh `cdylib`
crate, regenerated from the committed `ContextState`. The crate file lives
at `state.config.crate_dir()/src/lib.rs` (see `module.rs` ~254). On success
the new state is committed; on failure the old state is restored. So
"persistence" really means "this construct is reattached at the top of the
next generated crate."

---

## Construct matrix

For each Rust top-level form: does evcxr persist it across snippets, what
gotchas exist, and what UX surface (if any) belongs in the Typst package.
Behavior column is sourced from `eval_context.rs::ContextState::apply`
(lines ~1854–1943) unless noted.

| Construct | Persists? | How / where stored | Gotchas | Recommended UX |
|---|---|---|---|---|
| `fn foo() {…}` | yes | `items_by_name["foo"]` | redefining replaces (last write wins, see "Item shadowing" below) | none — just works in `#rust(```rust …```) ` |
| `struct S {…}` / `enum E {…}` / `union U {…}` | yes | `items_by_name["S"]` etc. | replacing the type does **not** invalidate already-stored `let` bindings of the old type — they will fail to recompile on the next snippet that touches them | document; see "Redefinition" rules below |
| `trait T {…}` | yes | `items_by_name["T"]` | same redefinition note | document |
| `impl … for …` (inherent or trait) | yes | attached to the *previous* named item via `previous_item_name` (see eval_context.rs ~1929–1938); orphan impls go into `unnamed_items` | impls accumulate; you cannot remove an old impl without `:clear`. A snippet that defines `impl Foo` and is later edited to `impl Bar` leaves the `Foo` impl in state until the context is reset | document; the watch-loop "non-leaf modified" path (D-003) already does the right thing |
| `mod foo { … }` (inline) | yes | `items_by_name["foo"]` | path resolution inside `mod foo` is normal (it's just nested code) | none |
| `mod foo;` (file-based) | **technically yes**, semantics broken | stored as an item but the file is searched relative to `crate_dir()/src/`, *not* the user's `.typ` file | nearly always wrong: paths break, edits to the external file aren't tracked, snippet identity hash misses changes | error nicely — detect bare `mod NAME;` (no body) at the top level of a snippet and reject with a hint pointing to inline `mod` or `:dep path = "…"` |
| `use a::b;` | yes, per-name | use-tree exploded by `use_trees.rs` into `items_by_name["b"]` (named) or `unnamed_items` (`*`, `_`, `as _`) | per-name shadowing means redefining the imported name in a later snippet replaces the import cleanly. Glob imports (`use foo::*;`) cannot be selectively undone — they live in `unnamed_items` forever | none — natural |
| `extern crate foo;` | yes | `extern_crate_stmts["foo"]`; **also** auto-added to `external_deps` with `version = "*"` if not already present (eval_context.rs ~1856–1869) | rarely needed in Rust 2018+; the implicit `:dep` is a footgun for an offline build. Prefer `:dep` explicitly. | document; recommend `:dep` over `extern crate` |
| `const NAME: T = …;` / `static NAME: T = …;` | yes | `items_by_name["NAME"]` | stored *as an item*, not a variable — `:vars` won't list it. Type must be fully written; no `_` inference | document |
| `let x = …;` | yes (the value) | runtime: variable copied into a hidden `EvcxrVariableStore`; `stored_variable_states["x"]` | **must be `'static`** — see "Variable references" rules. Cannot be `mut`-borrowed across snippets. Moves drop the binding. | the package should suggest `Box::leak` / scoped blocks in error rendering when the user hits the lifetime wall |
| `let x: T = …;` (typed) | yes | as above; explicit type avoids the "store-then-reinfer" hack | use this when type inference fails on store; covered in COMMON.md "References" | document in package help |
| `macro_rules! m {…}` | yes | `items_by_name["m"]` (special-cased, eval_context.rs ~1871–1879) | local-only definition works fine across snippets | none |
| `#[macro_use] extern crate foo;` | **no** | COMMON.md "Limitations": "There is currently no way to import macros from external crates" | macros from external crates are simply not available. Path-style invocation (`foo::bar!()`) works for macros 2.0 / `pub use` re-exports of `macro_rules!` in some crates, but classic `#[macro_use]` does not | error nicely on `#[macro_use]` at snippet top level; suggest path-style invocation as workaround |
| Derive macros (`#[derive(Serialize)]`) | yes (the attribute is part of the item's source, replayed verbatim each compile) | the attribute lives on the `struct`/`enum` item which is in `items_by_name` | requires the `:dep` to be active *at the time the snippet is evaluated*. If snippet 3 declares the struct and snippet 1 retroactively gets the dep, only re-eval from 1 onward picks it up — the watch-loop already handles this (D-003). | none |
| Attribute macros (`#[tokio::main]`, etc.) on items | usually wrong shape | evcxr generates the `fn` wrapper itself; user-attached `#[tokio::main]` on a snippet-level `fn` fights that | use `:dep tokio = { features = ["full"] }` plus bare `.await`; evcxr auto-wraps in a Tokio runtime when it sees `await` (COMMON.md "async-await") | document the async pattern; reject `#[tokio::main]` with a hint |
| Inner attributes (`#![feature(...)]`) | yes | `state.attributes` keyed by attribute name (eval_context.rs `attributes_code`) | all snippets share the same attribute set; one snippet enabling `#![feature(…)]` enables it for all subsequent compiles | document |
| `type Alias = …;` | yes | `items_by_name["Alias"]` | none beyond redefinition | none |
| Free expression / `42` / final-expression printing | n/a | rendered via `:fmt` formatter | only the *last* expression in a snippet prints automatically (COMMON.md, code_block.rs final-expression handling) | document; package's `#rust-out` vs `#rust-display` already separates this |

> ⚠ Contradicts ARCHITECTURE.md § "Composition across snippets": the table
> there says `Macros … Local macro_rules!: yes`. Confirmed accurate.
> No contradiction — flagged for completeness.

---

## Rules

### 1. Item shadowing / redefinition

evcxr stores items in a `HashMap<String, CodeBlock>` keyed by item name
(eval_context.rs ~1925–1938). The semantics that fall out:

- **Same name, same kind**: snippet 5 redefines `struct Foo { x: i32 }` →
  the entry in `items_by_name["Foo"]` is overwritten; subsequent snippets
  see the new definition. The old definition is gone from state.
- **Same name, different kind**: snippet 5 changes `Foo` from a `fn` to a
  `struct` → also overwritten. Rust will accept it on the new compile.
- **Existing `let` bindings of the old type**: a `let foo: Foo = …;`
  committed before the redefinition is stored *by serialized type name*
  (`stored_variable_states["foo"].type_name == "Foo"`). After redefinition,
  the next compile that mentions `foo` will fail because the stored bytes
  don't match the new layout. This is unavoidable without snapshotting.
- **`impl` blocks** are appended to the previous named item's `CodeBlock`
  (the `previous_item_name` mechanism). Redefining `Foo` does **not**
  drop accumulated `impl Foo` blocks — they go with the last item the
  parser saw. This means an edit can leave dangling impls that reference
  fields that no longer exist.

> Rule for our writers: **the watch loop (D-003) already invalidates the
> chain on any non-end edit.** No additional work needed at snippet level.
> The caching layer (T-D04) needs to know that "redefinition is destructive
> to downstream `let`s" so its cache key includes upstream item-source.

### 2. Variable-reference limitation

From COMMON.md "References" (verbatim restriction):

> Variables that persist cannot reference other variables.

The mechanism: persisted vars are moved into a hidden store and reloaded
into the next compile by name. The store function signature is roughly
`fn evcxr_variable_store<T: 'static>(_: T) {}` (eval_context.rs ~1523).
That `'static` bound is the wall.

```rust
// Snippet A — fails:
let all = vec![10, 20, 30];
let some = &all[..2];   // borrow of `all`, not 'static
```

Workarounds we'll teach:

```rust
// 1. Scope the borrow inside the snippet (preferred for one-shot use):
let all = vec![10, 20, 30];
{
    let some = &all[..2];
    println!("{some:?}");
}
// `all` persists; `some` does not.
```

```rust
// 2. Leak to 'static (when the borrow itself must persist):
let all: &'static Vec<i32> = Box::leak(Box::new(vec![10, 20, 30]));
let some: &'static [i32] = &all[..2];
```

```rust
// 3. Own each persisted binding:
let all = vec![10, 20, 30];
let some: Vec<i32> = all[..2].to_vec();   // owned copy
```

Package surfacing: when a snippet fails with the characteristic borrow-
checker error in the generated `pack_variable` line, the error renderer
(T-D06) should detect it and inject the three workaround patterns into the
rendered error box.

### 3. `:dep` semantics

`:dep name = "1.0"` calls `state.add_dep(name, config)` (command_context.rs
~794–815, eval_context.rs ~1386–1402). Behavior:

- **When evaluated**: at the time the snippet containing the `:dep`
  command is executed, in document order. Cargo metadata is fetched and
  validated immediately (`cargo_metadata::validate_dep`).
- **Persistence**: `external_deps` is a `HashMap<String, ExternalCrate>`,
  keyed by crate name. Last write wins.
- **Re-`:dep` with same config**: short-circuits; no re-validation
  (eval_context.rs 1388–1392).
- **Re-`:dep` with different config**: silently overwrites. Two snippets
  doing `:dep serde = "1.0.150"` and `:dep serde = "1.0.200"` later in the
  document → the later one wins for *all subsequent compiles*. There is no
  "version per snippet."

> Rule for the package: `dep()` calls in Typst should map 1:1 to a single
> `:dep` line emitted to evcxr at the corresponding snippet's position.
> If two `dep()` calls disagree on version, we **error at the CLI level
> before driving evcxr**, not silently let the later overwrite. evcxr's
> behavior here is too quiet to be useful for a multi-page document.
>
> Open question: should `dep()` calls be allowed *anywhere* in the
> document and pre-collected, or only at the top? T-D03 decision.

`extern crate foo;` inside a snippet *also* auto-adds `foo = "*"` to
`external_deps` if no entry exists. This is a footgun: it can trigger a
network fetch on a document that looked like it had no deps. Recommend the
package error on bare `extern crate` and direct users to `:dep`.

### 4. `mod foo;` (file-based modules)

Mechanically it persists like any other item — but the path resolution is
relative to `state.config.crate_dir()/src/`, the synthesized cdylib's `src/`
directory (module.rs ~250–254 sets `path = "src/lib.rs"`; rustc resolves
`mod foo;` relative to that). The user's `.typ` file's directory is
nowhere in the picture.

Implications:

- The user would have to know the evcxr crate dir (a temp path) and drop
  files there — non-starter.
- Even if we synced files into `crate_dir()/src/` from the document
  directory, our snippet-identity / cache key (T-D04) hashes only the
  `.typ` source. External `.rs` file edits would silently not invalidate.
- File-mode requires `mod` to be a **bare** declaration with no body. We
  can detect this syntactically (top-level `Item::Module` whose
  `item_list()` is `None`).

**Recommendation: error nicely, do not support.** Detect bare
`mod NAME;` at the top level of a user snippet and emit:

> `mod foo;` (file-based modules) isn't supported in evcxr-typst snippets.
> Use an inline module (`mod foo { … }`) or pull a separate crate via
> `:dep foo = { path = "./foo" }`.

Inline `mod foo { … }` works fine and should be the canonical
"organize a chunk of code" pattern in our gallery.

---

## Examples cross-reference (slated for T-D02 gallery)

These three concrete scenarios must appear in the example gallery and
each exercises one of the rules above.

1. **`struct-across-snippets.typ`** — Demonstrates §1 (item persistence
   without redefinition).

   ```rust
   // Snippet A, in the introduction:
   #[derive(Debug)]
   pub struct Measurement { pub label: String, pub value: f64 }

   impl Measurement {
       pub fn new(label: &str, value: f64) -> Self {
           Self { label: label.to_string(), value }
       }
   }
   ```

   ```rust
   // Snippet B, three pages later — uses Measurement directly:
   let m = Measurement::new("g", 9.81);
   println!("{m:?}");
   ```

   Demonstrates: `struct` and `impl` both live in `items_by_name` and
   are reattached to every subsequent compile. No `:dep` needed.

2. **`module-across-snippets.typ`** — Demonstrates inline-`mod`
   composition (§4: inline-mod is the supported pattern).

   ```rust
   // Snippet A:
   pub mod stats {
       pub fn mean(xs: &[f64]) -> f64 {
           xs.iter().sum::<f64>() / xs.len() as f64
       }
       pub fn stddev(xs: &[f64]) -> f64 {
           let m = mean(xs);
           (xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / xs.len() as f64).sqrt()
       }
   }
   ```

   ```rust
   // Snippet B:
   use stats::*;
   let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
   println!("mean={}, stddev={:.3}", mean(&xs), stddev(&xs));
   ```

   Demonstrates: inline `mod` persists, `use stats::*` is exploded into
   `unnamed_items` and reattached. `mean` and `stddev` resolve cleanly.

3. **`var-reference-limit.typ`** — Demonstrates §2 (the references
   limitation) and the three workarounds. This example is partly a
   teaching tool; it should *show the failing version commented out*
   alongside the working alternatives.

   ```rust
   // The version that fails to persist:
   //   let all = vec![10, 20, 30];
   //   let first_two = &all[..2];   // borrow → not 'static → rejected

   // Workaround 1: scope the borrow.
   let all = vec![10, 20, 30];
   { let first_two = &all[..2]; println!("{first_two:?}"); }
   ```

   The rendered Typst page shows the error box from the failing snippet
   (driven by T-D06's renderer) plus the working snippets, side by side.

Other gallery scenarios (hello-world, plot/image, `:dep`, async, error
case, multi-snippet report) are listed in BACKLOG.md T-D02 and don't
need to be re-specified here.

---

## Gotchas — "looks like it should work but doesn't"

Each item: behavior, why, recommendation.

### G1. `mod foo;` looks like a normal Rust pattern

- **Behavior**: compiles only if `foo.rs` happens to exist in evcxr's
  ephemeral `src/`; otherwise a confusing rustc "file not found" error.
- **Why**: § "mod foo; (file-based modules)" above.
- **Recommendation**: error nicely at the CLI / package layer before
  passing to evcxr. Detect `Item::Module` with no `item_list()`.

### G2. `#[macro_use] extern crate foo;`

- **Behavior**: silently does nothing useful; macros from `foo` aren't
  available. May *also* trigger the `extern crate` auto-`:dep`.
- **Why**: COMMON.md Limitations. evcxr doesn't propagate `#[macro_use]`
  through its synthesized crate boundary.
- **Recommendation**: error nicely at the package layer. Suggest
  path-style invocation (`foo::bar!()`) where the macro is exported
  that way, otherwise document as unsupported.

### G3. `#[tokio::main]` on a snippet-level `async fn main`

- **Behavior**: rejected by evcxr's wrapping (it generates its own entry
  point).
- **Why**: evcxr already detects `await` and inserts a Tokio runtime.
- **Recommendation**: document. Show the canonical pattern:
  `:dep tokio = { version = "1", features = ["full"] }` then write bare
  `something.await` at the top level of a later snippet.

### G4. Borrows across snippets

- **Behavior**: lifetime errors that reference generated code (the
  `pack_variable` segment).
- **Why**: § "Variable-reference limitation."
- **Recommendation**: detect in the error renderer (T-D06) and inject
  the three documented workarounds inline.

### G5. Two snippets `:dep` the same crate, different versions

- **Behavior**: in evcxr, last `:dep` wins silently. In a long document
  where two sections were authored independently, this is a footgun —
  the earlier section may suddenly behave differently.
- **Why**: `add_dep` overwrites in `external_deps`.
- **Recommendation**: the CLI pre-collects all `dep()` calls during the
  `typst query` pass and **errors before driving evcxr** if two calls
  disagree. The error points at both snippet locations.

### G6. `extern crate foo;` (no `#[macro_use]`)

- **Behavior**: auto-adds a wildcard `:dep foo = "*"`. May fetch the
  network in a previously-offline document.
- **Why**: eval_context.rs ~1856–1869.
- **Recommendation**: warn (not error) at the package layer; suggest
  `:dep foo = "X.Y"` for reproducibility. Hard-error in `--offline` mode.

### G7. Editing a `let x = …` redefines x's type but downstream uses it

- **Behavior**: `stored_variable_states["x"]` carries the *old* type name
  and serialized bytes. The next compile fails with a confusing type
  mismatch deep in generated code.
- **Why**: variables are stored byte-wise, type by name. A type-changed
  redefinition can't be deserialized into the new layout.
- **Recommendation**: the watch-loop's "non-leaf modified" path (D-003)
  already resets the context and re-evaluates from the change point, so
  in practice users only see this if they manually sequence edits in
  end-only mode. Document; no special detection needed.

### G8. Glob `use` (`use foo::*;`) can't be undone

- **Behavior**: lives in `unnamed_items` forever within a session; later
  snippets can't selectively shadow names from it.
- **Why**: `Import::Unnamed` doesn't go through `items_by_name`.
- **Recommendation**: document. Recommend named imports in shared
  documents for predictability. (For watch-mode this is fine because
  any edit upstream of the glob resets the context.)

### G9. Identical Rust source in two snippets collides on default ID

- **Behavior**: two `println!("hi")` snippets produce the same `blake3`
  ID; second overwrites the first's sidecar.
- **Why**: D-005 default ID = content hash, no doc_order mixed in.
- **Recommendation**: out of scope for this doc. Flagged for T-D04.

### G10. Snippet that only mutates a previously-`let`-bound var

- **Behavior**: works fine if the var is `let mut`. If not, fails with
  the obvious "cannot mutate" error. But if the var is moved (e.g. a
  `Vec` consumed by `.into_iter().collect::<…>()` and rebound), the
  *next* snippet sees a new binding — semantically `let` rather than
  mutation.
- **Why**: every snippet is a fresh top-level scope; rebinding is
  shadowing, not assignment.
- **Recommendation**: document briefly. This isn't a bug, but it
  surprises users coming from notebook environments where mutation is
  common.

---

## Open questions

Flagged for the orchestrator to resolve later, not blocking T-D01:

1. **Should `dep()` be lifted to a document-level construct, separate
   from inline `#rust(…)` snippets?** The CLI has to pre-collect deps
   anyway (G5). The Typst package could expose `#evcxr.dep("serde",
   "1")` at the top of the document and reject mid-document `:dep`.
   Decision deferred to T-D03.

2. **Do we want to support `:dep path = …` with paths *relative to
   the `.typ` file*?** evcxr resolves paths relative to its own
   working directory, which is the user's CWD when the CLI was
   started. Not the same thing. Probably we want to canonicalize at
   the CLI boundary. Decision deferred to T-D03 / T-I03.

3. **Cross-snippet `pub` visibility**: in the synthesized crate, every
   item is at the crate root. `pub` is effectively flat. If a writer
   declares `pub mod private { fn helper() {…} }` and then
   `private::helper()` from a later snippet, it fails (helper is not
   `pub`). Should we document this, or silently rewrite to encourage
   `pub fn` everywhere? Probably document; user-controlled visibility
   is part of the Rust surface.

4. **`:cache` interaction**: confirmed enabled by default (D-003
   relies on it), but is the cache shared across `.typ` files in the
   same project, or per-document? evcxr's cache is per-`CommandContext`
   on disk; so two CLIs running in parallel against different docs
   would share the on-disk artifact cache. Probably fine, flag for
   T-D04.
