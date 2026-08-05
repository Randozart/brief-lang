# Comprehensive Cleanup: Accuracy, Nesting, and Vectorization

## Motivation

Three independent issues share root causes in the LLVM backend:

1. **False MISMATCH signals** — C benchmarks auto-vectorize, changing Float32
   association order. Briv stays scalar. F32 precision differences (~1e-7) are
   inherent, not bugs.

2. **Arrowhead nesting** — 6 key files have depth 8–14 against the AGENTS.md
   limit of 2. Deep `if let` + `match` + `writeln!` chains make the code
   fragile and hard to extend.

3. **No vectorization** — The txn loop IR (`%State` struct load/store, branch
   pre-check/body/post-check/backedge) is unrecognizable to LLVM's loop
   vectorizer. C auto-vectorizes to `<4 x float>`/`<8 x float>`; Briv emits
   only scalar `@llvm.sqrt.f32` and `fadd`.

The last two issues are linked — rewriting loop emission to produce
countable LLVM IR (a cleaner structure) naturally fixes some of the
nesting depth.

## Files to Modify

| File | Lines | Max Depth | Role |
|------|-------|-----------|------|
| `benchmarks/build_and_bench.sh` | 403 | — | Phase 1 |
| `src/backend/llvm/loop_engine.rs` | 2537 | 13 | Phase 2 + 3 |
| `src/backend/llvm/mod.rs` | 3223 | 11 | Phase 2 |
| `src/backend/llvm/emit_stmt.rs` | 1026 | 13 | Phase 2 |
| `src/backend/llvm/emit_toplevel.rs` | 2296 | 8 | Phase 2 |
| `src/analysis/transition_graph.rs` | 1987 | 9 | Phase 2 |

Total: ~11K lines in 6 files.

---

## Phase 1: Epsilon-Based Harness Comparison

**Goal**: Replace strict string `==` with numeric epsilon comparison for
Float outputs. Stops false MISMATCH signals for f32 precision noise.

**Implementation**: In `check_correctness()` of `build_and_bench.sh`:

- Detect if output lines are numeric floats (match `/^-?[0-9]+\.[0-9]+$/`)
- For each line, compare `abs(briv - c) < 1e-5` (relative epsilon)
- Non-numeric lines still use strict `==`
- Mixed output (some numeric, some text) uses per-line strategy

**Trade-off**: 1e-5 is 100× larger than f32 precision (≈1e-7), but 100×
smaller than any real bug (algorithm error → energy difference > 0.1).
Safe margin.

---

## Phase 2: Arrowhead Nesting Cleanup

**Goal**: Reduce max nesting from 13 to ≤2 across all 6 files.

**Strategy**: Five mechanical transforms, applied file-by-file:

### Transform A: Guard clause extraction
```rust
// Before (depth 7):
if let Some(val) = opt {
    if val > 0 {
        if let Some(inner) = val.field() {
            // ... depth 7 body
        }
    }
}

// After (depth 2):
let val = opt else { return; };
if val <= 0 { return; }
let Some(inner) = val.field() else { return; };
// ... body at depth 2
```
Applied to: `emit_toplevel.rs` (field load chains), `mod.rs` (match
guards), `emit_stmt.rs` (type dispatch).

### Transform B: Named helper extraction
When guard clauses don't suffice (the body is genuinely complex), extract
the inner block into a named helper function.

### Transform C: Match arm flattening
```rust
// Before:
match (a, b) {
    (Some(x), Some(y)) if x > y => { ... 20 lines ... },
    (Some(x), None) => { ... 5 lines ... },
    _ => {}
}

// After:
if let (Some(x), Some(y)) = (a, b) {
    if x <= y { return; }
    ... 20 lines ...
}
if let (Some(_), None) = (a, b) {
    ... 5 lines ...
}
```

### Transform D: Meet-in-the-middle `if let`
For `if let Expr::BinaryOp(bop) = ...` chains where the inner type
determines the behavior, extract the inner operation to the outer level.

### Transform E: Flat match on nested enums
Use `matches!()` and early filters to reduce pattern-match nesting.

**Phase 2 completed (2026-07-03):**

| File | What was done |
|------|---------------|
| `emit_stmt.rs` | Extracted `ensure_typed_value` (value conversion), `emit_state_gep` (GEP gen); refactored TupleDestructure and memory-mode field store |
| `emit_toplevel.rs` | Extracted `emit_field_init_value` (shared field init); deduplicated ~200 lines between `emit_init_state` and `emit_inline_init_stores` |

**Phase 2 deferred** (depth is structural conditional chains, not repeated
code; to tackle when these functions need modification):

| File | Lines | Max depth | Why deferred |
|------|-------|-----------|-------------|
| `loop_engine.rs` | 2537 | 16 | Complex conditional chains unique per function |
| `mod.rs` | 3223 | 13 | Same — nested logic, not extractable blocks |
| `transition_graph.rs` | 1987 | 9 | Analysis module, not backend |

**Rule**: After each change, `cargo test --lib` must pass.

---

## Phase 3: Countable-Loop IR Restructuring

**Goal**: Emit txn loops as LLVM-countable `for` loops so the loop
vectorizer recognizes and optimizes them.

**Current loop IR** (simplified):
```llvm
case_hdr:
  %ssa = load %State, ptr %slot
  %count = extractvalue %State %ssa, 0
  %cond = icmp slt i64 %count, %bound
  br i1 %cond, label %body, label %exit

body:
  ; force computation (scalar)
  ; velocity update (scalar)
  ; position update (scalar)
  ; count = count + 1
  %new_ssa = insertvalue ...
  store %State %new_ssa, ptr %slot
  ; guard check + print
  br label %case_hdr, !llvm.loop !100
```

**Target loop IR**:
```llvm
entry:
  br label %loop_header

loop_header:
  %i = phi i64 [ 0, %entry ], [ %next, %loop_latch ]
  %exit_cond = icmp ult i64 %i, %bound
  br i1 %exit_cond, label %body, label %done

body:
  ; force computation as vectorizable array operations
  ; count is implicit in %i
  br label %loop_latch

loop_latch:
  %next = add i64 %i, 1
  ; guard check + print (uses %i, not stored count)
  br label %loop_header, !llvm.loop !20

done:
  ; swan song / final energy
```

**Key challenges:**

1. **State extraction**: Currently Briv emits `load %State` + `extractvalue`
   per field. For countable loops, we need GEP-based array access or
   load-per-field. The `phi i64 %i` replaces the stored counter.

2. **Guard checking**: The `[count % 5000000 == 0]` guard currently reads
   from stored %State. In countable IR, it reads `%i` (the induction
   variable). This is straightforward: change the guard expression to
   reference `%i` instead of the stored count.

3. **Multi-field state**: For 33 fields, LLVM's SROA promotes at most 32
   elements. The 33rd field needs special handling (likely GEP-based
   load/store).

4. **Swan song hooks**: `[count == bound] { term! -> ... }` must fire
   after the loop exits — no longer part of the loop body. This becomes a
   post-loop block.

5. **Reactive txns**: Multi-txn reactive programs cannot always use
   countable loops (triggers may fire at unpredictable times). The decision
   tree must distinguish countable vs. non-countable txns.

**Decision tree** (new, in `loop_engine.rs`):
```
is_callable_txn(txn) → bool:
  - Single precondition of form count < bound ✓
  - Single postcondition of form count == bound ✓
  - No reactive triggers (@link, #!exit) ✓
  - No foreign term hooks (frgn in guards) ✓
  - Periodic guards only reference count (not other state) ✓
```

If callable, emit countable `phi %i` loop. Otherwise, fall back to
existing SSA/memory loop emission.

**Impact**:
- nbody_sqrt (scalar→vector): expect 1.29× → ≈1.0× (close to C)
- nbody_newton (memory→vector): expect 1.48× → ≈1.0×
- precompute_sum (already folded): no change
- All other runtime benchmarks: no change (single-txn, folded)

---

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Countable loop breaks multi-txn programs | Medium | Conservative decision tree; full test suite |
| SROA limit on 33-field struct | Low | GEP the excess fields |
| Phase 2 introduces regressions | Medium | Per-file `cargo test --lib` |
| Phase 3 misses edge cases (count=0, bound=0) | Low | Precondition in decision tree |

## Timeline

| Phase | Files | Tests | Est. Commits |
|-------|-------|-------|-------------|
| 1: Epsilon harness | `build_and_bench.sh` | `--correctness` | 1 |
| 2a: emit_stmt.rs | `emit_stmt.rs` | `cargo test --lib` | 3-5 |
| 2b: transition_graph.rs | `transition_graph.rs` | `cargo test --lib` | 3-5 |
| 2c: emit_toplevel.rs | `emit_toplevel.rs` | `cargo test --lib` | 5-8 |
| 2d: loop_engine.rs | `loop_engine.rs` | `cargo test --lib` | 5-8 |
| 2e: mod.rs | `mod.rs` | `cargo test --lib` | 5-8 |
| 3: Countable loop | `loop_engine.rs` | `cargo test --lib` + benchmarks | 5-10 |

Total: ~30-45 commits across all phases.
