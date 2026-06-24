# Timing Bounds via Watchdog — Fully Proving Cycle/Seconds Constraints

**Date:** 2026-06-24
**Status:** Plan

## Goal

Integrate timing bounds (`cycles <= N`, `seconds <= N`) into the existing watchdog
bracket system (`?[cond]`, `?![cond]`, `?#`), with **compile-time proof enabled
at every level**.

## Semantics

| Syntax | Compile-time | Runtime |
|--------|-------------|---------|
| `?[timing <= N]` | Cost estimate from structural recursion. If provably ≤ N, **no runtime overhead**. If unprovable, insert runtime counter. | Cycle counter + bounds check + handler. |
| `?![timing <= N]` | Full proof attempt (same as `?`). Even if proven, **insert runtime counter anyway** (explicit double-check). | Same as `?` fallback. |
| `?#[timing <= N]` | **Maximum proof effort**: structural recursion + bounded counter analysis + SMT-ish coverage. If still unprovable, same runtime fallback. | Same as `?` fallback. |

All three **attempt compile-time proof**. The difference is effort and whether the
runtime guard is suppressed or required.

## Architecture

```
Proof Engine (src/proof_engine.rs)
  │
  ├── Structural Recursion Analysis
  │     └── CostEstimate { cycles: Range<u64>, seconds: Range<f64> }
  │         └── For each statement: add cost
  │         └── For each loop: compute bound × per-iteration cost
  │         └── For each intrinsic call: look up cost from intrinsic's ?# annotation
  │
  ├── Bounded Counter Analysis
  │     └── Simulate loop up to bound, measure actual cost in interpreter ticks
  │         (Already exists as the --optimize-budget precomputation path)
  │
  └── Comparison
        └── cost_estimate.cycles.max <= watchdog.cycles_bound → PROVEN
        └── cost_estimate.seconds.max <= watchdog.seconds_bound → PROVEN
        └── Otherwise → emit runtime counter
```

## Syntax

### On transactions

```brief
rct txn compute [i < N][i == N] ?[cycles <= 10000] {
    &i = i + 1; &acc = acc + data[i];
    term;
};

rct txn guarded [pre][post] ?![cycles <= 5000] {
    ...
};

rct txn heavy [pre][post] ?#[cycles <= 100000] {
    ...
};
```

### On intrinsics and frgn declarations

```brief
// Declare the intrinsic's own cost — used by the proof engine when this
// intrinsic appears in a transaction body.
intrinsic getenv_int#(name: String) -> Int ?# cycles <= 100;

intrinsic read_file#(path: String) -> Result<String, String> ?# cycles <= 50000;

intrinsic tty_read_key#() -> Int ?# cycles <= 10000;
```

### File-level default

```brief
// All transactions default to this timing bound unless overridden.
#!watchdog cycles 1000000;
// Or:
#!watchdog seconds 5;
```

## Cost Model (Structural Recursion)

Each Expr and Statement variant maps to a base cost in cycles:

| Construct | Base cost (cycles) |
|-----------|-------------------|
| `let x = <literal>` | 1 |
| `let x = <ident>` | 1 |
| `&x = <expr>` | 2 |
| Binary op (add, sub, mul, etc.) | 2 |
| `if [guard] { body }` | 2 + body |
| Loop `[i < N][i == N] { body }` | N × (body + 2) |
| Intrinsic call | intrinsic's declared cost |
| frgn call | frgn's declared cost (if annotated) or `u64::MAX` (unknown) |
| `term` / `term expr` | 1 |
| Contract pre/post check | 3 |

**Proving**: The proof engine walks the transaction body, summing costs.
For loops: if the bound `N` is compile-time known (const, env var captured at
compile time under `--optimize-budget`), use the concrete value.
If `N` is a runtime-only value, the cost estimate is `N × per_iteration` —
the proof engine checks if `N` has a declared upper bound (from a previous
contract or watchdog), and if so, uses that as the worst case.

## Files to modify

### Phase 1 — AST + Parser

| File | Changes |
|------|---------|
| `src/ast.rs` | Add `cycles_bound: Option<u64>`, `seconds_bound: Option<u64>`, `is_proven: bool` to `WatchdogSpec`. Add `TopLevel::WatchdogDirective { timing: TimingKind, bound: u64 }`. |
| `src/parser.rs` | Parse `?[cycles <= N]` / `?![seconds <= N]` / `?#[cycles <= N]` in `parse_contract()`. Parse `#!watchdog cycles <N>;` as `TopLevel::WatchdogDirective`. |

### Phase 2 — Proof Engine: Cost Model

| File | Changes |
|------|---------|
| `src/proof_engine.rs` | Add `CostEstimate { cycles: Range<u64>, seconds: Range<f64> }`. Implement `estimate_cost(stmts)`. For each intrinsic, look up its declared cost from `program.intrinsic_costs`. For loops, use the bound from the postcondition convergence contract. Compare against watchdog bound. Return `Proven` / `Unprovable`. |

### Phase 3 — Interpreter Runtime

| File | Changes |
|------|---------|
| `src/interpreter.rs` | Add `cycle_count: u64` and `watchdog_handler: Option<Box<dyn FnMut>>` fields. Increment `cycle_count` before each statement. Check against watchdog bound. On overflow: state rollback + error. Handle `seconds` bound via `Instant::elapsed()`. |

### Phase 4 — LLVM Backend Runtime

| File | Changes |
|------|---------|
| `src/backend/llvm/emit_toplevel.rs` | Add `%cycle_count` field to `%State` init. |
| `src/backend/llvm/loop_engine.rs` | Emit `%cycle_count = add %cycle_count, 1` at tick entry. Emit `icmp ule %cycle_count, <bound>`, branch to watchdog handler on overflow. |
| `src/backend/llvm/emit_stmt.rs` | Emit watchdog handler block (call to `@llvm.trap()` or user-defined exit). |
| `src/backend/llvm/dispatch.rs` | Pass bound to tick loop. |

### Phase 5 — Stdlib Annotations

| File | Changes |
|------|---------|
| `lib/std/env.bv` | `intrinsic getenv_int#(name: String) -> Int ?# cycles <= 100;` |
| `lib/std/time.bv` | Annotate intrinsics with cycle costs. |
| `lib/std/string.bv` | Annotate string intrinsics. |
| `lib/std/io.bv` | Annotate read/write intrinsics. |
| `lib/std/shm.bv` | Annotate shm/mmap intrinsics. |
| `lib/std/tty.bv` | Annotate TTY intrinsics. |
| `lib/std/process.bv` | Annotate spawn/* intrinsics. |

## Test Plan

- **Parser tests**: Parse `?[cycles <= 100]`, `?![seconds <= 5]`, `?#[cycles <= 1000]`, `#!watchdog cycles 100`
- **Proof engine tests**: `estimate_cost` returns correct sums for straight-line, loops, and intrinsic calls
- **Interpreter tests**: Cycle counter increments per statement, watchdog fires at bound
- **LLVM tests**: `%cycle_count` field exists in `%State`, bounds check branch emitted
- **Integration test**: Transaction with `?[cycles <= 100]` runs correctly, exceed-bounds triggers handler

## Execution Order

1. Phase 1 — AST + Parser ✅ ~1 session
2. Phase 2 — Proof Engine cost model ~1 session
3. Phase 3 — Interpreter runtime ~1 session
4. Phase 4 — LLVM Backend runtime ~1 session
5. Phase 5 — Stdlib annotations ~1 session
6. Test + Fix ~1 session
