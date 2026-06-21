# Cleanup Batch — AGENTS.md, Codegen Stubs, Docs, Kani

**Date:** 2026-06-21  
**Status:** In Progress

## Motivation

Discrepancies between AGENTS.md "LLVM Backend Gaps" table and actual backend
state have accumulated. The table claims 9+ expression types are stubs when
they were all implemented during Phase 3.5 and later sprints. Separately,
~17 `add i64 0, 0` stub sites in `emit_expr.rs` silently produce wrong
results, and 6 runtime functions are called without LLVM declare statements.

## Work Items

### 1. Update AGENTS.md LLVM Backend Gaps Section

Replace the outdated table with the actual remaining gaps inventory.

**Files:** `AGENTS.md:331-353`

### 2. Fix Error-Guard Stubs (11 sites)

The `else` branches of `sort`, `reverse`, `range`, `trim_left`, `trim_right`,
`to_lower`, `contains_at`, `splitn`, `int_to_str`, `strlen`, `float_to_str`,
`to_str` intrinsics emit `add i64 0, 0` when called with wrong arg count.
Change to emit LLVM `unreachable` + Rust `unreachable!()`.

**Files:** `src/backend/llvm/emit_expr.rs`

### 3. Fix FloatToStr Working Path (3 bugs in 5 lines)

- Uses `adapt_to_i64` instead of `ensure_float_reg` → passes i64 bits as double
- Calls `@__snprintf__` (doesn't exist) instead of `@snprintf`
- References `@.str.float_fmt` (undeclared) instead of declared format string

**Files:** `src/backend/llvm/emit_expr.rs:2087-2091`

### 4. Fix ToStr Working Path

Currently always calls `@__int_to_str__` regardless of input type. Need type
dispatch: Int → `__int_to_str__`, Float → `snprintf`, Bool → `__int_to_str__`,
Char → `__chr_to_str`, String → identity.

**Files:** `src/backend/llvm/emit_expr.rs:2096-2101`

### 5. Add 6 Missing Declare Statements

`__trim_left__`, `__trim_right__`, `__to_lower__`, `__contains_at__`,
`__find_from__`, `__splitn__` are called in emit_expr.rs but never declared
in the LLVM IR.

**Files:** `src/backend/llvm/emit_toplevel.rs:97-114`

### 6. Fix Unknown-Type Codegen

- `bytes` projection: return 0 for unknown types (should compute size)
- `FieldAccess` field not found: emit `unreachable` instead of `add i64 0, 0`
- Projection catch-all: add missing `return` statement
- `UserDefined`/`UserDefinedWithArg` projections: look up field in TypeUniverse

**Files:** `src/backend/llvm/emit_expr.rs:2482, 2616, 2619, 2639, 2747`

### 7. Fix Slice/MultiSlice Gaps

- Slice `stride` and `mask` silently ignored (lines 2904-2905)
- MultiSlice only handles `BracketOp::Coord(Index(_))` — stride, mask, range
  all silently ignored (lines 2784-2793)

**Files:** `src/backend/llvm/emit_expr.rs`

### 8. Stale Doc References

- `docs/reports/2026-06-03-agents-audit-and-backend-plan.md:36,61` — remove
  `ForAll`/`Exists` references (constructs removed from language)
- `docs/plans/2026-06-20-constraint-unification.md:4` — change status to
  "Completed"

### 9. Missing Architecture Docs

Create 10 docs in `docs/architecture/features/`:

| Doc | Source | Core Concept |
|-----|--------|-------------|
| `arrow.md` | `arrow.rs` | `<-` dispatch on Value type |
| `block.md` | `block.rs` | Block expression evaluation |
| `dbvl.md` | `dbvl.rs` | D-Brief line validation |
| `ellipsis.md` | `ellipsis.rs` | `...` spread operator |
| `field.md` | `field.rs` | Struct field access dispatch |
| `pattern.md` | `pattern.rs` | Unification/wildcard matching |
| `sigcall.md` | `sigcall.rs` | SigModifier: Export/Inline/Out |
| `subtype.md` | `subtype.rs` | `<:` subtype derivation |
| `traits.md` | `traits.rs` | ExprTypecheck/ExprEval/ExprCodegenLLVM |
| `tuple.md` | `tuple.rs` | Tuple construction/destructure |

### 10. Kani Harnesses for 3 Modules

| Module | Candidate |
|--------|-----------|
| `emit_toplevel.rs` | `sig_number()` — 32-arm pure match, ideal fast-group |
| `emit_stmt.rs` | `store_i64_result`, `adapt_to_i64` |
| `loop_engine.rs` | `LinkRef` dispatch arms |

---

## Execution Order

1. Plan file (`docs/plans/2026-06-21-cleanup-batch.md`)
2. AGENTS.md update (quick text fix)
3. All emit_expr.rs stubs and bug fixes (sub-items 2–7, ~6 hours)
4. Stale doc references (sub-item 8, ~10 min)
5. Architecture docs (sub-item 9, ~4 hours)
6. Kani harnesses (sub-item 10, ~3 hours)
7. `cargo test --lib` after each group
