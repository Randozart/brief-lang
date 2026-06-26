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
- **Also**: Run `--correctness` on all benchmarks to verify Brief == C output.
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
- **Known**: print_loop showed Brief: "0" vs C: "" at BOUND=5. The
  mismatch suggests C's output line is empty (possibly newline diff).
- **Action**: Fix mismatches or document as expected (e.g., float
  formatting differences).

### Phase 5: Trophy folder evaluation
- After Phase 1 fixes the nbody bound, re-benchmark. If any benchmark
  genuinely beats C by 1.5×+, move it to a `trophy/` directory with
  a README explaining why Brief outperforms C (e.g., "contracts enabled
  LLVM to unroll the loop by proving the iteration count at compile time").

## Success Criteria
- All 21 benchmarks compile and produce correct output matching C.
- fasta ratio < 1.5× (from current 2.29×).
- queue_drain ratio < 2.0× (from current 2.40×).
- knucleotide ratio < 1.15× (from current 1.28×).
- All other benchmarks within 15% of C.
- Every code change has a dated comment explaining why it exists.
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
No comment or commit message explains the duplication. **The double-load
is an accidental artifact with zero intentional design rationale.**

**Implementation approach (revised)**:
Instead of saving/restoring `let_bindings` maps (which would couple the
two emission phases), we can fix this more surgically: in
`pre_load_all_fields`, after loading each field, store the register name
into a new `preloaded_regs: HashMap<String, String>` map. Then, when a
subsequent `pre_load_all_fields` call sees the same field in
`preloaded_regs`, it emits `add i64 0, <saved_reg>` instead of
`GEP + load`. This reuses the SSA value from the dominating block
without any structural change to the tick/body block layout.

### Phase 3: Arena Bump Check — Confirmed Overly Conservative

Present since commit `d35fbd7e` (2026-06-23) where the arena was first
introduced. The straightforward "check before every malloc replacement"
was never tightened. There has never been a version that skipped the
per-allocation check. **The per-allocation check is an overly conservative
default, not an intentional safety measure.**

**Implementation approach (revised)**:
Add a `bool` field `arena_known_ok` to `LlvmBackend`. Set to `true` after
a successful bump alloc (the `icmp ule` passes). Before emitting the
check in `emit_arena_alloc`, skip it if the flag is set. Reset to `false`
in `emit_arena_reset()`. For the grow path, reset to `false` (the realloc
changed the arena size, so the next allocation needs a fresh check).
