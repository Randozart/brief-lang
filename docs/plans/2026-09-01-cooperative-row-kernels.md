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

## 5. Outcome (same session) — WIP, gated OFF

Landed behind `spirv_row_cooperative: false` (ir-lowering tuning table):

- **Working**: reduction recognition (§1); `SubgroupFAdd#` intrinsic →
  `OpGroupNonUniformFAdd` (Subgroup scope — the scope operand is IdScope,
  a uint CONSTANT reference, not a literal); capabilities; `LocalSize 32`
  for cooperative kernels; runner `(32, rows)` 2D dispatch; harness coop
  mode (`gemv_bench <spv> M K 1`).
- **Verified on device**: the minimal subgroup probe
  (`y[i] = SubgroupFAdd#(x[i])`, 32 lanes, 8 workgroups) returns exact
  results — the intrinsic, scope encoding, and capability set are sound.
- **THE OPEN BUG**: the full cooperative GEMV (strided foreach + reduce)
  executes ONLY workgroup 0; rows ≥ 1 keep their sentinel values in the
  download. Non-cooperative kernels with the same 2D dispatch run all
  workgroups, and the identical blob under 1D dispatch computes row 0
  correctly. So: recognition, synthesis, subgroup op, and dispatch each
  work in isolation; the integration of (synthesized strided foreach +
  subgroup reduce + multi-workgroup 2D) does not.
- **ROOT-CAUSED (same session, follow-up)**: the Y workgroup dimension of
  vkCmdDispatch never took effect on this driver — a gid.y probe kernel
  returned 0 for every workgroup under a (1, 64) dispatch. Every "2D"
  launch to date had been silently running only its X workgroups (the
  pairs 2D verification checked only the fast-forwarded counter, so this
  went unnoticed). Workaround shipped: the cooperative grid is FLATTENED
  into X — the kernel derives `row = gid.x >> 5`, `lane = gid.x & 31`
  (LocalSize 32), and the runner dispatches `rows` workgroups along X.
  The cooperative GEMV now verifies correct end-to-end (max_rel 7.3e-4,
  within tolerance; the reduction tree changes accumulation order vs the
  double reference).
- **Perf verdict**: cooperative = PARITY with the serial kernel (~0.9ms),
  not the hoped 4×. Both are latency/MLP-bound at ~75GB/s effective; the
  remaining gap to ggml-cuda (0.213ms) needs vec4 loads INSIDE the
  cooperative strided loop (lane handles 4 consecutive floats per
  iteration → 512B coalesced per warp-load) — the successor rung. The
  knob stays OFF until that lands.
- M1 UNREGRESSED: 0.93ms, exact gate, 2012 tests, spirv-val clean.

## Rung 2 — vec4 loads inside the cooperative strided loop (2026-09-01, same session)

**Result: M1 = 0.25ms min / 0.28ms avg (~120 GFLOP/s), max_rel_err = 0.000e+00.
1.75× over the scalar cooperative rung (0.44ms), 3.4× over serial (0.93ms);
ggml-cuda gap closed from 4.2× to ~1.2×.**

### Kernel changes (src/backend/spirv/kernel.rs)

- `emit_cooperative_reduce` detects vec4-eligible fields (offset%16==0,
  count%4==0, Float element) via `collect_vec4_indices`; when ALL body fields
  qualify, stride becomes 128 (4 elems × 32 lanes) and `repl = lane*4 + t*128`.
- The vec4 path emits a HAND-BUILT structured loop (`begin_structured_loop` /
  `end_structured_loop` helpers) — the Foreach machinery cannot interleave the
  per-iteration vec4 loads. Each iteration: one `v4float` SSBO load per field
  at `row*(K/4) + lane + t*(stride/4)`, 4 `CompositeExtract`s into synthetic
  `__vec4_<field>_<j>` vars (`FnLowerer.vec4_component_vars`, resolved by
  `emit_expr` without an OpLoad), then the body unrolled 4× with the loop var
  substituted by `repl + j` on the scalar side.
- x[] (Float[4096] at offset 8 mod 16) is NOT vec4-eligible — scalar loads
  stay; the pairing a[4k+j]·x[4k+j] is exact by construction.
- CRITICAL fix found on device: the `acc = 0` initialization was emitted in
  the loop MERGE block (after the loop), wiping the accumulator before the
  subgroup reduce → rel_err exactly 1.0. `split_at_foreach` now splits
  kernel_stmts at the Foreach; pre-statements emit before the loop, post
  statements (reduce + store) after.

### Runtime fixes (lib/runtime/) — three pre-existing bugs that made ANY
cooperative kernel unusable end-to-end

1. **`briev_accel_download` never pulled VRAM→staging** (`briev_accel_rt.c`):
   with the device-local working set, resident launches write the `dev_buffer`
   in VRAM, but the download memcpy'd the STAGING window — stale data. The
   driver comment even said the pull belongs in "briev_accel_download's tail".
   Now calls `g_driver->download_dev` before the copy (0 = all-host fallback,
   not an error).
2. **Dispatch divided by a hardcoded local size** (`briev_dev_vulkan.c`,
   `briev_dev_opencl.c`): both launches divided work items by
   `VK_LOCAL_SIZE_X 256`, but cooperative kernels declare LocalSize 32 → 8×
   too few workgroups. Both drivers now parse `OpExecutionMode LocalSize`
   (opcode 16, mode 17) from the SPIR-V at create_kernel and dispatch by the
   module's actual local size.
3. **Runner cooperative geometry** (`runner.rs` `dispatch_geometry_stmt`):
   emitted `(32, (n+31)/32)` — under the local_x-divided 2D semantics that is
   32× too few workgroups (128 of 4096 rows). Now `(32, n)`: one 32-lane
   workgroup per row. Also fixed the literal-newline-inside-C-string bug in
   the flat path's fprintf.

### Evidence protocol

- Baseline A/B on device (RTX 3060, Vulkan, M=K=4096, WARMUP=5 ITERS=20,
  interleaved runs, all runs max_rel_err = 0.000e+00):
  | variant        | min (ms) | avg (ms) | GFLOP/s |
  |----------------|----------|----------|---------|
  | scalar coop    | 0.437    | 0.494    | ~68     |
  | vec4 coop      | 0.245    | 0.281    | ~120    |
  | ggml-cuda ref  | 0.213    | —        | ~157    |
- spirv-val clean (vulkan1.3); 2012 lib tests green; runner runs standalone.
- Bench dispatch note: `gemv_bench <spv> M K 1` (coop=1 → `(32, M)`) is the
  correct harness geometry for cooperative kernels; `coop=0` dispatches
  `(M, 1)` → `ceil(M/32)` workgroups → only M/32 rows by design.

### Remaining gap to ggml (0.213ms)

~1.2×. Candidates, in order: (a) x[] vec4-eligibility (align x to 16B in the
state projection so both sides load vec4), (b) multiple K-elements per lane
per iteration (ILP), (c) Split-K for small M. The M1 rung knob
(`spirv_row_cooperative`) stays ON — the default path now picks vec4
automatically when fields qualify (MAXIMUM EFFICIENT DEFAULT: no keyword).
