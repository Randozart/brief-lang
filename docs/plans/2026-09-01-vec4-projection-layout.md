# Plan: Vec4-eligible projection layout (close the remaining GEMV gap)

**2026-09-01.** Successor to `2026-09-01-cooperative-row-kernels.md` (vec4
inside the cooperative strided loop — landed, M1 0.93ms → 0.25ms).

## Objective

`x[]` (Float[4096]) sits at projection offset 67108872 = 8 mod 16, so the
vec4 gate (`offset % 16 != 0`) rejects it and the cooperative loop loads the
x side with 4 scalar loads per lane-iteration while `a[]` gets one `v4float`
load. Aligning vec4-eligible arrays to 16B in the DEVICE projection makes x
qualify automatically — both sides load vec4, and the 4 scalar FMAs can
collapse into one componentwise `OpFAdd` on a `v4float` accumulator.

Goal: M1 GEMV from 0.25ms toward ggml-cuda's 0.213ms (currently ~1.2×).

## Baseline (current commit e4996fa2+docs, RTX 3060, M=K=4096, WARMUP=5 ITERS=20)

| variant | min (ms) | avg (ms) | GFLOP/s | max_rel_err |
|---|---|---|---|---|
| serial (pre-cooperative era) | 0.89 | 0.93 | ~36 | 7.3e-4 |
| cooperative scalar | 0.437 | 0.494 | ~68 | 0.000e+00 |
| **cooperative vec4 (current)** | **0.245** | **0.281** | **~120** | **0.000e+00** |
| ggml-cuda (the bar) | 0.213 | — | 157.8 | — |
| ggml-cpu 1T | 4.4 | — | 7.7 | — |

A/B after each phase with `benchmarks/gpu/gemv_bench <spv> 4096 4096 1`
(interleaved runs ×3); full-suite regression via
`bash benchmarks/compare_baseline.sh` at the end (this leg touches GPU
projection layout only — CPU codegen untouched).

## Design: one layout rule, compiler-owned

The packed name-sorted layout is implemented THREE times today (drift
hazard): `lower.rs::setup_state_buffer` (SPIR-V member offsets),
`runner.rs::ssbo_layout` (runner state + BrievField literals), and
`briev_accel_rt.c::proj_field_offset` (packed sums). Plus the bench
hardcodes it. The rule change — "a field the vec4 gate would accept
(Float32 array, count % 4 == 0) is aligned up to 16B" — must land in ONE
place with consumers deriving from it.

1. **Canonical rule** lives with the vec4 gate in `lower.rs`:
   `projection_offsets(state_fields, builder) -> Vec<u32>` — name-sorted,
   eligible arrays aligned up to 16, everything else packed. Returns the
   DEVICE projection offsets.
2. **`setup_state_buffer`** uses it for `OpMemberDecorate Offset`. The
   vec4 eligibility check is unchanged — aligned offsets make x qualify
   with zero new special cases (MAXIMUM EFFICIENT DEFAULT).
3. **`runner.rs::ssbo_layout`** calls the same rule: `RunnerField` gains
   `proj_offset` (host stays packed — the runner's `state[]` size and S_
   macros are unchanged; host ≠ device is already handled per-field by
   the RT). Emits `proj_offset` into the `BrievField` literals.
4. **`BrievField` gains `uint64_t proj_offset`** (lib/runtime/briev_accel_rt.c):
   `proj_field_offset()` is DELETED — the RT seeds/syncs/downloads through
   the declared offsets. `proj_size()` becomes max(proj_offset + bytes).
   The packed-sums C implementation of the rule dies with it.
5. **LLVM/.bv descriptor path** (`llvm/kernel.rs`): field entries gain the
   same `proj_offset` from the shared rule (matched by name). The .bv
   `%State` host layout is untouched (LLVM side, packed).
6. **Bench** (`gemv_bench.c`): declares the aligned offsets with a comment
   pointing at the rule.

## Phases

- **P1 — layout + alignment plumbing** (no behavior change for a[]):
  rule fn, RT BrievField.proj_offset, runner + llvm + bench emission.
  Gate: gemv still 0.000e+00, ~0.25ms (padding only shifts x by 8 bytes).
- **P2 — x vec4 loads** (automatic from P1): kernel declares x as
  `_arr_v4float`; the cooperative field_data loop already handles multiple
  vec4 fields — synthetic `__vec4_x_j` vars join the body. 5 loads/lane-iter
  → 2. Gate: correctness 0.000e+00; measure.
- **P3 — vector accumulator**: accumulate `OpFAdd v4float` (componentwise
  GLSL Fma on vectors) instead of 4 scalar FMAs; 4 `CompositeExtract` +
  final scalar `OpFAdd` hoisted AFTER the loop, then subgroup reduce.
  4 FMAs/lane-iter → 1. Gate: measure; keep only if it moves the number.

## Risks / blast radius

- `.bv` accel programs: descriptor proj_offset must match the kernel —
  covered by the shared rule fn; existing llvm tests at `llvm/tests.rs`
  (field table literals) get updated expectations.
- OpenCL driver: consumes `proj_bytes` only — updated via `proj_size`.
- Downstream projects embedding the RT ABI: BrievField is a C struct in a
  copied runtime — ABI bump is contained to this repo's runtime copies.
- State size for .abv: unchanged (host stays packed; padding is
  projection-only, ≤ 3 × 12 bytes of inert gaps at current field counts).

## Tests

- Unit: `projection_offsets` — eligible arrays aligned, scalars packed,
  deterministic under name sort; property: every vec4-eligible offset ≡ 0
  (mod 16).
- SPIR-V: member-decorate offsets equal the rule's output (existing layout
  tests extended with an 8-byte scalar before a Float array — the x case).
- Runner: emitted `BrievField` literals carry proj_offset; S_ macros still
  packed.
- RT: `proj_field_offset` gone; grep-clean.
- Integration: gemv end-to-end 0.000e+00; `cargo test --lib` green;
  spirv-val clean; praetor clean on changed files.

## Docs to update (same commit as the structural change)

- `docs/architecture/backend-contracts.md`: BrievField ABI (proj_offset),
  layout rule ownership.
- `docs/HANDOFF-2026-09-01-gpu.md`: trap list (stale proj_field_offset),
  ladder table after P2/P3.
- `docs/plans/2026-09-01-cooperative-row-kernels.md`: reference this plan.
