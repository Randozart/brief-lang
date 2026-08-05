# SPIR-V Backend for GPU Compute

**Date added:** 2026-06-18  
**Phase:** 1–6 (full GPU compute pipeline)  
**Status:** Active — canonical GPU target

---

## Module Layout

The GPU pipeline lives in `src/backend/llvm/gpu.rs` with cost analysis in
`src/analysis/gpu_cost.rs`. The runtime C library is `lib/runtime/briv_gpu_rt.c`.

### Public Functions in `gpu.rs`

| Function | Purpose |
|----------|---------|
| `check_eligibility(body)` | Walk statements and expressions to determine if a txn body is GPU-eligible |
| `extract_kernel(name, body, count, state_fields, field_types)` | Clone body and classify read/write fields |
| `emit_spirv_module(kernel)` | Produce LLVM IR string targeting `spirv64-unknown-unknown` |
| `embed_spirv_blob(binary, name)` | Format SPIR-V binary as LLVM IR constant array |
| `compile_to_spirv(ir)` | Run `llc --mtriple=spirv64-unknown-unknown` to produce `.spv` |

### Integration Points

- `mod.rs:collect_gpu_kernel` — called from `emit_transaction` and `emit_callable_txn`
- `emit_toplevel.rs` — `#gpu` directive detection per transaction
- `directive.rs` — `DirectiveEffect::GpuOffload` resolution

---

## Flow

```
Briv source → codegen (CPU IR) → collect_gpu_kernel():
  1. check_eligibility() — purity: no unsafe FFI/term/unification/escape
  2. gpu_cost::estimate() — ops vs bytes, crossover point, remarks
  3. extract_kernel() — clone body, classify read/write fields
  4. emit_spirv_module() — produce LLVM IR for spirv64-unknown-unknown
  5. compile_to_spirv() — llc --mtriple=spirv64-unknown-unknown
  6. SPIR-V blob embedded as @briv_kernel_N byte array in .rodata
  7. At runtime: briv_gpu_launch() via Vulkan or OpenCL
```

---

## Float Opcode Dispatch

The AST does not have separate `FAdd`/`FSub`/`FMul`/`FDiv` variants —
`Add`/`Sub`/`Mul`/`Div` are reused for both integer and float operations.
The SPIR-V backend determines the correct opcode via `is_float_context()`.

`is_float_context(expr, field_types) -> bool` walks the expression tree:

| Leaf type | Float? |
|-----------|--------|
| `Expr::Float(_)` | Always |
| `Expr::Identifier(name)` where `field_types[name] == "float"` | Yes |
| Math intrinsic (sin, cos, pow, sqrt, fabs) | Yes |
| Binary op with float operand | Yes (recurse) |
| Everything else | No (integer) |

### Expression → SPIR-V LLVM IR mapping

| Briv expression | Integer (i64) | Float (f32) |
|-----------------|---------------|-------------|
| `a + b` | `add i64 %a, %b` | `fadd float %a, %b` |
| `a - b` | `sub i64 %a, %b` | `fsub float %a, %b` |
| `a * b` | `mul i64 %a, %b` | `fmul float %a, %b` |
| `a / b` | `sdiv i64 %a, %b` | `fdiv float %a, %b` |
| `a % b` | `srem i64 %a, %b` | — |
| `-a` | `sub i64 0, %a` | — |
| `a < b` | `icmp slt i64 %a, %b` → zext | `fcmp olt float %a, %b` → zext |
| `a <= b` | `icmp sle i64 %a, %b` → zext | `fcmp ole float %a, %b` → zext |
| `a > b` | `icmp sgt i64 %a, %b` → zext | `fcmp ogt float %a, %b` → zext |
| `a >= b` | `icmp sge i64 %a, %b` → zext | `fcmp oge float %a, %b` → zext |
| `a == b` | `icmp eq i64 %a, %b` → zext | `fcmp oeq float %a, %b` → zext |
| `a != b` | `icmp ne i64 %a, %b` → zext | `fcmp one float %a, %b` → zext |

---

## Buffer Classification

Read/write analysis in `extract_kernel` classifies fields:

- **Read-only fields** → input buffer (`i8* %in_buf`, `nocapture readonly`)
- **Written fields** (read-write or write-only) → output buffer (`i8* %out_buf`, `nocapture`)

The kernel signature is:
```llvm
define spir_kernel void @kernel(
    i8* nocapture readonly %in_buf,
    i8* nocapture %out_buf,
    i64 %N
)
```

`ensure_field_loaded` selects `%base_in` or `%base_out` based on whether the
field is in `write_fields`. Stores always target `%base_out`.

---

## Intrinsic → SPIR-V Mapping

### Thread/Block ID Queries

| Briv intrinsic | SPIR-V LLVM IR |
|----------------|----------------|
| `get_global_id#(dim)` | `call i64 @_Z13get_global_idj(i32 %dim)` |
| `get_local_id#(dim)` | `call i64 @_Z12get_local_idj(i32 %dim)` |
| `get_group_id#(dim)` | `call i64 @_Z12get_group_idj(i32 %dim)` |
| `get_num_groups#(dim)` | `call i64 @_Z16get_num_groupsj(i32 %dim)` |
| `barrier#()` | `call void @_Z8barrierj(i32 0)` |

### Math Intrinsics

| Briv intrinsic | SPIR-V LLVM IR |
|----------------|----------------|
| `sin#(f)` | `call float @llvm.sin.f32(float %f)` |
| `cos#(f)` | `call float @llvm.cos.f32(float %f)` |
| `pow#(f, e)` | `call float @llvm.pow.f32(float %f, float %e)` |
| `sqrt#(f)` | `call float @llvm.sqrt.f32(float %f)` |
| `fabs#(f)` | `call float @llvm.fabs.f32(float %f)` |

---

## Shared Memory

`Expr::SharedMem(N)` is lowered to an `addrspace(3)` global:

```llvm
@shared_buf_K = internal unnamed_addr addrspace(3) global [N x i64] zeroinitializer
```

The expression evaluates to an `i64` pointer to the shared memory buffer:

```llvm
%sh_base = addrspacecast [N x i64] addrspace(3)* @shared_buf_K to i8*
%sh_ptr = ptrtoint i8* %sh_base to i64
```

Shared memory sizes are pre-scanned by `collect_shared_mem_sizes` before
emission so the `addrspace(3)` globals are declared at module scope.

---

## Eligibility

`check_eligibility` performs recursive expression walking via
`collect_unsafe_ffi`. The following are **allowed** in GPU kernels:

- Math intrinsics: `sin`, `cos`, `pow`, `sqrt`, `fabs`
- Thread/block ID intrinsics: `get_global_id`, `get_local_id`, `get_group_id`, `get_num_groups`
- Synchronization: `barrier`
- Shared memory: `SharedMem`

The following are **blocked**:
- User FFI calls (`Expr::Call`)
- Unsafe intrinsics (`PrintInt`, `ReadFile`, etc.)
- `term`/`term!`, `escape`, `unification`

---

## Multi-Dimensional Grid

The kernel's `%N` parameter represents the total linearized element count.
`get_global_id(0)` is used for the primary index in the GEP base computation
(`%base_in = GEP %in_buf, %gtid`). Multi-dimensional indexing is achieved
by calling `get_global_id(1)` and `get_global_id(2)` directly in the kernel
body — the user computes their own linearized index.

The runtime's `briv_gpu_launch` accepts `grid_y` and `grid_z` parameters,
passed through to `vkCmdDispatch` as the 2nd and 3rd dimensions.

---

## Storage Model

| Aspect | Detail |
|--------|--------|
| Float precision | 32-bit (`float` in LLVM SPIR-V) |
| Float literal | `bitcast i32 <hex> to float` |
| Memory model | i64-per-element, sequential fields, flat buffers |
| Buffers | Input (read-only) + Output (read-write), separate `i8*` params |
| Alignment | i64 → align 8, float → align 4 |

---

## Test Coverage

| Area | Tests | Location |
|------|-------|----------|
| Eligibility checks | 10 | `gpu.rs` |
| Kernel extraction | 2 | `gpu.rs` |
| SPIR-V IR structure | 3 | `gpu.rs` |
| Thread/block ID intrinsics | 5 | `gpu.rs` |
| Float arithmetic | 5 | `gpu.rs` |
| Integer arithmetic | 2 | `gpu.rs` |
| Multi-buffer | 2 | `gpu.rs` |
| Shared memory | 3 | `gpu.rs` |
| Multi-dim grid | 2 | `gpu.rs` |
| Blob embedding | 1 | `gpu.rs` |
| **Total** | **36** | — |
