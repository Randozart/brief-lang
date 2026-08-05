# Fannkuch Performance Regression: Missing TBAA Metadata

## Symptom

On `main`: Briv 0.0682s, C 0.0685s → **0.99x** (Briv wins, MATCH)
On branch:  Briv 0.1047s, C 0.0647s → **1.61x** (C wins, MATCH)

Briv slowed by **54%** with identical dispatch (both A005c, both work correctly).

## Root Cause

`emit_typed_store` emits stores without `!tbaa` metadata:

```llvm
store i64 %t30, ptr %ap_31, align 8                        ❌ branch
store i64 %t30, ptr %ap_31, align 8, !tbaa !1               ✅ main
```

TBAA (Type-Based Alias Analysis) metadata tags each store and load with
a type node that tells LLVM's optimizer which memory operations may alias.
Two accesses tagged with different type tree roots are guaranteed **no alias**
by the programmer, and LLVM can reorder, hoist, or eliminate them freely.

### What TBAA unlocks

| LLVM pass | Without TBAA | With TBAA |
|-----------|-------------|-----------|
| **GVN** (Global Value Numbering) | Cannot prove `load(p0)` after `store(p1)` is redundant | Proves p0 ≠ p1 → reuses register |
| **LICM** (Loop Invariant Code Motion) | Every store is a clobber — cannot hoist loads | Hoists invariant loads before the loop |
| **SROA** (Scalar Replacement of Aggregates) | Treats chunk alloca as one blob | Decomposes to per-field SSA registers |
| **DSE** (Dead Store Elimination) | Cannot prove `store(p0)` followed by `store(p0)` is dead | Proves no intervening `load(p0)` → removes dead store |

For fannkuch's 16-field-per-tick body, the effect cascades:
1. `&p0 = p1` stores to p0 field index N → TBAA tag = `!14` (Int type)
2. `&p1 = p2` stores to p1 field index N+1 → TBAA tag = `!14` (same Int type)
3. LLVM with TBAA knows `store(gep(state, N))` ≠ `store(gep(state, N+1))` → can keep in registers
4. Without TBAA, LLVM reloads EVERY field after EVERY store → **54% slower**

### How TBAA works in the compiler

The helper function `tbaa_node(&ty, universe)` returns a TBAA metadata index
string like `"1"` or `"14"` based on the field's LLVM type. This index
references a TBAA type tree in the LLVM IR that describes the type hierarchy.

`emit_memory_field_store` at `emit_stmt.rs:68`:
```rust
let tn = crate::backend::llvm::tbaa_node(&ty, self.ctx.type_universe.as_ref());
```

`emit_typed_store` at `emit_stmt.rs:296` does NOT call `tbaa_node`:
```rust
writeln!(out, "{}store {} {}, ptr {}, align {}", indent, ty, tv, p, self.align_of(&ty)).ok();
// No !tbaa !{} suffix
```

## Fix

Add TBAA metadata to the store in `emit_typed_store`, using the same pattern
as `emit_memory_field_store`:

```rust
let tn = crate::backend::llvm::tbaa_node(&ty, self.ctx.type_universe.as_ref());
let is_counter = self.fun.counter_field_name.as_deref() == Some(name);
if (self.fun.needs_state_stores_in_body || is_counter) {
    writeln!(out, "{}store {} {}, ptr {}, align {}, !tbaa !{}",
        indent, ty, tv, p, self.align_of(&ty), tn).ok();
}
```

Also add the `needs_state_stores_in_body` gate to match the conditions in
`emit_memory_field_store`. Without this gate, the compiler emits stores to
`%State` even in the phi-SSA path where they should be insertvalue ops.

## Test Plan

1. `cargo test --lib` — all tests pass
2. `cargo build --release` and `benchmarks/build_and_bench.sh --runtime`
3. fannkuch_redux ratio returns from 1.61x to ~1.0x
4. All other benchmarks remain MATCH
