# Dispatch Optimization & Benchmark Fairness

**Date**: 2026-06-03  
**Status**: Planned  
**368 tests pass (baseline)**

## Motivation

C beats Briev on sparse_dispatch by 141×. Investigation revealed this is partly a benchmark artifact (C's switch with empty `break;` cases is eliminated by clang) and partly a real codegen issue — Briev's `emit_reactor` evaluates all 8 txn preconditions serially, each on the **post-update** state of the previous txn (a cascade bug).

Separately, iir_filter and const_heavy show Briev winning by extreme margins because dead-field elimination removes work that C's `volatile` or `return` keeps alive. These benchmarks need cleaning and the philosophy around `#!exit` needs clarification.

## Philosophy

- **`#!exit` is a termination pragma**: It tells the compiler "if this condition holds, the program is done." It is NOT an observation mechanism to keep variables alive for benchmarking.
- **No artificial liveness hacks**: `x == x` guards in preconditions, `volatile` in C, and `x >= -1.0` in exit conditions are all cheats that correct compilers eliminate. Accept what the compiler proves dead and design benchmarks where work is structurally live.
- **Clean build, no prototyping**: Every optimization pass belongs in its proper module. Dispatch analysis goes in `transition_graph.rs`, codegen in `llvm.rs`.
- **Contracts are compile-time proofs**: Pre/post conditions verified by the proof engine are eliminated at codegen. They are not runtime checks.

## Plan

### Phase 0 — AGENTS.md additions

- Add "No prototyping — build clean" principle to For OpenCode section.
- (Benchmark harness rule already added in previous session.)

### Phase 1 — Fix cascade bug in `emit_reactor`

**File**: `src/backend/llvm.rs`, function `emit_reactor` (~2610-2693)

**Problem**: Each txn's precondition evaluates the state AFTER the previous txn's body has run. For sparse_dispatch's 8 txns with `count % 8 == N` preconditions and `count + 1` bodies, this cascades: ping fires (incrementing count), ack sees the new count and also fires, etc. All 8 fire per tick, net effect is `count += 8`.

**Fix**: Snapshot `%state` once before the dispatch chain. Evaluate all preconditions against the snapshot. Apply only the matching txn's body.

```llvm
; Before (broken):
%c0 = call pre_ping(%state)   ; reads original state
call body_ping(%state)        ; mutates state
%c1 = call pre_ack(%state)    ; reads MUTATED state  ← BUG
call body_ack(%state)         ; mutates again

; After (fixed):
%saved = load %State, %state  ; snapshot once
%c0 = call pre_ping(%saved)   ; all preconditions against snapshot
%c1 = call pre_ack(%saved)
...
; select matching txn, apply its body
```

### Phase 2 — Switch-dispatch detection in `transition_graph.rs`

**File**: `src/analysis/transition_graph.rs`, new function `detect_switchable_dispatch`

**New struct**: `SwitchDispatchGroup { group_key: Expr, cases: Vec<(i64, usize)>, is_pure_chain: bool }`

A group qualifies when:
1. Each txn has a precondition of the form `expr == literal_i` (integer literal)
2. `expr` is identical across all N txns (e.g., `count % 8`)
3. `{literal_i}` forms a contiguous range `0..N`
4. Preconditions are mutually exclusive (at most one true per state)
5. All bodies are pure (only modify bounded counters)

`is_pure_chain` = all bodies are `counter = counter + 1` with the same counter.

**Tests**: 4 cases — (a) no group, (b) contiguous range of 3, (c) non-contiguous, (d) pure chain.

### Phase 3 — Codegen: switch + chain collapse

**3a — Switch dispatch** (`llvm.rs::emit_reactor`):
After the cascade fix, check `detect_switchable_dispatch`. If found, emit `switch i64 %expr` with case arms instead of the serial chain.

```llvm
%expr = ... compute count % 8 ...
switch i64 %expr, label %done [
  i64 0, label %case_0
  i64 1, label %case_1
  ...
]
case_0: call void @ping(%state); br %done
case_1: call void @ack(%state); br %done
done:
```

**3b — Pure-chain collapse** (`llvm.rs`): If `is_pure_chain` AND the exit condition references the counter, replace the entire dispatch + loop with `counter = bound` (O(1)). Same mechanism as `emit_folded_pure_counter`.

**3c — Enum dispatch path** (`llvm.rs::emit_case_folded_loops`): Apply same detection to case arms in the enum dispatch path.

### Phase 4 — Clean dirty benchmarks

Remove artificial hacks. Accept what compilers eliminate.

| Benchmark | C changes | Briev changes | Classification |
|-----------|-----------|---------------|----------------|
| iir_filter | Remove `volatile long count` | None (`x==x` already stripped) | **Elimination frontier** — both converge to O(1) |
| const_heavy | None | None | **Asymmetrical** — C observes `acc` via `return`, Briev correctly eliminates it |
| sparse_dispatch | Replace empty `break` with `acc_N += 1` per case, sum all 8 at end | Phase 1-3 make dispatch O(N) correctly | **Real work benchmark** |

### Phase 5 — Proper structurally-live benchmarks (future)

Requires `frgn` output mechanism (`print_int` in `lib/std/`). Then:

1. **kalman_filter**: 12 float fields, covariance feedback — structurally live (already exists as `kalman_filter_runtime`)
2. **pid_controller**: Position/velocity/integral feedback — structurally live
3. **string_hash**: Rolling hash of integer sequence — uses `print_int` for observation

### Phase 6 — Final benchmark pass

Run `bash benchmarks/build_and_bench.sh`. Update AGENTS.md summary.

## File Changes Summary

| Phase | File | Change | Lines |
|-------|------|--------|-------|
| 0 | AGENTS.md | Add clean-build principle | +2 |
| 1 | `src/backend/llvm.rs` | Cascade fix in emit_reactor | ~30 |
| 2 | `src/analysis/transition_graph.rs` | Switch-dispatch detection | ~100 |
| 2 | `src/analysis/transition_graph.rs` | Tests for detection | ~60 |
| 3a | `src/backend/llvm.rs` | Switch codegen in emit_reactor | ~100 |
| 3b | `src/backend/llvm.rs` | Pure-chain collapse | ~50 |
| 3c | `src/backend/llvm.rs` | Enum dispatch integration | ~30 |
| 3 | `src/backend/llvm.rs` | Tests for switch codegen | ~60 |
| 4 | `benchmarks/sparse_dispatch_c.c` | Per-case accumulators | ~10 |
| 4 | `benchmarks/iir_filter_c.c` | Remove volatile | -1 |
| 5 | New files | Proper benchmarks (future) | ~200 |

## Dependencies

- Phase 2 must complete before Phase 3a.
- Phase 1 is independent of Phase 2 (it's a correctness fix).
- Phase 3c depends on Phase 2 and Phase 3a.
- Phase 4 depends on Phase 1 (for sparse_dispatch cascade fix).
- Phase 5 requires a `frgn` output mechanism first.
