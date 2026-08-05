# GPU Target: NVPTX / SPIR-V (Future Roadmap)

**Status:** Future target — not in current implementation plan.  
**Prerequisites:** All Phases 0-7 complete, self-hosted compiler parity achieved.

## 1. Pipeline Overview

Briv → LLVM IR → GPU-native virtual assembly → Physical machine code:

```text
  [ Briv Source (.bv) ]
           │
    [ LLVM IR (.ll) ]
           │
    ┌──────┴──────────────────────┐
    ▼ (NVIDIA)                    ▼ (Open / Vulkan)
 [ llc -march=nvptx64 ]       [ SPIR-V Translator ]
    │                              │
    ▼ (Virtual GPU Assembly)       ▼ (Open Shader IR)
  [ .ptx (PTX) ]               [ .spv (SPIR-V) ]
    │                              │
    ▼ (CUDA Driver: ptxas)         ▼ (Vulkan Driver)
 [ SASS (Physical Machine) ]   [ Native GPU Shader ]
```

Both paths are viable because LLVM IR is target-independent. The same Briv source compiles to both.

## 2. Four GPU Optimizations Briv Enables (That CUDA Cannot)

### 2.1 Static Bank Conflict Elimination

GPU shared memory is divided into 32 banks. If two threads in a warp access different addresses in the same bank, a **bank conflict** stalls the pipeline.

**In CUDA:** Programmers manually pad arrays (`shared[row * 33 + col]`) — error-prone.

**In Briv:** The SMT proof engine analyzes array indexing expressions:
```briv
// If compiler proves: thread_id < 32 and stride == 1
// → Zero bank conflicts guaranteed
let shared_data: Int[32];
let val = shared_data[thread_id];
```

The compiler emits LLVM IR with bank-conflict-free access patterns. If a conflict is detected, padding is automatically injected before IR emission.

### 2.2 Guaranteed Memory Coalescing

GPU global memory reads must be coalesced (contiguous blocks from a warp) for peak throughput.

**In CUDA:** Conservative pointer analysis prevents the compiler from proving coalesced access.

**In Briv:** The `noalias` model guarantees disjoint thread memory regions:
```llvm
; Compiler emits aligned vector loads:
%val = load <4 x float>, <4 x float>* %ptr, align 16
; Hardware coalesces this into a single 128-bit transaction
```

### 2.3 Automatic Memory-Tier Placement

GPU has three memory tiers: global (slow), constant (fast, read-only), shared (very fast, block-local).

**In Briv:** The dataflow analysis proves read/write patterns:
- **Read-only across all threads** → `__constant__` memory (cached, low latency)
- **Read-write within a block** → `__shared__` memory (register-speed)
- **Read-write across blocks** → global memory (VRAM)

The compiler emits the corresponding LLVM address spaces:
```llvm
; Constant memory (address space 4 in NVPTX)
@data = addrspace(4) constant [64 x float] zeroinitializer

; Shared memory (address space 3 in NVPTX)
@shared = addrspace(3) global [32 x float] zeroinitializer
```

### 2.4 Absolute Warp Divergence Elimination

When threads in a warp take different branches, all paths execute sequentially — halving or quartering throughput.

**In Briv:** The guard-to-`select` conversion (`05-CONTRACT-TO-METADATA.md`) flattens branches:
```llvm
; Instead of:
;   br i1 %cond, label %then, label %else
;   then: %r1 = fadd float %a, %b; br label %merge
;   else: %r2 = fmul float %a, %b; br label %merge
;   merge: %val = phi [%r1, %then], [%r2, %else]

; Emit (no branch at all):
%val = select i1 %cond, float %add_result, float %mul_result
```

On GPUs, `select` compiles to **predicated execution** — all 32 threads execute both the add and mul, but only the selected result is written. This eliminates warp divergence entirely.

## 3. Compilation Flow (When Implemented)

```bash
# Step 1: Compile Briv to LLVM IR
briv-compiler llvm kernel.bv --out output.ll

# Step 2: Generate PTX for NVIDIA GPUs
llc -march=nvptx64 -mcpu=sm_89 output.ll -o kernel.ptx
# sm_89 = Ada Lovelace (RTX 40xx), sm_90 = Blackwell (RTX 50xx)

# Step 3: Assemble to SASS (via CUDA driver)
ptxas kernel.ptx -o kernel.cubin

# Step 4: Or generate SPIR-V for Vulkan
spirv-as kernel.ll -o kernel.spv
```

## 4. Limitations & Prerequisites

- Requires LLVM with `nvptx64` target (confirmed available in LLVM 18.1.3)
- GPU kernels must not use `reactor_tick()` — they use a flat kernel entry point instead
- No FFI calls inside GPU kernels (no `frgn` — GPUs have no OS)
- Dynamic `List` (un-promoted) cannot be used in GPU kernels without a device-side allocator
- Must wait for Briv-in-Briv self-hosting (Phase 7) before Briv can compile its own GPU kernels