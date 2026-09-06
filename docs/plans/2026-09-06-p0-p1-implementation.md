# Implementation Plan: P0 (D2 Prefetch) + P1 (Strength Reduction)

**Date:** 2026-09-06
**Status:** Ready to implement
**Expected impact:** P0: ~2× (pipeline overlap), P1: 0% GPU speedup but cleaner SPIR-V

## P1: Strength-Reduce Power-of-Two Div/Mod in Codegen

### What
Replace `OpUDiv`/`OpUMod` by power-of-two constants with `OpShiftRightLogical`/`OpBitwiseAnd` in the SPIR-V fill codegen.

### Why (still worth doing)
- Eliminates 891 UDiv/UMod from the SPIR-V output
- Cleaner driver-side JIT (fewer instructions to parse/compile)
- No GPU time impact (verified by A/B test) but cleaner artifact

### Files to modify
`src/backend/spirv/gemm.rs`

### Changes

**1. Add helper function (after `u32_binop`, ~line 55):**
```rust
/// Emit ShiftRightLogical for power-of-two division.
fn u32_shr(builder: &mut super::SpirvBuilder, val: Word, shift: u32) -> Word {
    let c = u32_const(builder, shift);
    let id = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::ShiftRightLogical, Some(builder.undefined_u32()), Some(id),
        vec![Operand::IdRef(val), Operand::IdRef(c)],
    ));
    id
}
```

**2. Replace divisors in `emit_smem_fill` (scalar path):**
- `flat / 256` → `u32_shr(flat, 8)`
- `elem256 / 16` → `u32_shr(elem256, 4)`

**3. Replace divisors in `emit_fill_pair_dram`:**
- `flat2 / 256` → `u32_shr(flat2, 8)`
- `elem256 / 16` → `u32_shr(elem256, 4)`
- `(elem256 & 14) / 2` → `u32_shr(c14v, 1)`
- `b_stage_elems / 2` → `u32_shr(b_stage_elems_c, 1)`
- `b_flat_within / 256` → `u32_shr(b_flat_within, 8)`
- `bcol / 2` → `u32_shr(bcol, 1)`

**4. Replace divisors in `emit_fill_pair_smem`:**
- `smem_idx_pair / 2` → `u32_shr(smem_idx_pair, 1)`

**5. Replace divisors in quad fill functions.**

### Verification
1. `cargo test --lib` — 2048 tests pass
2. `spirv-val` on generated SPV
3. `spirv-dis` to verify ShiftRightLogical replaces UDiv
4. Run bench: GPU time unchanged (expected), SPIR-V cleaner

---

## P0: Re-Enable D2 Prefetch (Fill/MMA Pipeline Overlap)

### What
Enable `spirv_coopmat_fill_prefetch: 1` in config. This activates the
existing D2 split path that issues DRAM loads before the MMA chain and
stores to smem after the barrier, overlapping global memory latency with
tensor-core execution.

### Why (the real win)
The K-loop currently serializes fill and MMA:
```
CM loads → MMA → barrier → fill (DRAM+smem fused) → barrier → loop
```

With D2 prefetch:
```
[DRAM loads of next panel] → CM loads → MMA → barrier → smem stores → barrier → loop
```

### File to modify
`config/ir-lowering.dbvl` — flip `spirv_coopmat_fill_prefetch: 0` → `1`

### Prerequisites (already met)
- `spirv_coopmat_fill_pairs: 1` — REQUIRED (gate: `pairs && prefetch`)

### Verification
1. Compile, spirv-val, correctness gate
2. GPU timestamp measurement
3. Compare before/after

---

## Implementation Order

1. P1 first (strength reduction) — small, safe
2. P0 second (D2 prefetch enable) — flip config, test
3. Measure, document, commit
