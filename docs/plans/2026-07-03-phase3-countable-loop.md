# Phase 3: Countable-Loop IR Restructuring

## Motivation

Current loop IR for nbody benchmarks uses a `%slot_case` alloca round-trip:

```llvm
case_hdr:
  %ssa = load %State, ptr %slot_case       ; 33-field load each iteration
  %count = extractvalue %State %ssa, 1
  %cond = icmp slt i64 %count, %bound
  br i1 %cond, label %body, label %done
body:
  ;; 30+ extractvalue from %ssa
  ;; scalar compute (fadd, fmul, sqrt)
  ;; 30+ insertvalue into new %State
  store %State %new_ssa, ptr %slot_case    ; 33-field store each iteration
  br label %case_hdr
done:
  ...
```

LLVM cannot recognize this as a countable loop because:
- No phi node in the header — the counter is extracted from a loaded struct
- The 33-field struct load/store round-trip per iteration prevents SROA promotion
- C auto-vectorizes to `<4 x float>`/`<8 x float>`+ `vector.reduce.fadd` (233 vector ops);
  Brief emits only scalar `@llvm.sqrt.f32` and `fadd` (0 vector ops)

**Target IR** (LLVM-countable with per-field phi nodes):

```llvm
entry:
  %count0 = load i64, ptr %gep_count       ; initial count
  %bound  = load i64, ptr %gep_bound       ; or constant
  br label %loop_hdr

loop_hdr:
  %i = phi i64 [ %count0, %entry ], [ %next, %latch ]
  %exit = icmp slt i64 %i, %bound
  br i1 %exit, label %body, label %done

body:
  ;; per-field GEP loads: %vx0 = load float, ptr %gep_vx0
  ;; scalar compute (fadd, fmul, sqrt) — same as current
  ;; per-field GEP stores: store float %new_vx0, ptr %gep_vx0
  br label %latch

latch:
  %next = add i64 %i, 1
  br label %loop_hdr, !llvm.loop !0

done:
  ...
```

This IR IS LLVM-countable — LLVM's `-indvars` and `-loop-vectorize` recognize
the `phi` + `icmp slt` + `add` pattern as a canonical induction variable.

## Architecture

The key insight is that `emit_ssa_main` already has a **canonical loop path**
(lines ~1014-1152) that creates per-field phi nodes. Phase 3 generalizes this
pattern to the single-txn case, bypassing the `%slot_case` alloca round-trip.

### Current Loop Strategies

| Strategy | Counter | State mechanism | Used when |
|----------|---------|-----------------|-----------|
| A005a phi | phi `%i` (register) | none — no body emitted | Pure body + runtime bound |
| A005a SSA | extractvalue from `%State` | `%slot_case` alloca round-trip | Non-pure but provably linear |
| A005b mem | GEP+load+add+store | GEP+load+store per field | Non-pure, NOT provably linear |
| SSA pipeline | phi or GEP+load | Per-field phi nodes (canonical) | Multi-txn reactive |

### Phase 3 Strategy: Per-Field Phi Loop

A new **A005c** path for single-txn programs where the body is "countable"
(defined below). Replaces the `%slot_case` round-trip with per-field phi nodes:

```
entry:
  GEP all fields                         ; one GEP per field
  load initial values into old_* regs    ; initial field values
  br label %loop_hdr

loop_hdr:
  %i = phi i64 [ %count0, %entry ], [ %next, %latch ]
  %bx0 = phi float [ %bx0_init, %entry ], [ %bx0_next, %latch ]
  ...one phi per field...
  %exit = icmp slt i64 %i, %bound
  br i1 %exit, label %body, label %done

body:
  ;; compute uses phi regs
  ;; assignments update backedge regs
  br label %latch

latch:
  %next = add i64 %i, 1
  ; per-field phis get their backedge values
  br label %loop_hdr
```

## Design

### Decision Tree

A txn is **countable** when:

1. **Single txn** (`graph.nodes.len() == 1`)
2. **Bounded precondition** with increasing counter
3. **Body has no side-effecting guards** — all guard bodies are pure
   (read-only compute, no FFI calls, no assignments to state)
4. **No reactive triggers** (`!graph.has_triggers`)
5. **Counter field is not read by any guard body** — the counter is
   used only in the pre-check and guard conditions, not in guard bodies
   (otherwise the per-field phi for the counter would conflict with
   the phi in the loop header — they'd be redundant but harmless)

**When countable:**
- Emit A005c per-field phi loop
- For each field in `write_set`:
  - Create `phi` node at `loop_hdr`
  - Backedge value = updated value from body (via `pending_phi_backedge`)
- For fields NOT in `write_set`:
  - Create `phi` node at `loop_hdr` with identity backedge (same value)
  - (Needed for SROA to know all fields have defined values)

**When NOT countable:**
- Fall back to existing A005a/A005b paths unchanged

### Body Emission

The body emission uses the **memory-mode GEP store** pattern (already well
tested), but with `emit_stmt`'s existing `ssa_state_reg = None` branch
that uses `emit_state_gep` + `ensure_typed_value` + `GEP store`.

Key difference from A005b memory path:
- At entry, loads all read-set fields into SSA phi registers
- In body, `emit_stmt` writes go directly to GEP memory AND update
  `pending_phi_backedge` for the latch
- At latch, backedge values feed the `phi` nodes

### `pending_phi_backedge` Tracking

The existing `pending_phi_backedge` HashMap on `FunctionContext` already tracks
updated field values during body emission (used by `emit_ssa_main`'s canonical
loop path). Phase 3 reuses this mechanism:

1. Before body emission: clear `pending_phi_backedge`
2. Each `Assignment` to a state field inserts into `pending_phi_backedge`
   (already happens in `emit_stmt`'s memory-mode branch, line ~650)
3. After body emission: iterate `pending_phi_backedge` to generate backedge
   values for the phi nodes

### Swan Song Handling

For terminating guards (`[count == bound] { term! -> print(...); }`):
- The swan song is hoisted to a post-loop block (already done by
  `hoist_terminating_guard` in `loop_engine.rs`)
- After the countable loop exits, emit the post-loop block

For periodic guards like `[count % 5000000 == 0] { print_float#(energy); }`:
- These are NOT terminating (they continue after the print)
- They are side-effecting (FFI call `print_float#`)
- A txn with non-terminating side-effecting guards is NOT countable
  (must fall back to A005a/A005b)

BUT: we can handle the terminating case by hoisting the guard check to the
latch block (before the backedge). The guard reads `%i` (the phi counter)
instead of extracting from `%State`. The `energy` value needed for printing
must be live after the loop body — this is already true since it's just
computed from the field phis.

**Actually, the simplest approach**: a countable txn CAN have a
terminating guard `[count == bound] { term! -> ... }` — this guard fires
exactly when the loop would exit anyway (since `count == bound` is the
postcondition). So the guard body becomes the post-loop block. No extra
branch needed in the hot loop.

A txn with a **periodic** guard (`[count % N == 0] { ffm!; }`) is
NOT countable because the guard body has a side effect that fires during
the loop.

## Implementation Plan

### Step 1: Add `is_countable_txn()` helper

Location: `src/backend/llvm/loop_engine.rs` or `helpers.rs`

Logic:
```
fn is_countable_txn(node: &ReactorNode, body: &[Statement]) -> bool {
    // Must have bounded pre + increment
    let pre = node.bounded_pre.as_ref()?;
    let inc = node.increments.as_ref()?;
    // Must be single-counter-increasing
    if pre.var != inc.var || inc.delta <= 0 { return false; }
    // Body must not have non-terminating side-effecting guards
    for stmt in body {
        if let Statement::Guarded { condition, statements, .. } = stmt {
            if has_side_effect(statements) && !is_terminating(stmt) {
                return false;
            }
        }
    }
    true
}
```

### Step 2: Add `emit_countable_main()`

Location: `src/backend/llvm/loop_engine.rs`

Signature:
```rust
pub(crate) fn emit_countable_main(
    &mut self,
    out: &mut String,
    txn_name: &str,
    bounded_pre: &BoundedPre,
    increments: &IncrementInfo,
    body: &[Statement],
    write_set: &HashSet<String>,
    swan_song: Option<&Statement>,
);
```

Pseudocode:
```
emit_countable_main:
  1. function header: define i32 @main()
  2. %state alloca
  3. emit_inline_init_stores
  4. Determine counter field index from bounded_pre.var
  5. Build list of all fields + their indices + types:
     - Read set: fields referenced in body expressions
     - Write set: fields assigned in body
     - All fields (for phis): union of read + write + counter + bound
  6. Entry block:
     - Emit GEPs for all fields
     - Load initial values
     - Pick initial counter value from field_load[counter_idx]
     - Pick bound value (field or constant)
     - br label %loop_hdr
  7. Loop header (loop_hdr):
     - %i = phi i64 [ init_count, %entry ], [ %next, %latch ]
     - For each field: %f{idx} = phi <type> [ init_val, %entry ], [ %backedge_val, %latch ]
     - %exit = icmp slt i64 %i, %bound
     - br i1 %exit, label %body, label %done
  8. Body emission:
     - Push GEP results as old-value registers for expression emission
     - Call emit_stmt for each body statement (same as A005a/A005b)
     - Track pending_phi_backedge from stores
  9. Latch block:
     - %next = add i64 %i, 1
     - For each field: get backedge value (from pending_phi_backedge or identity)
     - br label %loop_hdr
  10. Done block:
     - Store final phi values back to %State
     - Emit swan song / post-loop block
     - ret i32 0
```

### Step 3: Modify dispatch in `mod.rs`

Location: Decision tree at lines ~2070-2167

Add a check after the existing foldable detection:
```rust
if foldable && is_countable_txn(&node, &body_stmts) {
    // Phase 3: per-field phi loop (A005c)
    emit_countable_main(out, ...);
} else if foldable {
    // existing A005a/A005b path
    ...
}
```

The check should come BEFORE the existing paths so countable txns take
the new path. All existing paths remain untouched.

### Step 4: Support `pending_phi_backedge` in folded mode

Currently `pending_phi_backedge` is only populated when `ssa_state_reg`
is `None` (memory mode). In countable mode:
- `ssa_state_reg` is `None` (we're not using insertvalue chains)
- `pending_phi_backedge` receives the adapted value after each store
- We iterate `pending_phi_backedge` at the latch to generate phi backedge values

If a field is NOT in `pending_phi_backedge` (unchanged), its phi backedge
is the same as its current value (identity). This is correct for SROA.

## Benchmark Impact

| Benchmark | Current gap | Expected after Phase 3 | Mechanism |
|-----------|-------------|----------------------|-----------|
| nbody_sqrt | 1.29× | ≈1.0× | Per-field phis let SROA decompose %State; LLVM sees countable loop with scalar float ops |
| nbody_newton | 1.48× | ≈1.0× | Counter moves from memory (GEP+load+add+store) to phi; same per-field phi benefits |
| nbody_sqrt_idio | 1.04× | ≈1.0× | Same as nbody_sqrt |
| precompute_sum | already folded | no change | Already uses A005a pure counter path |
| All other | no gap | no change | A005b/A004/A000 paths unchanged |

### Expected Vectorization Outcome

Even with per-field phis, LLVM may not auto-vectorize if the body contains:
- **`@llvm.sqrt.f32` calls** — LLVM can vectorize `sqrt` (it maps to
  `<N x float> @llvm.sqrt.vNf32`)
- **`fmul`/`fadd` chains** — LLVM can vectorize these with `fast` flags
- **`fdiv` instructions** — LLVM can vectorize these

The main remaining blocker is the **scalar `@dt` loads** — `load float, ptr @dt`
happens 12× per outer body iteration (one per body pair). LLVM should hoist
these with LICM. After hoisting, the body has no memory operations at all
(except the per-field phi loads/stores at the loop boundary).

After per-field phi emission:
- Loop body is pure math (no loads/stores to %State, no branches)
- LLVM can vectorize the inner float operations
- Expected: Brief matches C's vectorization pattern

## Files to Modify

| File | Change | Lines added |
|------|--------|-------------|
| `src/backend/llvm/loop_engine.rs` | Add `is_countable_txn()`, `emit_countable_main()` | ~200 |
| `src/backend/llvm/mod.rs` | Add dispatch branch for countable | ~20 |
| `src/backend/llvm/helpers.rs` | Optionally any shared helpers | ~10 |
| Total | | ~230 |

## Testing

1. **All existing tests pass** (`cargo test --lib`): 1363 tests
2. **New tests**: Add one new test case with a simple counter loop
   that can be verified to produce LLVM-countable IR
3. **Manual verification**: Compile nbody_sqrt to IR, check that the
   loop has `phi i64 %i` with `icmp slt` + `add` pattern

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Per-field phi breaks when body has statements outside the write set | Low | Identity backedge for unchanged fields |
| Latch block dominance failure with multi-block body | Low | `prove_linear` already ensures at most 1 branch path per iteration |
| Countable decision tree too conservative (misses vectorization opportunities) | Medium | Start conservative, widen as verified |
| `pending_phi_backedge` not populated for all field types | Low | Check all type arms in emit_stmt's memory-mode store |
| Swan song references `%count` but counter is now `%i` (phi) | Medium | Replace `%count` refs with `%i` in hoisted swan song |
