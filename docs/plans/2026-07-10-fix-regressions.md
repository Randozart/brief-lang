# Fix Regressions: Vector Phi Groups + A005c Dispatch Heuristic

## Issue 1: Vector Phi Groups in `emit_typed_store`

### Symptoms
nbody_newton, nbody_sqrt, nbody_sqrt_idio, queue_drain_idio fail with:
```
error: instruction forward referenced with type '<4 x float>'
```

### Root Cause
`emit_typed_store` (new in Phase 7) does not check `self.fun.vector_phi_groups`
before emitting a GEP+store. When a field like `px: Float` is part of a `<4 x float>`
vector group, `emit_memory_field_store` redirects to an `insertelement` instruction
(building the vector component by component). Without this check, the store goes to
`%State` directly, confusing the vector phi latch which expects the `<4 x float>`
accumulator to be in `pending_phi_native_backedge`.

### Fix
Add to the top of `emit_typed_store`, before the field_index_map lookup:

```rust
// 2026-07-10: Vector phi group — build vector via insertelement.
for (vec_phi, members) in &self.fun.vector_phi_groups {
    if let Some(comp_idx) = members.iter().position(|m| m == name) {
        let cur_vec = self.fun.vector_phi_current.get(vec_phi)
            .cloned().unwrap_or_else(|| vec_phi.clone());
        let ins = format!("%iv{}_{}", self.fun.txn_counter, &vec_phi[1..]);
        self.fun.txn_counter += 1;
        writeln!(out, "{} {} = insertelement <4 x float> {}, float {}, i32 {}",
            indent, ins, cur_vec, val.name, comp_idx).ok();
        self.fun.vector_phi_current.insert(vec_phi.clone(), ins.clone());
        self.fun.pending_phi_backedge.insert(name.to_string(), ins.clone());
        self.fun.pending_phi_native_backedge.insert(name.to_string(), ins);
        return;
    }
}
```

This mirrors the identical pattern at `emit_memory_field_store:42-54`.

## Issue 2: A005c Dispatch Heuristic for Complex Single-Txn Programs

### Symptoms
fannkuch_redux regresses from 0.99x (Briev wins) to 1.60x (C wins).
The program still produces correct output (MATCH), but performance is worse.

### Root Cause
The single-txn fold dispatch at `mod.rs:2240-2256` only checks two conditions:

```rust
let foldable = bounded_pre.is_some() && increments.is_some();
if foldable { folded = self.emit_countable_main(...); }
```

On main, `increments` was always `None` (OwnedRef invisible to `detect_increments`),
so `foldable` was always `false`, falling through to A006 (direct SSA loop with
full phi state). On the branch, `detect_increments` correctly returns `Some`,
making `foldable = true`, and the program enters A005c (`emit_countable_main`).

A005c is a per-field phi loop: only 1-2 fields get phi registers, and all other
fields are loaded/stored from `%State` each tick. For fannkuch (16 flat fields,
all read+written every iteration), this generates 14 loads + 14 stores per tick
that A006's full phi state doesn't need. With 50M iterations, that's 700M extra
memory operations.

### Fix
Add a field-count heuristic. If the single txn writes to more than 4 fields,
A005c is a poor fit — skip to A006 instead.

```rust
let foldable = bounded_pre.is_some() && increments.is_some();
let active_writes = self.region_analyzer.txn_writes.get(&txn.name)
    .map(|w| w.len()).unwrap_or(0);
let has_few_fields = active_writes <= 4;
if foldable && has_few_fields {
    folded = self.emit_countable_main(...);
}
```

The threshold of 4 was chosen because:
- ring_buffer, print_loop: 3 fields (counter, N, cycle_count) — fits A005c
- float_math: 13 fields — A006 is better
- fannkuch_redux: 16 fields — A006 is better
- cancel_math: 2 fields — fits A005c
- queue_drain: 2 fields — fits A005c
- interval_step: 2 fields — fits A005c
- nbody: 20+ fields — A006 is better, but also blocked by Issue 1

### Flat Control Flow

Both fixes are flat (max 2 nesting levels). The vector-phi-group loop is a
straight `for` loop with `if let` guard. The field-count heuristic is a single
`let` + `&&` condition with no nested control flow.

### Test Plan

1. `cargo test --lib` — 1444+ tests pass
2. `bash benchmarks/build_and_bench.sh --runtime` — all benchmarks MATCH
3. nbody_newton, nbody_sqrt, nbody_sqrt_idio compile to valid binaries
4. fannkuch_redux ratio returns to ~1.0x
