# Brief Compiler — SPIR-V & LUT Matmul Implementation Cookbook

**Date:** 2026-06-04 08:14 UTC
**Parent:** `2026-06-04-vitriol-integration-brief-plan.md`
**Status:** Design reference — ready for implementation after vectorization syntax lands

---

This document provides the concrete LLVM IR patterns, address space mappings, kernel emission designs, FFI patterns, and implementation guidance for Brief's SPIR-V backend and CPU LUT matmul compilation.

---

## 1. SPIR-V Target Header Block

When `--target spirv64` is specified, `emit_header` (src/backend/llvm.rs:1785) produces this header instead of the x86_64 header:

```llvm
; ModuleID = 'program.ll'
source_filename = "kernel.bv"

; SPIR-V 64-bit logical addressing, Vulkan 1.2 compute
target datalayout = "e-i64:64-v16:16-v24:32-v32:32-v48:64-v64:64-v96:128-v128:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n8:16:32:64"
target triple = "spirv64-unknown-vulkan1.2"

; Memory model: Logical addressing (0), VulkanKHR memory model (3)
!spirv.MemoryModel = !{!0}
!0 = !{i32 0, i32 3}
```

### What Changes from x86_64

| Aspect | x86_64 | SPIR-V |
|--------|--------|--------|
| `target datalayout` | `e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128` | SPIR-V logical layout (see above) |
| `target triple` | `x86_64-unknown-linux-gnu` | `spirv64-unknown-vulkan1.2` |
| Runtime declares | `@__rt_init`, `@__rt_poll`, `@__rt_wait` | **None** — no host runtime in kernels |
| Thread pool | `@brief_thread_pool_init`, `@brief_barrier_*` | **None** — parallelism via `vkCmdDispatch` |
| `@llvm.assume` | Declared and used for range metadata | **Replace** with `!spirv.Decorations` metadata |

### Implementation in `emit_header`

```rust
// In src/backend/llvm.rs, modify emit_header:

fn emit_header(&self, out: &mut String) {
    match self.target_triple {
        TargetTriple::X86_64 => {
            writeln!(out, "; ModuleID = 'program.ll'").ok();
            writeln!(out, "source_filename = \"program.bv\"").ok();
            writeln!(out, "target datalayout = \"e-m:e-p270:...\"").ok();
            writeln!(out, "target triple = \"x86_64-unknown-linux-gnu\"").ok();
        }
        TargetTriple::Spirv64 => {
            writeln!(out, "; ModuleID = 'program.ll'").ok();
            writeln!(out, "source_filename = \"kernel.bv\"").ok();
            writeln!(out, "target datalayout = \"e-i64:64-...\"").ok();
            writeln!(out, "target triple = \"spirv64-unknown-vulkan1.2\"").ok();
            writeln!(out).ok();
            writeln!(out, "!spirv.MemoryModel = !{{!0}}").ok();
            writeln!(out, "!0 = !{{i32 0, i32 3}}").ok();
        }
        TargetTriple::Nvptx64 => {
            // Phase 5
        }
    }
}
```

---

## 2. Address Space Mapping

### New Address Space Enum Values

In `src/analysis/address_space.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AddressSpace {
    Ddr4,             // 0x00000000 - 0xFFFFFFFF: CPU accessible main memory
    Mmio(u64),        // MMIO range, CPU accessible via bus
    FpgaInternal,     // FPGA internal BRAM/URAM, NOT CPU accessible
    CrossWorkgroup,   // GPU global memory (SSBO) — SPIR-V addrspace(1)
    Workgroup,        // GPU shared memory — SPIR-V addrspace(3)
    Uniform,          // GPU uniform / push constants — SPIR-V addrspace(2)
    Private,          // GPU private registers — SPIR-V addrspace(0) / default
    Unknown,
}
```

### SPIR-V Address Space Mapping Table

| Brief Semantic | LLVM Addrspace | SPIR-V StorageClass | Vulkan Binding | Use Case |
|---|---|---|---|---|
| `Ddr4` / host memory | 1 | `CrossWorkgroup` | SSBO (storage buffer) | LUT data, input/output buffers |
| `Private` / local vars | 0 (default) | `Function` | (none) | Loop counters, accumulators |
| `Uniform` / kernel params | 2 | `Uniform` | UBO or push constants | `layer_id`, dispatch config |
| `Workgroup` / scratch | 3 | `Workgroup` | (shared memory) | Reduction buffer, cache |

### Implementation in LLVM Backend

```rust
// In src/backend/llvm.rs, when emitting pointers:

fn emit_ptr_type(&self, out: &mut String, pointee_type: &str, as: &AddressSpace) {
    let spv_as = match self.target_triple {
        TargetTriple::Spirv64 => match as {
            AddressSpace::Ddr4 | AddressSpace::CrossWorkgroup => 1,
            AddressSpace::FpgaInternal | AddressSpace::Workgroup => 3,
            AddressSpace::Uniform => 2,
            _ => 0, // Private / Function
        },
        _ => 0, // x86_64 doesn't use explicit address spaces
    };
    if spv_as != 0 {
        write!(out, "ptr addrspace({}) ", spv_as);
    } else {
        write!(out, "ptr ");
    }
}
```

---

## 3. Kernel Entry Function Emission

### New Function: `emit_kernel`

In `src/backend/llvm.rs`, add a function to emit SPIR-V kernel entry points:

```rust
fn emit_kernel(&self, out: &mut String, txn: &Transaction) {
    // A SPIR-V kernel function has:
    //
    // 1. Calling convention: spir_kernel
    // 2. All buffer parameters: ptr addrspace(1) (CrossWorkgroup SSBO)
    // 3. Kernel metadata: !spirv.ExecutionMode for workgroup size
    // 4. Parameter decorations: !spirv.ParameterDecorations for Restrict

    let kernel_name = &txn.name;

    // Emit function declaration
    writeln!(out).ok();
    writeln!(out, "define spir_kernel void @{}(\\", kernel_name).ok();

    // State pointer → addrspace(1) SSBO
    writeln!(out, "    ptr addrspace(1) %state,\\").ok();

    // LUT data → addrspace(1) SSBO
    writeln!(out, "    ptr addrspace(1) %lut_data,\\").ok();

    // Layer ID → i32 (passed as push constant or uniform)
    // (This could also be a regular i32 parameter that maps
    //  to a SpecializationConstant or PushConstant)
    writeln!(out, "    i32 %layer_id").ok();
    writeln!(out, ") {{").ok();

    // Get global invocation ID → replaces loop index
    writeln!(out, "  %id_raw = call spir_func i64 @_Z33__spirv_BuiltInGlobalInvocationIdi(i32 0)").ok();
    writeln!(out, "  %id = trunc i64 %id_raw to i32").ok();

    // (kernel body emitted here by the normal emit_body path,
    //  but with loop indices replaced by %id)

    writeln!(out, "  ret void").ok();
    writeln!(out, "}}").ok();

    // Emit kernel metadata
    writeln!(out).ok();
    writeln!(out, "; Kernel entry point declaration").ok();
    let kernel_id = 1; // should be a unique ID counter
    writeln!(out, "!spirv.EntryPoint = !{{!{}}}", kernel_id).ok();
    writeln!(out, "!{} = !{{i32 {}, i32 3, !\"{}\", !{{}}}}",
             kernel_id, kernel_id + 100, kernel_name).ok();

    // Workgroup size: LocalSize(256, 1, 1)
    writeln!(out, "!spirv.ExecutionMode = !{{!{}}}", kernel_id + 1000).ok();
    writeln!(out, "!{} = !{{void ()* @{}, i32 17, i32 256, i32 1, i32 1}}",
             kernel_id + 1000, kernel_name).ok();
}
```

### When to Emit a Kernel vs. a Regular Function

| Condition | Emit as |
|-----------|---------|
| `DispatchMode::Parallel` AND target is SPIR-V | `spir_kernel` with `get_global_id` |
| `DispatchMode::Sequential` AND target is SPIR-V | Regular `spir_func` (called by a kernel wrapper) |
| Any mode AND target is x86_64 | Regular function (existing behavior) |

### Pascal FP16 Constraint (Phase 5 — NVPTX Backend)

**Critical hardware caveat:** The GTX 1070 Ti (Pascal, sm_6.x) has **no native hardware support for packed FP16 arithmetic**. FP16 operations on Pascal are emulated via FP32 conversion and run *slower* than native FP32.

This affects the NVPTX and SPIR-V backends in two ways:

1. **LUT data storage** (host side): FP16 is fine — the CPU LUT path benefits from the 50% memory savings.

2. **GPU compute kernels** (device side): When emitting PTX or SPIR-V for Pascal targets, the kernel must use **FP32** types even if the LUT data is FP16. The kernel loads FP16 values and converts to FP32 before arithmetic.

```llvm
; Correct for Pascal: load half, convert to float, compute in float
%val16 = load half, ptr addrspace(1) %lut_ptr
%val32 = fpext half %val16 to float
%result = fadd float %val32, %acc

; WRONG for Pascal — this is slow (emulated):
; %result16 = fadd half %val16, %acc16
```

In the `.bv` source, this means:
```brief
# When baking for Pascal target:
#   LUT stored as Float16 on host
#   GPU kernel loads as Float16, promotes to Float32 for computation
# When baking for Turing+ target:
#   LUT can stay as Float16 end-to-end
```

The `data_format` field in the `.vpo` section entry tells the runtime which format the LUT is stored in. The kernel emission code checks `g_device_caps.cuda_cc_major` and selects the appropriate float type for GPU compute kernels.

---

## 4. Global ID Access Pattern

The research confirmed that the canonical way to access the global invocation ID in LLVM IR targeting SPIR-V is:

```llvm
; Declare builtin
declare spir_func i64 @_Z33__spirv_BuiltInGlobalInvocationIdi(i32)

; Use in kernel body:
%id_raw = call spir_func i64 @_Z33__spirv_BuiltInGlobalInvocationIdi(i32 0)
; Parameters: i32 0 = x dimension, i32 1 = y, i32 2 = z
; Returns: i64 (must truncate to i32 if using 32-bit indices)
```

### Integration into Brief's Parallel Dispatch

When `DispatchMode::Parallel` is active and the target is SPIR-V, the loop over array elements is replaced by per-thread element access:

```rust
// In emit_expr, for DispatchMode::Parallel + Spirv64 target:

fn emit_parallel_body(&self, out: &mut String, body: &[Statement],
                       array_size: &Expr, element_type: &Type) {
    // Instead of emitting a loop:
    //   for i64 %i = 0 to %N { body(%i) }
    //
    // Emit per-thread dispatch:
    //   %id = get_global_id(0)
    //   if %id < %N { body(%id) }

    let global_id = self.emit_global_id_call(out, 0);
    // ... emit guard: if %id < %N ...
    // ... emit body with %id substituted for loop index ...
}

fn emit_global_id_call(&self, out: &mut String, dim: i32) -> String {
    let var = self.fresh_var("tid");
    writeln!(out, "  %{} = call spir_func i64 @_Z33__spirv_BuiltInGlobalInvocationIdi(i32 {})",
             var, dim).ok();
    var.to_string()
}
```

---

## 5. Metadata Emission

### Required Metadata Blocks

For a Vulkan compute kernel, the SPIR-V module needs:

```llvm
; 1. Memory model (module-level)
!spirv.MemoryModel = !{!0}
!0 = !{i32 0, i32 3}  ; Logical addressing, VulkanKHR memory model

; 2. Entry point
!spirv.EntryPoint = !{!1}
!1 = !{i32 5, i32 3, !"kernel_name", !2}  ; id=5, GLCompute=3, name, interface list
!2 = !{}  ; interface = empty (no global variables in minimal case)

; 3. Execution mode (workgroup size)
!spirv.ExecutionMode = !{!3}
!3 = !{i32 5, i32 17, i32 256, i32 1, i32 1}  ; id=5, LocalSize=17, (256,1,1)

; 4. Parameter decorations (optional, for optimizer hints)
define spir_kernel void @kernel(...) !spirv.ParameterDecorations !4
!4 = !{!5, !6}  ; one per parameter
!5 = !{i32 19}  ; Restrict decoration on param 0
!6 = !{i32 19}  ; Restrict decoration on param 1
```

### Implementation

```rust
fn emit_spirv_metadata(&self, out: &mut String, txn: &Transaction,
                        kernel_id: i32) {
    // Entry point
    writeln!(out, "!spirv.EntryPoint = !{{!{}}}", kernel_id).ok();
    writeln!(out, "!{} = !{{i32 {}, i32 3, !\"{}\", !{{}}}}",
             kernel_id, kernel_id + 10, txn.name).ok();

    // Execution mode
    writeln!(out, "!spirv.ExecutionMode = !{{!{}}}", kernel_id + 100).ok();
    writeln!(out, "!{} = !{{i32 {}, i32 17, i32 256, i32 1, i32 1}}",
             kernel_id + 100, kernel_id + 10).ok();
}
```

---

## 6. Constraints Gating for SPIR-V

In `src/backend/llvm.rs`, when the target is SPIR-V, gate (reject) these features:

```rust
fn validate_spirv_compatibility(&self, program: &Program) -> Vec<String> {
    let mut errors = Vec::new();

    for item in &program.items {
        match item {
            TopLevel::Transaction(t) => {
                // No recursion in SPIR-V (no indirect function calls)
                if self.has_recursion(&t.body) {
                    errors.push(format!(
                        "Transaction '{}' contains recursion, which is not supported \
                         in SPIR-V kernels. Use iterative loops instead.",
                        t.name
                    ));
                }

                // No dynamic memory allocation
                if self.has_heap_allocation(&t.body) {
                    errors.push(format!(
                        "Transaction '{}' uses heap allocation (new/malloc), \
                         which is not supported in SPIR-V. All buffers must be \
                         pre-allocated and passed as parameters.",
                        t.name
                    ));
                }

                // No FFI calls (kernel cannot load dynamic libraries)
                if self.has_ffi_calls(&t.body) {
                    errors.push(format!(
                        "Transaction '{}' calls frgn functions, which are not \
                         supported in SPIR-V kernels. FFI is host-only.",
                        t.name
                    ));
                }
            }
            _ => {}
        }
    }

    errors
}
```

---

## 7. CPU LUT Matmul FFI Pattern

The `.bv` source for the CPU LUT matmul uses Brief's `frgn` declarations to create C-compatible entry points:

```brief
# lut_matmul.bv
# FFI exports for VITRIOL's C++ bridge

# ── Exported functions (C ABI, mangled as C) ──

# Initialize: load .vpo file
# Returns 0 on success, -1 on failure
frgn lut_matmul_init(path: &Str) -> Int from "liblut_matmul";

# Evaluate one layer via LUT lookup
#   layer_id: which layer index in the VPO
#   input_acts: pointer to uint8_t activation values
#   output: pointer to float output buffer
#   input_len: number of input elements
# Returns 0 on success
frgn lut_matmul_eval(
    layer_id: Int,
    input_acts: Ptr,
    output: Ptr,
    input_len: Int
) -> Int from "liblut_matmul";

# Query output length for a layer
frgn lut_matmul_output_len(layer_id: Int) -> Int from "liblut_matmul";

# Get memory stats
frgn lut_matmul_stats(lut_bytes: Ptr, n_layers: Ptr) -> Void from "liblut_matmul";
```

### How `frgn` Compiles to LLVM IR (Existing Pattern)

The existing `src/backend/llvm.rs` already handles `frgn` declarations (lines 1811-1817). When it encounters a `TopLevel::ForeignBinding`, it:

1. Stores the signature in `self.frgn_map`
2. When the function is called in expression position, emits a `call` instruction with the C ABI

```llvm
; Compiling: lut_matmul_init(path_ptr)
;
; For x86_64 target:
declare i32 @lut_matmul_init(i8*) #0
; ...
%result = call i32 @lut_matmul_init(i8* %path_ptr)

; For SPIR-V target: frgn calls are REJECTED (no dynamic linking in kernels)
```

The same pattern works for the LUT matmul library. The `.bv` file is compiled to `liblut_matmul.so` via:

```bash
brief build lib/std/lut_matmul.bv --target x86_64 -o liblut_matmul.so
```

The resulting `.so` exports C-compatible symbols that VITRIOL's `vitriol-brief-bridge.cpp` can `dlopen`.

---

## 8. The `.bv` LUT Matmul Source Design

Once vectorization syntax is implemented, the matmul source will look like:

```brief
# lut_matmul.bv — LUT-based quantized matmul

# ── State ──────────────────────────────────────────

# VPO data (loaded once at init, never modified)
let lut_data: Vector<Float, @1:MAX_LUT_SIZE>;
let layer_index: List<LayerEntry>;

# Input configuration
let layer_id: Int;
let input_acts: Vector<UInt8, @1:MAX_INPUT_LEN>;
let output: Vector<Float, @1:MAX_OUTPUT_LEN>;

# ── Constants ──────────────────────────────────────

const MAX_ACT_4BIT: Int = 15;

# ── Transaction ────────────────────────────────────

txn compute_layer
    [layer_id >= 0 && layer_id < layer_count]
    [output_len == expected_output_len(layer_id)]
{
    let entry = layer_index[layer_id];
    let n_out = entry.shape[0];
    let n_in  = entry.shape[1];
    let n_act = 1 << entry.act_bits;

    for @neuron: 0..n_out {
        let sum: Float = 0.0;

        for @i: 0..n_in {
            let act = input_acts[i];

            # This is the hyperfold target:
            #   act ∈ [0, n_act)  (proved by range.rs from constraints)
            #   lut_data is constant after init
            #   → the multiply is folded to a LUT load:
            #     lut_data[entry.offset + neuron*n_act*n_in + act*n_in + i]
            sum = sum + lut_data[ /* computed index */ ];
        }

        output[neuron] = sum;
    }
};
```

The key insight: `range.rs` proves that `act` is bounded (0..16 for 4-bit activations), `region.rs` classifies `lut_data` as `Pure`, and the chain composition pass fuses the inner multiplication into a direct memory load. The LLVM backend emits:

```llvm
; Before folding (conceptual):
;   %prod = fmul float %lut_val, %act_val
;   %sum = fadd float %sum, %prod
;
; After folding:
;   %lut_index = ... compute offset ...
;   %prod = load float, ptr %lut_base, i64 %lut_index
;   %sum = fadd float %sum, %prod
;   (no fmul instruction emitted)
```

---

## 9. File Manifest for Brief Changes

| File | Change | Lines affected (approx) |
|------|--------|------------------------|
| `src/analysis/address_space.rs` | Add `CrossWorkgroup`, `Workgroup`, `Uniform`, `Private` enum variants | +10 |
| `src/analysis/address_space.rs` | Add `classify_for_target()` method that maps Brief→LLVM addrspace | +25 |
| `src/backend/llvm.rs` | Add `TargetTriple` enum and `--target` CLI propagation | +30 |
| `src/backend/llvm.rs` | Modify `emit_header` to dispatch on target triple | +40 |
| `src/backend/llvm.rs` | Add `emit_spirv_header()` for SPIR-V specific header | +30 |
| `src/backend/llvm.rs` | Add `emit_kernel()` for SPIR-V kernel entry points | +80 |
| `src/backend/llvm.rs` | Add `emit_global_id_call()` for builtin access | +20 |
| `src/backend/llvm.rs` | Modify `emit_parallel_body` for SIMT dispatch | +40 |
| `src/backend/llvm.rs` | Add `validate_spirv_compatibility()` | +40 |
| `src/backend/llvm.rs` | Add `emit_spirv_metadata()` | +30 |
| `src/backend/spirv.rs` | NEW — skeleton with self-contained backend notes | +200 |
| `src/main.rs` | Add `--target` CLI flag | +15 |
| **Total** | | **~560 lines** |

---

## 10. Testing Strategy

### SPIR-V Backend Tests

1. **Unit: IR emission** — compile a minimal `.bv` to SPIR-V target, check emitted LLVM IR contains `spir_kernel`, `addrspace(1)`, `@_Z33__spirv_BuiltInGlobalInvocationIdi`
2. **Integration: produce `.spv`** — full pipeline with `llc -mtriple=spirv64-unknown-vulkan1.2`, verify `spirv-val` passes
3. **Runtime: Vulkan dispatch** — C test program that loads `.spv`, dispatches via Vulkan compute, reads back result

### CPU LUT Matmul Tests

1. **Unit: range analysis** — compile `.bv` with bounded activation precondition, verify `range.rs` extracts correct bounds
2. **Unit: region classification** — verify matmul is classified as `Pure` + bounded → eligible for fold
3. **Integration: FFI call** — compile to `.so`, call from C test program, verify correct LUT lookup results
4. **Performance: compare to CPU loop** — benchmark LUT matmul vs naive CPU matmul, verify LUT path is faster
