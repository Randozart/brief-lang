# Webstack flush buffer — real update batches at term

**Date:** 2026-08-10
**Status:** active
**Related:** `docs/plans/2026-07-26-rendered-briev-webstack-v2.md` (Phase 4), `docs/architecture/features/rendered-briev-wasm.md` (the state_layout/flush contract), `src/glue/web_generator.rs` (JS shim).

## Problem

The webstack contract (rendered-briev-wasm.md) requires that at each transaction
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
Bool → `getUint8`, String → dereferences the stored pointer to `[len][bytes]`).

### Buffer + header

Emit a module-level buffer (only when webstack enabled):

```
@__web_flush_buf = private global [<N> x { i32, i32, i32 }] zeroinitializer
```

where `N` = max write_set size. The `state_layout` header's `flush_off` /
`max_entries` are updated to describe this buffer (offset via
`ptrtoint(@__web_flush_buf to i32)` so it resolves at link time; count = N).
The struct TYPE stays plain `i32` fields — `ptrtoint` belongs only in the
initializer body (a `ptrtoint` in the type position is invalid LLVM).

### Generation counter

Increment `@__web_generation` after each flush call so the JS `generation`
getter observes the commit (HMR/SSR precondition).

### Binding table wiring (2026-08-10, second commit)

`FieldLayout` gains a compile-time `name`; `LlvmBackend::web_state_layout()`
builds the Rust-side layout with names/handles matching the WASM table. The
webstack codegen arm captures it (`codegen` out-param), and compile_source
passes it to `GlueWebGenerator` (falling back to the hardcoded stub when no
webstack codegen ran). `binding_to_js` maps a binding's signal (a Briev field
name) to the field handle and emits a real `applyFn` override (Text →
`textContent`, Show/Hide → `display`, Trigger → `addEventListener` calling the
txn export). `_makeBinding` stores a type-aware `decode`; `_applyFlush` uses it
instead of blind `TextDecoder`.

## Work items

1. ✅ `emit_web_flush_batch` helper + wiring at Term/EndProgram (committed
   `fda40ca6`).
2. ✅ `@__web_flush_buf` global + real header + generation increment (same).
3. ✅ FieldLayout `name` + `web_state_layout()` + codegen capture.
4. ✅ Binding table wiring (`binding_to_js` real DOM ops, type-aware decode).
5. ✅ Tests (flush records, empty no-op, header, names, binding wiring, decode).
6. `cargo test --lib` full suite; docs updated.

## Notes

- The JS shim's record format (`{handle, val_ptr, val_len}` at 12 bytes) was
  already correct in `_loadStateLayout`/`_applyFlush`; the decode was the gap.
- Determinism: write_set iteration sorted by field index (AGENTS.md HashMap rule).
- No new intrinsics, no new stdlib, no type-name matching — the write_set is
  frontend-provided analysis (frontend-driven dispatch).
