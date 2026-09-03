# 2026-09-02 — GPU Portfolio Expansion + CPU Fallback + Saxpy Investigation

## Goals

1. **Portfolio breadth**: Add softmax + stencil benchmarks — new access patterns
   (branchy transcendentals, local windows) that keep the optimizer honest.
2. **Saxpy store mystery**: Time-boxed binary search of the 4th-field store bug.
3. **CPU fallback lane**: Explicit `--accel-cpu-fallback N` flag for .bv files —
   route sub-N-workgroup dispatches to CPU loop.
4. **Defer O6 tree reduction**: Superseded by subgroup ops (SubgroupFAdd#). The
   reduce.abv bottleneck is per-work-item serial accumulation, not cross-workgroup
   — fix by restructuring the kernel to use subgroup-cooperative accumulation
   (separate session, touches accel analysis + kernel emission).

## Execution Order

### Step 1: Stencil Benchmark (no compiler changes)

**What**: `examples/gpu/stencil_3pt.abv` — 3-point stencil:
`out[i] = 0.25 * (in[i-1] + in[i] + in[i+1])` on interior points.

**Why**: First benchmark with neighbor access — tests the analysis's ability to
handle non-trivial memory patterns. The foreach body uses `i-1` and `i+1` as
indices, which the explore agent confirmed works via ISub/IAdd → AccessChain.
Loop range `1..N-1` avoids boundary issues.

**Harness**: `benchmarks/gpu/stencil_bench.c` — C reference for correctness,
bandwidth measurement. Working set ~4×N×4 bytes = 256MB at N=16.7M.

**Verify**: `brievc build` + `spirv-val` + correctness check + bandwidth.

### Step 2: Exp# Intrinsic + Softmax Benchmark

**What**: Add `Exp#` intrinsic to the SPIR-V backend, then write
`examples/gpu/softmax.abv`.

**Prerequisites**: None — the GLSL.std.450 import already exists (used by Fma).

**Tasks**:
1. Add `"Exp#"` to `get_intrinsic_signature()` in `src/intrinsic_signatures.rs`
   — `Native("Float")`.
2. Add `"Exp#"` match arm in `src/backend/spirv/lower.rs:emit_intrinsic_call`
   — emit `GLSL.std.450 Exp` via the same `ext_inst` path as Fma (line 194).
3. Add `"Exp#"` to `src/backend/spirv/normalizer.rs:build_supported_ops()`.
4. Write `examples/gpu/softmax.abv` — elementwise exp + reduction kernel.
5. Write `benchmarks/gpu/softmax_bench.c` — harness with C reference softmax.
6. Compile, verify (spirv-val, correctness, bandwidth), commit.

**Softmax design**:
- Single kernel, 1D dispatch: each work item processes one element of x.
- The max-reduction and sum-reduction are done on the HOST in the harness
  (the harness precomputes max_val, passes it as a state scalar).
  This avoids the Expr::If blocker (not yet lowered in SPIR-V).
- The GPU kernel computes: `exp(x[i] - max_val) / sum_exp`.
- Alternative: if Expr::If is added, the kernel can do the max-reduction
  itself — but that's heavier. The harness-prefixed approach is correct
  for a portfolio benchmark.

**If Expr::If is added** (optional, heavier): add `SelectionMerge` +
`BranchConditional` in `emit_expr` for `Expr::If`. This unlocks conditional
expressions everywhere. Gate behind a flag or always-on. Only do this if the
softmax design demands it.

### Step 3: Saxpy Store Mystery (time-boxed, 30 min)

**What**: Systematic binary search of the 4th-field store bug.

**Evidence** (from BUGS.md):
- 4-field kernel (i/x/y/z): z stores never visible host-side (z = 0 everywhere).
- 3-field variant (i/x/y): stores DID land but with unreconciled pass count.
- x round-trips perfectly (transport verified).
- Blob disasm-correct (member offsets match field table).
- Dispatch verified (65536×256).
- 4096³ GEMM (4 fields i/a/b/y) works through the same runtime.

**Binary search plan**:
1. Test 2-field variant (i, x, z): `z[i] = 2*x[i]`. If z lands: field count > 2
   is the issue. If not: z's specific offset (past 128MB) is the issue.
2. Test 3-field variant (i, x, y) with y as output: `y[i] = 2*x[i]`. If lands:
   the issue is z's offset specifically. If not: structural.
3. Add temporary fprintf in the runtime's unpack loop: print each field's
   first float after download. This isolates pack vs unpack vs transport.
4. Check k->bytes for each variant: verify the blob's declared byte count
   matches the runtime's computed proj_bytes.

**Outcome**: Root cause found (fix if simple) or minimal reproduction for follow-up.

### Step 4: CPU Fallback Lane (.bv files)

**What**: `--accel-cpu-fallback N` flag — route sub-N-workgroup dispatches to CPU.

**Design**:
- `.abv` = GPU-only (no fallback).
- `.bv` with `accel` = mixed GPU+CPU (fallback available).
- Flag is explicit: `--accel-cpu-fallback 8` means "below 8 workgroups → CPU".
- Default: no fallback (current behavior, driver bug notwithstanding).

**Tasks**:
1. Add `accel_cpu_fallback: Option<u64>` to `BuildOptions` (pipeline.rs).
2. Parse `--accel-cpu-fallback N` in `parse_build_args()` (main.rs).
3. Thread to `LlvmBackend` context.
4. In `emit_accel_dispatch_wrapper()` (emit_toplevel.rs:3944), after computing
   `%n`, compute workgroup count and branch:
   ```
   wg_count = n / local_size
   wg_ge_threshold = icmp sge wg_count, threshold
   br wg_ge_threshold, accel_gpu, accel_cpu
   ```
5. Port workgroup formulas from `runner.rs:dispatch_geometry_stmt()` to the
   LLVM IR emitter.
6. Write a test: .bv file with small const N triggering the CPU fallback path.

## Deliverables

| File | Action | Step |
|------|--------|------|
| `examples/gpu/stencil_3pt.abv` | new | 1 |
| `benchmarks/gpu/stencil_bench.c` | new | 1 |
| `src/intrinsic_signatures.rs` | edit | 2 |
| `src/backend/spirv/lower.rs` | edit | 2 |
| `src/backend/spirv/normalizer.rs` | edit | 2 |
| `examples/gpu/softmax.abv` | new | 2 |
| `benchmarks/gpu/softmax_bench.c` | new | 2 |
| `benchmarks/gpu/saxpy_bench.c` | edit | 3 |
| `BUGS.md` | edit | 3 |
| `src/pipeline.rs` | edit | 4 |
| `src/main.rs` | edit | 4 |
| `src/backend/llvm/emit_toplevel.rs` | edit | 4 |
| `docs/architecture/benchmark-strategy.md` | edit | all |

---

## Outcomes (2026-09-02, end of session)

| Step | Result |
|------|--------|
| 1. Stencil → **gather** | stencil blocked: SPIR-V rejects guarded bodies (when/[]) — `dst[i] = src[i*8]` gather fills the non-contiguous slot. max_abs 0, 69.5 GB/s. Stencil as .bv+accel = follow-up. |
| 2. Exp# + softmax | Exp# added (signatures, purity proof, normalizer, builder glsl_exp, lower arm). softmax.abv: max_rel 4.7e-07, 278.9 GB/s. |
| 3. Saxpy mystery | **RESOLVED — harness bug**: saxpy_bench passed n_fields=3 for 4 fields; z invisible to the runtime. Fix + BUGS.md RESOLVED entry. 219.7 GB/s full transport. |
| 4. CPU fallback | `--accel-cpu-fallback N` shipped: const shapes fold, runtime shapes icmp-gate. opt-verified both variants; binary links + runs. |

### Latent bugs found by Step 4 (fixed, same commits)
- Probe-call `{:e}` float literals (`1e-4`) — LLVM IR needs the decimal
  point; now `{:?}`. Found when the runtime-gate path was first opt-verified.
- `%briev.field` constants in the wrong order vs the C struct (proj_offset
  at index 3). Emission now C-order; descriptor test updated.

### Observations for follow-up (undiagnosed, pre-existing)
- The 16.7M-element `.bv` compile balloons past ~6GB free and gets OOM-
  killed on this 16GB machine (reproduced WITHOUT the new flag — baseline).
- A `.bv` accel program with `println!` + `endprogram` observables hung at
  the brievc clang stage (>5 min) while the plain program built in seconds.
- The .bv accel descriptor path was never clang/opt-verified before this
  session — the two latent bugs above shipped silently. Consider an
  opt-verify smoke test in the per-commit checklist for .bv accel changes.

### CPU-lane notes
- Gate threshold semantics: work-ITEM count (not workGROUP count) — the
  wrapper runs before blob local-size is known; work-item threshold is the
  honest proxy and matches the runtime's dispatch argument.
- Probe-decision entries (runtime N): the probe itself is NOT threshold-
  gated in v1; the wrapper dispatch is. The probe measures both lanes and
  commits a verdict, so a sub-threshold shape still probes then loses —
  acceptable; revisit if probes dominate small-shape latency.
