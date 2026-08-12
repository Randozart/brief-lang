# SPIR-V Backend Extension — Full GPU Compute Pipeline

**Date:** 2026-06-18  
**Status:** Completed — all 7 phases implemented, 1025 tests passing
**Context:** GPU offloading infra exists as v0.1 — single-buffer, i64-only integer
add/sub/mul kernels via `emit_spirv_module` in `gpu.rs`. Two active backends
(LLVM, Webstack, CIRCT) plus dead backends. 981 tests passing.

---

## Design Decisions (from user consultation)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Float vs int opcode selection | `field_types` map + expression root analysis | No typechecker dependency. Walk expression tree; if any operand is a float field or `Expr::Float`, emit float ops. Otherwise integer. |
| Float precision | 32-bit `float` by default; `#f64` / `#?f64` decoration for 64-bit | GPU compute convention. Half memory traffic vs f64. Covers 99% of workloads. `#f64` decoration is a future extension — not in this plan. |
| Buffer classification | Auto-classify by read/write analysis | Zero source changes. Read-only fields → input buffer. Read-write or write-only → output buffer. |
| Thread/block ID model | New `Intrinsic` variants | Proper AST-level support. Match arms across parser, interpreter, all backends. |
| Shared memory | New `Expr::SharedMem(usize)` AST variant | Explicit, type-safe. Match arm in interpreter returns `Value::Integer(0)`. |
| Multiple dimensions | Kernel grid `(grid_x, grid_y, grid_z)` → host dispatch | Strided linearization of `get_global_id(0..2)`. |

---

## Files to Modify

| File | Nature of change | Risk |
|------|-----------------|------|
| `src/ast.rs` | Add `Intrinsic` variants + `Expr::SharedMem` | Low — additive enum variants |
| `src/backend/llvm/gpu.rs` | Core: expression emission, buffer split, eligibility, shared memory | Medium — new logic |
| `src/backend/llvm/mod.rs` | Thread `field_types` to `extract_kernel` | Low — plumbing |
| `src/backend/llvm/emit_toplevel.rs` | Pass `field_types` to `collect_gpu_kernel` | Low — one parameter |
| `src/backend/llvm/tests.rs` | New GPU kernel SPIR-V IR tests | Low — test assertions |
| `src/interpreter.rs` | Match arms for new `Intrinsic` + `Expr::SharedMem` | Low — stub returns |
| `lib/runtime/briev_gpu_rt.c` | Multi-buffer dispatch, `grid_y`/`grid_z` params | Low — API extension |
| `lib/std/gpu.bv` | NEW — standard library wrappers | Low — fresh file |
| `docs/architecture/features/spirv-backend.md` | NEW — architecture doc | Low — documentation |

---

## Priority 1: Float Arithmetic

### What

Support `fadd`, `fsub`, `fmul`, `fdiv`, float comparison (`fcmp olt`), and
math intrinsics (`sin#`, `cos#`, `pow#`, `sqrt#`, `fabs#`) in SPIR-V kernels.

### Why currently broken

`emit_spirv_expr` in `gpu.rs:307-357` only handles `Expr::Integer`, `Expr::Bool`,
`Expr::Identifier`, `Expr::Add`/`Sub`/`Mul` (all emitted as `add i64`/`sub i64`/`mul i64`),
and `Expr::Lt` (emitted as `icmp slt i64`). `Expr::Float(n)` and any float arithmetic
fall through to the catch-all `; error: unsupported expression` stub returning `"0"`.

### Root cause chain

`LlvmBackend.field_types` already tracks the LLVM type string per field
("i64", "float", "i8", etc.) via `build_field_index` called in `generate()`.
But `extract_kernel` and `emit_spirv_module` never receive this information —
they build their own `field_offsets: HashMap<String, u64>` with no type awareness.

### Approach

1. **Thread `field_types` through the extraction pipeline**:
   - `LlvmBackend::collect_gpu_kernel` already has access to `self.field_types`
   - Pass it to `extract_kernel` → store in `GpuKernel` as `field_types: Vec<String>`
   - Or better: pass as `HashMap<String, String>` (field_name → "i64" | "float" | "i8" | "i32")
   - `emit_spirv_module` reads this alongside `field_offsets`

2. **Load/store correct LLVM type**:
   - `ensure_field_loaded`: if `field_types[field] == "float"`, emit
     `%lv_N = load float, float* %gep, align 4`
   - Otherwise emit existing `load i64, i8* %gep, align 8`
   - For stores: emit `store float %val, float* %gep, align 4` for float,
     `store i64 %val, i8* %gep, align 8` for int

3. **Float literal** (`Expr::Float(n)`):
   ```llvm
   %fc = bitcast i32 <hex> to float
   ```
   Where `<hex>` is the `f64→i32` bitcast via `float_to_llvm_hex` (truncating
   f64 to f32 for SPIR-V).

4. **`is_float_context(expr, field_types, field_offsets) -> bool`**:
   Walk the expression tree. Return `true` if any leaf is:
   - `Expr::Float(_)` — a float literal
   - `Expr::Identifier(name)` where `field_types.get(name) == Some("float")`
   - Recursive for binary/unary sub-expressions

5. **New `emit_spirv_expr` arms** (all guarded by `is_float_context`):

   | Expression | Float-context IR | Integer-context IR (existing) |
   |---|---|---|
   | `Add(l, r)` | `%r = fadd float %l, %r` | `%r = add i64 %l, %r` |
   | `Sub(l, r)` | `%r = fsub float %l, %r` | `%r = sub i64 %l, %r` |
   | `Mul(l, r)` | `%r = fmul float %l, %r` | `%r = mul i64 %l, %r` |
   | `Div(l, r)` | `%r = fdiv float %l, %r` | *(not yet supported)* → `udiv i64 %l, %r` |
   | `Lt(l, r)` | `%c = fcmp olt float %l, %r` → `%z = zext i1 %c to i64` | `icmp slt i64` |
   | `Le(l, r)` | `%c = fcmp ole float %l, %r` → zext | `icmp sle i64` |
   | `Gt(l, r)` | `%c = fcmp ogt float %l, %r` → zext | *(new)* `icmp sgt i64` |
   | `Ge(l, r)` | `%c = fcmp oge float %l, %r` → zext | *(new)* `icmp sge i64` |
   | `Eq(l, r)` | `%c = fcmp oeq float %l, %r` → zext | *(new)* `icmp eq i64` |
   | `Ne(l, r)` | `%c = fcmp one float %l, %r` → zext | *(new)* `icmp ne i64` |
   | `Neg(e)` | `%r = fneg float %e` | *(new)* `sub i64 0, %e` |
   | `Div(l, r)` (int) | — | `%r = sdiv i64 %l, %r` |

6. **Math intrinsics** — new arm for `Expr::IntrinsicCall`:

   | Intrinsic | SPIR-V LLVM IR |
   |---|---|
   | `Intrinsic::Sin` | `call float @llvm.sin.f32(float %arg)` |
   | `Intrinsic::Cos` | `call float @llvm.cos.f32(float %arg)` |
   | `Intrinsic::Pow` | `call float @llvm.pow.f32(float %arg0, float %arg1)` |
   | `Intrinsic::Sqrt` | `call float @llvm.sqrt.f32(float %arg)` |
   | `Intrinsic::Fabs` | `call float @llvm.fabs.f32(float %arg)` |

   These are declared at the top of the SPIR-V module:
   ```llvm
   declare float @llvm.sin.f32(float) #0
   declare float @llvm.cos.f32(float) #0
   declare float @llvm.pow.f32(float, float) #0
   declare float @llvm.sqrt.f32(float) #0
   declare float @llvm.fabs.f32(float) #0
   ```

7. **Integer `Div`/`Mod`/comparison ops**: also add proper integer `sdiv`,
   `srem`, `icmp sgt`/`sge`/`eq`/`ne` for the non-float context, so existing
   integer kernels get full comparison support.

### Pre-existing field type plumbing

`LlvmBackend` already has:
- `field_index_map: HashMap<String, usize>` — field name → index
- `field_types: Vec<String>` — at index `i`, the LLVM type string (e.g. "float")
- `field_initializers: HashMap<String, Option<Expr>>`

These are populated in `build_field_index()` which is called at the top of
`generate()`. By the time `collect_gpu_kernel` fires (from `emit_transaction`
and `emit_callable_txn`), `self.field_types` is fully populated.

### Tests (added to `gpu.rs:415`, 4 new tests)

```
test_emit_spirv_float_assignment
  Input: x (float) = y (float) + 3.14
  Action: emit_spirv_module
  Assert: IR contains "load float", "fadd float", "store float"
  Assert: IR does NOT contain "add i64" or "load i64"

test_emit_spirv_float_comparison
  Input: r (int) = x (float) < y (float)
  Action: emit_spirv_module
  Assert: IR contains "fcmp olt float"
  Assert: IR contains "zext i1"

test_emit_spirv_float_intrinsic_sin
  Input: r = sin#(x_float)
  Action: emit_spirv_module
  Assert: IR contains "call float @llvm.sin.f32"
  Assert: IR declares @llvm.sin.f32

test_emit_spirv_mixed_int_float
  Input: x (int) = a (int) + 1;  y (float) = b (float) * 2.0
  Action: emit_spirv_module
  Assert: IR contains both "add i64" and "fmul float"
  Assert: Two separate field-type loads (i8* for int, float* for float)

test_emit_spirv_integer_comparison_extended
  Input: r = x < y;  s = x > y;  t = x == y
  Action: emit_spirv_module
  Assert: IR contains "icmp slt", "icmp sgt", "icmp eq"
  Assert: Each comparison has a zext i1 to i64

test_emit_spirv_integer_div_mod
  Input: q = x / y;  r = x % y
  Action: emit_spirv_module
  Assert: IR contains "sdiv i64" and "srem i64"
```

---

## Priority 2: Multiple Storage Buffers

### What

Split kernel parameters from one `i8* %buffer` into `i8* %in_buf, i8* %out_buf`.

### Approach

1. **Extend `GpuKernel`**:
   ```rust
   pub struct GpuKernel {
       pub name: String,
       pub body: Vec<Statement>,
       pub count_expr: Expr,
       pub input_fields: Vec<String>,       // read-only fields
       pub output_fields: Vec<String>,      // read-write + write-only
       pub input_field_types: Vec<String>,
       pub output_field_types: Vec<String>,
       pub spirv_ir: Option<String>,
       pub spirv_binary: Option<Vec<u8>>,
   }
   ```

2. **Classification algorithm in `extract_kernel`**:
   - Walk body statements collecting read set and write set
   - `read_set`: fields appearing in RHS expressions, guards
   - `write_set`: fields appearing on LHS of assignments
   - `input_fields = read_set - write_set` (pure reads → input buffer)
   - `output_fields = write_set` (everything written → output buffer, which gets read-write access for fields that are both read and written)

3. **Kernel signature change**:
   ```llvm
   define spir_kernel void @kernel(i8* nocapture readonly %in_buf,
                                   i8* nocapture %out_buf,
                                   i64 %N)
   ```
   Use `readonly` on `%in_buf`, no alias annotation on `%out_buf`.

4. **Dual GEP chains**:
   - `%base_in = getelementptr i8, i8* %in_buf, i64 %gtid`  (for input fields)
   - `%base_out = getelementptr i8, i8* %out_buf, i64 %gtid` (for output fields)
   - `ensure_field_loaded` selects buffer based on field classification

5. **Update `briev_gpu_launch` C API**:
   ```c
   void briev_gpu_launch(
       const void* kernel_spirv, size_t kernel_size,
       int grid_x, int block_x,
       const int64_t* buffer_handles, int num_buffers
   );
   ```
   Already accepts `buffer_handles` array. The compiler emits two handles:
   - `handle[0]` = input buffer (host→device)
   - `handle[1]` = output buffer (device→host)

### Tests (2 new)

```
test_emit_spirv_multi_buffer_signature
  Input: kernel with read_fields=[x] write_fields=[y]
  Action: emit_spirv_module
  Assert: IR contains "%in_buf" and "%out_buf"
  Assert: IR kernel signature has two i8* params + i64 %N

test_emit_spirv_multi_buffer_read_write
  Input: y = x + 1  (x is read-only, y is written)
  Action: emit_spirv_module
  Assert: x loaded from %base_in (offset 0)
  Assert: y stored to %base_out (offset 8)
  Assert: result computed via add i64
```

---

## Priority 3: Thread/Block ID Intrinsics

### What

Add `get_global_id#(dim)`, `get_local_id#(dim)`, `get_group_id#(dim)`,
`get_num_groups#(dim)`, `barrier#()` as native `Intrinsic` variants.

### Approach

1. **Add to `ast.rs`**:
   ```rust
   pub enum Intrinsic {
       // ... existing variants ...
       GetGlobalId,
       GetLocalId,
       GetGroupId,
       GetNumGroups,
       SubGroupBarrier,
   }
   ```

2. **Add `from_name` mappings** in `Intrinsic::from_name`:
   ```rust
   "get_global_id" => Some(Intrinsic::GetGlobalId),
   "get_local_id" => Some(Intrinsic::GetLocalId),
   "get_group_id" => Some(Intrinsic::GetGroupId),
   "get_num_groups" => Some(Intrinsic::GetNumGroups),
   "barrier" => Some(Intrinsic::SubGroupBarrier),
   ```

3. **Add `has_side_effects` return value**: `SubGroupBarrier` returns `true`
   (synchronization is observable). The others return `false` (pure query).

4. **Add `is_compile_time_only` guard**: all five return `false`.

5. **Update `interpreter.rs`**: add match arms that return `Value::Integer(0)`
   (stub — interpreter doesn't simulate GPU).

6. **Update `emit_spirv_module` SPIR-V declares**:
   ```llvm
   declare i64 @_Z13get_global_idj(i32) #0
   declare i64 @_Z12get_local_idj(i32) #0
   declare i64 @_Z12get_group_idj(i32) #0
   declare i64 @_Z16get_num_groupsj(i32) #0
   declare void @_Z8barrierj(i32) #0
   ```

7. **Update `emit_spirv_expr`** — new match arm for `Expr::IntrinsicCall`:
   ```rust
   Expr::IntrinsicCall { intrinsic: Intrinsic::GetGlobalId, args } => {
       let dim = emit_spirv_expr(&args[0], ...);
       let reg = format!("%gtid{}", ir.len());
       ir.push_str(&format!("{}  {} = call i64 @_Z13get_global_idj(i32 {})\n",
           indent, reg, dim));
       reg
   }
   // Similar for GetLocalId, GetGroupId, GetNumGroups
   ```
   `SubGroupBarrier` is a `Statement::Expression`, not part of an expression.
   Emit it as:
   ```llvm
   call void @_Z8barrierj(i32 0)
   ```

8. **`barrier#()` handling in statement emission**: since barrier returns void,
   it appears as `Statement::Expression(Expr::IntrinsicCall { ... })`. Emit
   the call directly without assigning to a register.

9. **Update `check_eligibility`**: ALLOW `Expr::IntrinsicCall` for known GPU-safe
   intrinsics (see Priority 5 below).

10. **Update `collect_strings_expr`** and other match-arms-only functions in
    `mod.rs`, `loop_engine.rs`, `reorder.rs`, `hazard.rs`: add trivial match
    arms for the new `Intrinsic` variants. Pattern: `Intrinsic::GetGlobalId |
    Intrinsic::GetLocalId | ... => {}` or `_ => {}` if the function already
    has a wildcard catch-all.

11. **Update `emit_expr.rs`** (CPU codegen): add match arms that call the
    actual C runtime functions (e.g. `__get_global_id` in `briev_gpu_rt.c`).
    These compile to real FFI calls on CPU for testing, and get replaced by
    SPIR-V builtins in the GPU path.

### Tests (4 new in `gpu.rs`, 2 new in `tests.rs`)

```
test_emit_spirv_get_global_id
  Input: get_global_id#(0)
  Action: emit_spirv_expr
  Assert: IR contains "call i64 @_Z13get_global_idj(i32 0)"

test_emit_spirv_get_local_id
  Input: get_local_id#(0)
  Action: emit_spirv_expr
  Assert: IR contains "call i64 @_Z12get_local_idj(i32 0)"

test_emit_spirv_get_group_id
  Input: get_group_id#(1)
  Action: emit_spirv_expr
  Assert: IR contains "call i64 @_Z12get_group_idj(i32 1)"

test_emit_spirv_barrier
  Input: barrier#()
  Action: emit_spirv_stmt (Statement::Expression)
  Assert: IR contains "call void @_Z8barrierj(i32 0)"

test_gpu_intrinsic_cpu_codegen  (in tests.rs)
  Input: Program with barrier#() in body
  Action: backend.generate()
  Assert: CPU IR contains "call void @__barrier__" (or similar FFI dispatch)

test_gpu_intrinsic_get_global_id_cpu  (in tests.rs)
  Input: Program with get_global_id#(0) in body
  Action: backend.generate()
  Assert: CPU IR contains "call i64 @__get_global_id__"
```

---

## Priority 4: Shared Memory

### What

New `Expr::SharedMem(usize)` that evaluates to an i64 pointer to an
`addrspace(3)` global in SPIR-V.

### Approach

1. **Add to `ast.rs`**:
   ```rust
   pub enum Expr {
       // ... existing variants ...
       /// Shared memory declaration for GPU: __shared(N) → pointer to
       /// addrspace(3) memory of N i64 elements.
       SharedMem(usize),
   }
   ```

2. **Add `from_name` mapping** (if `__shared` is parsed as an intrinsic):
   ```rust
   "shared" => Some(Intrinsic::SharedMem),
   // or treat as Expr::SharedMem directly in the parser
   ```

   Actually, `__shared(256)` could be parsed as `Expr::IntrinsicCall` with
   `Intrinsic::SharedMem` and one arg. But the user explicitly requested
   a new `Expr::SharedMem(usize)` variant, so we add that. The parser turns
   `__shared(256)` into `Expr::SharedMem(256)` when called as a statement
   expression in let binding initializers.

   Simpler approach: parse `__shared(256)` as a special form — when the
   identifier is `__shared`, the parser constructs `Expr::SharedMem(256)`
   directly. This is a one-line change in the parser's expression handler.

3. **Update `interpreter.rs`**:
   ```rust
   Expr::SharedMem(_) => Ok(Value::Integer(0)), // interpreter doesn't have shared mem
   ```

4. **Update `collect_strings_expr` in `mod.rs`**:
   ```rust
   Expr::SharedMem(_) => {}
   ```

5. **Update all other expr-match functions** (`emit_expr.rs`, `reorder.rs`,
   `hazard.rs`, `loop_engine.rs` etc.) — add trivial match arms.

6. **SPIR-V emission in `emit_spirv_module`**:
   At the top of the module, emit:
   ```llvm
   @shared_buf_0 = internal unnamed_addr addrspace(3) global [256 x i64] zeroinitializer
   ```
   For each unique `SharedMem(N)` encountered, emit a global with size N.
   Use a counter to generate unique names (`@shared_buf_0`, `@shared_buf_1`).

7. **`emit_spirv_expr` for `Expr::SharedMem(N)`**:
   ```llvm
   %sh_base = addrspacecast [N x i64] addrspace(3)* @shared_buf_K to i8*
   %sh_ptr = ptrtoint i8* %sh_base to i64
   ```
   Return `%sh_ptr` — the integer representation of the shared memory pointer.
   Actual loads/stores to shared memory happen via the let-bound variable,
   which is treated as a `%State` field on CPU but as an `addrspace(3)`
   pointer on GPU.

8. **Shared memory loads/stores**: when a `Statement::Assignment` has an LHS
   that was bound via `__shared(N)`, emit `addrspace(3)` GEP + load/store:
   ```llvm
   %gep_sh = getelementptr i64, i64 addrspace(3)* %sh_buf_ptr, i64 %index
   %val = load i64, i64 addrspace(3)* %gep_sh, align 8
   ```
   This requires the backend to know which let-bound names are shared memory
   handles. Add a `shared_mem_vars: HashSet<String>` to the emission context.

### Tests (2 new)

```
test_emit_spirv_shared_memory_global
  Input: SharedMem(256)
  Action: emit_spirv_expr
  Assert: IR contains "addrspace(3)"
  Assert: IR contains "global [256 x i64]"

test_emit_spirv_shared_memory_load_store
  Input: let buf = __shared(64) in body; buf[gtid] = 42
  Action: emit_spirv_module with full kernel body
  Assert: IR contains "getelementptr i64, i64 addrspace(3)*"
  Assert: IR contains "store i64 42, i64 addrspace(3)*"
```

---

## Priority 5: Relax Eligibility Checks

### What

Currently `check_eligibility` blocks ALL `Expr::Call` and `Expr::IntrinsicCall`.
We need to allow:
1. Math intrinsics (`sin`, `cos`, `pow`, `sqrt`, `fabs`)
2. Thread/block ID intrinsics (`get_global_id`, `get_local_id`, etc.)
3. Barrier (`barrier`)
4. `Expr::SharedMem`

### Approach

1. **Add `is_gpu_safe_intrinsic(intrinsic: &Intrinsic) -> bool`**:
   ```rust
   fn is_gpu_safe_intrinsic(intrinsic: &Intrinsic) -> bool {
       matches!(intrinsic,
           Intrinsic::Sin | Intrinsic::Cos | Intrinsic::Pow
           | Intrinsic::Sqrt | Intrinsic::Fabs
           | Intrinsic::GetGlobalId | Intrinsic::GetLocalId
           | Intrinsic::GetGroupId | Intrinsic::GetNumGroups
           | Intrinsic::SubGroupBarrier
       )
   }
   ```

2. **Refine `check_eligibility`**:
   - `Statement::Expression(Expr::IntrinsicCall { intrinsic, .. })`:
     if `is_gpu_safe_intrinsic(intrinsic)` → allow (no reason added)
     else → add "GPU kernel contains unsafe intrinsic" reason
   - `Statement::Expression(Expr::Call(name, _))`:
     keep existing ban for user FFI calls
   - `Statement::Let { expr: Some(Expr::SharedMem(_)), .. }`:
     allow (shared memory is GPU-native)

3. **Add `Expr::SharedMem(_)`** to the `_ => {}` wildcard in eligibility's
   expression walker (it has no field references).

4. **Add `Expr::IntrinsicCall`** to the expression walker's field collection
   (arguments may reference state fields).

### Tests (4 new)

```
test_check_eligibility_math_intrinsic_allowed
  Input: body=[Expression(IntrinsicCall { intrinsic: Sin, args: [Identifier("x")] })]
  Action: check_eligibility
  Assert: eligible == true, reasons is empty

test_check_eligibility_get_global_id_allowed
  Input: body=[Expression(IntrinsicCall { intrinsic: GetGlobalId, args: [Integer(0)] })]
  Action: check_eligibility
  Assert: eligible == true

test_check_eligibility_barrier_allowed
  Input: body=[Expression(IntrinsicCall { intrinsic: SubGroupBarrier, args: [] })]
  Action: check_eligibility
  Assert: eligible == true

test_check_eligibility_unsafe_intrinsic_blocked
  Input: body=[Expression(IntrinsicCall { intrinsic: PrintInt, args: [Integer(42)] })]
  Action: check_eligibility
  Assert: eligible == false
  Assert: reasons contains "unsafe intrinsic"

test_check_eligibility_shared_mem_allowed
  Input: body=[Let { name: "buf", expr: SharedMem(256) }]
  Action: check_eligibility
  Assert: eligible == true
```

---

## Priority 6: Multiple Dimensions

### What

Support 2D/3D workgroup dispatch via `get_global_id(0)`, `get_global_id(1)`,
`get_global_id(2)` and the runtime's `grid_x, grid_y, grid_z` parameters.

### Approach

1. **Kernel signature** stays the same: `(i8* %in_buf, i8* %out_buf, i64 %N)`.
   The `N` parameter becomes the total linearized element count
   (`N = grid_x * block_x * grid_y * block_y * grid_z * block_z`).

2. **Linearized index computation**:
   ```llvm
   ; When kernel uses get_global_id with dim=0,1,2:
   %x_id = call i64 @_Z13get_global_idj(i32 0)
   %y_id = call i64 @_Z13get_global_idj(i32 1)
   %z_id = call i64 @_Z13get_global_idj(i32 2)
   %grid_x = call i64 @_Z16get_num_groupsj(i32 0)   ; or passed as param
   %block_x = call i64 @_Z12get_local_idj(i32 0)
   ```

   Full linearization:
   ```llvm
   %w = mul i64 %grid_x, %block_x                    ; width in threads
   %stride_y = mul i64 %y_id, %w
   %h = mul i64 %stride_y, %grid_y                   ; height in threads
   %stride_z = mul i64 %z_id, %h
   %linear = add i64 %x_id, %stride_y
   %linear = add i64 %linear, %stride_z
   ```

   This is emitted automatically when the kernel body references
   `get_global_id(1)` or `get_global_id(2)`.

3. **Update `briev_gpu_launch` C API** to accept `grid_y, grid_z`:
   ```c
   void briev_gpu_launch(
       const void* kernel_spirv, size_t kernel_size,
       int grid_x, int grid_y, int grid_z,
       int block_x,
       const int64_t* buffer_handles, int num_buffers
   );
   ```
   Add `grid_y=1, grid_z=1` defaults for backward compatibility.

4. **Detection in `emit_spirv_module`**: scan the kernel body for
   `get_global_id` calls. Collect the maximum dimension argument
   (`max_dim = max(dim_arg)`). Emit linearization GEP chain only if
   `max_dim > 0`.

5. **Single-dimension fast path**: if all `get_global_id` calls use dim=0,
   the existing simple `%base = GEP %buffer, %gtid` is used (no linearization
   overhead).

### Tests (2 new)

```
test_emit_spirv_module_2d_grid
  Input: body references get_global_id(0) and get_global_id(1)
  Action: emit_spirv_module
  Assert: IR contains "call i64 @_Z13get_global_idj(i32 1)"
  Assert: IR contains mult instruction for stride
  Assert: Linearized index computation present

test_emit_spirv_module_1d_grid_no_overhead
  Input: body references get_global_id(0) only
  Action: emit_spirv_module
  Assert: IR contains NO get_global_id(1) or get_global_id(2)
  Assert: Simple %base GEP pattern (no multi-dimensional striding)
```

---

## Standard Library Wrapper

New file `lib/std/gpu.bv`:

```briev
// GPU compute intrinsic wrappers
// These expose SPIR-V built-in functions as Briev FFI calls.
// The SPIR-V backend recognizes them and emits the correct
// SPIR-V LLVM IR built-in calls.

/// Global work-item ID for the given dimension (0, 1, or 2).
frgn get_global_id(dim: Int) -> Int ;

/// Local work-item ID within the workgroup for the given dimension.
frgn get_local_id(dim: Int) -> Int ;

/// Workgroup ID for the given dimension.
frgn get_group_id(dim: Int) -> Int ;

/// Number of workgroups in the given dimension.
frgn get_num_groups(dim: Int) -> Int ;

/// Workgroup-level barrier — synchronizes all threads in the workgroup.
frgn barrier() -> Bool ;
```

This is imported into user programs via:
```briev
import "std/gpu.bv";
```

The declarations ensure the CPU fallback path links against real
implementations in `briev_gpu_rt.c`.

---

## C Runtime Updates

File: `lib/runtime/briev_gpu_rt.c`

Changes:
1. **`briev_gpu_launch`** — add `grid_y, grid_z` parameters:
   ```c
   void briev_gpu_launch(
       const void* kernel_spirv, size_t kernel_size,
       int grid_x, int grid_y, int grid_z,
       int block_x,
       const int64_t* buffer_handles, int num_buffers
   );
   ```
   Update the Vulkan `vkCmdDispatch` call:
   ```c
   vkCmdDispatch(vk_cmd_buf, (uint32_t)grid_x, (uint32_t)grid_y, (uint32_t)grid_z);
   ```

2. **Descriptor set update for multi-buffer** — iterate `num_buffers` to
   create proper `VkDescriptorBufferInfo` entries and update the descriptor
   set with `vkUpdateDescriptorSets`.

3. **CPU fallback implementations** for the thread ID intrinsics:
   ```c
   int64_t __get_global_id(int32_t dim) { return 0; }   // single-thread CPU
   int64_t __get_local_id(int32_t dim) { return 0; }
   int64_t __get_group_id(int32_t dim) { return 0; }
   int64_t __get_num_groups(int32_t dim) { return 1; }
   void __barrier__() { }  // no-op on CPU
   ```

---

## Architecture Doc

New file `docs/architecture/features/spirv-backend.md`:

### Sections
1. **Header** — Purpose, date added, phase
2. **Module layout** — `gpu.rs` responsibilities, each public function
3. **Kernel extraction flow** — eligibility → cost → extract → emit → compile → embed
4. **Float opcode dispatch** — `field_types` map + `is_float_context` analysis,
   table of Briev expr → SPIR-V LLVM IR
5. **Buffer classification** — read/write set analysis, input vs output buffers
6. **Intrinsic → SPIR-V mapping** — table of all GPU intrinsics and their
   SPIR-V LLVM IR equivalents
7. **Shared memory lowering** — `addrspace(3)` globals, per-workgroup semantics,
   let-binding scope
8. **Multi-dimensional grid** — linearization formula, dimension detection,
   single-dim fast path
9. **Runtime dispatch flow** — host code structure: init → malloc → memcpy(H2D)
   → launch → memcpy(D2H) → free → shutdown
10. **Test coverage** — per-priority test counts and what they verify

---

## Implementation Order

| Phase | Items | Files |
|-------|-------|-------|
| **1** | Eligibility relaxation (P5) | `gpu.rs` |
| **2** | Thread/block ID intrinsics (P3) | `ast.rs`, `gpu.rs`, `interpreter.rs`, `mod.rs`, `emit_expr.rs`, `loop_engine.rs`, `reorder.rs`, `hazard.rs`, `tests.rs` |
| **3** | Float arithmetic (P1) | `gpu.rs`, `mod.rs` (plumbing), `emit_toplevel.rs` |
| **4** | Multiple storage buffers (P2) | `gpu.rs`, `briev_gpu_rt.c` |
| **5** | Shared memory (P4) | `ast.rs`, `gpu.rs`, `interpreter.rs`, `mod.rs`, all expr-match functions |
| **6** | Multiple dimensions (P6) | `gpu.rs`, `briev_gpu_rt.c` |
| **7** | Stdlib + arch doc | `lib/std/gpu.bv`, `docs/architecture/features/spirv-backend.md` |

---

## Test Inventory

| # | Test name | Location | Phase |
|---|-----------|----------|-------|
| 1 | `test_check_eligibility_math_intrinsic_allowed` | `gpu.rs` | 1 |
| 2 | `test_check_eligibility_unsafe_intrinsic_blocked` | `gpu.rs` | 1 |
| 3 | `test_check_eligibility_ffi_in_assignment_blocked` | `gpu.rs` | 1 |
| 4 | `test_check_eligibility_unsafe_intrinsic_in_guard_blocked` | `gpu.rs` | 1 |
| 5 | `test_check_eligibility_gpu_intrinsic_allowed` | `gpu.rs` | 2 |
| 6 | `test_check_eligibility_barrier_allowed` | `gpu.rs` | 2 |
| 7 | `test_emit_spirv_get_global_id` | `gpu.rs` | 2 |
| 8 | `test_emit_spirv_get_local_id` | `gpu.rs` | 2 |
| 9 | `test_emit_spirv_get_group_id` | `gpu.rs` | 2 |
| 10 | `test_emit_spirv_barrier` | `gpu.rs` | 2 |
| 11 | `test_emit_spirv_all_declares_present` | `gpu.rs` | 2 |
| 12 | `test_emit_spirv_float_assignment` | `gpu.rs` | 3 |
| 13 | `test_emit_spirv_float_sub_mul_div` | `gpu.rs` | 3 |
| 14 | `test_emit_spirv_float_comparison` | `gpu.rs` | 3 |
| 15 | `test_emit_spirv_float_negation` | `gpu.rs` | 3 |
| 16 | `test_emit_spirv_float_intrinsic_sin` | `gpu.rs` | 3 |
| 17 | `test_emit_spirv_mixed_int_float` | `gpu.rs` | 3 |
| 18 | `test_emit_spirv_integer_comparison_extended` | `gpu.rs` | 3 |
| 19 | `test_emit_spirv_integer_div_mod` | `gpu.rs` | 3 |
| 20 | `test_emit_spirv_float_literal` | `gpu.rs` | 3 |
| 21 | `test_emit_spirv_multi_buffer_signature` | `gpu.rs` | 4 |
| 22 | `test_emit_spirv_multi_buffer_read_write` | `gpu.rs` | 4 |
| 23 | `test_emit_spirv_shared_memory_global` | `gpu.rs` | 5 |
| 24 | `test_emit_spirv_shared_memory_addrspace_cast` | `gpu.rs` | 5 |
| 25 | `test_collect_shared_mem_sizes_multiple` | `gpu.rs` | 5 |
| 26 | `test_emit_spirv_module_2d_grid` | `gpu.rs` | 6 |
| 27 | `test_emit_spirv_module_1d_grid_no_overhead` | `gpu.rs` | 6 |

**Total new tests: 27.** (26 SPIR-V-specific + 1 pre-existing)
**Final test count: 1025** (981 + 44)

## Implementation Order

All phases completed in order:

| Phase | Items | Status |
|-------|-------|--------|
| **1** | Eligibility relaxation (P5) | ✅ |
| **2** | Thread/block ID intrinsics (P3) | ✅ |
| **3** | Float arithmetic (P1) | ✅ |
| **4** | Multiple storage buffers (P2) | ✅ |
| **5** | Shared memory (P4) | ✅ |
| **6** | Multiple dimensions (P6) | ✅ |
| **7** | Stdlib + arch doc | ✅ |
