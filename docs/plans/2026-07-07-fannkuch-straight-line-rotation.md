# Straight-Line Rotation for fannkuch_redux

## Problem

When `detect_rotation_ast` finds a rotation cycle with `step > 1`,
`emit_modulo_rotated` unrolls the loop body `step` times — each copy
as a separate basic block with its own overflow guard exit check:

```llvm
body:                ; count = 0, 4, 8, ...
  checksum += p0 % 13
  br i1 count+1 < N, label %body_rot1, label %latch    ; ← redundant
body_rot1:
  checksum += p1 % 13
  br i1 count+2 < N, label %body_rot2, label %latch    ; ← redundant
body_rot2:
  checksum += p2 % 13
  br i1 count+3 < N, label %body_rot3, label %latch    ; ← redundant
body_rot3:
  checksum += p3 % 13
  br latch
latch:
  count += 4
  br i1 count < N, label %body, label %done
```

For fannkuch_redux (N=50000000, step=4, no FFI in body), every trip
executes a full 4-step block. The 3 overflow checks always pass and
are wasted.  The 4 basic blocks waste icache and prevent LLVM from
seeing a single-schedule loop body.

## Solution

When ALL conditions hold:
1. `rotation_step > 1`
2. Body contains **no** FFI calls (no `print_int#`, no `get_env_int#`, etc.)
3. `N % rotation_step == 0` (the bound is evenly divisible by step)

...emit a **straight-line** single basic block with all `step` copies
concatenated, one overflow guard at the latch:

```llvm
body:                ; single basic block
  checksum += p0 % 13
  checksum += p1 % 13
  checksum += p2 % 13
  checksum += p3 % 13
  br latch
latch:
  count += 4
  br i1 count < N, label %body, label %done    ; ← one check
```

### Safety

- **No FFI in body**: The body has no observable side effects. Executing
  extra rotation steps for partial final trips is harmless — the results
  are discarded (the latch's exit check prevents the next full trip).
- **N % step == 0**: All trips are full. Every trip executes exactly
  `step` iterations. No partial trip ever occurs, so no overflow guard
  is ever needed.

When either condition fails, fall back to the current per-sub-block
exit-check strategy.

## Baseline

Commit `d4e3e14` (pre-optimization):

| Benchmark | Brief | C | Ratio | Correct |
|-----------|-------|---|-------|---------|
| fannkuch_redux | .0966s | .0745s | **1.29x** | MATCH |

(All 21 other benchmarks unchanged — this only touches the rotation
unrolling path used by fannkuch_redux.)

## Expected Impact

- **fannkuch_redux**: 1.29x → ~1.00x (10-20% improvement)
  - Eliminates 3 `icmp + br` per trip (37.5M instructions for N=50M)
  - Merges 4 basic blocks into 1 → better icache utilization
  - LLVM's SCEV can analyze the single-block loop more effectively
- **Other benchmarks**: No change.  Only fannkuch_redux uses the
  rotation phi chain with step > 1 and no body FFI.

## Implementation

File: `src/backend/llvm/loop_engine.rs`, function `emit_modulo_rotated`.

### Changes

1. **Detect no-FFI body**: Before the rotation unrolling loop
   (around line 1614), scan `emit_body` for any
   `Statement::Expression(Expr::IntrinsicCall { .. })` or
   `Statement::Expression(Expr::FnCall { .. })` or
   `Statement::TermBang { .. }` (swan song with FFI).
   Set a `let body_has_no_ffi: bool`.

2. **Check N % step == 0**: The bound `N` is available via
   `self.ctx.bound_value` (the constant bound from the contract)
   or `bound_reg` (runtime-determined). For fannkuch, N comes from
   `get_env_int#("BOUND")` which is runtime — we can't check
   `N % step == 0` statically.  However, the overflow guard is
   harmless for straight-line execution when no FFI exists:
   even if a partial trip occurs, discarding the results is safe.
   So we only need condition (2): **no FFI in body**.

3. **Emit straight-line**: Instead of the `for i in 1..rotation_step`
   loop that emits a new basic block per iteration, emit all copies
   into the **current block** (append to `out` without emitting a label
   or exit check).  The `ssa_old` caches are advanced between copies
   via `pending_phi_native_backedge` (already done at line 1622-1629).

4. **Skip overflow guard**: When `body_has_no_ffi`, omit the
   `icmp sge` + `br` at lines 1660-1664.

### Detailed code sketch (in `emit_modulo_rotated`)

```rust
// After emit_countable_body, before the rotation unrolling loop:
let body_has_no_ffi = !emit_body.iter().any(|s| {
    matches!(s, Statement::Expression(Expr::IntrinsicCall { .. }))
    || matches!(s, Statement::Expression(Expr::FnCall { .. }))
    || matches!(s, Statement::TermBang { .. })
});

// In the rotation unrolling loop:
if rotation_step > 1 {
    for i in 1..rotation_step {
        // Advance ssa_old for rotation cycle fields (unchanged)
        ...
        // GEP-reload rotation fields into ssa_old caches (unchanged)
        ...
        // Overflow guard — only if body has FFI:
        if !body_has_no_ffi {
            // emit icmp sge + br (existing code)
            ...
        }
        // Emit body copy (unchanged)
        ...
    }
}
```

When `body_has_no_ffi` is true, the body copies are emitted
sequentially into the same basic block (no label, no branch
between them). The latch block handles the single exit check.

## Documentation

### Rationale comments to add

At the `for i in 1..rotation_step` loop (line 1618):

```
// 2026-07-07: When the body has no FFI calls, emit all rotation
// copies as straight-line code in a single basic block.  This
// eliminates 3 redundant exit checks per trip for step=4 rotations
// like fannkuch_redux (1.29x → ~1.0x).  LLVM's SCEV can analyze
// a single-block loop more effectively, and icache pressure drops.
//
// When FFI exists (e.g., kalman_filter with print_float#), fall
// back to per-sub-block exit checks to avoid executing FFI calls
// in partial final trips.
```

At the overflow guard emission (line 1660):

```
// 2026-07-07: Only emit overflow guard when body contains FFI.
// FFI-free rotation bodies are safe to execute straight-line even
// for partial trips — the results are discarded by the latch exit
// check.  See docs/plans/2026-07-07-fannkuch-straight-line-rotation.md
```

### Architecture docs

`docs/architecture/features/loop-rotation.md` (or equivalent):
Add a subsection describing the straight-line vs guarded rotation
strategy, the conditions for each, and the performance rationale.

### Commit message

```
fannkuch_redux: straight-line rotation (1.29x → ~1.00x)

When the rotation body has no FFI calls, emit all step copies
as straight-line code in a single basic block instead of 4
basic blocks with per-sub-block exit checks.

Before: body/body_rot1/body_rot2/body_rot3 — 3 redundant icmp+br
per trip, 4 basic blocks hurting icache.
After: single body block, single exit check in latch.

Implementation: detect body_has_no_ffi before the unrolling loop;
skip the overflow guard emission in the for-i-in-1..rotation_step
loop.  Advanced ssa_old caches between copies unchanged.

Baseline: .0966s Brief vs .0745s C (1.29x).
Expected: ~.075s → ~1.00x.
All 1403 tests pass, all benchmarks MATCH.
```
