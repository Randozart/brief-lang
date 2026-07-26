# Spec & Docs Update Plan — `:>` Projection Operator

**Date:** 2026-06-04
**Status:** Planning complete
**Constraint:** All `.bv` files use `:>` as the primary syntax. `len()` is
  a stdlib convenience function (`defn len(x) { term x :> Size }`), not magic.

## Dependency Order

```
Phase 1: Parser for :>   (blocking)
    ↓
Phase 2: lib/std/ .bv files   (parseable once parser is done)
    ↓
Phase 3: lib/compiler/ .bv files   (depend on stdlib)
    ↓
Phase 4: Spec + learn-brief docs   (can be done in parallel with Phases 2-3)
    ↓
Phase 5: Internal docs (AGENTS.md, plans, BUGS.md)
```

---

## Phase 1: Parser for `:>` (file: `src/parser.rs`)

The token `Token::ColonGreaterThan` exists in the lexer. The parser needs
to recognize it in `parse_postfix` or similar.

**Insertion point:** In the expression parser, after parsing a primary
expression (identifier, literal, parenthesized, etc.), check if the next
token is `ColonGreaterThan`. If so:
1. Consume it
2. Expect an identifier token (one of: `Size`, `Bytes`, `Ptr`, `Alignment`, `Range`)
3. Map the identifier to `ProjectionTarget` variant
4. Emit `Expr::Projection { source, target }`

**Parser target identifier → ProjectionTarget mapping:**
```
"Size"      → ProjectionTarget::Size
"Bytes"     → ProjectionTarget::Bytes
"Ptr"       → ProjectionTarget::Ptr
"Alignment" → ProjectionTarget::Alignment
"Range"     → ProjectionTarget::Range
```

The parser must reject unknown projection target names with a clear error.
No type information is needed at parse time — the typechecker validates later.

**Look for `parse_factor` or `parse_postfix` at ~line 1200-1500 in parser.rs.**

---

## Phase 2: `lib/std/` `.bv` files

Replace all `.len()` and `len(x)` usage with `:> Size`. The `len` convenience
functions become:

```
defn len(s: String) -> Int { term s :> Size; }
defn len(list: List<Int>) -> Int { term list :> Size; }
```

### Files and changes

| File | Lines to change | Pattern |
|------|----------------|---------|
| `lib/std/string.bv` | ~hundreds | Replace `s.len()`, `len(s)` with `s :> Size` |
| `lib/std/collections.bv` | 4, 10-11 | `term.len()` → `term :> Size`, `list.len()` → `list :> Size` |
| `lib/std/stack.bv` | 22, 27, 39, 48-49 | Same replacements |
| `lib/std/queue.bv` | 22, 27, 48-49 | Same replacements |
| `lib/std/string_builder.bv` | 12, 16, 23-93 | Same replacements |
| `lib/std/json.bv` | 132, 138 | `k.len()` → `k :> Size` |
| `lib/std/io.bv` | 54-208 | `term.len()` → `term :> Size` |
| `lib/std/metro_bridge.bv` | 77 | `request.len()` → `request :> Size` |

**Convention:** Only `len` functions use `:> Size`. For `push`, `pop`, `insert`,
etc., the `<-` arrow syntax is already correct and needs no changes.

---

## Phase 3: `lib/compiler/` `.bv` files

Same pattern — replace `len(x)` function-call syntax and `.len()` method-call
syntax with `x :> Size`. This is the self-hosted compiler code.

| File | Approximate count |
|------|------------------|
| `lib/compiler/typechecker.bv` | ~25 sites |
| `lib/compiler/proof_engine.bv` | ~40 sites |
| `lib/compiler/parser.bv` | ~6 sites |
| `lib/compiler/lexer.bv` | ~2 sites |
| `lib/compiler/range.bv` | ~3 sites |
| `lib/compiler/call_graph.bv` | ~20 sites |
| `lib/compiler/main.bv` | ~15 sites |
| `lib/compiler/backends/x86_64.bv` | ~5 sites |
| `lib/compiler/backends/wasm.bv` | ~4 sites |

---

## Phase 4: Spec + Learn-Brief Documentation

### `spec/SPEC.md` (7 sites)
Replace all `x.len()` and `len(x)` with `x :> Size`. Key locations:
- Line 1066: `list.len()` → `list :> Size`
- Line 1080: `vec.len()` → `vec :> Size`
- Line 1128: `s.len()` → `s :> Size`
- Line 1388: `data.len() > 0` → `data :> Size > 0`
- Line 1497: `string.len(s)` → `s :> Size`
- Line 1522: `list.len()` → `list :> Size`

### `spec/LANGUAGE-TUTORIAL.md` (3 sites)
- Line 1218: `string.len(s)` → `s :> Size`
- Line 1228: `list.len()` → `list :> Size`
- Line 1319: `observers.len()` → `observers :> Size`

### `learn-brief/` (8 files, ~50 sites)

Systematic search-and-replace:
- `items.len()` → `items :> Size`
- `buffer.len()` → `buffer :> Size`
- `observers.len()` → `observers :> Size`
- etc.

Note that in **contract positions** (pre/postconditions), the syntax changes:
```
// Before:
[i < items.len()]
[items.len() > 0]

// After:
[i < items :> Size]
[items :> Size > 0]
```

The relational operator order changes. Previously `items.len() > 0` (method
call then comparison). Now `items :> Size > 0` (projection then comparison).
This looks natural but may affect readability in contracts — worth noting
in spec docs.

### `docs/` files
- `docs/design/determinism-and-optimization-frontier.md`: `len(s) <= 140` → `s :> Size <= 140`
- `docs/reference/BRIEF_ADVENTURES_RESEARCH.md`: `inventory.len() > 0` → `inventory :> Size > 0`
- `docs/reference/DBRIEF_SPEC.md`: `name.len() > 0` → `name :> Size > 0`
- `docs/milestones/` (TIER1-6, SELF_HOSTING_PLAN): same pattern
- `docs/old_docs/guides/GUIDE.md`: ~15 sites

---

## Phase 5: Internal Docs

### `AGENTS.md`
- Section "LLVM Backend Gaps": Remove `ListLen` entry (line 213) — no longer stubbed
- Section "Expressions — Fully Implemented": Remove `ListLen` (line 149), add `Projection`
- Section "Collection Method Calls": Remove `list_append`/`get` references
- Section "Self-Hosting Pipeline": Update `len()`/`is_digit` references

### `.opencode/plans/2026-06-04-comprehensive-halting-plan.md`
- Section "UFCS Resolution Pipeline": Replace — no longer relevant
- Section "Step 4: Collection drain detection": Update `ListLen` references to `Projection(Size)`

### `.opencode/plans/2026-06-04-projection-operator.md`
Already up to date (written as the plan for this change).

### `BUGS.md`
- Update `len()` infinite recursion entry (line 212) — now resolved by `:>` operator
- Remove `.len()` guard references

### Other plan files in `plans/` and `llvm-spec/`
- `plans/2026-06-02-typed-ssa-and-commutativity.md`: `Expr::ListLen(a)` → removed
- `plans/active/BRIEF_OPTIMIZATION_PLAN.md`: `Expr::ListLen(Expr)` → removed
- `plans/2026-06-03-llvm-backend-completion.md`: Phase 3a ListLen → now Projection
- `llvm-spec/08e-AOT-SIZE-INFERENCE.md`: `list_len_ptr` → update

---

## Total Estimated Effort

| Phase | Files | Changes | Risk |
|-------|-------|---------|------|
| 1. Parser | 1 | ~25 lines | 🟢 Low |
| 2. lib/std/ .bv | ~8 | ~100 replacements | 🟡 Medium (must not break logic) |
| 3. lib/compiler/ .bv | ~10 | ~120 replacements | 🟡 Medium (self-hosted, harder to test) |
| 4. Spec + learn-brief | ~20 | ~200 replacements | 🟢 Low (docs only) |
| 5. Internal docs | ~15 | ~100 replacements | 🟢 Low (docs only) |

**Total:** ~54 files, ~545 replacements