# Next Optimization Targets

Date: 2026-07-06
Status: Plan

## 1. Current Benchmark Results (commit f598584)

All 22 benchmarks MATCH (0 MISMATCH, 1 SKIP). Ratios are Briev/C — lower is better.

| Benchmark | Ratio | Briev Time | C Time | Dispatch | State Size | Notes |
|-----------|-------|-----------|--------|----------|-----------|-------|
| interval_step | **0.01x** | .0006s | .0622s | A000 (fold) | 2 fields | Precomputed — trivial |
| bit_clear | **1.00x** | .0006s | .0006s | A005c (SSA) | 2 fields | Precomputed — 63 iters |
| mandelbrot | **1.00x** | .7236s | .7232s | A005c | 10 fields | ~tie |
| knucleotide | **1.00x** | .2006s | .2004s | A005c | 4 fields | ~tie |
| kalman_filter_runtime | **1.00x** | .1844s | .1836s | A005c | 13 fields | ~tie |
| queue_drain | **1.00x** | .0627s | .0621s | A005c | 2 fields | ~tie |
| queue_drain_sym | **1.01x** | .0621s | .0614s | A005c | 2 fields | ~tie |
| ring_buffer | **0.99x** | .0664s | .0666s | A005c | 2 fields | ~tie |
| float_math | **0.84x** | .0626s | .0739s | A005c | 13 fields | **BEATS C** |
| nbody_sqrt | **0.91x** | 2.9267s | 3.2106s | A005c | 31 fields | **BEATS C** |
| nbody_newton | **0.76x** | 7.2391s | 9.4519s | A005c | 32 fields | **BEATS C** |
| nbody_sqrt_idio | **0.75x** | 3.0738s | 4.0939s | A005c | 31 fields | **BEATS C** |
| float_math_nonzero | **1.03x** | .1877s | .1809s | A005c | 7 fields | **LOSES to C (-3%)** |
| cancel_math | **1.05x** | .0667s | .0630s | A005c | 2 fields | **LOSES to C (-5%)** |
| print_loop | **1.08x** | .0710s | .0653s | A005c | 2 fields | **LOSES to C (-8%)** |
| fasta | **1.01x** | .2538s | .2497s | A005c | 2 fields+FFI | **LOSES to C (-1%)** |
| sparse_dispatch | **1.52x** | .1006s | .0659s | Modulo-switch | 2 fields | **LOSES to C (-52%)** |
| fannkuch_redux | **1.44x** | .1036s | .0717s | A005c+rotation | 16 fields | **LOSES to C (-44%)** |
| queue_drain_idio | SKIP | — | — | — | — | No C binary |

### Winners (Briev beats C)
- **nbody_newton**: .76x (24% faster) — vector phis + force computation
- **nbody_sqrt_idio**: .75x (25% faster) — vector phis + idiomatic txn
- **nbody_sqrt**: .91x (9% faster) — sqrt in hot loop offsets vector phi benefit
- **float_math**: .84x (16% faster) — tight float loop, well-vectorized
- **ring_buffer**: .99x — within noise of C

### Losers (C beats Briev)
- **sparse_dispatch**: 1.52x — Briev does 8 bodies' work; C eliminates empty switch
- **fannkuch_redux**: 1.44x — Briev uses GEP-store-reload for rotation; C uses pure SSA
- **print_loop**: 1.08x — 2-field A005c overhead > 2-field C loop
- **cancel_math**: 1.05x — same issue as print_loop
- **float_math_nonzero**: 1.03x — within noise (likely variance)
- **fasta**: 1.01x — PutChar forces A005c (was .83x with wrong A005a dispatch)

## 2. Priority Targets

### Target 1: fannkuch_redux — Pure-SSA Rotation (estimated impact: 1.44x → <1.0x)

**Current codegen**: `detect_rotation_ast` finds the 12-field circular shift (p0←p1←...←p11←p0). With `rotation_step=4`, the backend unrolls the body 4× and uses GEP+store+reload through `%StateChunk` allocas to "break the phi chain for SCEV analysis." Result: ~64 stores + ~52 loads per loop trip, all through chunk allocas that SROA cannot decompose.

**C codegen**: clang at -O3 promotes all 12 local variables to SSA registers via mem2reg. The rotation becomes a pure phi chain: `%p0_new = phi [%init_p0, %entry], [%p1_old, %latch]`. Zero memory traffic. ~15 instructions in the hot loop: the LCG arithmetic + checksum accumulation.

**Fix**: When `detect_rotation_ast` finds a rotation cycle and all rotation fields are ONLY consumed within the rotation (no observable reads by FFI), emit a **circular phi chain** instead of the GEP-reload path. The phi backedge for each field takes the previous element's phi value:

```llvm
; Instead of: %phi_p0 = phi [init, %pre_phi], [%reload_p0, %latch]
; Emit:      %phi_p0 = phi [init, %pre_phi], [%phi_p1, %latch]
;            %phi_p1 = phi [init, %pre_phi], [%phi_p2, %latch]
;            ...
;            %phi_p11 = phi [init, %pre_phi], [%phi_p0, %latch]
```

**Prerequisites**:
- The rotation must be pure (no FFI reads the rotated values — true for fannkuch)
- The number of fields must be manageable (< 20 or so)
- The phi backedge must produce analyzable SCEV expressions

**Implementation**:
1. Add a `PureRotation` flag or enum variant to A005c
2. In `emit_countable_setup_phis_and_header`, emit the circular backedge references
3. Skip the rotation unrolling entirely (don't force body stores)
4. Handle the LCG+checksum computation normally within the body

**Risk**: The circular phi chain produces mutually-referencing φ nodes, which LLVM must resolve via cycle analysis. LLVM handles this fine (it's what clang produces), but some SCEV analyses might give up on certain expressions.

### Target 2: sparse_dispatch — Effect-Equivalent Collapse (estimated impact: 1.52x → 1.0x)

**Current codegen**: `try_modulo_switch_dispatch` detects the 8-txn modulo pattern (count % 8) and emits a rotated loop with all 8 bodies in sequence. Each body increments `count`, checks the print guard, and branches. This means **8 bodies of work per 8-count cycle**.

**C codegen**: clang recognizes that all 8 switch cases are empty (`break;`), eliminates the entire switch, and emits a simple counted loop with only the `count++` and the 5M-interval print guard. The C hot loop is 2 instructions: `add count, 1` + `cmp count, bound` + `jl`.

**Fix**: During `try_modulo_switch_dispatch`, analyze whether all K transaction bodies are **effect-equivalent**. For a body to be a no-op besides the mandatory counter increment:
- All assignments must be either dead (field never read by observable output) or identity
- No FFI calls in the body (besides the swan song)
- The body only reads and writes the counter

If all K bodies are effect-equivalent, collapse to a single body with `count += K` per iteration and emit the exit check accordingly.

**Implementation**:
1. Add `bodies_are_equivalent()` analysis function
2. When equivalence holds, emit a single body repeated K times conceptually but only once actually, with the counter advancing by K per trip
3. Keep the print guard at the correct interval (every 5M / interval)

**Risk**: Determining "effect-equivalence" requires liveness analysis on each body, which is already done for general A005 dispatch. Might need to rerun `trace_live_fields` on each body separately.

### Target 3: cancel_math — Wrong Dispatch (A005c instead of A005a)

**Finding**: cancel_math has 2 fields (count, acc), 100% write density, and **no body FFI**. It qualifies for A005a (inline SSA with insertvalue chain) under the existing criteria at `mod.rs:2265`:
```rust
if write_density >= 0.5 && total_fields < 8 && !has_body_ffi {
```
But it dispatches as A005c. This means the A005a check at line 2265 is either:
- Not reached for `node` + `#!exit` pattern (the dispatch skips the A005a check for reactive txns)
- Or the write_density calculation sees different fields
- Or the `node` body goes through a different dispatch path entirely

**Fix**: Diagnose why cancel_math misses A005a dispatch and fix it. This would give cancel_math (and any similar 2-field no-FFI txn) the A005a insertvalue-chain path, eliminating phi overhead.

### Target 4: print_loop — Body FFI Blocks A005a

**Current codegen**: print_loop has 2 fields but the body calls `print_int#(ops)` which is an FFI → A005a blocked. A005c overhead for 2 fields adds ~10 insns per iteration.

**Fix**: If the per-iteration FFI call is purely a "sink" (the FFI doesn't read state that the phi carries), A005a's insertvalue chain could still work. Requires classifying FFI calls as "state-modifying" vs "output-only."

### Target 5: fasta — Same PutChar FFI Issue

**Current codegen**: PutChar forces A005c for 2 fields. Was .83x (wrong A005a, LLVM optimized away fprintf) → now .96x (correct A005c).

**Fix**: Same as print_loop — if PutChar is an output-only FFI, allow A005a dispatch.

## 3. Implementation Order

1. fannkuch_redux pure-SSA rotation (biggest impact, cleanest fix)
2. sparse_dispatch effect-equivalent collapse (medium impact, requires new analysis)
3. print_loop/cancel_math A005a dispatch for tiny states (small impact, simple threshold change)
4. fasta PutChar optimization (requires FFI classification change)

## 4. Verification

After each optimization:
1. `cargo test --lib` — all 1403 tests pass
2. `bash benchmarks/build_and_bench.sh --runtime` — all 22 MATCH
3. Compare ratio against baseline; target benchmark should improve significantly
4. Ensure no other benchmark regresses >2%
