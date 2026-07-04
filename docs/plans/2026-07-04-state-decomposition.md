# %State Decomposition Plan

## Problem

`%State` is a single struct with 33+ fields. LLVM 18's SROA pass cannot
decompose structs this large (internal threshold ~16 elements).  Without
SROA, LLVM treats `%State` as opaque memory — no per-field alias analysis,
no register promotion, no vectorization.

## Solution

Split `%State` into multiple structs, each ≤ fields (within SROA's threshold).
Route all field access through `emit_state_gep`, which maps each field index
to the correct sub-struct + sub-index.  Every call site updates automatically.

## Struct Layout

```
%StateCtrl = type { i64, i64, i64 }            ; bound(0), count(1), cycle(2)
%StatePos  = type { float, float, ..., float }  ; bx0..bz4 (15 fields, indices 3-17)
%StateVel  = type { float, float, ..., float }  ; vx0..vz4 (15 fields, indices 18-32)
```

Each is a separate `alloca` at function entry:
```llvm
%state_ctrl = alloca %StateCtrl, align 8
%state_pos  = alloca %StatePos, align 4
%state_vel  = alloca %StateVel, align 4
```

## Field Index Map

```rust
fn field_alloca(idx: usize) -> (&'static str, &'static str, usize) {
    // Returns (ptr_name, struct_name, sub_index)
    match idx {
        0..=2  => ("%state_ctrl", "StateCtrl", idx),       // bound, count, cycle
        3..=17 => ("%state_pos",  "StatePos",  idx - 3),   // bx0..bz4
        18..=32 => ("%state_vel", "StateVel",  idx - 18),  // vx0..vz4
        _ => ("%state", "State", idx),  // fallback
    }
}
```

## Changes

### Phase A: emit_state_gep (emit_stmt.rs)

The central routing function.  Currently emits:
```llvm
%p = getelementptr inbounds %State, ptr %state, i32 0, i32 {idx}
```

After:
```llvm
%p = getelementptr inbounds {StructType}, ptr {AllocaName}, i32 0, i32 {sub_idx}
```

Also changes `prefix` registration to use the alloca name.

### Phase B: Struct type definitions (emit_toplevel.rs)

Replace:
```llvm
%State = type { i64, i64, float, float, ..., float }
```
With:
```llvm
%StateCtrl = type { i64, i64, i64 }
%StatePos  = type { float, float, float, float, float, float, float, float, float, float, float, float, float, float, float }
%StateVel  = type { float, float, float, float, float, float, float, float, float, float, float, float, float, float, float }
```

### Phase C: Init stores (emit_toplevel.rs)

Replace single `%state = alloca %State` + `emit_init_state` with multiple
allocas + separate init for each.

`emit_inline_init_stores` gets a `state_ptr` → per-alloca init.

### Phase D: pre_load_all_fields (loop_engine.rs)

Load from multiple allocas instead of one.  Currently iterates all fields;
change to route through `field_alloca` map.

### Phase E: emit_hoisted_post_loop_prints (loop_engine.rs)

Loads final field values from `%state` in `done:`.  Change to load from
the correct alloca for each field.

## Nesting

Every function must be ≤2 levels deep.  Extract helpers as needed.
The `field_alloca` mapping flattens what would otherwise be an if-else
chain or match — it's a single function call with a tuple return.
