# Optimization Plan — 2026-07-07

## Baseline: Commit c9b017d

All 22 benchmarks MATCH (0 MISMATCH, 1 SKIP). Run at BOUND=50000000, 5 iterations.

| Benchmark | Ratio | Briv Time | C Time | Dispatch | Fields | Status |
|-----------|-------|-----------|--------|----------|-------|--------|
| float_math | **0.84x** | .0636s | .0757s | A005c | 13 | **BEATS C** |
| nbody_sqrt | **0.70x** | 2.0985s | 2.9834s | A005c | 31 | **BEATS C** |
| nbody_newton | **0.72x** | 6.2740s | 8.6777s | A005c | 32 | **BEATS C** |
| nbody_sqrt_idio | **0.75x** | 2.9553s | 3.9158s | A005c | 31 | **BEATS C** |
| ring_buffer | **0.98x** | .0643s | .0653s | A005c | 2 | **BEATS C** |
| fasta | **0.99x** | .2290s | .2310s | A005c | 2+FFI | **BEATS C** |
| print_loop | **0.93x** | .0635s | .0676s | A005c | 2 | **BEATS C** |
| mandelbrot | **1.00x** | .6864s | .6859s | A005c | 10 | ~tie |
| knucleotide | **1.00x** | .1935s | .1908s | A005c | 4 | ~tie |
| kalman_filter | **0.99x** | .1805s | .1812s | A005c | 13 | ~tie |
| queue_drain | **1.00x** | .0619s | .0618s | A005c | 2 | ~tie |
| queue_drain_sym | **1.03x** | .0631s | .0608s | A005c | 2 | ~tie |
| fannkuch_redux | **1.04x** | .0698s | .0671s | A005c | 16 | **~tie** |
| float_math_nonzero | **1.02x** | .1772s | .1730s | A005c | 7 | ~tie |
| bit_clear | **1.00x** | .0006s | .0006s | A005c | 2 | Precomputed |
| cancel_math | **1.07x** | .0668s | .0620s | A005c | 2 | **LOSES** |
| sparse_dispatch | **1.36x** | .0903s | .0663s | Mod-switch | 2 | **LOSES** |
| interval_step | **0.01x** | .0008s | .0612s | A000 (fold) | 2 | Precomputed |
| queue_drain_idio | SKIP | — | — | — | — | No C binary |

## Target 1: sparse_dispatch (1.36x → estimate 0.95x)

**Problem**: The modulo-switch rotated loop emits all 8 transaction bodies in
sequence, each with full transaction overhead.  C eliminates the empty switch
and runs just `count++` with a 5M-interval print guard.

**Root cause**: `try_modulo_switch_dispatch` in `loop_engine.rs` detects the
`count % K` pattern and emits a rotated loop.  Each of the 8 bodies does:
- Load count from %State
- Increment (body's own `&count = count + 1`)
- Check print condition (every 5M)
- Store count back to %State

The rotated loop infrastructure ALSO increments tracking variable between
bodies.  Result: 8x more work than necessary.

**Fix**: Detect when all K modulo-switch bodies are effect-equivalent:
- All bodies produce the same observable side effects (same prints at same
  intervals)  
- The only difference is the guard condition modulo value, which is handled
  by the rotated loop framework

When effect-equivalent, emit a SINGLE body with `count += K` and the print
guard at the correct interval.  The `srem` for the modulo check is hoisted
out of the body.

**Implementation**:
1. Add `bodies_are_equivalent()` check in `try_modulo_switch_dispatch`
2. When equivalent: emit one body + `count += K` per trip
3. Handle print guard: check instead of `count % 5000000 == 0`, check
   `count % (5000000 * K) == 0`

**Risk**: Determining effect-equivalence requires comparing two reactive
transaction bodies.  For the sparse_dispatch case, all 8 are identical
(just different modulo guards).  But for general cases, bodies may differ
in their internal state transitions, making equivalence hard to prove.

## Target 2: cancel_math, print_loop (1.07x, 0.93x → both estimate ~1.00x or better)

**Problem**: 2-field txns dispatched via A005c (per-field phi loop).  The phi
infrastructure adds ~10 instructions per iteration (phi header, backedge, latch,
exit check).  For a 2-field body that does almost nothing, the overhead dominates.

**Root cause**: The dispatch decision tree at `mod.rs:2265` selects A005a when:
```rust
if write_density >= 0.5 && total_fields < 8 && !has_body_ffi {
    // A005a: inline SSA with insertvalue chain
}
```

cancel_math has 2 fields, 100% write density, no body FFI — it SHOULD get
A005a.  But it gets A005c.  This means the check is not reached for
`node` + `#!exit` pattern (reactive convergent txns may skip the A005a
path).

**Fix**: Diagnose why cancel_math misses A005a dispatch.  Check the dispatch
flow for `node step [count < N][count == N]` pattern.  The A005a check is
inside `emit_countable_main` — if `node` goes through a different dispatch
path (like the reactive loop), the check is never reached.  If so, widen the
A005a check to cover reactive txns that are also countable.

print_loop (0.93x) already BEATS C but is close; A005a could improve it
further.  print_loop has body FFI (`print_int#`) which blocks A005a.

**Implementation**:
1. Trace the dispatch path for `node step` with `#!exit`
2. If it bypasses the A005a check, add A005a for reactive countable txns
3. Ensure the insertvalue chain works with the convergent loop pattern

## Target 3: fannkuch_redux — Pure-SSA Rotation (1.04x → estimate 0.70x)

**Problem**: The rotation detection (12-field circular shift) uses GEP-reload
for ALL modified fields, adding stores + loads through %StateChunk allocas.
Currently 1.04x (essentially tied with C), but should be faster — the rotation
is pure arithmetic that can be expressed in SSA.

**Root cause**: The latch backedge for non-rotation fields (seed, checksum,
max_flips) references registers defined in the last unrolled body copy,
which doesn't dominate the latch (the latch is reachable from any copy's
overflow guard).  This causes an SSA dominance violation:
```
Instruction does not dominate all uses!
  %be_checksum = add i64 0, %t407
```
where `%t407` is defined in `body_rot3` but `%be_checksum` is in `latch`
(also reachable from `body_rot1` and `body_rot2`).

**Approach A**: Phi nodes at the latch
For each non-rotation field F, emit a phi at the latch that merges the
values from all predecessor body copies:
```
latch:
  %latch_F = phi i64 [%F_c1, %body], [%F_c2, %body_rot1], [%F_c3, %body_rot2], [%F_c4, %body_rot3]
  %be_F = add i64 0, %latch_F
```
This requires tracking the per-copy register names for each field.  Add a
`Vec<(String, String)>` per field that records the register defined in each
copy.  After the unrolling loop, emit phis from these Vecs.

**Approach B**: Single copy, no unrolling
Don't unroll the body (rotation_step=1, latch_inc=1).  Rotation fields use
pending_phi_native_backedge (circular phi chain).  Non-rotation fields also
use pending_phi_native_backedge.  Single body copy → all pending registers
come from the same block which dominates latch.  Trade-off: 4× more loop
trips, but each trip is much lighter.

From yesterday's experiment: single copy gave 1.83x (0.1242s) — worse than
the baseline.  The 4× extra loop overhead exceeds the memory savings.

**Recommendation**: Approach A (phi nodes at latch).  This keeps the unrolling
(counter inc by 4, 12.5M trips) while eliminating stores for rotation fields
(12 of 16 fields).  Only non-rotation fields (seed, checksum, max_flips,
count = 4 fields) need GEP-reload or phi merge.

**Implementation**:
1. In `detect_rotation_ast` result, mark which fields are in the rotation cycle
2. In `emit_countable_main`: for rotation fields, DON'T add to `rotation_fields`
   (no stores, no GEP-reload).  For non-rotation fields, add to `rotation_fields`
   as before.
3. In the unrolling loop: for rotation fields, update ssa_old from
   `pending_phi_native_backedge` between copies (so the next copy sees
   rotated values).  For non-rotation fields, keep GEP-reload.
4. In the latch: for rotation fields, use `pending_phi_native_backedge`
   (circular phi reference — safe because phi registers dominate latch).
   For non-rotation fields, keep GEP-reload.
5. The key constraint from yesterday: rotation fields in
   `pending_phi_native_backedge` contain PHI REGISTER names (e.g.,
   `%phi_p4` for p0 after 4 copies).  These are defined in the header
   which dominates latch → safe.  Non-rotation fields in
   `pending_phi_native_backedge` contain BODY-COMPUTED registers (e.g.,
   `%iv456` for seed).  These do NOT dominate latch → must use GEP-reload.

## Target 4: fasta (0.99x already winning — low priority)

fasta already BEATS C at 0.99x.  No optimization needed.

## Implementation Order

1. **cancel_math A005a dispatch** (Target 2) — low risk, quick win
2. **sparse_dispatch effect-equivalent** (Target 1) — medium risk, big win
3. **fannkuch phi-merge rotation** (Target 3) — higher risk, medium win

## Verification

After each optimization:
1. `cargo test --lib` — all 1403 pass
2. `bash benchmarks/build_and_bench.sh --runtime` — all 22 MATCH
3. Target benchmark ratio should improve; no other benchmark regresses >3%
