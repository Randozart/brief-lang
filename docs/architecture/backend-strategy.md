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
| `mod.rs` | 1,700 | `LlvmBackend` struct (45 fields in 9 groups), `generate()` entry point, builder methods, `build_field_index`, `validate_schema_types`, `is_ptr_expr`, `trg_llvm_storage_ty`, `sparsity_ratio`/`find_perfect_hash` |
| `emit_toplevel.rs` | 550 | Top-level emission: `emit_header`, `emit_declares`, `emit_init_state`, `emit_definition`, `emit_transaction`, `emit_callable_txn`, `emit_precondition_check`, `emit_pre_function`, `emit_async_body`, `emit_fused`, `emit_shape_guarded_body`, `emit_fused_composed`, `emit_trg_load`, `native_float_or_box`, `llvm_type`, `align_of`, `declare_state_type` |
| `emit_expr.rs` | 897 | `emit_expr()` router — all 20+ Expr variant arms including ProjectionTarget (18 targets), BracketOp (MultiSlice), Slice, collection emissions, field access, match/pattern, tuple |
| `emit_stmt.rs` | 394 | `emit_stmt()` router — all Statement variant arms including Let, Assignment, Guarded, Term/TermBang, Unification, Escape, InlineAsm |
| `loop_engine.rs` | 881 | Folded loop SSA engine: `emit_folded_loop`, `emit_folded_main`, `emit_ssa_main`, `emit_folded_multi_main`, `emit_folded_pure_counter`, `emit_exit_expr`, `emit_main`, `pre_extract_float_fields`, `pre_extract_int_fields` |
| `dispatch.rs` | 256 | Reactor dispatch: `emit_reactor`, `emit_parallel_reactor`, `extract_ranges`, `resolve_dispatch_first_txn`, `dispatch_has_pre`, `build_write_masks`, `check_exit_condition_idents` |
| `optimizer.rs` | 280 | Decision tree: `select_optimization_strategy`, `classify_txns`, `is_trigger_gated`, `extract_trigger_keys`, `extract_enum_keys`, `select_dispatch_mode` |
| `hazard.rs` | 249 | SLP hazard analysis: `estimate_slp_hazard`, `slp_attr`, `is_float_field`, `is_float_expr_pre_cg`, `count_cross_float_ops`, `collect_local_floats_and_temps`, `target_hardware`, `count_all_float_ops`, `count_float_arith_ops` |
| `tests.rs` | 2,882 | 80+ unit tests — backend correctness, wake triggers, SLP hazard, chain composition, exit conditions, natural death, struct/enum, collections, projections |
| `kani.rs` | 57 | 6 Kani proof harnesses (fast group) |

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
