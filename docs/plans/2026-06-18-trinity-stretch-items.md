# Stretch Items — OpenCL Backend, PGO + GPU, channel-map.md

**Date:** 2026-06-18
**Status:** Implementation

---

## Item 1: `--gpu-backend opencl`

### What
Add an OpenCL dispatch path to `briv_gpu_rt.c`, toggled by `--gpu-backend`
flag. Default is `vulkan`. Both can coexist (`--gpu-backend vulkan,opencl`).

### Changes

| File | Change |
|------|--------|
| `main.rs` | Parse `--gpu-backend <vulkan,opencl,vulkan,opencl>` flag |
| `backend/llvm/mod.rs` | Store `gpu_backend` on `LlvmBackend` |
| `lib/runtime/briv_gpu_rt.c` | Add `dlopen("libOpenCL.so.1")` path, `clCreateProgramWithIL` dispatch, OpenCL kernel launch. Vulkan preferred, OpenCL fallback. |

### How it works
SPIR-V is consumed natively by both Vulkan (`vkCreateShaderModule`) and
OpenCL (`clCreateProgramWithIL`). The SPIR-V blobs are identical — only
the runtime dispatch API changes.

`briv_gpu_init()` tries Vulkan first. If `--gpu-backend` includes `opencl`
and Vulkan fails, it falls back to OpenCL. If neither is available,
`briv_gpu_is_available()` returns 0.

---

## Item 2: PGO + GPU Cost Model

### What
When PGO data is available, use the recorded loop bounds as `N` in the
cost model instead of `0` (runtime-determined).

### Changes

| File | Change |
|------|--------|
| `backend/llvm/mod.rs` | Pass PGO profile to `collect_gpu_kernel`; query loop bound from profile |
| `analysis/gpu_cost.rs` | Accept `pgo_n: Option<u64>` parameter — use recorded bound when available |

### How it works
The `PgoProfile` stores iteration counts for hot loops. When
`--gpu-offload --pgo-generate` both execute, the second compilation has
actual loop bounds. The cost model uses these instead of guessing.

---

## Item 3: `channel-map.md` Update

### What
Document the GPU offloading pipeline in the architecture docs.

### Changes

| File | Change |
|------|--------|
| `docs/architecture/channel-map.md` | Add GPU kernel extraction → SPIR-V codegen → Vulkan/OpenCL dispatch flow |
