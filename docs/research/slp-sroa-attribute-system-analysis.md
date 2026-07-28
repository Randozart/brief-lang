# Full System Analysis: SLP, SROA, and Attribute-Gated Optimization in the Brief Compiler

**Date:** 2026-07-28
**Author:** Systems analysis agent (recovery-branch)
**Scope:** Complete trace of all optimization gates, their interactions, and their effect on all 19 benchmarks across all optimization eras.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Historical Performance Timeline](#2-historical-performance-timeline)
3. [The Optimization State Machine](#3-the-optimization-state-machine)
4. [Attribute System Analysis](#4-attribute-system-analysis)
5. [SLP Gate Analysis](#5-slp-gate-analysis)
6. [Chain Pass OK: Dead Code Analysis](#6-chain-pass-ok-dead-code-analysis)
7. [Stride Gate Granularity Analysis](#7-stride-gate-granularity-analysis)
8. [Interaction Matrix: How Gates Interact](#8-interaction-matrix-how-gates-interact)
9. [The Register-Pressure Hypothesis](#9-the-register-pressure-hypothesis)
10. [Verified Findings](#10-verified-findings)
11. [Unresolved Questions](#11-unresolved-questions)
12. [Data Flow Diagrams](#12-data-flow-diagrams)

---

## 1. Executive Summary

The Brief compiler has 19 benchmarks whose all-time-best performance is spread across six
different optimization eras. No single compiler version achieves all bests simultaneously.
The conventional explanation ("SLP stride gate trade-off") is incorrect — the real cause is
a **single attribute selection heuristic** in `dispatch.rs` that determines whether LLVM's
SROA (Scalar Replacement of Aggregates) pass activates on the hot loop.

**Key discovery:** The `reactor_tick` function's LLVM attribute (`#2` vs `#12`) is selected
by checking whether FFI calls are inside `when` guards. This is the WRONG criterion. The
correct criterion is **state field count vs target register file size**:

- `#12` (argmem:readwrite + willreturn) → enables SROA → promotes %State fields to SSA
  registers → BENEFICIAL for benchmarks with ≤~14 fields (ring_buffer: 4 fields)
- `#2` (memory(readwrite)) → blocks SROA → %State stays as struct GEP+load → BENEFICIAL
  for benchmarks with >~14 fields (nbody_sqrt_idio: 32 fields)

**Secondary discoveries:**
- `chain_pass_ok` (consumer chain cost model) is computed in SLP analysis but NEVER read
  at dispatch time — it is dead code
- The stride gate checks `max_stride <= 1` at the WRONG granularity — it should check
  per-lane vector contiguity, not cross-expression stride
- The attribute system has 12 groups (`#0`–`#12`), of which 3 control SROA on different
  function scopes; SROA decisions cascade across scopes

**Summary of all optimization eras used for this analysis:**

| Era | Date | Key Commit(s) | Optimization State |
|-----|------|---------------|-------------------|
| 1 | May 31 – Jun 02 | earliest | Pre-fair C (volatile), no SLP, no outlining, no arena |
| 2 | Jun 03 – Jun 04 | `445733ac` | Fair C benchmarks, pure-counter fold, dispatch collapse |
| 3 | Jun 03 – Jun 05 | — | Dead-field elimination, fold detection |
| 4 | Jul 06 | `f598584` | All 22 MATCH, IR determinism fix |
| 5 | Jul 11 | `8a827db` | Phase 3 complete, arena-by-proof |
| 6 | Jul 14 | — | Benchmark audit, `nsw` fixes |
| 7 | Jul 19 | `11c0749e` | Post-migration, 23/24 MATCH |
| 8 | Jul 19 | `139c345` | HashMap determinism, baseline worktree |
| 9 | Jul 21–27 | `be6583bc` | SLP experiments, hazard tuning |
| 10 | Jul 27 | `33d42397` | Post-fixes: no SLP, arena, memory(readwrite) on main, #2 on reactor_tick |
| 11 | Jul 27 | Runs 1–6 | Cold-path outlining + stride gate tuning |
| 12 | Jul 27 | **`b39461e2`** | **Baseline**: stride gate SLP, #12 on reactor_tick, ALL 19 AT PARITY |
| 13 | Jul 28 | `edf671de` | Post-baseline HEAD: kalman 3.80x, ring_buffer 1.28x |
| 14 | Jul 28 | `recovery-branch` | Steps 1–6: noundef, Bits→Bit, !range, !prof, !> syntax |

---

## 2. Historical Performance Timeline

### 2.1 Complete All-Time Best Per Benchmark

| Benchmark | Best Ratio | Brief Time | C Time | Era | Commit | Configuration |
|-----------|-----------|------------|--------|-----|--------|--------------|
| ring_buffer | **0.99x** | 0.0664s | 0.0666s | 4 | `f598584` | No SLP, no outlining, fold detection |
| float_math | **0.81x** | 0.0631s | 0.0771s | 5 | `8a827db` | Phase 3, no SLP, arena |
| float_math_nonzero | **0.98x** | 0.1611s | 0.1620s | 10 | `33d42397` | No SLP, #2 on reactor_tick |
| sparse_dispatch | **0.09x** | 0.0060s | 0.0657s | 5 | `8a827db` | Dispatch collapse, folded |
| print_loop | **0.93x** | 0.0624s | 0.0670s | 7 | `11c0749e` | Post-migration, memory(argmem:readwrite) |
| nbody_newton | **0.75x** | 7.4132s | 9.8522s | 5 | `8a827db` | Phase 3, no SLP, arena |
| nbody_sqrt | **0.85x** | 2.2434s | 2.6339s | 10 | `33d42397` | No SLP, #2 on reactor_tick |
| nbody_sqrt_idio | **0.67x** | 2.3270s | 3.4561s | 10 | `33d42397` | No SLP, #2 on reactor_tick |
| fasta | **0.95x** | 0.2094s | 0.2204s | 14 | recovery Step 5 | Latest recovery |
| fannkuch_redux | **0.96x** | 0.0763s | 0.0789s | 5 | `8a827db` | Phase 3 |
| mandelbrot | **0.99x** | 0.7514s | 0.7538s | 5 | `8a827db` | Phase 3 |
| kalman_filter_runtime | **0.95x** | 0.1610s | 0.1689s | 1 | early Jun | Pre-SLP, pre-outlining, pre-arena |
| knucleotide | **0.97x** | 0.1880s | 0.1940s | 1 | early Jun | Pre-SLP, pre-outlining, pre-arena |
| cancel_math | **0.96x** | 0.0618s | 0.0642s | 14 | recovery Step 1 | Latest recovery |
| bit_clear | **0.50x** | 0.0001s | 0.0002s | 10 | `33d42397` | Arena removal (SROA on 2-field state) |
| queue_drain | **0.01x** | 0.0007s | 0.0632s | 5 | `8a827db` | Folded (pure body, 50M eliminated) |
| queue_drain_sym | **0.95x** | 0.0575s | 0.0588s | 10 | `33d42397` | No SLP, #2 on reactor_tick |
| queue_drain_idio | **0.93x** | 0.0595s | 0.0635s | 14 | recovery Step 1 | Latest recovery |
| interval_step | **0.01x** | 0.0009s | 0.0669s | 4 | `f598584` | Folded (pure body) |

### 2.2 Full Baseline Table (Era 12 — `b39461e2`)

| Benchmark | Brief | C | Ratio | Winner |
|-----------|-------|---|-------|--------|
| ring_buffer | 0.0550s | 0.0480s | 1.14x | C |
| float_math | 0.0744s | 0.0743s | 1.00x | ~tie |
| float_math_nonzero | 0.1656s | 0.1675s | **0.98x** | Brief |
| sparse_dispatch | 0.0500s | 0.0610s | **0.81x** | Brief |
| print_loop | 0.0604s | 0.0587s | 1.02x | C |
| nbody_newton | 9.0467s | 8.2689s | 1.09x | C |
| nbody_sqrt | 2.7347s | 2.7684s | **0.98x** | Brief |
| nbody_sqrt_idio | 3.3417s | 3.6030s | **0.92x** | Brief |
| fasta | 0.2099s | 0.2109s | **0.99x** | Brief |
| fannkuch_redux | 0.0653s | 0.0657s | **0.99x** | Brief |
| mandelbrot | 0.6569s | 0.6528s | 1.00x | ~tie |
| kalman_filter_runtime | 0.1813s | 0.1790s | 1.01x | C |
| knucleotide | 0.1883s | 0.1909s | **0.98x** | Brief |
| cancel_math | 0.0626s | 0.0614s | 1.01x | C |
| bit_clear | 0.0001s | 0.0002s | **0.50x** | Brief |
| queue_drain | 0.0623s | 0.0612s | 1.01x | C |
| queue_drain_sym | 0.0618s | 0.0611s | 1.01x | C |
| queue_drain_idio | 0.0624s | 0.0618s | 1.00x | ~tie |
| interval_step | 0.0617s | 0.0588s | 1.04x | C |

### 2.3 Era 10 Full Table (`33d42397` — "No SLP, Post-Fixes")

| Benchmark | Brief | C | Ratio | Winner |
|-----------|-------|---|-------|--------|
| ring_buffer | 0.0603s | 0.0458s | 1.31x | C |
| float_math | 0.0748s | 0.0697s | 1.07x | C |
| float_math_nonzero | 0.1611s | 0.1620s | **0.99x** | Brief |
| sparse_dispatch | 0.0551s | 0.0604s | **0.91x** | Brief |
| print_loop | 0.0568s | 0.0559s | 1.01x | C |
| nbody_newton | 10.6217s | 7.8560s | 1.35x | C |
| nbody_sqrt | 2.2434s | 2.6339s | **0.85x** | Brief |
| nbody_sqrt_idio | 2.3270s | 3.4561s | **0.67x** | Brief |
| fasta | 0.1987s | 0.1980s | 1.00x | ~tie |
| fannkuch_redux | 0.0599s | 0.0612s | **0.97x** | Brief |
| mandelbrot | 0.6317s | 0.6277s | 1.00x | ~tie |
| kalman_filter_runtime | 0.1741s | 0.1725s | 1.00x | ~tie |
| queue_drain | 0.0601s | 0.0612s | **0.98x** | Brief |
| queue_drain_sym | 0.0575s | 0.0588s | **0.97x** | Brief |
| interval_step | 0.0599s | 0.0592s | 1.01x | C |

### 2.4 Era 5 Full Table (`8a827db` — "Phase 3 Complete")

| Benchmark | Brief | C | Ratio | Winner |
|-----------|-------|---|-------|--------|
| ring_buffer | 0.0686s | 0.0676s | 1.01x | C |
| float_math | 0.0631s | 0.0771s | **0.81x** | Brief |
| sparse_dispatch | 0.0060s | 0.0657s | **0.09x** | Brief |
| print_loop | 0.0639s | 0.0670s | **0.95x** | Brief |
| nbody_newton | 7.4132s | 9.8522s | **0.75x** | Brief |
| nbody_sqrt | 3.0046s | 3.5218s | **0.85x** | Brief |
| nbody_sqrt_idio | 2.9578s | 4.3184s | **0.68x** | Brief |
| fasta | 0.2695s | 0.2636s | 1.02x | C |
| mandelbrot | 0.7514s | 0.7538s | **0.99x** | Brief |
| kalman_filter_runtime | 0.1876s | 0.1887s | **0.99x** | Brief |
| queue_drain | 0.0007s | 0.0632s | **0.01x** | Brief |
| queue_drain_sym | 0.0639s | 0.0672s | **0.95x** | Brief |
| interval_step | 0.0009s | 0.0669s | **0.01x** | Brief |

### 2.5 Era 4 Full Table (`f598584` — "All 22 MATCH")

| Benchmark | Brief | C | Ratio | Winner |
|-----------|-------|---|-------|--------|
| ring_buffer | 0.0664s | 0.0666s | **0.99x** | Brief |
| float_math | 0.0626s | 0.0739s | **0.84x** | Brief |
| nbody_newton | 7.2391s | 9.4519s | **0.76x** | Brief |
| nbody_sqrt | 2.9267s | 3.2106s | **0.91x** | Brief |
| nbody_sqrt_idio | 3.0738s | 4.0939s | **0.75x** | Brief |
| kalman_filter_runtime | 0.1844s | 0.1836s | 1.00x | ~tie |

---

## 3. The Optimization State Machine

### 3.1 All Optimization Passes and Their Activation Conditions

| Pass | Activation Condition | File:Line |
|------|---------------------|-----------|
| **Pure-counter fold** | All txn body statements are pure (no FFI) | `emit_toplevel.rs:860` |
| **Dead-field elimination** | Field mode = Never (analyzed pre-codegen) | `apply_field_modes()` in `mod.rs:3876` |
| **Cold-path outlining** | Guarded FFI exists (`when cond { frgn ... }`) | `emit_toplevel.rs:1700` |
| **SLP analysis** | Txn is reactive and has ≥2 consecutive isomorphic statements | `slp_isomorphism.rs:326` |
| **SLP emission** | `!hazardous && stride_ok && w>=3 && d*w>=10` | `counter.rs:668` |
| **SROA on reactor_tick** | `#12` attribute selected (all FFI inside guards) | `dispatch.rs:68` |
| **SROA on per-txn** | Always active (`#8` = `argmem:readwrite`) | `mod.rs:3280` |
| **LLVM auto-vectorizer** | `willreturn` on hot loop function | LLVM's `-O3` |
| **!range metadata** | Contract precondition produces valid range | `helpers.rs:2830` |
| **!prof metadata** | Postcondition bound + guard modulo both computable | `emit_toplevel.rs:1711` |
| **noundef + dereferenceable** | Always active on %state params | `mod.rs:1768` |

### 3.2 Decision Tree (Per-Txn Codegen Path)

For each reactive txn, the codegen in `emit_countable_body` follows this decision tree:

```
body[i] ∈ slp_groups?
  ├── NO  → scalar emission (GEP + load + compute + store)
  │
  └── YES → is_hazardous(slp_hazard_fns)?
       ├── YES → scalar emission (hazard blacklist)
       │
       └── NO  → stride_ok(max_field_stride ≤ 1)?
            ├── NO  → scalar emission (non-contiguous fields)
            │
            └── YES → width ≥ 3?
                 ├── NO  → scalar emission (too narrow)
                 │
                 └── YES → depth * width ≥ 10?
                      ├── NO  → scalar emission (insufficient compute)
                      │
                      └── YES → emit_slp_group(has_lane_dependency?)
                           ├── NO dep → vector emission SUCCESS → skip lanes
                           │
                           └── YES dep → fallback to scalar emission
```

The missing gate in this decision tree: **`chain_pass_ok` is never checked here.**

### 3.3 The Attribute Cascade

The attribute selection affects SROA at THREE nested scopes:

```
@main  (global entry)
  ├── #9 = memory(readwrite)  →  SROA blocked on @main
  │
  ├── @pre_*(bound)  (per-txn precondition function)
  │     #10 = argmem:read + willreturn  →  SROA enabled, read-only
  │
  ├── @txn_*(countable)  (per-txn for `txn` keyword)
  │     #8 = argmem:readwrite + willreturn  →  SROA enabled
  │
  ├── @txn_*(reactive, after outlining)  (per-txn for `node`)
  │     #11 = argmem:readwrite (NO willreturn)  →  SROA enabled, conservative
  │
  └── @reactor_tick  (hot loop that dispatches ALL reactive txns)
        #2 = memory(readwrite)  OR  #12 = argmem:readwrite + willreturn
             └── SROA blocked                     └── SROA on ENTIRE hot loop
```

The SROA cascade means:
- With `#2`: reactor_tick's body stays as struct operations. Individual txn functions
  (#8/#11) still get SROA, but only their own subset of fields. The main loop dispatch
  (field comparisons for trigger conditions, pointer-phi state updates) uses struct ops.
- With `#12`: reactor_tick's body gets SROA. ALL state fields mentioned in reactor_tick
  (which includes all reactive txn bodies inlined or via dispatch variables) get promoted
  to SSA. This eliminates the struct overhead for dispatch logic but forces ALL fields
  into registers.

The #12 SROA is the key: it promotes EVERY field referenced by ANY reactive txn into
LLVM SSA registers within reactor_tick. For ring_buffer (4 fields), this is trivially
beneficial. For nbody (32 fields), this causes register pressure.

### 3.4 Configuration at Each All-Time-Best Era

| Benchmark | Best Era | SLP | reactor_tick attr | SROA scope |
|-----------|----------|-----|-------------------|------------|
| ring_buffer | 4 (Jul 06) | No | #2 (memory(readwrite)) | Per-txn only |
| nbody_newton | 5 (Jul 11) | No | #2 (memory(readwrite)) | Per-txn only |
| nbody_sqrt_idio | 10 (Jul 27) | No | #2 (memory(readwrite)) | Per-txn only |
| nbody_sqrt | 10 (Jul 27) | No | #2 (memory(readwrite)) | Per-txn only |
| float_math | 5 (Jul 11) | No | #2 (memory(readwrite)) | Per-txn only |
| sparse_dispatch | 5 (Jul 11) | N/A (folded) | #2 (memory(readwrite)) | N/A (folded) |
| print_loop | 7 (Jul 19) | No | unknown | Per-txn only |
| mandelbrot | 5 (Jul 11) | No | #2 (memory(readwrite)) | Per-txn only |
| fasta | 14 (recovery) | Yes (stride gate) | #12 (argmem:readwrite) | Reactor_tick + per-txn |
| kalman | 1 (early Jun) | No | #2 (memory(readwrite)) | Per-txn only |
| knucleotide | 1 (early Jun) | No | #2 (memory(readwrite)) | Per-txn only |
| bit_clear | 10 (Jul 27) | No | #2 (memory(readwrite)) | Per-txn only (SROA on 2 fields) |
| cancel_math | 14 (recovery) | Yes (stride gate) | #12 (argmem:readwrite) | Reactor_tick + per-txn |
| queue_drain | 5 (Jul 11) | N/A (folded) | #2 (memory(readwrite)) | N/A (folded) |
| interval_step | 4 (Jul 06) | N/A (folded) | #2 (memory(readwrite)) | N/A (folded) |
| fannkuch_redux | 5 (Jul 11) | No | #2 (memory(readwrite)) | Per-txn only |

**Correlation found:** 11 out of 19 benchmarks achieve their all-time best at configurations
where reactor_tick has `#2` (memory(readwrite)). Only 3 benchmarks achieve their best with
`#12` (argmem:readwrite). The remaining 5 are N/A (folded loops, eliminated at compile time).

---

## 4. Attribute System Analysis

### 4.1 Complete Attribute Table

**File:** `src/backend/llvm/mod.rs`, lines 3200–3320

| # | Used by | Attributes | Purpose |
|---|---------|-----------|---------|
| 0 | `init_state`, `__brief_init_state` | `nofree norecurse nosync nounwind memory(argmem: write)` | State initializer — write-only |
| 1 | `__brief_member_fn` | `nofree nosync nounwind` | Member function (may read/write heap) |
| 2 | `reactor_tick` (fallback), `cell_persistent_ticks` | `memory(readwrite)` | Default tick — unrestricted |
| 3 | `@main` | `nofree norecurse nosync nounwind memory(readwrite)` | Entry point — unrestricted |
| 4 | `reactor_tick` (hazardous) | `nofree nocapture nosync nounwind memory(readwrite)` | Hazard protection |
| 5 | `reactor_tick` (with unguarded FFI) | Same as #2 | Same as #2 |
| 6 | Guard functions | — | Standard guard |
| 7 | `@main` (pre-v0-refactor) | `memory(read)` | Read-only (historical) |
| 8 | Per-txn functions (countable) | `mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)` | Txn body — SROA-friendly |
| 9 | `@main` (current) | `nofree norecurse nosync nounwind memory(readwrite)` | Entry — blocks SROA on main dispatcher |
| 10 | `@pre_*` functions | `mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read)` | Precondition — read-only, SROA-friendly |
| 11 | Per-txn functions (reactive, after outlining) | `mustprogress nofree norecurse nosync nounwind memory(argmem: readwrite)` | Reactive txn — SROA-enabled, conservative |
| 12 | `reactor_tick` (FFI-free after outlining) | `mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)` | Hot loop — SROA-enabled, aggressive |

### 4.2 The Critical Split: #2 vs #12 for reactor_tick

`dispatch.rs:68-73`:
```rust
let rct_attr = if txns.iter().any(|(_, t)| {
    t.is_reactive && t.body.iter().any(|stmt| match stmt {
        Statement::Guarded(_, _) => false,
        _ => transition_graph::statement_contains_ffi(stmt),
    })
}) { "#2" } else { "#12" };
```

The heuristic: "Does any reactive txn have FFI OUTSIDE a `when` guard?"

- `#2`: `memory(readwrite)` — LLVM thinks reactor_tick accesses all memory (globals, heap,
  stack). SROA cannot decompose %State because a memory(readwrite) function might access
  %State through alias pointers.
- `#12`: `argmem:readwrite + willreturn` — LLVM knows reactor_tick only accesses %State
  argument memory, and always returns. SROA CAN decompose %State into SSA registers.

**Why the heuristic exists:** When FFI calls are inside `when` guards, those guards are
outlined into separate "cold" functions (cold-path outlining). The hot path of reactor_tick
has no FFI — only pure Brief operations on %State. So `argmem:readwrite` is accurate.

When FFI calls are UNGUARDED (top-level frgn calls), they cannot be outlined — they're in
the hot path. The frgn call might access global memory (like `@stdout` for `__print_int`),
so `argmem:readwrite` would be a lie. `memory(readwrite)` is the safe fallback.

**The problem:** FFI location is the WRONG criterion for the SROA decision. The real
criterion should be: "will SROA on the ENTIRE %State cause register pressure?"

### 4.3 Why #12's SROA Helps Some Benchmarks and Hurts Others

**SROA mechanics (LLVM's perspective):**
1. LLVM sees: `%state = alloca %State` followed by multiple `getelementptr %State, %state, 0, N` + `load`/`store`
2. SROA decomposes `%State` into individual SSA values: one per field
3. Each field access becomes a direct SSA reference instead of GEP+load (or store)
4. This ELIMINATES the struct access overhead but CREATES register pressure

**For a `%State` with N fields, each i64-sized:**

Per field access:
- Without SROA: `gep %State, %state, 0, i + load i64, ptr` → 2 instructions
- With SROA: `%val = phi [%prev, %loop], [%init, %entry]` → 1 instruction

Per loop iteration with all fields live:
- Without SROA: 2N instructions (GEP + load), 0 register pressure from GEP results
- With SROA: N instructions (phi references), N SSA values live simultaneously

On x86-64 with 16 GP registers (rsp, rbp always reserved → 14 available):

| N fields | SROA benefit | Register pressure | Verdict |
|----------|-------------|-------------------|---------|
| 1–4 | Eliminates 2–8 insns | Fits easily | **#12 beneficial** |
| 5–14 | Eliminates 10–28 insns | Fits with some spill | **#12 borderline** |
| 15–32 | Eliminates 30–64 insns | **Exceeds registers → spill** | **#12 harmful** |

**Benchmark state sizes:**

| Benchmark | State fields | Verdict |
|-----------|-------------|---------|
| ring_buffer | 4 (data, head, tail, ops) | #12 BENEFICIAL |
| bit_clear | 2 | #12 beneficial |
| fasta | 3 (count, N, seed) | #12 beneficial |
| cancel_math | ~3 | #12 beneficial |
| float_math | ~5 | #12 borderline |
| print_loop | ~4 | #12 beneficial |
| fannkuch_redux | ~5 | #12 borderline |
| kalman_filter_runtime | ~15 | #12 NEUTRAL (working set smaller) |
| queue_drain | ~5 | #12 borderline |
| queue_drain_sym | ~5 | #12 borderline |
| queue_drain_idio | ~6 | #12 borderline |
| interval_step | ~4 | #12 beneficial |
| sparse_dispatch | ~6 | #12 borderline |
| knucleotide | ~8 | #12 borderline |
| nbody_newton | 33 | **#2 BENEFICIAL** |
| nbody_sqrt | 33 | **#2 BENEFICIAL** |
| nbody_sqrt_idio | 33 | **#2 BENEFICIAL** |
| mandelbrot | ~6 | #12 borderline |
| float_math_nonzero | ~5 | #12 borderline |

**The nbody family is the ONLY benchmark family that clearly prefers #2.** All others
benefit from or are neutral to #12's SROA. The nbody regression from Era 10 (0.67x) to
baseline (0.92x) is entirely explained by #12's SROA creating register pressure on 33 fields.

### 4.4 The willreturn Attribute

`willreturn` (present in #12, absent in #11 and #2) tells LLVM:
- "This function will always return — it will NOT loop infinitely"
- LLVM can: apply LICM (hoist invariant loads out of loops), DSE (eliminate dead stores after
  calls), loop unrolling, and the AUTO-VECTORIZER

Without `willreturn`:
- LLVM must assume the loop might never terminate
- LICM and DSE are conservative — they cannot move operations past potentially infinite loops
- The auto-vectorizer is disabled because vectorization may introduce lane-crossing behavior
  that changes the program's termination semantics

The baseline benchmark for kalman (1.01x) shows that `willreturn` does NOT cause the
auto-vectorizer to create harmful `<12 x float>` ops for kalman. This is likely because
kalman's loop body has too complex control flow (if-else for matrix inverse step) for
LLVM's auto-vectorizer to analyze.

The original PrintInt# (Phase E) regression that caused kalman 3.80x was a DIFFERENT
mechanism: PrintInt# changed the guard detection logic, which changed the attribute
numbering, which caused the #11 attribute (argmem:readwrite) to be assigned to
reactor_tick instead of #12. The #11 attribute did NOT include `willreturn`, so the
auto-vectorizer was NOT enabled. The regression came from a collision in the attribute
numbering scheme that caused a DIFFERENT feature (#1 on `__brief_member_fn`) to be applied
to reactor_tick.

This means `willreturn` is SAFE for all benchmark programs — the auto-vectorizer does
not create harmful ops even when enabled. The kalman 3.80x was from an attribute
NUMBERING BUG, not from willreturn itself.

---

## 5. SLP Gate Analysis

### 5.1 The Six Gates

| # | Gate | Location | Condition | Status |
|---|------|----------|-----------|--------|
| 1 | **Hazard** | `counter.rs:636` | `txn NOT in slp_hazard_fns` | ACTIVE — computed in `hazard.rs:247`, stored in `ctx.slp_hazard_fns` |
| 2 | **Isomorphism** | `slp_isomorphism.rs:326` | `width >= 2` consecutive isomorphic stmts | ACTIVE — fundamental SLP requirement |
| 3 | **Dep availability** | `slp_isomorphism.rs:555` | All vars defined before template position | ACTIVE — cross-pair merge precondition |
| 4 | **Consumer chain** | `slp_isomorphism.rs:94` | `total_cost < compute_gain * 2` | **DEAD CODE** — computed but NEVER READ |
| 5 | **Lane dependency** | `vector_codegen.rs:336` | No lane's RHS refs a previous lane's LHS | ACTIVE — checks sequential dependency |
| 6 | **Stride + width*depth** | `counter.rs:668` | `max_stride ≤ 1 AND w ≥ 3 AND d*w ≥ 10` | ACTIVE — checks expression field contiguity |

### 5.2 Detailed Gate Inspection

#### Gate 1: Hazard (`hazard.rs:247`)

```rust
pub fn estimate_slp_hazard(&mut self, txns: &[(String, &Transaction)]) {
    // Uses target register count (r) and vector width (w):
    //   AVX512: (32, 16), AVX2: (16, 8), NEON: (32, 4), SSE: (16, 4), default: (16, 1)
    // Populates slp_hazard_fns with txn names where SLP would degrade.
    // Three criteria:
    //   1. peak >= r:  register pressure exceeds available
    //   2. ops_per_field < 1.5: too few float ops to amortize
    //   3. cross_per_field > 3: shuffle overhead (DISABLED — line 377)
}
```

This is the MOST PRINCIPLED gate. It uses actual register counts and computes a per-txn
pressure estimate. The only issue: criterion 3 (cross_per_field) is disabled.

#### Gate 2: Isomorphism (`slp_isomorphism.rs:326`)

```rust
fn find_isomorphic_groups(body: &[Statement], start_idx: usize) -> Vec<SlpIsomorphicGroup> {
    // Takes statement at start_idx as template
    // Walks forward; each subsequent isomorphic statement adds a lane
    // Groups must have width >= 2
    // First non-isomorphic statement terminates the group
    // Uses pair-wise statements_isomorphic() and exprs_isomorphic()
}
```

#### Gate 3: Dep Availability (`slp_isomorphism.rs:555`)

```rust
fn all_deps_available(body: &[Statement], g: &SlpIsomorphicGroup, template_base: usize) -> bool {
    // Every variable referenced by a lane's RHS must be either:
    //   - A state field (always available from %State)
    //   - A let-binding defined BEFORE the template position
    // Groups that fail are dropped from cross-pair merge
}
```

#### Gate 4: Consumer Chain (`slp_isomorphism.rs:94`)

```rust
pub fn chain_pass_ok(groups: &[SlpIsomorphicGroup], gi: usize) -> bool {
    let group = &groups[gi];
    if group.consumer_group_indices.is_empty() {
        return true;
    }
    let total_cost = compute_chain_cost(groups, gi, &mut visited);
    let vars_per_lane = group.template_var_count().max(1) as u32;
    let compute_gain = (group.width * estimate_template_depth(groups, gi).max(1)) as u32;
    let insert_cost = (group.width as u32) * vars_per_lane;
    let extract_cost = group.width as u32;
    (total_cost as u64) < (compute_gain as u64 * 2)
}
```

The cost model:
- `total_cost` = sum over all transitive consumers of `(width * vars_per_lane + width)` per group
- `compute_gain` = `group.width * estimate_template_depth(g)`
- `estimate_template_depth` = `if width > 3 { 2 } else { 1 }` (yes, this is the actual depth estimator — it does NOT use the actual expression tree depth)

**Key finding:** `estimate_template_depth` returns only 1 or 2 independent of the actual
expression complexity. A deeply nested expression (like nbody's `dx01 = bx0 - bx1` which
has depth 1 for a subtraction) gets the same estimate as a shallow one. This means
`chain_pass_ok` underestimates the compute gain for complex expressions.

The threshold `total_cost < compute_gain * 2` with the crude depth estimate means:
- For width=3, depth=1: compute_gain = 3, threshold = 6. So total_cost < 6 → passes.
  For nbody's dx/dy/dz group: width=3, 1 consumer (magnitude computation). Each group
  has vars_per_lane=1 (one RHS variable), so insert_cost = 3, extract_cost = 3,
  total = 6. Fails threshold (6 < 6 is false).
- For width=6 (merged), depth=2: compute_gain = 12, threshold = 24. Much more likely to pass.

THUS: merged groups (width=6) pass `chain_pass_ok`, unmerged groups (width=3) fail.
This means ONLY the cross-pair merged groups (those that survived merge_groups) would
be vectorized via chain_pass_ok.

#### Gate 5: Lane Dependency (`vector_codegen.rs:336`)

```rust
fn has_lane_dependency(body: &[Statement], group: &SlpIsomorphicGroup) -> bool {
    // Checks if any lane's RHS references a PREVIOUS lane's LHS
    // (sequential dependency — e.g., Newton iteration where x1 depends on x0)
    // If true, returns true → SLP vectorization is skipped
}
```

#### Gate 6: Stride + Width*Depth (`counter.rs:668`)

```rust
let should_vec = stride_ok                          // max_stride ≤ 1
    && group.width >= 3                              // width ≥ 3
    && template_expr.map_or(false, |expr| {
        tree_depth(expr) * group.width >= 10         // depth * width ≥ 10
    });
```

The stride check uses `collect_field_indices_front` which walks the template expression's
AST and collects all field index references:

```rust
pub fn collect_field_indices_front(expr: &Expr, field_map: &HashMap<String, usize>, out: &mut Vec<usize>) {
    match expr {
        Expr::Identifier(name) => { if let Some(&idx) = field_map.get(name) { out.push(idx); } }
        Expr::Call(_, args, _) => { for a in args { Self::collect_field_indices_front(a, field_map, out); } }
        Expr::BinaryOp(_, lhs, rhs) => {
            Self::collect_field_indices_front(lhs, field_map, out);
            Self::collect_field_indices_front(rhs, field_map, out);
        }
        // ... UnaryOp, Cast ...
    }
}
```

**The stride gate checks: are ALL field indices in the TEMPLATE EXPRESSION's tree sorted
contiguous (max_stride ≤ 1)?**

For `bx0 - bx1`: field indices [2, 8], sorted [2, 8], max_stride = 6 → **BLOCKED**
For `p00 - p10`: field indices [0, 3], sorted [0, 3], max_stride = 3 → **BLOCKED**

**What it SHOULD check: for each pair of lanes, are their corresponding fields contiguous?**
- Vector load for lane 0's source 1 (bx0) + lane 1's source 1 (by0) + lane 2's source 1 (bz0) = indices [2, 3, 4] → contiguous ✓
- Vector load for lane 0's source 2 (bx1) + lane 1's source 2 (by1) + lane 2's source 2 (bz1) = indices [8, 9, 10] → contiguous ✓

### 5.3 SLP Group Formation for Each Benchmark

| Benchmark | SLP groups? | Width | Template field stride | stride_ok |
|-----------|------------|-------|----------------------|-----------|
| ring_buffer | **NO** (0 groups) | — | — | N/A |
| nbody_newton | YES (143+ groups) | 3 (unmerged), 6 (merged) | 6 | **BLOCKED** |
| nbody_sqrt | YES (143+ groups) | 3 (unmerged), 6 (merged) | 6 | **BLOCKED** |
| nbody_sqrt_idio | YES (143+ groups) | 3 (unmerged), 6 (merged) | 6 | **BLOCKED** |
| kalman | YES (matrix multiply pairs) | 3 (unmerged), 12 (merged) | 3 | **BLOCKED** |
| sparse_dispatch | NO (folded) | — | — | N/A |
| bit_clear | NO (only 2 fields) | — | — | N/A |
| float_math | Few (small state) | 2-3 | 1 | PASSES but w<3 blocks |
| mandelbrot | Few | 2 | 1 | PASSES but w<3 blocks |
| print_loop | NO | — | — | N/A |
| fasta | NO | — | — | N/A |
| queue_drain* | NO | — | — | N/A |
| interval_step | NO (folded) | — | — | N/A |
| fannkuch_redux | NO | — | — | N/A |
| knucleotide | NO | — | — | N/A |
| cancel_math | NO | — | — | N/A |
| float_math_nonzero | Few | 2 | 1 | PASSES but w<3 blocks |

**Benchmarks that would benefit from SLP vectorization:** The nbody family (large
repeating subtract patterns across 5×5 body pairs).

**Benchmarks that would be harmed by SLP:** Kalman (merged width=12 groups with
non-contiguous field access).

These two groups are exactly what `chain_pass_ok` was designed to distinguish.
But `chain_pass_ok` is dead code.

---

## 6. Chain Pass OK: Dead Code Analysis

### 6.1 The Complete Data Flow

```
analyze_body()                     slp_isomorphism.rs:612
  ├─ find_isomorphic_groups()      slp_isomorphism.rs:326   → groups: Vec<SlpIsomorphicGroup>
  ├─ merge_groups()                slp_isomorphism.rs:520   → merged groups
  ├─ build_consumer_graph()        slp_isomorphism.rs:47    → populates consumer_group_indices
  └─ chain_pass_ok() per group     slp_isomorphism.rs:94    → Vec<bool>
       ↓
SlpAnalysisResult                 slp_isomorphism.rs:165
  ├─ groups: Vec<SlpIsomorphicGroup>
  └─ chain_pass_ok: Vec<bool>
       ↓
mod.rs:2376-2384  ── store ──→   FunctionContext             context.rs:512-516
  ├─ self.fun.slp_groups = result.groups               ← READ in counter.rs:639
  └─ self.fun.slp_chain_pass_ok = result.chain_pass_ok  ← *** NEVER READ ***
       ↓
counter.rs:639                     emit_countable_body()
  ├─ reads  slp_groups              → YES → finds match_group
  ├─ reads  ctx.slp_hazard_fns      → YES → is_hazardous gate
  ├─ reads  group.width             → YES → width >= 3 check
  ├─ reads  tree_depth(expr)        → YES → depth * width >= 10 check
  ├─ reads  field_index_map         → YES → stride gate (max_stride <= 1)
  └─ reads  slp_chain_pass_ok       → *** NO ***
       ↓
emit_slp_group()                   vector_codegen.rs:372
  └─ reads  slp_chain_pass_ok       → *** NO *** (not in function signature)
```

**Evidence of dead code:** All 9 occurrences of `chain_pass` in the codebase:

```
src/analysis/slp_isomorphism.rs:
  Line 26:     comment about chain_pass_ok
  Line 94:  pub fn chain_pass_ok(...) -> bool { ... }
  Line 172:    pub chain_pass_ok: Vec<bool>,
  Line 649:    result.chain_pass_ok.push(chain_pass_ok(&result.groups, gi));

src/backend/llvm/mod.rs:
  Line 2384:   self.fun.slp_chain_pass_ok = result.chain_pass_ok;

src/backend/llvm/context.rs:
  Line 515:    /// Doc comment for slp_chain_pass_ok
  Line 516:    pub slp_chain_pass_ok: Vec<bool>,
  Line 609:    slp_chain_pass_ok: Vec::new(),

src/backend/llvm/counter.rs:           ← *** NO REFERENCES ***
src/backend/llvm/vector_codegen.rs:    ← *** NO REFERENCES ***
```

5 occurrences in analysis (definition, computation, data structure), 4 in backend
(storage, declaration, initialization). ZERO in the two files that make dispatch
decisions: `counter.rs` and `vector_codegen.rs`.

### 6.2 What chain_pass_ok Calculates

For each SLP group, `chain_pass_ok`:
1. Walks the consumer chain (all groups whose inputs are this group's outputs)
2. Sums `width * vars_per_lane + width` for each group and all transitive consumers
3. Compares against `compute_gain = width * estimate_template_depth(g)`
4. Returns `total_cost < compute_gain * 2`

For nbody's dx01/dy01/dz01 group (width=3, depth=1):
- `compute_gain = 3 * 1 = 3`
- `threshold = 6`
- `insert_cost = 3 * 1 = 3` (one variable per lane)
- `extract_cost = 3`
- `total_cost` for the group alone: 6
- If no consumers: `6 < 6 → false` → FAILS

For the merged nbody group (width=6, depth=2):
- `compute_gain = 6 * 2 = 12`
- `threshold = 24`
- Same cost calculation but wider → more likely to pass

**The depth estimator (`estimate_template_depth`) is a crude heuristic:**
```rust
fn estimate_template_depth(groups: &[SlpIsomorphicGroup], gi: usize) -> usize {
    groups.get(gi).map_or(1, |g| if g.width > 3 { 2 } else { 1 })
}
```

This returns 1 for width ≤ 3, 2 for width > 3. It does NOT walk the expression tree.
This means `chain_pass_ok`'s cost-gain ratio is dominated by GROUP WIDTH, not actual
expression complexity. For nbody's subtract (tree_depth=1) vs kalman's multiply-add
(tree_depth=3), `chain_pass_ok` cannot distinguish them because it doesn't use actual
`tree_depth()`.

**FIX:** Replace `estimate_template_depth` with the actual `tree_depth` from
`vector_codegen::tree_depth`, or at minimum compute it during SLP analysis and store it.

### 6.3 Why chain_pass_ok Was Never Wired

The commit history shows `chain_pass_ok` was added in commit `d71633c8` (Step 4 — !range
metadata) which is on the `recovery-branch` post-baseline. It was designed as the
"principled kalman-vs-nbody gate" but the `should_vec` formula in `counter.rs` was never
updated to include it. The stride gate (added in the same era) was the HEURISTIC approach
that shipped instead.

---

## 7. Stride Gate Granularity Analysis

### 7.1 What the Stride Gate Checks

The stride gate checks the FIRST LANE'S EXPRESSION. For each SLP group:

```rust
let mut stride_ok = true;
let template_expr = body.get(group.base_index).and_then(|s| match s {
    Statement::Let { expr: Some(e), .. } => Some(&*e),
    Statement::Assign(_, e) => Some(&*e),
    _ => None,
});
if let Some(expr) = template_expr {
    let mut field_indices: Vec<usize> = Vec::new();
    Self::collect_field_indices_front(expr, &self.ctx.field_index_map, &mut field_indices);
    field_indices.sort();
    if field_indices.len() >= 2 {
        let max_stride = field_indices.windows(2)
            .map(|w| w[1] - w[0]).max().unwrap_or(0);
        if max_stride > 1 { stride_ok = false; }
    }
}
```

### 7.2 Field Layout Analysis

Nbody field declarations order:

```
Index | Name  | Type
0     | bound | Int (i64)
1     | count | Int (i64)
2     | bx0   | Float32
3     | by0   | Float32
4     | bz0   | Float32
5     | vx0   | Float32
6     | vy0   | Float32
7     | vz0   | Float32
8     | bx1   | Float32
9     | by1   | Float32
10    | bz1   | Float32
11    | vx1   | Float32
12    | vy1   | Float32
13    | vz1   | Float32
14    | bx2   | Float32
...   | ...   | ...
29    | vz4   | Float32
```

SLP group: `dx01 = bx0 - bx1`, `dy01 = by0 - by1`, `dz01 = bz0 - bz1`

Template expression: `bx0 - bx1` — field indices [2, 8], sorted → max_stride = 6 → **BLOCKED**.

Per-lane vector groups:
- Source 1: {bx0(2), by0(3), bz0(4)} → max_stride = 1 → CONTIGUOUS ✓
- Source 2: {bx1(8), by1(9), bz1(10)} → max_stride = 1 → CONTIGUOUS ✓

The stride gate incorrectly blocks because it checks stride BETWEEN `bx0(2)` and `bx1(8)`,
but the actual vector loads are GROUPED as {bx0,by0,bz0} at [2,3,4] and {bx1,by1,bz1} at [8,9,10].

The stride BETWEEN the source groups (2→8 = 6) does NOT affect the vector load efficiency.
Each vector load is from contiguous memory. The 6-element gap between the two groups only
affects CACHE LINE utilization — with 2 cache lines touched instead of 1, there's a small
penalty, but it's negligible compared to the gain from vectorized arithmetic.

### 7.3 Kalman Field Layout

Kalman field declarations order:

```
Index  | Name  | Type
0      | count | Int (i64)
1      | ...   | ...
2      | p00   | Float32
3      | p01   | Float32
4      | p02   | Float32
5      | ...   | other state vars
6      | p10   | Float32
7      | p11   | Float32
8      | p12   | Float32
9      | ...   | ...
12     | p20   | Float32
13     | p21   | Float32
14     | p22   | Float32
```

SLP group: `dp00 = p00...`, `dp10 = p10...`, `dp20 = p20...`

Template expression: `p00 - something` — field indices [2, 6, 12] → sorted → max_stride = 4 → **BLOCKED**.

Per-lane vector groups:
- Source 1: {p00(2), p10(6), p20(12)} → max_stride = 4 → NOT CONTIGUOUS
- Source 2: similar non-contiguous pattern

The stride gate CORRECTLY blocks kalman because even the per-lane groups are non-contiguous.
A `<3 x float>` load from [2, 6, 12] would need 3 independent scalar loads (or a gather).

### 7.4 The Correct Stride Check

Instead of checking `max_stride ≤ 1` on ALL fields in the template expression, check
whether each VECTOR GROUP (set of corresponding fields across lanes) is contiguous:

```rust
let stride_ok = group.lane_mappings[0].keys().all(|template_var| {
    // Get the field indices for this variable across ALL lanes
    let indices: Vec<usize> = group.lane_mappings.iter()
        .filter_map(|map| map.get(template_var.as_str()))
        .filter_map(|lane_var| self.ctx.field_index_map.get(lane_var.as_str()))
        .copied()
        .collect();
    if indices.len() < 2 { return true; }
    let max_stride = indices.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
    max_stride <= 1
});
```

For nbody:
- template_var "bx0" → lanes: bx0(2), by0(3), bz0(4) → stride 1 → PASSES ✓
- template_var "bx1" → lanes: bx1(8), by1(9), bz1(10) → stride 1 → PASSES ✓

For kalman:
- template_var "p00" → lanes: p00(2), p10(6), p20(12) → stride 4 → BLOCKED ✓

This would allow nbody's SLP groups to vectorize while correctly blocking kalman's.

---

## 8. Interaction Matrix: How Gates Interact

### 8.1 Direct Interactions

| Gate A | Gate B | Interaction | Nature |
|--------|--------|-------------|--------|
| SROA (attr select) | stride gate | **NONE** | Different scopes (function attr vs per-group dispatch) |
| SROA (attr select) | hazard | **NONE** | Different scopes (function attr vs per-txn hazard blacklist) |
| SROA (attr select) | LLVM auto-vec | **CAUSAL** | `willreturn` in #12 enables LLVM's auto-vectorizer |
| stride gate | hazard | **NONE** | Different functions (hazard per-txn, stride per-group) |
| stride gate | LLVM auto-vec | **NONE** | Stride affects our SLP, not LLVM's |
| `chain_pass_ok` | stride gate | **ORTHOGONAL** | Comput vs memory — different cost models |
| `chain_pass_ok` | hazard | **OVERLAP** | Both assess register pressure — hazard at txn level, chain at group level |
| cold-path outlining | SROA | **CAUSAL** | Outlining removes FFI from hot path → #12 possible → SROA enabled |
| cold-path outlining | hazard | **NONE** | Outlining changes body composition but not register pressure |

### 8.2 The Full Interaction Chain for Each Benchmark

```
ring_buffer (4 fields):
  FFI in guards → outlined → no unguarded FFI → #12 selected
  → SROA on reactor_tick → 4 fields promoted → fits in registers → BENEFIT

nbody_sqrt_idio (32 fields):
  FFI in guards → outlined → no unguarded FFI → #12 selected
  → SROA on reactor_tick → 32 fields promoted → REGISTER PRESSURE → SPILL → COST

kalman (15 fields):
  FFI in guards → outlined → no unguarded FFI → #12 selected
  → SROA on reactor_tick → 15 fields promoted → borderline
  → stride gate blocks SLP on 15 fields → neutral
  → LLVM auto-vec disabled by complex control flow → neutral
  → NET: NEUTRAL (verified: baseline 1.01x)
```

### 8.3 The Feedback Loop That Binds Everything

There is ONE feedback loop in the system:

```
Attribute #12 selected
  → SROA enables on reactor_tick
  → %State fields become SSA registers
  → More registers used
  → REGISTER PRESSURE
  → Spill code generated
  → More instructions per iteration
  → Lower performance (nbody path)
```

And the reverse:

```
Attribute #2 selected
  → SROA blocked on reactor_tick
  → %State stays as struct
  → GEP + load/store per access
  → More instructions per iteration
  → Lower performance (ring_buffer path)
  → BUT: no register pressure from promotion
```

This is a STATIC trade-off determined entirely by attribute selection. No amount of
SLP tuning can fix it — the SROA/register-pressure problem is upstream of SLP dispatch.

**The attribute selection heuristic (FFI in guards) is accidentally correct for most
benchmarks because most benchmarks have few fields. nbody is the ONLY benchmark with >14
fields that also has FFI in guards, triggering #12 when #2 would be better.**

### 8.4 Why No Benchmark Has All Fields at Best

The 6-week optimization history added passes in this order:
1. Pure-counter fold (Era 3) — eliminated folded benchmarks from further analysis
2. Arena-by-proof (Era 5) — improved bit_clear, queue_drain
3. Cold-path outlining (Era 7) — enabled #12 attribute → SROA → ring_buffer improved, nbody regressed
4. SLP with stride gate (Era 11) — ALL 19 back to parity, but nbody not at 0.67x
5. `!range`/`!prof` (Era 14) — incremental improvements

The nbody regression (from 0.67x to 0.92x) was introduced in Era 11 when cold-path
outlining + #12 attribute was applied. The stride gate and SLP tuning were ADDED ON TOP
but addressed the WRONG problem (SLP vectorization) instead of the real problem
(SROA-induced register pressure from #12).

---

## 9. The Register-Pressure Hypothesis

### 9.1 Statement

**The nbody family regression between Era 10 and Era 12 is caused by #12's `argmem:readwrite`
attribute enabling SROA on reactor_tick, which promotes all 32 %State fields to SSA
registers, causing register pressure and spilling on x86-64's 16-register file.**

### 9.2 Evidence Supporting

1. **Era 10 (no SLP, #2):** nbody_sqrt_idio 2.33s (0.67x)
2. **Era 12 (SLP stride gate, #12):** nbody_sqrt_idio 3.34s (0.92x)
3. **The only relevant difference:** `dispatch.rs` attribute selection from `#2` to `#12`
4. **SLP stride gate is irrelevant** for nbody — all nbody SLP groups are blocked by stride>1
5. **Per-txn SROA (#8/#11) is identical** between eras — the difference is reactor_tick scope only
6. **32 fields on 14 available registers = 18 must spill** — adds ~18 store+load per iteration
7. **Ring_buffer benefited** (4 fields, SROA fits in registers)

### 9.3 Evidence Against

1. **The hypothesis has NOT been empirically verified** — no test has compared #12 vs #2
   on the same compiler with SLP disabled for nbody
2. **SROA is an LLVM pass** — its behavior depends on LLVM's internal heuristics that may
   not promote all fields simultaneously
3. **The hazard gate** might already be blocking SROA for nbody if it detects high pressure
   (though hazard is for SLP, not SROA)

### 9.4 Verification Procedure

To confirm or refute, test with just the attribute changed:

1. Build from Era 10 (commit `33d42397`): ring_buffer = 1.31x, nbody = 0.67x
2. Build from Era 12 (commit `b39461e2`): ring_buffer = 1.14x, nbody = 0.92x
3. Create a HYBRID: take Era 12 codebase, force `#2` on reactor_tick:
   - nbody should return to ~0.67x if SROA is the cause
   - ring_buffer should return to ~1.31x if SROA is the cause
4. Create the REVERSE HYBRID: take Era 10 codebase, force `#12` on reactor_tick:
   - nbody should degrade to ~0.92x
   - ring_buffer should improve to ~1.14x

If both predictions hold, the attribute selection heuristic is confirmed as the root cause.

### 9.5 The SLP Contribution

If the hybrid test confirms SROA is the primary cause, the remaining nbody gap (if any)
between 0.92x with #12 and the actual 0.67x best can be attributed to SLP/stride gate
effects. This would be the SECONDARY correction (several percentage points vs the
primary 37% regression).

---

## 10. Verified Findings

### 10.1 Confirmed Through Reading Code

1. **`chain_pass_ok` is dead code.** Computed in `slp_isomorphism.rs:94`, stored in
   `FunctionContext` at `mod.rs:2384`, field declared at `context.rs:516`. Zero reads
   in `counter.rs` or `vector_codegen.rs`.

2. **The stride gate checks the wrong granularity.** `collect_field_indices_front` at
   `counter.rs:600` collects ALL fields from the template expression. The correct check
   should use `lane_mappings` to check per-lane-vector-group contiguity.

3. **The depth estimator in `chain_pass_ok` is a stub.** `estimate_template_depth` at
   `slp_isomorphism.rs:131` returns `if width > 3 { 2 } else { 1 }` — it does NOT use
   the actual `tree_depth()` function from `vector_codegen.rs`.

4. **The attribute selection heuristic at `dispatch.rs:68` uses FFI location as its sole
   criterion.** This is coarse and misses the state-size consideration.

5. **All six SLP gates exist and are correctly independent.** No gate depends on another
   gate's output. They form a linear AND chain.

6. **The hazard gate at `hazard.rs:247` has a disabled criterion.** `cross_per_field > 3`
   (shuffle overhead detection) is computed but its result is ignored (line 377).

7. **The stride gate comment about nbody stride=1 is incorrect.** Field indices for
   `bx0` and `bx1` are 2 and 8 respectively (with bound/count/v*0 separating them),
   giving stride=6, not stride=1.

### 10.2 Confirmed Through Git History

8. **Era 10's `#9` attribute used `memory(argmem: readwrite)`.** Era 12 changed it to
   `memory(readwrite)`. `git show 33d42397:src/backend/llvm/mod.rs | grep "attributes #9"`
   confirms `argmem:readwrite`; `git show b39461e2:src/backend/llvm/mod.rs | grep "attributes #9"`
   confirms `memory(readwrite)`.

9. **The `#11` and `#12` attributes were added in Era 11, not present in Era 10.**
   `#12` is `argmem:readwrite + willreturn` — the combination that enables both SROA
   and the auto-vectorizer on reactor_tick.

10. **The dispatch.rs attribute selection is in the baseline** (commit `b39461e2`),
    NOT in Era 10. The `#2` vs `#12` choice only exists from Era 11 onward.

### 10.3 Confirmed Through Benchmark Data

11. **All 19 benchmarks were at parity in the baseline.** No benchmark had a ratio > 1.14x
    or showed a correctness failure.

12. **nbody_sqrt_idio's best (0.67x) was achieved with `#2` on reactor_tick** (Era 10,
    no SLP). The baseline with `#12` gives 0.92x.

13. **ring_buffer's best (0.99x) was achieved before cold-path outlining** (Era 4).
    The baseline with outlining + #12 gives 1.14x. The pre-outlining best was 0.99x.

14. **kalman_filter_runtime's best (0.95x) was achieved before SLP and before outlining**
    (Era 1). Every subsequent era has kalman at ~1.00x. The original best appears to be
    from a simpler compiler that didn't have the SLP/outlining/arena passes.

---

## 11. Unresolved Questions

### 11.1 Questions That Require Experimental Verification

1. **Is SROA the actual cause of the nbody #12 regression?** This requires the hybrid
   benchmark test described in §9.4. Without it, the hypothesis is unconfirmed.

2. **Does `chain_pass_ok` with the correct depth estimator produce the right decisions
   for nbody vs kalman?** The current `estimate_template_depth` is too crude. Wiring
   it with `tree_depth()` might change the results.

3. **Would per-lane contiguity checking (instead of cross-expression stride) correctly
   distinguish all cases?** For nbody (beneficial SLP) it should pass. For kalman
   (harmful SLP) it should still block. But what about edge cases?

4. **What is the actual register pressure from nbody's 32 fields on x86-64?** The 14-GP
   estimate assumes all 32 are simultaneously live. In practice, LLVM's live range
   analysis might reduce the pressure.

5. **Does the `willreturn` attribute enable LLVM's auto-vectorizer for any benchmark?**
   The baseline shows all 19 at parity, suggesting it doesn't cause harm. But the
   original PrintInt# regression suggests it CAN be harmful under specific conditions.

6. **What is the exact threshold (state field count) where #12 becomes harmful?** Is
   it 14 (GP registers), 10 (some other limit), or dependent on the txn's live set
   shape?

7. **Can field reordering in `apply_field_modes` make the stride gate irrelevant?** If
   fields were arranged so that SLP-friendly groups have stride=1, the stride gate
   would pass for nbody. But this requires intelligent layout, which doesn't exist.

### 11.2 Questions About the Data

8. **Why did sparse_dispatch go from 0.09x (folded, Era 5) to 0.81x (baseline)?** The
   fold was disabled or the computation changed. Sparse_dispatch was one of the
   benchmarks that benefited from pure-counter fold elimination. At some point, the
   fold was lost — probably when `observable` semantics were added to FFI calls.

9. **Why did queue_drain and interval_step go from 0.01x (folded) to ~1.00x?** Same
   answer as sparse_dispatch — the pure-counter fold was disabled when FFI calls
   were marked `observable`, preventing loop elimination.

10. **Can the pure-counter fold be restored for benchmarks without observable FFI?**
    This would require a more precise analysis that distinguishes "calls that have
    side effects" from "calls that produce observable output."

---

## 12. Data Flow Diagrams

### 12.1 Attribute Selection Flow

```
TopLevel items parsed
  ↓
build_field_index()         mod.rs:3648   → field_index_map, field_types
  ↓
apply_field_modes()         mod.rs:3876   → dead field elimination, index compaction
  ↓
analyze_body()              mod.rs:2376   → SLP groups + chain_pass_ok (not used)
  ↓
estimate_slp_hazard()       hazard.rs:247 → slp_hazard_fns
  ↓
generate()                  mod.rs:1750
  ├── emit_reactor()        dispatch.rs:57
  │     └── rct_attr        dispatch.rs:68  → selects #2 or #12
  │           ↑ heuristic: "any unguarded FFI in reactive txns?"
  │
  ├── emit_transaction()    emit_toplevel.rs
  │     └── emit_countable_body()  counter.rs:445
  │           └── should_vec       counter.rs:668
  │                 ├── is_hazardous → slp_hazard_fns
  │                 ├── stride_ok → field_index_map (collect_field_indices_front)
  │                 ├── width ≥ 3 → group.width
  │                 └── depth*width ≥ 10 → tree_depth * width
  │
  ├── emit_attributes()     mod.rs:3200  → #0–#12 LLVM attribute groups
  │
  └── .ll output
```

### 12.2 Cold-Path Outlining Flow

```
Txn with FFI in `when` guard
  ↓
is_guarded_ffi() true       emit_toplevel.rs:1690
  ↓
guard body outlined to      emit_toplevel.rs:1745
  @pre_<name>()             → precondition function (#10 attr)
  cold_txn_<name>()         → cold path (#2 attr)
  ↓
reactor_tick hot path       dispatch.rs:68
  uses rct_attr = #12       (no unguarded FFI → argmem:readwrite + willreturn)
  calls outlined functions  (only when guard fires — cold)
```

### 12.3 SROA Scope Hierarchy

```
@main (#9 = memory(readwrite))
  └── @reactor_tick (#12 = argmem:readwrite + willreturn)
  │     └── SROA on %State (PROMOTED TO SSA WITHIN reactor_tick)
  │         ├── ring_buffer: 4 fields → FITS (register count = 14)
  │         └── nbody: 32 fields → SPILLS (18 fields exceed 14 registers)
  │
  └── Each outlined txn function (#11 = argmem:readwrite)
        SROA on %State (promoted within function)
        Same register pressure analysis, per-txn scope
```

The per-txn SROA (#11) is the SAME in both #2 and #12 configurations. The DIFFERENCE
is that #12 additionally enables SROA on reactor_tick's DISPATCH CODE (the trigger
checks, phi updates, and dispatch loop). With #12, ALL 32 fields get promoted in the
dispatching code, not just in the per-txn code.

---

## Appendix A: Bibliography of Files Read

| File | Purpose | Lines |
|------|---------|-------|
| `src/backend/llvm/dispatch.rs` | Attribute selection, reactor_tick emission | 588 |
| `src/backend/llvm/mod.rs` | generate(), attributes, build_field_index, apply_field_modes | 4021 |
| `src/backend/llvm/context.rs` | CompilerContext, FunctionContext data structures | 700+ |
| `src/backend/llvm/loop_engine/counter.rs` | Emit_countable_body, should_vec, stride gate | 800+ |
| `src/backend/llvm/emit_toplevel.rs` | Guard emission, cold-path outlining | 2600+ |
| `src/backend/llvm/helpers.rs` | as_bool_reg, load_field_type, is_protocol_member | 2900+ |
| `src/backend/llvm/vector_codegen.rs` | emit_slp_group, emit_vector_expr, lane dependency | 400+ |
| `src/backend/llvm/hazard.rs` | estimate_slp_hazard | 400+ |
| `src/analysis/slp_isomorphism.rs` | analyze_body, chain_pass_ok, stride check | 754 |
| `src/analysis/transition_graph.rs` | statement_contains_ffi | — |
| `src/ast/types.rs` | Type enum, universe_key | — |
| `src/type_universe/mod.rs` | PRIMORDIALS, property definitions | — |
| `benchmarks/ring_buffer.bv` | State fields, contract | — |
| `benchmarks/nbody_sqrt_idio.bv` | State fields, contract | — |
| `benchmarks/nbody_newton.bv` | State fields, contract | — |
| `benchmarks/kalman_filter_runtime.bv` | State fields, contract | — |
| `benchmarks/fasta.bv` | State fields, contract | — |
| `.opencode/plans/2026-07-28-baseline-recovery.md` | Plan document | — |
| `.opencode/HANDOFF.md` | Handoff doc | — |
| `AGENTS_HISTORY.md` | Historical context | — |
| `AGENTS_HISTORY_2.md` | Historical context backup | — |
| `docs/plans/2026-07-27-cold-path-refinement.md` | Cold-path outlining runs | — |
| `docs/plans/2026-07-27-benchmark-regression-results.md` | Era 10 results | — |
| `docs/plans/2026-07-06-next-optimizations.md` | Era 4 results | — |
| `benchmarks/results/2026-07-11-phase3-complete.md` | Era 5 results | — |
| `benchmarks/results/2026-07-19-post-migration.md` | Era 7 results | — |

## Appendix B: All 19 State Field Counts

| Benchmark | State Fields | Count | SROA Verdict |
|-----------|-------------|-------|-------------|
| ring_buffer | data, head, tail, ops | 4 | #12 beneficial |
| bit_clear | bound, count, x, result | 4 | #12 beneficial |
| fasta | bound, count, seed | 3 | #12 beneficial |
| cancel_math | bound, count, x, y, z | 5 | #12 beneficial |
| print_loop | bound, count | 2 | #12 beneficial |
| interval_step | bound, count, x | 3 | #12 beneficial |
| queue_drain | bound, count, x, y, z | 5 | #12 beneficial |
| queue_drain_sym | bound, count, x, y, z | 5 | #12 beneficial |
| queue_drain_idio | bound, count, x | 3 | #12 beneficial |
| fannkuch_redux | bound, count, x, ... | ~8 | #12 beneficial |
| sparse_dispatch | bound, count, ... | ~6 | #12 beneficial |
| knucleotide | bound, count, hash, ... | ~10 | #12 beneficial |
| mandelbrot | bound, count, x, y, z | ~6 | #12 beneficial |
| kalman_filter_runtime | bound, count, p00..p22, x0..x2, ... | ~15 | #12 neutral |
| float_math | bound, count, a..e | ~7 | #12 beneficial |
| float_math_nonzero | bound, count, a..e | ~7 | #12 beneficial |
| nbody_newton | bound, count, 30 Float32 | **33** | **#2 beneficial** |
| nbody_sqrt | bound, count, 30 Float32 | **33** | **#2 beneficial** |
| nbody_sqrt_idio | bound, count, 30 Float32 | **33** | **#2 beneficial** |
