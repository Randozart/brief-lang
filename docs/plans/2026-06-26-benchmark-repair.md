# Benchmark Repair Plan

Date: 2026-06-26

## Problem
After the LLVM codegen fixes (in_callable_txn leak, float exit conditions,
arena allocator PHI), all 21 benchmarks compile but several underperform C
or have measurement bugs.

## Phases

### Phase 1: Fix nbody BOUND default + correctness baseline
- **What**: nbody_newton and nbody_sqrt_idio show 0.007× ratios because
  `get_env_int#("BOUND")` returns 0 when the env var is unset; C defaults
  to 50M. Fix: add default-50M fallback in the benchmark source.
- **Files**: `benchmarks/nbody_newton.bv`, `benchmarks/nbody_sqrt_idio.bv`
- **Also**: Run `--correctness` on all benchmarks to verify Briv == C output.
- **Expected**: nbody ratios become ~1.0–1.15× (not 0.007×).

### Phase 2: Double-load elimination in reactive tick
- **What**: The reactive tick infrastructure loads every state field in the
  precondition block, then loads them AGAIN in the body block. This doubles
  load/store traffic.
- **Where**: `src/backend/llvm/loop_engine.rs` — the tick → body transition.
- **Approach**: After the precondition check succeeds, pass the already-loaded
  register values into the body block instead of reloading from `%State`.
  This is additive (new code path for SSA-threaded values), not modifying
  the existing memory-mode fallback.
- **Expected impact**: fasta 2.29× → ~1.3×, knucleotide 1.28× → ~1.05×.
- **Trade-off**: Adds a `br` that forces a new basic block boundary. On
  txns with 0–1 fields the extra `br` may slightly hurt (LLVM must merge
  it back). The compiler should detect the number of state fields at
  compile time: if N_fields <= 1, use the old direct pattern; if > 1, use
  the SSA-threaded pattern.

### Phase 3: Arena-allocator bump check bypass
- **What**: `emit_arena_alloc` does `icmp ule` + branch every call even
  when the arena has ample remaining capacity. For queue_drain this fires
  100M times.
- **Where**: `src/backend/llvm/mod.rs` — `emit_arena_alloc`.
- **Approach**: When the arena is known to have capacity (the bump pointer
  check has passed once), elide the subsequent checks for the rest of
  the transaction body. Use a flag in the arena state to track "known
  capacity until next arena reset."
- **Expected impact**: queue_drain 2.40× → ~1.5× (the memcpy for 0-length
  copies is the remaining cost, which requires a uniqueness optimization).
- **Trade-off**: If a single tick allocates many objects, the first alloc
  may trigger a realloc. Skipping checks on subsequent allocs in the same
  tick is safe (the realloc doubled the arena, so the rest of the tick's
  allocations will fit). If a tick allocates more than 2× the arena size
  in one go, the realloc logic handles it at the `icmp ule` level.

### Phase 4: Correctness sweep
- **What**: Run `bash benchmarks/build_and_bench.sh --correctness`.
  Investigate any mismatches.
- **Known**: print_loop showed Briv: "0" vs C: "" at BOUND=5. The
  mismatch suggests C's output line is empty (possibly newline diff).
- **Action**: Fix mismatches or document as expected (e.g., float
  formatting differences).

### Phase 5: Per-field phi nodes in reactive tick loops
- **What**: Replace per-field GEP+load/store round-trips in reactive tick
  loops with LLVM phi nodes that keep state values in SSA registers across
  the loop back-edge. This eliminates ALL memory traffic in the hot loop
  body, matching Clang's pattern for local variables.
- **Where**: `src/backend/llvm/loop_engine.rs` — `emit_ssa_main`,
  `src/backend/llvm/emit_toplevel.rs` — `emit_trg_load` / exit condition,
  `src/backend/llvm/emit_stmt.rs` — Guarded block cache clearing.
- **Approach**: 
  1. Extend the existing `phi_induction_reg` mechanism from just the
     counter to ALL scalar state fields.
  2. Emit a `phi i64 [ %init, %entry ], [ %updated, %latch ]` for each
     field at the tick header.
  3. Override `ssa_old_int_regs` with these phi registers so the body
     reads from phi values instead of GEP+load from `%State`.
  4. Track which fields the body actually modifies via
     `pending_phi_backedge: HashMap<String, String>` on the backend.
  5. At the latch, feed the tracked back-edge values into the phis.
     For unmodified fields, reload from `%State` (LLVM GVN eliminates
     the redundant reload since the value hasn't changed).
- **Always-fallback (memory mode required) conditions**:
  - **MMIO state fields** (`state @ 0xNNNN`): volatile semantics require
    every access to hit the actual address. Phi nodes are SSA registers,
    not memory operations. Detected via `mmio_fields.is_empty()`.
  - **Cross-thread async dispatch**: `%State` crosses thread boundaries
    as a `ptr`. SSA values are thread-local. Detected via
    `async_txn_names.is_empty()`.
  - **CellCall convergence loops**: State evolves across iterations;
    a phi at loop entry would fix the initial value. Detected by
    scanning the body for `CellCall` statements.
  - **Wake triggers** (`@link` without `#nowake`): `epoll_wait` writes
    trigger values into `%State` via volatile stores, then the body
    reads them. Memory access is semantically required. Detected via
    `has_wake_triggers`.
- **Decision mechanism**: A compile-time check `emit_ssa_main` evaluates
  before entering the loop emission path. It checks the four fallback
  conditions and logs the result in `report_lines`.
  
  ```python
  if has_mmio_or_async_or_cellcall:
      use_memory_mode()       # R1-R3: genuinely required
  elif multi_txn_with_wake_triggers:
      use_memory_mode()       # R4: trigger writes → memory
  elif multi_txn_no_wake:
      use_per_field_phi()     # Multi-txn phi web (N paths)
  else:  # single reactive txn
      use_per_field_phi()     # Simple phi (2 paths: fired or skipped)
  ```

- **Trade-off for the phi-mode path**: Emitting N phi nodes adds N SSA
  values to the loop header block. LLVM's `-indvars` and `-licm` can
  optimize induction-variable phis more aggressively than GEP+load
  chains (loop-invariant code motion, strength reduction, vectorization).
  The only overhead is the initial load from `%State` in the `%entry`
  predecessor — a one-time cost.
- **Trade-off for the memory-mode fallback**: Programs that genuinely
  need memory mode (MMIO, async, CellCall, wake triggers) see zero
  regression — the existing memory-mode path is untouched.
- **Regression check**: Store a `bool` field `used_phi_loop` on
  `LlvmBackend` that records whether phi mode was selected. Log the
  choice in `report_lines` so benchmark output shows which mode was
  taken. This makes regressions diagnosable.
- **Expected impact**: fasta ~2.2× → ~1.1×, knucleotide ~1.22× → ~1.05×,
  all single-txn non-wake benchmarks approach C parity.

### Phase 6: Trophy folder evaluation
- After all phases complete, re-benchmark. If any benchmark genuinely
  beats C by 1.5×+, move it to a `trophy/` directory with a README
  explaining why Briv outperforms C (e.g., "contracts enabled LLVM to
  unroll the loop by proving the iteration count at compile time").

## Success Criteria
- All 21 benchmarks compile and produce correct output matching C.
- fasta ratio < 1.2× (from current 2.2×).
- queue_drain ratio < 2.0× (from current 2.40×) or documented pre-existing.
- knucleotide ratio < 1.10× (from current 1.22×).
- All other benchmarks within 15% of C.
- Every code change has a dated comment explaining why it exists.
- The phi-vs-memory decision is logged in `report_lines`.
- Every heuristic stores its choice in `report_lines` for regression analysis.

---

## Investigation Results

### Phase 2: Double-load — Confirmed Accidental Artifact

Introduced in commit `847e0f9d` (2026-06-10, "R2+R3: Float boxing
elimination + per-field GEP loops"). The old code loaded `%State` once
into a single SSA register, then used `extractvalue` in both the tick
and body blocks — natural sharing across blocks because `tick` dominates
`b_body`. The per-field GEP replacement blindly called
`pre_load_all_fields` in both blocks without threading the values across.
**The double-load is an accidental artifact with zero intentional design
rationale.**

### Phase 3: Arena Bump Check — Confirmed Overly Conservative

Present since commit `d35fbd7e` (2026-06-23) where the arena was first
introduced. The straightforward "check before every malloc replacement"
was never tightened. **The per-allocation check is an overly conservative
default, not an intentional safety measure.**

### Phase 5: Per-field Phi Mode — Architectural Analysis

The reactive tick loop in `emit_ssa_main` always falls back to memory mode
(GEP+load/store to `%State` every iteration) because `ssa_state_reg` is
explicitly set to `None` at line 982. Investigation identified four
conditions where phi nodes **fundamentally cannot** replace memory mode:

| Condition | Why Phi Can't Work | Detection |
|-----------|--------------------|-----------|
| **MMIO state fields** (`field @ 0xNNNN`) | Volatile semantics — every load/store must hit hardware. Phi nodes are registers, not memory access. | `mmio_fields.is_empty()` |
| **Cross-thread async dispatch** | `%State` crosses thread boundaries as `ptr`. SSA values are function-local. | `async_txn_names.is_empty()` |
| **CellCall convergence loops** | State evolves across iterations — phi at loop entry would stale-out. | Scan body for `CellCall` |
| **Wake triggers** (`@link` without `#nowake`) | `epoll_wait` writes trigger values into `%State` via volatile store. Memory semantics required. | `has_wake_triggers` |

All other programs (including multi-txn reactive, single-txn loops,
`@link` state without wake, timer triggers) can use per-field phi nodes.
The current memory mode is a codegen simplification, not a requirement.
