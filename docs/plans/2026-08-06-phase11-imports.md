# Phase 11 (Slice 1) — Import renaming, collision errors, glob rejection, re-export

**Date:** 2026-08-06
**Status:** Implementation plan
**Source:** `docs/plans/2026-08-05-implement-normative-language-spec.md` §17 (Phase 11, SPEC §7)

---

## 0. Executive Summary

SPEC §7 modules/imports. Imports already support quoted paths, `<root>` angle
paths, selective `import { a, b } from`, and cycle detection. This slice adds
the remaining unambiguous §7.2/§7.3/§7.4 forms:

1. **Selective rename** `import { Local: Exported, Other } from <path>` — the
   module's `Exported` name is bound as `Local`.
2. **Glob rejection** `import * from <path>` — a hard parse error (globs are
   invalid, §7.2).
3. **Collision errors** — two imports (or an import + a local) providing the
   same unqualified name is a hard resolver error, resolvable via rename
   (§7.2 "conflicting unqualified imports are errors").
4. **`export import { ... } from <path>`** — the only re-export form (§7.3);
   re-exported names propagate to importers.

Deferred (needs a design decision): the `:` module alias (`import collections:
<std/collections>`). Briv inlines imports into one name space with no
module-qualified access operator, so the alias's collision-resolution semantics
(prefixing vs a new `alias.name` access form) is a language design question —
separate slice.

## 1. Investigation findings

- `ast::Import { kind, symbols: Vec<String>, span }` — no alias/rename
  representation. `filter_items` (`import_resolver.rs:780`) keeps an item iff
  `symbols.is_empty() || symbols.contains(name)`; no rename applied.
- `parse_import` (`parser/definitions.rs:756`) handles quoted/`<registry>`
  paths and `import { a, b } from` (no `Local: Exported`), `import sym from`.
- Cycle detection exists (`in_progress`); no collision detection.
- No existing usage of any of the four forms in `lib/glue/benchmarks/.smoke`.
- Diamond deps work naturally (module cache); no changes needed.

## 2. Design

### 2.1 AST (`ast/top.rs`)

```rust
pub struct Import {
    pub kind: ImportKind,
    /// (local_name, exported_name); export == local for an unrenamed symbol.
    pub symbols: Vec<(String, String)>,
    pub alias: Option<String>,       // 2026-08-06: reserved (module alias — deferred)
    pub re_export: bool,             // `export import`
    pub span: Option<Span>,
}
```

### 2.2 Parser (`parser/definitions.rs`)

- `{ Local: Exported, Other }` — a `:` inside a symbol list records the rename
  pair; unrenamed symbols push `(name, name)`.
- `import * from <path>` → `SyntaxError` ("glob imports are invalid").
- `export import` prefix → `re_export = true`.
- Keep the existing single-`sym` + bare forms (push `(sym, sym)`).

### 2.3 Resolver (`import_resolver.rs`)

- `filter_items` filters by EXPORTED name; the surviving items' names are
  rewritten to the LOCAL name (rename).
- **Collision detection**: while merging imported items, if a name already
  exists from a DIFFERENT source (module or the importing file) and is not the
  same declaration, hard error: "import name 'X' conflicts — use a selective
  rename or module alias". Re-exported names skip the local collision check
  (they are meant to pass through).
- **Re-export**: an `export import` resolves its module like a normal import
  but records the resolved module as re-exported so an importer of THIS module
  sees those names too (propagation at the importing file's merge step).

## 3. Tests

- Parser: `{ Local: Exported }` parses as a rename pair; `import *` errors.
- Resolver: selective rename binds the local name; collision between two
  imports errors; a rename resolves a collision; `export import` propagates
  to a second-level importer; diamond (A←B←D, A←C←D) resolves; a genuine
  cycle still errors.

## 4. Baseline

Commit `cf2a5659` (docs refresh). 37/37 runtime MATCH. Expectation unchanged.

## 5. Docs

- `docs/plans/2026-08-05-spec-implementation-status.md` §7 row → In progress.
- This plan's tracker.

## 6. Tracker

- [x] AST: symbols as (local, exported) pairs — 2026-08-06
- [x] Parser: rename, glob rejection, export import — 2026-08-06
- [x] Resolver: rename application, collision errors, re-export propagation — 2026-08-06
- [x] Tests + Praetor + benchmarks + commit — 2026-08-06

## 7. Delivered (2026-08-06)

- `Import.symbols` is now `Vec<(String, String)>` (local, exported) pairs.
- Parser: `{ Local: Exported }` selective rename; `import *` is a hard
  `StagedFeature` error; `export import { ... } from` parses (resolver treats
  the `Export(Import)` wrapper as a re-export and inlines it — imports are
  inlined, so re-exported names are visible to importers).
- Resolver: `filter_items` filters by EXPORTED name and RENAMES survivors to
  the local name (preserving the D3 transitive-referenced-type closure and
  the sed file-private filter); `record_imported_names` raises a hard error
  when two DIFFERENT modules provide the same unqualified name (diamond —
  same path — is fine); `export import` propagates.
- Discovered + fixed: `filter_items` previously used the free `item_name`
  (which lacks `Meld`/`ProtocolDef`/`RenderBlock`), silently dropping imported
  melds — the GLUE `CStr <-> String` meld failed to survive imports. The
  full `Self::item_name` is now used; the two GLUE reference tests pass again.
- Deferred: the `:` module alias (needs a module-qualified-access design
  decision — Briv inlines imports with no namespace operator).

## 8. Delivered (2026-08-09) — Slice 2

- **`:` module alias** (`import collections: <std/collections>`) — a
  collision-resolving local TAG (no qualified access; Briv inlines imports).
  `Import.alias` field; parser accepts `import <ident> : <path>`; the resolver
  treats two imports of the same exported name as legal when they carry
  DIFFERENT aliases (same alias still collides). Per SPEC §7.2.
- **Configured-root determinism records** — `ImportResolver.resolved_paths`
  (specifier → canonical resolved path), in source order (SPEC §7.1).
- **Coherence provenance** — an `impl T` no longer collides with the `type T`
  it extends across modules (`record_imported_names` skips impls): a valid
  cross-module coherence pair (`type Point` in a.bv + `impl Point` in b.bv)
  resolves cleanly (§17.2).
- **Target-selected module variants** — `TargetSettings.prefer_ebv` declared in
  `config/targets.dbvl` (4th tuning column); embedded targets
  (aarch64/arm/wasm/spirv) prefer the `.ebv` stdlib sibling for extensionless
  imports (SPEC §3.3). The extension remains the resolver-time proxy (no
  triple in BuildOptions yet).
- Tests: alias resolves collision, same-alias still collides, cross-module
  impl coherence, resolved_paths recorded, embedded target prefers .ebv.
  Full suite: 1716 pass.

1619 lib tests (5 new). Praetor: no new diagnostics (10 identical at HEAD).
36/36 runtime MATCH at the verification commit; unchanged expectation.
