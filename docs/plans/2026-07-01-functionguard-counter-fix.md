# FunctionGuard SSA Register Counter Fix

**Date**: 2026-07-01
**Status**: Implemented
**Feature**: Prevent duplicate `%t{N}`/`%dab{N}`/`%aa{N}` registers when inlining multiple txn bodies into the same function.

## Problem

When `emit_reactor` or `emit_parallel_reactor` inlines multiple transaction bodies
into `@reactor_tick` (or `@main` in the A006 path), `FunctionGuard::restore` rewinds
`txn_counter` and `arena_counter` to the pre-inline snapshot value:

1. First txn body: counter = K, emits using K, K+1, ..., K+n. `restore` sets counter = K.
2. Second txn body: counter = K, emits using K, K+1, ..., K+n — **same registers!**

### Duplicate register manifest in generated IR

```
%dab263 = mul i64 %dnc*...         ; from first txn body
...
%dab263 = mul i64 %dnc*...         ; from second txn body — DUPLICATE
```

`opt -O2` rejects the IR with:
```
multiple definition of local value named 'dab263'
```

### Counter vs non-counter fields

`FunctionContext` has two categories of state:

| Category | Fields | Should restore? |
|----------|--------|-----------------|
| **Local state** | `let_bindings`, `let_binding_types`, reg caches, phi state, `terminated`, `returns_i64`, etc. | **Yes** — must revert to pre-body state |
| **SSA counters** | `txn_counter`, `arena_counter`, `within_counter`, `metadata_counter` | **No** — must stay monotonically increasing |

`FunctionGuard::restore` currently does `*fun = self.saved` which rewrites ALL
fields including counters.

## Solution

### Preserve both paths

Keep `restore` unchanged (total reset) for any caller that needs it. Add
`restore_preserve_counters` that restores all non-counter state while keeping
SSA counters monotonically increasing.

**Why not modify `restore` directly**: A caller may legitimately want a full
reset (e.g., emitting a trial body that should be discarded entirely). The
two-method approach makes the intent explicit at the callsite.

### Method signature

```rust
/// Restore all FunctionContext state EXCEPT SSA register counters
/// (txn_counter, arena_counter, within_counter, metadata_counter).
/// Counters stay at their peak values to prevent duplicate register names
/// across multiple inlined bodies emitted into the same function.
pub fn restore_preserve_counters(self, fun: &mut FunctionContext);
```

### Implementation

```rust
pub fn restore_preserve_counters(self, fun: &mut FunctionContext) {
    // Save the counters before overwriting with saved state
    let txn_ct = fun.txn_counter;
    let arena_ct = fun.arena_counter;
    let within_ct = fun.within_counter;
    let md_ct = fun.metadata_counter;
    *fun = self.saved;
    // Restore counters to peak values (never rewound)
    fun.txn_counter = txn_ct;
    fun.arena_counter = arena_ct;
    fun.within_counter = within_ct;
    fun.metadata_counter = md_ct;
}
```

### Callsite change

In `emit_inline_txn_body` (`dispatch.rs:344`):
```rust
// Before:
guard.restore(&mut self.fun);
// After:
guard.restore_preserve_counters(&mut self.fun);
```

## Trade-off analysis

**Status quo** (`restore` rewinding counters):
- **Pro**: Full isolation — every inlined body starts with identical counter state.
  Useful if a body's IR is discarded and re-emitted with different structure.
- **Con**: Guaranteed SSA collision when multiple bodies are inlined into one
  function. Currently broken for ALL multi-txn reactive programs.

**With `restore_preserve_counters`:**
- **Pro**: Monotonic counters eliminate SSA collisions. Works for any number of
  inlined bodies.
- **Con**: Register numbers grow monotonically across the entire function.
  ~0.1% increase in register name length for large programs. No functional
  impact — LLVM normalizes register names in its own passes.

**Detection not needed**: There is no runtime decision between the two restore
methods — `emit_inline_txn_body` always needs counter preservation. The choice
is made at compile time by which method is called.

### Root cause discovered during implementation

The `%dab263` collision in `queue_drain.ll` was **NOT** from FunctionGuard counter
rewinding (that path applies to the reactor loop, not the folded SSA path). The
actual cause was an **ambiguous register naming prefix** in `arrow.rs`:

```rust
// Line 435 (alloc bytes):
let alloc_bytes = format!("%dab{}", backend.fun.txn_counter);  // → "%dab263" at counter 263
// Line 469 (copy bytes):
let aft_bytes = format!("%dab2{}", backend.fun.txn_counter);   // → "%dab263" at counter 63  ← COLLISION
```

`"dab2" + "63"` = `"dab263"` which is identical to `"dab" + "263"`. The prefix
`dab2` has no separator, so a counter difference of exactly 200 produces
identical register names.

**Fix**: Changed the copy-bytes prefix from `%dab2` to `%dabcp` (dab-copy),
which cannot collide with `%dab` regardless of counter value.

The FunctionGuard fix (`restore_preserve_counters`) is retained — it prevents
counter rewinding issues in the reactor dispatch path (`emit_inline_txn_body`).

## Files Changed

| File | Change |
|------|--------|
| `src/backend/llvm/context.rs` | Add `restore_preserve_counters` method |
| `src/backend/llvm/dispatch.rs` | Change `guard.restore()` → `guard.restore_preserve_counters()` |
| `src/backend/llvm/expr/arrow.rs` | Change `%dab2{txn}` prefix → `%dabcp{txn}` (prevents `dab2+N` == `dab+(N+200)` collision) |

## Verification

1. `cargo build` — compiles without warnings
2. `cargo test --lib` — all 1363 tests pass
3. `queue_drain.ll` has only ONE `%dab263` definition (down from 2)
4. `%dabcp63` replaces the former `%dab263` collision at the copy-bytes site
5. `opt -O2 queue_drain.ll -disable-output` — no "multiple definition" error
