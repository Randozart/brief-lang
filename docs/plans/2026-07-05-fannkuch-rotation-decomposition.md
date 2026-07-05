# Fix fannkuch_redux: Circular Phi Chain Decomposition

Date: 2026-07-05
Status: Execution-ready
Target: Reduce fannkuch_redux from 1.65x to ~1.00x

## 1. The Problem

fannkuch_redux has a 12-element rotation of permutation fields:

```
&p0 = p1;  &p1 = p2;  ...  &p10 = p11;  &p11 = saved;
```

In A005c per-field phi dispatch, each field has a phi node and a backedge.
The backedges form a 12-cycle:

```
phi_p0 = phi(init_p0, be_p0)  where be_p0 = phi_p1
phi_p1 = phi(init_p1, be_p1)  where be_p1 = phi_p2
...
phi_p11 = phi(init_p11, be_p11) where be_p11 = phi_p0
```

This 12-cycle exceeds LLVM's SCEV depth limit (~7). All 12 permutation
phis become SCEVUnknown. Without SCEV, LLVM cannot:
- Unroll the loop (no trip count analysis)
- Apply dependence analysis for the checksum reduction
- Perform induction variable simplification

C's clang breaks the 12-cycle into 4 independent 3-cycles by unrolling
4× and restructuring the phis.

## 2. First Attempt (Failed): Step-k Backedges

**Approach:** Change the backedge step from 1 to k (e.g., k=4 for 12-cycle).

```
// Step=4: p0←p4←p8←p0, p1←p5←p9←p1, etc.
be_p0 = phi_p4,  be_p1 = phi_p5,  ...,  be_p11 = phi_p3
```

**Why it failed — phi accumulation:** Phi registers ACCUMULATE the step
rotation across trips. After trip 1:
- `be_p0 = phi_p4`: `phi_p4` was computed from `be_p4 = phi_p8` during
  trip 1's latch. So `phi_p4 = old_p8` (the value that p8 had before
  trip 1, shifted by step from the correct value).
- Expected `be_p0 = old_p4`, but got `be_p0 = old_p8` — DOUBLE rotation.

The step-k backedge causes each trip to apply the rotation TWICE: once
in the body (1-step per copy × k copies = k-step total) and once in the
latch (k-step backedge). After each trip, values shift by 2k modulo n,
not k. This is a fundamental SSA property: phi nodes carry accumulated
values, not per-iteration deltas.

**Dominance failure was a secondary issue:** The body-copy registers
(`pending_phi_native_backedge`) were used in the latch block, which has
many predecessor blocks (body + body_rot1..body_rot{k-1}). Registers
from any single predecessor don't dominate the latch. This was fixed
with GEP reloads, but the phi accumulation remained as a correctness bug.

## 3. Correct Approach: GEP Reloads in the Latch

Instead of cross-referencing phi registers, emit INDEPENDENT GEP loads
from %State directly in the latch block:

```
latch:
  be_p0 = load i64, ptr %gep_p0    ; independent per field
  be_p1 = load i64, ptr %gep_p1    ; independent per field
  ...
  be_p11 = load i64, ptr %gep_p11  ; independent per field
```

Each backedge is an INDEPENDENT value — no phi cross-references. SCEV
sees 12 independent loop-invariant load-through-store patterns.

### Why this avoids BOTH problems

1. **Dominance:** The GEP load is defined in `%latch`, which IS the phi
   predecessor. `%state` (the alloca base) is defined in `entry` which
   dominates every block. The load results dominate their phi node uses.
   No dominance issue.

2. **No phi accumulation:** Each field's backedge is `load %gep_pN` —
   the current value stored in %State, which was written by the LAST
   body copy of the preceding trip. For step=4, after trip 1 (4 body
   copies): `%gep_p0` = `old_p4`. The latch loads `old_p4` into `be_p0`.
   Next trip: `phi_p0 = old_p4`. CORRECT — no double rotation.

3. **GVN eliminates the load:** LLVM's GVN sees:
   ```
   store i64 %val, ptr %gep_p0    ; in body (last copy)
   load i64, ptr %gep_p0           ; in latch
   ```
   The store dominates the load at the same GEP address → GVN replaces
   the load with `%val`. Final machine code has zero memory traffic.
   But SCEV analyzed the LOAD as an independent induction, so the
   12-cycle never formed in SCEV's analysis.

### Key mechanism: Forced stores for rotation fields

For the latch's GEP reload to work, the body MUST store to %State for
rotation fields. In commit-block mode (`needs_state_stores_in_body =
false`), stores are suppressed. The override: add rotation fields to a
`rotation_fields` set, and modify the store gate in
`emit_memory_field_store` to accept `rotation_fields` as an override:
```rust
if (self.fun.needs_state_stores_in_body || self.fun.rotation_fields.contains(fname))
    && (self.fun.done_needs_fields.is_empty() || ...)
```

This forces stores ONLY for rotation fields, preserving Path A for all
other fields (counter, checksum, seed, etc. benefit from zero stores).

## 4. Implementation

### 4.1 Files to modify

| File | Change | Lines added |
|------|--------|-------------|
| `context.rs` | Add `rotation_fields` to FunctionContext | 3 |
| `emit_stmt.rs` | Modify store gate in emit_memory_field_store | 2 |
| `loop_engine.rs` | Rotation detection, body unrolling, latch GEP reloads | ~60 |

### 4.2 `context.rs`

Add field to FunctionContext:
```rust
pub rotation_fields: HashSet<String>,
```
Initialize in `new()`:
```rust
rotation_fields: HashSet::new(),
```

### 4.3 `emit_stmt.rs` — emit_memory_field_store

Line 56 (integer store gate):
```rust
// From:
if self.fun.needs_state_stores_in_body && (self.fun.done_needs_fields.is_empty() || ...)
// To:
if (self.fun.needs_state_stores_in_body || self.fun.rotation_fields.contains(fname))
    && (self.fun.done_needs_fields.is_empty() || ...)
```

Line 87 (float store gate): same change.

This forces stores for rotation fields even when Path A is active.

### 4.4 `loop_engine.rs` — emit_countable_latch

In the modified-field backedge path (line 1141), add rotation check:
```rust
if rotation_step > 1 && self.fun.rotation_fields.contains(name) {
    // GEP reload in latch (dominates backedge trivially).
    let Some(&(idx, ref ty)) = field_map.get(name) else { continue; };
    let gep_reload = self.emit_state_gep(out, "  ", "be", "%state", idx);
    writeln!(out, "  {} = load {}, ptr {}, align {}",
        be_reg, ty, gep_reload, self.align_of(ty)).ok();
} else if let Some(typed_reg) = self.fun.pending_phi_native_backedge.get(name) {
    ...existing path...
}
```

### 4.5 `loop_engine.rs` — emit_countable_main

After parallel-safe exemption setup, before body emission:

```rust
let rotation_step = detect_rotation_ast(&filtered_body, &self.ctx.field_index_map);
let rotation_enabled = rotation_step > 1;
if rotation_enabled {
    for s in &filtered_body {
        if let Statement::Assignment { lhs, .. } = s {
            if let Some(fname) = target_field_name(lhs) {
                if self.fun.phi_field_regs.contains_key(&fname) {
                    self.fun.rotation_fields.insert(fname.clone());
                    self.fun.parallel_safe_exempt_fields.insert(fname.clone());
                }
            }
        }
    }
    // Don't suppress stores — latch GEP reloads need them.
    self.fun.needs_state_stores_in_body = true;
    // Override exit_label to "done" — commit block would load
    // from phi registers (stale), not %State (current).
    // (The exit_label was set above — we override it here.)
    // We just keep needs_state_stores_in_body=true.
}
```

Body emission (replace the single call with unrolled loop):
```rust
self.emit_countable_body(out, &filtered_body);
if rotation_enabled {
    for i in 1..rotation_step {
        // GEP-reload rotation fields into ssa_old caches for next copy.
        for fname in &self.fun.rotation_fields {
            let Some(&idx) = self.ctx.field_index_map.get(fname) else { continue; };
            let ty = &self.ctx.field_types[idx];
            let gep = self.emit_state_gep(out, "  ", "rr", "%state", idx);
            let ld = format!("%rld_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = load {}, ptr {}, align {}", ld, ty, gep, self.align_of(ty)).ok();
            if ty == "float" || ty == "double" {
                self.fun.ssa_old_float_regs.insert(fname.clone(), ld.clone());
            } else {
                self.fun.ssa_old_int_regs.insert(fname.clone(), ld.clone());
            }
        }
        // Overflow guard: if count >= bound, exit to done.
        let count_reg = self.fun.ssa_old_int_regs.get("count")
            .or_else(|| self.fun.ssa_old_int_regs.get(&counter_name)).cloned();
        if let Some(creg) = count_reg {
            let chk = format!("%ro_chk_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = icmp sge i64 {}, {}", chk, creg, bound_reg).ok();
            writeln!(out, "  br i1 {}, label %done, label %body_rot{}", chk, i).ok();
            writeln!(out, "body_rot{}:", i).ok();
        }
        // Emit body copy (preserves ssa_old from GEP reloads).
        self.fun.let_bindings.clear();
        self.fun.let_binding_types.clear();
        self.fun.reg_float_cache.clear();
        self.fun.reg_type_cache.clear();
        self.fun.expr_dedup_cache.clear();
        self.fun.terminated = false;
        self.fun.loop_exit_label = Some("done".into());
        for s in &filtered_body {
            if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                self.emit_stmt(out, s, "  ");
            }
        }
        self.fun.loop_exit_label = None;
    }
}
```

Latch call (at the normal position):
```rust
self.emit_countable_latch(out, &pi_name, &pn_name, &count_be_reg,
    &counter_name, rotation_step);
```

### 4.6 Cleanup

At the end of `emit_countable_main` (line 1420-1426):
```rust
self.fun.needs_state_stores_in_body = true;
self.fun.counter_field_name = None;
self.fun.parallel_safe_exempt_fields.clear();
self.fun.rotation_fields.clear();
```

## 5. Verification

1. `cargo test --lib` — all 1398+ tests pass
2. `bash benchmarks/build_and_bench.sh --correctness` — fannkuch_redux MATCH
3. `bash benchmarks/build_and_bench.sh --runtime` — ratio from 1.65x to ≤1.10x

## 6. Edge Cases

- **Non-divisible bounds**: The overflow guard after each body copy
  checks `count >= bound`. If the bound isn't divisible by step, the
  guard catches it and branches to `done`. This handles any bound value.

- **Step rotation for non-12-length cycles**: `detect_rotation_ast` returns
  the optimal step for any cycle length. For prime lengths (e.g., 11),
  `optimal_step_for_cycle_length` returns 1 (no rotation decomposition).
  Only cycles with a divisor producing sub-cycles ≤ 4 are decomposed.

- **Mixed rotation/non-rotation fields**: The `rotation_fields` set
  only includes fields assigned from other field phis (not from let
  bindings or expressions). Non-rotation fields like `checksum`, `seed`,
  and `max_flips` keep standard A005c behavior.
