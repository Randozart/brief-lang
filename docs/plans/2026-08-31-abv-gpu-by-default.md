# .abv GPU-by-default + first end-to-end GPU execution

**Date:** 2026-08-31
**Status:** B1–B3 + SPIR-V surface + dispatch wiring LANDED. Remaining work
below. 

## Design model (locked, 2026-08-31 — user)

- **`.abv` IS the GPU language.** Native GPU code: every eligible counted
  loop is a kernel, GPU is the default and only target. Compiles to a
  spirv-val-valid `.spv` binary. No host, no CPU fallback in the artifact.
- **`.bv` IS the CPU language** — with *annotated* GPU offload
  (`!> accel:` policy + optional per-node `accel` keyword) so GPU code can
  live inside a regular reactive script. Offload is probe-verified: the
  GPU lane runs only when it beats the vectorized CPU fold on the actual
  device/workload. `.bv` never silently requires a GPU.

Everything below serves that split: `.abv` gets a first-class runtime
(its kernels must run standalone); `.bv` gets an offload lane that is
correct by proof and profitable by measurement.

## Remaining work (in order)

1. **Fold-strategy defer (item 4, the last .bv blocker).** The multi-node
   fold inlines kernel bodies into `main` — wrappers exist but are dead
   code LTO removes. Every fold branch that would absorb an accel-kernel
   node (multi-txn pure fold, single-node counter fold, SSA pipeline,
   switch-dispatch) must exclude that node and route it through
   `@txn_<name>` (the wrapper), keeping non-kernel siblings folded.
   Verification: the .ll contains live `call void @txn_<name>` from the
   hot loop; nbody output still matches C; probe verdict observable.
2. **First measured GPU-vs-CPU verdict.** After (1): nbody_newton_accel
   BODYCOUNT sweep (1024→4096), probe commits, results recorded in
   `benchmarks/results/`. If the probe commits CPU everywhere, measure
   WHY (launch overhead vs kernel time) before touching the driver.
3. **Vulkan driver hardening (by measurement).** Persistent buffers +
   descriptor sets allocated once; `ceil(n/64)`×64 dispatch (LocalSize is
   already 64×1×1); proper memory-type search; push constants or UBO for
   scalars. Each change A/B'd on the probe.
4. **`.abv` standalone runtime.** A host runner (`brievc run x.abv` or a
   generated `x_runner.c`) that: loads the `.spv`, maps the SSBO, runs
   the phase machine (the .abv node graph IS a reactive program), prints
   observables. Reuses briev_accel_rt; the .abv compile must then emit
   the schedule/descriptor tables alongside the kernel.
5. **Performance primitives** (only after 1–4 establish honest numbers):
   shared-memory scratchpads, workgroup reductions, persistent buffers
   across launches for iterative kernels (nbody steps re-upload the whole
   state every dispatch today).
**Scope:** `.abv` route defaults, kernel-emission blockers, first real GPU
execution on this machine (2x NVIDIA, Vulkan 1.4, spirv-val/dis, llc 22.1.8
with spirv64 target — all present locally).

## Investigation findings (2026-08-31)

Two GPU mechanisms exist and must not be conflated:

1. **Standalone `.abv` → `.spv`** (`src/backend/spirv/`, plan
   `2026-08-23-spirv-kernel-emission.md`): ALL sections landed; 15 tests
   green including spirv-val. It writes a binary and stops — no host
   runner. The §2.5 "Vulkan smoke test" is a placeholder that probes for
   third-party runners and skips.
2. **`accel` offload in `.bv`** (plan `2026-08-06-accel-gpu-offload.md`):
   frontend analysis → LLVM kernel emission via `llc --mtriple=spirv64` →
   blob embedded in host module → `briev_accel_rt.c` dispatch (Vulkan →
   OpenCL → CPU) with auto-tuning probe. This is the only path that
   EXECUTES on a GPU today — and it is currently broken (below).

### Blockers found (blocking ANY GPU execution today)

- **B1 — llc emits SPIR-V text, not binary.** `compile_to_spirv`
  (`src/backend/llvm/kernel.rs`) runs llc without `-filetype=obj`. LLVM
  22's spirv64 backend emits SPIR-V *assembly text* at the default
  `-filetype=asm` (older LLVM emitted binary). The embedded blob is
  ASCII ("OpCapability Kernel…"). Fix: pass `-filetype=obj`.
- **B2 — embedded blob IR is unparseable > 32 bytes.** `embed_spirv_blob`
  wraps the byte string every 32 bytes with `""` + newline continuation;
  clang rejects juxtaposed `c"…"` segments (constant expression type
  mismatch). Reproduced standalone; every real kernel exceeds 32 bytes,
  so linking ANY accel program fails today. Fix: emit one long line (LLVM
  has no length limit) or single-token concatenation clang accepts.
- **B3 — design correction (user, 2026-08-31): `.abv` assumes GPU by
  default.** The extension is the accel intent: no `!> accel:` metadata
  and no `accel` node keyword required. Route-level default: module
  accel policy = `try_all` for `.abv` sources unless the module sets an
  explicit `!> accel:` binding. Eligibility proofs still gate (a body
  must still be a provable data-parallel map); a file with no eligible
  kernel errors helpfully.

### Known hardening debts in `briev_dev_vulkan.c` (after B1–B3 unblock)

Carried from the legacy port, disclosed in its header; fix as measured:

- Per-launch VkBuffer/VkDeviceMemory create+destroy churn (perf).
- Descriptor pool `max_sets=1`, never reset, return unchecked → second
  launch exhausts the pool (correctness for any multi-launch program).
- `vkCmdDispatch(n,1,1)` = n workgroups of 1 thread (works numerically
  — GlobalInvocationId.x == item i — but wastes 31/32 lanes). Dispatch
  should be `ceil(n/wg)` × workgroup size.
- Memory type 0 assumed host-visible; fence 1s timeout destroyed+recreated.

The standalone SPIR-V backend already emits `LocalSize 64,1,1`.

## Landed (2026-08-31, this session)

- **B3** — `.abv` injects `accel: try_all` module policy at parse time
  (`apply_abv_accel_default`, src/compile.rs). Annotation-free `.abv`
  kernels compile: `examples/gpu/{mini,scale,fmadd}.abv` → spirv-val PASS
  (int + float, OpIMul/OpFMul, LocalSize 64×1×1).
- **B1** — `-filetype=obj` in `compile_to_spirv`; magic-bytes test.
- **B2** — single-token blob embedding; >32-byte round-trip test.
- SPIR-V capability table updated to truth (`index`, `floats`, `casts`).
- **Normalizer strip regression fixed** — the keep-set property strip
  deleted the `bits`/protocol-category keys the accel proof and
  `resolve_spirv_shape` read; every real-pipeline .abv failed flatness
  while fresh-universe unit tests passed.
- **collect_state_fields** now reads the parser's real top-level form
  (`Statement(Let)`) alongside `StateDecl`.
- Float kernel lane (F* opcodes by protocol shape), unary ops, scalar
  casts (SConvert/UConvert/FConvert/ConvertSToF/…, identity passthrough),
  module-const materialization in kernels.
- Generalized work-item bound extraction (`X && i < N` conjunctions —
  phase-gated kernels like step_bodies were rejected).
- llc float literals: hex double-bit form (LLVM 22 SPIR-V backend rejects
  decimal floats); f32-exact widening.
- Descriptor-table dedup (read-write buffers were double-listed →
  redefinition of string globals); descriptor indices kept stable with
  empty-blob CPU-fallback slots.
- **Wrapper pre-registration** — accel_kernel_idx built at the TOP of
  generate() (it was populated after host emission, so NO dispatch wrapper
  was ever emitted); reactor_tick and the async body path now route accel
  nodes through the wrapper.
- Runtime: driver init()/create_kernel failures now mark the chain dead
  (previously ignored → "GPU chosen", launches no-op); per-kernel empty-blob
  fallback; `BRIEV_ACCEL_VERBOSE=1` diagnostics.
- llc keeps the failing kernel IR beside the error message.

## Remaining work

4. **Loop-strategy bypass (the last GPU-lane blocker for .bv offload).**
   The multi-node fold inlines kernel bodies into `main` (vectorized CPU,
   C-competitive, correct) — wrappers exist but are dead code LTO removes.
   The fold strategy selection (mod.rs ~3921 "Multi-txn all-pure folding"
   and the pure-counter/single-node paths) must exclude accel-kernel nodes
   so they dispatch through `@txn_<name>`. Then the probe compares a
   vectorized CPU loop against a real GPU dispatch and commits honestly.
5. **First verified GPU speedup** — after (4): nbody_newton_accel at
   BODYCOUNT≥2048, probe verdict recorded in benchmarks/results/, driver
   hardening items below by measurement.
6. Vulkan driver hardening (per-launch buffer churn, descriptor pool
   max_sets=1, dispatch shape ceil(n/64)×64, memory-type selection).

## Verification

- `cargo test --lib` green (new: route-default tests, magic-bytes test,
  embed round-trip).
- `/tmp`-free in-repo fixture: a .abv kernel file with NO accel
  annotations → `.spv` that passes `spirv-val`.
- nbody_newton_accel: correct output at a print-crossing bound vs
  `nbody_newton_accel_c`.
