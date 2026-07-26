# LLVM Backend Maintainability Refactoring

**Datetime**: 2026-06-10 11:45 UTC
**Author**: OpenCode
**Current state**: 7 files, 8,246 lines. `mod.rs` = 2,578 lines.
**Target state**: 10 files, ~8,000 lines total. `mod.rs` < 1,000 lines.

## Principle

Commit after every phase. Document all structural changes in BUGS.md.
No optimizations during refactoring — benchmarks verify we didn't break anything.

## Phase R1: Split `mod.rs` into domain modules

`mod.rs` has 2,578 lines containing mixed concerns:
- `LlvmBackend` struct + `new()` + builder methods
- `generate()` entry point (~1,000 lines)
- SLP hazard analysis (~200 lines)
- Top-level emission helpers (header, declares, state, definitions, txns, triggers, etc.)
- String collection helpers
- Perfect hash / sparsity helpers

**R1a**: Extract SLP hazard analysis → `hazard.rs`
- `estimate_slp_hazard`, `count_all_float_ops`, `count_float_arith_ops`, `count_cross_float_ops`, `collect_local_floats_and_temps`, `target_hardware`, `is_float_field`, `is_float_expr_pre_cg`, `slp_attr`

**R1b**: Extract top-level emission → `emit_toplevel.rs`
- `emit_header`, `emit_declares`, `declare_state_type`, `emit_init_state`
- `emit_definition`, `emit_transaction`, `emit_callable_txn`
- `emit_precondition_check`, `emit_pre_function`
- `emit_async_body`, `emit_fused`, `emit_shape_guarded_body`, `emit_fused_composed`
- `emit_trg_load`, `emit_cast_convert`, `native_float_or_box`
- `emit_wake_metadata`, `emit_thread_pool_metadata`, `emit_async_phase` (move from emit_expr.rs)
- `emit_precomputed_main` (move from emit_expr.rs)
- `align_of`, `llvm_type`, `is_ptr_expr`

**R1c**: Extract `emit_init_state` and state-building — keep in emit_toplevel.rs or separate if large

## Phase R2: Group `LlvmBackend` fields into sub-structs

Split the ~50 flat fields into ~7 domain groups:
- `StateAccess` — field_index_map, field_types, field_initializers, mmio_*, schema_aliases, range_bounds, field_to_meta_idx
- `CodegenState` — txn_counter, let_bindings, let_binding_types, terminated, returns_i64, fn_ret_ty, callable_txn_*, in_callable_txn, param_slots
- `FFIRegistry` — frgn_map, defn_params, triggers, trigger_names, program_txns, fused_to_first, sampled_triggers
- `TypeRegistry` — struct_types, enum_types, variant_disc, constants, string_constants
- `OptimizationConfig` — optimize_budget, optimize_report, optimize_size, slp_hazard_fns, pgo_profile, pgo_guard_idx, has_cycles
- `SSAState` — ssa_state_reg, ssa_old_*_regs, reg_float_cache, reg_type_cache, state_reg_name
- `AsyncState` — has_async_txns, async_txn_names, async_thread_pool_size, is_lightweight_async

Each sub-struct gets its own file:
```
state_access.rs
codegen_state.rs
ffi_registry.rs
type_registry.rs
optimization_config.rs
ssa_state.rs
async_state.rs
```

## Phase R3: Complete emit_expr → feature file migration

Already partially done (Phase A1-A8 thunks). Remaining:
- Move `emit_binop`, `emit_fcmp`, `i64_to_float_reg` → `BinaryOpExpr::emit_llvm` real impl
- Move `Expr::Neg` float handling → `UnaryOpExpr::emit_llvm` real impl
- Convert `Expr::Identifier` → to feature file (state access logic)
- Resolve `MatchExpr` type mismatch for pattern.rs

## Phase R4: Split `folded_loop.rs`

`folded_loop.rs` = 1,158 lines with complex SSA extraction logic:
- `emit_folded_main`, `emit_ssa_main`, `emit_folded_multi_main` → `loop_engine.rs`
- `emit_folded_pure_counter` → stays or moves
- Reactor dispatch + thread pool → `dispatch.rs`

## Execute

Phase R1 → R2 → R3 → R4. Commit after each sub-phase.
Benchmark check: `bash benchmarks/build_and_bench.sh --correctness` after each phase.
Test: `cargo test --lib` after every change.
