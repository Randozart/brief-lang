# Close Benchmark Gaps

**Date:** 2026-06-27
**Status:** Active

Five remaining benchmark gaps have been root-caused. This plan implements all fixes.

## Fix 1: nbody — float-typed phi registers dumped into int regs

**Root cause:** `loop_engine.rs:1145-1147` (during phi body emission) iterates
`init_regs` and dumps ALL registers into `ssa_old_int_regs`, including float
fields. When the body reads a float field, it looks up `ssa_old_float_regs`
(empty) → returns "0.0" → energy is always 0.

**Fix:** Classify phi regs by field type at the body entry point, mirroring
the `pre_extract_float_fields` pattern at lines 319-358. For each field name
in `init_regs`, check if its type is `Type::Float`; if so, insert into
`ssa_old_float_regs` instead of `ssa_old_int_regs`.

**Location:** `src/backend/llvm/loop_engine.rs`, around lines 1140-1150.

## Fix 2: queue_drain — push append element offset off by 2

**Root cause (primary):** `emit_expr.rs:3798` generates the append sequence
for `list <- val` as `header[old_len] = val`. But a list header is 2 slots:
`[capacity, len]`. The new element must go at `header[2 + old_len]`.

**Fix:** Change `old_len` to `add i64 %old_len, 2` in the store GEP.

**Root cause (secondary):** After push/pop updates the queue handle in `%State`,
the `pending_phi_backedge` is not updated for the queue field. The phi back-edge
(`%be_queue`) remains `add i64 0, %phi_queue` (pass-through), so the next tick
sees the OLD handle.

**Fix:** After the push/pop body stores the new handle, insert the GEP+load
sequence for the queue field into `pending_phi_backedge`.

## Fix 3: async_counters — post-tick exit check reads pre-tick state

**Root cause:** `emit_async_phase` at `emit_expr.rs:4851` passes `%state` to
`reactor_tick` instead of `%state_copy`. After `reactor_tick` returns, the
subsequent memcpy from `%state_copy` to `%state` restores the PRE-tick values.
Then the `#!exit` condition check reads these pre-tick values — the exit
condition never triggers because `done` never reaches its target.

**Fix:** Change `%state` to `%state_copy` in the `reactor_tick` call at
line 4851.

## Fix 4: precompute_sum — any_fired alloca missing in canonical loop path

**Root cause:** In `emit_ssa_main`, the `%any_fired = alloca i8, align 1` at
line 1050 is inside `if !has_canonical_loop`. But the `store i8 1, ptr %any_fired`
at lines 1184 and 1218 is emitted for ALL txn bodies, regardless of loop type.
When `has_canonical_loop` is true (per-field phi path), the alloca is missing
but stores reference it → LLVM verifier error.

**Fix:** Guard the `any_fired` stores at lines 1184 and 1218 with
`if !has_canonical_loop`. The canonical loop path uses the phi counter's
`icmp slt` check for loop exit, not `any_fired`.

## Fix 5: ring_buffer — guard reads phi register (pre-store) not stored value

**Root cause:** In the per-field phi path (`emit_ssa_main` canonical loop),
body statements use `ssa_old_int_regs` for field reads. On tick entry,
`ssa_old_int_regs` is populated with phi registers (pre-tick values). After
`&ops = ops + 1` stores the new value to `%State`, `ssa_old_int_regs["ops"]`
is NOT updated — it still contains the phi register. The guard
`[ops % 5000000 == 0]` reads the phi register (old ops, not the newly stored
value), producing incorrect output.

This matches C's `ops++` post-increment if it returned the old value — the
guard would read `ops` before increment, not after. But C checks `ops % k == 0`
AFTER `ops++`, so it reads the post-increment value.

**Fix:** In `emit_stmt.rs`, after the GEP+store for a state field, update
`ssa_old_int_regs[fname]` (or `ssa_old_float_regs[fname]` for float fields)
with the stored value register. This ensures subsequent body reads see the
new value, matching the SSA/extractvalue path behavior.

## Verification

```bash
cargo test --lib
bash benchmarks/build_and_bench.sh --correctness
bash benchmarks/build_and_bench.sh --runtime
```
