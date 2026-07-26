# Optimization Framework — Completion Plan

## Scope

Complete the optimization framework: fix a soundness bug in the enumerable dispatch path, implement compile-time complete evaluation, and add regression guards. All three items close the remaining gaps from `docs/design/optimization-cost-model.md` and the optimization framework specification.

---

## Phase A: Wake-Trigger / Enum-Dispatch Soundness Fix

### Background

A persistent wake reactor with enumerable triggers (e.g., `Bool` at size 2 ≤ budget 256) currently takes the `enumerable` path in `emit_enum_main`. Each switch arm ends with `ret i32 0` and no `@__rt_wait()` is called anywhere on this path. The program exits after one tick instead of blocking indefinitely.

```
Current flow:
  has_wake_triggers=true, triggers are enumerable (Bool, size=2)
    → emit_enum_main() called
    → switch i8 %sz_sensor { case 0: ... case 1: ... }
    → each case: while(count < N) { body() }; ret i32 0
    → PROGRAM EXITS — wake reactor never polls again
```

### Tasks

| # | Task | Files | Verification |
|---|------|-------|-------------|
| A.1 | **Lift `has_wake_triggers`** computation to before the `enumerable` block (currently at line 442 in `llvm.rs`). Currently it's computed at lines 568 and 583, after `emit_enum_main` already took effect. | `src/backend/llvm.rs` | No behavioral change; existing tests still pass |
| A.2 | **Gate enumerable on `!has_wake_triggers`** — change `if !self.trigger_names.is_empty()` to `if !self.trigger_names.is_empty() && !has_wake_triggers` | `src/backend/llvm.rs` | Wake-trigger programs no longer enter enum dispatch |
| A.3 | **New test: `test_wake_triggers_bypass_enum_dispatch`** — Bool wake trigger, budget=256. Assert `@__rt_wait()` is in output and `switch i8` is NOT in output. | `src/backend/llvm.rs` | Test passes |

### Design Notes

- The `has_wake_triggers` variable already exists and is computed at two later points (lines 568, 583). Lifting it earlier is trivial and avoids recomputation.
- The `foldable` path (line 463) already requires `!graph.has_triggers`, so it's unaffected by wake triggers.
- Non-wake programs with enumerable triggers remain fully optimized via switch dispatch.
- **Correctness impact:** This is a soundness fix — without it, persistent servers silently exit after one tick. No existing tests cover this path because current wake-trigger tests use `Type::Int` (non-enumerable, value-set size = None) to avoid the enum path.

---

## Phase B: Compile-Time Complete Evaluation

### Background

From `docs/design/optimization-cost-model.md` Section 1.1, axis 3:

> If state space ≤ budget, precompute all results at compile time. O(N) runtime → O(1) lookup / zero runtime.

When a region's total value-set size (product of all trigger value-set sizes) fits entirely within the optimization budget, the compiler can evaluate the entire region body at compile time via symbolic execution and emit a static final-state initializer. Runtime becomes a single `init_state()` call — zero instructions in the hot path.

### Architecture

```
generate(program, budget)
  │
  ├── [foldable path]     Single bounded-counter txn, no triggers
  ├── [precompute path]   NEW — region state space ≤ budget
  ├── [enumerable path]   Switch dispatch (existing)
  └── [standard path]     reactor_tick() loop (existing)
```

### Tasks

| # | Task | Files | Verification |
|---|------|-------|-------------|
| B.1 | **`is_fully_precomputable()`** on `RegionAnalyzer` — returns `true` if sum of all region value-set sizes ≤ budget, no Unbounded regions, no FFI. | `src/analysis/region.rs` | Unit test |
| B.2 | **`collect_final_values()`** — for each fully-enumerable region, evaluate the composed body over every trigger combination. Returns `Vec<(HashMap<String, i64>, counter_var, counter_val)>` — maps variable names to their compile-time-evaluated final values. | `src/analysis/region.rs` | Unit test with simple compose+eval |
| B.3 | **`emit_precomputed_main()`** in LLVM backend — emits a `@main` that calls `init_state()`, then stores final values directly into each state field via `getelementptr`+`store`. No while loops, no switch dispatch, no `reactor_tick()`. | `src/backend/llvm.rs` | Integration test |
| B.4 | **Wire into `generate()`** — after foldable check, before enumerable check, try precompute path. Fall through to enumerable/standard if precompute fails. | `src/backend/llvm.rs` | All existing tests pass |
| B.5 | **Report section** — `--optimize-report` shows "Precomputed: N regions" with per-region trigger combinations and final state values. | `src/backend/llvm.rs` | Report integration test |
| B.6 | **Tests (5):** `test_precompute_single_bool` — one Bool trigger, body sets x=trigger, should store final values directly. `test_precompute_two_bool` — two independent Bool triggers, 4 combos. `test_precompute_budget_exceeded` — state space > budget, falls through to enumerable. `test_precompute_pure_counter` — all-internal chain with precompute, no fused txn emitted. `test_precompute_with_chain` — composed chain fully precomputable, one store per state var. | `src/backend/llvm.rs` | All pass |

### Design Decisions

**Evaluation engine:** Use symbolic evaluation (constant folding + assignment tracking) rather than full `SymbolicExecutor` instantiation. The precompute path only needs to evaluate pure arithmetic expressions over known integer values — no symbolic variables, no path constraints, no postcondition checking. A simple recursive evaluator is sufficient:

```rust
fn eval_expr(expr: &Expr, bindings: &HashMap<String, i64>) -> Option<i64> {
    match expr {
        Expr::Integer(n) => Some(*n),
        Expr::Identifier(n) | Expr::OwnedRef(n) => bindings.get(n).copied(),
        Expr::Add(a, b) => Some(eval_expr(a, bindings)? + eval_expr(b, bindings)?),
        Expr::Sub(a, b) => Some(eval_expr(a, bindings)? - eval_expr(b, bindings)?),
        Expr::Mul(a, b) => Some(eval_expr(a, bindings)? * eval_expr(b, bindings)?),
        // ... other arithmetic ops
        _ => None, // Call, Term, etc. → not precomputable
    }
}
```

**Counter increments:** When a region has convergence txns (bounded pre with counter var), the final counter value is determined by the iteration bound (e.g., `total=100` → final count = 100). The evaluator doesn't need to simulate all iterations — it just stores the bound value directly.

**Trigger combinations:** For a region with triggers `(a: Bool, b: Bool)`, iterate over all 4 concrete bindings and evaluate the composed body for each. The final state is the same regardless of trigger values (the counter always converges to the bound) — so only one evaluation is needed for convergence chains. For non-convergence regions, each trigger combination produces a distinct final state, all emitted as static initializers.

**Budget cap:** Even on the precompute path, if total combinations exceed a compile-time limit (e.g., 1M), fall back to enum dispatch to avoid blowing up compilation time. This is separate from the user-facing `--optimize-budget` flag (which gates switch-dispatch emission size). The compile-time limit is a compiler-internal safety rail.

---

## Phase C: IIR Filter Benchmark Regression Test

### Background

From `docs/design/optimization-cost-model.md` Section 7.2:

> IIR 50M iterations unchanged at 0.15s

The IIR filter at `benchmarks/iir_filter.bv` is the canonical folded-path test. The optimization framework must not disrupt it.

### Tasks

| # | Task | Files | Verification |
|---|------|-------|-------------|
| C.1 | **Construct IIR filter AST in test** — mirror the `benchmarks/iir_filter.bv` program: 7 `const` declarations, 5+ `state` declarations, one `node` with convergence contract `[count < total][count == total]`, shift-register body of 5 assignments. | `src/backend/llvm.rs` | Test compiles |
| C.2 | **Assert folded path, not enum path** — verify output contains `while` loop pattern (folded main), `icmp slt` with counter comparison, and does NOT contain `switch i8`. | `src/backend/llvm.rs` | Test passes |
| C.3 | **Assert structure matches benchmark IR** — verify `@b0`, `@b1`, etc. constant globals, `%State` type with correct field count, `@init_state` with volatile stores. | `src/backend/llvm.rs` | Test passes |

### Design Notes

- The IIR filter has no triggers (`trg`), so `has_triggers=false` in the transition graph. The foldable path is selected before the enumerable/precompute paths are even considered.
- This test ensures the foldable path remains the first and only optimization applied to trigger-free convergence programs.
- The benchmark binary (`benchmarks/iir_filter`) should not be committed to git.

---

## File Impact Summary

| File | Lines Added | Description |
|------|------------|-------------|
| `src/backend/llvm.rs` | ~120 | Wake-trigger fix, precompute emission, report integration, 7 new tests |
| `src/analysis/region.rs` | ~60 | `is_fully_precomputable()`, `collect_final_values()`, expression evaluator |
| **Total** | **~180** | No new files, no breaking changes to existing APIs |

---

## Verification Strategy

Every phase must satisfy:

1. `cargo test --lib` — all existing 334 tests pass unchanged
2. IIR benchmark — Brief maintains ≥ C speed (0.15s vs 0.23s)
3. Phase-specific new tests (7 integration tests for LLVM backend)
4. **New: wake-trigger soundness** — Bool wake trigger program no longer exits after one tick

### Expected Final Test Count

| Source | Count |
|--------|-------|
| Existing | 334 |
| Phase A (wake fix) | +1 |
| Phase B (precompute) | +5 |
| Phase C (IIR regression) | +1 |
| **Total** | **341** |

---

## Execution Order

1. **Phase A** — critical soundness fix, no dependencies on B/C (independent)
2. **Phase B.1–B.3** — precompute engine and emission, depends on A for correct dispatch gating
3. **Phase B.4–B.5** — wiring and report, depends on B.3
4. **Phase B.6** — tests, depends on B.4
5. **Phase C** — regression test, independent (can run in parallel with B)

---

## Design Documents Referenced

- `docs/design/determinism-and-optimization-frontier.md` — Section 1.1, axis 3 (compile-time evaluation)
- `docs/design/optimization-cost-model.md` — Sections 1.1, 1.2, 4.6c, 6.2, 7.1, 7.2
- `plans/2026-06-01-optimization-framework.md` — Phase 5 (compile-time complete evaluation)
