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
- `txn_counter` is NEVER rewound — prevents `%t{N}` collisions

### 3. `BlockContext` (src/backend/llvm/context.rs)

**Lifetime:** One basic block.

**Contains:** Current label.

**Rules:**
- Lightweight, rarely needed beyond label tracking
- Created per basic block in loop/phi heavy paths

### 4. Remaining `LlvmBackend` Fields

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

## File Layout

| File | Purpose |
|------|---------|
| `context.rs` | CompilerContext, FunctionContext, BlockContext, FunctionGuard |
| `mod.rs` | LlvmBackend struct, generate(), shared helpers |
| `builder.rs` | *(Phase 1)* LLVMBuilder for structured IR emission |
| `emit_expr.rs` | Expression codegen (to be split into `expr/` submodules) |
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
2. **Single-Source Registry:** All SSA register names come from
   `FunctionContext::next_reg()`.
3. **Flat Code Layout:** Max 3 levels of nesting. Guard clauses first.
4. **Centralized Type Conversions:** All `trunc`/`zext`/`bitcast`/`ptrtoint`
   go through a single type conversion helper.
5. **Explicit FFI Declares:** Every foreign function must have an LLVM
   `declare`.
6. **Unique Temp Files:** GPU compilation uses process+thread ID in filenames.
