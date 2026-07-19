# Close the Loop Engine Performance Gap

**Date:** 2026-07-19
**Status:** Plan — ready to implement
**Problem:** nbody_newton regressed from 6.1s (Phase 3) to 13.3s (current) — 2.2× slowdown
**Root Cause:** Single-txn programs with ≥8 state fields are dispatched to `emit_countable_main` (per-field phi nodes), which adds overhead from per-field phis, backedge registers, and convergence checks. The Phase 3 baseline used a simple counter-phi loop with GEP+load+store for state fields — less SROA-friendly but faster for tight float loops.

---

## Strategy: Add `emit_simple_counter_loop` dispatch path

### Current dispatch (single-txn fold path, mod.rs ~2462-2540)

```
foldable = true, single txn, bounded_pre, increments
    → if pure + !swan_song + const bound → emit_folded_pure_counter
    → elif (write_density >= 0.5 && total_fields < 8 && !has_body_ffi)
        → emit_folded_main(use_phi=false)   [EmitInlineSsa]
    → else
        → emit_countable_main                [EmitPerFieldPhi]
```

The **else** branch fires for nbody_newton (33 fields, ≥8 threshold). This adds per-field phi nodes.

### New dispatch (add before the else)

```
    → elif (!needs_phi_for_sroa)
        → emit_simple_counter_loop()          [NEW — Phase 3 style]
    → else
        → emit_countable_main                [EmitPerFieldPhi — unchanged]
```

### Conditions for `needs_phi_for_sroa`

The per-field phi pattern from `emit_countable_main` is valuable when LLVM's SROA can decompose `%State` fields into scalar registers, eliminating memory traffic from the hot loop. This pays off when:

1. **Body has many field reads across iterations** — per-field phis keep values in SSA registers
2. **Fields have cross-iteration dataflow** — phi nodes model the dataflow explicitly
3. **Sparse writes** — fewer phi-tracked fields means fewer backedge registers

But when the body is dense (reads AND writes every field every iteration), the phis don't help — SROA can't eliminate memory because each iteration stores results that the next iteration loads.

**Heuristic:** Skip per-field phis (use simple counter loop) when:
- All state fields are written every iteration (`write_density >= 0.8`)
- Counter convergence (`bounded_pre` + `increments`)
- No body FFI
- No async, no wake triggers, single txn

nbody_newton meets ALL these criteria — it writes ALL 33 fields every iteration (positions, velocities, energy, counters).

### What `emit_simple_counter_loop` does

Emits the Phase 3 style loop:

```
entry:
  %counter_gep = getelementptr %State, ptr %state, i32 0, i32 COUNTER_IDX
  br label %.loop_hdr

.loop_hdr:
  %counter_phi = phi i64 [0, %entry], [%next, %latch]
  %bound = load i64, ptr @TOTAL
  %done = icmp sge i64 %counter_phi, %bound
  br i1 %done, label %.done, label %.body

.body:
  // Txn body — inline, all state via GEP+load+store
  call void @txn_simulate(ptr %state)
  br label %.latch

.latch:
  %next = add i64 %counter_phi, 1
  store i64 %next, ptr %counter_gep
  br label %.loop_hdr

.done:
  ret i32 0
```

Key characteristics:
- **1 phi node** — only the counter
- **No per-field phis** — all state via memory (GEP+load+store)
- **`@txn_<name>` call** — reuses existing body function (already emitted)
- **No convergence check** — just counter ≥ bound

---

## HashMap Determinism Audit

### Risk

Rust's `HashMap` with default hasher (SipHash-1-3) uses a random seed per process. Iteration order differs across compilations. If any codegen path iterates a HashMap to emit IR instructions, the IR order changes → LLVM optimization may produce different machine code → up to ~9% performance variation (per AGENTS.md).

### What to check

In the codegen paths affected by our dispatch change:

| HashMap | File | Iterated? | Sorted? | Risk |
|---------|------|-----------|---------|------|
| `field_index_map` | mod.rs | ✅ loop_engine uses it | No | HIGH — used for IR emission |
| `pending_phi_backedge` | counter.rs | ✅ iterated for stores | Check | HIGH |
| `phi_field_regs` | counter.rs | ✅ iterated for phi nodes | Check | HIGH |
| `backedge_field_regs` | counter.rs | ✅ iterated for edges | Check | HIGH |
| `last_val_temps` | ssa.rs | ✅ iterated for hoisted prints | ✅ (sorted) | LOW |
| `done_needs_fields` | counter.rs | ✅ iterated for post-loop stores | Check | HIGH |

**Fix for all:** Before any emission loop, collect keys into `Vec<String>`, sort, iterate sorted.

---

## Latch Structure

The current `emit_countable_main` uses a separate `.cm_latch` block for backedge register shuffles. This is where per-field phi values are "identity-added" (`add i64 0, %val`) to break SSA dominance in LLVM.

In `emit_simple_counter_loop`, the latch is much simpler — just increment the counter and branch back. No per-field shuffles needed. This reduces basic block transitions and register pressure.

Phase 3 didn't need a latch at all — it used `loop_hdr` → body → backedge to `loop_hdr`. The current codegen can do the same: `.loop_hdr` → `.body` → `br %next_loop_hdr`. No separate latch block.

---

## Implementation

### Files to modify

| File | Change |
|------|--------|
| `src/backend/llvm/mod.rs` | Add dispatch condition choosing `emit_simple_counter_loop` |
| `src/backend/llvm/loop_engine/ssa.rs` | Add `emit_simple_counter_loop` function |
| `src/backend/llvm/loop_engine/` (various) | Add `sort_keys()` calls for HashMap determinism |

### `emit_simple_counter_loop` signature

```rust
pub(crate) fn emit_simple_counter_loop(
    &mut self,
    out: &mut String,
    txns: &[(String, &crate::ast::Transaction)],
    counter_idx: usize,
    bound_idx: Option<usize>,
    bound_const_name: Option<&str>,
)
```

### Dispatch insertion point (mod.rs ~2530)

```rust
// 2026-07-19: Single-txn counter loop — for programs with dense writes
// (≥80% write density) where per-field phis don't help SROA.
// Uses a simple counter-phi loop with GEP+load+store for state fields,
// avoiding the overhead of per-field phi nodes and backedge registers.
if !has_body_ffi && write_density >= 0.8 && !self.async_txn_names.is_empty() {
    self.emit_simple_counter_loop(&mut out, &txns, counter_idx,
        bound_idx, bound_const_name);
}
```

---

## Testing

| Test | What it asserts |
|------|----------------|
| `test_simple_counter_loop_emission` | IR contains `.loop_hdr`/`.body`/`.latch`/`.done` with single counter phi |
| `test_deterministic_ir` | Two compilations produce identical IR (HashMap sorted) |
| `nbody_newton timing` | 6-8s (close to Phase 3 baseline of 6.1s, not 13.3s) |
| `full correctness` | All 23 benchmarks still MATCH |
