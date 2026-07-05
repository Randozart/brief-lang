# Vector Phi Emission for Register Pressure Reduction

Date: 2026-07-05
Status: Execution
Target: Reduce nbody_sqrt from 1.25x to ≤1.05x

## 1. The Problem

nbody_sqrt runs at **1.25x vs C** (3.69s vs 2.93s). The gap is caused by
**register pressure from 32 scalar float phi nodes** at the loop header.

### 1.1 Investigation History

Five hypotheses were investigated and disproven:

| Hypothesis | Finding |
|------------|---------|
| **FMA codegen**: Brief might not emit FMA instructions | Both emit zero FMA on Ivybridge (no `-fma` flag). Not the issue. |
| **Phi emission order**: HashMap iteration scrambles phi order, confusing SLP | SLP traces def-use chains, not phi list position. No impact. |
| **Energy computation location**: Placed outside hot loop vs C's inside | Both correctly place it in a post-loop epilogue. No difference. |
| **Field reindexing**: stride-5 stores prevent SLP packing | SLP operates on the IR, not field order. No stores in Path A, so SLP traces nothing. |
| **Store reordering**: sort stores by field index for stride-1 | Path A suppresses ALL stores. Nothing to reorder. |

### 1.2 Root Cause: 32 Scalar Float Phis = 16 Register Spills

The A005c dispatch creates one `float` phi per state field at the loop header.
For nbody_sqrt's 30 position/velocity fields + 1 counter + 1 bound, this is
**32 live float values** across the backedge.

| Metric | Brief | C |
|--------|-------|---|
| **Float values in hot loop** | **32** (phi nodes) | **22** (phi nodes, some vector) |
| **XMM registers** | 16 | 16 |
| **Spills** | **16 values spilled** | **6 values spilled** |
| **Spill cost** | 16 stores + 16 loads × ~3 cycles × 50M = **~0.69s** | ~0.26s |
| **Total runtime** | 3.69s | 2.93s |
| **Gap explained** | 0.76s | ← spill overhead accounts for **97% of gap** |

C's clang uses array accesses (`vx[i]`) which LLVM promotes to `<4 x float>`
vector phis via SROA. With vector phis, 4 fields fit in one register.
5 velocity components per dimension → 1 `<4 x float>` + 1 scalar = 2 registers
instead of 5. This reduces the 32 values to ~8 vector phis, eliminating spills.

Brief's flat fields (`vx0`, `vx1`, ..., `vx4`) are each individual scalar phis.
LLVM cannot promote them to vector phis because they're not in an array.

## 2. Solution: Grouped Vector Phi Emission

Instead of emitting individual `float` phis for related fields, group them
into `<4 x float>` vector phis.  Inside the body, use `extractelement` to
access individual components and `insertelement` to update them.

### 2.1 Grouping Strategy

Group fields by their prefix and base name:

| Pattern | Fields | Group | Vector phi |
|---------|--------|-------|------------|
| `bx0..bx4` | 5 floats | pos_x | `<4 x float>` + scalar |
| `by0..by4` | 5 floats | pos_y | `<4 x float>` + scalar |
| `bz0..bz4` | 5 floats | pos_z | `<4 x float>` + scalar |
| `vx0..vx4` | 5 floats | vel_x | `<4 x float>` + scalar |
| `vy0..vy4` | 5 floats | vel_y | `<4 x float>` + scalar |
| `vz0..vz4` | 5 floats | vel_z | `<4 x float>` + scalar |
| `count, bound` | 2 ints | control | scalar i64 |

Result: **~8 vector phis** instead of **32 scalar phis**. Fits in 16 XMM
registers with room to spare.

### 2.2 Implementation

The change is in `emit_countable_setup_phis_and_header` and the latch
backedge emission.

#### 2.2.1 Detection

A function `group_fields_for_vector_phi` that:

1. Scans `field_index_map` for fields with names matching `[a-z]+[0-9]+`
   (letter prefix + digit)
2. Groups by the letter prefix (e.g., `vx`, `vy`, `vz`)
3. For each group of 4+ fields with the same type, creates a `<4 x float>` phi
4. Remaining fields (≤3 per group, or non-matching names) stay as scalar phis

For nbody_sqrt:
- `vx0..vx4` → no match for base (prefix "vx" has group size 5)
- Actually, the detection should look for fields with the same BASE (first letter)
  and COMPONENT (second letter), grouped by INDEX (digit):
  - `vx0, vx1, vx2, vx3, vx4`: base="v", component="x", indices=0..4 → vel_x group
  - `vy0..vy4`: base="v", component="y" → vel_y group
  - `vz0..vz4`: base="v", component="z" → vel_z group

But wait — each group has 5 fields. `<4 x float>` holds 4. The 5th stays scalar.
Better: group ALL 15 velocity fields (`vx0..vz4`) into `<4 x float>` phis with
some pairs and a remainder.

Actually, the simplest approach: group `{vx0, vx1, vx2, vx3}` into `<4 x float>`.
`vx4` stays as a scalar float phi. Same for `vy`, `vz`, `bx`, `by`, `bz`.

That's 6 vector phis + 6 scalar phis = 12 values, down from 30. Fits in 16 XMM
regs with 4 spare.

#### 2.2.2 Phi Emission Change

In `emit_countable_setup_phis_and_header`, instead of:
```llvm
%phi_vx0 = phi float [%init_vx0, %pre], [%be_vx0, %latch]
%phi_vx1 = phi float [%init_vx1, %pre], [%be_vx1, %latch]
...
```

Emit:
```llvm
%phi_vx = phi <4 x float> [%init_vx_v4, %pre], [%be_vx_v4, %latch]
%phi_vx4 = phi float [%init_vx4, %pre], [%be_vx4, %latch]
; ... same for vy, vz, bx, by, bz ...
```

The `init` must construct `<4 x float>` from the 4 initial scalar loads.

#### 2.2.3 Body Access Change

In `phi_regs_to_ssa_old`, instead of:
```llvm
ssa_old_float_regs["vx0"] = "%phi_vx0"
```

Need:
```llvm
%vx0 = extractelement <4 x float> %phi_vx, i32 0
ssa_old_float_regs["vx0"] = "%vx0"
```

And for stores, instead of `pending_phi_native_backedge["vx0"] = %val`:
```llvm
%be_vx = insertelement <4 x float> %phi_vx, float %new_vx0, i32 0
```

The backedge for the vector phi is the updated vector after the last component
insertion within the body.

#### 2.2.4 Latch Change

Instead of individual backedges for each component:
```llvm
be_vx0 = add i64 0, %t_vx0
```

Use vector backedge:
```llvm
be_vx = %be_vx    ; the accumulated insertvalue chain from the body
```

## 3. Implementation Plan

### Phase 1: Field Grouping Analysis (loop_engine.rs)

Add a function `build_vector_phi_groups` that:
1. Scans `field_index_map` for fields with pattern `[a-z][a-z][0-9]+`
2. Groups by base+component: all `vx*` together, all `vy*`, etc.
3. For groups of 4+: creates a `<4 x float>` phi, remaining 1-3 as scalar
4. Returns a `Vec<VectorPhiGroup>` describing the mapping

### Phase 2: Phi Emission Change

Modify `emit_countable_setup_phis_and_header` to:
1. Call `build_vector_phi_groups`
2. Emit `<4 x float>` phis for grouped fields
3. Emit scalar phis for non-grouped fields

### Phase 3: Body Access Change

Modify `phi_regs_to_ssa_old` to:
1. For vector group members, emit `extractelement` and insert the result
   into `ssa_old_float_regs`
2. For scalar phis, keep existing behavior

### Phase 4: Store + Backedge Change

Modify `emit_memory_field_store` and `emit_countable_latch` to:
1. When storing to a field in a vector group, accumulate via `insertelement`
2. The vector backedge is the accumulated vector from all component inserts
3. The latch emits the vector backedge as the phi backedge

### Phase 5: Cleanup & Testing

1. Clear vector group state after body emission
2. `cargo test --lib` (all 1398+ tests pass)
3. `benchmarks/build_and_bench.sh --runtime` (nbody_sqrt from 1.25x to ≤1.05x)

## 4. Files to Modify

| File | Change |
|------|--------|
| `src/backend/llvm/loop_engine.rs` | Field grouping, phi emission, body access, latch |
| `src/backend/llvm/context.rs` | Vector group state in FunctionContext |
| `src/backend/llvm/emit_stmt.rs` | Store handling for vector group fields |

## 5. Edge Cases

- **Groups of 5**: First 4 get vector phi, 5th stays scalar
- **Non-matching names**: `count`, `bound`, `seed` stay as scalar phis
- **Cross-chunk groups**: A group may span chunk boundaries; the vector phi
  is independent of chunk allocas
- **Parallel-safe mode**: Vector phis don't affect parallel-safety — individual
  component access still uses ssa_old values (all reads use old phi values)
