# Plan: Dead-Field Elimination (Effectively-Pure Body Detection)

**Date:** 2026-06-01
**Status:** In Progress

## Problem

After Phase 1 (fair C benchmarks), C beats Briev on IIR filter 1.67×:
- Briev: 0.15s — keeps 50M biquad iterations (FMUL/FADD/FSUB + stores)
- C: 0.09s — clang proved non-volatile floats are never observed, eliminated ALL float math, kept only `volatile long count` incq loop

Briev emits every state store faithfully, regardless of whether the field value
is ever consumed. Clang performs liveness analysis and eliminates dead stores.
Briev should match this — the contract system already gives us precise knowledge
of which fields matter.

## Root Cause

**No liveness analysis in Briev.** The program's output set is well-defined:

```briev
// IIR filter's state:
let x1: Float = 0.0;  // dead — consumed only within process body
let x2: Float = 0.0;  // dead
let y1: Float = 0.0;  // dead
let y2: Float = 0.0;  // dead
let count: Int = 0;   // LIVE —  #!exit count == N

#!exit count == N;     // defines the live-output set
```

Once we know `{x1, x2, y1, y2}` are dead, the body is **effectively pure** —
the only live effect is `count = count + 1`, which is a pure counter increment
with a known bound. The same O(1) store optimization we already apply to enum/async
dispatch (Paths 4/5) should apply to Path 2 (folded while-loop).

## Solution

A `compute_live_fields()` pass that determines which state fields are observable,
then extends the existing `is_pure_body` classification to `is_effectively_pure`.

### Step 1: `compute_live_fields()` in `transition_graph.rs`

```rust
pub fn compute_live_fields(
    exit_condition: &Option<Box<Expr>>,
    graph: &TransitionGraph,
    txns: &[(String, &Transaction)],
    field_index_map: &HashMap<String, usize>,
) -> HashMap<String, HashSet<String>> {
    // Returns per-txn live-field set
    // Live fields for a txn:
    //   1. Identifiers in #!exit expr (e.g. {count})
    //   2. Fields read by OTHER txns' pre/post bodies AND written by THIS txn
    //      (inter-txn dataflow — a field written by txn A and read by txn B is live in A)
    //   3. Fields in the write-set of OTHER txns that share precond vars with THIS txn
}
```

**Live-set rules**:

| Rule | Description |
|------|-------------|
| `exit_ids` | All identifiers in `#!exit <expr>` are globally live |
| `cross_write_read` | If txn A writes `f` and txn B reads `f`, then `f` is live for txn A |
| `cross_precond` | If txn A and txn B share a precondition variable that both write, that variable is live |

For single-txn programs like IIR filter, the live set is just `exit_ids`.

### Step 2: Extend `TransitionNode` with `is_effectively_pure`

```rust
pub struct TransitionNode {
    pub name: String,
    pub pure_vars: HashSet<String>,
    pub is_pure_body: bool,          // existing: body has NO state writes at all
    pub is_effectively_pure: bool,   // NEW: body writes only live fields through pure counters
    // ... existing fields ...
}
```

In `build_graph()`, after computing `is_pure_body`:
```rust
node.is_effectively_pure = live_fields_for_txn.iter().all(|f| {
    node.is_pure_body || (
        node.increments.as_ref().map_or(false, |inc| inc.var == *f)
        && node.bounded_pre.as_ref().map_or(false, |bp| bp.var == *f)
        // AND all OTHER writes are to dead fields
    )
});
```

Actually, simpler: the body is effectively pure if:
- It has a `bounded_pre` + `increments` on a live field (counter convergence)
- Every other store in the body writes to a dead field
- Bodies with no live stores at all (all writes are dead) → effectively pure + total stays at initial

### Step 3: Wire into fold detection in `llvm.rs`

**Path 2 (folded while-loop)**: In `generate()`, where `bounded_pre` is detected and the folded path is taken, check `is_effectively_pure` instead of `is_pure_body`:

```rust
// Before (line ~570 area):
if node.is_pure_body && bounded_pre && increments && total_const {
    // emit_folded_pure_counter — O(1) store
} else {
    // emit_folded_main — O(N) while-loop
}

// After:
if node.is_effectively_pure && bounded_pre && increments && total_const {
    // emit_folded_pure_counter — O(1) store  
    // count gets store i64 total; dead fields stay at init value (0.0/0)
} else {
    // emit_folded_main — O(N) while-loop with full body
}
```

**Paths 4/5 (enum/async)**: Already handled via `enum_fold_pure` — the pure-counter detection already picks up `is_pure_body` txns. No change needed if they were already `is_pure_body = true` (which the IIR case isn't — it writes float state). For the IIR case (single-txn folded loop), the main path already handles it.

Wait — actually the IIR filter goes through Path 2 (folded while-loop), not Path 4 (enum). So the change point is in the folded-main emission path.

Looking at the code flow:
1. `generate()` detects `bounded_pre` + increments → folded
2. Calls `emit_folded_main()` or `emit_folded_pure_counter()` based on `is_pure_body`
3. `is_pure_body` is false for IIR (it writes floats) → `emit_folded_main()` → full while-loop

The fix: use `is_effectively_pure` instead of `is_pure_body` in the fold-emission branch.

### Step 4: Classification details

For a txn to be **effectively pure**, ALL of these must hold:
1. `bounded_pre` exists (counter variable with known bound)
2. `increments` exists (pure counter increment on that variable)
3. `bounded_pre.var` is a live field (this is the actual work)
4. Every other field written by the body is dead (can be dropped)

Implementation in `build_graph()` after computing the live-field set:

```rust
node.is_effectively_pure = if let (Some(ref bp), Some(ref inc)) = (&node.bounded_pre, &node.increments) {
    if inc.var == bp.var && inc.delta > 0 {
        // The counter increment is on a proven bounded var.
        // Check if ALL other writes are to dead fields.
        let non_counter_writes: Vec<&String> = node.write_set.iter()
            .filter(|f| *f != &inc.var)
            .collect();
        non_counter_writes.iter().all(|f| !live_set.contains(*f))
    } else {
        false
    }
} else {
    false
};
```

### Expected Results

| Benchmark | Before | After | 
|-----------|--------|-------|
| iir_filter | 0.15s (50M biquad iterations) | 0.00s (store count=50M) |
| precompute_sum | 0.00s | 0.00s (unchanged) |
| ring_buffer | 0.00s | 0.00s (unchanged) |
| async_counters | 0.00s | 0.00s (unchanged) |

C is still at 0.09s (volatile incq loop). Briev should drop to 0.00s — the
`store i64 50000000` is faster than 50M `incq`.

## Files Changed

| File | Change | Lines |
|------|--------|-------|
| `src/analysis/transition_graph.rs` | `compute_live_fields()`, extend `TransitionNode`, extend `build_graph()` | ~60 |
| `src/backend/llvm.rs` | Wire `is_effectively_pure` into fold-emission branch | ~5 |
| Tests | 1 new test: IIR body classified as effectively pure | ~25 |
