# Arena Allocator: Cross-Function Sharing

**Date:** 2026-07-19
**Status:** Design — ready to implement

## Problem

`emit_arena_init` initializes arena slot pointers (`%arptr`, `%arend`, `%arbase`) as function-local allocas. When `Alloc#(8)` is called from a helper function (`memcmp`, `memcmp_loop`) called inside a txn, the helper can't access the txn's arena allocas — they're in a different function's stack frame.

## Symptom

Without the arena fix, every `Alloc#(8)` in the UTF8_ops benchmark falls through to `@malloc(8)`, even though the reactive txn has a bounded scope where arena allocation should work. The arena init is emitted in `@txn_work`, but `memcmp_loop` calls `Alloc#(8)` and can't see `%arptr0`.

## Architecture

Three approaches, ordered by preference:

### Approach A: Pass Arena Pointers as Hidden Parameters (Recommended)

**How it works:**
- `emit_arena_init` stores arena pointers in a struct stored on the txn function's stack
- The struct address is passed as an extra parameter to helper functions
- Helper functions (`memcmp_loop`, `memcmp`) receive `ptr %arena_state` parameter
- All helper callees that need `Alloc#` read arena pointers from this parameter

**Changes:**
1. `emit_callable_txn` — if the caller has arena slots, add a hidden `ptr %arena_state` parameter to the function signature
2. All call sites of callable txns — pass the arena state pointer
3. `emit_alloc` / `emit_arena_alloc` — if `arena_slots` is `None`, try reading from a known global or parameter

### Approach B: Growable Arena Global

**How it works:**
- Arena buffer is allocated on the heap (once, at startup)
- Pointers stored in module-level globals (`@__arena_ptr`, `@__arena_end`, `@__arena_base`)
- Bump pointer operations use atomic CAS on globals (for async safety)
- Any function can access the arena via the globals

**Changes:**
1. Replace per-function arena allocas with module-level globals
2. `emit_arena_init` initializes the globals once
3. `emit_arena_alloc` operates on globals directly
4. `emit_arena_fini` frees the arena buffer

**Tradeoffs:** No parameter plumbing needed. Slightly more expensive (global access vs alloca). Simpler implementation.

### Approach C: Arena in State Struct

**How it works:**
- Add arena pointer fields to `%State` struct
- Initialize in `init_state` function
- All txns and helpers receive `%state` pointer — can access arena fields

**Changes:**
1. Add `arena_ptr`, `arena_end`, `arena_base` as system state fields (not user-facing)
2. Modify `emit_arena_init` to store pointers in `%State` instead of allocas
3. All functions already have `ptr %state` — no parameter changes needed

**Tradeoffs:** Adds 3 fields to `%State` (24 bytes). Works automatically for all functions. Simplest implementation.

## Recommendation: Approach C — Arena in %State

This is the simplest approach because every function already receives `ptr %state`. Adding arena fields to `%State` requires:

1. Add 3 system fields to `%State` (hidden from user, managed by the runtime)
2. `emit_arena_init` → store ptrs in these fields
3. `emit_arena_alloc` → load ptrs from these fields
4. No parameter changes to any function signatures

## Implementation Plan

### Step 1: Add arena fields to %State

In `push_field_type` or a new `add_system_fields` function, reserve 3 extra i64 slots at the end of `%State`:

```rust
// After processing all user fields:
let aptr_idx = self.ctx.field_types.len();
self.ctx.field_types.push("i64".to_string()); // arena pointer
self.ctx.field_briev_types.push(Type::int());
self.ctx.field_types.push("i64".to_string()); // arena end
self.ctx.field_briev_types.push(Type::int());
self.ctx.field_types.push("i64".to_string()); // arena base
self.ctx.field_briev_types.push(Type::int());
```

### Step 2: Store arena ptrs in %State during init

Modify `emit_arena_init` to GEP+store to %State fields instead of alloca:

```rust
// Instead of:
//   %arptr0 = alloca i8*
//   store ptr %arinit0, ptr %arptr0
// Emit:
//   %aptrgep = getelementptr %State, ptr %state, i32 0, i32 ARENA_PTR_IDX
//   store i64 %arinit0_i64, ptr %aptrgep
```

### Step 3: Load arena ptrs from %State during alloc

Modify `emit_arena_alloc` (`check_l` + `grow_l` blocks) to GEP+load from %State instead of using alloca names:

```rust
// Instead of:
//   %aacur = load ptr, ptr %arptr0
// Emit:
//   %aptrgep = getelementptr %State, ptr %state, i32 0, i32 ARENA_PTR_IDX
//   %aacur_ptr = inttoptr i64 %loaded to ptr
```

### Step 4: Clean up

Remove the alloca-based arena slot tracking (`arena_slots` field, `%arptr0`/`%arend0`/`%arbase0` allocas). All arena state is in `%State`.

### Impact

- UTF8_ops: `Alloc#(8)` uses bump pointer instead of `@malloc(8)` — expected ~50-100× speedup
- All other benchmarks: no change (arena is a no-op when `arena_slots` is `None`)
- Only reactive txns with called helpers that use `Alloc#` benefit
