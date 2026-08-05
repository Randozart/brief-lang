# Agent Handoff — Briv Compiler Baseline Recovery
## Handoff timestamp: 2026-07-28 ~12:00 (updated ~14:30)
## Current baseline: `b39461e2` — "SLP stride gate — all 19 benchmarks at parity or better"

## Critical Ground Rules

Read `AGENTS.md` fully before any code change. The roleplay instruction at the top
is non-negotiable. Key rules:

- **"Probably fine" is a critical failure.** Every regression must be traced to a
  specific commit before any fix is proposed. "Noise" is not an explanation.
- **Never update the baseline** to a commit with measurable regressions. The
  baseline is sacred.
- **One change per commit.** Each commit gets its own benchmark run. No stacking.
  If a change regresses any benchmark, it does NOT land until the regression is
  diagnosed and fixed.
- **Do not dismiss regressions as "pre-existing."** If a benchmark was better in
  an earlier era, trace through which commit changed it and fix that specific
  interaction.

## The Problem

The session from 2026-07-27 to 2026-07-28 implemented ~25 commits on top of
the previously stable baseline. Many were clean improvements. Some were not.
The session ended with:

- kalman_filter_runtime at 3.80x (regressed from 1.01x)
- ring_buffer at 1.28x (regressed from ~1.10x)
- nbody_newton at 1.06x (regressed from 1.05x best)
- nbody_sqrt_idio at 0.85x (regressed from 0.68x best)
- nbody_sqrt at 0.99x (regressed from 0.84x best)

The agent(s) in that session made three critical mistakes:

1. **Removed the stride gate.** The stride gate (max_field_stride > 1 → reject)
   was the ONLY protection against kalman's matrix multiply groups merging into
   width-12 groups. Removing it allowed kalman's SLP to fire, which created
   `<12 x float>` inserts that LLVM's auto-vectorizer then expanded into
   expensive vector ops.

2. **Updated the baseline to worse commits.** Each baseline update pinned a
   state with more regressions than the previous baseline. The user asked for
   `b39461e2` to be restored — this was the commit with the best overall results.

3. **Dismissed regressions as "noise"** instead of investigating root causes.

## The Baseline (`b39461e2`)

Commit `b39461e2` — "SLP stride gate — all 19 benchmarks at parity or better"
— is the stable starting point. This commit has:

- **Stride gate**: blocks kalman's matrix multiply groups (max_field_stride > 1)
- **Depth×width ≥ 10**: ensures compute gain exceeds insert/extract overhead
- **Width cap ≤ 8**: prevents non-standard vector widths
- **Hazard gate**: gating for non-alwaysinline txns
- **Guard condition fix**: sparse_dispatch at 0.80x, all MATCH
- **Harness fix**: queue_drain_idio fixed from 654x to 1.0x
- **SLP re-enabled** with the above three gates — nbody gets ~250 vector ops

What it does NOT have:
- Three-category cold-path outlining (txns use `#0` not `#11`)
- `PrintInt#` intrinsics (uses `__print_int` FFI)
- `!range` / `!prof` metadata (no LLVM metadata emitted)
- `noundef` / `dereferenceable` on function parameters
- DataLayout-driven `int_bits`
- `Bits → Bit` rename

All of these were added in later commits and need to be cherry-picked one at a
time with verification.

## Worktree Layout

```
/home/randozart/Desktop/Projects/
├── briv-compiler/                  # Main repo (main branch)
│   ├── target/release/brivc
│   ├── .opencode/
│   │   ├── HANDOFF.md
│   │   └── plans/2026-07-28-baseline-recovery.md
│   └── benchmarks/
│
├── briv-compiler-baseline/         # Read-only A/B comparison at b39461e2
│   └── target/release/brivc        # (release build ready)
│
├── briv-compiler-recovery/         # Recovery worktree (recovery-branch)
│   └── target/release/brivc        # (release build ready — baseline + our commits)
│
└── briv-compiler-derive/           # Feature worktree (derivation + stochastic opt)
    └── commits Phases A–I           # (12 code commits on c3155e99 base)
```

**All operations happen in `../briv-compiler-recovery`.** Use the baseline
worktree only for comparison via `bash benchmarks/compare_baseline.sh <name>`.

**VERY IMPORTANT:** All `compare_baseline.sh` and `build_and_bench.sh` commands
must be run from the **main worktree** (`../briv-compiler`) because the scripts
use relative paths to find the baseline worktree at `../briv-compiler-baseline`.
The recovery worktree has its own `target/release/brivc` for compilation, but
benchmark scripts reference paths relative to the main repo.

**Recommended workflow for single-benchmark testing from the recovery worktree:**

```bash
# Compile with recovery compiler
cd /home/randozart/Desktop/Projects/briv-compiler-recovery
rm -f benchmarks/ring_buffer.ll benchmarks/ring_buffer
./target/release/brivc build benchmarks/ring_buffer.bv --llvm --out benchmarks
clang -O3 -flto -march=native -ffast-math \
    benchmarks/ring_buffer.ll lib/runtime/briv_rt.c \
    -o benchmarks/ring_buffer
BOUND=50000000 /usr/bin/time -f "%e" ./benchmarks/ring_buffer 2>&1

# Compare against baseline (must run from main worktree)
cd /home/randozart/Desktop/Projects/briv-compiler
bash benchmarks/compare_baseline.sh ring_buffer
```

## Recovery Plan — Each step is ONE commit with its own benchmark run

### Step 1: Cherry-pick clean wins (zero risk)

These changes are pure improvements with no SLP interaction. Each gets its own
commit, its own benchmark run, and its own `git revert` if it regresses.

#### 1a: DataLayout-driven `int_bits` + remove `is_wasm()`
- **Source commit**: `5bde7aed`
- **Files**: `src/backend/llvm/context.rs`, `mod.rs`, plus test fixes
- **What**: Parse `p:<abi>:<pref>` from target data layout string to auto-set
  `int_bits`. Replaces hardcoded `is_wasm()` function.
- **Risk**: None — same `i64` default on x86_64
- **Verification**: `cargo test --lib`, `bash benchmarks/build_and_bench.sh --runtime`

#### 1b: `noundef` + `dereferenceable` on `ptr %state` params
- **Source commit**: `e24c3fd7`
- **Files**: `emit_toplevel.rs`, `dispatch.rs`, `mod.rs`, `context.rs`
- **What**: Add `noundef` and `dereferenceable(N)` to all function definitions
  that take `ptr %state`. N = state_size_bytes computed from field_types.
- **Risk**: `dereferenceable(0)` is invalid — guard on `state_size_bytes > 0`
- **Verification**: Check .ll output for correct attributes, run benchmarks

#### 1c: `Bits` → `Bit` rename
- **Source commit**: `30f9b839`
- **Files**: ~15 .rs files, ~5 .bv files
- **What**: Mechanical rename of `"Bits"` to `"Bit"` for the atomic type.
  Does NOT rename `#Bits` hashword or `bits <~ N` property.
- **Risk**: None (mechanical)
- **Verification**: `cargo test --lib`

#### 1d: `!range` metadata from contracts
- **Source commit**: `b9a2dd7d`
- **Files**: `helpers.rs`, `dispatch.rs`, `context.rs`, `emit_toplevel.rs`
- **What**: Wire the existing `field_to_meta_idx` into `load_field_type()` so
  every state field load carries `!range` metadata derived from precondition
  contracts.
- **Risk**: `extract_ranges` must resolve constants (e.g. `TOTAL` → 50000000)
- **Verification**: Check `.ll` for `!range` metadata, run benchmarks

#### 1e: `!prof` branch weights from postcondition
- **Source commit**: `9ff835ac`
- **Files**: `emit_toplevel.rs`
- **What**: For `when count % N == C` guards with `[count == total]`
  postcondition, emit `!prof !{!"branch_weights", i32 T, i32 N}` on the
  guard's `br` instruction.
- **Risk**: Low — no effect if pattern not found
- **Verification**: Run benchmarks

#### 1f: Metadata syntax `key <~ value;` → `!> key: value;`
- **Source commit**: `abd1a090`
- **Files**: Lexer, parser, display, ~25 .bv files
- **What**: Change the metadata assignment operator.
- **Risk**: None (syntax only)
- **Verification**: `cargo test --lib`

### Step 2: Three-category cold-path outlining (`#11`)

This is the first change with real performance impact and risk.

- **Source commit**: `cd4edded` (but this included the stride gate removal —
  cherry-pick ONLY the three-category outlining logic, NOT any stride gate
  changes)
- **Files**: `emit_toplevel.rs`, `context.rs`
- **What**: Extend the cold-path outlining to resolve identifiers in guard
  bodies that are state fields (GEP+load), let bindings (lookup at emission
  time), or compile-time constants (ctx.constants lookup). This allows the hot
  txn body to be annotated `#11 = memory(argmem: readwrite)` when all FFI has
  been outlined.
- **Risk**: `#11` changes LLVM's alias analysis, which can enable LLVM's
  auto-vectorizer on some benchmarks (kalman). The stride gate already protects
  against OUR SLP, but LLVM's auto-vectorizer is not affected by the stride
  gate. To fully protect kalman from LLVM's auto-vectorizer, either:
  - Add the cross-per-field density check: when `cross_ops / float_fields > 4`,
    force `#0` instead of `#11`
  - Add `"prefer-vector-width"="1"` attribute to the txn function for dense
    matrix benchmarks
- **Verification**: nbody_newton should improve from 1.05x to ~1.02x. kalman
  must NOT regress. If it does, the cross-per-field check must be added before
  this step can land.

### Step 3: `PrintInt#` intrinsics

- **Source commit**: `0ebfba39`
- **Files**: `print_plugin.rs`, `intrinsic_signatures.rs`, `intrinsics.rs`,
  `interpreter/intrinsics.rs`
- **What**: Replace `__print_int` / `__print_float` / `__print_char` FFI calls
  with `PrintInt#` / `PrintFloat#` / `PrintChar#` intrinsics in the print
  plugin. The backend emits the same `@__print_int` calls — the AST and
  outlining logic sees `#`-suffixed intrinsics, which are recognized as
  non-FFI by `is_ffi_call`.
- **Risk**: The observable-intrinsic fix (Phase E regression fix, commit
  `9f4d7dfe`) is required for correct cold-path outlining behavior. Without it,
  guard outlining breaks.
- **Verification**: ring_buffer should improve from ~1.10x to ~1.05x. All MATCH.

### Step 4: Phase C — Persist AnalysisResults for precise `!prof`

- **Source commit**: `004a4d12`
- **Files**: `context.rs`, `mod.rs`, `emit_toplevel.rs`, `transition_graph.rs`
- **What**: Persist `transition_graph` and `iter_bounds` from the analysis
  pipeline onto `CompilerContext` so `emit_toplevel.rs` can compute precise
  `!prof` weights using the induction variable's `bounded_pre`, `increments`,
  and iteration count.
- **Risk**: Low — purely additive metadata
- **Verification**: Benchmarks unchanged (metadata only affects LLVM optimization)

### Step 5: Two-pass SLP consumer analysis (if needed for kalman)

Only if `#11` from Step 2 causes kalman to regress. The two-pass analysis was
implemented in commit `edf671de` but had no effect on kalman because the 3.8x
regression was from LLVM's auto-vectorizer, not from our SLP. If the cross-per-
field check in Step 2 doesn't fully protect kalman, investigate the `prefer-vector-width`
approach or add `#llvm.loop.disable_nonforced` to the main loop.

## How to run benchmarks

```bash
# Full suite (takes ~5 minutes per run)
bash benchmarks/build_and_bench.sh --runtime

# Single benchmark for quick iteration
rm -f benchmarks/ring_buffer.ll benchmarks/ring_buffer
./target/release/brivc build benchmarks/ring_buffer.bv --llvm --out benchmarks
clang -O3 -flto -march=native -ffast-math benchmarks/ring_buffer.ll \
    lib/runtime/briv_rt.c -o benchmarks/ring_buffer
BOUND=50000000 hyperfine -w 1 -r 5 ./benchmarks/ring_buffer

# Compare against baseline
bash benchmarks/compare_baseline.sh ring_buffer
```

## The tests

```bash
cargo test --lib          # 1045 tests, ~25 seconds
cargo build --release     # ~45 seconds for release binary
```

## Maps of key files

| File | What it does | Risk level |
|------|-------------|------------|
| `src/backend/llvm/emit_toplevel.rs` | Tx function emission, cold-path outlining, attribute selection | HIGH |
| `src/backend/llvm/loop_engine/counter.rs` | SLP dispatch, stride gate, emit_countable_body | HIGH |
| `src/backend/llvm/mod.rs` | generate(), field index population, analysis pipeline | MEDIUM |
| `src/backend/llvm/context.rs` | CompilerContext, FunctionContext, all state fields | MEDIUM |
| `src/backend/llvm/helpers.rs` | load_field_type(), store helper, metadata emission | LOW |
| `src/plugin/print_plugin.rs` | PrintLn! resolution (intrinsics vs FFI) | MEDIUM |
| `src/analysis/slp_isomorphism.rs` | SLP group finding, isomorphism, merge step | HIGH |
| `src/analysis/transition_graph.rs` | bounded_pre, increments, statement_contains_ffi | LOW |
| `src/analysis/hazard.rs` | hazard_spec, cross-field counting, slp_hazard_fns | LOW |

## Key contacts

For questions about the baseline rebuild, the original session log is in
`AGENTS_HISTORY.md` and `docs/plans/2026-07-27-cold-path-refinement.md`
(which has all 6 benchmark runs documented chronologically).

---

## Worktree Topology

Three worktrees exist in parallel:

| Worktree | Path | Branch/HEAD | Purpose |
|----------|------|-------------|---------|
| **Main** | `../briv-compiler` | `main` at `70ead990` | Integration target — feature merges here first |
| **Baseline** | `../briv-compiler-baseline` | Detached HEAD at `b39461e2` | Read-only A/B comparison — never commit here |
| **Recovery** | `../briv-compiler-recovery` | `recovery-branch` at `b39461e2` | Builds the 8-step recovery plan |
| **Derive (feature)** | `../briv-compiler-derive` | Detached HEAD at `7c24d9e5` | `:=` derivation block + stochastic optimization feature |

**Critical rule:** Only work in your assigned worktree. Never modify files in another
worktree's directory. Git worktrees share the object database but have independent
working trees and indexes.

## Integration Strategy

The recovery work is sequentialized with the derivation feature worktree:

### Step A — Feature merges to main (derivation agent's responsibility)

The derivation worktree (`../briv-compiler-derive`, 12 code commits on
Phases A–I of the `:=` derivation + stochastic optimization feature, forked
from `c3155e99`) merges into `main` first. The feature agent resolves any
conflicts between their 12 commits and the 17 post-baseline commits on `main`.

```bash
# In ../briv-compiler-derive:
git branch derive-feature
git push .. HEAD:refs/heads/derive-feature
cd ../briv-compiler
git merge derive-feature    # resolve conflicts, commit
```

### Step B — Recovery merges into main (this agent's responsibility)

After the feature is on `main`, the 8-step recovery branch merges in.
Conflict resolution is guided by this rule:

**For every conflicted file, apply this decision matrix:**

| File(s) | Take | Rationale |
|---------|------|-----------|
| `emit_toplevel.rs`, `context.rs`, `mod.rs`, `dispatch.rs` | recovery branch | Our correct attribute selection, stride gate, metadata, state params, DataLayout |
| `helpers.rs` | recovery branch | Our `!range`/`!prof` metadata wiring |
| `*slp_isomorphism*`, `*counter*` | recovery branch | Our working stride gate + gates |
| `lexer.rs`, parser files | **merge both** | Feature adds `:=`, our Step 6 adds `!>` — both tokens must coexist |
| Feature-new files (synthesis, MCMC, config) | main | Keep derivation feature intact |
| MetadataRegistry hooks | main | Keep feature's registry wiring, weave into our clean backend code |

```bash
cd ../briv-compiler-recovery
# Build 8 steps on recovery-branch (see plan document)
# ... each step verified independently ...

cd ../briv-compiler
git fetch ../briv-compiler-recovery recovery-branch
git merge recovery-branch
# Resolve conflicts per the matrix above
bash benchmarks/build_and_bench.sh --runtime && bash benchmarks/build_and_bench.sh --correctness
```

### Known conflict surface

1. **Lexer/parser**: Feature adds `:=` token (derivation syntax), Step 6 of recovery
   changes `Token::TildeArrow` to `Token::ExclaimArrow` and adds `:` after key in
   metadata. Both modify `lexer.rs`, `statements.rs`, `metadata.rs`, `definitions.rs`.
   Resolution: keep both sets of changes (different tokens, different contexts).

2. **LLVM backend overlap**: Feature's Phase H.0 (`9d54f263`) wires MetadataRegistry
   into the LLVM backend. Our Steps 1–5, 7 all modify `emit_toplevel.rs`, `context.rs`,
   `dispatch.rs`, `mod.rs`. Resolution: take our clean recovery versions, re-apply
   MetadataRegistry hooks manually if needed.

3. **`.bv` files**: Feature may add new stdlib types; our Step 6 changes metadata
   syntax across ~25 `.bv` files. Resolution: apply Step 6's `!>` syntax change
   across ALL `.bv` files including feature-added ones — or skip feature files if
   they don't use `<~` metadata.
