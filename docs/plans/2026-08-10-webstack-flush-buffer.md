# Webstack flush buffer — real update batches at term

**Date:** 2026-08-10
**Status:** active
**Related:** `docs/plans/2026-07-26-rendered-briv-webstack-v2.md` (Phase 4), `docs/architecture/features/rendered-briv-wasm.md` (the state_layout/flush contract), `src/glue/web_generator.rs` (JS shim).

## Problem

The webstack contract (rendered-briv-wasm.md) requires that at each transaction
commit (`term;`) the WASM module call

```
__web_flush_state(updates_ptr: i32, count: i32)
```

with a batch of `Update { field_handle: u32, value_ptr: u32, value_len: u32 }`
records, so the JS shim can apply DOM mutations. Today the call is a stub:

```
call void @__web_flush_state(i32 0, i32 0)   // ptr=0, count=0 — no updates
```

The comment at `src/backend/llvm/emit_stmt.rs:663` says "Phase 6 will wire the
actual flush buffer with modified fields" — that wiring was never done.

Consequences of the stub:
1. `_applyFlush(0, 0)` in the JS shim reads nothing — DOM never updates.
2. The `state_layout` header claims `flush_off=64, max_entries=16`, but no
   flush buffer exists at any known offset.
3. The generation counter (`@__web_generation`) is never incremented — the HMR
   / SSR contract that depends on it cannot work.

## Design

### Where the flush values live

The transaction bodies commit every state field write to `%State` inline
(direct GEP + store into `%state`), in every emission shape:

- standalone `@txn_<name>(ptr %state)` functions (emit_toplevel.rs:2156)
- the SSA main loop (ssa.rs) — `%state` alloca in `@main`
- the loop engines (counter.rs) — `emit_state_store_i64_by_idx` stores to `%State`

`Statement::Term`/`EndProgram` are the single choke point (`emit_stmt.rs`), and
`%state` is always in scope there. So the flush is emitted at the Term arm.

### What gets flushed

The transition graph gives each txn's `write_set` (field *names*). Map to field
indices via `field_index_map`. For each field in the write_set (sorted by index
for deterministic IR):

```
%gep = getelementptr inbounds %State, ptr %state, i32 0, i32 <idx>
store i32 <handle>, ptr @__web_flush_buf + (i*12)
store i32 ptrtoint(%gep to i32), ptr @__web_flush_buf + (i*12 + 4)
store i32 <byte_size>, ptr @__web_flush_buf + (i*12 + 8)
```

then

```
call void @__web_flush_state(i32 ptrtoint(@__web_flush_buf to i32), i32 <count>)
```

The `value_ptr` points *at the `%State` slot*, which holds the committed value
(the JS shim reads by type tag: Int → `getInt32`, Float → `getFloat64`,
Bool → `getUint8`, String → `[len][bytes]` at that ptr). This matches the
existing `decoder_expr` in web_generator.rs.

### Buffer + header

Emit a module-level buffer (only when webstack enabled):

```
@__web_flush_buf = private global [<N> x { i32, i32, i32 }] zeroinitializer
```

where `N` = max write_set size (or a safe cap — number of fields). The
`state_layout` header's `flush_off` / `max_entries` are updated to describe this
buffer (offset via `ptrtoint(@__web_flush_buf to i32)` so it resolves at link
time; count = N).

### Generation counter

Increment `@__web_generation` after each flush call so the JS `generation`
getter observes the commit (HMR/SSR precondition).

## Work items

1. Add a helper `emit_web_flush_batch(out, indent, txn_name)` in the LLVM
   backend that: looks up the txn's `write_set` from the transition graph,
   maps field names → indices, sorts, emits the buffer stores + the call.
   Uses the existing `web_llvm_byte_size` for `value_len`.
2. Wire it at both stub sites (`Statement::Term`, `Statement::EndProgram` in
   emit_stmt.rs), replacing `call ... (i32 0, i32 0)`. Skip when webstack is
   disabled. Guard: if the txn has an empty write_set, emit `(0, 0)` (no
   changes) — the JS no-op path stays valid.
3. Emit `@__web_flush_buf` global + correct header `flush_off`/`max_entries`
   in the state_layout block (llvm/mod.rs ~3952). Increment `@__web_generation`
   after the flush call.
4. Tests:
   - IR test: webstack txn that writes `count` emits a `__web_flush_state`
     call whose first arg is `ptrtoint(@__web_flush_buf to i32)` and whose
     records cover the written field's handle/offset/size.
   - IR test: a txn with an empty body/empty write_set emits `(i32 0, i32 0)`.
   - Keep `test_webstack_emits_flush_state`/`test_webstack_emits_flush_at_term`
     green (they only assert `__web_flush_state` appears).
   - Existing `test_state_layout_emits_real_field_rows` still passes (header
     gains real flush_off/max_entries).
5. `cargo test --lib` full suite; update `docs/architecture/features/rendered-briv-wasm.md`
   if the contract details change (it already describes the record format).

## Notes

- The JS shim's `_applyFlush` already reads `{handle, val_ptr, val_len}` at
  12 bytes/record (web_generator.rs:202-227) — no JS change needed for the
  batch format itself. Binding-table wiring (`binding_to_js`) remains a
  separate placeholder concern.
- Determinism: write_set iteration must be sorted by field index (AGENTS.md
  HashMap rule).
- No new intrinsics, no new stdlib, no type-name matching — the write_set is
  frontend-provided analysis (frontend-driven dispatch).
