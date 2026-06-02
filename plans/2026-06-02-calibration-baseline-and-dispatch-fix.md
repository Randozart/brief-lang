# Plan: Calibration Baseline & Dispatch Bloat Fix

**Date:** 2026-06-02  
**Status:** Plan — ready for implementation

## Summary

Two problems block actionable benchmarking:
1. Benchmarks show `0.00s` because LLVM SCEV eliminates linear recurrences (const_heavy) or they hit O(1) folded paths (precompute_sum, ring_buffer, async_counters). We need non-zero numbers to measure improvements.
2. Multi-txn dispatch in `emit_reactor` forces `alwaysinline` on all txn body functions. When `opt -O2` inlines 16 functions into `reactor_tick`, it creates a massive phi/select cascade that makes dispatch pathological (less than 1 iter/5s).

New calibration benchmarks (float_math, sparse_dispatch, const_heavy) exist but need fixes to produce meaningful data.

## Plan Steps

### Step 1: Baseline — 4-decimal precision timing

**Problem**: `/usr/bin/time -f "%e"` gives `0.73` (2 decimal places). Zero-ish results show as `0.00`. 4-decimal precision (`0.0000`) reveals sub-100ms differences and distinguishes "exactly 0.0000" (O(1), no loop) from "near-zero but non-zero" (elided loop).

**Fix**: Replace `/usr/bin/time` with `date +%s.%N + TIMEFORMAT` wrapper in `bench_self_term()`:

```bash
bench_self_term() {
    local name="$1"
    echo ""
    echo "=== $name ==="
    local brief_start=$(date +%s.%N)
    BOUND=50000000 ./benchmarks/"${name}" >/dev/null 2>&1
    local brief_end=$(date +%s.%N)
    local brief_time=$(printf "%.4f" $(echo "$brief_end - $brief_start" | bc))
    echo "  Brief: ${brief_time}s"
    local c_start=$(date +%s.%N)
    BOUND=50000000 ./benchmarks/"${name}_c" >/dev/null 2>&1
    local c_end=$(date +%s.%N)
    local c_time=$(printf "%.4f" $(echo "$c_end - $c_start" | bc))
    echo "  C:     ${c_time}s"
}
```

Requires `bc` for arithmetic (standard on Linux).

### Step 2: Commit current state as baseline

Current uncommitted changes:
- `build_and_bench.sh`: Updated for BOUND, inline per-benchmark output, removing conditional clang
- `const_heavy.bv`: Added `acc == acc` to precondition (keeps acc field live)
- `iir_filter.bv`: Added `x1 == x1 && x2 == x2 && y1 == y1 && y2 == y2` (keeps float fields live)
- `sparse_dispatch.bv`: Restructured with `count % 8 == N` preconditions, no `io_pending`, no `count < total`

Commit message: `calibration: baseline benchmarks with liveness fixes`

### Step 3: Run baseline benchmarks

Run `bash benchmarks/build_and_bench.sh all` to get 4-decimal-precision measurements of the current compiler.

### Step 4: Fix A — Guard `alwaysinline` / `noinline` (SKIPPED)

**Decision**: NOT implementing. Baseline data refutes the premise.

**Data**: sparse_dispatch (8 txns, pure bodies) with `alwaysinline` + `opt -O2`:
- Brief: 0.0558s (50M iterations)
- C: 0.0019s (pure-counter O(1))
- The 0.0558s includes runtime startup overhead; Brief's loop is O(1) after SCEV optimization

**Why skipped**: `opt -O2`'s SCEV pass fully optimizes the inlined phi/select cascade into a pure counter for pure-body txns. The `alwaysinline` attribute does NOT cause bloat — it helps `opt -O2` prove the equivalence. Adding `noinline` would make multi-txn dispatch *slower* (function call overhead prevents SCEV from seeing the aggregate effect).

**When this would matter**: For multi-txn dispatch with NON-PURE bodies (each txn writes different fields, different operations). This case doesn't match any current benchmark. A new benchmark would be needed to exercise it.

**Lesson**: The phi/select cascade concern was theoretical. Real `opt -O2` + SCEV handles it.

### Step 5: Fix C — Break const_heavy SCEV optimization

**File**: `benchmarks/const_heavy.bv:22`

**Problem**: `acc = acc + C00 + ... + C19` is a linear recurrence (`acc += 21000` per iter). LLVM SCEV reduces it to `acc = 21000 * BOUND` (O(1) mul). Loop eliminated. Brief shows 0.00s.

**Fix**: Add `+ count / 100` to the accumulation. `sdiv` of the loop induction variable is non-linear — SCEV classifies it as `SCEVUnknown` and cannot fold it.

```diff
-    &acc = acc + C00 + C01 + C02 + C03 + C04
+    &acc = acc + count / 100 + C00 + C01 + C02 + C03 + C04
```

### Step 6: Fix D — Mirror const_heavy fix in C reference

**File**: `benchmarks/const_heavy_c.c:39`

```diff
-    acc = acc + C00 + C01 + C02 + C03 + C04
+    acc = acc + count / 100 + C00 + C01 + C02 + C03 + C04
```

Both Brief and C now run the full 50M loop with a non-linear `sdiv` per iteration. Neither compiler can cheat.

### Step 7: Run post-fix benchmarks

Rebuild with `cargo build --release`, recompile all benchmarks, run `bash benchmarks/build_and_bench.sh all` again. Compare with baseline.

## Acceptance Criteria

1. `cargo test --lib` passes (368+ tests)
2. Baseline: All 7 benchmarks produce non-zero Brief timings (except precompute_sum which is legitimately O(1))
3. Post-fix sparse_dispatch: no longer pathological (<5s at 50M iterations, dispatch overhead visible as call-based dispatch vs C's switch)
4. Post-fix const_heavy: ~same as C (~0.2-0.4s at 50M, both doing `sdiv` per iteration)
5. All other benchmarks unchanged in timing

## Expected Results

### Baseline (before fixes)

| Benchmark | Brief | C | Note |
|-----------|-------|---|------|
| iir_filter | ~0.15s | ~0.10s | Brief SSA loop vs C native loop |
| precompute_sum | 0.0000s | 0.0000s | Both O(1) |
| ring_buffer | 0.0000s | 0.0000s | Both O(1) |
| async_counters | 0.0000s | 0.0000s | Both O(1) |
| float_math | ~0.42s | ~0.38s | Float boxing overhead |
| sparse_dispatch | >5s or crash | ~0.05s | alwaysinline bloat makes Brief pathological |
| const_heavy | 0.0000s | 0.0000s | SCEV eliminates both loops |

### After Fix A (alwaysinline/noinline) + Fix C/D (const_heavy)

| Benchmark | Brief | C | Note |
|-----------|-------|---|------|
| sparse_dispatch | ~3-5s | ~0.05s | noinline removes bloat; dispatch overhead now visible as call overhead vs switch |
| const_heavy | ~0.2-0.4s | ~0.2-0.4s | Both do sdiv per iteration; comparable |
| Others | unchanged | unchanged | No regression |

### Analysis

- **sparse_dispatch**: Brief's 8-precondition + 8-body call dispatch is ~100× slower than C's `switch(count%8)`. This is expected — Brief evaluates all 8 preconditions every tick, while C jumps directly. The benchmark shows WHERE Brief needs improvement: indirect dispatch (computed goto or jump table) instead of call-chain dispatch.
- **const_heavy**: After fixing, this measures pure arithmetic throughput. Brief and C should be similar since both generate `sdiv` + `add` per iteration through `opt -O2`.
