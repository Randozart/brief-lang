# Plan: Benchmark Repair — Dead-Field Elimination, Exit Expression Codegen, @llvm.trap Audit

**Date**: 2026-06-26
**Author**: OpenCode
**Status**: Active
**Priority**: Critical

## Executive Summary

Benchmarks that use `let N: Int = getenv_int#("BOUND")` as a loop bound and
reference `N` only in preconditions/postconditions/exit-conditions produce
binaries that crash with `SIGILL` (Illegal Instruction). Root cause: three
interacting bugs in the LLVM backend.

## Root Causes

### Bug 1: Dead-field elimination drops precondition-only fields

`compute_referenced_fields` (transition_graph.rs:1040) scans only transaction
and definition **bodies**. It does NOT scan:
- Preconditions (`[ops < N]`)
- Postconditions (`[ops == N]`)
- Exit conditions (`#!exit ops == N`)
- State field initializers (`let N = getenv_int#(...)`)

`assign_field_modes` then assigns `FieldMode::Never` to any field NOT in
`referenced_fields`. The field `N` is eliminated from `%State` entirely.

`apply_field_modes` receives `live_fields` as a parameter but **never uses it**.
`live_fields` IS the correct superset (includes precondition/exit references).

**Fix**: Union `live_fields` with `referenced_fields` in `apply_field_modes`.
Also scan pre/post/exit conditions in `compute_referenced_fields` for defense
in depth.

### Bug 2: Exit expression codegen emits `@llvm.trap()` for unknown identifiers

`emit_exit_expr` (loop_engine.rs:36) handles identifiers by looking them up
in `field_index_map`, `constants`, and `trigger_names`. When a field has been
eliminated (Bug 1), none of these lookups succeed, and the function emits:

```llvm
call void @llvm.trap()
%tN = add i64 undef, 0
```

This binary trap is unconditionally reached. `opt -O3` then sees `@llvm.trap()`
followed by a conditional branch and optimizes the branch away (treating the
trap as noreturn), producing `ud2` — hence `SIGILL`.

**Fix**: The user directive says: "If a contract violation is suspected, it
should not even compile." Replace defensive `@llvm.trap()` in `emit_exit_expr`
with `panic!()` so that the field resolution failure is caught at compile time
as a clear error. This makes Bug 1 impossible to ignore — the compiler exits
with a diagnostic instead of emitting a crashing binary.

### Bug 3: `check_exit_condition_idents` runs AFTER dead-field elimination

The validation at mod.rs:1677 runs AFTER `apply_field_modes` (line 1627) has
already eliminated fields. If the exit condition references an eliminated
field, the check correctly catches it — but only because the field was
eliminated first. The order should be: validate first, eliminate second.

**Fix**: Move `check_exit_condition_idents` before `apply_field_modes`, or
conversely, ensure eliminated fields are invisible to the exit condition by
treating the exit condition as a liveness source (Bug 1 fix).

### Bug 4: `@llvm.trap()` used as defensive catch-all in 47 locations

47+ locations in the LLVM backend emit `@llvm.trap()` for unsupported
expression types or unexpected states. These are latent compiler bugs that
produce crashing binaries instead of compile-time errors. Each should be
audited and either:
- Replaced with `panic!()` (unreachable — compiler bug)
- Replaced with proper error propagation (reachable — missing feature)

## Fix Plan

### Part 1: Fix dead-field elimination (Bug 1 + Bug 3)
**File**: `src/backend/llvm/mod.rs`
**File**: `src/analysis/transition_graph.rs`

1. In `mod.rs:apply_field_modes`, union `live_fields` into `referenced_fields`
   before passing to `assign_field_modes`.
2. In `transition_graph.rs:compute_referenced_fields`, also scan:
   - `Transaction.contract.pre_condition`
   - `Transaction.contract.post_condition`
   - `Program.exit_condition`
   - `StateDecl.expr` (initializer)
3. In `mod.rs`, swap the order of `check_exit_condition_idents` and
   `apply_field_modes` so validation runs first.

### Part 2: Fix `@llvm.trap()` in emit_exit_expr (Bug 2)
**File**: `src/backend/llvm/loop_engine.rs`

Replace all defensive `@llvm.trap()` + `undef` returns in `emit_exit_expr`
(lines 87-89, 97-98, 100-103, 175-177) with `panic!()` calls that include
the unknown identifier or expression type. This converts silent binary crashes
into compile-time errors.

### Part 3: Audit `@llvm.trap()` in emit_expr (Bug 4)
**File**: `src/backend/llvm/emit_expr.rs`

Review ~44 `@llvm.trap()` occurrences. For each:
- If the code path is truly unreachable (compiler invariant violation):
  replace with `panic!()`.
- If the code path is reachable (e.g., missing intrinsic handling):
  replace with proper error return or codegen.

Keep `@llvm.trap()` in `emit_guard_check` (emit_stmt.rs:809) — these are
valid contract-violation traps for type constraints (`let x: Int[0 < x]`).

### Part 4: Verify
1. `cargo test --lib` passes
2. `cargo build` succeeds (no warnings)
3. `print_loop.bv` now COMPILES (new behavior: errors on `N` not being in
   field_index_map — which will only be fixed after Part 1 is done)
4. After Part 1 fixes: `print_loop.bv` compiles AND runs correctly

## Implementation Order

1. Part 1 (dead-field fix) — restores `N` to field_index_map
2. Part 2 (`@llvm.trap()` in emit_exit_expr → panic) — makes failures
   compile-time errors
3. Part 4 (test print_loop.bv) — verify the fix chain
4. Part 3 (audit remaining `@llvm.trap()` in emit_expr) — if time permits
