# Benchmark Gap Fix Plan (2026-07-01)

## Overview

After the runtime benchmark run (BOUND=50000000, 5 iterations), 8 of 18 benchmarks
trailed C. This document analyzes root causes and prescribes fixes for each,
following AGENTS.md rules: additive-only, dual-path where workloads diverge,
never weaken optimization for the test case alone.

---

## P0: emit_neg Float64 Gap (math.rs:59-63, call.rs:21-24)

### Root Cause

`emit_neg` checks `inner.ty == Type::Float` (32-bit) but not `Type::Float64`.
For Float64 operands it emits `sub i64 0, %double_val` — invalid LLVM IR.
The same bug exists in `call.rs` for the `negated` intrinsic.

### Impact

Every `-(x)` on a Float64 value produces `sub i64 0, <double>`, which:
1. Fails `llc` verification (type mismatch)
2. Prevents all subsequent LLVM optimization passes on the function
3. Affects nbody_newton (~30 occurrences), nbody_sqrt (~38), and any Float64 negation

### Fix

Add `Type::Float64` check to emit the correct `fneg double` instruction.
Scoped to math.rs emit_neg and call.rs negated intrinsic.

### Architectural Note

This is NOT a general type-dispatch fix — it is a specific missing-arm patch.
The `fneg` approach mirrors clang's behavior: LLVM has a single `fneg` that
works for both `float` and `double` (type is encoded in the operand, not the
opcode string).

---

## P0: --gc-sections in build_and_bench.sh (line 134)

### Root Cause

The C-linking path (line 134) uses `clang -O3 filename.ll` but does not pass
`-Wl,--gc-sections` or `-fdata-sections -ffunction-sections`. Unused functions
(string builder, runtime init, signal handlers) survive to the binary.

### Impact

.text section is 15-21x larger than C for simple benchmarks (fannkuch_redux,
mandelbrot). The L1I cache (32KB on x86_64) is filled entirely by Briv's
.text, causing cache pressure on every function call (like fprintf).

### Fix

Add `-fdata-sections -ffunction-sections -Wl,--gc-sections` to the clang link
invocation at line 134. This tells the linker to eliminate unused sections.

### Architectural Note

`--gc-sections` works at the section level. Since `clang -O3` places each
function in its own section when `-ffunction-sections` is enabled, this
effectively provides dead-function elimination. It does NOT eliminate dead
code within a function — that's LLVM's job. The Briv backend should still
emit clean IR; gc-sections is a safety net, not a substitute.

**Dual-path consideration**: For small programs (< 10 functions), the
gc-sections overhead is negligible. For large programs with dynamic dispatch,
section-based layout may interact poorly with hot/code splitting. Since we
control the linker invocation, we can add `--gc-sections` unconditionally —
it's harmless for non-section-split binaries and beneficial for all others.

---

## P1: Expression Hash-Consing Dedup (helpers.rs:917)

### Root Cause

`emit_binop` (helpers.rs:917) calls `emit_expr` independently for each operand.
For `dsq01 = dx01*dx01 + dy01*dy01 + dz01*dz01`, the sub-expression `dx01*dx01`
may be evaluated multiple times across different parent expressions. There is
NO sharing of emitted registers for identical sub-expression trees.

In nbody_newton.ll, the same `dxe23*dxe23` appears **4 times** in the IR —
3 of those are dead code that LLVM must eliminate.

### Impact

- 2-4x redundant fmul/fadd instructions in the IR for FP-heavy benchmarks
- IR bloat: nbody_newton.ll is 650KB vs ~100KB if deduped
- Extra compile time for LLVM to CSE the redundant instructions
- ~0.10x throughput gap on nbody family

### Fix

Add a per-expression-frame hash-consing table in `LlvmBackend`:
- Key: `(op_code, lhs_reg, rhs_reg)` or `(op_code, inner_reg)` for unary
- Value: register name already emitted
- Lifetime: scoped to a single `emit_expr` call (cleared at entry)
- Only cache "expensive" operations: fmul, fadd, fdiv, sdiv, srem (not add/mul on i64)

The caching table is a `HashMap<(u64, String, String), String>` on the
`FunctionContext`. The hash key encodes:
- The operation (as a discriminant)
- The LHS register name
- The RHS register name (or empty for unary ops)

### Architectural Note

**Dual-path**: Cheap ops (i64 add/mul) should NOT be cached — the table
lookup overhead exceeds the cost of recomputing them. Only cache ops where
the operation string is ≥ 4 chars OR the type is Float/Float64. This is a
compile-time cost heuristic: we estimate `hashmap lookup + insert ~= 50ns`,
vs `fmul latency ~= 4 cycles ~= 2ns`. On balance, caching even cheap ops
costs ~25x more in compile time than it saves in runtime. But for FP ops
that prevent vectorization, the benefit is in IR compactness and LLVM's
ability to find SIMD patterns, not just instruction count.

**Key design rule**: The cache must be scoped to a single expression tree
evaluation, NOT the entire function. Cross-expression caching would require
 dominance analysis to check if the cached value is still live — which is
LLVM's job, not ours. Our goal is to avoid emitting the same instruction
twice within one expression tree.

---

## P1: Modulo-Switch Dispatch (loop_engine.rs:1116)

### Root Cause

When N reactive txns have preconditions of the form `count % K == {0, 1, ..., N-1}`,
the backend emits N sequential `br i1` checks instead of a single `switch i64`.
Mutual exclusion analysis (`are_mutually_exclusive` in proof_engine.rs:3946) exists
but is never called during codegen.

In sparse_dispatch.ll: 24 GEP+load pairs (3 fields × 8 txns), 8 sequential
branches, ~14 basic blocks per iteration vs C's 3-4.

### Impact

- 1.06x gap on sparse_dispatch (8-way dispatch)
- For N-way dispatch: O(N) branches vs O(1) switch
- Redundant field loads: each txn re-loads the same 3 fields independently

### Fix

Add a new codegen path between the existing decision tree branches in mod.rs.

**Detection**: In the optimizer's strategy selection (optimizer.rs), after
checking for bounded_pre + increments, additionally check if the preconditions
form a modulo dispatch pattern:
1. All preconditions are `&&`-chains ending with `count % K == N`
2. The N values are complete and mutually exclusive (0..K-1 or a subset)
3. All bodies share the same counter increment pattern

**Emission**: New function `emit_modulo_switch_main` in loop_engine.rs that:
1. Hoists the modulo computation: `%mod_result = srem i64 %count, K`
2. Pre-loads all shared fields ONCE before the switch
3. Emits `switch i64 %mod_result, label %fallthrough [ i64 0, label %case_0, ... ]`
4. Each case block contains one txn body
5. All cases merge back to a single loop backedge
6. The `any_fired` flag is set per case but checked only at loop backedge

**Fallback**: If preconditions do NOT form a modulo pattern, fall through to
the existing sequential `emit_ssa_main`.

### Architectural Note

**Dual-path design**: There are three valid dispatch strategies for reactive txns:

| Pattern | Dispatch | When to use |
|---------|----------|-------------|
| `count % K == N` modulo cover | `switch i64 (count % K)` | All preconditions are modulo checks, values cover 0..K-1 (or a known subset) |
| Sequential bounded-pre | `br i1` chain | Txns have independent preconditions |
| Enum triggers | `switch i64 trigger` | Explicit `@link` trigger declarations with value sets ≤ budget |

The decision tree in mod.rs should try them in this order:
1. Trigger-based switch (existing)
2. Modulo-based switch (new)
3. Sequential SSA chain (existing, current fallback)

**Co-evolved field load hoisting**: When emitting modulo switch, the 3 shared
field loads (count, bound, cycle_count) must be emitted BEFORE the switch,
not per-case. This eliminates 21 of 24 GEP+load pairs for 8-way dispatch.

---

## P2: Ring Buffer for Steady-State Pop/Push (arrow.rs:361-447)

### Root Cause

`emit_arrow_discard` + `emit_arrow_push` always allocate + free + memcpy,
even when the list length is bounded and the pop/push pattern is steady-state
(one pop, one push per tick, length never exceeds 1). The stale SSA handle
issue compounds this: push reads `old_len=1` from the stale handle and allocates
for `(1+3)*8 = 32 bytes`, then memcpy's 8 bytes of garbage.

In queue_drain.ll: 2 arena allocs + 1-3 memcpy per tick, even though the queue
never holds more than 1 element. The C reference does zero memory operations.

### Impact

- 3.23x gap on queue_drain (the single largest gap)
- Every list drain pattern (pop-and-discard, pop-and-push) pays allocator tax
- The stale-handle issue also causes a double-free in the standalone function path

### Fix

Add a new `InsertStrategy::RingBuffer` variant and corresponding codegen paths.

**Detection**: The optimizer analyzes the access pattern to a list-typed field:
1. Is the field only accessed via `<-` (pop/discard) and `<-` (push)?
2. Is the maximum live element count bounded? (proved by contract or inferred
   from steady-state: pops == pushes per interval)
3. Is the access pattern FIFO/LIFO? (FIFO needs a ring buffer, LIFO can use
   a simple stack with slot reuse)

**The ring buffer layout** (for a 1-element queue):
- `{ i64 base_ptr, i64 head, i64 tail, i64 capacity }`
- Initial allocation: `malloc(32)` (4 × i64 header + 1 × data slot)
- Pop: `result = base_ptr[head]; head = (head + 1) & (capacity - 1)`
- Push: `base_ptr[tail] = value; tail = (tail + 1) & (capacity - 1)`
- No malloc/free/memcpy after the initial allocation

**The stack-with-slot-reuse layout** (for steady-state LIFO):
- `{ i64 ptr, i64 count }` where count oscillates 0/1
- Pop: if count == 0 → no-op (dead element), else count--, return ptr[count]
- Push: ptr[count] = val; count++ (no overflow check needed if proved bounded)

**Emission**: New path in `emit_arrow_discard`:
```rust
if let Some(rb_info) = self.fun.ring_buffer_info.get(field_name) {
    // Emit head = (head + 1) & (capacity - 1)
    // No memory operation needed
    return;
}
```

And new path in `emit_arrow_push`:
```rust
if let Some(rb_info) = self.fun.ring_buffer_info.get(field_name) {
    // Emit base_ptr[tail] = val; tail = (tail + 1) & (capacity - 1)
    return;
}
```

### Architectural Note

**Dual-path (critical)**: This optimization must NOT apply to all list accesses.
It is ONLY valid when:
1. The compiler can prove bounded liveness (no element lives longer than
   the pop/push interval)
2. The access pattern is exclusively pop+push (no random access, no persistent
   references)
3. The list type supports the ring buffer strategy

The `InsertStrategy` enum gains a new variant `RingBuffer`. The `check_insert_strategy`
function in emit_toplevel.rs checks if the field's type universe registration includes
`RingBuffer`. For the built-in `List<T>`, this strategy is selected when the optimizer
proves the above conditions.

**Capacity sizing**: The ring buffer capacity is always a power of 2 (so
`& (capacity - 1)` replaces `% capacity`). If the maximum live element count
is N, capacity = `1 << ceil_log2(N + 1)`. For the 1-element queue case,
N=1 → capacity=2.

**No stale-handle fix needed**: With ring buffers, the handle IS the field
state (head/tail pointers stored in the struct). The pop updates head in the
struct, and push reads tail from the same struct — no stale handle possible.

---

## Implementation Order

1. **P0: emit_neg Float64** — 1-line fix in math.rs, 1-line fix in call.rs
   Estimated impact: unblocks nbody_* benchmarks, correctness fix

2. **P0: --gc-sections** — 1 flag in build_and_bench.sh
   Estimated impact: 5-8% on I-cache-bound benchmarks

3. **P1: Expression hash-consing** — ~50 lines in helpers.rs + FunctionContext
   Estimated impact: 0.10x on nbody family, reduces IR bloat

4. **P1: Modulo-switch dispatch** — ~200 lines in loop_engine.rs + optimizer.rs
   Estimated impact: 0.05x on sparse_dispatch, matches C

5. **P2: Ring buffer codegen** — ~300 lines in arrow.rs + emit_toplevel.rs + type_universe.rs
   Estimated impact: eliminates 3.23x gap on queue_drain

---

## Pre-Commit Checklist

- [ ] `cargo test --lib` passes
- [ ] `cargo build` has no warnings
- [ ] Praetor on changed files
- [ ] Architecture comments in every changed file
- [ ] Benchmark results for affected benchmarks
