# Brief Compiler — VITRIOL Integration Plan

**Date:** 2026-06-04 08:14 UTC
**Status:** Design complete — awaiting vectorization syntax (`@`, `<-`, `...`) implementation
**Parent:** VITRIOL × Brief Master Plan (`../VITRIOL/.opencode/plans/vitriol-brief-integration-master-plan-2026-06-04.md`)

---

## 1. Overview

This document covers all changes to the **Brief Compiler** required for the VITRIOL integration. The compiler gains three new capabilities:

1. **SPIR-V backend** (via LLVM `spirv64-unknown-unknown` target)
2. **CPU LUT matmul FFI library** compilation (x86_64, existing backend)
3. **NVPTX backend** (later phase, via LLVM `nvptx64-nvidia-cuda` target)

All three backends compile from the **same `.bv` source** — a single LUT-based quantized matmul written using the new `@` dimension targeting and `<-` arrow mutation syntax.

---

## 2. Brief Compiler Changes

### Phase 0 — SPIR-V Backend

| Step | Detail | Files affected |
|------|--------|----------------|
| **0.1** | Parameterize `target_triple` — add `--target` flag to CLI, propagate to `LlvmBackend` | `src/main.rs`, `src/backend/llvm.rs:1789` |
| **0.2** | Add GPU address space variants to `AddressSpace` enum: `CrossWorkgroup`, `Function`, `Uniform`, `Private` | `src/analysis/address_space.rs` |
| **0.3** | Map Brief address spaces → SPIR-V addrspaces: `Ddr4`→CrossWorkgroup(1), `FpgaInternal`→Function(4), `Mmio`→Uniform(2), stack→Private(5) | `src/analysis/address_space.rs` |
| **0.4** | In `emit_header`: switch target triple + datalayout for `spirv64-unknown-unknown` | `src/backend/llvm.rs:emit_header` |
| **0.5** | Emit kernel metadata: `[[spirv::kernel]]` attribute, work-group size, `reqd_work_group_size` | `src/backend/llvm.rs:emit_kernel` (new function) |
| **0.6** | Map `DispatchMode::Parallel` → `@llvm.spirv.get_global_id(i32)` for thread-ID-based array indexing | `src/backend/llvm.rs:emit_expr` |
| **0.7** | Gate unsupported features for SPIR-V target: reject recursion, module-scope `alloca`, `indirectbr` at typecheck or codegen | `src/backend/llvm.rs`, `src/proof_engine.rs` |
| **0.8** | Write `src/backend/spirv.rs` — skeleton with comprehensive inline comments documenting every LLVM IR→SPIR-V translation decision discovered during 0.1-0.7 | New file |

### Phase 0B — Self-Contained SPIR-V Backend Knowledge

The `src/backend/spirv.rs` skeleton serves as a living design document. Every time a translation choice is made in `llvm.rs` for SPIR-V, the equivalent self-contained approach is documented in `spirv.rs`:

```rust
// src/backend/spirv.rs (skeleton)
//
// This file documents how to build a self-contained SPIR-V emitter
// that does not depend on LLVM's spirv64 target. Once complete, it
// can replace the LLVM SPIR-V path entirely.
//
// Key findings from LLVM SPIR-V backend (added during implementation):
//
// ## Address Space Mapping (from address_space.rs)
// - CrossWorkgroup(1) → SPIR-V StorageClassCrossWorkgroup
// - Function(4)       → SPIR-V StorageClassFunction
// - Uniform(2)        → SPIR-V StorageClassUniform
//
// ## Kernel Emission (from llvm.rs emit_kernel)
// - SPIR-V requires OpEntryPoint + OpExecutionMode
// - Work-group size metadata → OpExecutionMode LocalSize
// ...
```

### Phase 1 — CPU LUT Matmul (No New Backend)

The CPU LUT matmul uses the existing x86_64 LLVM backend. The work is in the `.bv` source and the FFI bridge:

| Step | Detail | Files affected |
|------|--------|----------------|
| **1.1** | Write `lib/std/lut_matmul.bv` — LUT-based quantized matmul using `@` dimension targeting + `<-` arrow syntax | New file in `lib/std/` |
| **1.2** | Export `frgn`-compatible C ABI entry points: `lut_matmul_init(vpo_path: Str)`, `lut_matmul_eval(layer_id: Int, input_ptr: Ptr, output_ptr: Ptr)` | New file |
| **1.3** | Brief's `range.rs` proves activation bounds from type constraints or preconditions | Already works |
| **1.4** | Brief's `region.rs` classifies matmul as `Pure` + bounded → chain composition folds the inner loop | Already works |
| **1.5** | Brief's `proof_engine.rs` verifies LUT lookup safety (bounds checking on activation values) | Already works |
| **1.6** | Compile `lut_matmul.bv` → `liblut_matmul.so` via `brief build --target x86_64` | Existing |

### Phase 5 — NVPTX Backend

| Step | Detail | Files affected |
|------|--------|----------------|
| **5.1** | Add `nvptx64-nvidia-cuda` target triple to `emit_header` | `src/backend/llvm.rs` |
| **5.2** | Map address spaces for PTX: `CrossWorkgroup`→`addrspace(1)` (global), `Function`→`addrspace(3)` (shared) | `src/analysis/address_space.rs` |
| **5.3** | Emit PTX kernel attributes: `.visible .entry`, `.reg`, `.param`, `.shared` | `src/backend/llvm.rs:emit_kernel` |
| **5.4** | Add runtime GPU arch detection metadata (Pascal→dp4a, Turing+→mma.sync) | `src/backend/llvm.rs` |

---

## 3. The `.bv` Source (Conceptual)

The LUT matmul source, written in post-vectorization-syntax Brief:

```brief
# lut_matmul.bv
#
# LUT-based quantized matrix multiplication.
# Reads pre-baked LUT data from a .vpo file at init time.
# At inference time, each multiply is a single LUT load.

# ── State ──────────────────────────────────────────────────

# LUT data: loaded from .vpo at init
let lut_data: Vector<Float, @1:MAX_LUT_SIZE>;
# -> ranges are proven from constraints at compile time.

# Layer index: (layer_id → { offset, shape, quant_type, act_bits })
let layer_index: List<LayerEntry>;

# ── Constants ──────────────────────────────────────────────

const MAX_ACT_VALUE: Int = 15;   # for 4-bit activations

# ── Transaction ─────────────────────────────────────────────

txn compute_layer [layer_id >= 0 && layer_id < layer_count]
                  [output_len == input_len && ...]
{
    # Lookup layer metadata from index
    let entry = layer_index[layer_id];
    let act_bits = entry.act_bits;
    let max_act = 1 << act_bits;

    # Bounds: input activations are in [0, max_act)
    # (proved by range.rs from the quantization type constraint)

    for @neuron: 0..entry.shape[0] {
        let sum: Float = 0.0;
        for @i: 0..entry.shape[1] {
            let act = input_acts[i];
            # This inner multiply is recognized by region.rs
            # as bounded (act ∈ [0, max_act]) × Pure (lut_data).
            # Chain composition folds it into a LUT load.
            sum = sum + lut_data[entry.offset + neuron * max_act + act];
        }
        output[neuron] = sum;
    }
};
```

The `@` dimension specifier lets the compiler reason about tensor shapes statically. The `<-` arrow syntax is not used directly in this matmul (it's a pure read operation), but the vectorization infrastructure (`@`, `...`, `Vector`) is required for the type system to express multi-dimensional LUT indexing.

---

## 4. FFI Contract

The Brief-compiled `liblut_matmul.so` exposes:

```c
// Initialize the LUT engine with a .vpo file.
// Must be called once at model load time.
int lut_matmul_init(const char* vpo_path);

// Evaluate one layer using pre-baked LUTs.
// input_acts: quantized activation values (uint8_t per element)
// output: float partial sums (preallocated by caller)
// Returns 0 on success, nonzero on error.
int lut_matmul_eval(
    uint32_t layer_id,
    const uint8_t* input_acts,
    float* output,
    uint32_t input_len
);

// Get the maximum output length for a layer.
uint32_t lut_matmul_output_len(uint32_t layer_id);

// Memory stats for profiling.
void lut_matmul_stats(uint64_t* lut_bytes, uint32_t* n_layers);
```

These are declared in Brief via:

```brief
frgn lut_matmul_init(path: Str) -> Int from "liblut_matmul";
frgn lut_matmul_eval(layer_id: Int, input: Ptr, output: Ptr, len: Int) -> Int from "liblut_matmul";
frgn lut_matmul_output_len(layer_id: Int) -> Int from "liblut_matmul";
frgn lut_matmul_stats(lut_bytes: Ptr, n_layers: Ptr) -> Void from "liblut_matmul";
```

---

## 5. Build System Changes

### Cargo.toml

No new Rust dependencies. The SPIR-V and NVPTX backends use LLVM IR text output, same as the existing x86_64 backend.

### Target Detection

`brief build --target` accepts:

| Target | Triple | Backend |
|--------|--------|---------|
| `x86_64` | `x86_64-unknown-linux-gnu` | LLVM (existing) |
| `spirv64` | `spirv64-unknown-unknown` | LLVM (new) |
| `nvptx64` | `nvptx64-nvidia-cuda` | LLVM (new, later) |

### Required LLVM Build

SPIR-V target requires LLVM built with `-DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD=SPIRV`. Documented in `AGENTS.md`.

---

## 6. Related Documents

- Master plan: `../VITRIOL/.opencode/plans/vitriol-brief-integration-master-plan-2026-06-04.md`
- VITRIOL-specific plan: `../VITRIOL/.opencode/plans/vitriol-integration-vitriol-plan-2026-06-04.md`
- Collection mutation + dimension syntax design: `./2026-06-04-collection-mutation-language-design.md`

---

## 7. Implementation Order

The Brief compiler changes must happen in this order (dependencies within the compiler):

1. Vectorization syntax (`@`, `...`, `<-`) — required by all `.bv` source
2. Phase 0 steps 0.1-0.8 — SPIR-V backend
3. Phase 1 — CPU LUT matmul (depends on vectorization syntax, not on SPIR-V)
4. Phase 5 — NVPTX backend (depends on SPIR-V backend experience)
