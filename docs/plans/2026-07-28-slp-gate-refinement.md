# SLP Gate Refinement — Remove Stride Gate + Fix Vector Sqrt
## 2026-07-28

## Root Cause Analysis

### The stride gate is the sole blocker of merged groups

The three SLP gates interact in a way that makes one gate cover for another:

| Gate | Logic | Effect |
|------|-------|--------|
| **Stride** | `max_field_stride > 1` → reject | Blocked ALL merged groups. Field indices of merged groups are non-contiguous because the template expression's identifiers span different state fields. The insert-chain cost is identical whether stride is 1 or 100 — this gate was measuring the wrong thing. |
| **Depth×Width** | `depth * width >= 10` | Works correctly: blocks width=3 depth≤3 (kalman's 9<10, nbody's 3<10). Passes width≥4 or depth≥4. Merged width=9 depth=2 passes (18≥10). |
| **Width cap** | `width <= 8` | Blocked width=9+ merged groups. Unneeded if merge step controls width via source group count. |

The stride gate was the ONLY gate blocking merged groups. With it removed:
- Pre-merge width=3 depth=1 groups stay blocked (3<10) — fine, they contribute ~130 of 252 ops
- Pre-merge width=3 depth=3 groups (dsq) stay blocked (9<10) — was contributing ~40 of 252 ops
- **Merged width=9 depth=2 groups (dsq merged 3×3): 18≥10 → pass** — recovers ~80 of 252 ops

### The `Sqrt#` scalar fallback

When an SLP group contains `Sqrt#(dsq)`, the `emit_vector_expr` function hits the `_ =>` catch-all and falls back to per-lane scalar emission:

```
for each lane:
    %sqrt_i = call @__sqrt_f32(float %dsq_i)
    %vec = insertelement <width x float> %vec, %sqrt_i, i
```

This adds width insertelement + width extractelement overhead WITHOUT any vectorization benefit. Fix: emit a vector call instead:

```
%dsq_vec = <build vector>
%sqrt_vec = call <4 x float> @__sqrt_f32(<4 x float> %dsq_vec)
```

LLVM scalarizes the vector call during `opt -O3`, splitting into 4 scalar sqrt calls. The inserts are handled inside the vector codegen pass where LLVM can eliminate them, rather than at the scalar codegen level where they're "baked in."

### Why kalman still stays blocked

kalman's SLP groups are width=3, depth=3. `3×3=9 < 10` → blocked by the existing depth×width gate, unchanged by removing stride.

## Changes

### Change 1: Remove the stride gate entirely

**File:** `src/backend/llvm/loop_engine/counter.rs`

Delete the `collect_field_indices` function and the `stride_ok` check. The `should_vec` logic simplifies to:

```rust
let should_vec = group.width >= 4 && group.width <= self.ctx.max_simd_lanes
    || (group.width >= 3 && group.width <= self.ctx.max_simd_lanes
        && template_expr.map_or(false, |expr| {
            crate::backend::llvm::vector_codegen::tree_depth(expr)
                * group.width >= 10
        }));
```

Also remove `collect_field_indices` (unused) and all `HashMap` imports no longer needed.

### Change 2: Fix vector `Sqrt#` emission

**File:** `src/backend/llvm/vector_codegen.rs`

In `emit_vector_expr`, add a match arm for `Expr::Call` BEFORE the scalar fallback:

```rust
Expr::Call(name, args, _) if name.ends_with('#') && !args.is_empty() => {
    let vec_args: Result<Vec<_>, _> = args.iter()
        .map(|a| emit_vector_expr(backend, out, a, lane_exprs, lane_mappings, width, indent))
        .collect();
    let vec_args = vec_args?;
    let v = backend.fun.next_reg_with_prefix("sv");
    let vec_ty = vector_type_str(&vec_args[0].ty, width);
    writeln!(out, "{}{} = call {} @{}({} {})", indent, v, vec_ty, name, vec_ty, vec_args[0].name).ok();
    Ok(TypedRegister { name: v, ty: vec_args[0].ty })
}
```

### Change 3: Add target-aware width limit

**File:** `src/backend/llvm/context.rs`

Add field:
```rust
pub max_simd_lanes: u32,
```

Initialize to 8 (default for AVX2). Set from target spec capabilities in `mod.rs`.

## Root Cause: Dense Matrix → LLVM Auto-Vectorizer Regression

The kalman 3.5x regression is NOT from our SLP — txn_propagate has 0 SLP groups.
The 84 `<12 x float>` ops in main's `.wbody` hot loop are from **LLVM's
auto-vectorizer**, not our SLP. After inlining the alwaysinline @txn_propagate
into @main, LLVM sees the dense 9-field matrix multiply and creates `<12 x float>`
vectors. This auto-vectorization is supposed to help, but for kalman's 9-field
matrix, it creates register pressure that dominates the ALU gain.

The baseline had `#0 = memory(readwrite)` on the txn, which blocked LLVM's
auto-vectorizer. Our changes (Phase B through E) resulted in `#11 = memory(argmem: readwrite)`,
which TELLS LLVM the function has no pointer aliasing — enabling the aggressive
auto-vectorization that causes the regression.

**Fix:** Force `#0 = memory(readwrite)` when cross-per-field density > 8 and
n_float_fields > 4. This blocks LLVM's auto-vectorizer on dense matrix
computations (kalman) while keeping `#11` for sparse computations (nbody).

The cross-per-field metric: `total_cross_float_ops / float_field_count`.
- Kalman: 84 / 9 = 9.3 → `#0` (dense)
- Nbody: 50 / 30 = 1.7 → `#11` (sparse)
- Float_math: depends on structure

## Fix: Cross-Per-Field Density Check (5 lines)

In `emit_toplevel.rs`, after the outlining analysis selects `txn_attr`:

```rust
// If the txn has dense matrix-like float computation (cross-per-field > 8),
// force #0 = memory(readwrite) to block LLVM's auto-vectorizer.
// This prevents the kalman 3.5x regression where LLVM creates expensive
// <12 x float> vectors that cause register spilling.
if has_ffi_guard {  // would otherwise get #11
    let cross_per_field = compute_cross_per_field(&txn.body, &self.ctx.field_index_map);
    if cross_per_field > 8.0 {
        // Dense matrix — LLVM's auto-vectorizer makes things worse
        txn_attr = "#0".to_string();  
    }
}
```

## Regressions Addressed

| Benchmark | Current | Expected | Fix |
|-----------|---------|----------|-----|
| nbody_sqrt_idio | 0.92x | ~0.75x | Stride gate removed → merged width=9 groups pass. Vector sqrt eliminates scalar fallback overhead. Width=9 depth=2: 18≥10 ✅ |
| nbody_sqrt | 0.96x | ~0.87x | Same — merged groups recovered |
| nbody_newton | 1.08x | ~1.06x | Merged groups partially recovered (depth=2 pass, depth=1 still blocked) |
| kalman_filter_runtime | 1.01x | 1.01x | Unchanged — width=3 depth=3: 9<10 → blocked |
| mandelbrot | 1.00x | 1.00x | Unchanged — `has_lane_dependency` blocks |
| ring_buffer | 1.19x | 1.19x | Unchanged — no SLP groups |

## Verification

1. `cargo test --lib` — all pass
2. `bash benchmarks/build_and_bench.sh --runtime --correctness`
3. Compare nbody_sqrt_idio ratio — expected improvement from 0.92x to ~0.75x
4. No regression in kalman, float_math_nonzero, or mandelbrot

## Commit

```
git commit -m "2026-07-28: Remove stride gate + vector sqrt emission

The stride gate (max_field_stride > 1 → reject) was blocking ALL merged
SLP groups because merged groups reference non-contiguous field indices.
The insert-chain cost is identical regardless of stride — the gate was
measuring the wrong thing. Removed.
    
Also added vector Sqrt# emission to avoid the scalar fallback in
emit_vector_expr, which was adding insertelement/extractelement overhead
without any vectorization benefit.
    
Depth×width >= 10 threshold kept — correctly blocks kalman (3×3=9<10)
while allowing merged width=9 depth=2 groups (18>=10) after stride
gate removal.
"
```
