# Fix fannkuch_redux: Circular Phi Chain Decomposition

Date: 2026-07-05
Status: Plan for execution
Target: Reduce fannkuch_redux from 1.65x to ~1.00x

## 1. Problem

**fannkuch_redux (1.65x vs C)** has a 12-element circular phi chain:

```
phi_p0 = phi(init_p0, be_p0)  where be_p0 = phi_p1  (p0 ← p1)
phi_p1 = phi(init_p1, be_p1)  where be_p1 = phi_p2  (p1 ← p2)
...
phi_p11 = phi(init_p11, be_p11) where be_p11 = phi_p0  (p11 ← p0)
```

This 12-cycle exceeds LLVM's SCEV analysis depth (~7). All 12 phis become
`SCEVUnknown`, which prevents loop unrolling and dependence analysis.
C's clang unrolls 4× and creates 4 independent 3-cycles:

```
Cycle A: p0 ← p4 ← p8 ← p0   (3-step, starting at p0)
Cycle B: p1 ← p5 ← p9 ← p1   (3-step, starting at p1)
Cycle C: p2 ← p6 ← p10 ← p2   (3-step, starting at p2)
Cycle D: p3 ← p7 ← p11 ← p3   (3-step, starting at p3)
```

## 2. Approach: Rotation-Aware A005c Latch

The fix modifies `emit_countable_main` and `emit_countable_latch` in
`loop_engine.rs` to detect rotation patterns and emit step-k backedges.

### 2.1 Detection (in `emit_countable_main`, after body emission)

```rust
/// 2026-07-05: Detect if the body's field assignments form a rotation
/// (permutation cycle).  Returns the optimal step size that decomposes
/// the largest cycle into sub-cycles of length ≤ 4.
fn detect_rotation_step(&self) -> Option<usize> {
    let n = self.ctx.field_index_map.len();
    if n < 4 { return None; }
    
    // Build permutation: written_field → source_phi_register
    let mut perm: Vec<(usize, usize)> = Vec::new();
    for (name, typed_reg) in &self.fun.pending_phi_native_backedge {
        let Some(&dst_idx) = self.ctx.field_index_map.get(name) else { continue; };
        // typed_reg looks like "%phi_p1" — extract field name
        let reg_name = typed_reg.trim_start_matches('%');
        // Check if the register is "phi_<field_name>" (a phi value)
        if let Some(src_name) = reg_name.strip_prefix("phi_") {
            if let Some(&src_idx) = self.ctx.field_index_map.get(src_name) {
                perm.push((dst_idx, src_idx));
            }
        }
    }
    if perm.len() < 4 { return None; }
    
    // Find cycles in the permutation
    let cycles = find_cycles(&perm, n);
    if cycles.is_empty() { return None; }
    
    // Find the maximum cycle length
    let max_len = cycles.iter().map(|c| c.len()).max()?;
    if max_len <= 4 { return None; } // LLVM handles ≤4 already
    
    // For a length-L cycle, find the optimal step k where
    // all sub-cycles have length ≤ 4.  For 12: gcd(12,k) * L/gcd(12,k) = 12.
    // Sub-cycle length = L / gcd(L, k).  We want ≤ 4.
    // For L=12: k=4 gives sub-cycles of length 3. k=3 gives length 4.
    // Pick k that maximizes SCEV friendliness (smaller sub-cycles).
    Some(optimal_step_for_length(max_len))
}
```

### 2.2 Rotation-Aware Latch Emission

When `rotation_step > 1` is detected, the latch (emit_countable_latch)
uses step-k backedges instead of step-1 backedges.

**Normal (step=1):**
```
%be_p0 = add i64 0, %phi_p1    ; p0 → p1
%be_p1 = add i64 0, %phi_p2    ; p1 → p2
...
%be_p11 = add i64 0, %phi_p0   ; p11 → p0
```

**Rotation (step=4):**
```
%be_p0 = add i64 0, %phi_p4    ; p0 → p4 (cycle: p0→p4→p8→p0, length 3)
%be_p1 = add i64 0, %phi_p5    ; p1 → p5
%be_p2 = add i64 0, %phi_p6    ; p2 → p6
%be_p3 = add i64 0, %phi_p7    ; p3 → p7
%be_p4 = add i64 0, %phi_p8    ; p4 → p8
%be_p5 = add i64 0, %phi_p9    ; p5 → p9
%be_p6 = add i64 0, %phi_p10   ; p6 → p10
%be_p7 = add i64 0, %phi_p11   ; p7 → p11
%be_p8 = add i64 0, %phi_p0    ; p8 → p0 (cycle: p8→p0→p4→p8, length 3)
%be_p9 = add i64 0, %phi_p1    ; p9 → p1
%be_p10 = add i64 0, %phi_p2   ; p10 → p2
%be_p11 = add i64 0, %phi_p3   ; p11 → p3
```

### 2.3 Body Unrolling (in `emit_countable_main`)

The body must be unrolled `step` times per loop trip since each body
copy processes one original iteration's rotation step:

```rust
fn emit_countable_body_rotated(
    &mut self, out: &mut String, body: &[Statement], step: usize
) {
    for _ in 0..step {
        // Clear caches for each copy
        self.fun.let_bindings.clear();
        self.fun.let_binding_types.clear();
        self.fun.reg_float_cache.clear();
        self.fun.reg_type_cache.clear();
        self.fun.expr_dedup_cache.clear();
        self.fun.terminated = false;
        self.fun.loop_exit_label = Some("done".into());
        
        // Emit body
        for s in body {
            if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                self.emit_stmt(out, s, "  ");
            }
        }
        
        // Update ssa_old caches from pending_phi_native_backedge so the
        // next body copy sees the just-computed values (not stale phi regs)
        for (name, typed_reg) in &self.fun.pending_phi_native_backedge {
            let Some(&idx) = self.ctx.field_index_map.get(name) else { continue; };
            let ty = &self.ctx.field_types[idx];
            if ty == "float" || ty == "double" {
                self.fun.ssa_old_float_regs.insert(name.clone(), typed_reg.clone());
            } else {
                self.fun.ssa_old_int_regs.insert(name.clone(), typed_reg.clone());
            }
        }
    }
    self.fun.loop_exit_label = None;
}
```

### 2.4 Counter Adjustment

The counter phi increments by `step` instead of 1:

```llvm
%pn_cnt = add i64 %pi_cnt, <step>  ; step=4
```

### 2.5 Remainder Loop

For `N % step` remaining iterations (0..step-1), emit a separate remainder
loop with step=1 (normal A005c) and 12-cycle phis. Since the remainder runs
at most `step-1` iterations (≤3 for step=4), the 12-cycle is acceptable
(LLVM fully unrolls tiny loops).

The remainder loop runs from the LAST full-trip counter value to the bound.
Its header phi uses the FINAL values from the main loop as initial values.

## 3. Implementation Files

**Only `loop_engine.rs` needs changes** — the fix is entirely within A005c:

- `emit_countable_main` (line 1278): Add rotation detection, body unrolling,
  counter adjustment, remainder loop emission
- `emit_countable_latch` (line 1114): Accept optional `step` parameter for
  step-k backedges
- New helper functions:
  - `detect_rotation_step()` — analysis
  - `find_permutation_cycles()` — cycle detection
  - `emit_countable_body_rotated()` — body unrolling

## 4. Verification

1. `cargo test --lib` — all 1398+ tests pass
2. `bash benchmarks/build_and_bench.sh --correctness` — all MATCH (including fannkuch_redux)
3. `bash benchmarks/build_and_bench.sh --runtime` — ratio improves from 1.65x to ≤1.10x
