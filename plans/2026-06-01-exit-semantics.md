# Plan: Program Exit Semantics — `#!exit` Pragma + Natural Death

**Date:** 2026-06-01
**Status:** Planned

## Motivation

Reactive wake-hybrid programs (ring buffer, async counters) have no natural termination.
After all convergence chains complete, the main loop continues indefinitely:

```
tick:
  switch ... → case_done → do_wait → __rt_wait() → br label %tick
```

The `__rt_wait()` call blocks for 100ms, wakes on epoll timeout, the switch re-samples triggers,
hits the converged arm, routes to `do_wait` again, blocks again. Forever.

The compiler already computes chain convergence via `collect_final_values()`, `is_fully_precomputable()`,
and the folded-loop convergence counter. That data just isn't wired into the wake main loop yet.

Two complementary mechanisms address this:

| | `#!exit` Pragma | Natural Death |
|---|---|---|
| **Who declares it** | Programmer | Compiler |
| **When it exits** | Expression evaluates true | All foldable chains converged, no persistent txns remain |
| **Use case** | Explicit control; programs too complex for compiler to prove; server cleanup | Bounded-counter programs; one-shot work units |
| **Risk** | User may write wrong condition | Compiler may miss non-obvious convergence |

Both coexist. Either mechanism triggering = `ret i32 0`. Neither blocks the other.

## Phase 1: `#!exit <expr>;` — Programmer-Declared Exit

### Syntax

```
#!exit ops == N;
#!exit ops == N && b == N;
```

Follows existing `#!pragma` pattern. File-level, single boolean expression.
Semicolon-terminated, positioned before or after any top-level declaration.

### AST

```rust
pub struct Program {
    pub items: Vec<TopLevel>,
    // ... existing fields ...
    pub exit_condition: Option<Box<Expr>>,  // #!exit <expr>;
    // ... 
}
```

### Parser

In `parse()` at `parser.rs:482`, after processing `#!pragma` attributes, check for `#!exit`:

```rust
// After the #!pragma attribute parsing loop in parse():
while let Some(Ok(Token::HashBang)) = self.current_token() {
    self.advance();
    if let Some(Ok(Token::Identifier(kw))) = self.current_token() {
        if kw == "exit" {
            self.advance();
            let condition = self.parse_expression()?;
            self.expect(Token::Semicolon)?;
            exit_condition = Some(Box::new(condition));
        } else {
            break;
        }
    } else {
        break;
    }
}
```

`#!exit` is intentionally NOT stored as a generic `Attribute`. It needs to carry an `Expr`,
not a string, so codegen can emit it directly without re-parsing.

### Codegen — `emit_main` (standard reactor, `llvm.rs:1984`)

Current (wake mode):
```llvm
tick:
  [async_phase block OR] call void @reactor_tick()
  call void @__rt_wait()
  br label %tick
```

After (`#!exit ops == N` set):
```llvm
tick:
  [async_phase block OR] call void @reactor_tick()
  ; Evaluate exit condition
  %gp_exit_0 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 <ops_idx>
  %lp_exit_0 = load i64, i64* %gp_exit_0, align 8
  %lt_exit_0 = load i64, i64* @N, align 8
  %cp_exit_0 = icmp eq i64 %lp_exit_0, %lt_exit_0
  br i1 %cp_exit_0, label %done, label %wait
wait:
  call void @__rt_wait()
  br label %tick
done:
  ret i32 0
```

If `exit_condition` is `None` (no `#!exit` declared): current behavior unchanged — `br label %tick`.

If `exit_condition` is `None` AND natural death detects convergence: insert auto-generated exit check
(see Phase 2).

### Codegen — `emit_enum_main` (enum dispatch, `llvm.rs:2063`)

Current flow:
```
switch → case_done → br %do_wait (or %async_phase)
do_wait → __rt_wait → br tick
```

After:
```
switch → case_done → br %exit_check
exit_check:
  %e0 = <evaluate exit condition>
  br i1 %e0, label %done, label %async_phase   ; or %do_wait if no async
async_phase:
  barrier_release → reactor_tick → barrier_wait
  br label %do_wait
do_wait:
  call void @__rt_wait()
  br label %tick
done:
  ret i32 0
```

The `done_label` routing at `llvm.rs:2139` becomes `$exit_check` instead of `$do_wait` when
an exit condition exists or natural death is active.

### Design Decisions

**Single-expression, not block**: `#!exit ops == N;` — one boolean expression. A `#exit { ... }` block
form would be syntactic sugar for a txn with `term;`, which is already supported. A block form adds
no new capability.

**Evaluated per tick**: The exit condition is checked once per reactor tick, after the tick's work
completes but before `__rt_wait()`. This means the condition is sampled at tick N+1, not mid-tick.

**Only for wake programs**: One-shot programs already exit naturally. If `#!exit` appears in a
one-shot program, emit a compile-time warning (restricted confidence) but still emit the check
(harmless).

**Multiple `#!exit` lines**: Error — at most one. A program has one exit condition. Combine with `&&`.

---

## Phase 2: Natural Death — Compiler-Automated Exit

### When the Compiler Knows Convergence

The compiler already computes convergence in two places:

1. **`is_fully_precomputable()`** — full state space ≤ budget. `emit_precomputed_main` emits `ret i32 0`
   with no loop at all. Already handled — no change needed.

2. **Folded loop convergence** — bounded counter txns. The fold loop at `emit_folded_loop` compares
   `counter < bound` in a while-loop and exits to `_done` when false. The `_done` label means that
   specific counter converged.

The gap: after `_done`, the compiler doesn't know whether OTHER txns can still fire. For programs
with ONE foldable counter and NO persistent txns, convergence of that counter = program death.

### Implementation

**In `generate()` at `llvm.rs:300-400`**, after computing the transition graph, classify the program:

```rust
let has_persistent_txns = program.items.iter().any(|item| {
    if let TopLevel::Transaction(t) = item {
        if t.is_reactive {
            // A txn is persistent if it's not a convergence-chain foldable txn.
            // Check: does the graph have a node for this txn with bounded_pre?
            let has_bounded = graph.nodes.iter().any(|n| {
                n.name == t.name && n.bounded_pre.is_some()
            });
            !has_bounded
        } else { false }
    } else { false }
});

self.has_natural_exit = !has_persistent_txns
    && graph.nodes.iter().all(|n| n.bounded_pre.is_some());
```

Properties:
- `has_natural_exit = true` only when ALL reactive txns have bounded convergence AND
  the graph has at least one foldable node.
- Trigger-gated server txns without bounded convergence → `has_persistent_txns = true` →
  `has_natural_exit = false`. The program runs forever — correct behavior for servers.
- A program with zero reactive txns → no convergence needed, one-shot path handles it.

### Natural Death in `emit_enum_main`

When `has_natural_exit` is true and no `#!exit` declared, emit convergence check after
case arms:

```llvm
convergence_check:
  ; For each counter variable in the graph:
  %gpc0 = getelementptr ... %State* @global_state, i32 0, i32 <ops_idx>
  %lpc0 = load i64, i64* %gpc0
  %ltc0 = load i64, i64* @N
  %cmpc0 = icmp sge i64 %lpc0, %ltc0    ; counter >= bound → converged
  ; AND all counter checks together:
  %all_converged = <AND of all %cmpcN>
  br i1 %all_converged, label %done, label %do_wait
```

For the common case (single counter, single foldable txn), this is a direct comparison.
For multi-counter programs, it's a chain of ANDs.

### Natural Death in `emit_main` (standard reactor)

When `has_natural_exit` and no `#!exit`, `reactor_tick()` stays `void`.
The convergence check is inserted after `reactor_tick()` returns, same pattern as `#!exit`:

```llvm
tick:
  [async OR] call void @reactor_tick()
  ; convergence check (same pattern as enum dispatch)
  %all_converged = <AND of all counter≥bound comparisons>
  br i1 %all_converged, label %done, label %wait
wait:
  call void @__rt_wait()
  br label %tick
done:
  ret i32 0
```

### What Natural Death Handles

| Program | Natural Death? | `#!exit` needed? | Why |
|---|---|---|---|
| Single foldable txn (ring_buffer) | Yes | No | `ops == N` → counter converged → exit |
| Multi-foldable independent (async_counters) | Yes | No | `a == N && b == N` → all converged → exit |
| Trigger-gated server + foldable worker | No | Yes: `#!exit worker_done` | Server txn might fire at any time |
| External-trigger-only (GUI, MMIO) | No | Yes | Triggers can always change, no convergence |
| One-shot (IIR, precompute) | N/A (already exits) | No | `ret i32 0` already emitted |

---

## Phase 3: Benchmark Fixes

### `benchmarks/ring_buffer.bv`

```brief
#!exit ops == N;

import { io_pending } from "std/brief_rt.bv";

let ops: Int = 0;
const N: Int = 50000000;

rct txn work [io_pending && ops < N][ops == N] {
    &ops = ops + 1;
};
```

After fold loop converges (`ops == 50000000`), `case_done → br exit_check → ops == N is true → ret i32 0`.
Clean exit, exit code 0, no timeout needed. `#!exit` is redundant here (natural death would also exit),
but serves as an explicit declaration of intent.

### `benchmarks/async_counters.bv`

```brief
#!exit a == N && b == N;

import { io_pending } from "std/brief_rt.bv";

let a: Int = 0;
let b: Int = 0;
const N: Int = 25000000;

rct async txn inc_a [io_pending && a < N][a == N] {
    &a = a + 1;
};

rct async txn inc_b [io_pending && b < N][b == N] {
    &b = b + 1;
};
```

Natural death handles this automatically (both txns foldable, no persistent txns).
After both converge, exit. `#!exit` is redundant but explicit.

### `benchmarks/iir_filter.bv` and `benchmarks/precompute_sum.bv`

No changes. One-shot programs already exit cleanly.

---

## Implementation Order

| Step | What | Prerequisite | Est. lines |
|------|------|-------------|-----------|
| 1 | `Program.exit_condition: Option<Expr>` in AST | None | +3 |
| 2 | Parser: `#!exit <expr>;` | Step 1 | +20 |
| 3 | Parser: propagate `exit_condition` through desugarer + import_resolver | Step 2 | +6 |
| 4 | Codegen: `emit_main` — emit exit check before `__rt_wait()` | Step 3 | +15 |
| 5 | Codegen: `emit_enum_main` — emit exit check between `case_done` and `do_wait` | Step 3 | +25 |
| 6 | Codegen: interface method `has_exit_condition()` on LlvmBackend | Steps 3-5 | +5 |
| 7 | Benchmarks: add `#!exit` to ring_buffer.bv, async_counters.bv | Step 5 | +2 each |
| 8 | Test: `test_exit_pragma_emitted` — verify exit check in LLVM IR | Step 5 | +15 |
| 9 | Test: `test_no_exit_without_pragma` — verify no exit check without `#!exit` | Step 5 | +10 |
| 10 | Run all 4 benchmarks — verify clean exit, `/usr/bin/time` reports meaningful numbers | Step 7 | — |
| 11 | `cargo test --lib` — 347 + new tests pass | Steps 1-9 | — |
| 12 | Natural Death: classification in `generate()`, convergence check codegen | Step 5 | +30 |
| 13 | Test: `test_natural_death_single_txn` — foldable-only program exits without `#!exit` | Step 12 | +15 |
| 14 | Test: `test_natural_death_persistent_server` — trigger-gated txn prevents auto-exit | Step 12 | +15 |
| 15 | `build_and_bench.sh` — remove `timeout --signal=KILL`, plain `/usr/bin/time` | Step 10 | -3 |

Total: ~160 lines added, no lines deleted (except benchmark script simplification).

## Risks

| Risk | Mitigation |
|------|-----------|
| Exit condition involves a variable not in the field index | Emit `unreachable` for exit check, let llc verify fail loudly |
| `#!exit` on one-shot program | Emit warning, still emit check — harmless, LLVM dead-code-eliminates it |
| Multiple `#!exit` lines | Parser error: "Only one #!exit declaration is allowed per file" |
| Exit check overhead in hot path | One comparison + branch per tick — negligible vs. 100ms `__rt_wait()` and 50M-iteration fold loops |
| Natural death false positive (claims convergence, misses persistent path) | Guarded by `!has_persistent_txns` — any txn without bounded convergence blocks auto-exit. Conservative, not aggressive. |
| Natural death false negative (fails to detect convergence) | No crash — program idles. User adds `#!exit`. Safe fallback. |
