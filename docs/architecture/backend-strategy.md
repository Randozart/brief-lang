<!-- 2026-06-09. Updated 2026-06-09 — LLVM backend split into subdirectory -->

# Backend Strategy

## Principle

Backend codegen is extracted into feature files via per-backend traits.
Each backend is a separate trait so changing VHDL emission never
recompiles LLVM codegen.

## LLVM Backend (Split into Subdirectory)

The LLVM backend is now `src/backend/llvm/` (7 files, 7,981 total lines;
original monolithic `llvm.rs` was ~7,800 lines).

### File Layout

| File | Lines | Content |
|------|-------|---------|
| `mod.rs` | 2,361 | `LlvmBackend` struct (38 fields), `generate()` entry point, builder methods, emit helpers (`emit_header`, `emit_declares`, `emit_init_state`, `emit_definition`, `emit_transaction`, `emit_callable_txn`, `emit_precondition_check`, `emit_pre_function`, `emit_async_body`, `emit_fused`, `emit_shape_guarded_body`, `emit_fused_composed`), SLP hazard analysis, trigger/extraction helpers, `sparsity_ratio`/`find_perfect_hash` |
| `emit_expr.rs` | 897 | `emit_expr()` router — all 20+ Expr variant arms including ProjectionTarget (18 targets), BracketOp (MultiSlice), Slice, collection emissions, field access, match/pattern, tuple |
| `emit_stmt.rs` | 394 | `emit_stmt()` router — all Statement variant arms including Let, Assignment, Guarded, Term/TermBang, Unification, Escape, InlineAsm |
| `folded_loop.rs` | 1,158 | Reactor dispatch (sequential + parallel), folded loop SSA engine, `emit_folded_main`, `emit_ssa_main`, `emit_folded_multi_main`, `emit_folded_pure_counter`, `emit_precomputed_main`, `emit_wake_metadata`, `emit_thread_pool_metadata`, `check_exit_condition_idents` |
| `optimizer.rs` | 280 | Decision tree: `select_optimization_strategy`, `classify_txns`, `is_trigger_gated`, `extract_trigger_keys`, `extract_enum_keys`, `select_dispatch_mode` |
| `tests.rs` | 2,834 | 80+ unit tests — backend correctness, wake triggers, SLP hazard, chain composition, exit conditions, natural death, struct/enum, collections, projections |
| `kani.rs` | 57 | 6 Kani proof harnesses (fast group — pure match dispatch, no heap allocation, no loops) |

### Emit Functions Remain Centralized

The `emit_expr` and `emit_stmt` functions remain in `emit_expr.rs` and
`emit_stmt.rs` as methods on `LlvmBackend`. The long-term plan is to
move individual Expr-variant arms into feature file `ExprCodegenLLVM`
impls (~20 cycles), matching the interpreter's `ExprEval` migration
pattern from Phase 9. For now, the directory split is sufficient: each
file is small enough to navigate, and no optimization path was touched.

## VHDL Backend

(`src/backend/vhdl.rs`, 1,261 lines) — expression emission extracted
into feature `ExprCodegenVHDL` impls. Optimizations deferred until
LLVM pattern is proven.

## Webstack Backend

(`src/backend/webstack.rs`, 2,230 lines) — expression emission extracted
into feature `ExprCodegenWebstack` impls. Optimizations deferred until
LLVM pattern is proven.
