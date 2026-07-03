# LLVM IR Builder Migration Plan

## Current state

The builder (`builder.rs`, 738 lines) has typed methods for most IR instructions:
arith (add/sub/mul/div), memory (alloca/load/store/gep), control flow
(br/cond_br/switch/ret/phi/select), calls, conversion (trunc/zext/bitcast),
and labels.  It also has a `writeln` bridge and `emit_raw` for gradual migration.

The builder is used in `expr/rest.rs` (BinaryOp, UnaryOp, Literal emission) and
the `Foreach` statement.  Everywhere else uses raw `writeln!`.

## Why not a piecemeal conversion

The builder auto-generates register names (`%t{N}`) that are unique per builder
instance.  Existing code pre-determines register names (e.g. `%phi_count`,
`%be_count`, `%pi_cnt_650`) because they must match across basic blocks.

Converting a SINGLE basic block (e.g. the latch block) to use the builder
forces `emit_raw` for every instruction (to match the pre-determined names).
`emit_raw` is just a reformatted `writeln!` — it adds the builder's
instruction-vec overhead without any of the naming benefits.

## Strategy: convert one function at a time

Each function converts fully to the builder.  The builder generates ALL
register names for that function.  Names are consistent across blocks
because the same builder instance produces them all.

Order of conversion (smallest/least dependent first):

| Order | Function | Lines | Builder methods needed | Blocks |
|-------|----------|-------|----------------------|--------|
| 1 | `emit_countable_latch` | 42 | add, gep, load, label, br, raw | 1 |
| 2 | `emit_countable_main` | 70 | add, phi, icmp, br, gep, load, store, call | 4 (entry, pre_phi, loop_hdr, body, latch) |
| 3 | `emit_hoisted_post_loop_prints` | 20 | load, store, call | 1 |
| 4 | `emit_ssa_canonical_loop_setup` | 85 | gep, load, add, icmp, phi, br | 3 |
| 5 | `emit_countable_body` | 25 | calls via emit_stmt | 1 |
| 6 | `emit_folded_main` | 75 | add, phi, br, sub, icmp | 3 |
| 7 | `emit_folded_memory_main` | 100 | gep, load, add, store, br, icmp | 3 |
| 8 | `emit_ssa_main` (remaining) | 450 | all methods | 10+ |

Each function gets its own builder instance, used for the duration of that
function.  The builder's `finish_into(out, indent)` flushes instructions
at the end.  Any interleaved `writeln!` calls exist outside the builder
(i.e., before or after the builder's scope, not mixed).

## Register naming with the builder

The builder uses `%t{N}` for all registers.  Existing code uses
`%phi_count`, `%be_{name}`, `%pi_cnt_{N}`, etc.  The functional register
names are stored in `HashMap`s like `phi_field_regs` — the HashMap maps
field name → register name, regardless of the format.  The builder's
`%t{N}` names go into these HashMaps just as easily as `%phi_{name}`.

The only difference is LLVM IR readability — `%phi_count` is more readable
than `%t42`.  The builder has `gen_reg_with_prefix("phi_")` which produces
`%phi_42` instead of `%t42`.  This can be used for phi registers to keep
debuggable names.

## Observation: the builder and opaque pointers

The builder uses `ptr` (LLVM 15+ opaque pointers).  The existing code uses
typed pointers (`i64*`, `float*`, `double*`).  LLVM 18 auto-upgrades typed
to opaque during `opt`, so the mix is safe.  But consistency demands that
once a function is converted to builder, the ENTIRE function uses opaque
pointers.  This means all `float*` → `ptr` conversions happen at conversion
time.

This is the main engineering effort: every load, store, and gep format
string must change from typed to opaque pointer syntax.  The typed methods
on the builder already do this (emit_load uses `load i64, ptr %p` not
`load i64, i64* %p`), so it's handled automatically.

## Immediate next steps

The builder migration is deferred — it's a separate project that converts
one complete function at a time.  For the current session, continue with
traditional extraction on `emit_ssa_main`:

1. Extract the three-way txn dispatch branches into helpers:
   - `emit_ssa_txn_canonical` (phi_induction_reg.is_some)
   - `emit_ssa_txn_with_precond` (precondition not Bool(true))
   - `emit_ssa_txn_no_precond` (precondition is Bool(true))
2. These extract the body emission + cache setup + ssa_old tracking
   into single-purpose helpers.

This gives us depth reduction now and clean function boundaries for the
builder migration later (each helper converts independently).
