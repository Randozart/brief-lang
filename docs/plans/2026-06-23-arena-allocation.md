# Arena Allocation for LLVM Backend

**Date:** 2026-06-23
**Status:** Implemented (see architecture doc §20)
**Execution record:** implemented 2026-06-23 across all three phases.
Phase 1: per-scope bump arena with inline emit_arena_alloc.
Phase 2: contract-driven preallocation with capacity-aware push fast path.
Phase 3: cross-tick pool via pointer reset (arena_reset instead of arena_fini)
— see commits for the full change set.

## Motivation

Every `<-` push/pop/discard/transfer and string concat currently emits
`@free(@malloc(...)) + @memcpy`. For reactive transactions with N
iterations, this means O(N) malloc/free cycles on the hot path. Malloc
is optimized for general-purpose use (fragmentation, thread-safety, page
tables); Briev's semantics are far more constrained and can exploit that.

Three observations make this a natural fit:

1. **Transaction tick boundaries** define clear lifetimes. Every
   allocation during a tick dies at tick end.
2. **Contracts prove bounds**. `[i < N]` on a loop building a list
   gives the compiler an exact capacity ceiling.
3. **No shared ownership**. The `<-` arrow produces new buffers without
   GC, refcounting, or atomics.

## Inline Reactor Tick (2026-06-23, post-hoc fixes)

Three remaining gaps were closed:

**Enum dispatch arena:** `emit_folded_multi_main` (`loop_engine.rs:1189`)
now calls `emit_arena_init` at function entry and `emit_arena_fini`
before each `ret i32 0` exit point (4 ret sites). Covers all case arms,
the uniform-body path, and the residual `call @reactor_tick` fallback.

**Multi-txn SSA preallocation:** `emit_ssa_main` (`loop_engine.rs:887`)
now scans all txn bodies for `<- push` targets and preallocates using
the first txn's contract bound when `!has_canonical_loop`. Uses the new
`emit_prealloc_for_targets` helper that accepts a pre-collected list of
field names, shared with the single-txn path.

**Inline reactor tick:** Both `emit_reactor` and `emit_parallel_reactor`
(`dispatch.rs`) now call `emit_arena_init` at tick entry, `emit_arena_fini`
at tick exit, and `emit_inline_txn_body` instead of `call void @txn_name`.
The `emit_inline_txn_body` helper saves/restores `self.terminated`,
`self.let_bindings`, `self.let_binding_types`, `self.reg_float_cache`, and
`self.reg_type_cache` to prevent cross-txn contamination. This shares one
arena across all txns per tick (vs. N×64KB in Approach 2) and eliminates
the separate `@txn_name` function call overhead for the reactor path.
The `@txn_name` functions are still emitted (needed for callable txns) —
LLVM DCE removes the dead reactive ones.

## Phase 1: Per-Tick Bump Arena

Replace `@malloc`/`@free` in collection/string ops with a bump arena
scoped to one transaction tick.

### Implementation

**LLVM IR signature change** — The `%State` struct gains an arena pointer
field (or a separate `%arena` alloca is passed alongside):

```llvm
%state = alloca %State, align 8
%arena_ptr = alloca i8*, align 8
%arena_end = alloca i8*, align 8
; Before first tick:
%arena_base = call i8* @malloc(i64 65536)  ; 64KB per tick
store i8* %arena_base, i8** %arena_ptr
%end = getelementptr i8, i8* %arena_base, i64 65536
store i8* %end, i8** %arena_end
```

**Bump allocation helper** (inline IR, no function call):

```llvm
define internal i8* @bump_alloc(ptr %arena_ptr, ptr %arena_end, i64 %size) {
  %cur = load i8*, i8** %arena_ptr
  %new = getelementptr i8, i8* %cur, i64 %size
  %ov = icmp ugt i8* %new, %arena_end
  br i1 %ov, label %grow, label %ok
grow:
  ; Grow arena (double or fall back to malloc)
  %old_base = load i8*, i8** %arena_ptr
  call void @free(i8* %old_base)
  %new_base = call i8* @malloc(i64 max(65536, %size * 2))
  store i8* %new_base, i8** %arena_ptr
  %new_end = getelementptr i8, i8* %new_base, i64 max(65536, %size * 2)
  store i8* %new_end, i8** %arena_end
  ret i8* %new_base
ok:
  store i8* %new, i8** %arena_ptr
  ret i8* %cur
}
```

**At tick end** (just before backedge or return):

```llvm
%arena_base = load i8*, i8** %arena_ptr  ; reset to base
store i8* %arena_base, i8** %arena_ptr    ; free = pointer reset
```

No `@free` call per operation. The full tick's worth of allocations is
reclaimed with a single pointer store.

### Replacing Each Site

| Current pattern | Arena pattern |
|----------------|---------------|
| `@free(old)`, `@malloc((len+3)*8)`, memcpy | `%p = bump_alloc(size)`, memcpy |
| `@free(old)`, `@malloc((len+1)*8)`, memcpy | `%p = bump_alloc(size)`, memcpy |
| `@malloc(header+chars+1)`, memcpy | `%p = bump_alloc(size)`, memcpy |
| `@free(temporary)` in concat | ~~skip~~ — arena reset at tick end |

### Files to Modify

- `src/backend/llvm/loop_engine.rs` — add `%arena_ptr`/`%arena_end` allocas
  and `bump_alloc` emission in `emit_folded_loop`, `emit_folded_memory_main`,
  `emit_ssa_main`, and the reactor tick
- `src/backend/llvm/emit_expr.rs` — replace `@malloc`/`@free` calls in:
  - `ArrowMut::Push` (line ~3572)
  - `ArrowMut::Pop` (line ~3664)
  - `ArrowMut::Discard` (line ~3740)
  - `ArrowTransfer` (line ~3825)
  - `emit_inline_concat` (line ~4764)
  - Slice results (line ~3314)
  - Map/Set literals (line ~3500, 3525)
- `src/backend/llvm/emit_toplevel.rs` — emit `bump_alloc` declare, reset
  at tick end

### Test Plan

- Existing LLVM backend tests must pass unchanged (arena is a transparent
  replacement — same observable behavior)
- New test: `test_arena_reuse_in_push` — verify that after tick reset,
  the same arena memory is reused
- New test: `test_arena_string_concat` — verify concat doesn't free
  temporaries individually
- Benchmark: `benchmarks/build_and_bench.sh --runtime` — expect same or
  better throughput (fewer syscalls, better cache locality)

### Risks

- Arena size selection: too small → repeated grow+free+remalloc (worst
  than current per-op malloc); too large → memory bloat
  - **Mitigation**: start with 64KB default, grow-double on overflow,
    warn if arena exceeds 1MB per tick
- `@free` in existing string concat (tag-bit-driven) must be preserved
  for cross-tick scenarios — only tick-scoped allocations use arena
  - **Mitigation**: tag convention extended: bit 2 = arena-allocated.
    Arena-owned temporaries skip `@free`; heap-owned still freed.

## Phase 2: Contract-Driven Capacity Preallocation

When a loop contract provides an upper bound (`[i < N]`), preallocate
the collection at full capacity before the loop:

```briev
txn build_list(list: List<Int>, i: Int) [i < 100][i == 100] -> List<Int> {
    &list = list <- i;
    &i = i + 1;
    term list;
};
```

The compiler knows `list` will contain at most 100 elements. It can
`@malloc(102 * 8)` once before the loop instead of growing on every
`<- push`.

### Implementation

1. **Loop bound extraction** — already done in `region_analyzer` and
   `extract_ranges()` (`dispatch.rs:78-91`)
2. **Capacity annotation** — `field_types` gains a `capacity: Option<i64>`
   field for collection-typed state fields. Set during analysis when a
   bound is proven.
3. **Preallocation** — `emit_loop_metadata` or the loop entry block
   emits a single `@malloc(capacity * elem_size + header)` and writes
   the initial header. The loop body's `<- push` checks length against
   capacity (no realloc needed).

### When to Skip

- **Open-ended loops** (no contract bound) — fall back to current
  grow-on-push or use arena
- **Compile-time bound > 1M elements** — warn: large static prealloc
  may waste memory; arena path preferred

## Phase 3: Cross-Tick Arena Pool (Deferred)

Keep allocated arena pages alive across ticks instead of freeing them.
Reuse the same 64KB page pool without returning to the OS.

**Good for:** steady-state reactive programs with stable allocation
patterns (e.g., a game loop that builds and discards the same amount
of data each frame).

**Not good for:** one-shot programs, programs with variable allocation
over time (memory bloat).

Decision: implement Phase 1 first, measure, then decide if Phase 3
adds value over the simpler per-tick reset.

## Summary

| Phase | What | When |
|-------|------|------|
| 1 | Per-tick bump arena — replace `@malloc`/`@free` per op with pointer bump | Next sprint |
| 2 | Contract-driven preallocation — `@malloc` once per bounded loop | After Phase 1 |
| 3 | Cross-tick arena pool — reuse pages across ticks | Deferred |
