# Fix Four Pre-Existing Correctness MISMATCHes

Date: 2026-07-06
Status: Complete

## Summary

Four benchmarks had pre-existing MISMATCH status (not introduced by `nsw` changes).
All four now MATCH. The nbody_sqrt root cause was deeper than initially scoped:
3 surface bugs (ordering, buffer overflow, break-without-vec_field) masked a 4th
logic bug in the vector phi backedge value selection.

| Benchmark | Root cause (initial) | Root cause (actual) |
|-----------|---------------------|---------------------|
| **bit_clear** | Guard reads pre-tick `reg` | Same — fix via local variable |
| **fasta** | `is_output_call()` missing PutChar | Same |
| **sparse_dispatch** | Dual-alloca mismatch in modulo dispatch | Same — copy loop + threshold fix |
| **nbody_sqrt** | 3 vector-phi emission bugs | 4 bugs: 3 ordering/buffer/break + vector phi backedge uses per-field `pending_phi_native_backedge[name]` instead of accumulated `vector_phi_current[vec_phi]` — elements 1-3 stagnate |

## Changes by File

### `benchmarks/bit_clear.bv` (benchmark-level fix)

Replace:
```
&reg = reg & (reg - 1);
[reg % 100000 == 0] { print_int#(reg); };
```
With a local variable that captures the immediate (non-deferred) value:
```
let next_reg = reg & (reg - 1);
[next_reg % 100000 == 0] { print_int#(next_reg); };
&reg = next_reg;
```

Why: `node` defers `&reg = ...` writes to `term;`. The guard `[reg % 100000 == 0]` reads from the pre-tick `reg` cache (`ssa_old_int_regs`). No pre-tick value of `reg` (0x7FFF... → 0x4000...) is divisible by 100000. A `let` binding is evaluated immediately, so `next_reg` holds the post-update value.

### `src/backend/llvm/loop_engine.rs` — `is_output_call()` (fasta fix)

At line 3650, the `matches!` macro for `IntrinsicCall` only lists `Print`, `Println`, `PrintInt`, `PrintFloat`. `Intrinsic::PutChar` is missing. When the parser creates `Expr::IntrinsicCall { intrinsic: PutChar }` for `putchar#()`, `trace_live_fields` doesn't seed the LCG seed as live, and `filter_dead_assignments` removes `&seed = ...`.

Fix: Add `| Intrinsic::PutChar` to the match arm at `src/backend/llvm/loop_engine.rs:3650`.

### `src/backend/llvm/loop_engine.rs` — `emit_modulo_rotated()` (sparse_dispatch fix)

The A005c modulo-rotated dispatch creates two allocas:
- `%state_0` (chunk) — for SROA-friendly access via `emit_state_gep`
- `%state` (monolith) — for raw GEP access

`emit_inline_init_stores` writes init values to the **chunk** (via `emit_state_gep` routing when `main_body=true`). But guard loads in the rotated loop read from the **monolith** via raw GEPs in `emit_identifier`. The monolith is uninitialized stack garbage.

Fix: After `emit_state_allocas` and `emit_inline_init_stores`, add a copy loop that copies every field from the chunk `%state_0` to the monolith `%state`. This is a one-time ~360-byte copy at loop entry — negligible cost.

The copy loop iterates over all field indices, computing the chunk index and sub-index for each, then emits `load` from chunk and `store` to monolith.

### `src/backend/llvm/loop_engine.rs` — Vector phi emission (nbody_sqrt fix)

Bug 1 — **Ordering** (lines 1401 vs 1465): `last_val_temps` allocation at line 1401 checks `vector_phi_groups` to share `<4 x float>` allocas, but `vector_phi_groups` isn't populated until line 1465. All 30 fields get individual scalar `alloca float` (4 bytes each) instead of 6 shared vector allocas.

Fix: Move `build_vector_phi_groups()` call to BEFORE the `last_val_temps` allocation block (before line 1401).

Bug 2 — **Buffer overflow** (commit block ~line 1601-1623): After Bug 1, the commit block stores `<4 x float>` (16 bytes) into `alloca float` (4 bytes). 6 oversized stores corrupt 18 adjacent scalar allocas.

Fix: Automatically resolved by Bug 1 fix — when `last_val_temps` correctly creates `<4 x float>` allocas, the commit block's vector stores match the alloca sizes.

Bug 3 — **Break without setting vec_field** (line 2482): In `load_last_val_temps`, when a vector phi was already loaded, the `break` at line 2482 exits the inner for loop without setting `vec_field = Some(())`. The code falls through to a scalar load from the wrong alloca (which was corrupted by Bug 2).

Fix: Add `vec_field = Some(());` before `break;` at line 2482.

## Implementation Order

1. `benchmarks/bit_clear.bv` — simplest, benchmark-only
2. `src/backend/llvm/loop_engine.rs` — `is_output_call` (fasta, one line)
3. `src/backend/llvm/loop_engine.rs` — sparse_dispatch copy loop
4. `src/backend/llvm/loop_engine.rs` — nbody_sqrt 3 vector-phi bug fixes

## Verification

After each fix:
- `cargo test --lib` — all 1403+ tests pass
- `cargo build --release` — no warnings

After all fixes:
- `bash benchmarks/build_and_bench.sh --correctness` — all 4 previously-MISMATCH benchmarks show MATCH
- `bash benchmarks/build_and_bench.sh --runtime` — no regressions in other benchmarks
