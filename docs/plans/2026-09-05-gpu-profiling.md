# GPU Pipeline Profiling — Phase 1

**Date:** 2026-09-05
**Status:** Complete — timestamps working, baseline captured
**Depends on:** Stage 1 rungs (D1-D5 all evaluated, D3 landed)
**Blocks:** Phase 2 (targeted optimization based on profile data)

## Motivation

All Stage 1 rungs (D1-D5) are exhausted. The production GEMM runs at
~23 TFLOP/s @4096³, while the Stage 0 microkernel proved the
coopmat mma reaches ≥107 TFLOP/s (100% of F16-acc hardware peak).
This is a ~4.6× overhead in the pipeline around the mma.

**Root cause identified:** 891 integer div/mod instructions per workgroup
for shared memory fill index decomposition, drowning the 16 useful MMA
operations. See `docs/plans/2026-09-05-kernel-profiling-analysis.md`.

We've been doing config-knob A/B testing, but we've never profiled
WHERE the overhead lives. Without a measured breakdown, any further
optimization is a guess.

## Phase 1 Results

### Bug Fixed: VK_QUERY_TYPE_TIMESTAMP enum value

The timestamp queries returned zero because `VK_QUERY_TYPE_TIMESTAMP`
was defined as `0u` (which is `VK_QUERY_TYPE_OCCLUSION`). The correct
value is `2u` per the Vulkan spec.

**Commit:** (pending)

### Baseline Measurement (RTX 3060, 4096³ FP16 GEMM)

| Metric | Value |
|--------|-------|
| GPU kernel time (solo) | 5.332 ms |
| GPU kernel time (batched ×10) | 5.858 ms |
| Host-side per-call | 5.892 ms |
| Host overhead | ~0.034 ms (negligible) |
| FLOPS | 23.3 TFLOP/s |
| Correctness | 4.436e-03 OK |
| Hardware peak (F16-acc) | 102 TFLOP/s |
| Target (ggml-cuda) | 42.0 TFLOP/s |

**Key finding:** Host overhead is negligible. The bottleneck is
ENTIRELY in the GPU kernel (5.3 ms vs ~1.3 ms theoretical at peak).
The kernel is ~4× slower than the mma ceiling, suggesting significant
inefficiency in fills, memory access patterns, or occupancy.

### What We Need Next

1. **NSight Compute profiling** (or equivalent) to identify which
   bottleneck dominates:
   - Global memory bandwidth (fills too slow?)
   - Shared memory bank conflicts
   - Low occupancy (WGs per SM)
   - Instruction mix (too many scalar ops, not enough vectorized)
   - Pipeline stalls (waiting for mma results)

2. **Phase breakdown** with multiple kernels:
   - Is A/B fill the bottleneck?
   - Is the mma loop itself stalled?
   - Are epilogue loads slow?

## Approach

### Part 1: Vulkan Timestamp Queries (DONE)

Added `VK_QUERY_TYPE_TIMESTAMP` pool to the Vulkan driver. Hardware
supports 1ns precision, 64-bit range on both RTX 3060 and GTX 1070 Ti.

**Driver changes** (`lib/runtime/briev_dev_vulkan.c`):

1. Added 5 dlsym entries:
   - `vkCreateQueryPool`
   - `vkDestroyQueryPool`
   - `vkGetQueryPoolResults`
   - `vkCmdResetQueryPool`
   - `vkCmdWriteTimestamp`

2. Added to `BrievVulkanKernel` struct:
   - `VkQueryPool query_pool` — 2-query pool (pre/post dispatch)

3. Capture `timestampValidBits` from `vkGetPhysicalDeviceQueueFamilyProperties`

4. Query pool created in `create_kernel()`

5. Per dispatch (both launch paths A and B):
   - `vkCmdResetQueryPool` before recording
   - `vkCmdWriteTimestamp(ALL_COMMANDS_BIT, idx=0)` before dispatch
   - `vkCmdWriteTimestamp(ALL_COMMANDS_BIT, idx=1)` after dispatch
   - After fence wait: `vkGetQueryPoolResults` → `(ts[1] - ts[0]) × period`
   - Print `# gpu_time: X.XXX ms` to stderr

6. Query pool destroyed in `destroy_kernel()`

### Part 2: Host-Side Phase Splitting

The bench harness wraps `briev_accel_launch_resident_batch()` in a
single `now_ms()` bracket. We don't need a new API — the Vulkan
timestamp gives us GPU time, and the existing `now_ms()` gives total.
The difference = host overhead (memcpy + fence wake + driver jitter).

## Files to Modify

| File | Changes |
|------|---------|
| `lib/runtime/briev_dev_vulkan.c` | dlsym entries, query pool, timestamps, result reading |

## Verification

1. `cargo build --release` — compiler builds
2. `gcc` bench harness compiles
3. `spirv-val` on generated SPV — correctness unchanged
4. `cargo test --lib` — no regressions
5. Run bench: gpu_time is reasonable (< total_time, > 0)
6. Correctness gate: max_rel_err within tier bounds

## Risks

- Query pool overhead: minimal (2 queries, reset per dispatch)
- Timestamp accuracy: 1ns period on both GPUs, sufficient
- Driver dlsym: all 5 functions are core Vulkan 1.0, guaranteed available
- No behavioral change: timestamps are read-only instrumentation
