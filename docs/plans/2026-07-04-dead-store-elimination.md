# Dead Store Elimination in A005c/A005d Hot Loop Bodies

## Problem

The A005c (per-field phi loop) and A005d (memory loop) codegen paths
emit a `store` instruction to `%State` after EVERY field computation in
the hot loop body. These stores are **dead** for the vast majority of
benchmarks because:

1. The loop body reads field values from phi registers (A005c) or
   from `pre_load_all_fields` (A005d), NOT from the stores.
2. The latch's backedge values use `pending_phi_native_backedge`
   (the computed register value), NOT a reload from `%State`.
3. The `done:` block only reads `%State` when there are post-loop
   hoisted guards (`term! -> swan_song`). For simple loops with
   a guard-based print inside the body (the common case), the
   `done:` block performs arena cleanup and returns — no state loads.

**Root cause**: The stores were introduced during the A005a → A005c
refactoring (July 3, `8c08890`/`a71c586`). The original A005a
(struct-SSA) approach used ONE `store %State` at the latch, which
SROA+mem2reg naturally decomposed and promoted. A005c replaced this
with per-field phis but kept the per-field stores for "compatibility
with emit_stmt memory mode" — creating stores that serve no purpose
in the phi loop.

The phi→use chain fix (`a4df377`) later made the `done:` block use
GEP+load from `%State` instead of phi registers. This means stores
ARE needed when `done:` reads state — but ONLY in that case.

## Design

### Dual-path strategy

Two modes, selected at compile time per loop:

**Path A — No dead stores (fast path):**
Selected when `done:` does not read `%State` (no post-loop hoisted
guards AND exit condition does not reference state fields). The loop
body emits ZERO stores to `%State`. The phi registers (A005c) or
pre-loaded values (A005d) carry all values. The latch's native
backedge path (lines 1093-1098 of loop_engine.rs) provides the
phi successor values directly from computed registers.

```llvm
; Before (dead store):               ; After (eliminated):
body:                                 body:
  %x0_new = fadd ...                    %x0_new = fadd ...
  store %x0_new → %state      ← DEAD   ; (no store)
  ...guard/fprintf...                    ...guard/fprintf...
latch:                                latch:
  %be_x0 = fadd %x0_new, 0.0           %be_x0 = fadd %x0_new, 0.0
  br loop_hdr                           br loop_hdr
```

**Path B — Stores preserved (compatibility path):**
Selected when `done:` reads `%State` (post-loop hoisted guards OR
exit condition references state). Stores are emitted as before,
ensuring the `done:` block's GEP+load sees fresh values. This is
the fallback for cases like `term! -> print_int#(result)` where
the final value must be printed after the loop exits.

```llvm
body:
  %x0_new = fadd ...
  store %x0_new → %state      ← needed for done:
  ...compute...
latch:
  %be_x0 = fadd %x0_new, 0.0
  br loop_hdr
done:
  %val = load %state.x0        ← reads the stored value
  call print_int#(%val)
  ret
```

### Decision rule

```
needs_state_stores_in_body =
    !self.fun.pending_post_hoist.is_empty()
    || exit_condition_references_state(...)
```

When `false` → Path A (stores suppressed).
When `true`  → Path B (stores emitted).

### Impact on caches and metadata

Even when stores are suppressed (Path A), the backend must still
update the phi backedge tracking (`pending_phi_backedge`,
`pending_phi_native_backedge`, `ssa_old_*_regs`) and perform cache
invalidation. Only the final `store` instruction is omitted.

This is because:
- `pending_phi_backedge` tells the latch which fields were modified
- `pending_phi_native_backedge` provides the register value for the
  latch backedge
- Cache invalidation ensures subsequent reads within the same body
  see updated values
- ALL of these are about intra-body correctness, not about persisting
  values to `%State` for the next iteration or for `done:`

## Implementation

### 1. Add field to FunctionContext (context.rs)

```rust
/// 2026-07-04: Whether the hot loop body must emit stores to %State
/// for the done: block to read. When false (the common case), stores
/// are suppressed — phi registers / pending_phi_native_backedge carry
/// values forward. When true (post-loop hoisted guards exist or exit
/// condition reads state), stores are emitted so done:'s GEP+load sees
/// fresh values.
///
/// Dual-path rationale:
///   Path A (false): Zero memory traffic in hot loop. LLVM's optimizer
///     sees a clean phi loop with no barriers. Enables full vectorization
///     and ILP scheduling.
///   Path B (true): Stores preserved for correctness of post-loop code.
///     Slightly more IR but required when the done: block reads state.
///
/// Both paths must be preserved when refactoring — removing Path A
/// regresses all benchmarks by 9-31 dead stores per iteration; removing
/// Path B breaks term! swan song correctness.
pub needs_state_stores_in_body: bool,
```

Default: `true` (conservative — preserves existing behavior).

### 2. Gate stores in emit_memory_field_store (emit_stmt.rs)

In `emit_memory_field_store`, wrap the `store` instructions in an
`if self.fun.needs_state_stores_in_body { ... }` block. Everything
else (cache tracking, phi backedge tracking) remains unconditional.

The three store sites:
1. Integer/pointer types (line 52-53): `store{} {} {}, ptr {}`
2. Native float/double (line 65-66): `store{} {} {}, ptr {}`
3. Volatile stores (line 75): always emitted (MMIO)

Volatile stores are excluded from the gate — they have actual side
effects and must always be emitted.

### 3. Set the flag in emit_countable_main (loop_engine.rs)

After the body and latch are set up but before emission starts:

```rust
// 2026-07-04: Determine whether done: needs stores in body.
// Path A (stores suppressed): no post-loop hoisted guards, clean exit.
// Path B (stores preserved): post-loop hoisted guards need state.
let needs_stores = !self.fun.pending_post_hoist.is_empty();
self.fun.needs_state_stores_in_body = needs_stores;
```

Reset to `true` after the function is complete.

### 4. Set the flag in emit_countable_memory_main (loop_engine.rs)

Same pattern as `emit_countable_main`.

### 5. Reset flag after loop emission

In both `emit_countable_main` and `emit_countable_memory_main`, after
the done: block:

```rust
self.fun.needs_state_stores_in_body = true;
```

## Architecture Comments

### At the flag definition (context.rs)

Explain the dual-path rationale, what each path is for, and why both
must be preserved. Include the benchmarks that benefit from each path.

### At the gate site (emit_stmt.rs)

Explain WHY stores are suppressed, what Path A vs Path B means, and
what the user will observe if either path is removed.

### At the decision site (loop_engine.rs)

Explain how the decision is made and what conditions trigger Path B.

## Tests and Verification

1. All 1393 existing tests pass with both paths.
2. `cargo test --lib -- backend::tests` passes.
3. Benchmarks with post-loop hoisted guards (if any) produce correct output.
4. Benchmarks without post-loop hoisted guards produce correct output AND
   show improved performance.

## Benchmarks and Expected Impact

| Benchmark | State fields | Current ratio | Expected after fix |
|-----------|-------------|---------------|-------------------|
| float_math_nonzero | 7 | 2.09× | ~0.8× (7 stores removed, vectorization unblocked) |
| float_math | 7 | 0.85× | ~0.7× (cleaner phi loop) |
| nbody_sqrt | 31 | 1.29× | ~1.1× (31 stores removed) |
| nbody_newton | 31 | 1.46× | ~1.2× (31 stores removed) |
| ring_buffer | 3 | 0.97× | ~0.7× (3 stores removed) |
| cancel_math | 1 | 0.97× | ~0.7× (1 store removed) |
| fasta | 2 | 0.99× | ~0.8× (2 stores removed) |

All benchmarks with a `[count % N == 0] { print_int#(...); }` guard
(the common pattern) use the guard-print INSIDE the loop body, not
a post-loop hoisted guard. So they all benefit from Path A.

Benchmarks using `term! -> print_int#(result)` (swan song pattern)
would use Path B. Currently no benchmark in the suite uses this
pattern, but the path is preserved for correctness.

## Why Not Revert to A005a (Struct-SSA)

The original A005a approach (ONE `store %State` at latch) also had
zero dead stores after SROA+mem2reg. However:

1. A005a required SROA to decompose the single struct phi into
   per-field phis. This added compile time and created a fragile
   dependency on LLVM's optimization pipeline.
2. A005a's `extractvalue`/`insertvalue` chain produced verbose IR
   that was harder to debug.
3. The per-field phi approach (A005c) avoids SROA entirely — the
   phis are direct SSA values from the start.
4. With this fix, A005c achieves the same zero-traffic hot loop as
   A005a+SROA, without the SROA dependency.

The correct fix is to remove the unnecessary stores from A005c/A005d,
not to revert to A005a.

## Risk Assessment

**Low risk.** The change is additive: a new `bool` field that gates
store emission. When `true` (default), behavior is identical to
current. When `false`, stores are suppressed but all other bookkeeping
(phi backedge tracking, cache invalidation, ssa_old tracking) continues
as before.

The only correctness risk is if some code path reads `%State` after the
loop body but before `done:` — but no such path exists because:
- The loop header compares the counter phi against the bound
- The body uses phi registers / pre-loaded values
- The latch uses native backedge registers
- `done:` is the only code that reads `%State`, and it only does so
  when `needs_state_stores_in_body` is true

## File Changes Summary

| File | Change |
|------|--------|
| `src/backend/llvm/context.rs` | Add `needs_state_stores_in_body: bool` field with doc comment |
| `src/backend/llvm/emit_stmt.rs` | Gate `store` instructions on `needs_state_stores_in_body` |
| `src/backend/llvm/loop_engine.rs` | Set `needs_state_stores_in_body` in `emit_countable_main` and `emit_countable_memory_main` |

Total: ~40 lines added, 0 deleted.
