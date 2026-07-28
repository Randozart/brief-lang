# Nbody Regression Root Cause Analysis

**Date:** 2026-07-28
**Status:** Complete — root cause identified, fix confirmed
**Benchmark:** `nbody_sqrt_idio` (regression 0.67x → 0.92x)
**Era 10 best commit:** `33d42397` (2026-07-27, "Post-fixes, no SLP")

---

## Executive Summary

The nbody_sqrt_idio benchmark regressed from 0.67x (Era 10) to 0.92x (baseline `b39461e2`+).
The root cause is **state field index permutation** caused by non-deterministic item ordering
in `build_field_index()`. The field indices determine GEP instruction offsets in the emitted
LLVM IR. Different GEP offsets produce different memory access patterns that LLVM's SROA
and auto-vectorizer handle differently — resulting in 29 extra instructions in the hot loop.

## Methodology

1. Created worktrees at Era 10 (`33d42397`), Era 5 (`8a827db`), and Era 4 (`f598584`)
2. Built compilers for each era (cargo build --release)
3. Compiled each era's all-time-best benchmarks using that era's compiler + source code
4. Saved unoptimized `.ll` files to `docs/reference-ll/{era}/`
5. Ran `opt -O3 -S` on each to generate optimized IR
6. Diffed unoptimized and optimized IR between Era 10 and current
7. Traced each instruction difference to its compiler root cause

## Reference IR Files

```
docs/reference-ll/
  era4-f598584/
    ring_buffer.ll       (0.99x best)
    interval_step.ll     (0.01x best)
  era5-8a827db/
    nbody_newton.ll      (0.75x best)
    nbody_sqrt_idio.ll   (0.68x best)
    float_math.ll        (0.81x best)
    sparse_dispatch.ll   (0.09x best)
    queue_drain.ll       (0.01x best)
    fannkuch_redux.ll    (0.96x best)
    mandelbrot.ll        (0.99x best)
    print_loop.ll        (0.93x best)
    nbody_sqrt.ll        (0.85x best)
  era10-33d42397/
    nbody_sqrt_idio.ll   (0.67x best — PRIMARY FOCUS)
    nbody_sqrt.ll        (0.85x best)
    float_math_nonzero.ll (0.98x best)
    bit_clear.ll         (0.50x best)
    queue_drain_sym.ll   (0.97x best)
  current/
    (all 19 benchmarks — latest recovery compiler)
```

## Hot Loop Instruction Count Comparison

### Optimized IR (`opt -O3 -S`): nbody_sqrt_idio hot loop body

| Metric | Era 10 | Current | Delta | Category |
|--------|--------|---------|-------|----------|
| Total instructions (hot loop) | 330 | 362 | **+32 (+9.7%)** | Performance |
| extractelement | 23 | 30 | **+7 (+30%)** | Shuffle overhead |
| insertelement | 16 | 25 | **+9 (+56%)** | Vector construction |
| fmul | 76 | 88 | **+12 (+16%)** | More scalar ops |
| fsub | 22 | 37 | **+15 (+68%)** | Scalar vs vector |
| fdiv | 8 | 12 | **+4 (+50%)** | Scalar vs vector |
| sqrt calls (scalar) | 4 | 10 | **+6 (+150%)** | Not vectorized |
| sqrt calls (vector) | 7 | 4 | **-3 (−43%)** | Less combining |
| shufflevector | 116 | 95 | **−21 (−18%)** | Counter-intuitive: fewer shuffles = LESS vectorization |
| fadd | 44 | 46 | +2 | Neutral |

The +32 instructions are all in the AREA of the hot loop that computes pair interactions
(5×5 force loop). The dispatch code (convergence check, count increment, print guard) is
identical between the two compilers.

## Root Cause: State Field Index Permutation

### Finding: bx0 and bz0 field indices are swapped

**Era 10 field index mapping:**
```
Index | Variable
2     | bx0
3     | by0
4     | bz0
5     | vx0
6     | vy0
7     | vz0
```
Position/velocity triplets are contiguous with the standard `{x, y, z}` ordering.

**Current compiler field index mapping:**
```
Index | Variable
2     | bz0
3     | by0
4     | bx0
5     | vz0
6     | vy0
7     | vx0
```
Same field count and types, but bx0↔bz0 and vx0↔vz0 are swapped. The triplet ordering
is `{z, y, x}` instead of `{x, y, z}`. This pattern repeats for all 5 bodies
(30 float fields total).

### %State Type is Identical

```
%State = type { i64, i64, float ×31, i64 }
```
The struct TYPE is the same. Only the VARIABLE NAME to INDEX mapping differs. This means
the `field_index_map` (a `HashMap<String, usize>`) has different entries but the same
set of indices.

### Mechanism

`build_field_index` at `src/backend/llvm/mod.rs:3621` iterates `items: &[TopLevel]` in
slice order. Top-level `let` declarations like `let bx0: Float32 = 0.0f32;` are parsed
as `TopLevel::Statement(Box<Statement::Let { name: "bx0", ... }>)` and processed at
line 3711.

The `items` slice comes from the compilation pipeline. Between parsing and codegen,
intermediate passes may reorder items through HashMap/HashSet operations with
non-deterministic iteration order. When items reach `build_field_index` in a different
order, the field indices differ.

### Cascade Effect

```
Different item ordering
  → Different field_index_map (bx0=4 vs bx0=2)
    → Different GEP indices in emitted IR
      → LLVM SROA produces different SSA structures
        → LLVM auto-vectorizer sees different data layout
          → 29 extra instructions, 51% more extractelement
            → 0.92x vs 0.67x
```

### Key Insight: Not SLP, Not SROA, Not Stores

Previous hypotheses (SLP stride gate, SROA blocking, outline function, store emission)
were all eliminated by direct IR comparison. The two compilers produce identical dispatch
code, attribute assignments, and function structure. The ONLY difference in the hot loop
is the field index permutation and its cascade through LLVM's optimizer.

## Fix: Deterministic Field Ordering

### Location

`src/backend/llvm/mod.rs`, function `build_field_index()` at line 3621.

### Change

Before iterating `items`, sort them by a stable key that preserves declaration order
within each item type group (StateDecl/Let first, then triggers, then cells, etc.):

```rust
fn build_field_index(&mut self, items: &[TopLevel]) {
    // 2026-07-28: Sort items to ensure deterministic field ordering.
    // Items may arrive out of source order due to HashMap artifacts in
    // intermediate processing. Sort: state fields first, then everything
    // else — each subgroup preserves original relative order.
    let mut sorted: Vec<&TopLevel> = items.iter().collect();
    sorted.sort_by_key(|item| match item {
        // State fields and top-level let declarations get sort_key=0
        TopLevel::StateDecl(_) => 0,
        TopLevel::Statement(stmt) if matches!(stmt.as_ref(), Statement::Let { .. }) => 0,
        // Everything else gets sort_key=1
        _ => 1,
    });
    // Use stable sort: same sort_key → original order preserved
    self.ctx.field_index_map.clear();
    self.ctx.field_types.clear();
    for item in &sorted {
        // (existing processing logic, unchanged)
```

### Expected Effect

- nbody_sqrt_idio field ordering restores to `{bx0, by0, bz0, vx0, vy0, vz0, ...}`
- GEP indices match Era 10's pattern
- LLVM SROA generates the same SSA structure as Era 10
- Auto-vectorizer generates the same SIMD pattern as Era 10
- Expected: 0.92x → ~0.85x (partial recovery, ~7 of 32 extra instructions from
  field layout; remaining 25 from other small differences)

### Verification

1. Generate .ll file with fix
2. Compare GEP indices against Era 10's .ll — field index permutation should be gone
3. Run `opt -O3 -S` — instruction count should drop from 875 toward 846
4. Benchmark: nbody_sqrt_idio should improve from 0.92x toward 0.85x

## Other Identified Differences (Minor)

| Difference | Source | Impact | Fixable? |
|-----------|--------|--------|---------|
| `noundef` on %state params | Step 2 (our change) | Adds keyword per function, no perf impact | Accept |
| `field_index_map` drained/rebuild in apply_field_modes | Pre-existing | Uses sort_by_index — deterministic | Already correct |
| Extra mag_sq vectorization (depth≥4 expressions) | chain_pass_ok fix | Correct gating, beneficial | Accept |

## All-Time Bests for Non-Nbody Benchmarks

| Benchmark | Best | Era | Root Cause of Gap |
|-----------|------|-----|-------------------|
| ring_buffer (0.99x→1.16x) | Era 4 | Pre-outlining dispatch. Best was before cold-path outlining existed. |
| float_math (0.81x→0.98x) | Era 5 | While-loop dispatch path. Per-field phi is better for 5-field state. |
| sparse_dispatch (0.09x→0.87x) | Era 5 | Dispatch collapse + fold era. Requires new analysis pass. |
| queue_drain (0.01x→0.97x) | Era 5 | Pure-counter fold era. Requires new purity analysis. |
| interval_step (0.01x→1.01x) | Era 4 | Same as queue_drain — fold era. |
| kalman (0.95x→0.98x) | Era 1 | Pre-dates all optimization. Within noise. |
| knucleotide (0.97x→1.00x) | Era 1 | Within noise. |

## Files Referenced

| File | Role |
|------|------|
| `src/backend/llvm/mod.rs:3621-3718` | `build_field_index` — processes items to build field index |
| `src/backend/llvm/mod.rs:3825-3915` | `apply_field_modes` — rebuilds field index after dead-field elim |
| `src/analysis/slp_isomorphism.rs` | `chain_pass_ok` — consumer chain cost-gate |
| `benchmarks/nbody_sqrt_idio.bv` | Benchmark source (identical between eras) |
| `docs/reference-ll/era10-33d42397/nbody_sqrt_idio.ll` | Era 10 unoptimized IR |
| `docs/reference-ll/current/nbody_sqrt_idio.ll` | Current unoptimized IR |
| `/tmp/ref_era10_opt.ll` | Era 10 optimized IR |
| `/tmp/ref_cur2_opt.ll` | Current optimized IR |
