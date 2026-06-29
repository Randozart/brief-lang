# LLVM Backend Refactoring Plan

**Date:** 2026-06-29
**Status:** Draft
**Target:** Complete within 3-4 weeks of sequential work

## 1. Executive Summary

The LLVM backend (`src/backend/llvm/`) has grown to ~24,795 lines across 13 files.
It compiles a sophisticated domain-specific language with GPU offloading, formal
verification hooks, and advanced optimization passes. However, it has reached
the "textual IR generation wall" — `writeln!`-based string formatting for LLVM IR
has become fragile, the `LlvmBackend` struct is a God object with 60+ fields mixing
global config and per-function state, and several known bugs exist (duplicate SSA
register post-processing, pointer corruption in `emit_decay`, dead fallback blocks,
TOCTOU race in GPU temp files, TBAA metadata gaps).

This plan outlines a phased, additive refactoring that never breaks the working
compiler. Each phase can be verified independently via `cargo test --lib`.

### Core Tenets

1. **ADDITIVE ONLY** — Never modify existing optimization paths. New match arms
   only. The `_ => return None;` fallthrough must remain unchanged.
2. **INTERPRETER IS REFERENCE** — If the interpreter runs it, the backend must
   compile it. Fix codegen, never the interpreter.
3. **EVERY CHANGE IS TESTED** — No exception. Each phase must pass the full test
   suite before the next phase begins.
4. **EVERY CHANGE IS COMMENTED** — Every code change must include a comment
   explaining why it was made and what it fixes or enables (format:
   `// YYYY-MM-DD: <description>`).

---

## 2. Current Architecture Analysis

### 2.1 File Inventory

| File | Lines | Role | Fragility Score |
|------|-------|------|-----------------|
| `emit_expr.rs` | 6,145 | Expression codegen (all Expr variants) | **CRITICAL** |
| `tests.rs` | 6,138 | Backend tests | Low |
| `mod.rs` | 3,399 | `LlvmBackend` definition, `generate()`, shared helpers | **CRITICAL** |
| `loop_engine.rs` | 2,042 | 3 loop emission strategies | High |
| `emit_toplevel.rs` | 1,970 | Header, declares, definitions, state type, init | High |
| `gpu.rs` | 1,892 | GPU offloading / SPIR-V | Medium |
| `emit_stmt.rs` | 944 | Statement codegen | Medium |
| `hazard.rs` | 619 | SLP vectorization hazard analysis | Low |
| `directive.rs` | 425 | Directive resolution (#inline, #unroll, etc.) | Low |
| `reorder.rs` | 397 | Statement reordering for ILP | Low |
| `dispatch.rs` | 389 | Reactor dispatch (sequential / parallel) | Medium |
| `optimizer.rs` | 338 | Optimization strategy auto-selection | Low |
| `kani.rs` | 97 | Kani proof harnesses | Low |
| **Total** | **24,795** | | |

### 2.2 Root Problems

#### P1. God Object (`LlvmBackend` in `mod.rs:634-834`)

The struct carries 60+ fields mixing four distinct lifetimes:

- **Global/immutable during codegen:** `spec`, `field_index_map`, `field_types`,
  `frgn_map`, `triggers`, `constants`, `struct_types`, `enum_types`, `cell_defs`,
  `dep_graph`, `string_constants`, `optimize_budget`, `gpu_offload`, `is_embedded`
- **Per-function/mutable during emission:** `txn_counter`, `within_counter`,
  `let_bindings`, `let_binding_types`, `reg_float_cache`, `reg_type_cache`,
  `ssa_old_int_regs`, `ssa_old_float_regs`, `pending_phi_backedge`,
  `phi_field_regs`, `backedge_field_regs`, `terminated`, `returns_i64`,
  `fn_ret_ty`, `callable_txn_result`, `in_callable_txn`, `loop_exit_label`,
  `phi_induction_reg`, `pending_post_hoist`, `used_phi_loop`, `param_slots`,
  `state_reg_name`, `chimera_map`, `arena_slots`, `field_prealloc_info`
- **Per-block:** `ssa_state_reg`, `reg_float_cache` (transient)
- **Accumulator/global mutable:** `spirv_kernels`, `spirv_blobs`, `report_lines`,
  `warnings`, `pending_metadata`, `remarks`

**Consequence:** `emit_inline_txn_body` (`dispatch.rs:305-344`) must manually
clone/restore 7 state fields. Every new feature that adds a per-function field
risks being forgotten here, causing state contamination across inlined txns.

#### P2. String-Based SSA Register Allocation

```rust
let v = format!("%t{}", self.txn_counter);
self.txn_counter += 1;
```

**Issues:**
- `txn_counter` is sometimes saved/rewound, creating duplicate `%t{N}` defs
- Post-processing pass (`mod.rs:2944-2984`) renames duplicates via string matching
  but does NOT rename subsequent **uses** — only the definition line
- No type tracking at generation time; types are inferred from context, leading to
  type mismatch bugs

**Consequence:** The `%tddup` post-processing pass is fragile, expensive (string
scan of entire output), and creates SSA violations where uses point to the wrong
definition.

#### P3. Scattered Type Boxing

Type coercion logic is duplicated across:
- `emit_stmt.rs:29-100` — `adapt_to_i64()` (Bool→zext, Char→identity,
  String→ptrtoint, Float→bitcast+zext, Float64→bitcast, fixed-width→sext/zext)
- `emit_toplevel.rs:198-226` — `native_float_or_box()`, `ensure_float_reg()`
- Countless inline `trunc`/`zext`/`bitcast`/`ptrtoint` in `emit_expr.rs`

Each new type requires editing all three locations, and they can easily drift.

#### P4. Missing TBAA for "double" (`mod.rs:496-505`)

```rust
pub(super) fn tbaa_node(ty_str: &str) -> i32 {
    match ty_str {
        "i64" => 1,
        "i8"  => 2,
        "i32" => 3,
        "i8*" | "ptr" => 4,
        "float" => 5,
        _ => 1,  // fallback: Int ← "double" falls here!
    }
}
```

**Consequence:** Float64 fields get TBAA node 1 (i64/Int), so LLVM thinks Int and
Float64 accesses MAY ALIAS, preventing GVN load elimination.

#### P5. Known Bugs (see also `BUGS.md`)

1. **`emit_decay` pointer corruption** (`emit_expr.rs:6077-6096`): Uses
   `struct_ptr_name` before declaration, returns GEP of last field instead of
   struct base pointer.
2. **`Within` dead fallback block** (`emit_expr.rs:4474-4536`): Body evaluated
   unconditionally, fallback block never branched to.
3. **TOCTOU race in GPU temp files** (`gpu.rs:1091-1124`): Fixed filenames
   `brief_kernel.ll`/`.spv` collide under parallel builds.
4. **Missing socket declarations** (`emit_expr.rs`): `accept`, `socket`, etc.
   called without LLVM `declare` — rejected by LLVM 15+.
5. **List/tuple literals bypass arena** (`emit_expr.rs:~2753,~2785`): Direct
   `@malloc` instead of `emit_arena_alloc` — memory leak in tight loops.
6. **Hardcoded target triple** (`emit_toplevel.rs:87`): Always emits
   `x86_64-unknown-linux-gnu`, ignoring `TargetSpec`.

---

## 3. Refactoring Phases

### Phase 0: Context Separation (Week 1)

**Goal:** Divide `LlvmBackend` into three distinct context structs to eliminate
the manual save/restore pattern and prevent state leakage.

#### 0a. Define `CompilerContext` (immutable during codegen)

Extract all read-only fields from `LlvmBackend`:

```rust
// src/backend/llvm/context.rs
pub struct CompilerContext {
    // Target & Spec
    pub spec: Option<TargetSpec>,
    pub is_embedded: bool,
    pub library_mode: bool,

    // State layout (immutable after build)
    pub field_index_map: HashMap<String, usize>,
    pub field_types: Vec<String>,
    pub field_brief_types: Vec<Type>,
    pub field_initializers: HashMap<String, Option<Expr>>,
    pub field_modes: HashMap<String, FieldMode>,
    pub cache_slots: HashMap<String, HashMap<String, (usize, usize)>>,

    // FFI & Declarations
    pub frgn_map: HashMap<String, ForeignSignature>,
    pub inop_decls: HashMap<String, InopDeclaration>,
    pub triggers: HashMap<String, TriggerDeclaration>,
    pub trigger_names: Vec<String>,

    // Type info
    pub struct_types: HashMap<String, Vec<(String, Type)>>,
    pub enum_types: HashMap<String, EnumDefinition>,
    pub cell_defs: HashMap<String, CellDef>,
    pub constants: HashMap<String, (Type, Expr)>,
    pub string_constants: Vec<String>,

    // Analysis results
    pub dep_graph: DependencyGraph,
    pub optimize_budget: u64,

    // GPU config
    pub gpu_offload: bool,
    pub gpu_backend: String,

    // Misc
    pub type_universe: Option<TypeUniverse>,
    pub schema_aliases: HashMap<String, DbriefType>,
    pub variant_disc: HashMap<String, (String, u64, usize)>,
    pub exit_condition: Option<Box<Expr>>,
    pub has_natural_exit: bool,

    // Reporting
    pub explain: bool,
    pub dump_layout: bool,
    pub emit_remarks: bool,
}
```

#### 0b. Define `FunctionContext` (per-function, mutable)

```rust
// src/backend/llvm/context.rs
pub struct FunctionContext {
    // SSA register counter — NEVER rewound
    pub txn_counter: usize,
    pub within_counter: usize,
    pub metadata_counter: usize,

    // Local bindings
    pub let_bindings: HashMap<String, String>,
    pub let_binding_types: HashMap<String, Type>,
    pub let_original_types: HashMap<String, Type>,
    pub reg_float_cache: HashMap<String, String>,
    pub reg_type_cache: HashMap<String, Type>,

    // Loop/phi state
    pub ssa_old_int_regs: HashMap<String, String>,
    pub ssa_old_float_regs: HashMap<String, String>,
    pub pending_phi_backedge: HashMap<String, String>,
    pub phi_field_regs: HashMap<String, String>,
    pub backedge_field_regs: HashMap<String, String>,
    pub used_phi_loop: bool,
    pub phi_induction_reg: Option<(String, String, String)>,
    pub loop_exit_label: Option<String>,

    // Function state
    pub terminated: bool,
    pub returns_i64: bool,
    pub fn_ret_ty: String,
    pub main_body: bool,
    pub in_callable_txn: bool,
    pub callable_txn_result: Option<String>,
    pub callable_txn_post_label: Option<String>,

    // Arena (per-function scope)
    pub arena_slots: Option<(String, String, String)>,
    pub field_prealloc_info: HashMap<String, (String, String)>,

    // Accumulators
    pub pending_metadata: String,
    pub pending_post_hoist: Vec<(String, String)>,
    pub pending_cleanup: Vec<Statement>,

    // Params
    pub param_slots: HashMap<String, String>,
    pub state_reg_name: String,
    pub ssa_state_reg: Option<String>,

    // Chimera tracking
    pub chimera_map: HashMap<String, ChimeraInfo>,
}
```

#### 0c. Define `BlockContext` (per-basic-block, lightweight)

```rust
pub struct BlockContext {
    pub label: String,
    // Transient allocations freed at block exit
    pub transient_regs: Vec<String>,
}
```

#### 0d. Migration Strategy

1. Create `src/backend/llvm/context.rs` with the three context structs
2. Add a `CompilerContext` field and `FunctionContext` field to `LlvmBackend`
3. Move all read-only accesses from `self.field_index_map` etc. to `self.ctx.field_index_map`
4. Move all per-function accesses from `self.txn_counter` etc. to `self.fun.txn_counter`
5. Replace the save/restore block in `emit_inline_txn_body` with:
   ```rust
   let saved_fun = self.fun.clone(); // if FunctionContext: Clone
   // ... emit body ...
   self.fun = saved_fun;
   ```
   **Or** use an RAII guard:
   ```rust
   struct FunGuard<'a> { slot: &'a mut FunctionContext, saved: FunctionContext }
   impl Drop for FunGuard<'_> { fn drop(&mut self) { std::mem::swap(self.slot, &mut self.saved); } }
   ```

**Verification:** `cargo test --lib` passes. No behavioral change.

---

### Phase 1: LLVM IR Builder (Week 1-2)

**Goal:** Replace raw `writeln!` instruction emission with a structured builder
that handles SSA register allocation, label generation, and type verification.

#### 1a. Define the Builder

```rust
// src/backend/llvm/builder.rs
pub enum LlvmType { I1, I8, I16, I32, I64, Float, Double, Ptr(Option<Box<LlvmType>>), Void }
pub struct Instruction { result: Option<String>, op: String, comment: Option<String> }

pub struct LLVMBuilder {
    instructions: Vec<Instruction>,
    reg_counter: usize,
    label_counter: usize,
}

impl LLVMBuilder {
    pub fn new() -> Self;
    pub fn gen_reg(&mut self) -> String;        // Returns "%t{N}" — NEVER rewound
    pub fn gen_label(&mut self, prefix: &str) -> String;

    // High-level builders (DRY + type-safe)
    pub fn emit_add(&mut self, ty: LlvmType, lhs: &str, rhs: &str) -> String;
    pub fn emit_sub(&mut self, ty: LlvmType, lhs: &str, rhs: &str) -> String;
    pub fn emit_mul(&mut self, ty: LlvmType, lhs: &str, rhs: &str) -> String;
    pub fn emit_zext(&mut self, from: LlvmType, to: LlvmType, val: &str) -> String;
    pub fn emit_sext(&mut self, from: LlvmType, to: LlvmType, val: &str) -> String;
    pub fn emit_trunc(&mut self, from: LlvmType, to: LlvmType, val: &str) -> String;
    pub fn emit_bitcast(&mut self, from: LlvmType, to: LlvmType, val: &str) -> String;
    pub fn emit_ptrtoint(&mut self, val: &str, to: LlvmType) -> String;
    pub fn emit_inttoptr(&mut self, val: &str, to: LlvmType) -> String;
    pub fn emit_load(&mut self, ty: LlvmType, ptr: &str, align: usize) -> String;
    pub fn emit_store(&mut self, ty: LlvmType, val: &str, ptr: &str, align: usize);
    pub fn emit_alloca(&mut self, ty: LlvmType, align: usize) -> String;
    pub fn emit_gep(&mut self, base_ty: LlvmType, ptr: &str, indices: &[&str]) -> String;
    pub fn emit_call(&mut self, ret_ty: LlvmType, callee: &str, args: &[(&str, &str)]) -> String;
    pub fn emit_icmp(&mut self, cond: &str, ty: LlvmType, lhs: &str, rhs: &str) -> String;
    pub fn emit_fcmp(&mut self, cond: &str, lhs: &str, rhs: &str) -> String;
    pub fn emit_br(&mut self, dest: &str);
    pub fn emit_cond_br(&mut self, cond: &str, true_dest: &str, false_dest: &str);
    pub fn emit_label(&mut self, label: &str);
    pub fn emit_phi(&mut self, ty: LlvmType, incoming: &[(&str, &str)]) -> String;
    pub fn emit_select(&mut self, ty: LlvmType, cond: &str, true_val: &str, false_val: &str) -> String;

    /// Emit metadata attachment to the last instruction
    pub fn attach_metadata(&mut self, node: &str);

    /// Finalize: convert to formatted LLVM IR string
    pub fn finish(&self) -> String;
}
```

#### 1b. Type Converter (Centralized)

```rust
// src/backend/llvm/builder.rs (or typeconv.rs)
pub struct TypeConverter;

impl TypeConverter {
    /// Box any value to i64 for uniform storage in %State
    pub fn box_to_i64(builder: &mut LLVMBuilder, val: &str, ty: &Type) -> String;
    /// Unbox i64 from %State back to native type
    pub fn unbox_from_i64(builder: &mut LLVMBuilder, val: &str, target_ty: &Type) -> String;
    /// Box float to i64 (bitcast float→i32→zext→i64)
    pub fn float_to_boxed(builder: &mut LLVMBuilder, val: &str) -> String;
    /// Unbox i64 to float (trunc i64→i32→bitcast→float)
    pub fn boxed_to_float(builder: &mut LLVMBuilder, val: &str) -> String;
}
```

#### 1c. Migration Pattern

Not all instructions need to migrate at once. The migration plan:

1. Add `LLVMBuilder` alongside the existing `out: &mut String` parameter
2. Add a `fn emit_llvm_ir(&mut self, builder: &mut LLVMBuilder, ...)` alternative
   entry point for new code
3. Gradually migrate match arms in `emit_expr.rs` to use the builder
4. Once all arms use the builder, remove the `out: &mut String` parameter and
   the post-processing `%tddup` pass

**Key rule:** `builder.gen_reg()` is the **sole** source of register names.
No more `format!("%t{}", self.txn_counter)` outside the builder. This
mathematically eliminates duplicate SSA definitions.

**Verification:** After each sub-phase migration, `cargo test --lib` passes.
The final phase removes the `%tddup` pass and verifies all tests still pass.

---

### Phase 2: Fix Known Bugs (Week 2)

**Goal:** Fix all known bugs without architectural changes. Each fix in its own
commit with a test.

#### Bug 1: `emit_decay` scope/pointer fix

In `emit_expr.rs:6077-6096`:
- Replace `struct_ptr_name` with `struct_ptr` inside the loop
- Return `alloc` (the base pointer) instead of the last field's GEP

#### Bug 2: TBAA "double" mapping

In `mod.rs:496-505`, add:
```rust
"double" | "float" => 5, // Float64 → same TBAA node as Float32
```

#### Bug 3: TOCTOU race in GPU temp files

In `gpu.rs:1091-1124`, use unique filenames:
```rust
let unique_id = format!("brief_kernel_{}_{}",
    std::process::id(),
    std::thread::current().id().as_u64());
let ir_path = tmp_dir.join(format!("{}.ll", unique_id));
let spv_path = tmp_dir.join(format!("{}.spv", unique_id));
```

#### Bug 4: `Within` dead fallback block

In `emit_expr.rs:4474-4536`:
- Remove the unconditional body evaluation before the IR blocks
- Emit a cooperative polling counter that conditionally branches to `l_fallback`
- If cooperative polling is not feasible, emit a compiler warning and document
  the limitation

#### Bug 5: List/tuple literal arena allocation

Route `ListLiteral` and `Tuple` literal allocations through
`emit_arena_alloc()` instead of direct `@malloc`, so temporaries are reclaimed
at tick end.

#### Bug 6: Socket API declarations

Add LLVM `declare` lines in `emit_toplevel.rs:emit_declares()` for:
`socket`, `bind`, `connect`, `accept`, `send`, `recv`, `setsockopt`, `getsockopt`

#### Bug 7: Hardcoded target triple

In `emit_toplevel.rs:emit_header()`:
- Read `target_triple` and `data_layout` from `self.ctx.spec` if available
- Fall back to `x86_64-unknown-linux-gnu` if no spec is loaded

**Verification:** Each bug fix has its own test. `cargo test --lib` passes.

---

### Phase 3: Modularize `emit_expr.rs` (Week 2-3)

**Goal:** Split the 6,145-line monolith into focused submodules.

#### New Submodule Structure

```
src/backend/llvm/expr/
  mod.rs            -- Re-exports, common helpers
  literal.rs        -- Expr::Integer, Expr::Float, Expr::Float64, Expr::Bool,
                       Expr::String, Expr::Char, Expr::Term, Expr::Literal
  math.rs           -- Expr::Add, Sub, Mul, Div, Mod, Neg, BinaryOp, UnaryOp
  compare.rs        -- Expr::Eq, Ne, Lt, Le, Gt, Ge, And, Or, Not, BitAnd, etc.
  collections.rs    -- ListLiteral, MapLiteral, SetLiteral, Tuple, Slice,
                       MultiSlice, ListIndex, Concat
  field.rs          -- FieldAccess, StructInstance, ObjectLiteral, ArrowMut,
                       ArrowDiscard, ArrowTransfer
  control.rs        -- Expr::Match, PatternMatch, Block, Within
  call.rs           -- Expr::Call, CellCall, IntrinsicCall, SigCall
  projection.rs     -- Projection, SubtypeProjection
  intrinsics/
    mod.rs
    io.rs           -- print, read, file I/O
    sys.rs          -- process spawn, signals, memory, threading
    net.rs          -- socket, bind, connect, accept
    math_intr.rs    -- sqrt, fabs, ceil, floor, ctpop, etc.
  misc.rs           -- Expr::Identifier, OwnedRef, PriorState, Cast, IsType,
                       FromCheck, Like, Ellipsis, Interpolate
```

#### Migration Strategy

1. Create the `expr/` directory with `mod.rs`
2. One submodule at a time: move match arms from the big `emit_expr` match
   into the submodule's handler function
3. Each submodule exposes `pub fn emit_<variant>(...) -> TypedRegister`
4. The main `emit_expr` becomes a thin dispatcher:
   ```rust
   pub(crate) fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> TypedRegister {
       let expr = expr.clone();
       let indent = if indent.is_empty() { "  " } else { indent };
       match &expr {
           Expr::Integer(_) => expr::literal::emit_integer(out, &expr, indent, &mut self.txn_counter),
           Expr::Float(_) => expr::literal::emit_float(out, &expr, indent, &mut self.txn_counter),
           Expr::Add(_, _) => expr::math::emit_binary(out, &expr, indent, self),
           // ... one line per variant
       }
   }
   ```

**Verification:** `cargo test --lib` passes after each submodule migration.

---

### Phase 4: Decouple the Backend as a Reference (Week 3)

**Goal:** Make the LLVM backend's architecture a clean, documented template that
other backends (Webstack, CIRCT) can follow.

#### 4a. Define a `BackendContext` trait

```rust
// src/backend/traits.rs
pub trait BackendContext {
    fn compiler_ctx(&self) -> &CompilerContext;
    fn compiler_ctx_mut(&mut self) -> &mut CompilerContext;
    fn function_ctx(&self) -> &FunctionContext;
    fn function_ctx_mut(&mut self) -> &mut FunctionContext;
}
```

#### 4b. Document the architecture pattern

In `docs/architecture/backend-refactor.md`:

```
# Backend Architecture Template

Every backend must implement:

1. **CompilerContext** — read-only during codegen. Holds AST-level definitions,
   target spec, FFI signatures. Immutable after construction.

2. **FunctionContext** — per-function mutable state. Holds SSA counter, local
   bindings, phi state. Clone at inline boundaries; restore after.

3. **BlockContext** — per-basic-block. Holds label, transient registers.
   Rarely needed beyond label tracking.

4. **IR Builder** — sole interface for IR instruction emission. Handles all
   register allocation. Mathematical guarantee: no duplicate register definitions.
```

#### 4c. Update `router.rs` to support the new pattern

The `ExprCodegenLLVM` trait in `features/traits.rs:69-76` currently passes
`&mut LlvmBackend`. Change to pass `(&mut CompilerContext, &mut FunctionContext, &mut LLVMBuilder)`
so feature structs don't depend on the backend directly.

**But**: This is a large interface change that touches every feature trait impl.
Strategy:
1. Add a new trait `ExprCodegenLLVM2` with the cleaner signature
2. Migrate features one at a time
3. Remove the old trait when migration is complete

```rust
pub trait ExprCodegenLLVM2 {
    fn emit_llvm(
        &self,
        ctx: &mut CompilerContext,
        fun: &mut FunctionContext,
        builder: &mut LLVMBuilder,
    ) -> TypedRegister;
}
```

---

### Phase 5: Remove Post-Processing & Stabilize (Week 3-4)

**Goal:** Once all expression emission uses `LLVMBuilder`, remove the
`%tddup` post-processing pass entirely.

#### 5a. Verification Checklist

- [ ] All `writeln!(out, ...)` in `emit_expr.rs` replaced with `builder.emit_*(...)`
- [ ] `gen_reg()` is the sole register name source
- [ ] No line in the codebase contains `format!("%t{}", ...)` except inside `gen_reg()`
- [ ] The post-processing block at `mod.rs:2944-2984` is deleted
- [ ] `cargo test --lib` passes
- [ ] All benchmarks produce correct output

#### 5b. Performance Regression Check

Benchmark both before and after:
```bash
bash benchmarks/build_and_bench.sh --runtime
bash benchmarks/build_and_bench.sh --optimizer
bash benchmarks/build_and_bench.sh --correctness
```

The builder abstraction adds a tiny allocation cost (building `Vec<Instruction>`
then formatting). If this is measurable (>1% regression), add a `finish_fast()`
path that writes directly to a `&mut String` while still centralizing register
allocation.

---

## 4. Migrating `expr/` Submodule Structure

### Detailed Plan: `src/backend/llvm/expr/math.rs`

```rust
// src/backend/llvm/expr/math.rs
use crate::ast::{Expr, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

pub fn emit_binary_op(
    out: &mut String,
    expr: &Expr,
    indent: &str,
    backend: &mut LlvmBackend,
) -> TypedRegister {
    // 2026-06-29: Normalize new-style BinaryOp to old variants so the
    // match below can process them. Without this, emit_expr falls through
    // without matching, producing wrong IR for all binary operations.
    if let Some(norm) = expr.normalize_to_old() {
        return backend.emit_expr(out, &norm, indent);
    }
    // ... existing match arms for Add, Sub, Mul, Div, Mod ...
}
```

Each submodule follows the same pattern:
1. Normalize BinaryOp/UnaryOp to old-style variants
2. Match the operation
3. Emit via builder or direct `writeln!`
4. Return `TypedRegister`

---

## 5. Testing Strategy

### Per-Phase Tests

| Phase | Tests | Cmd |
|-------|-------|-----|
| Phase 0 | Existing tests + context isolation test | `cargo test --lib backend::llvm` |
| Phase 1 | Builder unit tests, SSA uniqueness test | `cargo test --lib builder` |
| Phase 2 | Bug-specific regression tests | `cargo test --lib` |
| Phase 3 | Sub-module tests per expression category | `cargo test --lib` |
| Phase 4 | Trait coherence tests | `cargo test --lib` |
| Phase 5 | Full regression + benchmarks | `cargo test --lib` + `bash benchmarks/build_and_bench.sh --correctness` |

### Kani Harnesses

Every new public function in the builder module must have a Kani proof harness
that verifies:
1. `gen_reg()` never returns the same name twice within a builder instance
2. `emit_add()` produces correctly formatted IR
3. `finish()` returns valid LLVM IR string

### Anti-Regression Check

The following must never change:
- **Existing match arms in optimization paths** — only add new arms
- **The `_ => return None;` fallthrough** in optimization passes
- **Interpreter output** for any given program input

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Context separation breaks existing codegen | Medium | High | Additive approach: keep old fields alongside new context structs until fully migrated |
| Builder abstraction adds compile-time overhead | Low | Medium | `finish_fast()` direct-write fallback path |
| Phase 3 submodule split creates circular deps | Low | Medium | `emit_expr.rs` retains `mod` declarations; sub-modules only depend on `crate::backend::llvm` |
| `ExprCodegenLLVM2` trait migration creates churn across all feature files | High | High | Keep old trait active until ALL features migrated; one feature at a time |
| Removing `%tddup` pass reveals latent SSA bugs | Medium | Critical | Thorough testing with `--correctness` before removal |

---

## 7. Files to Create / Modify

### New Files

| File | Purpose | Phase |
|------|---------|-------|
| `src/backend/llvm/context.rs` | `CompilerContext`, `FunctionContext`, `BlockContext` | 0 |
| `src/backend/llvm/builder.rs` | `LLVMBuilder`, `LlvmType`, `Instruction` | 1 |
| `src/backend/llvm/typeconv.rs` | `TypeConverter` (centralized box/unbox) | 1 |
| `src/backend/llvm/expr/mod.rs` | Sub-module re-exports | 3 |
| `src/backend/llvm/expr/literal.rs` | Literal expression codegen | 3 |
| `src/backend/llvm/expr/math.rs` | Arithmetic expression codegen | 3 |
| `src/backend/llvm/expr/compare.rs` | Comparison/boolean codegen | 3 |
| `src/backend/llvm/expr/collections.rs` | List/map/set/tuple codegen | 3 |
| `src/backend/llvm/expr/field.rs` | Field/struct/arrow codegen | 3 |
| `src/backend/llvm/expr/control.rs` | Match/block/within codegen | 3 |
| `src/backend/llvm/expr/call.rs` | Call/intrinsic codegen | 3 |
| `src/backend/llvm/expr/projection.rs` | Projection codegen | 3 |
| `src/backend/llvm/expr/intrinsics/mod.rs` | Intrinsic sub-modules | 3 |
| `src/backend/llvm/expr/intrinsics/io.rs` | I/O intrinsics | 3 |
| `src/backend/llvm/expr/intrinsics/sys.rs` | System intrinsics | 3 |
| `src/backend/llvm/expr/intrinsics/net.rs` | Network intrinsics | 3 |
| `src/backend/llvm/expr/intrinsics/math_intr.rs` | Math intrinsics | 3 |
| `src/backend/llvm/expr/misc.rs` | Misc expression codegen | 3 |
| `src/backend/traits.rs` | `BackendContext` trait | 4 |
| `docs/architecture/backend-refactor.md` | Architecture template guide | 4 |

### Modified Files

| File | Changes | Phase |
|------|---------|-------|
| `src/backend/llvm/mod.rs` | Add context/builder fields, remove post-processing | 0-5 |
| `src/backend/llvm/emit_expr.rs` | Thin dispatcher after Phase 3 | 3 |
| `src/backend/llvm/emit_stmt.rs` | Use builder, fix `adapt_to_i64` | 1-2 |
| `src/backend/llvm/emit_toplevel.rs` | Dynamic target triple, fix TBAA | 2 |
| `src/backend/llvm/dispatch.rs` | Simplify save/restore w/ `FunctionContext` | 0 |
| `src/backend/llvm/loop_engine.rs` | Use builder, `FunctionContext` | 0-1 |
| `src/backend/llvm/gpu.rs` | Fix TOCTOU | 2 |
| `src/features/traits.rs` | Add `ExprCodegenLLVM2` | 4 |
| `src/backend/router.rs` | Flesh out router | 4 |
| `BUGS.md` | Mark fixed bugs | 2 |

---

## 8. Prioritized Execution Order

```
Week 1:
  Mon-Tue:  Phase 0a — CompilerContext extraction
  Wed-Thu:  Phase 0b — FunctionContext extraction
  Fri:      Phase 0c-d — BlockContext, save/restore simplification

Week 2:
  Mon-Tue:  Phase 1a — LLVMBuilder core + type converter
  Wed:      Phase 1b — Migrate literal emission to builder
  Thu-Fri:  Phase 2a — Bug fixes (emit_decay, TBAA, TOCTOU)

Week 3:
  Mon:      Phase 2b — Bug fixes (Within, malloc→arena, sockets, triple)
  Tue-Fri:  Phase 3a-d — expr/ submodulization (literal, math, compare, collections)

Week 4:
  Mon-Tue:  Phase 3e-h — expr/ submodulization (field, control, call, intrinsics)
  Wed:      Phase 4a — BackendContext trait + router update
  Thu:      Phase 5 — Remove %tddup post-processing, final verification
  Fri:      Documentation, AGENTS.md update, cleanup
```

---

## 9. Success Criteria

1. `cargo test --lib` passes at every commit
2. All benchmarks produce correct output (symmetric with C references)
3. No `%tddup` post-processing pass — SSA registers are unique by construction
4. `emit_inline_txn_body` save/restore is a single `std::mem::swap` or RAII guard
5. Target triple is read from `TargetSpec`, not hardcoded
6. All 6 known bugs listed in Phase 2 are fixed with regression tests
7. Architecture is documented as a template for other backends
8. `docs/architecture/backend-refactor.md` exists and is accurate
9. No TODO, todo!, unreachable!, or stubs remain
10. Praetor complexity ≤ 15, lines ≤ 100, params ≤ 6 on all new files
