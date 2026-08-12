# AGENTS.md + LLVM Backend Completion Plan — Audit Report

**Date:** 2026-06-03
**Author:** Randozart

## Problem

Every AI session resets context about Briev's capabilities. Agents repeatedly conclude:
- "Briev is a reactive state machine DSL with no arrays"
- "Briev has no strings or collections"
- "Briev needs malloc for buffers"
- "The interpreter has known gaps in Block/Tuple/Struct/Match"

All of these are false. The interpreter (`src/interpreter.rs`, 2327 lines) already supports the full expression language. The LLVM backend (`src/backend/llvm.rs`, 6024 lines) lags behind with stubs returning `0` for 10 Expr variants and silently skipping Struct/Enum TopLevel nodes.

## What Was Done

### AGENTS.md Overhaul
- Added **Language Architecture** section explaining Briev IS general-purpose
- Added **Misconceptions to Avoid** table — 6 common AI mistakes with corrections
- Added **Interpreter Completeness** table — every Expr/Statement variant with line numbers and status
- Added **LLVM Backend Gaps** table — exact line numbers for every stub, empty catch-all, and degraded codegen path
- Removed stale "Known gaps in interpreter" section — all variants are now confirmed implemented
- Added **Key Philosophy for Backend Work** section — contract-preserving, additive-only, interpreter-as-reference principles

### LLVM Backend Completion Plan
7-phase plan document at `plans/2026-06-03-llvm-backend-completion.md`:

| Phase | Feature | Lines Affected | Status |
|-------|---------|---------------|--------|
| 1 | Struct codegen (LLVM types, StructInstance, FieldAccess) | llvm.rs:427, 2674-2676 | Not started |
| 2 | Enum codegen (tagged union, field binding in Match) | llvm.rs:427, 2616, 2691-2733 | Not started |
| 3 | Collection ops (2-slot list header, Slice, MultiSlice) | llvm.rs:2657-2669 | Not started |
| 4 | Tuple + TupleDestructure | llvm.rs:2672-2673 | Not started |
| 5 | Runtime-sized allocation (contract-proven bounds) | llvm.rs:2641 (extension) | Not started |
| 6 | ForAll + Exists quantification | llvm.rs:2746-2752 | Not started |
| 7 | Nested recursive types | llvm.rs:2616 (extension) | Not started |

### Key Design Decisions
1. **Struct representation**: `alloca i64, i64 <n_fields>` with `ptrtoint` — same convention as ListLiteral and enum constructors. SROA handles scalar promotion.
2. **List header**: Change from `[ptr]` to `[ptr, len]` — enables ListLen without a separate tracking mechanism.
3. **Enum discriminants**: Slot 0 contains discriminant. Slot 1..N contain field values. Already used in current code; formalized in the plan.
4. **All additions are additive**: New match arms only. No existing optimization path is touched.
5. **Interpreter is source of truth**: Every codegen decision references the specific interpreter line that implements the behavior.

## Verification

The interpreter was audited line-by-line against the AST. Confirmed implementations:
- `Expr::Block` at interpreter.rs:1806 ✅
- `Expr::Tuple` at interpreter.rs:1815 ✅
- `Expr::TupleDestructure` at interpreter.rs:1822 ✅
- `Expr::Match` at interpreter.rs:1897 ✅
- `Expr::PatternMatch` at interpreter.rs:1682 ✅
- `Expr::StructInstance` at interpreter.rs:1662 ✅
- `Expr::FieldAccess` at interpreter.rs:1648 ✅
- `Expr::MultiSlice` at interpreter.rs:1848 ✅
- `Statement::Unification` at interpreter.rs:579 ✅
- `Value::HashMap`, `Value::HashSet`, `Value::Stack`, `Value::Queue`, `Value::StringBuilder` ✅

Two genuine gaps remain in the interpreter:
- `Expr::ForAll` — stub returning always true (line 1838)
- Recursive `defn` calls compile but have no stack depth limit

## Files Changed
- `AGENTS.md` — comprehensive rewrite (152 lines added, 18 removed)
- `plans/2026-06-03-llvm-backend-completion.md` — new plan document
