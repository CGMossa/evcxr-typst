# Schema versioning policy (T-D10)

How the four `v` fields and two semver streams in this project evolve, what
older/newer pieces do when they meet, and how a Typst package release announces
"you need a newer CLI."

This is the **canonical reference** for versioning. Schema docs
(`package-api.md` § 5, `errors.md` § 2, `cache.md` § "Cache layout") link here
rather than duplicating the rules.

---

## 1. Inventory of versioned things

| # | Versioned thing | Where it lives | Current version | Who controls it | Bump rule |
|---|---|---|---|---|---|
| i | `<evcxr-snippet>.v` | Typst metadata emitted by package's `rust*()` functions | `1` | Typst package source (`packages/evcxr/lib.typ`) | **Major-breaking-only.** Bump on rename / remove / type-change of any field. Adding a new optional field, a new `kind` enum variant, or a new key inside `options` does **not** bump `v`. |
| ii | `<evcxr-dep>.v` | Typst metadata emitted by `dep()` | `1` | Typst package source | **Major-breaking-only.** Same rule as (i): renaming `spec`, removing `features`, or changing `loc.doc_order` to a string would bump. Adding `git`/`path`/`package` sub-fields does not. |
| iii | `<id>.error.json.v` | CLI-written sidecar | `1` | CLI source (`crates/evcxr-typst/src/sidecar.rs`) | **Major-breaking-only.** Bump on schema-incompatible changes to `errors[]` shape. New `phase` enum variants, new optional sub-objects (e.g. a future `helps[].source_url`), or a new `severity` value do not bump. |
| iv | On-disk cache layout (`.evcxr-typst-cache/v1/`) | Workspace directory | `v1` | CLI source | **Major-breaking-only.** Bump on a change that would make the existing CAS layout, `index.json` shape, or cache-key formula incorrect to consume. Adding a new sidecar MIME type or a new `meta.json` field does not. |
| v | Typst package version | `packages/evcxr/typst.toml` | `0.1.0` | Typst package source | **Semver.** User-visible API changes (function rename, kwarg rename, default-value change) bump minor pre-1.0 / major post-1.0. Internal-only refactors bump patch. The package version **does not need to track** any of (i)–(iii); those are independent. |
| vi | `evcxr-typst` CLI version | `crates/evcxr-typst/Cargo.toml` | `0.1.0` | CLI source | **Semver.** CLI-flag changes, sidecar-emission changes, exit-code changes are minor pre-1.0. |

The four `v` fields are **independent** of the two semver streams. A patch
release of the CLI may not change any `v`; a major release may change one
without touching another.

---

## 2. Bump rules — concrete examples

### (i) `<evcxr-snippet>.v` — currently `1`

| Change | Bumps `v`? | Why |
|---|---|---|
| Add `options.theme: "dark"` | no | `options` is documented as a forward-compatible bag (`package-api.md` § 5.1). Older CLIs ignore unknown keys. |
| Add a new `kind: "rust-bench"` | no | Older CLIs can fall back to treating it as `rust-hidden` (no rendering) or surface "unknown kind" — both are graceful. New variant is purely additive. |
| Rename `src` → `source` | **yes (`v` → `2`)** | Older CLIs would fail to find the field. |
| Change `loc.doc_order` from int to `{file, order}` | **yes** | Type change. |
| Drop `deps` (move into `options.deps`) | **yes** | Existing readers would silently miss explicit deps. |

### (ii) `<evcxr-dep>.v` — currently `1`

| Change | Bumps `v`? |
|---|---|
| Add `default-features: bool` | no |
| Add `package: string` rename field | no |
| Rename `spec` → `crate` | **yes** |
| Make `features` accept either array or comma-string | **yes** (type-narrowing for readers) |

### (iii) `<id>.error.json.v` — currently `1`

| Change | Bumps `v`? |
|---|---|
| Add a new `phase: "build-script"` | no |
| Add `errors[].source_url` | no |
| Add a new top-level `summary: string` | no |
| Rename `errors[].message` → `errors[].text` | **yes** |
| Change `primary_span` from object to nullable | **yes** |
| Make `errors[]` allow nested errors (recursive schema) | **yes** |

### (iv) Cache layout `v1/` — currently `v1`

| Change | Bumps to `v2`? |
|---|---|
| Add a new sidecar MIME (`<id>.parquet`) | no |
| Add new fields to `meta.json` / `status.json` | no |
| Switch CAS fan-out from 2-char to 3-char (`9f3/...`) | **yes** |
| Add `prior_chain_hash` salt to the cache-key formula | **yes** (every key would shift) |
| Move `index.json` from `v1/` to `v1/state/` | **yes** |

The cache layout is **the most expensive to bump** because users pay full
re-evaluation on the next run. We are conservative.

---

## 3. Back- and forward-compatibility promises

### Forward compatibility (older reader, newer writer)

- **Older CLI reading newer package metadata** (e.g. user installed
  `@preview/evcxr:0.4.0` but is running `evcxr-typst 0.1`):
  - **Unknown fields**: older CLI **ignores them silently**. This is a
    promise. Documented in `package-api.md` § 5.1: "`options` is a
    forward-compatible bag of kind-specific kwargs; older CLIs ignore unknown
    keys." Same applies to top-level fields added under the same `v`.
  - **Unknown `kind` values**: older CLI emits a warning ("unknown kind
    `rust-bench`, treating as `rust-hidden`") and proceeds.
  - **Unfamiliar `v`** (older CLI sees `v: 2`): hard error. The CLI prints
    the min-CLI message (§ 4) and exits non-zero. It does **not** attempt
    best-effort parsing.

- **Older Typst package reading newer sidecars** (e.g. user pinned package
  to `0.1.0` but ran `evcxr-typst 0.4`):
  - **Unknown sidecar fields**: older package ignores them. Typst's
    `json("...")` returns the whole dict; the package only reads the keys
    it knows.
  - **Unfamiliar `<id>.error.json.v`**: package renders an error box (§ 7).

### Backward compatibility (newer reader, older writer)

- **Newer CLI reading older package metadata**: supported across one major
  bump. CLI ships migration code for `v: N-1` → `v: N` for each schema.
  Older-than-N-1 is rejected with "regenerate from a newer package."
  Concretely: a `v0.4` CLI reads `v: 1` and `v: 2` snippet metadata;
  a hypothetical `v: 0` would be rejected.
- **Newer CLI reading older on-disk sidecars**: supported (`v1` and `v2`
  sidecars co-exist on disk per § 6).
- **Newer Typst package reading older sidecars**: supported across one major
  bump. The package reads `<id>.error.json.v: 1` and `v: 2`. Bumping the
  package's min-CLI requirement (§ 4) is the way to drop older support.

### What we do **not** promise

- We do **not** promise downgrade safety. A user who runs `evcxr-typst 0.4`,
  then downgrades to `0.1`, may find a `v: 2` sidecar on disk that the older
  CLI rejects. Recovery: `evcxr-typst clean`.
- We do **not** promise wire-stable Cargo features / kwarg defaults across
  major package versions. The package version (semver) covers that.

---

## 4. Minimum-CLI mechanism — `min_cli` in `setup()`

**Decision.** A Typst package release announces "you need CLI ≥ X" via a
`min-cli:` kwarg on the package's `setup()` call. The package emits this as a
separate top-level metadata marker `<evcxr-min-cli>` that `typst query`
returns alongside `<evcxr-snippet>` markers. The CLI reads it during the
discovery pass (before evaluating any snippet) and aborts with a helpful
message if its own version is too old.

### Concrete shape

```typ
#import "@preview/evcxr:0.4.0" as evcxr
#evcxr.setup(
  show-source: true,
  min-cli: "0.4.0",        // semver requirement; the CLI must satisfy it
)
```

The package's `setup()` body emits:

```typ
[#metadata((v: 1, min_cli: "0.4.0"))<evcxr-min-cli>]
```

When `min-cli` is omitted, the package emits no `<evcxr-min-cli>` marker and
the CLI imposes no minimum (compatible with any CLI that knows the schema
versions present in the document).

### CLI behaviour

During `discover.rs` after `typst query`, the CLI:

1. Looks up `<evcxr-min-cli>` markers (zero or one expected; if multiple from
   nested imports, takes the **highest** requirement).
2. Compares its own `env!("CARGO_PKG_VERSION")` against `min_cli` using
   semver `Version::matches(&VersionReq::parse(format!(">={}", min_cli)))`.
3. On mismatch, prints to stderr and exits `2`:

```
error: this document requires evcxr-typst >= 0.4.0
       this binary is evcxr-typst 0.1.0
       (the document's @preview/evcxr package was published with a newer
        sidecar schema; upgrade evcxr-typst or pin the package to an
        older version)
   --> main.typ
       (declared at: setup() call, main.typ:3)

upgrade with:  cargo install --force evcxr-typst
or pin the package:  #import "@preview/evcxr:0.3.0" as evcxr
```

The package itself never tries to detect CLI version. It only declares its
requirement; the CLI enforces.

### Why this and not the alternatives

- A top-level `<evcxr-min-cli>` standalone (no `setup()`) would force every
  document to add a magic incantation. Folding it into `setup()` (already
  recommended) keeps the surface small.
- Having the CLI consult `evcxr-typst --version` from the package side is
  impossible — Typst packages can't shell out.
- A version-checking *function* on the package side (`evcxr.require(">=0.4")`)
  was rejected: would only fire if the user calls it; declarative `min-cli:`
  in `setup()` is harder to forget.

---

## 5. Minimum-package mechanism

The CLI does **not** enforce a minimum package version. Rationale: the
package only emits metadata; if its `v` fields are ones the CLI knows, the
CLI can drive evaluation. If they aren't, the CLI errors per § 7.

If a future CLI ships a feature that requires a particular `kind` or
`options` key the older package can't emit, the user simply doesn't get that
feature — no hard failure. We may add an advisory log line ("feature X
requires package ≥ Y; package is Z") but won't refuse to run.

This is asymmetric on purpose: the CLI is a binary the user actively
chose; the package is a transitive dependency they may not have curated.
We make the CLI the source of strictness.

---

## 6. Cache layout migration (`v1/` → `v2/` and beyond)

**Decision.** Side-by-side directories with a one-time migration prompt;
old caches are **never auto-deleted**.

### Concrete behaviour when the CLI bumps cache to `v2`

1. CLI sees `.evcxr-typst-cache/v1/` exists and `v2/` does not.
2. CLI creates `.evcxr-typst-cache/v2/` and proceeds with all writes there.
3. CLI emits one informational log line:
   ```
   info: cache layout upgraded to v2; old cache at .evcxr-typst-cache/v1
         is preserved but unused. Run `evcxr-typst clean --layout v1` to
         delete it.
   ```
4. `evcxr-typst clean` (no flags) cleans the *current* layout only.
   `clean --layout v1` deletes a specific older layout. `clean --all`
   deletes every `v?/` directory.
5. **No automatic migration of contents.** The cache-key inputs that
   typically change between layouts (formula, fan-out) make migration
   approximately as expensive as just re-evaluating; we let users choose
   when to take that hit by re-running.

### Why side-by-side, not delete-on-upgrade

- Users may downgrade for debugging; preserving `v1/` lets the older CLI
  keep working.
- Disk cost is bounded: snippet sidecars are small; CAS contents for a
  typical document are MB-not-GB.
- Auto-deletion would be a foot-gun if a `v2`-bumping CLI were run by
  mistake on a project the user wasn't ready to migrate.

### `gc` interaction

`evcxr-typst gc` only walks the current layout's CAS. Old layouts are
opaque blobs to it.

---

## 7. Unknown `v` rendering — what the package does

When the Typst package reads a sidecar with a `v` it doesn't know:

| Sidecar | Behaviour |
|---|---|
| `<id>.error.json.v` is unknown | Package renders an `_evcxr-error-box` (the same helper used in `errors.md` § 4) with `phase = "internal"`, header label `schema mismatch`, body text: *"This document was rendered with a newer evcxr CLI ({sidecar.v}) than the installed @preview/evcxr package supports ({package.max_supported_v}). Upgrade @preview/evcxr or downgrade evcxr-typst."* The error box is red-bordered and visible like any other error. Per `errors.md` § 4 the box replaces the snippet output. |
| `<id>.txt` / `<id>.png` / `<id>.json` etc. | These are MIME blobs, not versioned. They render as usual; the package only checks `v` on metadata-bearing files. |
| `<id>.meta.json` (the catch-all for non-canonical MIME) is unknown shape | Renders the raw box with the MIME stamp; if the schema is too new to parse, falls through to a placeholder with reason `"sidecar schema unknown"`. |

The package exposes `evcxr.max-supported-error-v` as a constant
(currently `1`). This is what the unknown-`v` error box quotes back to
the user.

When the **CLI** sees an unknown `v` on metadata it queried (e.g. a future
package emits `<evcxr-snippet>.v: 2`), the CLI behaves as in § 4: hard
error with the min-CLI guidance. It does not attempt best-effort parsing
because doing so risks driving evcxr with mis-typed input.

---

## 8. Examples — three scenarios

### Scenario A — CLI older than package

> User installs `@preview/evcxr:0.4.0` (which emits `<evcxr-snippet>.v: 2`
> and declares `min-cli: "0.4.0"`). They have `evcxr-typst 0.1` on PATH.

1. User runs `evcxr-typst run main.typ --allow-eval`.
2. CLI runs `typst query`, gets `<evcxr-min-cli>` with `min_cli: "0.4.0"`.
3. CLI compares: `0.1.0 < 0.4.0` → fail.
4. CLI prints the message in § 4, exits `2`.
5. User upgrades the CLI (or pins the package to `0.3.0`).

If the package had not declared `min-cli`, the CLI would have proceeded
to read `<evcxr-snippet>` markers, found `v: 2`, hit the unknown-`v`
branch in § 7, and emitted a similar error — but later in the pipeline
and with less actionable copy. `min-cli` exists to fail fast.

### Scenario B — Package older than CLI

> User pinned `@preview/evcxr:0.1.0` (emits `<evcxr-snippet>.v: 1`,
> `<id>.error.json.v: 1`). They run `evcxr-typst 0.6`, which natively
> writes `v: 2` sidecars.

1. CLI sees `<evcxr-snippet>.v: 1` from the package. It knows `v: 1` (one
   major back is supported per § 3) and reads it.
2. CLI evaluates snippets normally.
3. When the CLI writes error sidecars, it must check what the **package**
   supports. The CLI reads the imported package version from the
   `<evcxr-min-cli>` marker if present; otherwise from a separate
   `<evcxr-package-version>` marker the package always emits (a tiny
   addition: package version is one field, always present, in the `setup()`
   metadata). If the package version says it only supports
   `<id>.error.json.v ≤ 1`, the CLI writes `v: 1` sidecars.
4. The PDF renders with no schema mismatch.

> Note: writing `v: 1` from a CLI that internally prefers `v: 2` requires
> the CLI to keep `v: 1` writers around for one major. This is the
> cost of supporting older packages — bounded.

### Scenario C — On-disk cache from a previous CLI

> Project's `.evcxr-typst-cache/v1/` was populated by `evcxr-typst 0.4`.
> User upgrades to `evcxr-typst 0.6`, which uses cache layout `v2`.

1. CLI sees `v1/` exists, `v2/` does not.
2. CLI creates `v2/` and writes all new entries there.
3. CLI logs the migration line in § 6.
4. First run is effectively a cold cache (everything misses against `v2/`)
   — every snippet re-evaluated. Subsequent runs hit `v2/` normally.
5. `v1/` sits on disk untouched. User runs
   `evcxr-typst clean --layout v1` when they're confident they don't want
   to roll back.

If the user later downgrades to `evcxr-typst 0.4`, that older CLI sees
`v1/` (still there) and `v2/` (ignored — unknown layout). It reads from
`v1/`, writes to `v1/`, and works exactly as before.

---

## 9. Summary checklist for a release

When cutting a new CLI or package release, walk this:

- [ ] Did any of (i)–(iv) change incompatibly? If yes, bump that `v`.
- [ ] If the package's emitted schemas changed (i, ii) or its expected
      sidecar schema changed (iii), update `min-cli:` in the package's
      `setup()` template / docs.
- [ ] If the cache layout changed (iv), bump to `v{N+1}` and add the
      migration log line to release notes.
- [ ] Update the "Current version" column in § 1 of this doc.
- [ ] Add a row to the bump-examples tables in § 2 if the change isn't
      already represented.
- [ ] Update `evcxr.max-supported-error-v` in `lib.typ` if (iii) bumped.

Cross-references:

- `docs/design/package-api.md` § 5 — schema definitions for (i) and (ii).
- `docs/design/errors.md` § 2 — schema definition for (iii).
- `docs/design/cache.md` § "Cache layout on disk" — definition of (iv).
- `docs/DECISIONS.md` D-019 — the decision record summarizing this policy.
