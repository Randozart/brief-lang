# Flat Node Decomposition — Principled Replacement for Batch-Loop Heuristics

**Date:** 2026-07-30
**Status:** Plan — pre-implementation
**Author:** Agent (investigation session with user)
**Feature worktree:** (created after this plan is committed)

---

## 1. Executive Summary

The current batch-loop optimization (`emit_countable_batched_main`) uses heuristics to split a composite convergence node into an inner pure-compute loop and outer boundary guard checks. This produces correct results for nbody_newton (0.83× C) but has a correctness bug for knucleotide and mandelbrot (missing the count=0 periodic print), and the heuristics are fragile, poorly-framed, and require repeated iteration to maintain.

The root cause is not the batch-loop mechanism — it's the **framing**. The batch-loop was written as a collection of heuristics (`extract_batch_size_from_guards`, `is_safe_to_hoist`, `let_to_field` remapping, count=0 peel) instead of being derived from Brief's own reactor design.

**Brief's reactor design (from the user, re-iterated):**
1. If two nodes can fire together simultaneously, they should.
2. If two nodes firing together would lead to a race (one reading / one writing / both writing the same field), deny compilation. **Writing is a XOR condition.**
3. Nodes should be hoisted with additional preconditions/postconditions injected to logically separate them.
4. This has the additional advantage of being foldable if needed.
5. **Good compiler design: extract everything into flat nodes first where possible.**

A composite `node { compute; when cond { io }; term; }` is a **latent second node trapped inside the first node's body**. The `when` guard is a reactive sub-contract that should be its own flat node. The batch-loop's mechanism (folded compute loop + boundary guard checks in one `@main`) is the correct CODEGEN of this decomposition — it just needs to be DERIVED from the decomposition semantics rather than guessed.

---

## 2. Background: The Reactor Model

### 2.1 What Already Exists

The reactor infrastructure (dispatch.rs, strategy.rs, transition_graph.rs) already implements most of the design:

| Mechanism | Location | What it does |
|-----------|----------|-------------|
| **Sequential reactor** | `dispatch.rs:33-155` | Evaluates ALL preconditions on tick-start snapshot, then fires bodies in declaration order |
| **Parallel reactor** | `dispatch.rs:326-475` | Uses `%fired_mask` bitmask to prevent write-after-write hazards; fires conflict-free txns |
| **Dispatch mode selection** | `strategy.rs:50-105` | Selects Parallel when all pairs are conflict-free, else Sequential |
| **Fusion** | `helpers.rs:459-492`, `backend/mod.rs:313-353` | Fuses non-conflicting node pairs into one loop |
| **Write-set extraction** | `transition_graph.rs:1265-1280` | Provenance-aware per-node write sets |
| **Read-set extraction** | `backend/mod.rs:288-309` | `collect_read_identifiers` per node |
| **Folded single-node path** | `mod.rs:2678-2787` | `graph.nodes.len() == 1` → tight phi-node loop in `@main`, NO memcpy-per-tick |

### 2.2 The Critical Performance Constraint

The **reactor path** (multi-node) emits a main loop with a **272-byte memcpy snapshot every tick** (`loop_engine/mod.rs:241-246`):

```llvm
.loop:
  call @llvm.memcpy(%state_save, %state, 272, false)   ; snapshot EVERY tick
  call @reactor_tick(ptr %state)
  br label %.loop
```

For 50M iterations this is **13.6 GB of copy traffic** — a massive regression.

The **folded single-node path** (`graph.nodes.len() == 1`) emits a tight phi-node loop with no memcpy. This is how nbody_newton, knucleotide, and mandelbrot currently run fast.

**Therefore: the decomposition must NOT create actual separate `TopLevel::Transaction` entries.** That would make `graph.nodes.len() == 2`, forcing the reactor path. The decomposition must be INTERNAL — the folded compute loop with boundary checks must stay in one `@main`, which is exactly what the batch-loop already does.

### 2.3 The Existing Dispatch Mode Selection

`strategy.rs:50-105` (`select_dispatch_mode`):

```rust
if !a_writes.is_disjoint(&b_writes) { cf = false; break; }        // write-write: ALWAYS checked
if !a_pre_ids.is_disjoint(&b_pre_ids) {                            // read-write: ONLY when preconditions share IDs
    if !a_writes.is_disjoint(&b_reads) { cf = false; break; }
    if !b_writes.is_disjoint(&a_reads) { cf = false; break; }
}
```

**Gap (user request #1):** Read-write overlap is only checked when the two nodes' preconditions share identifiers. Two nodes with DISJOINT preconditions but overlapping read-write sets would incorrectly be classified as conflict-free → Parallel → potential race. The read-write checks must be **unconditional**.

---

## 3. Problems Investigated

### 3.1 The Batch-Loop Heuristics (Current State)

The batch-loop (`emit_countable_batched_main` in counter.rs) was written across several commits with these heuristic components:

| Component | Function | Fragility |
|-----------|----------|-----------|
| Guard detection | `split_hoistable` | Only detects guards with function calls; complex body analysis |
| Batch-size extraction | `extract_batch_size_from_guards` / `extract_batch_size` | Only recognizes `count % N == 0`; falls back to default 5M |
| Safety check | `is_safe_to_hoist` / `expr_uses_only_safe_refs` | Requires guard body to reference only state fields + `let_to_field` remappables + self-defined; broke on `Expr::Block` (post-print-plugin) |
| Identifier remapping | `let_to_field` map + `remap_stmt_identifiers` | Maps `energy → last_energy`; must handle `Let`, `PluginIntercept`, `Block` |
| Count=0 handling | (missing) | Inner loop runs count 0..N-1 without guard checks; `0 % N == 0` print at count=0 is MISSED |

### 3.2 The Count=0 Bug (knucleotide, mandelbrot)

Both benchmarks have `when count % 5000000 == 0 { PrintLn!(...) }`.

- **C reference:** fires at `count = 0, 5M, 10M, ..., 50M` — 11 times.
- **Brief batch-loop:** fires at `count = 5M, 10M, ..., 50M` — 10 times (missing count=0).

The inner loop runs `batch_size` iterations from `count = 0` to `count = 4999999` with NO guard checks. The guard only fires in `.inner_exit_124` when `count = 5000000`. The `0 % 5000000 == 0` check at count=0 is inside the inner loop where guards are removed.

### 3.3 The Count=0 Peel Is Sound

Running 1 iteration before the batch loop catches the `count % N == 0` at count=0:

| Guard condition | At count=0 after 1 iteration | Result |
|---|---|---|
| `count % 5M == 0` | `1 % 5M != 0` | Guard doesn't fire — correct |
| `count == bound` | `1 != bound` | Guard doesn't fire — correct |
| `count % 1 == 0` (every iter) | `1 % 1 == 0` | Guard fires — correct |
| `count > 0 && count % 5M == 0` | `1 > 0 && 1 % 5M != 0` | Guard doesn't fire — correct |
| No guard at all | N/A | One extra body execution — harmless |

**This is NOT a hack** — it's the io node's precondition evaluated at the initial state. `0 % N == 0` is true at count=0, so the io node fires before the first batch. The peel is the principled consequence of the decomposition.

**Overhead:** 1 extra iteration per benchmark. For batch_size = 5M, negligible.

### 3.4 The Special-Casing Concern

The user correctly pushed back on special-casing `count`. Investigation confirmed:

- `extract_batch_size` uses `bp.var` (the dispatch's loop counter) — NOT a hardcoded `"count"` string. It works for any counter variable name.
- The recognized patterns (`counter % N == 0`, `counter == bound`) are structural, derived from the dispatch's actual counter variable.
- Guards with independent conditions (not modulo/equality on the counter) are NOT recognized and stay in the inner loop — safe fallback.

But this is still framing-dependent. The **principled** framing: the io node's precondition IS the batch boundary. `count % N == 0` is one recognized precondition form. Other forms (equality, independent flags) would define different boundaries or no batching.

### 3.5 The Monolithic %State Struct

`push_field_type` (mod.rs:918) forces all state fields to `i64` in a single `%State` struct. This blocks LLVM SROA, forcing the backend to manage phi nodes manually. The batch-loop is a workaround for this. A future flat-allocas refactor would eliminate the need for manual phi management, but it is OUT OF SCOPE for this plan.

---

## 4. The Principled Solution

### 4.1 Framing: The Decomposition IS the Design

A composite node `{ compute; when cond { io }; }` is decomposed into flat nodes:

```brief
// Original composite:
node kn [count < N][count == N] {
    compute body...
    when count % 5000000 == 0 { PrintLn!(chksum); };
    count = count + 1;
    term;
};

// Flat decomposition (conceptual):
node kn_compute [count < N][count == N] { compute body; count++; term; };
node kn_io [count % 5000000 == 0][true] { PrintLn!(chksum); term; };
```

**The batch boundary IS the io node's precondition interval.** `kn_io` fires when `count % 5M == 0`. The compute loop runs until `count` reaches the next multiple of 5M, THEN evaluates `kn_io`'s precondition. The batch_size is not guessed — it's the io precondition's interval.

### 4.2 What Stays in `@main`

The folded compute loop + boundary guard checks stay in one `@main` (as the batch-loop already does). This preserves:
- `graph.nodes.len() == 1` → foldable → tight loop
- NO memcpy-per-tick
- The io boundary check is emitted at the loop boundary (`.inner_exit_124`)

### 4.3 Read-Write Conflict Analysis (Fix #2)

**`select_dispatch_mode` (strategy.rs):** Move the two read-write checks out of the `pre_ids` block so they run unconditionally:

```rust
if !a_writes.is_disjoint(&b_writes) { cf = false; break; }
if !a_writes.is_disjoint(&b_reads) { cf = false; break; }
if !b_writes.is_disjoint(&a_reads) { cf = false; break; }
```

**`resolve_fusable_pairs` (helpers.rs):** Add read-write overlap to the fusion block:

```rust
let aw = collect_assigned_identifiers(&ta.body);
let bw = collect_assigned_identifiers(&tb.body);
let ar = collect_read_identifiers(&ta.body);
let br = collect_read_identifiers(&tb.body);
if aw.iter().any(|w| bw.contains(w)) { return false; }       // write-write
if aw.iter().any(|w| br.contains(w)) { return false; }       // A writes / B reads
if bw.iter().any(|w| ar.contains(w)) { return false; }       // B writes / A reads
```

**Why this matters for the decomposition:** `kn_compute` writes `chksum`, `kn_io` reads `chksum`. This read-write dependency means:
- They must NOT be fused back into a composite (would recreate the problem)
- They must NOT run in Parallel (race on chksum)
- They run SEQUENTIALLY: compute fires, commits chksum, then io fires, reads committed chksum

### 4.4 The Count=0 Case (Principled Peel)

Before the first inner loop, evaluate the io node's precondition at the initial state. Since `0 % N == 0` is true, the io node fires. Then the compute loop runs. This is the io precondition at the initial boundary, NOT a heuristic peel.

**Implementation:** Emit the guard checks in the ENTRY block (or outer header) BEFORE the first inner loop, in addition to `.inner_exit_124`. The guard body reads state fields (via `let_to_field` remapping), which are valid at the entry after init.

### 4.5 What the Decomposition Derives

| Current heuristic | Principled derivation |
|---|---|
| `extract_batch_size_from_guards` guesses `N` | The io node's precondition `count % N == 0` DEFINES the boundary |
| `is_safe_to_hoist` guesses which guards are safe | The io node's read-set (what it reads at the boundary) determines what must be available at the boundary |
| `let_to_field` remapping remaps names | The io node reads state fields written by compute — the read-write dependency is what the remapping resolves |
| Count=0 peel | The io precondition at the initial state (`0 % N == 0`) fires at the first boundary |
| `select_dispatch_mode` read-write gap | Block read-write overlap ALWAYS (fix #2) |

---

## 5. Implementation Plan

### Phase 1: Fix Read-Write Conflict Detection (~30 min)

**Files:**
- `src/backend/llvm/strategy.rs` — `select_dispatch_mode` (lines 50-105)
- `src/backend/llvm/helpers.rs` — `resolve_fusable_pairs` (lines 459-492)

**Changes:**
1. In `select_dispatch_mode`: hoist the two read-write checks (`a_writes` vs `b_reads`, `b_writes` vs `a_reads`) out of the `if !a_pre_ids.is_disjoint(&b_pre_ids)` block so they run unconditionally.
2. In `resolve_fusable_pairs`: add `collect_read_identifiers` for both txns; reject fusion if A writes what B reads OR B writes what A reads.

**Verification:** `cargo test --lib` — all tests pass. No benchmark change (decomposed nodes don't exist yet — this is foundational).

### Phase 2: Make Count=0 Fire Correctly (~1h)

**File:** `src/backend/llvm/loop_engine/counter.rs` — `emit_countable_batched_main`

**Change:** Emit the guard checks (from `batch_info.outer_guards`) at the ENTRY block (after `init_state`, before the first inner loop) IN ADDITION to `.inner_exit_124`.

```llvm
entry:
  %state = alloca %State
  call void @init_state(ptr %state)
  ; NEW: evaluate io preconditions at initial state (count = 0)
  ; if count % batch_size == 0: emit guard body
  ; (guard body reads state fields — valid after init)
  br label %.oh_0
```

**Constraint:** The guard body must reference only state fields (via `let_to_field` remapping) or constants — no let-bindings from the compute loop (they don't exist yet at entry). The existing `is_safe_to_hoist` check already enforces this for self-contained guards, but the ENTRY emission must NOT assume compute-loop let-bindings are available.

**Implementation detail:** At entry, the guard body's identifiers resolve to state field INITIAL values (before any compute). For knucleotide, `chksum = 0` at init, and `PrintLn!(chksum)` prints 0 — matching C's `chksum` value at count=0? NO — C prints `chksum` AFTER the first compute. Need to verify this resolves correctly.

**Alternative:** If the entry emission can't produce the correct value (compute-loop let-binding needed), run ONE compute iteration before the guard check at count=0:

```llvm
entry:
  init_state
  ; run 1 compute iteration
  emit body (count = 0)
  count++
  ; now count = 1, chksum updated
  ; check guard: (count - 1) % batch_size == 0 → 0 % N == 0 → true
  ; print chksum (the updated value from iteration 0) ✓
  br label %.oh_0
```

**Verification:** `BOUND=50 ./target/release/briefc build benchmarks/knucleotide.bv` — output should now have 11 lines matching C (including count=0). Same for mandelbrot.

### Phase 3: Formalize the Decomposition Semantics (~2h)

**File:** `src/analysis/loop_peeling.rs` — refactor the heuristics into a decomposition-derived pass.

**Change:** Rename/reframe the batch-loop analysis as a **node decomposition**:
- `decompose_io_nodes(body, field_index_map)` — extracts `when` guards with side effects into conceptual io nodes (replaces `split_hoistable`)
- `io_node_boundary(io_node_precondition, counter_var)` — derives the batch interval from the io precondition (replaces `extract_batch_size_from_guards`)
- `io_node_requirements(io_node_body)` — computes the read-set; what must be available at the boundary (replaces `is_safe_to_hoist` + `let_to_field` construction)

The key reframing: instead of "detect a modulo guard and guess a batch size," it's "decompose the io node, derive its boundary from its precondition, derive its requirements from its read-set."

**Verification:** All existing batch-loop tests pass. nbody_newton still 0.83× C.

### Phase 4: Remove Redundant Heuristics (~30 min)

After Phase 3, remove or de-emphasize heuristic framing:
- `extract_batch_size_from_guards` — replaced by `io_node_boundary`
- `is_safe_to_hoist` — replaced by `io_node_requirements`
- The count=0 special-case comment in the batch-loop — now documented as "io precondition at initial state"

Keep the function names as aliases if tests reference them, or update tests.

### Phase 5: Full Benchmark Verification (~30 min)

```bash
cargo test --lib
bash benchmarks/build_and_bench.sh --correctness   # all MATCH (incl. knucleotide, mandelbrot)
bash benchmarks/build_and_bench.sh --runtime       # no regressions
```

**Expected outcomes:**
- knucleotide: MATCH, 11 output lines (was MISMATCH, 10 lines)
- mandelbrot: MATCH, 11 output lines (was MISMATCH)
- nbody_newton: 0.83× C (unchanged)
- All other benchmarks: unchanged

---

## 6. Verification Matrix

| Benchmark | Before | After | Correctness |
|-----------|--------|-------|-------------|
| nbody_newton | 0.83× C | ~0.83× C | MATCH |
| nbody_sqrt | 0.77× C | ~0.77× C | MATCH |
| nbody_sqrt_idio | 0.72× C | ~0.72× C | MATCH |
| knucleotide | 1.20× C (MISMATCH) | ~1.20× C (MATCH) | 11 lines |
| mandelbrot | 1.04× C (MISMATCH) | ~1.04× C (MATCH) | 11 lines |
| ring_buffer | 1.18× C | ~1.18× C | MATCH |

---

## 7. Risks and Mitigations

| Risk | Probability | Mitigation |
|------|:-----------:|------------|
| Entry emission of guard body can't resolve compute-loop let-bindings | Medium | Run 1 compute iteration before the count=0 guard check (Phase 2 alternative) |
| Read-write conflict change breaks existing parallel programs | Low | `cargo test --lib` covers parallel dispatch; benchmark suite validates |
| Fusion read-write block changes fusion decisions | Medium | Verify existing fused programs still fuse (write-write-disjoint pairs with no read-write overlap) |
| Reframing (Phase 3) changes behavior subtly | Medium | Keep behavior identical; only rename/restructure the analysis |
| memcpy-per-tick regression if decomposition creates real TopLevel nodes | High | MUST keep `graph.nodes.len() == 1` — decomposition is internal, not AST-level |

---

## 8. Out of Scope

- **Flat allocas**: Replacing the monolithic `%State` struct with per-field allocas. This would let LLVM SROA handle phi placement and eliminate manual phi management, making the batch-loop (and even the decomposition) unnecessary. But it's a large refactoring and this plan focuses on the decomposition semantics.
- **Auto-vectorization**: LLVM's loop vectorizer already handles the folded compute loop. No changes needed.
- **New protocol types**: The casting graph already handles the protocols used by these benchmarks.

---

## 9. Historical Context

This plan documents the investigation of commit `c4cec5d9` (batch-loop guard hoisting fix, nbody_newton at 0.83× C). The batch-loop was introduced in commits `12e5435f`+ (feat/derivation-synthesis merge) and iterated on through `7e9de00b`, `f9d994ff`, and `aa174b14`. The knucleotide/mandelbrot count=0 MISMATCH was discovered during the 2026-07-30 baseline investigation.

**Key commits:**
- `066b86a7` — reference state: all benchmarks MATCH, nbody_newton 1.22× C
- `c4cec5d9` — batch-loop guard hoisting fix, nbody_newton 0.83× C, knucleotide/mandelbrot MISMATCH

---

## 10. Appendix: Brief's Reactor Design (as re-iterated by the user)

1. **If two nodes can fire together simultaneously, they should.** The reactor supports concurrent node firing.
2. **If two nodes firing together would lead to a race condition due to one reading or one writing or both writing, deny compilation. Writing is a XOR condition.** Write conflicts (read-write or write-write on the same field) are denied at compile time.
3. **The nodes should be hoisted so that they get additional preconditions/postconditions injected to logically separate them.** Injected contracts resolve conflicts without changing semantics.
4. **This has the additional advantage of being foldable if needed.** Non-conflicting nodes can fold back together.
5. **Extract everything into flat nodes first where possible.** Composite nodes with `when` guards are latent multi-node reactors.
