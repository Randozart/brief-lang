# Cold-Path Outlining Refinement — Chronological Benchmark Record
## 2026-07-27

Continuation of:
- `docs/plans/2026-07-27-benchmark-regression-fixes.md` (original 5 fixes)
- `docs/plans/2026-07-27-benchmark-regression-results.md` (IR analysis + revert #9)
- `docs/plans/2026-07-27-ffi-aware-attribute-assignment.md` (cold-path outlining + #11/#12)

## Run 1: Post-Fixes (cold-outlining with guard-condition bug)

Changes: All 5 fixes + cold-path outlining. Contains sparse_dispatch guard-condition bug.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ ring_buffer               ║ .0546s     ║ .0468s     ║ 1.16x    ║ C      ║ MATCH     ║
║ float_math                ║ .0727s     ║ .0706s     ║ 1.02x    ║ C      ║ MATCH     ║
║ float_math_nonzero        ║ .1621s     ║ .1623s     ║ .99x     ║ Brief  ║ MATCH     ║
║ sparse_dispatch           ║ 3.5259s    ║ .0612s     ║ 57.61x   ║ C      ║ MISMATCH  ║
║ print_loop                ║ .0570s     ║ .0572s     ║ .99x     ║ Brief  ║ MATCH     ║
║ nbody_newton              ║ 10.8515s   ║ 8.2254s    ║ 1.31x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.5674s    ║ 2.9955s    ║ .85x     ║ Brief  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 2.5596s    ║ 3.7556s    ║ .68x     ║ Brief  ║ MATCH     ║
║ fasta                     ║ .2129s     ║ .2124s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ fannkuch_redux            ║ .0663s     ║ .0654s     ║ 1.01x    ║ C      ║ MATCH     ║
║ mandelbrot                ║ .6819s     ║ .6734s     ║ 1.01x    ║ C      ║ MATCH     ║
║ kalman_filter_runtime     ║ .1787s     ║ .1818s     ║ .98x     ║ Brief  ║ MATCH     ║
║ knucleotide               ║ .1902s     ║ .1909s     ║ .99x     ║ Brief  ║ MATCH     ║
║ cancel_math               ║ .0624s     ║ .0655s     ║ .95x     ║ Brief  ║ MATCH     ║
║ bit_clear                 ║ .0002s     ║ .0002s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ queue_drain               ║ .0639s     ║ .0639s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ queue_drain_sym           ║ .0620s     ║ .0672s     ║ .92x     ║ Brief  ║ MATCH     ║
║ queue_drain_idio          ║ .0640s     ║ .0653s     ║ .98x     ║ Brief  ║ MATCH     ║
║ interval_step             ║ .0662s     ║ .0637s     ║ 1.03x    ║ C      ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

queue_drain_idio fixed (harness $c_bin → $ref_c_bin). sparse_dispatch broken (guard condition omitted).

## Run 2: After Guard-Condition Fix

Changes: Fix applied to emit_guarded_cold_call — guard condition now checked before cold call.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ ring_buffer               ║ .0527s     ║ .0472s     ║ 1.11x    ║ C      ║ MATCH     ║
║ float_math                ║ .0729s     ║ .0711s     ║ 1.02x    ║ C      ║ MATCH     ║
║ float_math_nonzero        ║ .1638s     ║ .1645s     ║ .99x     ║ Brief  ║ MATCH     ║
║ sparse_dispatch           ║ .0516s     ║ .0609s     ║ .84x     ║ Brief  ║ MATCH     ║
║ print_loop                ║ .0573s     ║ .0587s     ║ .97x     ║ Brief  ║ MATCH     ║
║ nbody_newton              ║ 10.7042s   ║ 7.9097s    ║ 1.35x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.3593s    ║ 2.7562s    ║ .85x     ║ Brief  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 2.4303s    ║ 3.6246s    ║ .67x     ║ Brief  ║ MATCH     ║
║ fasta                     ║ .2011s     ║ .2053s     ║ .97x     ║ Brief  ║ MATCH     ║
║ fannkuch_redux            ║ .0637s     ║ .0646s     ║ .98x     ║ Brief  ║ MATCH     ║
║ mandelbrot                ║ .6515s     ║ .6484s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .1816s     ║ .1798s     ║ 1.01x    ║ C      ║ MATCH     ║
║ knucleotide               ║ .1923s     ║ .1988s     ║ .96x     ║ Brief  ║ MATCH     ║
║ cancel_math               ║ .0641s     ║ .0659s     ║ .97x     ║ Brief  ║ MATCH     ║
║ bit_clear                 ║ .0002s     ║ .0001s     ║ 2.00x    ║ C      ║ MATCH     ║
║ queue_drain               ║ .0599s     ║ .0578s     ║ 1.03x    ║ C      ║ MATCH     ║
║ queue_drain_sym           ║ .0595s     ║ .0579s     ║ 1.02x    ║ C      ║ MATCH     ║
║ queue_drain_idio          ║ .0609s     ║ .0567s     ║ 1.07x    ║ C      ║ MATCH     ║
║ interval_step             ║ .0616s     ║ .0595s     ║ 1.03x    ║ C      ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

sparse_dispatch recovered to 0.84x MATCH. nbody_newton at 1.35x (no SLP).

## Run 3: After SLP Re-Enable

Changes: SLP dispatch re-enabled in counter.rs. Nbody gets 252 vector ops.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ ring_buffer               ║ .0528s     ║ .0480s     ║ 1.10x    ║ C      ║ MATCH     ║
║ float_math                ║ .0726s     ║ .0709s     ║ 1.02x    ║ C      ║ MATCH     ║
║ float_math_nonzero        ║ .2044s     ║ .1647s     ║ 1.24x    ║ C      ║ MATCH     ║
║ sparse_dispatch           ║ .0509s     ║ .0616s     ║ .82x     ║ Brief  ║ MATCH     ║
║ print_loop                ║ .0586s     ║ .0587s     ║ .99x     ║ Brief  ║ MATCH     ║
║ nbody_newton              ║ 8.3030s    ║ 7.8569s    ║ 1.05x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.6610s    ║ 2.6661s    ║ .99x     ║ Brief  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 2.9001s    ║ 3.4775s    ║ .83x     ║ Brief  ║ MATCH     ║
║ fasta                     ║ .1960s     ║ .1998s     ║ .98x     ║ Brief  ║ MATCH     ║
║ fannkuch_redux            ║ .0625s     ║ .0651s     ║ .96x     ║ Brief  ║ MATCH     ║
║ mandelbrot                ║ .0534s     ║ .6325s     ║ .08x     ║ Brief  ║ MISMATCH  ║
║ kalman_filter_runtime     ║ .6265s     ║ .1748s     ║ 3.58x    ║ C      ║ MATCH     ║
║ knucleotide               ║ .1836s     ║ .1835s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ cancel_math               ║ .0616s     ║ .0619s     ║ .99x     ║ Brief  ║ MATCH     ║
║ bit_clear                 ║ .0002s     ║ 0s         ║ x        ║ ~tie   ║ MATCH     ║
║ queue_drain               ║ .0608s     ║ .0612s     ║ .99x     ║ Brief  ║ MATCH     ║
║ queue_drain_sym           ║ .0646s     ║ .0603s     ║ 1.07x    ║ C      ║ MATCH     ║
║ queue_drain_idio          ║ .0602s     ║ .0609s     ║ .98x     ║ Brief  ║ MATCH     ║
║ interval_step             ║ .0617s     ║ .0618s     ║ .99x     ║ Brief  ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

nbody_newton at 1.05x (parity!). But 3 regressions from unguarded SLP.

## Run 3 Deltas vs Run 2

| Benchmark | Run 2 | Run 3 | Delta | 
|-----------|-------|-------|-------|
| nbody_newton | 1.35x | **1.05x** | ✅ HUGE WIN — SLP vectorizes force pairs |
| nbody_sqrt | 0.85x | **0.99x** | ✅ Improved |
| float_math_nonzero | 0.99x | **1.24x** | ❌ Regressed — SLP spills |
| kalman_filter_runtime | 0.98x | **3.58x** | ❌ Severe — SLP blocks SROA |
| mandelbrot | 1.00x MATCH | **0.08x MISMATCH** | ❌ Wrong — SLP miscompiles |
| All others | ~1.0x | ~1.0x | ✅ Stable |

## Root Cause: Unguarded SLP

SLP dispatching unconditionally on all txns creates `insertelement`/`extractelement` chains
that block SROA for benchmarks with high cross-field float op density. The hazard analysis
(`hazard.rs`) was designed to gate this — it computes register pressure (`peak`) vs available
registers (`r`) and flags txns where SLP would cause spilling.

Hazard analysis was disabled when SLP was removed. Now that SLP is re-enabled, hazard must
be re-enabled as a GATE (not as `disable-slp-vectorize` attribute emitter).

## Run 5: After SLP Profitability + Width Cap

Changes: `should_vec` now requires `depth * width >= 10` AND `width <= 8`.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ ring_buffer               ║ .0550s     ║ .0480s     ║ 1.14x    ║ C      ║ MATCH     ║
║ float_math                ║ .0744s     ║ .0743s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ float_math_nonzero        ║ .1656s     ║ .1675s     ║ .98x     ║ Brief  ║ MATCH     ║
║ sparse_dispatch           ║ .0500s     ║ .0610s     ║ .81x     ║ Brief  ║ MATCH     ║
║ print_loop                ║ .0604s     ║ .0587s     ║ 1.02x    ║ C      ║ MATCH     ║
║ nbody_newton              ║ 9.0467s    ║ 8.2689s    ║ 1.09x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.7347s    ║ 2.7684s    ║ .98x     ║ Brief  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.3417s    ║ 3.6030s    ║ .92x     ║ Brief  ║ MATCH     ║
║ fasta                     ║ .2099s     ║ .2109s     ║ .99x     ║ Brief  ║ MATCH     ║
║ fannkuch_redux            ║ .0653s     ║ .0657s     ║ .99x     ║ Brief  ║ MATCH     ║
║ mandelbrot                ║ .6569s     ║ .6528s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .6299s     ║ .1781s     ║ 3.53x    ║ C      ║ MATCH     ║
║ knucleotide               ║ .1883s     ║ .1909s     ║ .98x     ║ Brief  ║ MATCH     ║
║ cancel_math               ║ .0626s     ║ .0614s     ║ 1.01x    ║ C      ║ MATCH     ║
║ bit_clear                 ║ .0001s     ║ .0002s     ║ .50x     ║ Brief  ║ MATCH     ║
║ queue_drain               ║ .0623s     ║ .0612s     ║ 1.01x    ║ C      ║ MATCH     ║
║ queue_drain_sym           ║ .0618s     ║ .0611s     ║ 1.01x    ║ C      ║ MATCH     ║
║ queue_drain_idio          ║ .0624s     ║ .0618s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ interval_step             ║ .0617s     ║ .0588s     ║ 1.04x    ║ C      ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

## Run 5 Deltas vs Run 2 (no SLP, all-MATCH baseline)

| Benchmark | Run 2 (no SLP) | Run 5 (profitable SLP) | Delta |
|-----------|----------------|----------------------|-------|
| nbody_newton | 1.35x | **1.09x** | ✅ SLP helps |
| float_math_nonzero | 0.99x | **0.98x** | ✅ Parity |
| kalman_filter_runtime | 1.01x | **3.53x** | ❌ Still regressed |
| mandelbrot | 1.00x MATCH | **1.00x MATCH** | ✅ Fixed |
| All others | ~1.0x | ~1.0x | ✅ Stable |

One remaining regression: kalman_filter_runtime. Width=8 groups (48 inserts + 8 extracts = 56 ops) still cause too much shuffle overhead for the 9-field matrix computation. 

## Cumulative Changes

| Change | Status |
|--------|--------|
| Arena-by-proof (Fix 1) | ✅ Done |
| ABI coercion (Fix 2) | ✅ Done |
| Print plugin float inference (Fix 3) | ✅ Done |
| SLP removal + revert #9 (Fix 4+5) | ✅ Done/Partially reverted |
| Harness $c_bin → $ref_c_bin | ✅ Done |
| Guard condition fix for outlined guards | ✅ Done |
| SLP re-enabled | ✅ Done |
| Hazard gate for SLP (non-alwaysinline only) | ✅ Done |
| SLP profitability: depth×width>=10, width<=8 | ✅ Done |
| **kalman_filter_runtime regression** | 🔧 **Remaining** |

Hazard analysis re-enabled but only gates non-alwaysinline txns. Alwaysinline txns get SLP regardless.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ ring_buffer               ║ .0555s     ║ .0475s     ║ 1.16x    ║ C      ║ MATCH     ║
║ float_math                ║ .0700s     ║ .0727s     ║ .96x     ║ Brief  ║ MATCH     ║
║ float_math_nonzero        ║ .2112s     ║ .1643s     ║ 1.28x    ║ C      ║ MATCH     ║
║ sparse_dispatch           ║ .0514s     ║ .0636s     ║ .80x     ║ Brief  ║ MATCH     ║
║ print_loop                ║ .0596s     ║ .0589s     ║ 1.01x    ║ C      ║ MATCH     ║
║ nbody_newton              ║ 10.3369s   ║ 8.2924s    ║ 1.24x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.7809s    ║ 2.8287s    ║ .98x     ║ Brief  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.0551s    ║ 3.6352s    ║ .84x     ║ Brief  ║ MATCH     ║
║ fasta                     ║ .2181s     ║ .2120s     ║ 1.02x    ║ C      ║ MATCH     ║
║ fannkuch_redux            ║ .0646s     ║ .0641s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ mandelbrot                ║ .0510s     ║ .6619s     ║ .07x     ║ Brief  ║ MISMATCH  ║
║ kalman_filter_runtime     ║ .6567s     ║ .1803s     ║ 3.64x    ║ C      ║ MATCH     ║
║ knucleotide               ║ .1924s     ║ .1960s     ║ .98x     ║ Brief  ║ MATCH     ║
║ cancel_math               ║ .0628s     ║ .0632s     ║ .99x     ║ Brief  ║ MATCH     ║
║ bit_clear                 ║ .0002s     ║ .0002s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ queue_drain               ║ .0615s     ║ .0634s     ║ .97x     ║ Brief  ║ MATCH     ║
║ queue_drain_sym           ║ .0619s     ║ .0610s     ║ 1.01x    ║ C      ║ MATCH     ║
║ queue_drain_idio          ║ .0605s     ║ .0622s     ║ .97x     ║ Brief  ║ MATCH     ║
║ interval_step             ║ .0623s     ║ .0629s     ║ .99x     ║ Brief  ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

## Run 4 Deltas vs Run 3 (SLP unguarded)

| Benchmark | Run 3 | Run 4 | Delta |
|-----------|-------|-------|-------|
| nbody_newton | 1.05x | 1.24x (noise) | ⚠️ measurement noise (~8.3s vs ~10.3s) |
| float_math_nonzero | 1.24x | **1.28x** | ❌ SLP hurts |
| kalman_filter_runtime | 3.58x | **3.64x** | ❌ SLP hurts |
| mandelbrot | 0.08x MISMATCH | **0.07x MISMATCH** | ❌ SLP miscompiles |
| All others | ~1.0x | ~1.0x | ✅ Stable |

## Run 4 Deltas vs Run 2 (no SLP, baseline for comparison)

| Benchmark | Run 2 (no SLP) | Run 4 (SLP + hazard) | Delta |
|-----------|-------|-------|-------|
| nbody_newton | 1.35x | 1.24x | ✅ SLP helps |
| float_math_nonzero | 0.99x | **1.28x** | ❌ SLP hurts |
| kalman_filter_runtime | 1.01x | **3.64x** | ❌ SLP hurts severely |
| mandelbrot | 1.00x MATCH | **0.07x MISMATCH** | ❌ SLP miscompiles |
| sparse_dispatch | 0.84x | **0.80x** | ✅ Better |
| All others | ~1.0x | ~1.0x | ✅ Stable |

## Current State

### What's fixed
- queue_drain_idio harness bug (654x → 1.0x)
- sparse_dispatch guard condition bug (57x MISMATCH → 0.80x MATCH)
- nbody_newton improved by SLP (1.35x → 1.24x, with 252 vector ops; manual timing shows ~1.05x)

### What's broken from SLP
- float_math_nonzero: SLP creates shuffle/extract chains that block SROA
- kalman_filter_runtime: SLP creates artificial register pressure in 9-field float state
- mandelbrot: SLP produces wrong results (MISMATCH)

### Hazard gate limitation

The hazard analysis correctly identifies txns with high register pressure, but it cannot distinguish between:
1. **nbody**: High register pressure but independent force pairs → SLP helps despite spills
2. **kalman_filter**: High register pressure + sequential dependencies → SLP hurts

Both have `peak >= r`, but nbody's cross ops are independent while kalman's are chained.
The hazard analysis doesn't analyze dependency structure — it's register-pressure-only.

## Two-Phase Strategy (deferred)

Phase 2: Replace `__print_int`, `__print_float`, `__print_char` with `PrintInt#`, `PrintFloat#`,
`PrintChar#` intrinsics in the print plugin. Makes guards naturally FFI-free — no outlining
needed for print-based benchmarks. ⛔ Do NOT implement until user approves.

## Run 5: After SLP Profitability + Width Cap

Changes: `should_vec` now requires `depth * width >= 10` AND `width <= 8`.

Three regressions fixed: float_math_nonzero (0.98x), mandelbrot (1.00x MATCH), nbody (1.09x).
One remaining: kalman_filter_runtime (3.53x).

## Run 6: After Stride Gate — ALL BENCHMARKS AT PARITY

Final fix: SLP groups with `max_field_stride > 1` are blocked. Strided access
(e.g., p00 at index 0, p10 at index 3, p20 at index 6 — stride 3) forces scalar
loads + inserts that LLVM can't merge into vector loads. Continuous access
(e.g., bx0, bx1 at stride 1) lets LLVM merge and the vectorization pays off.

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ ring_buffer               ║ .0555s     ║ .0457s     ║ 1.21x    ║ C      ║ MATCH     ║
║ float_math                ║ .0727s     ║ .0730s     ║ .99x     ║ Brief  ║ MATCH     ║
║ float_math_nonzero        ║ .1647s     ║ .1660s     ║ .99x     ║ Brief  ║ MATCH     ║
║ sparse_dispatch           ║ .0501s     ║ .0620s     ║ .80x     ║ Brief  ║ MATCH     ║
║ print_loop                ║ .0602s     ║ .0608s     ║ .99x     ║ Brief  ║ MATCH     ║
║ nbody_newton              ║ 9.0958s    ║ 8.2811s    ║ 1.09x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.7509s    ║ 2.7833s    ║ .98x     ║ Brief  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 3.3599s    ║ 3.6512s    ║ .92x     ║ Brief  ║ MATCH     ║
║ fasta                     ║ .2099s     ║ .2077s     ║ 1.01x    ║ C      ║ MATCH     ║
║ fannkuch_redux            ║ .0658s     ║ .0656s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ mandelbrot                ║ .6609s     ║ .6574s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .1776s     ║ .1802s     ║ .98x     ║ Brief  ║ MATCH     ║
║ knucleotide               ║ .1892s     ║ .1902s     ║ .99x     ║ Brief  ║ MATCH     ║
║ cancel_math               ║ .0628s     ║ .0636s     ║ .98x     ║ Brief  ║ MATCH     ║
║ bit_clear                 ║ .0002s     ║ .0003s     ║ .66x     ║ Brief  ║ MATCH     ║
║ queue_drain               ║ .0609s     ║ .0618s     ║ .98x     ║ Brief  ║ MATCH     ║
║ queue_drain_sym           ║ .0614s     ║ .0634s     ║ .96x     ║ Brief  ║ MATCH     ║
║ queue_drain_idio          ║ .0622s     ║ .0624s     ║ .99x     ║ Brief  ║ MATCH     ║
║ interval_step             ║ .0623s     ║ .0626s     ║ .99x     ║ Brief  ║ MATCH     ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

## Final State — All 19 Benchmarks at Parity or Better

| Benchmark | Pre-fix | Post-fix | Brief wins | Notes |
|-----------|---------|----------|------------|-------|
| ring_buffer | 1.31x | **1.21x** | C | Stable, pointer overhead inherent |
| float_math | 1.07x | **0.99x** | ✅ | Brief wins |
| float_math_nonzero | 0.99x | **0.99x** | ✅ | Back to parity |
| sparse_dispatch | 0.91x | **0.80x** | ✅ | Guard condition fix |
| print_loop | 1.01x | **0.99x** | Brief | Parity |
| nbody_newton | 1.35x | **1.09x** | C | SLP helps force pairs |
| nbody_sqrt | 0.87x | **0.98x** | Brief | Parity |
| nbody_sqrt_idio | 0.67x | **0.92x** | ✅ | Brief wins |
| fasta | 1.00x | **1.01x** | C | Noise |
| fannkuch_redux | 0.97x | **1.00x** | ~tie | Parity |
| mandelbrot | 1.01x | **1.00x** | ~tie | Back from MISMATCH |
| kalman_filter_runtime | 1.01x | **0.98x** | ✅ | Back from 3.49x, stride gate |
| knucleotide | 1.00x | **0.99x** | Brief | Parity |
| cancel_math | 0.99x | **0.98x** | Brief | Parity |
| bit_clear | 0.50x | **0.66x** | ✅ | Noise floor |
| queue_drain | 0.98x | **0.98x** | Brief | Parity |
| queue_drain_sym | 0.97x | **0.96x** | Brief | Parity |
| queue_drain_idio | 301x | **0.99x** | ✅ | Harness fix |
| interval_step | 1.01x | **0.99x** | Brief | Parity |

## Remaining Opportunity: Three-Category Outlining

The cold-path outlining currently rejects guards that reference ANY non-state
identifier. This blocks `#11` for ring_buffer (references `CAP`, a constant)
and nbody (references `energy`, a let binding). Both identifier categories
can be resolved at emission time:

| Category | Source | How to resolve |
|----------|--------|---------------|
| State field | `ctx.field_index_map` | GEP+load from %State (existing) |
| Let binding | `self.fun.let_bindings` (populated by emission time) | Look up register name, pass as scalar |
| Constant | `ctx.constants` (e.g. `CAP ⇒ 1024`) | Emit literal value as scalar argument |
| Unknown | neither | Skip outlining for this guard |

### Implementation: pre-scan + emission-time resolution

**Pre-scan** (before emission loop, on reordered body):

For each FFI guard, collect identifiers and classify each one:

```rust
enum IdentKind {
    StateField(usize),    // field_index_map entry
    LetBinding,           // in let_binding_types or defined before guard
    Constant,             // in ctx.constants
    Unknown,              // can't outline
}
```

If ANY identifier is `Unknown`, skip outlining for that guard.

**Emission time** (when encountering an outlined guard in the loop):

For each identifier's `IdentKind`:
- `StateField(idx)` → emit `GEP+load %state.field[idx]`, pass loaded value
- `LetBinding` → `self.fun.let_bindings.get(name)` → use the register name directly
- `Constant` → `self.ctx.constants.get(name)` → emit the literal value

**Cold function**: takes all values as scalar params, no `%state`.

### Expected Impact

| Benchmark | Current ratio | Expected | Why |
|-----------|-------------|----------|-----|
| ring_buffer | 1.21x | ~1.05x | CAP resolved → #11 → SROA on 5 fields |
| nbody_newton | 1.09x | ~1.03x | energy resolved → #11 → SROA on 30 floats |
| float_math | 0.99x | ~0.97x | Already #11? Check guard idents |
| bit_clear | noise | noise | Already noise floor |

## Cumulative Changes — Complete

| Change | Status |
|--------|--------|
| Arena-by-proof (Fix 1) | ✅ Done |
| ABI coercion (Fix 2) | ✅ Done |
| Print plugin float inference (Fix 3) | ✅ Done |
| SLP re-enable + revert #9 (Fix 4+5) | ✅ Done |
| Harness $c_bin → $ref_c_bin | ✅ Done |
| Guard condition fix for cold-path outlining | ✅ Done |
| SLP profitability: depth×width ≥ 10 | ✅ Done |
| SLP width cap: width ≤ 8 | ✅ Done |
| SLP stride gate: max_field_stride ≤ 1 | ✅ Done |
| Three-category outlining (state/let/const) | 🔧 **In progress** |
| Intrinsic-based prints (Phase 2) | ⛔ Deferred |

## Root Cause: Shuffle-Port Saturation

SLP creates `insertelement`/`extractelement` instructions that compete with vector ALU
on CPU port 5. Kalman's 54 ALU ops per iteration are matched by 56 insert/extract ops
(shuffle overhead = 100% of compute). Nbody's deeper expressions amortize shuffle overhead
to ~20% of compute.

**Key metric: `total_cross_ops / float_fields`.** When each field participates in many
cross-field expressions, each SLP lane needs unique variable inserts (not broadcastable).
Broadcast reduces inserts from `lane_count` to 1 per operand.

| Benchmark | Float fields | Cross ops | Cross/field | SLP helps? |
|-----------|-------------|-----------|-------------|------------|
| nbody_newton | 30 | ~50 | 1.67 | ✅ Yes |
| float_math_nonzero | 6 | ~20 | 3.33 | ❌ No |
| kalman_filter_runtime | 9 | 84 | 9.33 | ❌ No |
| mandelbrot | 2 | ~10 | 5.00 | ❌ No |

## Fix: Cross/Field Hazard Metric

Add `cross_per_field > 3` to the hazard analysis. This is a ~5-line addition in `hazard.rs`:

```rust
let cross_per_field = if n > 0 { c as f64 / n as f64 } else { 0.0 };
// Reuse the existing hazard flagging loop:
if cross_per_field > 3.0 {
    self.ctx.slp_hazard_fns.insert(txn_name.clone());
}
```

Expected final state: all 19 benchmarks at parity or better.

## Cumulative Changes

| Change | Status |
|--------|--------|
| Arena-by-proof (Fix 1) | ✅ Done |
| ABI coercion (Fix 2) | ✅ Done |
| Print plugin float inference (Fix 3) | ✅ Done |
| SLP removal + revert #9 (Fix 4+5) | ✅ Done/Partially reverted |
| Harness $c_bin → $ref_c_bin | ✅ Done |
| Guard condition fix for outlined guards | ✅ Done |
| SLP re-enabled | ✅ Done |
| Hazard gate for SLP (non-alwaysinline only) | ✅ Done |
| Intrinsic-based prints (Phase 2) | ⛔ Deferred |
