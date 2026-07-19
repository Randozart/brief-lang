# Arena Allocator Rewrite — Cross-Function via %State

**Date:** 2026-07-19
**Status:** Plan — ready to implement
**Problem:** `emit_arena_alloc` uses closures capturing `&mut self`, causing borrow conflicts. Falls back to `@malloc` for all allocations.

## Root Cause

The previous approach used closures (`load_aptr`/`store_aptr`) that captured an intermediate counter `c` from `arena_counter`. When `restore_preserve_counters` reset `arena_counter` at scope boundaries, the counter wrapped, causing duplicate register names. Fixing the counter with `txn_counter` required closures capturing `&mut self`, which conflicted with other borrows.

## Fix

**Eliminate closures entirely.** Use `next_reg_with_prefix()` for ALL register names — it auto-increments `txn_counter` internally, requires no closure state, and is the dominant pattern in `counter.rs` (11+ uses). No `arena_counter` needed.

## Implementation Steps

### Step 1: Rewrite `emit_arena_alloc` (mod.rs ~1244)

Replace the current `@malloc` fallback with proper bump-pointer logic using `next_reg_with_prefix()`:

```rust
pub(crate) fn emit_arena_alloc(&mut self, out, indent, size_reg) -> String {
    let aptr_idx = self.arena_ptr_idx?;  // fallback to @malloc if no arena
    
    // Helper: GEP+load+inttoptr from %State field
    // All registers via next_reg_with_prefix — no closures
    
    // 1. Load current arena ptr and end from %State
    // 2. Bump check: cur + size ≤ end?
    // 3. If yes → phi, store new bump, return old ptr
    // 4. If no → grow: realloc(cur, max(size*2, 65536)), store back, retry
}
```

### Step 2: Rewrite `emit_arena_init` (mod.rs ~1397)

Replace `arena_counter` with `next_reg_with_prefix()` for all register names. The 4 format strings and 3 GEP names become `next_reg_with_prefix` calls.

### Step 3: Eliminate `arena_counter` from `FunctionContext`

Remove dead `arena_counter` field, its initialization, and its restore/reset logic.

### Step 4: Activate arena init in standard txn path

Add `self.emit_arena_init(out, "  ")` before the body emission in the standard reactive path (`emit_toplevel.rs:1456`). Now safe with `txn_counter`.

## Expected Impact

utf8_ops `Alloc#(8)` switches from `@malloc(8)` (~100ns each) to bump pointer (~2ns). With the already-working auto-inline + memcmp fast path, 50M iterations should go from 2.2s to ~0.02s.
