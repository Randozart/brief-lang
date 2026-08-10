# Width-aware loop engines: %State Int slots = i{int_bits}

**Date:** 2026-08-10
**Status:** implemented (all work items done; verified `.bv → .wasm` via llc)
**Related:** `--int-bits` CLI flag (commit eb812be6, "WASM uses 32 to avoid BigInt"),
`docs/plans/2026-08-10-webstack-flush-buffer.md`, BUGS.md "wasm32 webstack: %State
i64 storage vs i{int_bits} arithmetic".

## Problem

The webstack wasm32 path (int_bits=32) emits invalid LLVM IR in the loop
engines. Root cause: three inconsistent width models coexist:

1. `%State` slots for flexible Int/UInt are hardcoded **i64** (`push_field_type`
   else-branch, llvm/mod.rs:1240).
2. Arithmetic/comparisons use `binop_int_type()` = **i{int_bits}** (i32 wasm32).
3. Loop engines hardcode **i64** for counter phis/backedges.

Result: `%flc6 = phi i64` feeding `add nsw i32` — llc rejects the module.
The models only agree at int_bits=64 (x86_64), which is why x86_64 is unaffected.

## Principle

`--int-bits` exists to set the target Int width. A flexible Int field's %State
slot SHOULD be `i{int_bits}` — exactly as exact-width ints already get native
iN (Int32 → i32) and floats get native float/double. Making flexible Int/UInt
slots `i{int_bits}` activates the loop engines' existing narrow-counter
machinery (counter_ty != "i64" → sext to i64 for bound compare, counter.rs:351)
and makes loads/stores/arithmetic agree at one width.

On x86_64 (int_bits=64) `i{int_bits}` = i64 — the emitted IR is byte-identical,
so benchmarks and the C backend are unaffected. The change only bites targets
with int_bits != 64 (wasm32 webstack, user `--int-bits 8/16/32`).

## Work items

1. ✅ `push_field_type` (llvm/mod.rs:1240): flexible Int/UInt else-branch emits
   `i{int_bits}` instead of hardcoded "i64". Bool/Ptr/String stay i64.
2. ✅ Removed the load-trunc hack from the Identifier arm — `load_field_type`
   now returns i{int_bits} for Int slots.
3. ✅ Loop engines: `emit_folded_loop` phi/backedge use `counter_ty`; per-field
   phi backedges store the native-width value (not the i64 box); the countdown
   and batch phis already used `counter_ty`.
4. ✅ Store paths: array field store (`emit_array_state_store`) + row-view GEPs
   widen the index via the new `gep_index` helper (i{int_bits} → i64).
5. ✅ `web_llvm_byte_size` already parsed `iN` (reports 4 for i32) — the flush
   value_len for Int is now 4, matching the JS shim's `getInt32`.
6. ✅ llc-validated webstack `.bv` → `.wasm` end-to-end (folded, per-field phi,
   countdown, direct-SSA, runtime-bound; Int/Float/String/array fields).
7. ✅ Tests: `test_webstack_state_int_slots_are_target_width`,
   `test_webstack_folded_loop_is_width_consistent`,
   `test_webstack_array_field_store_gep_widens_index` (1738 total).
8. ✅ Full suite green (x86_64 byte-identical); docs + BUGS.md updated; Praetor
   clean on changed files.

## Why not keep %State i64 and truncate at every loop boundary

That would need trunc/zext at 40+ load/store sites (rule 16: DRY violation) and
leave the loop engines at i64 while the body arithmetic is i32 — patching
symptoms. Making the slot width match arithmetic is the single source of truth:
the casting graph's `resolve_llvm_type` already returns `i{int_bits}` for Int,
so `%State` slots, `binop_int_type()`, `llvm_type(Int)`, and loop SSA all agree.

## Regression guard

- x86_64 full suite must stay green (int_bits=64 → identical IR).
- llc must validate every webstack loop shape.
- Existing `test_webstack_*` tests keep passing (they assert emission, not
  exact widths — audit each for hardcoded `i64` assumptions).
