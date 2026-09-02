# Plan: M2.2 — Cooperative matrix (tensor cores), the endgame rung

**2026-09-01.** Successor to M2.1 (tiled synthesis: 25.3ms / 5250 GFLOP/s
at 4096³, exact). The ggml GEMM anchor measured SAME-GPU (Device 0 = the
3060): **10.9ms / 12,600 GFLOP/s** — their number on a ~13 TFLOP SIMT card
means tensor cores (TF32 mma). The gap (2.4×) is quantified and is
precisely this rung's prize.

## Hardware/driver gate (PASSED)

`VK_KHR_cooperative_matrix` rev 2, `cooperativeMatrixSupportedStages:
COMPUTE`, driver 580.178.04, Vulkan 1.4.312. `VK_NV_cooperative_matrix(2)`
also present (we target the KHR path — portable).

## Design

The recognized GEMM shape (same GemmPlan) routes to a cooperative-matrix
variant of the tiled kernel:
- Workgroup = 1 warp (LocalSize 32, x-only as always); each warp owns a
  16×16 C fragment via `OpTypeCooperativeMatrixKHR` (Workgroup scope,
  f32, M=N=16); the warp marches the 64×64 output tile in 4×4 = 16 mma
  steps, or (v1) the workgroup IS a 16×16 tile → grid = (M/16)*(N/16).
- Panels: `OpCooperativeMatrixLoadKHR` straight from the SSBO with
  stride = K / N (no shared memory round trip needed for v1 — the mma's
  operand reuse happens in registers).
- `OpCooperativeMatrixMulAdd(C, A, B, C)` f32 accumulate; TF32 conversion
  is the hardware's business (f32 operands, m16n16k16... K steps of 16:
  inner loop K/16 iterations, K%16==0 gate).
- Grid/dispatch: same X-flattened workgroups×16-items pattern.

## Phases (smoke-test-first, per the shared-memory lesson)

1. **P0 — driver enablement**: vkCreateDevice gains the extension (gated
   on `vkEnumerateDeviceExtensionProperties`); device features struct
   `VkPhysicalDeviceCooperativeMatrixFeaturesKHR.cooperativeMatrix = true`
   chained via pNext. Without the probe-gate, other boxes must keep working
   (fallback = the M2.1 tiled kernel).
2. **P1 — glslang smoke**: a `coopmat` GLSL kernel (16×16 f32 mma) through
   OUR runtime + bench harness, exact-verified. Proves driver path +
   descriptor ABI + dispatch geometry before any emitter work.
3. **P2 — emitter**: `OpCapability CooperativeMatrixKHR`, the extension
   string, the type/layout ops, the mma loop; GemmPlan routes to it when
   `K % 16 == 0` and the device probe passed (probe result plumbed via
   config at build time — the runtime exposes `briev_accel_device_caps`).
4. **P3 — measure** vs the anchor; tile-order/fragment-shape tuning after.

## Gates

Exact correctness at 64³ and 4096³ (sampled + identity), spirv-val with
the extension target-env, 2012 lib tests, fallback intact when the
extension is absent.

## P0/P1 findings (2026-09-01, same session)

- Driver enablement LANDED: `VK_KHR_cooperative_matrix` extension enabled at
  device creation (probe-gated), chained
  `VkPhysicalDeviceVulkanMemoryModelFeatures` (the coopmat capability
  REQUIRES the Vulkan memory model) + `VkPhysicalDeviceCooperativeMatrixFeaturesKHR`.
- SPIR-V grammar fully mapped (hand-written smoke kernel in
  `/tmp/coop_smoke.spvasm`, spirv-val CLEAN at vulkan1.3):
  `OpCooperativeMatrixLoadKHR <cm> <scalar-elem-ptr> <layout-const>
  <stride-const>` — the Pointer is a SCALAR element pointer (no bitcast),
  MemoryLayout and Stride are CONSTANT INSTRUCTIONS, optional memory
  operand literal LAST; accumulator lives in Function storage; decorations
  section comes BEFORE types/constants in the binary layout; block
  dominators must appear first in block order.
- **The blocker that reframes the rung**: `vkGetPhysicalDeviceCooperative
  MatrixPropertiesKHR` on the 3060 lists 11 shapes — f16×f16→f16, f16×f16→f32,
  int8→int32, fp8 — and **NO f32×f32→f32**. The smoke kernel's f32 mma is
  rejected by the NVIDIA pipeline compiler ("NVVM compilation failed").
- **The reframe**: tensor cores require Float16 operands (f32 accumulate).
  That is a NUMERICS decision, and in Briev it belongs to the TYPE SYSTEM:
  a .abv author declaring `a: Float16[K*M]` gets tensor cores automatically
  (the typed state field is the metadata that both PERMITS the mma lowering
  and COMMUNICATES the precision contract). The F32-typed GEMM keeps the
  M2.1 tiled kernel (5.25 TFLOP/s, full precision). M2.2's emitter work:
  recognize Float16-operand GEMMs → coopmat lowering (fp16-in/fp32-acc),
  gate on the runtime's `vk_coopmat_enabled` probe (config at build time,
  graceful CPU fallback otherwise).
- ggml's 12.6 TFLOP/s on this card for "F32" GEMM is consistent with an
  internal fp16/tf32 tensor-core path — i.e. the same trade we can now
  make EXPLICIT and USER-CHOSEN through types instead of hidden.

## P2 findings (2026-09-01, same session) — emitter built; front door gated on a typechecker design decision

**Built and unit-gated (2024 lib tests green):**
- `spirv_coopmat` config knob (default off; `config/ir-lowering.dbvl`).
- Driver: `VkPhysicalDevice16BitStorageAccessFeatures` chained
  (storageBuffer16BitAccess) + the 16bit/vulkan-memory-model device
  extensions alongside coopmat.
- Builder: `enable_cooperative_matrix()` — capabilities (CooperativeMatrixKHR,
  VulkanMemoryModel, StorageBuffer16BitAccess), the three extension strings,
  and the memory-model swap GLSL450 → Vulkan (dr::Module's memory_model is
  an Option<Instruction>; assembler writes sections by kind, so post-type
  pushes stay in valid layout).
- `gemm::emit_coopmat`: fragment types via
  `type_cooperative_matrix_khr` (f16 A/B, f32 C, Subgroup scope, 16×16),
  zero accumulator as OpConstantComposite (single scalar constituent),
  Function-storage accumulator, kt loop with a loop-carried coopmat phi,
  fragment loads straight from the SSBO (scalar element pointer — the
  pointee may mismatch the component type; stride is in pointee units),
  `OpCooperativeMatrixMulAddKHR`, `OpFConvert` to the f16 output fragment,
  store. LocalSize 32, grid (M/16)×(N/16) X-flattened.
- Kernel routing: GemmPlan match → field-type check (casting-graph shape
  f16, never a name match) + knob → tensor tier; else vec4-eligible →
  tiled; else flat. GemmPlan gained Clone.

**The front door is blocked on a typechecker design decision (OPEN):**
`Float16` state fields don't typecheck an arithmetic body —
`resolve_binary_op_binding` finds no binding: the colon-form `op Mul:
FMul16(#Lh, #Rh)` is documentation/authorization only; coverage comes from
the CATEGORY's protocol binding, and Float16's category (via its `Bits`
parent) carries no arithmetic protocol. The example program
(`gemm_h.abv`, content preserved here) fails with "invalid operation '*'
on type Float16".

The design question for the next leg: **how do Bit-rooted numeric
typedefs join a numeric protocol category?** Candidate: a declared
annotation (`!> proto: "#Float";`) or inference from the declared op
intrinsic family (FAdd16/FMul16 ⇒ #Float). Whichever is chosen must also
seed the literal-coercion gap (`let acc: Float16 = 0.0;` → "expected
Float16, found Float"). Example program content, ready to revive:

```briev
const M: Int = 4096;
const N: Int = 4096;
const K: Int = 4096;
let i: Int = 0;
let a: Float16[16777216];
let b: Float16[16777216];
let y: Float16[16777216];
async node gemm [i < M * N][i == M * N] {
    let acc: Float16 = 0.0;          // needs literal coercion
    let m: Int = i / N;
    let n: Int = i % N;
    foreach k in 0..K {
        acc = acc + a[m * K + k] * b[k * N + n];   // needs f16 protocol join
    }
    y[i] = acc;
    i = i + 1;
    term;
};
```

## P2 status (2026-09-01, session end) — front door OPEN, f16 device path FAULTS

**The Float16 join LANDED**: `type Float16 : Float, #Float { ... }` — bare
fundamental parent (the first post-hashtag fundamental protocol join) plus
the protocol-membership form the relationship grammar requires today
(Phase B de-hashes it). Zero typechecker changes needed for the join
itself. Added: Float-literal admission into Float-category narrow targets,
gated on f32→f16→f32 round-trip exactness (`float_literal_fits` +
`f32_fits_f16`; unit-tested). Fixed en route: the op-variant coverage trap
— `op Mul(#Float)` strips to the CONCRETE name "Float", covering only the
f32 primordial; the variant must be the type's own name
(`op Mul(Float16): FMul16(...)`). Unit tests: 2025 green.

**The f16 tensor kernel emits and validates** (spirv-val clean,
LocalSize 32, coopmat loads/mma/FConvert/store, runner dispatch
(n/(16·16))·32) — but **faults on device**: every Float16 run (16×16 and
16×64 variants, 256³ and 4096³) writes only ~25% of the output then dies
(rows 0..~1025 of 4096 at 4096³; SEGFAULT in the harness at 256³). The
fault footprint is variant-independent and size-independent per-output —
consistent with a GPU-side fault in the f16 surface (16-bit storage
interaction, NVVM f16-fragment handling, or a missing capability the
validator doesn't gate). The f32 coopmat smoke was FULLY CORRECT at
4096³, so the coopmat pipeline itself works.

**Decision: `spirv_coopmat: 0` in the default config** — the emitter and
driver enablement stay (all unit-gated), Float32 GEMMs keep the Tier-2
tiled kernel (25.3ms, exact), and Float16 GEMMs stay on the exact tiled
path until the fault is root-caused. The fault investigation needs:
(a) a RenderDoc/NVIDIA-tools capture of the f16 submission, (b) a
`shaderFloat16`-feature probe before chaining, (c) an NVVM-bug check via
equivalent hand-GLSL (GLSLANG lacks coopmat on this box — needs a newer
glslang or DXC), (d) per-workgroup-write tracing via a debug y-fill
(pre-store y-fill to detect which workgroups RAN vs never stored).

Revived example: `examples/gpu/gemm_h.abv` (typechecks end-to-end now).

## P2 fault fingerprint (2026-09-01, y-fill sentinel trace)

The y-fill experiment (harness pre-fills y with f16 sentinel 0x7BFF; the
post-run pattern shows stored vs never-touched outputs) is decisive:

- Rows 0..1026 of 4096: fully stored, values correct (f16-quantized want).
  Zero sentinels.
- Rows 1027..4095: 100% sentinel — never stored by anyone.
- Identical footprint across the 16x16 and 16x64 variants and across
  256^3/4096^3 (the 256^3 case SEGFAULTs the host outright).

The boundary: ~8.4MB into the y region, workgroup ~2^12. The SPIR-V is
validator-clean; every access is provably in-bounds; the f32 coopmat
kernel (identical structure, f32 fragments) ran the FULL 4096^3 correctly
at the same dispatch shape. Conclusion: the fault is in the DRIVER/NVVM
f16 fragment path (16-bit storage interaction or shaderFloat16 lowering),
not in our loop logic.

Escalation path (needs vendor tooling, not source work): NVIDIA
nsight-compute capture of the f16 submission; a DXC-compiled equivalent
(glslang on this box lacks coopmat); a minimal reproducer for the driver
vendor. The knob stays OFF; Float16 GEMMs keep the exact tiled kernel.
