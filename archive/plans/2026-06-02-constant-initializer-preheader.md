# Constant-Initializer Preheader — Unlock GVN Float Propagation (2026-06-02)

## Problem

`float_math` (x0=x1=x2=0) runs 0.43s vs C's 0.04s (7.42× gap). C eliminates
the matrix multiply entirely by proving x0/x1/x2 are always 0.0. Despite
`@global_state internal`, non-volatile init, and phi-preheader + extractvalue
counter, LLVM's GVN cannot fold the zero matrix.

## Root Cause

The loop preheader builds the initial `%State` struct from **loads from
`@global_state`**, not from known constants:

```llvm
; opaque — GVN cannot see through
%ssa_init.unpack0 = load float, ptr @global_state
%1 = insertvalue %State %0, float %ssa_init.unpack0, 0
```

LLVM's GVN cannot deduce `load float, ptr @global_state` returns `0.0` even
though the global is `internal zeroinitializer`. The phi carries an opaque
value → GVN gives up → entire matrix computation stays.

Compare C's IR (clang -O3): stack locals promoted to scalar phis with
**literal initializer**:

```llvm
phi float [ %nx0, %body ], [ 0.0, %entry ]  ; constant in phi
```

C's GVN sees `0.0` in the phi, proves recurrence produces `0.0`, eliminates
the matrix.

## Fix

In `emit_folded_loop` SSA mode, build the preheader `%State` from known
constant initializers (`self.field_initializers`) instead of loading every
field from `@global_state`:

**Before:**
```rust
writeln!(out, "  {} = load %State, %State* @global_state, align 8", init_reg).ok();
```

**After:** Iterate `self.field_initializers` sorted by index. For each field:
- **Float**: emit `bitcast i32 <hex_of_value> to float` + `insertvalue`
- **Integer**: emit `insertvalue %State ..., i64 <N>, idx`
- **Bool**: emit `insertvalue %State ..., i8 <0|1>, idx`
- **Runtime expression** (e.g. `__get_env_int`): emit GEP + load from `@global_state` + `insertvalue`

This produces IR where the phi preheader carries known constants:

```llvm
phi_pre:
  %fbc0 = bitcast i32 0 to float           ; 0.0 constant
  %fiv0 = insertvalue %State zeroinitializer, float %fbc0, 0
  %fbc1 = bitcast i32 0 to float           ; 0.0 constant
  %fiv1 = insertvalue %State %fiv0, float %fbc1, 1
  ... 12 float fields as constants ...
  %gepN = getelementptr ...
  %ldN = load i64, i64* %gepN              ; total (runtime)
  %init_state = insertvalue %State %fiv11, i64 %ldN, 13
  br label %hdr

hdr:
  %ssa_phi = phi %State [ %backedge, %body ], [ %init_state, %phi_pre ]
```

GVN sees `0.0` in the phi initializer → SCCP propagates → `fmul 0.0, 0.0` →
folded → entire matrix eliminated.

## Expected Impact

| Benchmark | Before | After | C | Ratio |
|-----------|--------|-------|---|-------|
| float_math (original) | 0.4297s | **~0.04s** | 0.0579s | ~tie |
| float_math_nonzero | 0.4411s | ~0.17s? | 0.1676s | ~2.6× (remainder: NaN overhead) |
| const_heavy | 0.0029s | 0.0029s | 0.0436s | Briv wins |

## Implementation

**File**: `src/backend/llvm.rs`, function `emit_folded_loop`, SSA mode branch
(lines ~2937-2955).

1. After bound-total load, replace `load %State, %State* @global_state` with
   per-field insertvalue loop over `self.field_initializers`.
2. Use `float_to_llvm_hex()` for float constants.
3. Fall back to GEP + load for non-constant initializers.
4. Assign the final assembled register to `init_reg`.
5. `init_state()` call becomes DCE fodder — keep it for non-folded paths.

## Timestamp

2026-06-02 16:30 UTC — Discovery and fix specification.
