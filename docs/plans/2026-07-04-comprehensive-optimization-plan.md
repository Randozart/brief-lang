# Comprehensive Optimization Plan — Phase 3+

## Status Summary

Current: Brief wins 8, C wins 8 (of 18 benchmarks).
Target: Brief wins 14+, C wins 2-3 (only fannkuch_redux fencepost and
        irreducible gaps).

## Gap Analysis

### Gaps we closed (2026-07-04)
- A005c dead stores eliminated (ring_buffer .88×, float_math .78×, bit_clear .68×)
- Benchmark intrinsic cleanup (removed dead frgn __print_int)

### Remaining gaps

| Benchmark | Ratio | Root cause | Fix | Expected |
|-----------|-------|------------|-----|----------|
| float_math_nonzero | 2.12× | Sequential in-place mutation creates serial dependency chain. `&x0 = ...; &x1 = A10*new_x0 + ...` prevents SIMD vectorization because x1 depends on new x0. | Parallel-update detection | ~0.8× |
| nbody_newton | 1.45× | A005d memory loop (31 fields > 8 threshold). Counter loaded from %State at header every iteration. Stores to all fields in body. No per-field phis. | Counter phi in A005d | ~1.1× |
| nbody_sqrt | 1.26× | Same as nbody_newton. | Same | ~1.1× |
| nbody_sqrt_idio | 1.05× | Same + idiomatic structure slightly different. | Same | ~0.9× |
| fannkuch_redux | 1.76× | Pre-existing fencepost bug (6 vs 10 lines). Codegen is correct for what it compiles. | Bug fix in Brief source | ~1.0× |
| mandelbrot | 1.09× | Small gap, likely from loop structure differences. | Minor tuning | ~1.0× |
| print_loop | 1.07× | Small gap, noise-dominated. | Minor tuning | ~1.0× |
| queue_drain_sym | 1.04× | Small gap. | Minor tuning | ~1.0× |

## Optimization 1: Counter Phi in A005d

### Problem

A005d (memory loop for >8 fields) loads the counter from %State via GEP
at the loop header every iteration. This means:
1. The counter field MUST have a body store (so the header load sees fresh data)
2. All non-counter fields also have body stores (pre_load_all_fields runs at
   body start, loading from %State)
3. The needs_state_stores_in_body flag cannot help because the header load needs
   the counter store, and pre_load_all_fields needs the non-counter stores

### Fix

Make the counter a phi node in A005d — same as A005c. The header loads the
counter INITIALLY from %State (for the first iteration), then uses a phi for
subsequent iterations.

```llvm
_hdr:
  %cnt_phi = phi i64 [ %init_cnt, %entry ], [ %cnt_next, %latch ]
  %cmp = icmp slt %cnt_phi, %bound
  br %cmp, _body, _done

_body:
  pre_load_all_fields(%state)   ← still needs stores for non-counter fields
  ...
  store %counter_new → %state.counter  ← STILL NEEDED for header's init load
  ...all other field stores...

_latch:
  %cnt_next = add %cnt_phi, 1
  br _hdr
```

Wait — the counter store is still needed at THE FIRST ITERATION (the `%entry`
incoming value of the phi comes from %State). But once the phi is established,
the header doesn't need the counter store anymore (it uses `%cnt_next` from
the latch).

So the counter store is needed for the FIRST iteration only. After that, the
phi carries it forward. We could:
1. Load the counter once before the loop (entry), then use a phi
2. The body's counter store is dead from iteration 2 onwards
3. LLVM's DSE should eliminate it

This is NOT a complete solution for A005d. The non-counter fields still need
stores because pre_load_all_fields loads them at body start.

But wait — we could also eliminate pre_load_all_fields from A005d. Instead of
loading from %State at body start, we could use phi nodes for ALL fields, same
as A005c. This would make A005d identical to A005c in terms of the phi loop
structure, just with more phis.

The reason A005d was created was the concern about 31 phi nodes causing SROA
problems. But with chunk allocas (≤15 per chunk), SROA decomposes each chunk
independently. And since we already have the dead-store fix for A005c, the
per-field phi approach with 31 fields should work fine.

**Better fix**: Remove A005d entirely. Use A005c for ALL field counts. The
31-phi concern was based on pre-chunk-alloca monolithic %State. With chunk
allocas, SROA handles each chunk separately. And Path A (no dead stores)
applies to all A005c loops.

### Decision

Remove A005d dispatch threshold. Use A005c for all field counts. Then:
- All benchmarks get per-field phi loop with Path A (zero stores in body)
- Nbody benchmarks (31 fields) get the same clean phi loop as ring_buffer
- Counter is a phi → no header GEP+load → no counter store needed

This is the SINGLE BIGGEST change. It eliminates:
- A005d code path (less code to maintain)
- 31 dead stores per iteration for nbody
- The header GEP+load for the counter
- pre_load_all_fields call at body start

### Trade-off

- A005c with 31 fields generates 31 phi nodes. These create 31 backedge
  registers. In A005d with 1 phi + GEP+load for 30 fields, there's 1 phi
  and 30 GEP+load pairs. The per-field phi approach has MORE SSA values
  but ZERO memory traffic. LLVM's optimizer strongly prefers SSA values
  over memory traffic.
- Compile time may increase slightly (more phi nodes for SROA/mem2reg).
  But with chunk allocas, this is negligible.

## Optimization 2: Parallel-Update Detection

### Problem

Brief's sequential mutation semantics create serial dependency chains
that prevent SIMD vectorization. Example:

```
&x0 = A00*x0 + A01*x1 + A02*x2   ; computes new x0 from old x0, x1, x2
&x1 = A10*x0 + A11*x1 + A12*x2   ; uses NEW x0 (depends on result of above)
```

The second assignment depends on the first because `x0` was mutated. This
creates a serial chain: compute x0 → compute x1 → compute x2.

In contrast, the C reference computes all three independently:
```c
float nx0 = A00*x0 + A01*x1 + A02*x2;  // uses old x0
float nx1 = A10*x0 + A11*x1 + A12*x2;  // uses old x0 (nx0 not assigned yet)
float nx2 = A20*x0 + A21*x1 + A22*x2;  // uses old x0, x1
x0 = nx0; x1 = nx1; x2 = nx2;
```

LLVM's vectorizer can parallelize the three independent FMAs across SIMD
lanes. With the serial chain, it cannot.

### Fix: Simple dataflow analysis

In the emit_stmt body emission path, when we encounter:
```
&f = expr(field1, field2, ..., fieldN)
```

If every `fieldX` reference resolves to a phi register (from
ssa_old_float_regs / ssa_old_int_regs), then the assignment is using
"old" values (before the loop iteration's mutations). The new value
can be computed from old values regardless of order.

If we detect that ALL assignments in the body follow this pattern, we
can emit the body as a "parallel block": compute all new values into
temporary registers, then store all at once. This mirrors C's pattern
of computing into locals then assigning.

But this is a complex analysis for limited gain (only float_math_nonzero
benefits significantly). A simpler approach:

**Auto-temporaries**: When the body has:
```
&f1 = expr1(...)
&f2 = expr2(..., f1, ...)  ← reads f1 after &f1
```

Emit `&f1 = expr1(...)` with a normal store (since f1 is "committed").
But for `&f2`, if the RHS reads f1, use `phi_f1` (the OLD value) instead
of the stored value. This way, all computations use old values, and the
stores happen sequentially but the computations are independent.

Wait — this already happens! In A005c, reads use `ssa_old_float_regs` which
contain the phi values (old values). The stores go to %State but reads use
phi registers. So `&x1 = A10*x0 + ...` reads x0 from the phi register,
not from the store. The computations ARE independent!

Let me verify this. In the current IR for float_math_nonzero:
```
body:
  %bfr147 = fmul fast %phi_x0, %A00  ← uses phi_x0 (old value)
  ...
  store %bfr147 → %state_0            ← store NEW x0
  ...
  %bfr158 = fmul fast %bfr147, %A10  ← uses %bfr147 (NEW x0, not phi_x0!)
```

Wait — %bfr158 uses %bfr147 which is the NEW x0 (the result of `fadd fast` for
x0's computation). But that's the computed new value, not a load from %State.

Actually, the issue is different. Let me re-read the source:
```
29:     &x0 = A00 * x0 + A01 * x1 + A02 * x2;
30:     &x1 = A10 * x0 + A11 * x1 + A12 * x2;
```

For line 30, `x0` refers to the CURRENT value of x0. Since line 29 already
mutated it, `x0` on line 30 is the NEW value.

The backend emits `&x0 = ...` as:
1. Compute `%bfr147 = new_x0` (using phi_x0, phi_x1, phi_x2)
2. Store `%bfr147 → %state.x0`
3. Register `%bfr147` as the ssa_old value for x0

Then for `&x1 = ...`:
1. Read `x0` from `ssa_old_float_regs["x0"]` which is `%bfr147` (the NEW value)
2. Compute using `%bfr147` instead of `phi_x0`

So the backend intentionally uses the new x0 for subsequent computations.
This is CORRECT Brief semantics — `&x0 = ...` mutates x0, subsequent uses
see the new value.

The problem is that this creates a data dependency. The fix would be to
DETECT that all new values can be computed from old values, and emit
temporaries. But this requires:

1. Analyzing the assignment chain to find the dependency graph
2. If the graph has no cycles, compute all new values from old values in
   parallel, then store all at once
3. If the graph has cycles (e.g., `&a = b; &b = a`), use sequential updates

For float_math_nonzero:
- x0 depends on: old x0, old x1, old x2 (NO dependency on new x1, new x2)
- x1 depends on: NEW x0, old x1, old x2 (depends on new x0)
- x2 depends on: NEW x0, NEW x1, old x2 (depends on new x0, new x1)

The dependency graph is: x0 → x1 → x2 (serial chain).

If we detect this, we can compute:
```
let nx0 = A00*x0 + A01*x1 + A02*x2;    // all old
let nx1 = A10*x0 + A11*x1 + A12*x2;    // all old (ignore &x0 for RHS)
let nx2 = A20*x0 + A21*x1 + A22*x2;    // all old
&x0 = nx0; &x1 = nx1; &x2 = nx2;
```

But this changes semantics — the C reference does this, but Brief's semantics
say x1 should use new x0. If the user wrote this pattern intentionally to
express Gauss-Seidel iteration, parallel-update would break it.

**Decision**: Do NOT add parallel-update detection. It's a semantic change.
The user's code IS sequential for a reason. If the benchmark author wanted
parallel semantics, they'd write `let nx0 = ...; let nx1 = ...; &x0 = nx0; &x1 = nx1;`.

Instead, focus on making the sequential phi loop as fast as possible (which
we already do — the phi loop with Path A has zero memory traffic). The
sequential dependency is a fundamental Brief semantics choice.

The remaining gap (2.12×) is because C auto-vectorizes the three independent
computations. Brief cannot do this because the semantics are different. The
gap is inherent, not a codegen regression.

**Remove from scope**: This optimization changes Brief semantics. Drop it.

## Optimization 3: Counted-Down Loop

### Problem

Current A005c loop:
```llvm
loop_hdr:
  %pi_cnt = phi i64 [ %init_count, %pre_phi ], [ %pn_cnt, %latch ]
  %cmp = icmp slt i64 %pi_cnt, %bound
  br %cmp, body, done
body:
  ...
latch:
  %pn_cnt = add i64 %pi_cnt, 1
  br loop_hdr
```

This uses `icmp` + `add` — two instructions for the counter. A counted-down
loop uses `sub` which sets the ZF (zero flag), allowing a single instruction
for the exit check on x86:

```llvm
loop_hdr:
  %rem_phi = phi i64 [ %init_remaining, %pre_phi ], [ %rem_next, %latch ]
  %cmp = icmp sgt i64 %rem_phi, 0
  ; or just: %rem_next = sub i64 %rem_phi, 1 ; br if not zero
  br %cmp, body, done
latch:
  %rem_next = sub i64 %rem_phi, 1
  br loop_hdr
```

Clang emits counted-down loops for C for-loops. The benefit is:
- `sub` sets ZF (x86), eliminating `cmp`
- The counter value decreases to 0, which LLVM's IV analysis prefers
- Simpler backedge: `sub` + `br` instead of `add` + `icmp` + `br`

### Fix

In `emit_countable_setup_phis_and_header` and `emit_countable_latch`:
1. Compute `init_remaining = bound - init_count` at loop entry
2. Use `%rem_phi` counting down to 0
3. Exit check: `icmp sgt %rem_phi, 0` (or just check if sub result is 0)
4. Latch: `%rem_next = sub i64 %rem_phi, 1`
5. After loop: store final counter = bound to %State (if needed)

The counter phi needs to track `remaining` not `count`. The actual counter
value for body use is `count = bound - remaining`, but we compute this from
the phi: `%count = sub i64 %bound, %rem_phi`.

Actually, this adds an extra `sub` per body access. Better approach: keep
the counter as-is but change the exit check to use `sub` setting flags.

On x86, `add i64 %pi_cnt, 1` followed by `icmp slt %pn_cnt, %bound` is:
```
add rax, 1       ; single uop
cmp rax, rbx     ; single uop (sets flags)
jl loop_hdr      ; single uop (reads flags)
```

With counted-down:
```
sub rax, 1       ; single uop (sets ZF)
jg loop_hdr      ; single uop (reads flags)
```

That's 2 uops vs 3 uops — saves 1 uop per iteration. At 50M iterations, this
is ~0.03s on a 3GHz CPU with 4 uop/cycle throughput. The benefit is marginal
(~1-2%).

### Decision

Implement as a micro-optimization after the big items. Low priority but
trivial to do.

## Optimization 4: Clean Up Latch Identity Operations

### Problem

The latch emits:
```
%be_x0 = fadd float %bfr147, 0.0
%be_total = add i64 0, %phi_total
```

These are identity operations (`x + 0 = x`) that LLVM's optimizer eliminates.
But they bloat the IR by 2N instructions (N = field count).

### Fix

In `emit_countable_latch`, for modified fields with native backedge:
```rust
// Instead of:
writeln!(out, "  {} = fadd float {}, 0.0", be_reg, typed_reg)
// Emit:
writeln!(out, "  {} = add i64 0, {}", be_reg, typed_reg)  // or just assign
```

Actually, we can't just skip the instruction — LLVM requires each register
to be defined exactly once. The backedge register IS defined here and used
in the phi. We could use the register directly in the phi:

Instead of:
```llvm
%be_x0 = fadd float %bfr147, 0.0
, [ %be_x0, %latch ]
```
Use:
```llvm
, [ %bfr147, %latch ]
```

But this requires changing the phi node construction to use the computed
register directly instead of a dedicated backedge register. This is a deeper
change to `emit_countable_setup_phis_and_header`.

### Decision

Skip this for now. The identity operations are trivially eliminated by
LLVM's optimizer. Zero runtime impact. Only matters for IR readability.

## Optimization 5: Arena Allocator Dead-Code Elimination

### Problem

The arena allocator setup/teardown is emitted for EVERY A005c/A005d loop,
even when the body has no collection operations (`<- push`, `<- pop`, etc.).
This adds ~30 IR instructions for the arena setup and ~5 for the teardown.

### Fix

Before emitting arena init, scan the body for arrow operations. If none
exist, skip arena init and fini. Simple: check if `collect_push_targets`
returns empty.

### Decision

Low priority. The arena overhead is negligible (~0.01s for 50M iterations).
Skip for now.

## Implementation Order

1. **Remove A005d, use A005c for all field counts** — biggest gain (nbody)
   - Remove the `num_fields > 8` dispatch
   - A005c Path A (no dead stores) applies to all fields
   - All fields get per-field phis with zero memory traffic

2. **Counted-down loop** — small but consistent gain
   - Change counter to count down
   - Save 1 uop per iteration

3. Clean up identity operations and arena allocator (nice-to-have)

### Why remove A005d entirely instead of fixing it

A005d exists for a concern (31 phis choke SROA) that was solved by chunk
allocas (≤15 per chunk). Chunk allocas mean SROA sees 2-3 independent
structs of ≤15 fields each — well within SROA's 64-element threshold.
With Path A applied to A005c for ALL field counts, the 31-phi case has
ZERO memory traffic. A005d with GEP+load/store has 31 GEP+load+store
operations per iteration. The per-field phi path is strictly better.

Keeping A005d is dead weight — two code paths to maintain, and the
"optimization" (memory over phis) is actually a pessimization.
