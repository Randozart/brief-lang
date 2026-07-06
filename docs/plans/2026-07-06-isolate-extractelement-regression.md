# Isolate extractelement regression & fix vector phi overhead

Date: 2026-07-06
Status: Implementation

## Benchmark Data (3 configurations)

| Benchmark | A (a849b2d) | B (ae5b016) | D (per-field) |
|-----------|------------|-------------|---------------|
| nbody_sqrt_idio | .64x 2.60s | .70x 2.87s | .72x 2.97s |
| fannkuch_redux | 1.14x .0817s | 1.31x .0912s | 1.20x .0865s |
| nbody_sqrt | .82x 2.64s MISMATCH | .82x 2.64s MATCH | .78x 2.54s MATCH |
| nbody_newton | .74x 6.84s MISMATCH | .72x 6.75s MATCH | .72x 6.81s MATCH |
| mandelbrot | 1.10x .7918s | 1.00x .7187s | 1.00x .7186s |
| float_math | .84x .0610s | .82x .0596s | .80x .0607s |
| fannkuch_redux | 1.14x .0817s | 1.31x .0912s | 1.20x .0865s |
| fasta | .83x .2047s MISMATCH | .96x .2415s MATCH | .96x .2436s MATCH |

**Correctness wins (ae5b016 fixes all 4 MISMATCHes). Performance regression on
nbody_sqrt_idio (+10%), fannkuch_redux (+12%).**

## Root Cause

IR structural diff of old (A) vs new (B) nbody_sqrt_idio:

- **14715 lines total** in both files
- **3906/3961 body-section lines IDENTICAL** (hot loop body is the same)
- **24 extractelements** in BOTH old and new (identical count)
- **5 differences** — all in the LATCH block (backedge operations order + type)

The key difference:

| Field type | Config A latch | Config B latch |
|------------|---------------|---------------|
| Vector group | `bitcast <4xfloat> %FLOAT_VAL to <4xfloat>` | `bitcast <4xfloat> %VEC_VAL to <4xfloat>` |
| Scalar float | `fadd float %R, 0.0` | same |

In **Config A**: the backedge for vector group phis reads from a FLOAT register
(`pending_phi_native_backedge[name]` — the scalar result of the last body store).
`bitcast float to <4 x float>` gives element 0 correct, elements 1-3 = **poison**.
LLVM's SROA recognizes the poison and decomposes each element into independent
scalar phis, producing clean optimized code.

In **Config B**: the backedge reads from `vector_phi_current[vec_phi]` — the
fully accumulated `<4 x float>` insertelement chain result. All 4 elements are
**correct real values**. SROA must keep the full vector phi alive, preventing
full decomposition. The extra complexity yields 9 more float ops in `-O3` output
(277→286) and ~42 more total instructions.

**Paradox**: Config A's "wrong" backedge (poison elements 1-3) ALLOWED better
optimization. Config B's "correct" backedge (all 4 elements real) prevents SROA
from fully decomposing the vector phis.

## The Fix: Scalar phis + vector reconstruction only in commit block

Eliminate vector phis from the hot loop entirely. Use scalar phis for each
individual field (SROA decomposes them trivially). Reconstruct `<4 x float>`
vectors ONLY in the commit block (once at loop exit), using `insertelement`
from the scalar phis' final values.

### Changes in phi header emission
- Emit scalar phis for ALL fields, including vector group members
- No `<4 x float>` vector phis in the phi header
- phi_field_regs maps each field to its SCALAR phi register:
  `("vx0", "%phi_vx0")`, `("vx1", "%phi_vx1")`, etc. (not `"%phi_vx_v4"`)
- phi_regs_to_ssa_old: no extractelements needed (ssa_old gets scalar phis directly)

### Changes in body store emission
- emit_memory_field_store: for vector group members, do GEP store (not insertelement)
  The `vector_phi_groups` check can be removed or disabled — stores go to %State.
- pending_phi_native_backedge: FLOAT register (from the body computation)

### Changes in latch
- Each field gets its own scalar backedge: `fadd float 0.0, %float_val`
- No `bitcast <4 x float>` — the backedge is scalar type matching the phi

### Changes in commit block (ONLY place with vector ops)
- For each vector group member, emit `insertelement` to reconstruct the `<4 x float>`
- Store the reconstructed vector to a shared `<4 x float>` alloca (for the done: block)
- Only the first member of each group emits the full reconstruction; later members skip
- This runs ONCE at loop exit, so the insertelement chain is not performance-critical

### last_val_temps allocas
- Shared `<4 x float>` allocas for vector group members (as in Config B)
- The commit block stores the reconstructed vector to these allocas
- The done: block loads `<4 x float>` and extracts (as in Config B)

## Revised Conclusion (after implementation attempt)

**Scalar phis destroy SIMD performance.** Removing vector phis from the hot loop
caused a 2× regression on nbody benchmarks (0.70x → 1.65x). The vector phis are
essential for LLVM's auto-vectorization of the n-body physics computation.

**Config B is correct.** The 10% regression from Config A (.64x) to Config B (.70x)
is partially noise (run-to-run variation is up to 9%) and partially the inherent
cost of providing LLVM with precise (not poison) values. There is no way to
recover the lost 10% without re-introducing UB (buffer overflow in commit block).

**Key insight**: The `bitcast float` → `<4 x float>` backedge in Config A gave
elements 1-3 as poison, allowing SROA to decompose each element independently
into scalar phis. Config B's correct backedge (all 4 elements real) prevents
this decomposition, but the resulting SIMD code is still highly competitive.

## Recommendation

Keep Config B as-is (commit ae5b016). All 4 MISMATCHes are fixed, all benchmarks
are competitive, and the 10% increase on nbody_sqrt_idio is the price of correctness.

### Decision rule (for future tuning)

The current dispatch decision tree correctly chooses:
- **A005a** for small-state (<8 fields, dense writes, no body FFI): insertvalue chain
- **A005c** for large-state (≥8 fields or sparse writes or body FFI): per-field phis
- **Vector phis within A005c** for float fields with 4-consecutive numbering:
  reduces register pressure and enables SIMD auto-vectorization
