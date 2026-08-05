# Full Optimization Roadmap — Round 3 (2026-06-02)

## Background

After Round 2 code-review fixes (5 bugs resolved, 368 tests pass), the benchmark picture at 50M iterations:

| Benchmark | Briv | C | Gap |
|-----------|-------|---|-----|
| iir_filter | 0.1876s | 0.1466s | 1.28× (**regression** — was 0.000s) |
| float_math | 0.0251s | 0.0584s | Briv wins |
| float_math_nonzero | 0.4779s | 0.2126s | **2.25×** (µarch scheduling) |
| sparse_dispatch | 0.0795s | 0.0044s | 18× (call-chain overhead) |
| const_heavy | 0.0074s | 0.0519s | Briv 7× faster |
| precompute_sum | 0.0125s | 0.0024s | O(1) — wall-clock noise |
| ring_buffer | 0.0050s | 0.0022s | O(1) — wall-clock noise |
| async_counters | 0.0073s | 0.0039s | O(1) — wall-clock noise |

Key finding: O(1) benchmarks have 0.00s user time. All sub-20ms wall-clock variance is `exec()` + scheduler noise — confirmed via `time -v` and 5× repeated runs showing identical variance in both Briv and C binaries.

## Collection of All Architecture Ideas from Discussion

### Already Done (context, not in this plan)

| Idea | Where | Status |
|------|-------|--------|
| Inferential compile-time precomputation | `region.rs`, `dataflow.rs` — RegionAnalyzer auto-detects Pure/Bounded/Opaque, folds closed chains to O(1) | Done (Path 3) |
| L2 Symbolic Assertion Verification | `assertion_verify.rs`, `symbolic.rs`, `proof_engine.rs` — `sig → true` path exploration, both guard branches checked | Done |
| Adaptive Multi-Rate Reactor Scheduling | `scheduler.rs`, `parser.rs` — `@Hz` declarations, GCD-based interval skipping, zero-overhead library detection | Done |
| Zero-Copy Lock-Free IPC FFI (Metropolitan) | `metro_cli.rs`, `metropolitan.rs`, `/dev/shm` CAS headers, memory pipe mapping | Done |
| Typed SSA | `src/backend/llvm.rs` — `TypedRegister { name, ty }`, removed `is_float_expr` heuristic, 49 call sites updated | Done (A4) |
| Dead-Field Elimination | `transition_graph.rs` — `compute_live_fields()`, IIR filter delay-line state detected as dead | Done |
| Pure-Counter Fold | `llvm.rs` — O(1) `store i64 N` instead of while-loop, enum_fold_pure companion map | Done |
| alloca+SROA in emit_folded_loop | `llvm.rs` — Phase A, decomposes %State to scalar phis via SROA | Done |
| fast-math flags on all float ops | `llvm.rs` — Phase C, compounds with SROA | Done |
| Per-function SLP guard (#4/#5 attributes) | `llvm.rs` — dual LLVM-compat attributes, global -O3 safe | Done (Phase 1) |
| SLP hazard analyzer | `llvm.rs` — union-based float tracking, peak register formula, Kalman verification | Done |
| opt -O3 pipeline + llc --mcpu=native | `main.rs` — SROA+GVN+loop vectorization, AVX codegen | Done (Phase 3) |
| P0 bug fixes (B1-B4) | `lib/ffi/native/src/lib.rs`, `entry_point.rs`, `assertion_verify.rs`, `cross_reference.rs` | Done (Phase 2) |
| iir_filter x==x fix | `transition_graph.rs` — collect_identifiers skips tautological Eq/Ge/Le | Done (Phase 3) |
| Commutativity pattern fix (A6) | `llvm.rs` — removed duplicate match arm in extract_trigger_keys | Done (Phase 3A) |
| Round 2 code-review fixes | `__find_from`, dbriv pipeline, dataflow extracts, protocol verifier, parser dedup | Done (Round 2) |
| Parser deduplication | `parser.rs` — keyword_token_to_name + parse_keyword_as_expr, −236 lines | Done (Round 2) |

---

## High-Priority New Ideas

### A. iir_filter Regression Investigation & Fix

**Problem**: iir_filter was 0.000s after dead-field elimination + pure-counter fold (O(1) `store i64 50000000`). It's now 0.1876s — running the full 50M-iteration folded loop. Something broke the dead-field detection or the pure-counter fold path.

**Hypotheses**:
1. The `collect_identifiers` fix (skipping `Eq(x,x)` in preconditions like `[count < total && x1 == x1]`) may have changed which identifiers are considered "live" — x1/x2/y1/y2 might no longer be collected as live identifiers, but the computation of `effectively_pure` might depend on counting them somehow.
2. The round-2 dataflow.rs changes added `extract_ids_from_statement` — this was a new method only called from `Expr::Block`, but didn't affect the main dataflow analysis path. Unlikely to cause regression.
3. The `llvm.rs` refactoring for Typed SSA changed `emit_expr` return types but should be semantically equivalent.
4. The .opt.ll file for iir_filter may no longer show the O(1) store — need to inspect.

**Investigation method**: 
- Compile iir_filter with current code, inspect `benchmarks/iir_filter.opt.ll`
- Compare against know-good O(1) IR from the calibration baseline commit
- If fold is missing, bisect the relevant files (transition_graph.rs, llvm.rs)

**Files**: `src/analysis/transition_graph.rs`, `src/backend/llvm.rs`

**Expected impact**: Recover 0.000s, eliminate the 1.28× regression.

---

### B. Loop Unrolling in emit_folded_loop

**Problem**: float_math_nonzero shows 2.25× gap (0.4779s vs 0.2126s C). Both Briv and C produce identical 15-instruction AVX hot loops (`vmulss`/`vaddss`/`vaddps`/`dec`/`jne`), zero spills, same loop alignment. The gap is µarch scheduling — phi-per-iteration overhead.

**Solution**: In `emit_folded_loop`, emit the loop body N times (4× or 8×) with a guarded remainder:

```llvm
; BEFORE (1× body):
loop:
  %counter = phi i64 [ 0, %entry ], [ %next, %loop ]
  ; ... 15 instructions of work ...
  %next = add i64 %counter, 1
  %cond = icmp slt i64 %next, 50000000
  br i1 %cond, label %loop, label %exit

; AFTER (4× body):
loop:
  %counter = phi i64 [ 0, %entry ], [ %next4, %loop ]
  ; body iteration 1
  ; body iteration 2
  ; body iteration 3
  ; body iteration 4
  %next1 = add i64 %counter, 1
  %next2 = add i64 %next1, 1
  %next3 = add i64 %next2, 1
  %next4 = add i64 %next3, 1
  %cond = icmp slt i64 %next4, 50000000    ; bound - 3 for guard
  br i1 %cond, label %loop, label %exit
```

**Implementation notes**:
- The unroll factor should be configurable (default 4, try 8)
- Need a remainder loop for cases where bound % unroll_factor != 0
- The `fp1`/`fp2`/... SSA variables in the body need unique names per unrolled copy — suffix with `_u1`, `_u2`, etc.
- The phi nodes at the loop header change: float accumulators gain corresponding operands for each unrolled copy
- Bound adjustment: `bound - (unroll_factor - 1)` for the icmp, to prevent overshoot
- Must regenerate SSA register names within each unrolled copy (not reuse the same `%fp1` across copies)

**Files**: `src/backend/llvm.rs` — `emit_folded_loop()`

**Expected impact**: May reduce the 2.25× gap by distributing phi overhead across 4× more work per iteration. Reduces branch count and icmp count proportionally.

---

## Medium-Priority New Ideas

### C. Register-Level Chain Pipelining

**Problem**: In composed transaction chains (detected by `region.rs`), the codegen emits independent IR for each step. Each step stores its output to `%State`, and the next step loads it back. This creates unnecessary store→load pairs for intermediate chain values.

**Solution**: During composed chain codegen, detect variables that are:
1. Written by step N and read by step N+1
2. Not read by any other transaction (chain-internal)
3. Not part of any precondition or exit condition

For these variables, bypass the struct entirely:
```llvm
; BEFORE (with store/load round-trip):
  %tmp_a = fmul float %x1, 0.01
  ; store to state struct...
  %loaded = load float, float* %state.field
  %result = fadd float %loaded, %y1

; AFTER (register pass-through):
  %tmp_a = fmul float %x1, 0.01
  %result = fadd float %tmp_a, %y1    ; direct register reference
```

**Implementation notes**:
- Extend `RegionAnalyzer` to compute `chain_internal_vars: HashSet<String>` — variables only used within the composed chain
- In codegen, when a chain-internal variable is referenced, use the SSA register from the producing step directly
- The `%State` store is omitted for chain-internal variables (they're dead after the chain completes)
- Only the chain's final output values (those consumed outside the chain) need to hit memory

**Files**: `src/analysis/region.rs`, `src/backend/llvm.rs`

**Expected impact**: Eliminates L1 cache traffic for intermediate chain values. Substantial for programs with deep transaction chains.

---

### D. Register-Resident State (alloca-based, eliminate @global_state)

**Problem**: Currently `@global_state = internal global %State zeroinitializer`. Even though LLVM's GlobalOpt promotes internal globals, it may not derive `noalias` attributes during promotion. Explicit `noalias nocapture` on a local alloca gives stronger alias-analysis guarantees.

**Why the discussion argues for alloca**: LLVM's `mem2reg` does MORE than just insert phi nodes when promoting from alloca. It runs in tandem with Global Value Numbering (GVN) and `LiveIntervals`, which can **split** a variable's lifetime into smaller disconnected intervals. This "live-range splitting" gives the instruction scheduler more freedom to interleave independent ops onto CPU execution ports. Manual phi nodes create a single long-lived virtual register that constrains the scheduler.

**Solution**:
1. In `generate()`, omit `@global_state` global declaration
2. In main() emission, allocate state locally:
   ```llvm
   define i32 @main() {
   entry:
     %state = alloca %State, align 8
     call void @init_state(%State* noalias nocapture %state)
     br label %tick
   tick:
     call void @reactor_tick(%State* noalias nocapture %state)
     ; ...
   }
   ```
3. Pass `%State* noalias nocapture %state` as parameter to all internal functions (reactor_tick, individual txn functions, async workers)
4. Update `emit_exit_expr` to use the passed-in state pointer instead of `@global_state`

**Implementation notes**:
- All function signatures change: add `%State* noalias nocapture %state` first parameter
- `emit_body`, `emit_async_body`, `emit_enum_main`, etc. all use the parameter
- `emit_folded_loop` already allocates local alloca — this change makes the un-folded path consistent
- Thread pool workers need their own state pointer (it's the same alloca from main, passed via argument)

**Files**: `src/backend/llvm.rs`

**Expected impact**: Cleaner alias semantics, guaranteed SROA for all state fields. Marginal for existing benchmarks (folded paths already use alloca), but architecturally correct and eliminates a defensive pattern.

---

### E. Explicit `!range` Metadata on Loads

**Problem**: Briv knows variable bounds statically from preconditions (e.g., `counter < limit`, `enum_field in {0, 1, 2, 3}`). This information is currently not communicated to LLVM.

**Solution**: When emitting loads from state fields that have known ranges, attach `!range` metadata:
```llvm
  %counter = load i64, i64* %state.counter, !range !{i64 0, i64 50000000}
  %enum_val = load i32, i32* %state.mode, !range !{i32 0, i32 4}
```

**Implementation notes**:
- Use existing `field_index_map` and precondition analysis to determine ranges
- For enum fields with known value-set sizes, emit the exact range
- For bounded counters from preconditions, emit the bound
- LLVM can use this to: eliminate redundant bounds checks, optimize switch-to-lookup-table, simplify division/multiplication for loop unrolling
- This is purely an "information dividend" — the analysis already exists, we just need to emit it

**Files**: `src/backend/llvm.rs`

**Expected impact**: Incremental — LLVM can already prove some bounds from the IR structure. Explicit metadata makes it faster and more reliable.

---

## Lower-Priority / Speculative Ideas

### F. Bitmask-Parallel Dispatch Scheduler

**Problem**: For parallel dispatch programs, the runtime scheduler evaluates preconditions and dispatches non-conflicting transactions. This has scheduling overhead.

**Solution**: Compile a static conflict matrix and use bitmask operations:
1. Map each transaction to a bit position in a `u64`
2. Statically compute `conflict_mask[i: u64]` — which transactions conflict with transaction `i`
3. At runtime, evaluate preconditions and pack active transactions into a bitmask
4. Use `popcnt`/`ctz` to select and dispatch non-conflicting tasks

**Implementation notes**:
- Requires new codegen path for `DispatchMode::Parallel`
- The conflict matrix is computed from existing RegionAnalyzer write-sets
- Only worth implementing if parallel dispatch programs show measurable scheduling overhead

**Files**: `src/backend/llvm.rs` (new parallel dispatch codegen path)

**Expected impact**: Replaces runtime scheduler with clock-cycle bitwise ops. Only for parallel dispatch programs.

---

### G. Aggressive Hyper-Folding (Static Control-Flow Collapsing)

**Problem**: Currently, chain composition (`compose_chains` in region.rs) merges sequential transaction bodies. But the result is still a sequence of basic blocks with conditional branches between them.

**Solution**: Extend the current chain composition to perform more aggressive collapsing:
- Detect when a composed chain has a single deterministic execution path (all guards compile to true)
- Collapse the entire chain into a single flat basic block with no intermediate branches
- The CPU sees a linear instruction stream — no branch predictor targets, no icache fragmentation
- The discussion calls this "syntactic abstraction without runtime cost"

**Implementation notes**:
- Requires analysis of guard conditions within composed chains
- If all guards in a chain are `[true]` or compile-time-decidable as true, eliminate the branching
- This is a more aggressive form of the existing chain composition

**Files**: `src/analysis/region.rs`, `src/backend/llvm.rs`

**Expected impact**: For programs with long deterministic chains, eliminates all intra-chain branching overhead.

---

### H. Extend `noalias`/`nocapture` to All Function Boundaries

**Problem**: Briv guarantees non-overlapping addresses at compile time (verified by address consistency pass). But these guarantees aren't fully communicated to LLVM across all function boundaries.

**Solution**: Audit and extend `noalias` and `nocapture` annotations on all function parameters and return values where the language semantics guarantee it:
- State pointers: `noalias nocapture` (no aliasing, no pointer escape)
- FFI call arguments: `noalias` where the type system guarantees no overlap
- Returned pointers from triggers: `noalias` where the source is guaranteed unique

**Implementation notes**:
- Requires careful audit — incorrectly applied `noalias` can cause miscompilation
- The address consistency pass (`check_address_consistency`) provides the proof
- Incrementally add annotations, test, and verify

**Files**: `src/backend/llvm.rs`

**Expected impact**: Incremental — lets LLVM's alias analysis be more aggressive. Most impactful for functions with multiple pointer arguments.

---

## Implementation Order

| Step | ID | Description | Priority |
|------|----|-------------|----------|
| 0 | — | Git commit current state | Immediate |
| 1 | A | Investigate iir_filter regression — check .opt.ll for O(1) fold | High |
| 1a | A | Fix regression if confirmed | High |
| 2 | B | Loop unrolling 4×/8× in emit_folded_loop | High |
| 3 | C | Register-level chain pipelining | Medium |
| 4 | D | Register-resident state (alloca-based) | Medium |
| 5 | E | `!range` metadata on loads | Medium |
| 6 | G | Aggressive hyper-folding | Low |
| 7 | F | Bitmask-parallel dispatch | Low |
| 8 | H | Extend `noalias`/`nocapture` | Low/audit |

---

## Relevant Files

| File | What changes |
|------|-------------|
| `src/analysis/transition_graph.rs` | iir_filter regression fix (live fields, pure detection) |
| `src/backend/llvm.rs` | Loop unrolling, chain pipelining, alloca state, `!range` metadata, hyper-folding, bitmask dispatch |
| `src/analysis/region.rs` | Chain-internal variable detection for pipelining |
| `src/main.rs` | No planned changes (llc pipeline is stable) |
