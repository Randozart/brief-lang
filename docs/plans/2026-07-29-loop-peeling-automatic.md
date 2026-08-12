# Phase 6: Automatic Loop Peeling for Mixed Compute + I/O Loops

**Date:** 2026-07-29
**Based on diagnostic experiment:** `benchmarks/nbody_newton_peeled.bv` — see `docs/plans/2026-07-29-flat-allocas-and-loop-peeling.md`

## Executive Summary

A manual loop-peeling experiment on the nbody_newton benchmark **eliminated 0.39× of the 0.39× performance gap in one step** — improving from 1.22× C (Briev loses) to **0.83× C (Briev wins)**. The only change was removing the `when count % 5000000 == 0 { PrintLn!(energy) }` guard from the loop body.

This confirms that **LLVM's if-conversion blocker** (not phi-register pressure) was the dominant bottleneck. The `control flow cannot be substituted for a select` diagnostic occurs because the guarded block contains an opaque function call (`PrintInt#`), which LLVM cannot speculate.

The fix is a **front-end loop peeling pass** that automatically detects infrequent side-effecting guards and splits the loop into an inner pure-compute loop and an outer structural loop.

## The Diagnostic Experiment

### Methodology

Created `benchmarks/nbody_newton_peeled.bv` — identical to `nbody_newton.bv` except:

```diff
-    when count % 5000000 == 0 {
-        let __periodic: Bool = PrintLn!(energy);
-    };
     when count == bound {
         term! -> PrintLn!(last_energy);
     };
```

This is the exact same 32-phi structure, same state fields, same computation. The ONLY change is removing one `when` guard.

### Results

| Version | Time (5M BOUND) | Ratio vs C | LLVM vectorizer remark |
|---------|:--------------:|:----------:|------------------------|
| **Original nbody** | 1.096s | **1.22×** | `control flow cannot be substituted for a select` |
| **Peeled nbody** | **0.740s** | **0.83×** | `value not identified as reduction used outside loop` |
| C reference | 0.896s | 1.00× | (no remark — C was not compiled with -Rpass) |

**Improvement: 0.39× of the gap eliminated. 32 phi nodes still present. 0 remaining vector phis.**

### Implications

1. **The phi-register-pressure theory was WRONG.** Despite 32 phi nodes on a machine with 16 XMM registers, the peeled loop runs faster than C. The register allocator handles the phis efficiently — the 31 identity `fadd float 0.0` ops are eliminated by LLVM's peephole optimization, and spill pressure is manageable.

2. **The branch-guard was the DOMINANT bottleneck.** The `when` guard containing `PrintLn!` prevented LLVM from applying if-conversion to the entire loop. The `select` instruction can replace simple conditional branches, but not branches containing opaque function calls.

3. **After loop peeling, a SECOND blocker appears**: `value that could not be identified as reduction is used outside the loop`. This is the `last_energy = energy` pattern — the energy computation is a reduction whose final value is read after the loop. LLVM still handles this better than the branch guard (0.83× vs 1.22×).

4. **A fully automatic peeling pass should target ≤ 0.80× C** by also hoisting the termination print guard.

## The Loop Peeling Transformation

### Input (before)

```briev
node simulate [count < bound][count == bound] {
    // ... compute physics ...
    energy = epp + ekc;
    last_energy = energy;

    when count % 5000000 == 0 {    // ← HOISTABLE: infrequent, side-effecting
        let _: Bool = PrintLn!(energy);
    };
    when count == bound {           // ← HOISTABLE: terminal, side-effecting
        term! -> PrintLn!(last_energy);
    };
    term;
};
```

### Output (after)

```briev
txn inner_body(count: Int, bound: Int,
    bx0: Float32, ..., last_energy: Float32)
    -> (Int, Float32, ..., Float32)
    [count < bound][count == bound]
{
    // ... compute physics (same as before) ...
    count = count + 1;
    // ... compute energy ...
    let new_last_energy: Float32 = energy;

    // NO when guards — pure compute only
    // All state fields passed as parameters and returned

    term (count, bx0, ..., new_last_energy);
};

node outer_loop
    [inner_count < bound][inner_count == bound]
{
    // Call inner_body for one iteration (pure compute)
    (inner_count, bx0, ..., last_energy) = inner_body(
        inner_count, bound, bx0, ..., last_energy
    );

    // Structural loop: check and print
    when inner_count % 5000000 == 0 {
        let _: Bool = PrintLn!(last_energy);
    };
    // ... continue until convergence ...
    term;
};
```

### Alternative — Simpler Single-Function Approach

If the above is too complex (function calls between transactions may not be fully supported), a simpler approach is to **peel the loop within the same node** using Briev's compound block syntax:

```briev
node simulate [count < bound][count == bound] {
    // BATCH 1: Pure compute (N iterations without side effects)
    [count < bound];  // convergence gate — continue while not exhausted
    // ... compute physics ...
    count = count + 1;
    // ... compute energy ...
    last_energy = energy;

    // BATCH 2: Side effects (periodic print, termination)
    when count % 5000000 == 0 {
        let _: Bool = PrintLn!(last_energy);
    };
    when count == bound {
        term! -> PrintLn!(last_energy);
    };
    term;
};
```

This preserves both batches within the same node structure. The key is that BATCH 1 (lines 5-8) is a **single basic block** — no branches, no function calls. LLVM can if-convert BATCH 1 independently of BATCH 2.

However, this may not help because BATCH 2 still contains the `when` guards, and the loop header branches OVER BATCH 2 when it's not taken. The issue is that the loop still has the conditional branch to the print block — it's just at the bottom of the body instead of in the middle.

### Recommended: Two-Transaction Approach

The cleanest approach is to emit **two separate LLVM functions**:

```llvm
; Inner — pure compute, single block, no branches
define void @inner(ptr %state) {
  ; ... compute physics ...
  ; ... update state fields ...
  ret void
}

; Outer — structural, contains the when guards
define i32 @main() {
  %state = alloca ...
  call @init_state(ptr %state)
  br label %.loop

.loop:
  call void @inner(ptr %state)  ; N iterations of pure compute
  ; ... periodically check and print ...
  ; ... termination check ...
  br i1 %cond, label %.loop, label %.exit

.exit:
  ret i32 0
}
```

The `inner` function is pure (no branches, no function calls) and can be fully vectorized by LLVM. The `outer` function handles the structural logic.

The transformation from the original single-loop `node` to the two-function `inner`+`outer` is:
1. **Hoist all `when` guards** from the body to the outer function
2. **The inner function body** is the original body with guards removed
3. **The outer function** calls `@inner` and implements the `when` checks

### Implementation Strategy

**Phase 1 — Analysis** (new pass in `src/analysis/loop_peeling.rs`):

```rust
/// Analyze a node body for hoistable when guards.
///
/// A guard is hoistable if:
/// 1. It contains a function call (PrintLn, PrintInt, etc.)
/// 2. It is infrequent (guarded by modulo, terminal check, etc.)
///   - Terminal check: `when count == bound`
///   - Periodic check: `when count % N == 0`
///
/// Returns the hoistable guard(s) and the remaining body.
fn find_hoistable_guards(body: &[Statement]) -> (Vec<usize>, Vec<Statement>) {
    let mut hoist_indices = Vec::new();
    let mut remaining = Vec::new();
    for (i, stmt) in body.iter().enumerate() {
        if is_hoistable_guard(stmt) {
            hoist_indices.push(i);
        } else {
            remaining.push(stmt.clone());
        }
    }
    (hoist_indices, remaining)
}

fn is_hoistable_guard(stmt: &Statement) -> bool {
    // Match: when <expr> { <body containing fn call> }
    match stmt {
        Statement::Guarded { cond: _, body } => {
            contains_function_call(body)
        }
        _ => false,
    }
}
```

**Phase 2 — Two-function emission** (in `mod.rs` dispatch):

When hoistable guards are found:
1. Emit `@inner_<txn>(ptr %state)` — the pure compute function
2. Emit `@main` — calls `@inner` in a loop, checks guards

Otherwise, emit the current single-function code (no change).

**Phase 3 — Benchmark verification:**

- `nbody_newton`: 1.22× → ≤ 0.85×
- All other benchmarks: still MATCH, unchanged
- Regression check: compare `.ll` output before/after for non-peeled loops

## LLVM Diagnostic Reference

The following LLVM diagnostics indicate specific blockers that our peeling pass addresses:

| Diagnostic | Cause | Our fix |
|------------|-------|---------|
| `control flow cannot be substituted for a select` | Loop body has a conditional branch containing an opaque call | Peel the guard containing the call into the outer loop |
| `value that could not be identified as reduction is used outside the loop` | A reduction's final value is read after the loop | Peel the termination guard (last_energy print) into the outer loop |
| `loop not vectorized: call instruction cannot be vectorized` | Any opaque function call in the loop body | All function calls must be hoisted to the outer loop |

## Detailed Implementation

### File: `src/analysis/loop_peeling.rs` (new)

```rust
// ── Loop Peeling Analysis ──────────────────────────────────────
//
// 2026-07-29: Detects infrequent side-effecting guards in loop bodies
// and facilitates peeling them into an outer structural loop.
//
// A guard is hoistable if:
//   1. It's a Statement::Guarded with a side-effecting body
//   2. The condition is infrequent (terminal check, periodic check)
//
// See docs/plans/2026-07-29-loop-peeling-automatic.md

use crate::ast::{Expr, Statement};
use std::collections::HashSet;

/// Result of the loop peeling analysis.
pub struct PeelingResult {
    /// Guards that can be hoisted to the outer loop (indices into original body).
    pub hoistable_guards: Vec<usize>,
    /// The body with hoistable guards removed — this is the inner loop body.
    pub inner_body: Vec<Statement>,
    /// The hoistable guard statements themselves — these go in the outer loop.
    pub outer_guards: Vec<Statement>,
}

/// Analyze a transaction body for hoistable guards.
pub fn analyze_loop(body: &[Statement]) -> PeelingResult {
    let mut hoistable_guards = Vec::new();
    let mut outer_guards = Vec::new();
    let mut inner_body = Vec::new();

    for (i, stmt) in body.iter().enumerate() {
        if is_hoistable_guard(stmt) {
            hoistable_guards.push(i);
            outer_guards.push(stmt.clone());
        } else {
            inner_body.push(stmt.clone());
        }
    }

    PeelingResult { hoistable_guards, inner_body, outer_guards }
}

/// Check if a statement is a hoistable guard.
fn is_hoistable_guard(stmt: &Statement) -> bool {
    match stmt {
        Statement::Guarded { cond: _, body } => {
            // A guard is hoistable if its body contains a function call
            // and/or a term! -> side_effect pattern
            body.iter().any(|s| contains_function_call(s))
        }
        _ => false,
    }
}

/// Check if a statement contains a function call (directly or through term!)
fn contains_function_call(stmt: &Statement) -> bool {
    match stmt {
        Statement::Expr(e) | Statement::Term(Some(e)) => {
            has_call_expr(e)
        }
        Statement::Let { expr, .. } => {
            expr.as_ref().map_or(false, |e| has_call_expr(e))
        }
        Statement::Assign(_, rhs) => has_call_expr(rhs),
        _ => false,
    }
}

fn has_call_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Call(_, _, _) => true,
        Expr::BinaryOp(_, l, r) => has_call_expr(l) || has_call_expr(r),
        Expr::UnaryOp(_, e) => has_call_expr(e),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Statement};

    #[test]
    fn test_guard_with_println_is_hoistable() {
        let body = vec![
            Statement::Guarded {
                cond: Box::new(Expr::Decimal(1)),
                body: vec![
                    Statement::Expr(Box::new(
                        Expr::Call("PrintLn".to_string(), vec![], None)
                    )),
                ],
            },
        ];
        let result = analyze_loop(&body);
        assert_eq!(result.hoistable_guards.len(), 1);
        assert_eq!(result.inner_body.len(), 0);
    }

    #[test]
    fn test_pure_assign_not_hoistable() {
        let body = vec![
            Statement::Assign(
                Expr::Identifier("x".to_string()),
                Expr::Decimal(42),
            ),
        ];
        let result = analyze_loop(&body);
        assert_eq!(result.hoistable_guards.len(), 0);
        assert_eq!(result.inner_body.len(), 1);
    }

    #[test]
    fn test_term_with_println_is_hoistable() {
        let body = vec![
            Statement::Guarded {
                cond: Box::new(Expr::Decimal(1)),
                body: vec![
                    Statement::Term(Some(Box::new(
                        Expr::Call("PrintLn".to_string(), vec![], None)
                    ))),
                ],
            },
        ];
        let result = analyze_loop(&body);
        assert_eq!(result.hoistable_guards.len(), 1);
    }
}
```

## Implementation: Batch-Loop Approach

The correct fix is to emit TWO nested loops instead of one:

```
Outer loop: tracks batch boundaries, handles print guards
  Inner batch loop: pure compute, bounded by next print point
```

### LLVM IR Structure

```llvm
define i32 @main() {
entry:
  %state = alloca %State
  call void @init_state(ptr %state)
  br label %.outer_header

; ── Outer Loop ──────────────────────────────────────────────────
; Tracks batch boundaries. Each batch runs up to the next periodic
; print point, then checks guards.

.outer_header:
  %oh_count = phi i64 [ 0, %entry ], [ %inner_count, %.outer_latch ]
  ; 33 outer phis for state fields (3 i64 + 30 float)
  %oh_bx0 = phi float [ %bx0_init, %entry ], [ %il_bx0, %.outer_latch ]
  ; ... 30 more float phis ...
  
  ; Compute inner bound for this batch
  ; inner_end = min(bound, ((count / batch_size) + 1) * batch_size)
  %batch_size = add i64 0, 5000000  ; from the `when count % N == 0` condition
  %next_boundary = ... ; ((count / batch_size) + 1) * batch_size
  %inner_end = ... ; min(bound, next_boundary)
  
  br label %.inner_header

; ── Inner Loop (Pure Compute) ───────────────────────────────────
; NO branches, NO function calls. Single basic block compute.
; Runs count from oh_count to inner_end.

.inner_header:
  %ic = phi i64 [ %oh_count, %.outer_header ], [ %ic_next, %.inner_latch ]
  %il_bx0 = phi float [ %oh_bx0, %.outer_header ], [ %il_bx0_next, %.inner_latch ]
  ; ... 30 more inner phis
  
  ; Pure compute body (same as current, but without when guards)
  %dx01 = fsub float %il_bx0, %il_bx1
  ; ... full compute body ...
  %il_bx0_next = fadd fast float %il_bx0, %step
  
  %ic_next = add i64 %ic, 1
  %inner_done = icmp slt i64 %ic_next, %inner_end
  br i1 %inner_done, label %.inner_latch, label %.inner_exit

.inner_latch:
  br label %.inner_header

.inner_exit:
  ; Final values flow to outer latch via memory or forwarding phis
  br label %.outer_body

; ── Outer Body (Guard Checks) ──────────────────────────────────
; Runs after each batch. Checks periodic + termination conditions.

.outer_body:
  ; Check: did we cross a print boundary?
  %mod_check = urem i64 %ic_next, %batch_size
  %should_print = icmp eq i64 %mod_check, 0
  br i1 %should_print, label %.print_block, label %.outer_end_check

.print_block:
  call void @txn_simulate_cold_0(float %il_energy)
  br label %.outer_end_check

.outer_end_check:
  %is_done = icmp slt i64 %ic_next, %bound
  br i1 %is_done, label %.outer_latch, label %.exit

.outer_latch:
  br label %.outer_header

.exit:
  ret i32 0
}
```

### Key Design: Two-Level Phi Nodes

The outer loop has its OWN set of phis (one per written field). The inner loop has ANOTHER set of phis (one per written field). The inner phis get their initial values from the outer phis. After the inner loop exits, the FINAL inner phi values flow back to the outer phis for the next batch.

This avoids memory round-trips — ALL state passes through SSA registers. No stores to `%State` needed.

The cost: 2 × N phi nodes instead of N. But the inner loop phis are the same as the current single-loop phis — we're only adding the outer phis as an extra level. With 31 field phis + 1 counter phi, we go from 32 to 64 phis total. But the OUTER phis don't have backedges (they only feed inner phis), so they don't contribute to register pressure across the inner loop iterations.

### Implementation in `counter.rs`

Modify `emit_countable_main` to accept an optional `batch_info` parameter:

```rust
pub struct BatchInfo {
    pub batch_size: usize,
    pub outer_guards: Vec<Statement>,
    pub counter_var: String,
}
```

When `batch_info` is `Some`:
1. Emit `entry:` with alloca, init, load bound → br `.outer_header`
2. Emit `.outer_header:` with outer phis (one per written field)
3. Compute `inner_end` from `count`, `bound`, `batch_size`
4. Emit `.inner_header:` with inner phis (same as current per-field phis)
5. Emit pure compute body (no guards) — same as current body emission
6. Emit inner exit check (count < inner_end) instead of (count < bound)
7. Emit inner latch (backedge)
8. Emit `.inner_exit:` (forward values to outer latch)
9. Emit `.outer_body:` with guard checks (from `outer_guards`)
10. Emit `.outer_latch:` (forward to outer header or exit)

When `batch_info` is `None`: emit the current single-loop code.

### Integration in Dispatch (`mod.rs`)

```rust
// After hoist_terminating_guard and LICM:
let guards = crate::analysis::loop_peeling::split_guards(&body_stmts);
let batch_size = crate::analysis::loop_peeling::detect_batch_size(
    &guards, &bp.var
);

if !guards.is_empty() && batch_size.is_some() && total_fields > 8 {
    // Batch loop mode: emit outer + inner loop
    // Pass batch_info to emit_countable_main
} else {
    // Standard single-loop mode
}
```

### `src/analysis/loop_peeling.rs` additions

Add functions:
- `split_guards(body) -> (Vec<Statement>, Vec<Statement>)` — separates hoistable guards from pure compute
- `detect_batch_size(guards, counter_var) -> Option<usize>` — extracts N from `when count % N == 0` or uses default

## Verification

1. `cargo test --lib` — all tests pass (including new batch detection tests)
2. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks MATCH
3. Check `.ll` output for nbody: should show `outer_header` / `inner_header` blocks
4. `bash benchmarks/build_and_bench.sh --runtime` — nbody ratio should drop from 1.22× toward 0.83×
5. All other benchmarks: unchanged (single-loop code path when no guards detected)

## Timeline

| Step | Description | Effort |
|------|-------------|--------|
| 1 | Add batch detection + guard splitting to `loop_peeling.rs` | 30min |
| 2 | Modify `emit_countable_main` to accept batch_info | 1h |
| 3 | Implement outer header + outer phi emission | 1h |
| 4 | Implement inner bound computation | 30min |
| 5 | Wire dispatch to use batch mode | 30min |
| 6 | Test + benchmark | 1h |

## Diagnosting Experiment Details

The peeled benchmark at `benchmarks/nbody_newton_peeled.bv` is committed as evidence. To reproduce:

```bash
BOUND=5000000 bash -c 'time ./benchmarks/nbody_newton_peeled 2>&1'
```

Results (C reference at 0.896s, peeled at 0.740s → 0.83× C).

## Appendix: Why Loop Peeling Works

LLVM's loop vectorizer requires the loop body to be in a specific form for if-conversion:

1. The body must be a **single basic block** OR a **set of if-convertible blocks**
2. If-convertible blocks are those where **all conditional branches can be replaced by `select` instructions**
3. `select` instructions can represent `if (cond) a else b` without actual branches
4. Branches containing **opaque function calls** CANNOT be if-converted — `select` would speculate the call, executing it on every iteration instead of conditionally

By peeling the guard (removing the call from the loop body), we make the remaining body if-convertible. LLVM's loop vectorizer can then flatten the remaining branches (which are just 1-2 cycle arithmetic operations) into `select` instructions, producing a single-block loop body that can be vectorized.

The 0.83× result shows that even WITHOUT loop vectorization (blocked by the reduction issue), the if-conversion alone improves performance by 33%. The removed branch mispredictions and improved instruction cache behavior from the cleaner loop structure account for most of the gain.
