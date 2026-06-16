# Per-Field Phi Nodes for Folded SSA Path

**Date:** 2026-06-16  
**Status:** Plan — implementation in progress

## Problem

The folded SSA path (`emit_folded_loop` with `use_phi = false`) uses an **alloca slot** to hold the `%State` struct between loop iterations:

```
case_pre:
  ; 17 GEP + load + insertvalue chain → build %State struct
  store %State, %State* %slot_case    ; store to alloca
  br label %case_hdr

case_hdr:
  %load = load %State, %State* %slot_case  ; reload from alloca
  ; 17 extractvalue → get individual fields
  ...
```

This `store %State` / `load %State` round-trip through memory blocks SROA and prevents
LLVM from tracking reductions. It was the state of the art before commit `847e0f9`
(R3: Per-field GEP loops) which fixed `emit_ssa_main` but NOT the folded path.

## Fix

Replace the alloca slot with per-field `phi i64` nodes. Each field gets its own phi:

```llvm
case_hdr:
  %count = phi i64 [ 0, %entry ], [ %count_next, %case_body_end ]
  %seed = phi i64 [ 42, %entry ], [ %ns, %case_body_end ]
  %checksum = phi i64 [ 0, %entry ], [ %nchecksum, %case_body_end ]
  ...
  ; body uses %count, %seed, %checksum directly
```

### Benefits
1. **LLVM tracks reductions**: `%checksum` phi → LLVM identifies reduction → vectorization
2. **No wide SSA register**: each phi is an independent `i64`
3. **No alloca slot**: pure SSA, no memory round-trip
4. **No GEP+load+insertvalue**: fields used directly

### Edge Cases to Handle

| Edge case | Issue | Mitigation |
|---|---|---|
| **Init with getenv_int#** | Field initial value is runtime-determined | Emit runtime computation in entry, phi uses result |
| **Fields not updated body** | If a field is read but not written, phi is unnecessary | Only create phi for fields that are WRITTEN in the body |
| **Read-only fields** | `N` (bound) is constant through the loop | Don't phi — just load once in entry block |
| **Field type float** | Float fields need `phi float` not `phi i64` | Emit `phi float` for float-typed fields |
| **Empty initializer** | Field starts at 0/null | Phi entry value = `add i64 0, 0` or `null` |
| **Auto-generated state fields** | `#!exit` creates synthetic fields | Skip non-body fields (exit condition is tracked separately) |
| **Alloca-based still needed for...** | Truly dynamic control flow | Keep alloca slot as fallback |

## Implementation

### File changes: `src/backend/llvm/loop_engine.rs`

In `emit_folded_loop` (~line 950):

1. **Detect**: Check if we can use per-field phis (not `use_phi`, body is provided)
2. **Emit phi header**: At `case_hdr`, emit one `phi` per written field
3. **Remap field reads**: `pre_load_all_fields` replaced with direct phi references
4. **Emit latch**: At end of body, collect updated field values and branch to `case_hdr`
5. **Fallback**: If per-field phis don't apply, keep old alloca slot path

### Key function: `emit_folded_loop` signature change

Currently:
```rust
fn emit_folded_loop(..., use_phi: bool, body: Option<&[Statement]>, uf: usize, ...)
```

New behavior: when `use_phi = false` and `body.is_some()`, use per-field phis instead
of alloca slot. When `use_phi = true`, use the pure phi-pipeline path (unchanged).

## Verification

1. `cargo test --lib` — 909 tests pass
2. Built fannkuch_redux at BOUND=50M — ratio < 1.10x
3. `opt -pass-remarks-missed=loop-vectorize` no longer reports "value used outside loop"
4. Correctness: BOUND=10 output matches C
