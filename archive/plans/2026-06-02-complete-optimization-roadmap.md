# Complete Optimization Roadmap — All Ideas (2026-06-02)

## Background

After ASR profitability gate, benchmark infrastructure overhaul, and fixing exit conditions to prevent dead-field elim from eating float benchmarks:

### Current Benchmark State (5-iteration avg, CLOCK_MONOTONIC nanosecond timing)

| Benchmark | Briv | C | Ratio | Winner |
|-----------|-------|---|-------|--------|
| iir_filter | 0.0333s | 0.1526s | 0.21× | **Briv** (O(1) fold) |
| precompute_sum | 0.0009s | 0.0005s | 1.80× | startup noise |
| ring_buffer | 0.0006s | 0.0006s | 1.00× | ~tie (O(1) fold) |
| async_counters | 0.0005s | 0.0006s | 0.83× | ~tie (O(1) fold) |
| **float_math** | 0.0161s | 0.0066s | **2.43×** | C |
| **float_math_nonzero** | 0.5737s | 0.2431s | **2.35×** | C |
| sparse_dispatch | 0.0018s | 0.0011s | 1.63× | startup noise |
| const_heavy | 0.0007s | 0.0548s | 0.01× | **Briv** (7× faster) |

Root cause of 2.4× gap: struct `alloca` + `extractvalue`/`insertvalue` inside the loop body creates wider register lifetimes than C's local-variable register allocation. Zero SLP packing ops survive in `.opt.ll`.

---

## Already Done (context)

| Idea | What | When |
|------|------|------|
| Inferential compile-time precomputation | RegionAnalyzer auto-detects Pure/Bounded/Opaque, folds closed chains to O(1) | Path 3 |
| Dead-field elimination | compute_live_fields() + effectively_pure detection | Path 2 |
| Pure-counter fold | O(1) `store i64 N` instead of while-loop | — |
| alloca+SROA in emit_folded_loop | Phase A, decomposes %State to scalar phis | Phase A |
| fast-math flags | Phase C, compounds with SROA | Phase C |
| Per-function SLP guard (#4/#5 attributes) | Dual LLVM-compat attributes, global -O3 safe | Phase 1 |
| SLP hazard (register pressure) | Peak register formula, disables SLP when peak ≥ R | Phase 1 |
| SLP hazard (ASR profitability gate) | ops_per_field = total_float_ops / distinct_fields; if < 1.5, disable SLP | 2026-06-02 |
| Loop unrolling 4× in emit_folded_loop | Body4 + remainder body1 + guarded header | 2026-06-02 |
| Typed SSA | TypedRegister { name, ty }, removed is_float_expr heuristic | A4 |
| Commutativity pattern fix | Removed duplicate match arm in extract_trigger_keys | A6 |
| P0 bug fixes (B1-B4) | UTF-8 boundaries, entry-point values, assertion false-path, overlap detection | Phase 2 |
| Round 2 code-review fixes | __find_from, dbriv pipeline, dataflow extracts, protocol verifier, parser dedup | Round 2 |
| Benchmark infrastructure | Nanosecond fork+exec CLOCK_MONOTONIC timer, 5-iter avg, winner column | 2026-06-02 |
| Exit condition fixes | Added `&& x0 >= 0.0` to float benchmarks to prevent dead-field elim | 2026-06-02 |

---

## Remaining Ideas — Implementation Order

### High Priority

#### D. Register-Resident State (alloca-based, eliminate @global_state)

**Problem**: `@global_state = internal global %State zeroinitializer` forces functions to reference a module-level global. Even though LLVM's GlobalOpt promotes internal globals, the resulting alias analysis is weaker than explicit `noalias nocapture` on a local alloca parameter. Inside folded loops, the struct load/store pattern (`load %State` → `extractvalue` → compute → `insertvalue` → `store %State`) survives longer in the optimization pipeline than C's simple `float` local variables.

**Theory**: LLVM's `mem2reg` from `alloca` does MORE than insert phi nodes — it runs in tandem with GVN and LiveIntervals, splitting variable lifetimes into smaller disconnected intervals. This live-range splitting gives the instruction scheduler more freedom. Manual phi nodes (and global-derived struct loads) create longer-lived virtual registers that constrain scheduling.

**What changes** (all in `src/backend/llvm.rs`):

1. **`emit_state_struct()`** — stop emitting `@global_state = internal global %State zeroinitializer`. Keep struct type definition only.

2. **`emit_main()`** — emit `%state = alloca %State, align 8` in entry. Pass `%State* noalias nocapture %state` to all internal function calls.

3. **All internal function signatures** — add `%State* noalias nocapture %state` as first parameter:
   - `@init_state(%State* noalias nocapture)`
   - `@reactor_tick(%State* noalias nocapture)`
   - `@tick` (each transaction function)
   - `@tick_worker` (async worker functions)

4. **Replace all `@global_state` GEPs** — change to GEPs on `%state` parameter in:
   - `emit_body()`
   - `emit_reactor_tick()`
   - `emit_main()` exit check
   - `emit_folded_loop()` preheader
   - `emit_enum_main()`
   - `emit_async_body()`

5. **Thread `%state` through `emit_exit_expr()`** — currently references `@global_state` for identifiers. Change to accept state register name as parameter.

6. **All `call` sites** — add `%State* %state` as first argument to every non-FFI function call.

7. **Thread pool** — workers get state pointer via argument, operate on same alloca. Conflict-free field access guaranteed by proof engine.

**Expected impact**: SROA promotes all state fields to scalar registers for the entire tick loop. The 2.4× gap should close or nearly close.

**Risk**: High surface area — every function signature changes. Mitigation: implement one function at a time, test after each step.

**Files**: `src/backend/llvm.rs`

---

### Medium Priority

#### N1. `@llvm.assume` from ProofEngine

**Problem**: The ProofEngine and `range.rs` prove bounds and invariants, but this information is not communicated to LLVM.

**Solution**: For any proven precondition reaching a loop body, emit:
```llvm
%cmp = icmp slt i64 %counter, %bound
call void @llvm.assume(i1 %cmp)
```
LLVM uses `@llvm.assume` to eliminate dead branches, simplify arithmetic, and unroll more aggressively. The `@llvm.assume` intrinsic is already declared in the codebase (used in convergent fold paths for `llvm.assume` on convergence preconditions). Extend to:
- Non-convergent Tier-2 loops where `range.rs` can still prove spatial safety bounds
- Any postcondition the ProofEngine has verified
- Loop invariants derived from partial contracts

**Implementation notes**:
- Extract proven bounds from `range.rs` and `ProofEngine` results during analysis
- In `emit_main()` or `emit_reactor_tick()`, inject `@llvm.assume` calls right after precondition checks that succeed
- The `!range` metadata idea (E) is a more passive form of this — `@llvm.assume` is stronger

**Files**: `src/backend/llvm.rs` (emission), `src/analysis/range.rs` (bound extraction), `src/proof_engine.rs` (contract results)

**Expected impact**: Amplifies Idea D's benefit by giving LLVM explicit bounds information inside register-resident loops.

---

#### N2. Equality Saturation (egg e-graph) for Composed Chains

**Problem**: Transaction chain composition combines sequential bodies, but the composed expression still mirrors the original source structure. Algebraic simplifications that a human would spot (`(a * b) + (a * c)` → `a * (b + c)`) are not performed.

**Solution**: Integrate the `egg` Rust crate into `src/analysis/region.rs`. After composing chains:

1. **Construct an e-graph** from the mathematical expressions in the composed body
2. **Define algebraic rewrite rules**: distributivity, associativity, float identities (`x * 1.0 → x`, `x * 0.0 → 0.0`), bitwise equivalences
3. **Run saturation**: the e-graph compactly represents ALL equivalent forms
4. **Cost-function extraction**: extract the mathematically cheapest representation
5. **Replace the composed body** with the extracted minimal form

**Why Briv is uniquely suited**: Composed chains have no side effects, no pointer aliasing, no FFI calls — they are pure mathematical expressions. Equality saturation is designed exactly for this scenario.

**Implementation notes**:
- New dependency: `egg` crate in `Cargo.toml`
- Integration point: `RegionAnalyzer::compose_chains()` in `src/analysis/region.rs`
- Rules should be parameterized by target architecture (x86 favors shifts over multiplies, AArch64 has different cost model)
- Only run when `optimize-budget` flag is above a threshold (egg can be slow on large graphs)

**Files**: `src/analysis/region.rs`, `Cargo.toml`

**Expected impact**: A complex 10-step composed chain can be programmatically folded into a single closed-form equation. C compilers don't do this — they apply peephole rules sequentially without global exploration.

---

#### N3. Compile-Time PGO via Interpreter

**Problem**: LLVM's code generator makes branch-layout decisions blindly. Without profile data, it assumes equal branch probability.

**Solution**: Briv has a built-in `Interpreter` (`src/interpreter/mod.rs`). Use it for zero-overhead compile-time profiling:

1. **Interpret with profiling mode**: Execute the Briv program in the interpreter with sample inputs (or generate reasonable defaults from state initial values)
2. **Record branch probabilities**: How often each guarded condition (`[sensor > 100]`) evaluates true/false
3. **Emit `!prof` metadata**:
   ```llvm
   br i1 %cmp, label %then, label %else, !prof !{!"branch_weights", i32 999, i32 1}
   ```
4. LLVM uses `!prof` at `-O3` to physically arrange the frequent path as fall-through, improving I-cache locality and eliminating branch-misprediction penalties

**Implementation notes**:
- New flag: `--profile-inputs <file>` to specify interpreter input values
- Default: interpret with zero/initial values, record branch distributions
- Integrate with `src/interpreter/mod.rs` — add a `profile_mode: bool` flag that records branch outcomes instead of just executing
- Thread the profile data through to `src/backend/llvm.rs` for metadata emission

**Files**: `src/interpreter/mod.rs`, `src/backend/llvm.rs`, `src/main.rs` (flag)

**Expected impact**: 5-15% improvement on branch-heavy programs via better instruction layout. Zero runtime cost — no instrumentation recompile cycle like C needs.

---

#### N4. Tiered Optimization Architecture

**Problem**: Currently the compiler either hyper-folds (dead-field + pure-counter → O(1)) or falls through to standard codegen. There's no explicit recognition of the "partial contract" middle ground — loops that can't be folded but CAN have safety checks stripped.

**Solution**: Formalize a two-tier architecture:

```
Tier 1 (Total, provably convergent):
  - Hyper-folding to O(1)
  - Precomputation at compile time
  - Full AoRTE check removal
  - No runtime loop structures emitted

Tier 2 (Partial, Turing-complete):
  - Standard register loops with recursion guards
  - Still emit @llvm.assume + !range for spatial safety
  - Still strip bounds checks where range.rs proves safety
  - Loop invariants from partial contracts injected as @llvm.assume
```

**Implementation notes**:
- Tier classification already partially exists: `is_fully_precomputable()` in `region.rs` is Tier 1 detection
- Add `is_spatially_safe()` — can prove no out-of-bounds accesses even if termination isn't proven
- For Tier-2 loops, add a new codegen path that emits the loop body but with `@llvm.assume` on proven bounds and without runtime bounds checks
- Integrate with the existing `range.rs` and `ProofEngine` passes

**Files**: `src/analysis/region.rs`, `src/backend/llvm.rs`, `src/proof_engine.rs`

**Expected impact**: Makes Briv's optimization decisions explicit and auditable. Enables future passes to target specific tiers.

---

### Lower Priority

#### C. Register-Level Chain Pipelining

Skip store→load round-trips for chain-internal variables in composed transaction chains. Detect variables that are written by step N and read by step N+1 but never exposed outside the chain. Pass SSA register outputs directly.

**Files**: `src/analysis/region.rs`, `src/backend/llvm.rs`

---

#### E. `!range` Metadata on Loads

Passive form of N1 — emit known bounds from preconditions directly on load instructions. Less powerful than `@llvm.assume` but easier to implement.

**Files**: `src/backend/llvm.rs`

---

#### F. Bitmask-Parallel Dispatch

Compile-time conflict matrix as `u64` bitmasks, `popcnt`/`ctz` for zero-overhead scheduling. Only for parallel dispatch programs.

**Files**: `src/backend/llvm.rs` (new parallel dispatch codegen path)

---

#### G. Aggressive Hyper-Folding

Extend chain composition to collapse everything with compile-time-decidable guards into a single flat basic block — no intra-chain branches.

**Files**: `src/analysis/region.rs`, `src/backend/llvm.rs`

---

#### H. Extend `noalias`/`nocapture`

Audit all function boundaries and add `noalias`/`nocapture` where Briv's semantics guarantee it. Incremental.

**Files**: `src/backend/llvm.rs`

---

## Execution Order

| Step | ID | Description | Files |
|------|----|-------------|-------|
| **1** | **D** | Register-Resident State (alloca from main) | `src/backend/llvm.rs` |
| 2 | N1 | `@llvm.assume` from ProofEngine | `src/backend/llvm.rs`, `proof_engine.rs` |
| 3 | N2 | Equality Saturation (egg) | `src/analysis/region.rs`, `Cargo.toml` |
| 4 | C | Chain Pipelining | `src/analysis/region.rs`, `src/backend/llvm.rs` |
| 5 | N3 | Compile-Time PGO via Interpreter | `src/interpreter/mod.rs`, `src/backend/llvm.rs` |
| 6 | N4 | Tiered Architecture | `src/analysis/region.rs`, `src/backend/llvm.rs` |
| 7 | E | `!range` metadata | `src/backend/llvm.rs` |
| 8 | G | Aggressive Hyper-Folding | `src/analysis/region.rs`, `src/backend/llvm.rs` |
| 9 | F | Bitmask-Parallel Dispatch | `src/backend/llvm.rs` |
| 10 | H | Extend noalias/nocapture | `src/backend/llvm.rs` |

## Relevant Files

| File | What changes |
|------|-------------|
| `src/backend/llvm.rs` | Idea D (all function signatures), N1 (@llvm.assume), C (chain pipelining), E (!range), F (bitmask), G (hyper-folding), H (noalias) |
| `src/analysis/region.rs` | N2 (egg integration), C (chain-internal var detection), N4 (tier classification), G (guard collapsing) |
| `src/analysis/range.rs` | N1 (bound extraction from range analysis) |
| `src/proof_engine.rs` | N1 (contract proof results), N4 (termination vs safety distinction) |
| `src/interpreter/mod.rs` | N3 (profile mode, branch recording) |
| `Cargo.toml` | N2 (egg dependency) |
| `src/main.rs` | N3 (--profile-inputs flag) |
