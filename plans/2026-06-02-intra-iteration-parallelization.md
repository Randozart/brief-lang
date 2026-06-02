# Intra-Iteration Parallelization (De-Chaining) in SSA Folded Loops (2026-06-02)

## Problem

The SSA body in `emit_folded_loop` creates unnecessary serial dependencies between float statements within a single iteration. Consider:

```
&x0 = A00*x0 + A01*x1 + A02*x2;   // writes to ssa_state_reg
&x1 = A10*x0 + A11*x1 + A12*x2;   // reads x0 from JUST-WRITTEN ssa_state_reg → DEPENDENCY
&x2 = A20*x0 + A21*x1 + A22*x2;   // reads x1,x0 from JUST-WRITTEN ssa_state_reg → DEPENDENCY
```

Statement 2's `x0` reads the value Statement 1 just computed — not the phi register's x0 from the previous iteration. This chains all 9 float operations into a single serial dependency, preventing the CPU's 3 float execution ports from operating in parallel.

## Solution

**Load all, compute all, store all.**

Before emitting body statements, pre-extract all float field values from the phi register into named SSA registers ("old values"). All body statements read these old values — never the freshly-computed ones. After all statements are emitted, insertvalue all results back.

```
// Extract old values from phi register (all independent)
%x0_old = extractvalue %State %ssa_phi, 0
%x1_old = extractvalue %State %ssa_phi, 1
%x2_old = extractvalue %State %ssa_phi, 2

// All body ops read old values → all 15 ops are independent
%tmp_x0 = fmul %A00, %x0_old  → fadd ... // chain 1
%tmp_x1 = fmul %A10, %x0_old  → fadd ... // chain 2 (reads x0_old, NOT tmp_x0)
%tmp_x2 = fmul %A20, %x0_old  → fadd ... // chain 3

// Insert results at bottom
%new = insertvalue %State %ssa_phi, %tmp_x0, 0
%new = insertvalue %State %new, %tmp_x1, 1
%new = insertvalue %State %new, %tmp_x2, 2
```

All 15 float operations are now independent — LLVM's MachineScheduler fires them across all 3 ports simultaneously.

## Implementation

**File**: `src/backend/llvm.rs`

### Change 1: Add `ssa_old_float_regs` field

```rust
pub struct LlvmBackend {
    // ... existing fields ...
    ssa_old_float_regs: HashMap<String, String>,  // field_name → old-value register
}
```

### Change 2: In `emit_folded_loop`, SSA body section (~line 3065)

After the phi register is loaded and `self.ssa_state_reg` is set, but BEFORE emitting body statements:

```rust
// Pre-extract all float fields from the phi register so body
// statements can read OLD values independently — no intra-iteration
// dependencies between field writes.
self.ssa_old_float_regs.clear();
for (field_name, &field_idx) in &self.field_index_map {
    if self.field_types[field_idx] == "float" {
        let old_reg = format!("%{}_old_{}", field_name, self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "  {} = extractvalue %State {}, {}", old_reg, phi_reg, field_idx).ok();
        self.ssa_old_float_regs.insert(field_name.clone(), old_reg);
    }
}
```

### Change 3: Float field identifier lookup

In `emit_expr`, when resolving `Expr::Identifier`:

```rust
// SSA body mode: check for pre-extracted old-value register
if let Some(ref old_reg) = self.ssa_old_float_regs.get(name) {
    return TypedRegister { name: old_reg.clone(), ty: Type::Float };
}
```

This uses the existing `self.ssa_old_float_regs` map. Float field identifiers in SSA body mode return the pre-iteration extracted value, not the intra-iteration freshly-computed one.

### Change 4: Cleanup at body end

After all body statements are emitted, clear `ssa_old_float_regs`.

## Effect on the SSA body flow

```
BEFORE (serialized within iteration):
  extract %ssa_phi, 0  →  compute x0  →  insertvalue
  extract %ssa_phi, 1  →  compute x1 (depends on x0's insertvalue)
  extract %ssa_phi, 2  →  compute x2 (depends on x0,x1's insertvalue)

AFTER (parallel within iteration):
  extract %ssa_phi, 0  ─┐
  extract %ssa_phi, 1  ─┤ all independent extracts
  extract %ssa_phi, 2  ─┘
                          
  compute x0 (from old values)  ─┐
  compute x1 (from old values)  ─┤ all independent computations
  compute x2 (from old values)  ─┘

  insertvalue x0  ─┐
  insertvalue x1  ─┤ all independent inserts (fixed positions)
  insertvalue x2  ─┘
```

## Correctness

- Each assignment `&fn = expr` writes to the SSA state register via `insertvalue`, but READS for field name `fn` use the pre-extracted old value, not the intra-iteration value
- The final state after all insertvalues is identical — all writes are field-disjoint (each field written exactly once per iteration)
- Counters and non-float fields are unaffected — their `emit_expr` path uses existing `ssa_state_reg` based extraction since they don't appear in `ssa_old_float_regs`
- The phi's backedge value is the state after all insertvalues, which correctly captures the "next iteration" state
- Pure O(1) bodies (phi mode) skip the SSA body path entirely — no change

## Test Strategy

Existing LLVM tests parse emitted IR — no structural change expected since we're adding `extractvalue` instructions BEFORE the body, and changing which register aliases `emit_expr` resolves. The SSA body emission tests should still pass unchanged (they check for counters/state stores, not for internal extract ordering).

New test: verify that the emitted SSA body has `_old` suffixed registers and that a float assignment's RHS uses them.

## Files Changed

- `src/backend/llvm.rs`: +struct field, +pre-extraction block, +lookup logic in emit_expr, +cleanup

## Expected Impact

**float_math_nonzero**: The 9 float ops become 3 independent chains of 5 ops each → all 3 ports busy → throughput matches C's parallel layout. Expected: 2.3× → ~1.1×.

**float_math**: Same benefit — the 12-field body goes from 1 serial chain to 12 independent chains. Expected: 1.45× → ~1.0×.
