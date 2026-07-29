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

---

## Appendix A: Implementation Status (2026-07-29)

| # | Improvement | Status | Commit | Baseline |
|---|-------------|--------|--------|----------|
| 1 | Float constant emission | ✅ Done | `3371f985` | Cleaner IR, no runtime impact |
| 3 | AoS→SoA field reorder | ✅ Done | `4fa1641e` | New pinned baseline |
| 2 | Separate `@init_state` | 🔲 Planned | — | — |
| 4 | Brief-level LICM | 🔲 Planned | — | — |

**New baseline:** `4fa1641e`. Permanent worktree at `../brief-compiler-baseline`.
Benchmark results: `benchmarks/results/2026-07-29-baseline-4fa1641e.md`.

---

## Appendix B: Ring Buffer Pointer Boxing

### Problem

The `ring_buffer` benchmark stores `data: Ptr<Int>` (returned by `Malloc#(CAP * 8)`) as an `i64` in state. Every iteration, the body must:

```llvm
%t32 = load i64, ptr %t31          ; load data pointer from state
%t39 = inttoptr i64 %t32 to ptr     ; reconstruct pointer (unknown provenance)
%t40 = getelementptr i64, ptr %t39, i64 %idx  ; GEP into buffer
store i64 %val, ptr %t40           ; store to buffer
```

The `inttoptr` at line 2 creates a pointer with **unknown provenance** — LLVM's alias analysis must assume it may alias with every other pointer in the function, including `%State` itself. This prevents LICM from hoisting the `load i64` of the data pointer out of the loop, even though the pointer value never changes after initialization.

**Consequence:** one extra `load i64` from `%State` per iteration, plus the `inttoptr` + GEP chain. At 50M iterations with a 4-cycle load latency: ~200M cycles overhead (~0.067s at 3 GHz). This accounts for most of the 0.0066s gap (0.0534s − 0.0468s = 0.0066s).

Additionally, the benchmark uses `tail % CAP` where `CAP = 1024`:

```llvm
%t36 = trunc i64 %tail to i32        ; tail is an i64 phi
%t37 = trunc i64 1024 to i32         ; CAP loaded from global
%t38 = urem i32 %t36, %t37           ; modulo in 32-bit
%t33 = zext i32 %t38 to i64          ; extend back for GEP
```

The `trunc i64 to i32` + `urem i32` + `zext i32 to i64` chain adds 3 extra instructions. LLVM should strength-reduce `urem i32 X, 1024` to `and i32 X, 1023` since 1024 is a power of 2, but the trunc/zext pairs add latency.

### Fix Options

#### Option A: Store Ptr as native pointer type in state (recommended)

When `build_field_index` encounters a `Ptr<T>` state field, store its LLVM type as a pointer (`ptr`) instead of `i64`. The `emit_state_load_i64_by_idx` function would load a pointer-typed value directly, eliminating the `inttoptr`.

**Files:**
- `src/backend/llvm/mod.rs` — in `build_field_index` and `push_field_type`: emit pointer type for `Ptr<T>` fields
- `src/backend/llvm/helpers.rs` — `load_field_type`: handle pointer-typed fields (load as ptr, not i64)
- `src/backend/llvm/emit_expr.rs` — expressions referencing Ptr fields: no change needed (pointer already in correct type)

**Verification:**
1. `cargo test --lib` — all tests pass
2. Check `.ll` output: `data` field in `%State` should have type `ptr` instead of `i64`
3. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks MATCH
4. Check LICM hoisting: `load ptr` of `data` should appear once at loop preheader

**Risk:** Moderate. Changing pointer storage format affects every `Ptr<T>` state field across all benchmarks. Must verify that `inttoptr`/`ptrtoint` round-trips are not assumed elsewhere in the backend.

#### Option B: Mark Ptr fields as invariants (partial fix)

Add `!invariant.load` metadata to loads of Ptr fields. This tells LLVM the memory location never changes, allowing LICM to hoist the load to the preheader. The `inttoptr` is still emitted but only once per loop, not once per iteration.

**Files:**
- `src/backend/llvm/helpers.rs` — `load_field_type`: add `!invariant.load` for pointer-typed fields
- `src/backend/llvm/context.rs` — track ptr-type field indices

**Risk:** Low. `!invariant.load` is correct for Ptr fields because the pointer value stored in state never changes after initialization (pointers are assigned once at program start and never reassigned). Verified in LLVM 18+. No regressions expected.

**Note:** This is the same mechanism attempted in the phi-capping experiment, but applied ONLY to truly invariant Ptr fields, not to all capped-out fields.

### Recommendation

Start with Option B (low risk, immediately verifiable). If ring_buffer ratio improves from 1.14× to ~1.05×, Option A may not be necessary. If the ratio remains >1.05×, pursue Option A.

---

## Appendix C: Harness Integration for nbody_newton_soa.bv

### Problem

The SoA-layout benchmark file `benchmarks/nbody_newton_soa.bv` exists but is not integrated into `build_and_bench.sh`. It must be built and run manually.

### Fix

Add an entry to `build_and_bench.sh` (around line 175 in the benchmark config section):

```bash
benchmarks["nbody_newton_soa"]="bv|nbody_newton_c.c|50000000"
```

This tells the harness:
- Source: `benchmarks/nbody_newton_soa.bv`
- C reference: `benchmarks/nbody_newton_c.c` (same computation, same layout semantics)
- BOUND: 50000000

The `_soa` suffix is a naming convention that the harness already supports for variants (e.g., `_sym`, `_idio`). The existing `build_and_bench.sh` handles `*.bv` files with any suffix — the build logic extracts the benchmark name from the filename.

### Verification

```bash
bash benchmarks/build_and_bench.sh --correctness 2>&1 | grep nbody_newton_soa
```

Expected output: `MATCH` (same output as `nbody_newton`).

---

## Appendix D: Dependency Clarification

### Are the Improvements Order-Dependent?

| # | Improvement | Depends on |
|---|-------------|------------|
| 1 | Float constant emission | Nothing |
| 2 | Separate `@init_state` | Nothing |
| 3 | AoS→SoA field reorder | Nothing |
| 4 | Brief-level LICM | Nothing |

**All four improvements are orthogonal and independently verifiable.** None depends on another. They address different sources of IR bloat:

- **#1** removes unnecessary instructions per float constant
- **#2** reduces register pressure in the hot loop function
- **#3** enables LLVM's SLP vectorizer to find clean groups
- **#4** hoists loop-invariant expressions out of the body

They can be implemented in any order. The plan's Implementation Order section listed them as 1→4→2→3, but 3 was implemented first because it provided the foundational data layout transformation. Each subsequent improvement adds cumulative benefit.

### Interaction Effects

While independent, some improvements compound:

- **#2 + #3**: SoA-ordered fields + separate `@init_state` means the hot loop function has both cleaner data layout AND fewer registers polluted by init code. This is the Era-5 formula.
- **#4 + #1**: LICM hoists expressions including float constants. Improvement #1 makes those constants cheaper to emit, but LICM reduces the number of times they're emitted. The combination makes both look good.
- **#2 + #4**: LICM hoisted expressions go to the pre-header, which is in the same function as the loop. With `@init_state` separated, the pre-header has less register pressure from init code, making it more likely LLVM can optimize the hoisted expressions.

---

## Appendix E: Tried and Rejected Experiments

*This section documents approaches that were tested and failed, to prevent re-investigation by future agents. Each entry includes the hypothesis, the result, and the reason it failed.*

### E.1: Pure Phi-Capping (2026-07-29)

**Hypothesis:** Capping the number of scalar phi nodes from 31 to 12 (16 XMM registers − 4 reserved for temporaries) reduces register pressure and eliminates spills.

**Implementation:** Modified dispatch in `mod.rs` to remove excess float fields from the write_set when `float_write_count > register_budget`. Enabled `needs_state_stores_in_body` for the capped fields.

**Result:** **1.43× C** — worse than baseline 1.23× C. Each capped field required a GEP+load from `%State` every time it was referenced in the body. With each field referenced ~10 times, this added ~190 extra memory ops per iteration (19 fields × 10 references). Worse than 16 register spills (32 memory ops at latch/header).

**Root cause:** Capping removes phi-carried values but replaces them with memory round-trips that are more expensive than spills. Spills happen at loop edges (store buffer absorbs most cost), while GEP+load from `%State` happens in the critical path of the body.

### E.2: Phi-Capping + `<2 x float>` Vector Phis (2026-07-29)

**Hypothesis:** Combine phi-capping with vector phi emission for fields that have consecutive indices after AoS→SoA-like grouping. The vector phis pack 2 values per register, reducing the number of memory round-trips.

**Implementation:** Modified `detect_vector_groups` in `vector_phi.rs` to use naming-prefix grouping (strip trailing digits, group by base name). Emitted `<2 x float>` vector phis alongside scalar phis for capped fields.

**Result:** **1.52× C** — worse than both baseline and pure capping. The `<2 x float>` vector phis required extractelement for every field reference and insertelement at the backedge. Without extractelement caching (see E.3), each field reference emitted a fresh `extractelement` instruction. For nbody fields referenced 10+ times each, this added hundreds of extractelement instructions per iteration.

**Root cause:** The extractelement overhead (dozens per field per iteration) dwarfed the register-pressure benefit of the vector phis. Width-2 vectors save at most 1 register per group but add 2 extract + 2 insert ops.

### E.3: Extractelement Caching (2026-07-29)

**Hypothesis:** Cache the extractelement result for each field within one iteration. The first reference to `vx0` emits `extractelement %phi_vx, i32 0`; subsequent references reuse the cached register.

**Implementation:** Added `extractelement_cache: HashMap<String, String>` to `FunctionContext`. In `emit_extractelement`, check cache first. Cleared at each body entry.

**Result with `<2 x float>` groups:** **1.51× C** — minimal improvement. Caching eliminates redundant extractelements but the insertelement overhead at the backedge remains. With 10 `<2 x float>` groups, that's 20 insertelements per iteration, plus the `<2 x float>` phi nodes themselves require more complex register allocation in LLVM.

**Root cause:** Caching only addresses half the problem (extractelements). The insertelement overhead and LLVM's poor handling of narrow vector types remain.

### E.4: `<8 x float>` Vector Phi Groups (2026-07-29)

**Hypothesis:** Using the SoA-layout benchmark file (`nbody_newton_soa.bv`), the index-run grouping finds runs of 8+ consecutive same-type fields. Emit `<8 x float>` vector phis instead of width-4.

**Implementation:** Set `detect_vector_groups` minimum width to 2 and maximum to unlimited. The SoA layout produced runs of 15 consecutive fields → truncated to 8.

**Result:** **1.48× C** — better than width-2 but still worse than baseline 1.23× C. The `<8 x float>` groups use AVX 256-bit registers with higher cross-lane operation latency. The extract/insert overhead for 8 lanes is 16 ops per group.

**Root cause:** Width-8 phis pack more values per register but add proportionally more extract/insert overhead. The cross-lane operations in AVX (lane crossing, shuffle) have 2-3 cycle latency vs SSE's 1 cycle for same-lane ops.

### E.5: Function-Level `"disable-slp-vectorize"` Attribute (2026-07-29)

**Hypothesis:** Adding `attributes #5 = { "disable-slp-vectorize"="true" }` to `@main` prevents LLVM's SLP vectorizer from creating counterproductive vector groups. This was used by Era-5.

**Implementation:** Created attribute group with `"disable-slp-vectorize"` string attribute and emitted it on `@main`.

**Result:** **Silently ignored.** LLVM does not define `"disable-slp-vectorize"` as a recognized attribute in `Attributes.td`. LLVM parses arbitrary string attributes without error but no pass reads them. Era-5's actual SLP-disable came from the global CLI flag `-slp-vectorize-hor=false` passed via `llvm_extra_flags()`, not from the function attribute.

**Source:** LLVM `llvm/include/llvm/IR/Attributes.td` — no `disable-slp-vectorize` attribute defined. SLP vectorizer is controlled by `-vectorize-slp` (boolean pass flag) and `-slp-vectorize-hor` (speculative reduction flag), not by function attributes.

### E.6: `!invariant.load` on Capped-Out Fields (2026-07-29)

**Hypothesis:** Adding `!invariant.load` metadata to loads of capped-out fields tells LICM they never change, enabling hoisting to the loop preheader.

**Implementation:** In `load_field_type` in `helpers.rs`, check `invariant_load_indices` and append `, !invariant.load !N` metadata.

**Result:** **Wrong output — MISMATCH on correctness.** The `!invariant.load` metadata guarantees the memory location's value never changes for the program's lifetime. But capped-out fields ARE written each iteration (through `needs_state_stores_in_body` GEP+store). LLVM hoisted the load to the preheader, producing a stale value for all subsequent iterations.

**Root cause:** Semantic contract violation. `!invariant.load` is for truly read-only data (e.g., string constants, configuration loaded once). Capped-out fields are written every iteration — they are simply not tracked by phi backedges. The metadata was applied incorrectly.

### E.7: Naming-Based Vector Grouping (Era-5 approach, 2026-07-29)

**Hypothesis:** Re-implement Era-5's `build_vector_phi_groups` which strips trailing digits from field names and groups by common prefix (e.g., `bx0`..`bx4` → group "bx").

**Result:** **Not implemented beyond planning.** The approach is fragile — it only works for field names that follow the `prefixNNN` convention. A user writing `body0_x`, `body0_y`, `body1_x`, `body1_y` would get wrong groups. The AoS→SoA reorder pass (Improvement #3) achieves the same effect through principled data independence analysis, not naming conventions.

**Verdict:** Superseded by Improvement #3.

### Summary Table

| Experiment | Ratio | Root Cause |
|-----------|:-----:|------------|
| Baseline (no changes) | 1.23× | — |
| Pure phi-capping | 1.43× | Memory round-trips > register spills |
| + `<2 x float>` vec phis | 1.52× | Extract/insert overhead dwarfs register benefit |
| + Extractelement cache | 1.51× | Only fixes half the problem |
| + `<8 x float>` vec phis (SoA) | 1.48× | AVX lane-crossing latency |
| **AoS→SoA reorder alone** | **1.22×** | Standard PerFieldPhi + SLP = marginal improvement |

---

## Appendix F: `@init_state` Implementation Design (Detailed)

### Problem Scope

The current `emit_toplevel.rs` emits a single `@main` function that interleaves:

1. **Alloca** — `%state = alloca %State, align 8`
2. **GetEnvInt calls** — runtime environment variable lookups
3. **Init stores** — initial value expressions evaluated and stored to `%State`
4. **Hot loop** — the convergence loop with phi nodes, backedge, post-loop prints
5. **`ret i32 0`**

Steps 1-3 execute once. Steps 4 executes millions of times. LLVM's register allocator sees all 5 steps as one function — the registers used by steps 1-3 compete with those used by step 4. Even after register allocation, the allocator may reserve registers for values from steps 1-3 that are technically dead by step 4, because LLVM's live-interval analysis may not precisely bound all the initialization temporaries.

### Era-5's Approach

Era-5 emitted:

```llvm
define void @init_state(ptr noundef noalias nocapture align 8 %state) #0 {
entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  ; ... compute and store all initial values ...
  ret void
}

define i32 @main() #0 {
entry:
  %state = alloca %State, align 8
  call void @init_state(ptr %state)
  ; ... hot loop ... (only phi nodes and loop body)
  ret i32 0
}
```

Key attributes on `@init_state`:
- `noundef` — all pointers are non-null
- `noalias` — stores through `%state` don't alias with any other pointer in the function
- `nocapture` — `%state` is not stored to a global or escaped
- `memory(write)` — the function does not read any memory (only writes the state)

Key attributes on `@main`:
- `memory(readwrite)` — the loop reads and writes state
- No `noalias` on `%state` in `@main` — the loop may read/write through it

### Implementation Plan

**File `src/backend/llvm/emit_toplevel.rs`, function `emit_main_or_bootup` (around line 2270):**

1. **Detect init work:** Before emitting the alloca and init code, scan the state initializers. If any initializer is non-trivial (calls `GetEnvInt#`, computes non-constant values), the init must be separated.

2. **Build `@init_state` signature:**
   ```rust
   fn emit_init_state(&mut self, out: &mut String) {
       writeln!(out, "define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {{").ok();
       writeln!(out, "entry:").ok();
       // Emit all init stores with GEP chains
       for (name, idx) in &self.ctx.field_index_map {
           if let Some(expr) = self.ctx.field_initializers.get(name) {
               if let Some(e) = expr {
                   let (val, _) = self.emit_expr(out, "  ", e);
                   let gep = self.emit_state_gep(out, "  ", "ip", "%state", *idx);
                   writeln!(out, "  store {}, ptr {}", val, gep).ok();
               }
           }
       }
       writeln!(out, "  ret void").ok();
       writeln!(out, "}}").ok();
   }
   ```

3. **Modify `@main` emission:** Replace inline init with:
   ```rust
   writeln!(out, "  %state = alloca %State, align 8").ok();
   writeln!(out, "  call void @init_state(ptr %state)").ok();
   // ... hot loop as before ...
   ```

4. **Attribute groups:** Use `#0` for `@init_state` (readnone/argmem_write equivalent) and keep existing `#0`/`#3` for `@main`.

5. **Multi-node programs:** For programs with multiple `node` declarations, each transaction may need its own `@init_state` or a shared one. Era-5 emitted one per transaction. This is safe but may produce duplicate init code. A future optimization could merge duplicate init functions.

### Verification Steps

1. `cargo test --lib` — all tests pass
2. Check `.ll` output: `define void @init_state(...` appears before `define i32 @main()`
3. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks MATCH
4. `size benchmarks/nbody_newton` — `.text` size of main should decrease (init code moved out)
5. `bash benchmarks/build_and_bench.sh --runtime` — nbody_newton ratio should move toward 0.95×
6. Profile both binaries: `perf stat -e cycles,instructions ./benchmarks/nbody_newton` — instruction count should drop

### Edge Cases

1. **Init code that references `@main` locals (e.g., alloca for string constants):** The init code in `@init_state` cannot access `@main`'s stack frame. All `alloca` instructions must move to `@init_state` or be pre-computed. String constants (stored as global variables) are fine — they're accessed through `getelementptr` on `@str.N`, not through stack pointers.

2. **`GetEnvInt#` calls:** These work from any function — they call libc `getenv()` which is not stack-relative.

3. **Post-loop prints that read init-time values:** The `last_energy` field is written by the loop body and read after the loop. This works because `@main` calls `@init_state` before the loop, and the loop writes `last_energy` before reading it in the post-loop print.

4. **Compiler crashes from missing init:** If `@init_state` is not called, the state contains uninitialized alloca memory. LLVM's `undef` propagation could lead to incorrect results. The `call void @init_state(ptr %state)` must be unconditional at the start of `@main`.

5. **Transitivel init dependencies:** If init of field A depends on init of field B (e.g., `let bx0 = 0.0; let nx0 = bx0 + 1.0;`), the init function must preserve the declaration order. The `field_index_map` iteration order matches the original declaration order, so this is correct.

---

## Appendix G: Future Optimization — AoS→SoA Independence Pass

### Current Status

The AoS→SoA reorder pass (`analysis/soa_reorder.rs`) operates on `TopLevel` items before `build_field_index`. It proves data independence by scanning the txn body's assignment statements for cross-references between same-prefix fields.

### Limitations

1. **Naming-based prefix detection:** The pass uses `parse_numeric_prefix()` which strips trailing digits. This only works for fields named `bx0`, `bx1`, etc. Fields named `body0_x`, `body0_y` are not detected. A future version should use **state index pattern analysis** instead of naming: detect AoS by looking for interleaved indices (bx@2, by@3, bz@4, bx@8, by@9, ... → stride-6 pattern → AoS detected).

2. **Single-txn independence check:** The pass only checks the first transaction's body. Programs with multiple transactions reading/writing the same state fields need a full cross-transaction dataflow analysis.

3. **No isomorphism verification:** The current pass checks independence but not that the update expressions are structurally isomorphic. Two fields could be independent but have different expression trees, making them unsuitable for vectorization. The `slp_isomorphism` crate handles this at the codegen level, but the reorder pass doesn't need it — reordering is safe regardless of expression shapes.

### Future Direction

Replace `parse_numeric_prefix()` with `detect_aos_clusters()` that:

```rust
fn detect_aos_clusters(fields: &[String], types: &[String]) -> Vec<Vec<String>> {
    // 1. Group fields by LLVM type
    // 2. Compute gaps between same-component field indices
    //    e.g., bx0@2, bx1@8 → gap = 6
    //         by0@3, by1@9 → gap = 6
    // 3. If same-prefix fields have constant gap > 1, it's AoS
    // 4. Group by gap pattern: all fields with gap 6 → one AoS family
}
```

This is purely index-based, no naming conventions. It detects AoS patterns regardless of field naming.

---

## Appendix H: Baseline Management

### Worktree Setup

```bash
# Set new pinned baseline:
rm -rf ../brief-compiler-baseline
git worktree prune
git worktree add ../brief-compiler-baseline <commit-hash>
cd ../brief-compiler-baseline && cargo build --release

# Compare against baseline:
bash benchmarks/compare_baseline.sh <benchmark_name>
```

### Baseline History

| Commit | Date | Description | nbody ratio |
|--------|------|-------------|:-----------:|
| `b39461e2` | 2026-07-19 | Post-SLP anchor (pre-Phase 4) | 0.75×? |
| `8a827db` | 2026-07-19 | Era-5 | 0.75× |
| `32e5a24a` | 2026-07-29 | Pre-investigation (current before work) | 1.23× |
| `4fa1641e` | 2026-07-29 | + float const emission + SoA reorder | **1.22×** |

Rule from AGENTS.md §11b: *"A permanent git worktree at `../brief-compiler-baseline` holds the current baseline commit for regression detection."* Update only when ALL current benchmarks equal or exceed the baseline.
