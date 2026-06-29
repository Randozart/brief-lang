# Backend Architecture: Context-Driven LLVM Codegen

**Updated:** 2026-06-29
**Applies to:** LLVM Backend (`src/backend/llvm/`)

## Architecture Overview

The LLVM backend is organized around a **three-tier context architecture** that
replaces the previous monolithic `LlvmBackend` struct. This prevents state
leakage across compiled functions and eliminates fragile manual save/restore
patterns.

```
┌─────────────────────────────────────────────────────────┐
│                    LlvmBackend                          │
│  (orchestration, I/O, reporting, accumulators)          │
│                                                         │
│  ┌────────────────────┐   ┌─────────────────────────┐   │
│  │  CompilerContext    │   │  FunctionContext        │   │
│  │  (global, read-only)│   │  (per-function, mutable)│   │
│  │                     │   │                         │   │
│  │  • field_index_map  │   │  • txn_counter          │   │
│  │  • field_types      │   │  • let_bindings         │   │
│  │  • frgn_map         │   │  • reg_float_cache      │   │
│  │  • triggers         │   │  • ssa_old_int_regs     │   │
│  │  • constants        │   │  • pending_phi_backedge │   │
│  │  • struct_types     │   │  • arena_slots          │   │
│  │  • 40+ other fields │   │  • 30+ other fields     │   │
│  └────────────────────┘   └─────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Context Tiers

### 1. `CompilerContext` (src/backend/llvm/context.rs)

**Lifetime:** Entire compilation. Created in `generate()`, never modified
during per-function codegen.

**Contains:** Target spec, state layout, FFI signatures, type definitions,
constants, trigger declarations, optimization parameters.

**Rules:**
- No `&mut` access during expression/statement emission
- Read-only through `&self.ctx`
- Fields populated during `generate()` setup, then frozen

### 2. `FunctionContext` (src/backend/llvm/context.rs)

**Lifetime:** One function or transaction. Reset at each function entry.

**Contains:** SSA register counter, local variable bindings, register type
caches, phi/loop state, function-level flags, arena allocator state.

**Rules:**
- Created fresh for each `emit_definition`, `emit_transaction`, etc.
- Cloned via `FunctionGuard::new()` for inline body scopes
- Restored via `guard.restore(&mut self.fun)`
- `txn_counter` reset at function boundaries only (each LLVM `define`
  has its own SSA namespace)
- `next_reg()` is the SOLE source of `%t{N}` register names

### 3. `BlockContext` (src/backend/llvm/context.rs)

**Lifetime:** One basic block.

**Contains:** Current label.

**Rules:**
- Lightweight, rarely needed beyond label tracking
- Created per basic block in loop/phi heavy paths

### 4. `LLVMBuilder` (src/backend/llvm/builder.rs)

**Lifetime:** Per-expression or per-function. Created locally in emit_expr
dispatch arms and passed to feature trait methods.

**Contains:** Accumulated `Vec<Instruction>`, `reg_counter` (monotonically
increasing), `label_counter` (monotonically increasing).

**Features:**
- 38 typed instruction builders (emit_add, emit_store, emit_zext, etc.)
- `gen_reg()` as sole register name source (parallels `next_reg()`)
- `finish()` / `finish_into()` for IR string output
- `writeln()` bridge method for gradual migration from raw string output
- `TypeConverter` with `box_to_i64()` / `unbox_from_i64()`

### 5. Remaining `LlvmBackend` Fields

Fields that did not fit into CompilerContext or FunctionContext stay directly
on `LlvmBackend`:
- **Accumulators:** `report_lines`, `warnings`, `remarks`
- **GPU:** `spirv_kernels`, `spirv_blobs`
- **Async/threading:** `has_async_txns`, `async_txn_names`, `is_lightweight_async`
- **Working state:** `fused_to_first`, `sampled_triggers`, `txn_write_masks`

## FunctionGuard Pattern

The `FunctionGuard` replaces the old manual save/restore pattern:

```rust
// OLD: fragile 7-field save/restore
let saved_bindings = self.let_bindings.clone();
let saved_types = self.let_binding_types.clone();
// ... modify ...
self.let_bindings = saved_bindings;
self.let_binding_types = saved_types;
// Forgets newly added fields!

// NEW: snapshot entire FunctionContext
let guard = FunctionGuard::new(&self.fun);
// ... modify self.fun extensively ...
guard.restore(&mut self.fun);
// All fields automatically restored.
```

## Trait Architecture

Both codegen traits now pass `builder: &mut LLVMBuilder` and
`emit_expr: &mut dyn FnMut(...)`:

```rust
pub trait ExprCodegenLLVM {
    fn emit_llvm(
        &self,
        ctx: &mut LlvmBackend,       // backend access (for emit_expr, etc.)
        out: &mut String,            // output buffer (being phased out)
        builder: &mut LLVMBuilder,   // structured IR builder (new, preferred)
        dispatch: &ExprDispatch,
        emit_expr: &mut dyn FnMut(   // sub-expression recursion
            &mut LlvmBackend, &mut String, &mut LLVMBuilder, &Expr, &str
        ) -> TypedRegister,
    ) -> TypedRegister;
}
```

## File Layout

| File | Purpose |
|------|---------|
| `context.rs` | CompilerContext, FunctionContext, BlockContext, FunctionGuard |
| `builder.rs` | LLVMBuilder, Instruction, LlvmType, TypeConverter |
| `mod.rs` | LlvmBackend struct, generate(), shared API |
| `helpers.rs` | Shared helper functions (emit_binop, emit_fcmp, etc.) |
| `emit_expr.rs` | **43 lines** — thin dispatcher to `expr/` submodules |
| `expr/literal.rs` | Integer, Float, Bool, String, Char, Term |
| `expr/math.rs` | Add, Sub, Mul, Div, Mod, Neg, bitwise ops |
| `expr/compare.rs` | Eq, Ne, Lt, Le, Gt, Ge, And, Or, Not |
| `expr/collections.rs` | ListLiteral, Tuple |
| `expr/intrinsics.rs` | 200+ intrinsic variants (sqrt, sin, print, etc.) |
| `expr/identifier.rs` | Identifier, OwnedRef, PriorState |
| `expr/rest.rs` | All remaining handlers (Call, FieldAccess, Arrow, Match, etc.) |
| `emit_stmt.rs` | Statement codegen |
| `emit_toplevel.rs` | Top-level emits (header, declares, definitions) |
| `loop_engine.rs` | 3 loop emission strategies |
| `dispatch.rs` | Reactor dispatch (sequential/parallel) |
| `optimizer.rs` | Optimization strategy auto-selection |
| `hazard.rs` | SLP vectorization hazard analysis |
| `directive.rs` | Directive resolution |
| `reorder.rs` | Statement reordering for ILP |
| `gpu.rs` | GPU offloading / SPIR-V |
| `kani.rs` | Kani proof harnesses |

## Design Rules

1. **No Global State Pollution:** Transient compilation variables go in
   `FunctionContext`, never on `LlvmBackend`.
2. **Single-Source Registry:** All `%t{N}` SSA register names come from
   `FunctionContext::next_reg()`. The `%tddup` post-processing pass was
   removed because it's impossible to produce duplicate `%t{N}` definitions
   when all allocation goes through `next_reg()`.
3. **Flat Code Layout:** Max 3 levels of nesting. Guard clauses first.
   Deeply nested match arms get extracted into submodule functions.
4. **Centralized Type Conversions:** All `trunc`/`zext`/`bitcast`/`ptrtoint`
   go through a single type conversion helper (`TypeConverter`).
5. **Explicit FFI Declares:** Every foreign function must have an LLVM
   `declare`.
6. **Unique Temp Files:** GPU compilation uses process+thread ID in filenames.
7. **RAII FunctionGuard:** Inline txn bodies use `FunctionGuard::new()` +
   `guard.restore()` instead of manual 7-field save/restore.
