# Flat Emission Refactoring: Getting Out of LLVM's Way

**Date:** 2026-07-29
**Goal:** Restructure expression emission to produce instruction sequences that
LLVM's SLP vectorizer can optimally merge — then remove all hand-rolled SLP passes.

## Motivation

Our current `emit_expr` processes expression trees recursively, interleaving loads
with arithmetic. For `dx01 = bx0 - bx1`:

```llvm
GEP %state, bx0 → load ; load bx0
GEP %state, bx1 → load ; load bx1
fsub dx01               ; compute
```

Three statements for dx01, dy01, dz01:
```llvm
load bx0, load bx1, fsub dx01
load by0, load by1, fsub dy01
load bz0, load bz1, fsub dz01
```

No two consecutive operations are isomorphic → LLVM's SLP finds nothing.

Target (flat emission):
```llvm
load bx0, load by0, load bz0   ; consecutive loads → SLP vectorizes
load bx1, load by1, load bz1   ; consecutive loads → SLP vectorizes
fsub dx01, dy01, dz01           ; consecutive fsub → SLP merges into <3xf>
fmul ...                        ; consecutive fmul → SLP merges
```

## Approach: Two-Pass Expression Emission

Each expression statement is processed in TWO passes:

**Pass 1 (Load):** Walk the expression tree, collect ALL field references
(`getelementptr` + `load`), emit them grouped by field region (contiguous GEP
indices). Each load result is stored in a temporary with a known name.

**Pass 2 (Compute):** Walk the expression tree again, emit arithmetic operations
in dependency order:
1. Leaf operations (loads — already done in Pass 1)
2. Single-arg operations (unary, cast — use loaded values)
3. Binary operations at dependency depth 1 (operands are loads or constants)
4. Binary operations at dependency depth 2+ (operands are earlier binary results)

Within the same dependency depth, operations are sorted by OPERATOR (all `fmul`
first, then all `fsub`, then all `fadd`). This produces consecutive isomorphic
operation groups that LLVM's SLP can merge.

## Data Structure

```rust
/// A flattened instruction ready for emission.
struct FlatOp {
    op: OpKind,          // Load, Cast, Unary, Binary(Sub), Binary(Add), etc.
    dest: String,        // Result register name
    src: Vec<String>,    // Source register names
    depth: usize,        // Dependency depth within expression tree
}

/// Result of the two-pass emission for a group of statements.
struct FlatBody {
    loads: Vec<FlatOp>,      // All field loads, GVN-deduplicated
    computes: Vec<FlatOp>,   // All arithmetic operations, sorted by operator
    results: Vec<(String, Option<String>)>,  // (field_name, result_reg_or_None)
}
```

## Procedure

### Phase 1: Build `emit_flat_body` (parallel to `emit_countable_body`)

**Step 1.1:** Add `FlatOp` and `FlatBody` data structures to `counter.rs`.

**Step 1.2:** Implement `collect_flat_refs(expr)` — walks the expression tree
and returns all field identifier references (for Pass 1 load grouping).

**Step 1.3:** Implement `build_flat_ops(expr, &load_results) → Vec<FlatOp>` —
walks the expression tree and returns FlatOps with dependency depth computed.

**Step 1.4:** Implement `emit_flat_body(out, body)` — the top-level function that:
1. Calls `collect_flat_refs` on each statement to collect all loads
2. Deduplicates loads to the same field (GVN)
3. Emits grouped GEP+load for all fields
4. Calls `build_flat_ops` for each statement
5. Sorts FlatOps by (depth, operator)
6. Emits sorted operations

**Step 1.5:** Wire `emit_flat_body` into `emit_countable_main` (replace
`emit_countable_body` call on line 494).

**Verification:**
- `cargo test --lib` — 1045 pass
- `nbody_newton SLP remarks` — should show -283 horizontal reduction
- `nbody_sqrt_idio` — should stay at 0.67x (already using LLVM's SLP)

### Phase 2: Remove Hand-Rolled SLP Passes

After `emit_flat_body` is verified:

**Step 2.1:** Remove `emit_slp_group` call in `counter.rs:760` — the `if false`
becomes the default. The FlatOp emission handles all vectorization via LLVM.

**Step 2.2:** Remove `is_reduction_pattern` from `chain_pass_ok` in
`slp_isomorphism.rs:96-98`.

**Step 2.3:** Remove `chain_pass_ok` entirely — the cost model is unnecessary
when LLVM handles SLP directly.

**Step 2.4:** Remove stride gate from counter.rs.

**Step 2.5:** Simplify hazard analysis — remove `slp_hazard_fns` computation,
keep only `raw_peak_live_floats` for dispatch path selection.

**Step 2.6:** Remove `slp_chain_pass_ok` fields and references.

**Verification after each step:** `cargo test --lib` + full benchmark run.
No regression allowed. 1045 tests pass.

## Expected Benchmark Impact

| Benchmark | Before (1.09x) | After (flat) | Mechanism |
|-----------|---------------|--------------|-----------|
| nbody_newton | 1.09x (phi) | **~0.72x** | LLVM SLP finds -283 reduction from flat fsub groups |
| nbody_sqrt_idio | 0.67x (good) | **~0.67x** | Already flat-compatible; stable |
| nbody_sqrt | 0.85x (good) | **~0.85x** | Stable |
| ring_buffer | 1.06x | 1.06x | No float ops, unaffected |
| float_math | 0.96x | 0.96x | Small state, already optimal |
| kalman | 0.99x | 0.99x | Complex inner loop, LLVM handles SLP |
| All others | parity | parity | No change |

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| FlatOp sort by operator breaks dependency order | Wrong output | Topological sort by depth FIRST, then by operator within same depth |
| Two-pass emission doubles compile time | +50% compile | GVN deduplication reduces redundant loads; profile and optimize if needed |
| Flat emission changes phi node semantics | Wrong output | Flat body replaces `emit_countable_body` for ONE path only; existing paths unchanged |
| LLVM's SLP doesn't find -283 even with flat emission | nbody stays at 1.09x | Investigate remaining structural differences (phi order, attribute layout) |
| Hand-rolled SLP removal regresses kalman | kalman > 1.0x | Keep hazard analysis (reg pressure guard) but remove SLP emission |

## Measurement Protocol

Each change measured with:
1. `cargo test --lib`
2. Single nbody_newton benchmark: `BOUND=50000000 brivc build` + `clang` + `time`
3. SLP remarks: `opt -O3 -pass-remarks=slp-vectorizer`
4. Full suite: `bash benchmarks/build_and_bench.sh --runtime`

## Rollback

If flat emission doesn't improve nbody_newton after Phase 1:
1. Keep `emit_flat_body` as an OPTIONAL path (gated behind a strategy flag)
2. Revert to `emit_countable_body` for all benchmarks
3. Report findings — flat emission is correct but doesn't help with our compiler's structural differences
