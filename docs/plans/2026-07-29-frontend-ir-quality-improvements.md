# Frontend IR Quality Improvements

**Date:** 2026-07-29
**Author:** Agent (investigation session)
**Context:** nbody_newton benchmark investigation revealed four opportunities to improve LLVM IR quality at the front-end level, without changing the backend's codegen strategy.

## Background

The nbody_newton benchmark (31 float state fields, Newton's method sqrt, 5-body gravity simulation) regressed from 0.75× C (Era-5, commit `8a827db`) to 1.23× C (current). Investigation traced the regression to three causes:
1. Vector phi groups disabled (dead code since Phase 4 refactoring)
2. Naming-based grouping replaced by SLP-isomorphism grouping
3. SLP hazard gating removed (hazard.rs deleted)

However, attempts to restore these features made performance **worse** (1.63× C with phi-capping). The correct fix is not at the codegen level but at the IR quality level: **make the front-end emit cleaner IR that LLVM can optimize naturally.**

## Plan Directives Compliance

| Directive | How this plan follows it |
|-----------|--------------------------|
| **Flat control flow** | Each improvement is a self-contained change in 1-2 files |
| **Comment the code** | Every modified site gets `// 2026-07-29: <why>` rationale |
| **Update all examples** | No syntax changes — no example updates needed |
| **Documentation is code** | This plan document; update docs/architecture/ where relevant |
| **Behavioral tests** | New tests for float constant emission, LICM hoisting correctness |

## Improvement 1: Float Constant Emission

**File:** `src/backend/llvm/emit_expr.rs`, lines 52-74 (the `Expr::Float` handler)

### Current behavior

When the compiler encounters a float literal like `0.5f32`, it emits three LLVM instructions:

```llvm
%t330 = add i32 0, 1056964608           ; 1056964608 = 0x3F000000 = float 0.5f32
%t331 = bitcast i32 %t330 to float       ; reinterpret bits as float
%t329 = fadd float 0.0, %t331            ; produce float-typed register v
```

**Why three instructions?** (from the comment at line 53-58)
- `add i32 0, <bits>` — LLVM's verifier rejects high-precision float literals like `float 0.001660076642744037` because the string has more significant digits than f32 can represent. The workaround is to emit the hex i32 bit pattern and bitcast it.
- `bitcast i32 <bits> to float` — reinterprets the i32 bits as float
- `fadd float 0.0, <result>` — the variable `v` (the output register name) was already assigned by `gen_reg()`, but the bitcast used a different register (`flt_reg`). The `fadd` copies the value to `v`.

### Fix

Replace the three-instruction sequence with a single bitcast:

```llvm
%t329 = bitcast i32 1056964608 to float
```

**Code change:**

```rust
Expr::Float(f) => {
    // 2026-07-29: Direct bitcast from i32 hex bits to float.
    // The i32 literal is accepted directly by LLVM IR —
    // no need for an add + fadd wrapper.
    // The raw bits avoid LLVM's verifier rejecting high-precision
    // float literals like "0.001660076642744037" as f32.
    let h = crate::backend::llvm::float_to_llvm_hex(*f);
    writeln!(out, "{}{} = bitcast i32 {} to float", indent, v, h).ok();
    TypedRegister {
        name: v.to_string(),
        ty: Type::float(),
    }
}
```

**Verification:**
1. `cargo test --lib` — all tests pass
2. Build any benchmark: grep for `add i32 0, 10` pattern — should be 0 occurrences
3. nbody_newton: count "add i32" lines in `.ll` — should drop by ~1052
4. Run `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks MATCH

**Risk:** Very low. `bitcast i32 <constant> to float` is standard LLVM IR. The `float_to_llvm_hex` function returns the correct i32 bit pattern.

---

## Improvement 2: Separate `@init_state` Function

**Files:**
- `src/backend/llvm/emit_toplevel.rs` — emit `@init_state` before `@main`
- `src/backend/llvm/mod.rs` — dispatch decision for separate init

### Current behavior

The compiler emits a single `@main()` function that does everything:
1. Alloca `%State`
2. Compute initial values and store to `%State`
3. Run the hot loop (phi nodes, backedge, etc.)
4. Post-loop prints
5. `ret i32 0`

This puts init code (which executes once) in the same function as the hot loop (which executes millions of times). The init code's register usage competes with the loop's register usage during LLVM's register allocation, because the allocator sees the entire function at once.

### Era-5 behavior

Era-5 emitted three functions:
1. `@init_state(ptr %state)` — computes initial values, stores to state
2. `@txn_simulate(ptr %state)` — the hot loop (with vector phis)
3. `@main()` — calls `@init_state`, then the loop, then post-loop prints

This separation:
- Keeps init registers separate from loop registers
- Allows different LLVM attributes on each function
- Enables better SROA for the loop function (no alloca in the loop function — the state is passed as a pointer argument)

### Implementation

In `emit_toplevel.rs` (around line 2300-2400, where `@main` is emitted):

1. Before emitting `@main`, check if this txn has a separate state initialization (most do — the init block in the node declaration).

2. If yes, emit:

```llvm
define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
entry:
  ; ... init stores ...
  ret void
}
```

3. In `@main`, replace the inline init with:

```llvm
define i32 @main() local_unnamed_addr #0 {
entry:
  %state = alloca %State, align 8
  call void @init_state(ptr %state)
  ; ... hot loop ...
  ret i32 0
}
```

**Verification:**
1. `cargo test --lib` — all tests pass
2. Check `.ll` output: `define void @init_state(...` must be present
3. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks MATCH
4. Compare nbody_newton `.text` size before/after (should be smaller — loop function is smaller)
5. Compare nbody_newton runtime (should be closer to Era-5's 0.81× C)

**Risk:** Moderate. Changes the function signature and call structure of `@main`. Must verify that:
- The init function's state stores are visible to the main function (LLVM's `memory(argmem: readwrite)` attribute handles this)
- The init function is `alwaysinline` or `noinline` depending on trade-off (inline for small states, noinline for large states)

---

## Improvement 3: AoS→SoA Field Reorder Pass

**Files:**
- `src/analysis/soa_reorder.rs` (new module)
- `src/backend/llvm/mod.rs` — call the reorder pass in `generate()`

### Why

The state field index assignment (`build_field_index` at `mod.rs:3614`) assigns indices in **source declaration order**. If the source declares fields in per-body (AoS) order:

```brief
let bx0: Float32; let by0: Float32; let bz0: Float32;
let vx0: Float32; let vy0: Float32; let vz0: Float32;
let bx1: Float32; let by1: Float32; ...
```

The state ends up with the same AoS layout:
```
bx0@2, by0@3, bz0@4, vx0@5, vy0@6, vz0@7, bx1@8, ...
```

Same-component fields (all bx) have non-consecutive indices (gap of 6). This prevents the index-run grouping from forming `<4 x float>` vector phis.

If the source declared in SoA order:
```brief
let bx0; let bx1; let bx2; let bx3; let bx4;
let by0; let by1; ...
```

The state would be SoA:
```
bx0@2, bx1@3, bx2@4, bx3@5, bx4@6, ...
```

Same-component fields are consecutive → vector phi groups form naturally.

### The Reorder Pass

Instead of changing source code, a pre-codegen analysis pass reorders `field_index_map` and `field_types` before any backend code runs.

**Algorithm:**

Step 1: **Partition** — Group state fields by LLVM type (float, double, i64). Only same-type fields can form vector groups.

Step 2: **Detect AoS clusters** — Within each type partition, look for fields whose names share a common prefix followed by a numeric suffix (e.g., `bx0`, `bx1`, `bx2`). These are "component families."

Step 3: **Prove independence** — For each component family `F = {bx0, bx1, ..., bxN}`:
- For each family member `bx_i`, scan its RHS expression in the loop body
- Check that `bx_i`'s RHS does NOT reference any `bx_j` for j ≠ i (even transitively through let-bindings)
- If ANY cross-reference exists, the family cannot be reordered

Step 4: **Verify isomorphism** — Check that all members of a family have isomorphic update expressions (same binary-op tree, differing only in leaf variable names). Non-isomorphic updates cannot form vector groups.

Step 5: **Reorder** — If all checks pass, assign consecutive indices to each component family in SoA order:
```
bx0→2, bx1→3, bx2→4, bx3→5, bx4→6,
by0→7, by1→8, ...
```

Step 6: **Update dependent data structures** — Remap:
- `field_index_map`
- `field_types` (reorder to match new indices)
- `field_brief_types` (same reorder)
- `idx_to_field_name` (regenerate from new `field_index_map`)
- Analysis results that used field indices (dependence graph, channel map)

**Where to call it:** In `generate()` at `mod.rs:1784`, after `build_field_index` and before any code generation.

**Safety:** The independence proof is conservative. If any `bx_i` reads `bx_j` (even through a let-binding chain), the reorder is blocked. This ensures correctness for ALL programs, not just nbody.

**Verification:**
1. `cargo test --lib` — all tests pass
2. Create a test case: AoS-declared nbody state → verify `field_index_map` has SoA ordering after the pass
3. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks MATCH
4. Compare `.ll` output: same-component fields must have consecutive indices
5. For nbody: compare phi structure — should show 6 `<4 x float>` groups

**Risk:** Moderate-high. The reorder changes the state layout that codegen depends on. Every codegen pass that uses `field_index_map` must see the post-reorder indices. The key insight: **analysis passes (pre-codegen) operate on field names, not indices.** Only codegen (post-reorder) uses indices for GEP operations, which are correct after reorder.

---

## Improvement 4: Brief-Level LICM (Loop-Invariant Code Motion)

**Files:**
- `src/analysis/licm.rs` (new module)
- `src/backend/llvm/mod.rs` — call LICM pass before body emission
- `src/backend/llvm/emit_expr.rs` — no changes (hoisted expressions referenced by name)

### Why

In the nbody hot loop, many expressions are recomputed every iteration but never change:

```brief
let dist01a: Float32 = dsq01 * 0.5f32;        // 0.5f32 is constant
let mag01: Float32 = dt / (dsq01 * dist01);   // dt is constant (const, never changes)
```

`0.5f32` and `dt` are loop-invariant. Yet the compiler emits fresh `bitcast i32 1056964608 to float` for every `0.5f32` in every iteration. LLVM's LICM may hoist these, but:
1. The `0.5f32` constant emission was fixed in Improvement 1
2. For `dt * m1` patterns, LLVM LICM may not hoist through `memory(readwrite)` function boundaries
3. Brief-level LICM runs BEFORE codegen, so the backend never sees the redundant computation

### Algorithm

A classic loop-invariant code motion pass on the transaction body's `Statement` vector:

**Input:** `body: &[Statement]` (the loop body)
**Output:** `(hoisted: Vec<Statement>, body: Vec<Statement>)` — split into pre-loop and in-loop

Step 1: **Mark loop-invariant let-bindings** — for each `Statement::Let { name, expr, ... }`:
- If `expr` references only:
  - Constants (`Expr::Decimal`, `Expr::Float`, `Expr::Bool`)
  - Other loop-invariant let-bindings
  - State fields that are never written (read-only state)
- Then mark this let-binding as loop-invariant

Step 2: **Fixed-point iteration** — repeat Step 1 until no new loop-invariants are found. A let-binding marked invariant in iteration N may make other let-bindings invariant in iteration N+1.

Step 3: **Collect invariannts** — build a set of all named expressions that are safe to hoist.

Step 4: **Hoist** — move all invariant let-bindings to a `pre_body` vector that will be emitted before the loop.

**Safety check:** A let-binding is NOT loop-invariant if:
- It references a state field that is written in the loop body
- It calls a side-effecting intrinsic (`PrintInt#`, `Malloc#`, etc.)
- It references another let-binding that is not loop-invariant

**Example:**

Before LICM:
```brief
txn advance [count < N][count == N] {
    let dt: Float32 = 0.01f32;       // loop-invariant
    let step: Float32 = dt * 0.5f32; // loop-invariant (dt constant)
    let mag: Float32 = dt / (dsq * dist); // NOT invariant (dsq changes)
    ...
};
```

After LICM:
```brief
txn advance [count < N][count == N] {
    term {
        let dt: Float32 = 0.01f32;       // hoisted before loop entry
        let step: Float32 = dt * 0.5f32; // hoisted before loop entry
    };
    // body no longer has dt or step — they're resolved
    // to the pre-loop hoisted registers
    let mag: Float32 = dt / (dsq * dist); // still in loop
    ...
};
```

**Implementation location:** In `mod.rs`, inside `emit_transaction`, before the dispatch that chooses VectorPhi/InlineSsa/PerFieldPhi. The hoisted let-bindings are emitted as `let` statements before the loop entry point.

**Verification:**
1. `cargo test --lib` — all tests pass
2. Check `.ll` output: loop-invariant expressions should appear outside the loop body
3. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks MATCH
4. For nbody: count `fadd float 0.0, ...` inside the loop body — should be fewer

**Risk:** Low. This is a pure front-end transformation. The hoisted expressions are evaluated exactly once before the loop, producing the same values. No codegen changes needed.

---

## Implementation Order

| Priority | Improvement | Effort | Risk | Expected Impact |
|----------|-------------|--------|------|-----------------|
| **1** | Float constant emission | 5 min | Near-zero | Removes ~2100 IR instructions from nbody |
| **2** | Brief-level LICM | 2-3 hours | Low | Removes redundant loop-body computation |
| **3** | Separate `@init_state` | 2-4 hours | Moderate | Shrinks hot loop register pressure |
| **4** | AoS→SoA reorder pass | 4-6 hours | Moderate-high | Enables `<4 x float>` vector groups automatically |

Each improvement is independently verifiable and can be committed separately. No improvement depends on another.

## Benchmark Comparison

After all 4 improvements, expect nbody_newton to approach Era-5's 0.75-0.85× C ratio:

| Stage | Expected ratio vs C | Notes |
|-------|:---:|-------|
| Current baseline | 1.23× | No changes |
| + #1 (const emission) | ~1.20× | ~3% from cleaner IR |
| + #4 (LICM) | ~1.15× | ~5% from removing redundant computation |
| + #2 (separate init) | ~0.95× | ~20% from reduced loop register pressure |
| + #3 (SoA reorder) | ~0.80× | ~15% from vector phi groups |

Total expected improvement: 1.23× → ~0.80× C (recovering Era-5 performance).
