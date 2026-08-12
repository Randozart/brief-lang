# Vector Phi Emission for Register Pressure Reduction

Date: 2026-07-05
Status: Implementation plan
Target: Reduce nbody_sqrt from 1.25x to ≤1.05x

## 1. Problem

nbody_sqrt runs at **1.25x vs C** (3.69s vs 2.93s). Root cause: **32 scalar
float phi nodes → 16 register spills/iteration** because x86 has only 16 XMM
registers. Each spill costs ~3 cycles×50M iterations = ~0.69s, accounting for
97% of the 0.76s gap.

C's clang avoids this by using arrays (`float vx[5]`) which LLVM's SROA
promotes to `<4 x float>` vector phis.  Briev uses flat fields (`vx0..vx4`)
that stay as individual scalar phis.

**The fix**: Group fields (e.g., `vx0..vx3`) into `<4 x float>` vector phis.
From 32 scalar phis → ~8 vector phis, fitting in 16 XMM registers.

## 2. Design: Grouped Vector Phis

### 2.1 Field Groups

Detect and group fields matching pattern `[a-z][a-z][0-9]+`:

| Group key | Fields | Phi type | Registers saved |
|-----------|--------|----------|-----------------|
| `vx` | vx0..vx3 | `<4 x float>` | 3 |
| `vy` | vy0..vy3 | `<4 x float>` | 3 |
| `vz` | vz0..vz3 | `<4 x float>` | 3 |
| `bx` | bx0..bx3 | `<4 x float>` | 3 |
| `by` | by0..by3 | `<4 x float>` | 3 |
| `bz` | bz0..bz3 | `<4 x float>` | 3 |
| Remainder | vx4..bz4, count, bound | scalar | — |
| **Total** | **32→14 phis** | | **18 regs saved** |

### 2.2 IR Structure

**Before (scalar phis):**
```llvm
%phi_vx0 = phi float [%init_vx0, %pre], [%be_vx0, %latch]
%phi_vx1 = phi float [%init_vx1, %pre], [%be_vx1, %latch]
%phi_vx2 = phi float [%init_vx2, %pre], [%be_vx2, %latch]
%phi_vx3 = phi float [%init_vx3, %pre], [%be_vx3, %latch]
```

**After (vector phis):**
```llvm
; Initial vector construction
%ivx_v4 = insertelement <4 x float> undef, float %init_vx0, i32 0
%ivx_v4 = insertelement <4 x float> %ivx_v4, float %init_vx1, i32 1
%ivx_v4 = insertelement <4 x float> %ivx_v4, float %init_vx2, i32 2
%ivx_v4 = insertelement <4 x float> %ivx_v4, float %init_vx3, i32 3

; Vector phi at loop header
%phi_vx_v4 = phi <4 x float> [%ivx_v4, %pre_phi], [%be_vx_v4, %latch]

; Body reads via extractelement
%vx0_e = extractelement <4 x float> %phi_vx_v4, i32 0
%vx1_e = extractelement <4 x float> %phi_vx_v4, i32 1
%vx2_e = extractelement <4 x float> %phi_vx_v4, i32 2
%vx3_e = extractelement <4 x float> %phi_vx_v4, i32 3

; Body stores via insertelement (chained by component order)
%t0 = insertelement <4 x float> %phi_vx_v4, float %new_vx0, i32 0
%t1 = insertelement <4 x float> %t0, float %new_vx1, i32 1
%t2 = insertelement <4 x float> %t1, float %new_vx2, i32 2
%t3 = insertelement <4 x float> %t2, float %new_vx3, i32 3

; Latch: single vector backedge
%be_vx_v4 = %t3     ; identity — accumulated vector
```

### 2.3 No body stores (Path A preserved)

Vector phi groups use `insertelement` for the backedge, NOT memory stores.
Path A (zero memory traffic in hot loop) is preserved for ALL fields.

The only exception: counter field (i64) still uses `pending_phi_native_backedge`
and the standard latch backedge for i64. Counter remains scalar.

## 3. Implementation Steps

### Step 1: `build_vector_phi_groups` — detection

**File:** `loop_engine.rs`

Scans `field_index_map` for fields matching `[a-z][a-z][0-9]+`. Groups of
4 same-prefix fields become vector groups. Returns `HashMap<String, Vec<String>>`
mapping vector phi register → field name list.

Already partially implemented. Needs to store results in `self.fun.vector_phi_groups`.

### Step 2: `emit_countable_setup_phis_and_header` — vector phi emission

**File:** `loop_engine.rs`

After scalar phi setup, emit vector phis for each group:

```
%ivx_v4 = insertelement <4 x float> undef, float %init_vx0, i32 0
... (3 more insertelements)
%phi_vx_v4 = phi <4 x float> [%ivx_v4, %pre_phi], [%be_vx_v4, %latch]
```

Register mappings:
- `phi_field_regs["vx0"]` = `"%phi_vx_v4"` (same for vx1..vx3)
- `backedge_field_regs["vx0"]` = `"%be_vx_v4"` (same for vx1..vx3)

Scalar fields unchanged. Counter unchanged.

### Step 3: `phi_regs_to_ssa_old` — element extraction

**File:** `loop_engine.rs`

For each field in a vector group, emit `extractelement` and use the
result as the ssa_old value. For non-grouped fields, use the phi register
directly (unchanged).

```rust
for (name, phi_reg) in &phi_field_regs {
    if is_in_vector_group(name) {
        let idx = component_index_in_group(name);
        let ext = format!("%{}_e{}", &phi_reg[1..], idx);
        emit out: extractelement <4 x float> %phi_reg, i32 idx → %ext
        ssa_old_float_regs.insert(name.clone(), ext);
    } else {
        // existing: direct phi register
        ssa_old_{float/int}_regs.insert(...);
    }
}
```

### Step 4: `emit_memory_field_store` — insertelement for backedge

**File:** `emit_stmt.rs`

When the stored field is in a vector group:

1. Get current vector value: `vector_phi_current["%phi_vx_v4"]`
   (initialized to `%phi_vx_v4` at body start, updated after each insertelement)
2. Emit `%new_vec = insertelement <4 x float> %cur_vec, float %new_val, i32 N`
3. Update `vector_phi_current["%phi_vx_v4"] = "%new_vec"`
4. Set `pending_phi_backedge[name] = "%new_vec"` (the VECTOR register)
5. Set `pending_phi_native_backedge[name] = "%new_vec"` (the VECTOR register)
6. Skip the GEP store (no memory operation)

For non-grouped fields: existing behavior (store gate, GEP, etc.)

### Step 5: `emit_countable_latch` — skip duplicates for groups

**File:** `loop_engine.rs`

When iterating backedge entries, skip entries whose `be_reg` name has
already been emitted (because 4 group members share `%be_vx_v4`).

Add a `HashSet<String>` tracking emitted backedge register names.

### Step 6: Commit block — handle `<4 x float>` types

**File:** `loop_engine.rs` (lines 1493-1508)

When storing `last_val_temps` in the commit block:

For fields in a vector group, determine the vector type from the group size:
- Size 4 → `<4 x float>` instead of `float`
- The phi register name (e.g., `%phi_vx_v4`) has no type info. Use
  `self.ctx.field_types[idx]` for scalar, and `"<4 x float>"` for vector.

Add a helper: `fn vector_type_for(name: &str, groups: &...) -> Option<String>`

### Step 7: `load_last_val_temps` — extract from vector

**File:** `loop_engine.rs` (line 2247+)

When loading from `last_val_temps`, if the field is in a vector group:

1. Load `<4 x float>` instead of `float`
2. Extract element at the component's position
3. Use the extracted element for `ssa_old_float_regs`

### Step 8: Cleanup

**File:** `loop_engine.rs`

At the end of `emit_countable_main`:
- `self.fun.vector_phi_groups.clear()`
- `self.fun.vector_phi_current.clear()`

## 4. Files to Modify

| File | Changes |
|------|---------|
| `context.rs` | Add `vector_phi_current` field + init |
| `loop_engine.rs` | Steps 1, 2, 3, 5, 6, 7, 8 |
| `emit_stmt.rs` | Step 4 |

## 5. Verification

1. `cargo test --lib` — all 1398+ tests pass
2. `bash benchmarks/build_and_bench.sh --correctness` — nbody_sqrt MATCH
3. `bash benchmarks/build_and_bench.sh --runtime` — nbody_sqrt from 1.25x to ≤1.05x

## 6. Risks

- **LLVM may not SROA-promote `<4 x float>` phis back to scalars** —
  unlikely to be an issue since LLVM handles vector phis efficiently.
- **Instruction overhead from extract/insert** — each insertelement and
  extractelement is ~1 cycle. With 6 groups × (4 extracts + 4 inserts) =
  48 ops per iteration. At 50M iterations, that's 2.4B cycles = ~0.69s.
  This OFFSETS the spill savings! Net impact could be ~0.0x.

Actually wait — let me reconsider. The extracts and inserts add ~0.69s of
overhead. The spills save ~0.69s. Net: ~0.0x improvement. PLUS the vector
phi mechanism adds complexity.

However, LLVM can OPTIMIZE the extract/insert chain. With `-ffast-math`,
LLVM's shuffle optimization can fold consecutive extracts+inserts into
register moves (no latency). And the vector phi means the loop has fewer
live values, which helps the register allocator even if spills aren't
eliminated.

The real win: the register allocator sees 14 live values instead of 32.
With 16 XMM registers, this fits perfectly — no spills, no register
pressure. The extract/insert overhead exists but is much less than spill
loads+stores (1 cycle vs 3-4 cycles).

Revised estimate: extracts+inserts cost ~0.3s per 50M iterations.
Spill savings: ~0.69s. Net: ~0.39s saved → 3.69s → 3.30s = **1.12x**.
Plus baseline improvements: **target: ≤1.10x**.
