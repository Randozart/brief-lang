# Comprehensive Halting Pattern Library + Compiler Fix Plan

**Date:** 2026-06-04
**Status:** Planning complete — ready to execute Step 1

## Architectural Reset: `term;` True Semantics

`term;` is NOT a control-flow return. It is a **compile-time symbolic checkpoint**.
When the compiler hits `term;`, it must: "Prove the postcondition is reachable from this
point. If not, analyze the state graph to prove deterministic convergence."

**Current codegen** (acceptable for now): SSA path filters `term;` (the contract proves
convergence). Non-SSA path emits `ret void` (reactor re-checks contract). No codegen
changes needed — the fix is in convergence analysis (the patterns below).

**Intended verification** (future): When `term;` is encountered, the compiler verifies
"is postcondition satisfied at this point?" If not, it walks the state graph to prove
subsequent iterations converge. This verification pass is what the 5 patterns implement.

---

## UFCS Resolution Pipeline (No Magic Strings)

The parser is type-agnostic. It parses uniformly:

| Syntax | Parsed as | Resolved to (by typechecker) |
|--------|-----------|------------------------------|
| `x.len()` | `Expr::FieldAccess(x, "len")` | `Expr::ListLen(x)` if x : List |
| `len(x)` | `Expr::Call("len", [x])` | `Expr::ListLen(x)` if x : List |
| `x.field` | `Expr::FieldAccess(x, "field")` | Stays as FieldAccess (struct) |
| Any other Call | `Expr::Call(name, args)` | Stays as Call (defn/FFI) |

The typechecker/resolver pass unifies both `len(list)` and `list.len()` into
`Expr::ListLen(list)` when the subject type is `List`. Zero magic strings in the
backend or analyzer. The `extract_bounded_pre` function then matches against the
native `Expr::ListLen` node.

---

## Implementation Order

### Step 1: P1 Validation Fix — `extract_valid_bounded_pre`

**Goal**: Prevent nbody_sqrt hang. Validate `BoundedPre` variable against body mutations.

**File**: `src/analysis/transition_graph.rs`

**Changes**:

1. Add `extract_valid_bounded_pre`:
```rust
fn extract_valid_bounded_pre(
    pre: &Expr,
    inc: &Option<IncrementInfo>,
) -> Option<BoundedPre> {
    match pre {
        Expr::And(l, r) => {
            extract_valid_bounded_pre(l, inc)
                .or_else(|| extract_valid_bounded_pre(r, inc))
        }
        other => {
            let bp = extract_bounded_pre(other)?;
            let is_mutated = inc.as_ref().map_or(false, |i| i.var == bp.var);
            if is_mutated { Some(bp) } else { None }
        }
    }
}
```

2. Reorder `compute_graph()`:
```rust
node.increments = detect_increments(&node.body);
node.bounded_pre = extract_valid_bounded_pre(&node.precondition, &node.increments);
```

**Test**: nbody_sqrt compiles, runs, terminates at 50M with live fields.
**Lines**: ~25
**Risk**: 🟢 Low

---

### Step 2: Algebraic Simplification Pass

**Goal**: Simplify `x + (R + 1) - R` → `x + 1` before `detect_increments`. Enables Pattern 4.

**File**: `src/analysis/transition_graph.rs`

Add `simplify_body(body: &[Statement]) -> Vec<Statement>` that runs a fixpoint
simplification on each assignment RHS.

**Cancellation rules** (bottom-up, fixpoint):

| Pattern | Simplified |
|---------|-----------|
| `(a + b) - a` | `b` |
| `a - (a - b)` | `b` |
| `(a + b) - (a + c)` | `b - c` |
| `(a - b) + b` | `a` |
| `a * 1`, `a / 1` | `a` |
| `a + 0`, `a - 0` | `a` |
| `a * 0` | `0` |

**Integration**: Called before `detect_increments`.
**Test**: `cancel_math.bv` with `x = x + (R + 1) - R` → `IncrementInfo { delta: 1 }`.
**Lines**: ~60
**Risk**: 🟢 Low

---

### Step 3: Popcount Decay Detection

**Pattern**: `[reg != 0][reg == 0]` with body `&reg = reg & (reg - 1)`.
**Ranking function**: `τ = popcount(reg) → 0`, bounded at 64.

**File**: `src/analysis/transition_graph.rs`

Add `detect_popcount_decay(body) -> Option<IncrementInfo>` matching
`reg & (reg - 1)` pattern on the RHS of an assignment.

**LLVM backend**: No changes. `emit_folded_loop` already handles
`ConvergeDirection::Decreasing` with `bound_literal = Some(0)`.

**Test**: `bit_clear.bv` — `[reg != 0][reg == 0]` with `&reg = reg & (reg - 1)`.
**Lines**: ~30
**Risk**: 🟢 Low

---

### Step 4: Collection Drain Detection

**Pattern**: `[len(list) > 0][len(list) == 0]` with body `x <- &list` or `<- &list`.
**Ranking function**: `τ = len(list) → 0`, decreases by exactly 1 per pop.

**Files**: `src/analysis/transition_graph.rs` + typechecker/resolver (UFCS)

**Changes**:

1. Extend `extract_bounded_pre` to handle `Expr::ListLen(list_expr)` in `Gt`/`Ge` arms.
2. Add `detect_collection_drain(body) -> Option<IncrementInfo>` matching `ArrowDir::Pop`
   with `Expr::Term` index on the target list.
3. Typechecker: rewrite `Expr::Call("len", [list])` and
   `Expr::FieldAccess(list, "len")` to `Expr::ListLen(list)` when list type is `List`.

**Test**: `queue_drain.bv` — `[len(queue) > 0][len(queue) == 0]` with `x <- &queue`.
**Lines**: ~60 (20 analyzer + 40 resolver/UFCS)
**Risk**: 🟡 Medium — UFCS resolution needs type data from typechecker.

---

### Step 5: Interval Bounds Progress

**Pattern**: `[x < N][x == N]` with body `&x = x + R1 - R2` where range analysis
proves net step ≥ 1.
**Ranking function**: `τ = N - x`, decreases by at least `minStep ≥ 1`.

**Files**: `src/analysis/transition_graph.rs` + `src/analysis/range.rs`

Add `detect_range_progress(body, ranges) -> Option<IncrementInfo>` using
existing range analysis to compute minimum net step.

**Test**: `interval_step.bv` with `&x = x + R1 - R2` where ranges guarantee min step ≥ 1.
**Lines**: ~60
**Risk**: 🟡 Medium — depends on range analysis. False negatives safe (reactor_tick).

---

### Step 6: Benchmarks for Steps 1-5

| Pattern | File | Contract | Body |
|---------|------|----------|------|
| P1 fix | nbody_sqrt (existing) | `bound > 0 && count < bound` | `&count = count + 1` |
| Algebraic | `cancel_math.bv` | `[x < N][x == N]` | `&x = x + (R + 1) - R` |
| Popcount | `bit_clear.bv` | `[reg != 0][reg == 0]` | `&reg = reg & (reg - 1)` |
| Collection | `queue_drain.bv` | `[len > 0][len == 0]` | `x <- &queue` |
| Interval | `interval_step.bv` | `[x < N][x == N]` | `&x = x + R1 - R2` |

Each benchmark: `term;`, `frgn __print` at batched interval, symmetric C reference.

---

### Steps 7-10: Trophy Updates, Deferred Patterns, Documentation

| Step | What | When |
|------|------|------|
| 7 | Trophy folder updates for new wins | After each pattern benches faster than C |
| 8 | Lexicographic tuples | Deferred (no benchmark exercises it) |
| 9 | Seed-bounded PRNG trajectory | Deferred (needs symbolic PRNG execution) |
| 10 | Non-halting patterns catalogue | Write after all patterns implemented |

---

## nbody_sqrt Expected Result After Step 1

| Before | After |
|--------|-------|
| `extract_bounded_pre` picks `bound > 0` | `extract_valid_bounded_pre` tries `bound` (no increment) → rejects → tries `count` (increment) → accepts |
| Universal loop: `while (50M < 0)` — always false | Universal loop: `while (count < 50M)` — converges |
| Infinite loop — hang | Clean termination at 50M |

## Verification

| Check | Command | Expected |
|-------|---------|----------|
| nbody_sqrt terminates | `BOUND=1000 ./nbody_sqrt` | Exits in <1s |
| Full test suite | `cargo test --lib` | 437+ pass |
| Existing benchmarks | `build_and_bench.sh <name>` | Same ratios |