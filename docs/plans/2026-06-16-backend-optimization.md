# Backend Optimization — Loop Canonicalization & Reduction Hoisting

**Date:** 2026-06-16  
**Status:** Plan — implementation in progress

## Root Causes Identified

LLVM optimization remarks (`-pass-remarks-missed=loop-vectorize`) on fannkuch_redux.ll reveal two blockers:

1. **"could not determine number of loop iterations"** — The `rct txn` convergence loop uses manual phi-node merge patterns that don't match LLVM's canonical loop form. LLVM can't analyze the trip count, so it refuses to unroll or vectorize.

2. **"value that could not be identified as reduction is used outside the loop"** — The `[count == N] { term! -> print_int#(nchecksum) }` guard causes loop-carried values (checksum, max_flips) to be "used outside the loop" (by the print inside the guard), so LLVM can't identify them as reductions.

3. **"load of type ptr not eliminated"** — `ptrtoint`/`inttoptr` boxing of String/pointer fields destroys LLVM's pointer provenance, disabling alias analysis.

## Fix Plan

### Fix 2: Emit LLVM-Canonical Loop Structure

**File:** `src/backend/llvm/loop_engine.rs`

**Current:** The loop emission uses a complex phi-node merge pattern for the folded-loop/precomputation path, and the direct-SSA dispatch path inherits some of this complexity. The loop structure is:
```
tick:        ; loop header
  ; check precondition [count < N]
  br i1 %cond, label %body, label %done

body:        ; transaction body
  ; computation, field stores
  br label %check

check:       ; convergence check
  ; phi merge for SSA state
  ; check postcondition [count == N]
  br i1 %done_cond, label %done, label %tick

done:        ; exit
  ret i32 0
```

**Target:** For the common case of `[count < N][count == N]` loops with a predictable induction variable, emit a standard LLVM canonical loop:
```llvm
entry:
  br label %loop.header

loop.header:
  %i = phi i64 [ 0, %entry ], [ %next, %loop.latch ]
  %exit = icmp slt i64 %i, %N
  br i1 %exit, label %loop.body, label %loop.done

loop.body:
  ; transaction body — computation, field stores
  br label %loop.latch

loop.latch:
  %next = add i64 %i, 1
  br label %loop.header

loop.done:
  ret i32 0
```

This matches LLVM's canonical form: one phi induction variable, constant step of 1, clear pre-header (entry), single latch (loop.latch), single back-edge.

**Implementation approach:**
- Detect when the transaction has a simple `[count < bound][count == bound]` pattern
- If detected, emit the canonical loop structure instead of the general phi-merge pattern
- The SSA state register pipeline (insertvalue/extractvalue) stays the same — just the loop control flow changes

### Fix 3: Hoist `[count == bound]` Final Print Out of Hot Loop

**Root cause:** The `[count == bound] { term! -> print_int#(nchecksum); }` guard is inside the loop body, evaluated every iteration. LLVM sees `checksum` used in the guard's print and can't identify it as a reduction.

**Fix:** When the guard's then-path terminates (via `term!`), move it to a post-loop block. The loop body only contains the computation (which LLVM can vectorize), and the final print fires after the loop exits.

**Implementation:**
- In `emit_ssa_main` (or equivalent), detect terminating guards
- Emit the guard's body in a separate block after the loop exit
- The loop's exit path branches to the post-loop block
- The post-loop block runs the final print and exits

### Fix 1: Eliminate `ptrtoint`/`inttoptr` Boxing in Hot Paths

**Root cause:** `emit_expr.rs:61-65` and `emit_stmt.rs:311-314` convert String/pointer fields from `i8*` to `i64` via `ptrtoint`, pass through i64, then `inttoptr` back on use. This destroys pointer provenance.

**Fix:** Keep pointer types as native `ptr` in SSA registers. Only box to `i64` at the FFI boundary.

**Implementation:**
- Change `TypedRegister` for `Type::String` to hold `ptr` (i8*) instead of `i64` in SSA mode
- Change `emit_expr.rs:61-65` to pass `i8*` directly without `ptrtoint`
- Change `emit_stmt.rs:311-314` to store `i8*` directly without `inttoptr`

### Priority and Expected Impact

| Fix | Impact | Effort | Key benchmarks |
|---|---|---|---|
| Fix 2: Canonical loop | Enables trip-count analysis → loop unrolling + vectorization | Medium | fannkuch, mandelbrot, knucleotide |
| Fix 3: Hoist final print | Enables reduction identification → vectorization | Small | fannkuch, knucleotide |
| Fix 1: No ptrtoint | Enables alias analysis → LICM + GVN | Large | All benchmarks |

## Verification

1. `opt -O3 -pass-remarks-missed=loop-vectorize` no longer reports "could not determine number of loop iterations"
2. `opt -O3 -pass-remarks-missed=loop-vectorize` no longer reports "value used outside the loop"
3. Benchmark timing improves (target: <1.20x for all integer benchmarks vs C)
4. All 909 tests pass
