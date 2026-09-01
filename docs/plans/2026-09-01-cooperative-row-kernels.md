# Cooperative row kernels — subgroup-reduced dot products (M1 → parity push)

**Date:** 2026-09-01
**Status:** plan — implementation starts in this session
**Bar:** ggml-cuda on this box does the same GEMV in 0.213ms (157.8
GFLOP/s, 83% of VRAM roofline); we are at 0.93ms (36 GFLOP/s). The gap is
parallelism: theirs runs a warp per row (~4096 warps), ours one thread per
row (128 warps) with a serial K-chain.

## 1. Shape recognition (analysis — accel.rs)

A `foreach k in 0..K` whose body is exactly

```
acc = acc + f1[...k...] * f2[...k...]     // either mul order
```

with `acc` a kernel-local float accumulator is a **dot-product reduction**
— a distinct kernel shape, not a general loop. `KernelShape` gains

```rust
pub reduction: Option<ReductionInfo>,   // { inner_len: u64 }  (K, const)
```

Detection is conservative: single-Assign body, additive accumulator
self-reference, both mul operands index-containing the loop var, K a
literal const resolvable via `const_int_values`. Anything else stays on
the existing paths.

## 2. Cooperative lowering (backend)

Dispatch `(lane, row)` 2D — one 32-thread workgroup per row:

- `i` (row) binds to `GetGlobalId#(1)`; `lane` is `GetGlobalId#(0)`.
- Each lane accumulates elements `k = lane + t*32` for `t in 0..K/32`
  (coalesced stride-32 reads; K/32 iterations serial per lane).
- Subgroup reduction: `total = SubgroupFAdd#(acc)` →
  `OpGroupNonUniformFAdd(Subgroup, None, acc)`; the fixed-shape tree is
  bit-exact across runs (no atomics anywhere).
- Single store: `if lane == 0 { y[i] = total }`.
- Capabilities: `GroupNonUniform` + `GroupNonUniformArithmetic`.

Implementation is mostly **AST synthesis** in `kernel.rs` (the existing
lowering handles synthesized foreach/getglobalid forms), plus:

| piece | site |
|-------|------|
| reduction recognition | `accel.rs` (shape) |
| `SubgroupFAdd#` intrinsic + capabilities | `lower.rs`, `builder.rs` |
| index binding: `i = gid.y` for reduction kernels | `bind_work_item_index` |
| synthesis: strided accumulate loop + reduce + lane-0 store | `kernel.rs` |
| runner dispatch `launch_resident_2d(idx, state, 32, M)` | `runner.rs` |

The runtime's 2D dispatch already covers the geometry: LocalSize X for
reduction kernels is 32 (per-kernel execution mode), nx=32 → groups_x=1,
ny=M workgroups.

## 3. Risks / notes

- Subgroup size is 32 on NVIDIA (queried value matches the lane count);
  a device with different subgroup size needs the queried value wired
  into the stride — noted, not this session (our floor is the 3060).
- `spirv-val` must see the two new capabilities and the Subgroup-scope
  group op; test asserts both.
- The scalar x[] broadcast reads stay per-lane (32 reads of the same
  address per workgroup — L1-served, fine at this stage).
- Numerics: the reduction tree changes accumulation order vs the serial
  chain — the correctness gate is the harness tolerance (1e-3), not
  bit-exactness against the double reference.

## 4. Measurement

Bar: within 2× of 0.213ms on the first cut (≤ 0.43ms); parity is the
follow-up tuning target (LocalSize, unroll of the strided loop).
Before/after rows in the ledger; a miss is a VERDICT entry.
