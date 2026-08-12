# Memory Loop Dispatch — Investigation & Fix

**Date:** 2026-07-19
**Problem:** `emit_folded_memory_main` produces wrong output when dispatched for nbody_newton.
**Symptom:** Output is ~1.19e-18 or ~3e-29 instead of correct -0.169.

---

## Hypothesis

The memory loop was producing wrong output because it used a STALE binary (not rebuilt after dispatch was forced). The `needs_state_stores_in_body = true` + empty `write_set` combination is known to work correctly in `emit_countable_body` for state field stores. Let bindings are re-computed each iteration from state values, so there should be no cross-iteration dependency issue.

## Investigation Steps

### Step 1: Force memory loop dispatch

Add back the memory counter loop condition in mod.rs (non-pure path, line 2641 area):

```rust
} else if true {  // FORCE memory loop for testing
    self.fun.pending_post_hoist = post_hoist;
    self.warnings.push(format!("info: txn '{}' dispatched via memory counter loop (EmitMemoryCounter, {}/{} fields written)", &node.name, write_count, total_fields));
    self.emit_folded_memory_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, &body_stmts);
    true
```

### Step 2: Clean rebuild + compile nbody_newton

```bash
cargo build --release
rm -f benchmarks/nbody_newton.ll benchmarks/nbody_newton
BOUND=50000000 timeout 30 ./target/release/briev-compiler build benchmarks/nbody_newton.bv --out benchmarks
```

### Step 3: Check the IR

- **Bound loading:** grep for `fmb` — should load 50000000 from state[0]
- **Loop structure:** grep for `fm_loop`/`fm_body`/`fm_end` — should show proper loop
- **Periodic print guards:** grep for `cmgb` or `guard.then` — should not be empty
- **Phi backedge stores:** grep for `store.*cms` — should use native types (store float, not i64)
- **load_last_val_temps:** grep for `lvt`/`lvv` — should load with native types

### Step 4: Run the binary

```bash
BOUND=50000000 timeout 30 ./benchmarks/nbody_newton
```

Expected: ~6-8 seconds, output matching C reference.

### Step 5: If wrong output — deeper investigation

- Check if the periodic print guard body is empty (missing `__print_float` call)
- Check if `energy` is resolved via `last_val_temps` correctly
- Check if `last_energy = energy` store uses native type (store float)
- Check if `last_energy` load in swan song uses native type (load float)

### Step 6: After fix — remove the `true` condition, restore `write_density >= 0.8 && total_fields >= 8`

### Step 7: Run full correctness + timing comparison
