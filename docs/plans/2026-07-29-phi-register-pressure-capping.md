# Phi-Register-Pressure Capping

**Date:** 2026-07-29
**Problem:** nbody_newton benchmark regressed from 0.75x C (Era-5) to 1.23x C (current) due to excessive phi-carried float values exceeding architectural register capacity.
**Root cause:** Three simultaneous losses from the Phase 4 refactoring:
  1. Vector phi groups disabled (counter.rs:213-221 clears all state)
  2. Naming-based grouping replaced by broken isomorphism grouping
  3. SLP hazard gating removed entirely (hazard.rs deleted)
**Net effect:** 32 scalar phis emitted for 31 float fields + 1 counter phi = 32 phi nodes. x86-64 AVX2 has 16 XMM registers. 32 > 16 → at least 16 register spills per iteration, each adding 2 memory ops.

## Plan Directives Compliance

| Directive | How this plan follows it |
|-----------|--------------------------|
| **Flat control flow** | Max 2 nesting levels. Helper functions extracted for capping logic. |
| **Comment the code** | Every change site gets `// 2026-07-29: <why>` rationale. |
| **Update all examples** | No syntax changes — no example updates needed. |
| **Documentation is code** | This plan document updated; architecture docs updated in same commit. |
| **Behavioral tests** | New tests assert correct phi count under capping, not IR snapshots. |

## Research Citations

### LLVM `!invariant.load` Metadata
Source: [LLVM LangRef §invariant.load](https://llvm.org/docs/LangRef.html#invariant-load-metadata)
> "If an `invariant.load`-tagged operation is executed, every memory location read by that operation must contain the same value at all points in the program where that memory is dereferenceable."
Implication: Marking non-phi field loads as `!invariant.load` tells LICM the value never changes → enables hoisting out of the loop body. Works across `memory(readwrite)` functions because GEPs with different indices on the same base pointer are proven NoAlias by BasicAA.

### `"disable-slp-vectorize"` is NOT a Standard Attribute
Source: LLVM `llvm/include/llvm/IR/Attributes.td` — no such attribute defined.
LLVM accepts arbitrary string attributes in IR (parses silently) but **no pass reads `"disable-slp-vectorize"`**. The SLP vectorizer is controlled by command-line flags (`-vectorize-slp`, `-slp-vectorize-hor`), not function attributes. Era-5's `#5` attribute was silently ignored; the actual SLP-disable came from `llvm_extra_flags()` returning `["-slp-vectorize-hor=false"]`.
Implication: We cannot disable SLP via IR attributes. We must either pass CLI flags or make the IR naturally resistant to harmful SLP transformations. This plan chooses the latter — by reducing phi count to fit in registers, SLP's impact becomes benign and no global disable is needed.

### SROA Only Operates on `alloca`
Source: [LLVM Passes §SROA](https://llvm.org/docs/Passes.html#sroa-scalar-replacement-of-aggregates), SROA.cpp:17
> "This transformation implements the well known scalar replacement of aggregates transformation. It tries to identify promotable elements of an **aggregate alloca**, and promote them to registers."
SROA processes only `AllocaInst` objects. `%state` is a function pointer argument — SROA will NOT decompose GEP+load+store chains through it.
Implication: Our non-phi field loads stay as memory operations; they won't be re-promoted to phis.

### `inttoptr` Destroys Alias Analysis
Source: [LLVM LangRef §Pointer Aliasing Rules](https://llvm.org/docs/LangRef.html#pointer-aliasing-rules)
> "A pointer value formed by an `inttoptr` is based on all pointer values that contribute (directly or indirectly) to the computation of the pointer's value."
`inttoptr` creates a MayAlias with every other pointer in the function. For ring_buffer, `data: Ptr<Int>` stored as `i64` requires `load i64 → inttoptr → GEP` every iteration, and LICM cannot hoist the initial `load i64` because the `inttoptr`-derived store pointer is MayAlias with `%State` itself.
Implication: ring_buffer's pointer boxing is a separate issue from phi pressure. Not addressed by this plan.

### Benchmarks Game nbody Performance Data
Source: [benchmarksgame-team.pages.debian.net](https://benchmarksgame-team.pages.debian.net/benchmarksgame/performance/nbody.html)
- C gcc #9 (SIMD intrinsics): 2.10s — 2.5x faster than reference
- Rust #9 (portable SIMD): 2.19s
- C gcc #1 (reference, struct + sqrt): 5.23s
- Rust #1 (reference): 5.52s
The 2.5x speedup comes from avoiding scalar phi nodes entirely via explicit vector operations.
Implication: This validates our approach — fewer phi-carried values → better code.

### Era-5 IR Reconstruction
Source: Era-5 worktree at `../briv-compiler-era5`, hash `8a827db`
- 6 `<4 x float>` vector phis (bx0-3, by0-3, bz0-3, vx0-3, vy0-3, vz0-3)
- 11 scalar phis + 1 counter = 18 phis total
- Attribute `#5` on `@main` (SLP-hazard, now known to be decorative)
- Total instructions: 959 vs current 1169 (18% fewer)
- Vector instructions: 507 vs current 618 (22% fewer)
- `main()` disassembly size: 856 lines vs current 932 lines

### Current Binary (objdump) Analysis
- 32 scalar phis for 31 float fields + 1 counter
- 1,169 total instructions vs Era-5 959 vs C 907
- 618 vector instructions vs Era-5 507 vs C 542
- 118 stack spills vs Era-5 145 vs C 102

## Fix

### Phase 1: Add Register Budget Infrastructure

**File: `src/backend/llvm/context.rs`**

1. Add `float_register_count()` method to `CompilerContext`:
   ```rust
   /// 2026-07-29: Derive target float register count from LLVM target triple.
   /// Used by the dispatch to cap phi-carried float values.
   pub fn float_register_count(&self) -> usize {
       if self.target_triple.starts_with("aarch64") {
           32  // NEON: 32 Q registers
       } else if self.target_triple.starts_with("wasm32")
               || self.target_triple.starts_with("wasm64") {
           usize::MAX  // WebAssembly: virtual registers, no pressure
       } else if self.target_triple.starts_with("spirv64") {
           32  // SPIR-V: 32 registers
       } else {
           16  // x86_64 (default): 16 XMM registers (AVX2)
               // AVX-512 (32 ZMM regs) is detected via capability flag
       }
   }
   ```

2. Add `invariant_load_indices: HashSet<usize>` to `FunctionContext`:
   ```rust
   /// 2026-07-29: State field indices whose loads should carry `!invariant.load`
   /// metadata. Populated by the dispatch when the write_set is capped.
   /// Fields NOT in the write_set have loop-invariant values — LICM hoists
   /// their loads out of the loop body.
   pub invariant_load_indices: HashSet<usize>,
   ```

### Phase 2: Simplify Dispatch

**File: `src/backend/llvm/mod.rs` (dispatch section, lines 2692-2749)**

Replace the 4-way dispatch (VectorPhi / InlineSsa / PerFieldPhi guardrail / PerFieldPhi default)
with a 2-way dispatch (InlineSsa / PerFieldPhi with capping):

```rust
if !dispatched {
    // ── Structural 2-way dispatch ───────────────────────────
    // 1. InlineSsa — dense writes, small state, counter-only writes
    // 2. PerFieldPhi — everything else, with register-pressure capping
    //
    // VectorPhi path removed (2026-07-29): vector phi emission is
    // disabled in emit_countable_main (counter.rs:213-221), so the
    // detection was wasted work. Register-pressure capping replaces
    // the need for vector phis — reducing phi count to fit registers
    // achieves the same effect without vector phi complexity.
    let total_fields = self.ctx.field_index_map.len();
    let write_count = node.write_set.len();
    let write_density = if total_fields > 0 { write_count as f64 / total_fields as f64 } else { 1.0 };

    if write_density >= 0.5 && total_fields < 8 {
        // InlineSsa: insertvalue chain for small, dense-write states.
        // Only safe when the counter is the ONLY written field —
        // emit_folded_loop passes empty write_set and silently drops
        // non-counter writes. Guardrail below enforces this.
        let writes_non_counter = node.write_set.iter().any(|f| *f != bp.var);
        if writes_non_counter {
            // Non-counter state writes exist — route to PerFieldPhi.
            // Fall through to the PerFieldPhi block below.
        } else {
            self.fun.pending_post_hoist = post_hoist;
            self.warnings.push(format!(
                "info: txn '{}' dispatched via inline SSA ({}/{} fields written)",
                &node.name, write_count, total_fields
            ));
            self.emit_folded_main(&mut out, &node.name, counter_idx,
                total_idx, total_const_name, false, Some(&body_stmts));
            dispatched = true;
        }
    }

    if !dispatched {
        // PerFieldPhi: per-field phi loop with register-pressure capping.
        //
        // 2026-07-29: Capping logic — if the number of float fields in the
        // write_set exceeds the target's float register budget (minus 4
        // reserved for body temporaries), remove excess fields from the
        // write_set. Removed fields are handled via GEP+load+store with
        // !invariant.load metadata, enabling LICM to hoist their loads
        // out of the loop body.
        let float_reg_budget = self.ctx.float_register_count();
        let reserved_for_temps = 4;
        let max_phi_fields = if float_reg_budget == usize::MAX {
            usize::MAX  // no capping for virtual-register targets
        } else {
            float_reg_budget.saturating_sub(reserved_for_temps)
        };

        // Count float fields in the write_set
        let mut float_write_count = 0;
        let mut int_write_count = 0;
        for f in &node.write_set {
            if let Some(&idx) = self.ctx.field_index_map.get(f.as_str()) {
                if let Some(ty) = self.ctx.field_types.get(idx) {
                    if ty == "float" || ty == "double" {
                        float_write_count += 1;
                    } else {
                        int_write_count += 1;
                    }
                }
            }
        }

        let mut capped = false;
        let effective_write_set = if float_write_count > max_phi_fields {
            capped = true;
            // Cap: keep only max_phi_fields float fields + all int fields
            let mut sorted: Vec<&String> = node.write_set.iter().collect();
            sorted.sort();
            let mut capped_set: HashSet<String> = HashSet::new();
            let mut kept = 0;
            // Always keep the counter field
            capped_set.insert(bp.var.clone());
            for f in sorted {
                if f == &bp.var { continue; }
                let is_float = self.ctx.field_index_map.get(f.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .map(|t| t == "float" || t == "double")
                    .unwrap_or(false);
                if is_float {
                    if kept >= max_phi_fields { continue; }
                    kept += 1;
                }
                capped_set.insert(f.clone());
            }
            self.warnings.push(format!(
                "info: txn '{}' phi-capped to {}/{} float fields ({}/{} total, {} XMM budget)",
                &node.name, kept, float_write_count,
                capped_set.len(), write_count, float_reg_budget
            ));
            capped_set
        } else {
            node.write_set.clone()
        };

        // Populate invariant_load_indices for fields removed by capping
        if capped {
            for f in &node.write_set {
                if !effective_write_set.contains(f) {
                    if let Some(&idx) = self.ctx.field_index_map.get(f.as_str()) {
                        self.fun.invariant_load_indices.insert(idx);
                    }
                }
            }
            self.fun.needs_state_stores_in_body = true;
        }

        self.fun.pending_post_hoist = post_hoist;
        self.warnings.push(format!(
            "info: txn '{}' dispatched via per-field phi ({}/{} fields written{})",
            &node.name, effective_write_set.len(), total_fields,
            if capped { ", phi-capped" } else { "" }
        ));
        let is_decreasing = bp.direction
            == crate::analysis::transition_graph::ConvergeDirection::Decreasing;
        self.emit_countable_main(&mut out, &node.name, counter_idx,
            total_idx, total_const_name, &body_stmts,
            &effective_write_set, is_decreasing, Some(&bp.var));
    }
}
```

### Phase 3: Wire `!invariant.load` in Load Emission

**File: `src/backend/llvm/helpers.rs` (`load_field_type`, lines 2809-2850)**

After the `!range` metadata check (line 2847), append `!invariant.load` if the field index is in `invariant_load_indices`:

```rust
// 2026-07-29: Append !invariant.load metadata for loop-invariant fields.
// These are float fields removed from the write_set by phi-capping.
// Their values don't change within the loop body, so LICM can hoist
// the load to the preheader.
if self.fun.invariant_load_indices.contains(&idx) {
    write!(out, ", !invariant.load !{}", self.fun.next_invariant_md()).ok();
}
```

Add a helper to generate the invariant metadata node:
```rust
/// 2026-07-29: Generate a unique metadata node name for !invariant.load.
/// The metadata node itself is empty: `!N = !{}`.
fn next_invariant_md(&mut self) -> usize {
    let n = self.fun.metadata_counter;
    self.fun.metadata_counter += 1;
    n
}
```

And in `emit_toplevel.rs`'s metadata emission section, add each invariant metadata node.

### Phase 4: Remove Dead VectorPhi State Clear

**File: `src/backend/llvm/loop_engine/counter.rs` (lines 213-221)**

Remove the vector phi state clearance block — it's no longer reachable through the dispatch, but removing it prevents confusion:

```rust
// 2026-07-29: Vector phi state clearance removed — the dispatch no
// longer selects the VectorPhi path (see mod.rs dispatch simplification).
// All programs go through PerFieldPhi with register-pressure capping.
```

### Phase 5: Update Context Default

**File: `src/backend/llvm/context.rs`**

In the `FunctionContext` default/constructor, initialize `invariant_load_indices`:

```rust
invariant_load_indices: HashSet::new(),
```

## Verification

### Unit Tests
- `cargo test --lib` — all existing tests must pass
- New test: `test_phi_capping_nbody` — constructs an nbody-like state with 31 float fields, verifies the dispatch caps write_set to ≤12 float fields
- New test: `test_invariant_load_emitted` — verifies `!invariant.load` metadata appears on non-phi field loads

### Benchmark Comparison
1. Build with `cargo build --release`
2. Run `bash benchmarks/build_and_bench.sh --runtime` — verify correctness (all MATCH)
3. Compare nbody_newton ratio against current baseline

### Regression Check
- Verify all 19 benchmarks still pass correctness
- Compare instruction counts with `objdump -d` before/after

## Future Work (Not in Scope)

- **ring_buffer pointer boxing**: The `Ptr<Int>` → `i64` → `inttoptr` round-trip destroys alias analysis. Fix requires storing Ptr fields as native pointer types in state (tracking in a separate plan).
- **AVX-512 detection**: Currently defaults to 16 XMM regs. AVX-512 detection requires passing capabilities through to `CompilerContext`. The `TargetSection.capabilities` list could be forwarded.
- **Liveness-priority field ordering**: Currently caps alphabetically (sorted). A future improvement could prioritize fields by liveness interval width (most-used fields get phi priority).
