# GPU Offloading via SPIR-V + Vulkan Compute

**Date added:** 2026-06-18
**Phase:** 4

---

## Architecture

Briv's GPU offloading uses a **dual compilation** model. When `--gpu-offload`
is active (or `#gpu`/`#?gpu` directives are present), the compiler emits TWO
outputs from a single source:

1. **Native CPU binary** via the existing LLVM x86/ARM backend
2. **SPIR-V blobs** via `llc --mtriple=spirv64-unknown-unknown`

At runtime, `briv_gpu_rt.c` dispatches the SPIR-V kernels via Vulkan compute,
with a transparent CPU fallback when Vulkan is unavailable.

---

## Compilation Flow

```
Briv source
    │
    ├──→ Normal LLVM IR codegen →
    │       GPU loops become dispatch calls to briv_gpu_rt functions
    │       (with CPU fallback baked in)
    │
    └──→ SPIR-V kernel extraction (gpu.rs) →
            For each GPU-eligible loop:
              1. Clone + rewrite AST for SPIR-V buffer semantics
              2. Emit as LLVM IR with spirv64-unknown-unknown triple
              3. llc → .spv binary blob
              4. Embed in .rodata of the native binary
```

---

## CLI Flags

| Flag | Effect |
|------|--------|
| `--gpu-offload` | Treat all transactions as candidates for GPU offloading |
| `--target spirv64` | Emit standalone SPIR-V module (no native binary) |

---

## Kernel Eligibility (gpu.rs)

A transaction body is GPU-eligible when:

1. No FFI calls in the body (purity)
2. No loop-carried dependencies (parallelizable)
3. Contiguous memory access patterns (coalesced reads/writes)
4. Bounded iteration count (known or provably finite)
5. No `term`/`term!`/`unification`/`escape` statements
6. Only integer and float types (no String, HashMap, struct, enum)

`GpuEligibility` struct reports eligibility + reasons for rejection.

---

## Runtime Library (briv_gpu_rt.c)

The Vulkan compute runtime is a C library that:

- Loads `libvulkan.so.1` dynamically via `dlopen`
- Creates a Vulkan instance + compute device
- Manages device memory allocation for storage buffers
- Loads embedded SPIR-V as shader modules
- Dispatches compute pipelines with configurable workgroup sizes
- Falls back gracefully: `briv_gpu_is_available()` returns 0 when Vulkan
  is not present, triggering the CPU path

**API:**

```c
int     briv_gpu_init();
int     briv_gpu_is_available();
int64_t briv_gpu_malloc(size_t bytes);
void    briv_gpu_free(int64_t handle);
void    briv_gpu_memcpy(int64_t dst, int64_t src, size_t bytes, int dir);
void    briv_gpu_launch(void* kernel_spirv, size_t kernel_size,
                         int grid_x, int block_x,
                         const int64_t* buffer_handles, int num_buffers);
void    briv_gpu_shutdown();
```

SPIR-V is the Vulkan shader format — no translation needed. The same `.spv`
binary works on NVIDIA (NVK), AMD (RADV), Intel (ANV), Apple (MoltenVK),
and software (LLVMPipe/Mesa).

---

## Performance Considerations

The Vulkan runtime uses storage buffers (`VK_DESCRIPTOR_TYPE_STORAGE_BUFFER`)
for state field access. Key optimization opportunities:

1. **Buffer coalescing**: Map multiple state fields into one storage buffer
   at different byte offsets (fewer descriptor bindings)
2. **PGO integration**: Record actual loop bounds during instrumented runs
   to compute precise crossover points for `#?gpu` decisions
3. **Workgroup sizing**: Tune `block_x` based on field access patterns and
   GPU architecture (warp/wavefront size)
