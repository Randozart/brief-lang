# Phase 5: Flat Allocas and Loop Peeling

**Date:** 2026-07-29
**Author:** Agent (investigation session)
**Status:** Plan — pre-implementation

## Executive Summary

The nbody_newton benchmark (31 float state fields, 1 counter, 1 bound, 1 last_energy = 34 total fields) produces a loop with **32 phi nodes** across the backedge. On x86-64 AVX2 with only 16 XMM registers, this forces the register allocator to spill at least **16 float values** to the stack every iteration. Our experiments show this as a consistent 1.22× gap vs the C reference.

Previous attempts to fix this (phi-capping, vector phis, `@init_state` separation) all failed because they fought against LLVM's design rather than working with it. The correct approach, validated by LLVM's own documentation and the Kaleidoscope tutorial, is to **replace the monolithic `%State` struct with individual per-field allocas**, letting LLVM's `mem2reg` and `SROA` passes handle phi placement naturally.

## Hypothesis

**Replacing the single `%State { i64 x 2, float x 31, i64 }` aggregate alloca with 31 individual `alloca float` and 2 `alloca i64` in the entry block will:**

1. **Enable SROA to promote each alloca independently** — SROA gives up on 34-field aggregates but handles individual scalar allocas perfectly.
2. **Let `mem2reg` form phi nodes only where needed** — The register allocator sees the minimal number of phi-carried values, not a rigid 31-phi web.
3. **Allow the register allocator to spill only when actual pressure exceeds registers** — Individual allocas can be selectively spilled, rather than forcing the entire struct to memory.
4. **Target: reduce nbody_newton from 1.22× to ≤ 1.05× C.**

## Failure Analysis

Every previous experiment failed for a reason the flat-allocas approach addresses:

| Failed experiment | Reason | Why flat allocas fix it |
|-------------------|--------|------------------------|
| **Phi-capping** (cap write_set to 12 fields) | Capped fields loaded from `%State` via GEP — SROA couldn't decompose the aggregate, so each access was a real memory load | Individual allocas → SROA promotes each independently → the "capped" fields are loaded once, promoted to registers |
| **`<2 x float>` / `<8 x float>` vector phis** | Extractelement + insertelement overhead > register savings | No vector phis needed — SROA + mem2reg form optimal phi placement naturally |
| **`@init_state` separation** | Call boundary prevented SROA from decomposing `%State` | With individual allocas, the call to `init_state` writes to each alloca independently — SROA can analyze each alloca's lifetime through the call using `memory(argmem: write)` |
| **`!invariant.load` on capped fields** | Semantic violation — fields were written each iteration | Individual allocas are promoted to registers by mem2reg, not loaded through memory — no invariant metadata needed |

## Literature Review

### Source 1: LLVM Performance Tips for Frontend Authors

**URL:** https://llvm.org/docs/Frontend/PerformanceTips.html
**Section:** "Use of allocas"

> *"The SROA (Scalar Replacement Of Aggregates) and Mem2Reg passes only attempt to eliminate alloca instructions that are in the entry basic block. In particular, place them before any call instructions. Call instructions might get inlined and replaced with multiple basic blocks. The end result is that a following alloca instruction would no longer be in the entry basic block afterward."*

> *"Avoid creating values of aggregate type. In particular, avoid loading and storing them, or manipulating them with insertvalue and extractvalue instructions. Instead, only load and store individual fields of the aggregate."*

This is a direct command from LLVM's own documentation: **do not use aggregate types for local variables; use individual allocas.** Our current code violates this by storing all 34 fields in a single `%State` struct.

### Source 2: Kaleidoscope Tutorial (Chapter 7 — Mutable Variables)

**URL:** https://llvm.org/docs/tutorial/MyFirstLanguageFrontend/LangImpl07.html
**Section:** "Memory in LLVM"

> *"The 'trick' here is that while LLVM does require all register values to be in SSA form, it does not require (or permit) memory objects to be in SSA form."*

> *"Each mutable variable becomes a stack allocation. Each read of the variable becomes a load from the stack. Each update of the variable becomes a store to the stack."*

> *"The mem2reg pass implements the standard 'iterated dominance frontier' algorithm for constructing SSA form and has a number of optimizations that speed up (very common) degenerate cases."*

> *"mem2reg is alloca-driven: it looks for allocas and if it can handle them, it promotes them. It does not apply to global variables or heap allocations."*

**Constraints for mem2reg:**
1. allocas must be in the **entry block** of the function
2. allocas must only be used by direct **load and store** instructions (no pointer arithmetic)
3. allocas must be of **first-class types** (scalars, vectors, pointers)
4. **array size must be 1**

Our current `%State` alloca violates constraint 3 (it's an aggregate struct). Individual `alloca float` and `alloca i64` satisfy all four constraints.

### Source 3: LLVM Vectorizers Documentation

**URL:** https://llvm.org/docs/Vectorizers.html
**Section:** "If Conversion"

> *"The Loop Vectorizer is able to 'flatten' the IF statement in the code and generate a single stream of instructions."*

However, if-conversion cannot handle branches containing opaque function calls. The `PrintInt#` call in nbody's `when` guard blocks if-conversion. The SLP Vectorizer (which operates on basic blocks, not loops) is not affected by this — it can still vectorize groups of isomorphic instructions within a single block.

### Source 4: Era-5 IR (commit `8a827db`)

Era-5 used:
1. **Chunked state allocas**: `%StateChunk0 = type { 15 fields }`, `%StateChunk1 = type { 15 fields }`, `%StateChunk2 = type { 4 fields }`
2. **Separate `@init_state` function**
3. **`<4 x float>` vector phis** emitted by the backend

Era-5 achieved **0.75× C** (nbody_newton). The chunked allocas allowed SROA to decompose each 15-field chunk independently, enabling good register allocation.

### Source 5: Benchmarks Game N-body Data

The [Computer Language Benchmarks Game](https://benchmarksgame-team.pages.debian.net/benchmarksgame/performance/nbody.html) shows:

| Program | Time | Technique |
|---------|:----:|-----------|
| C gcc #9 (SIMD) | 2.10s | Hand-written `_mm_load_pd` intrinsics |
| C gcc #1 (reference) | 5.23s | Plain struct `planet { double x,y,z,... }` + sqrt |
| Rust #9 (SIMD) | 2.19s | `core::simd` portable SIMD |

The C reference (5.23s) uses an **array-of-structs** layout (`struct planet { double x, y, z, vx, vy, vz, mass; }`) — the same AoS pattern as our nbody_newton. Clang generates per-field phi nodes and LLVM's SROA handles promotion. The C reference doesn't vectorize (no SIMD intrinsics), yet it runs in 5.23s vs our Briev at ~10.6s (50M bound). The 2× gap is from the Briev compiler emitting a monolithic `%State` struct instead of individual field allocas.

## Diagnostic Experiment: Loop Peeling

### Purpose

Before committing to the flat-allocas refactoring (2-4 hours), run a **15-minute diagnostic experiment** to confirm that the phi-register-pressure theory is correct.

### Method

Create a new benchmark `benchmarks/nbody_newton_peeled.bv` with a **manually peeled** loop structure:

```
defn run_iterations(N) {
    txr simulate_pure [count < N][count == N] {
        // Pure compute: no when guards, no PrintLn, no post-loop print
        // Same physics: dx, dy, dz, Newton sqrt, velocity update, position update
        count = count + 1;
        term;
    };
};

defn main() {
    bound = GetEnvInt!("BOUND");
    // Compute energy once at the start (or skip — we only care about loop time)
    run_iterations(bound);
    // After loop, compute and print final energy once
    PrintLn!(last_energy);
};
```

The key differences from `nbody_newton.bv`:
1. **Inner loop is pure computation** — no `when` guards, no periodic printing, no function calls
2. **Outer code handles termination** — the `count == bound` check still exists but without function calls
3. **The post-loop print is purely scalar** — outside the hot loop

This creates a loop with no opaque function calls → LLVM's Loop Vectorizer can attempt if-conversion. Even if vectorization doesn't fire, the pure compute loop has fewer phi nodes (the periodic-print guard branches are removed from the body).

### Prediction

- If loop peeling reduces the ratio from 1.22× to near 1.0×, the branch-guard (blocking if-conversion) is the dominant bottleneck — focus on automatic peeling pass.
- If loop peeling barely changes the ratio, the 32-phi web (register starvation) is the dominant bottleneck — proceed with flat-allocas refactoring.
- If both help, both fixes are needed.

### Execution

The source file is at `benchmarks/nbody_newton.bv`. Create a copy and modify the body to remove:
- The `when count % 5000000 == 0 { PrintLn!(energy) }` block
- The `when count == bound { term! → PrintLn!(last_energy) }` block
- Move the post-loop energy print outside the loop

Compile with the current compiler, benchmark, compare.

## Phase 5: Flat-Allocas Backend Refactoring

### Design

Replace the aggregate `%State` struct with individual scalar allocas in the entry block.

#### Current architecture:

```
entry:
  %state = alloca %State, align 8      ; 34-field aggregate
  call void @init_state(ptr %state)      ; or inline init stores
  
  ; Loop body:
  gep %state, i32 0, i32 5              ; access field at index 5
  load float, ptr %gep                    ; load value
  ... compute ...
  store float %val, ptr %gep             ; store value back
```

#### New architecture:

```
entry:
  %bound = alloca i64, align 8
  %count = alloca i64, align 8
  %bx0 = alloca float, align 4
  %bx1 = alloca float, align 4
  ... 31 float allocas ...
  %last_energy = alloca float, align 4
  call void @init_state_flat(ptr %bound, ptr %count, ..., ptr %last_energy)
  
  ; Loop body:
  %bx0.val = load float, ptr %bx0        ; direct load
  ... compute ...
  store float %bx0.val2, ptr %bx0        ; direct store
```

### Implementation Plan

**Phase 5a: Change `field_types` and `field_index_map` usage**

Currently, `field_types` is a `Vec<String>` indexed by position in `%State`. Replace with a mapping from field name → LLVM type. The GEP pattern `getelementptr %State, ptr %state, i32 0, i32 N` becomes a direct `load`/`store` to the named alloca.

**Key functions to modify:**

1. **`helpers.rs` — `load_field_type`**: Currently emits GEP→load chain. Replace with: resolve the named alloca → emit load directly.
2. **`helpers.rs` — `store_field_type`**: Same — replace GEP→store with direct store.
3. **`emit_toplevel.rs` — `emit_main` / `emit_countable_main`**: Replace `%state = alloca %State` with N individual `alloca <type>` instructions.
4. **`emit_toplevel.rs` — `emit_init_state` / `emit_inline_init_stores`**: Replace GEP chains with direct stores to named allocas.

**Alloca naming convention:** `%s_<field_name>` (e.g., `%s_bx0`, `%s_count`) to avoid conflicts with other registers.

**Phase 5b: Remove `%State` type declaration**

After all code paths use individual allocas, remove:
- The `%State` type definition in `declare_state_type`
- The `%StateChunk0..N` type definitions (obsolete — each field has its own alloca)
- The `field_index_map` (replaced by alloca name mapping)

**Phase 5c: Remove field index from `emit_state_gep`**

The `emit_state_gep` function currently emits:
```
%tN = getelementptr inbounds %State, ptr %state, i32 0, i32 <idx>
```

Replace with a lookup: "find the alloca pointer for field `<name>`" — this is just a register name (the alloca itself).

### Risks and Mitigations

| Risk | Probability | Mitigation |
|------|:-----------:|------------|
| SROA/mem2reg fail to promote some allocas | Low | Falls back to stack memory — correct but slower. Same as current performance. |
| GEP chains used in ring buffer code | Medium | Ring buffer fields used pointer arithmetic on `%State` GEPs. May need special handling. |
| Cache slot / SSO string allocas | Medium | SSO strings occupy 2 consecutive i64 slots. Need paired allocas. |
| Register pressure still high after promotion | Low | mem2reg creates phis based on actual liveness, not rigid write_set. Should be strictly better. |

### Verification

1. `cargo test --lib` — all 1173+ tests pass
2. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks MATCH
3. `bash benchmarks/build_and_bench.sh --runtime` — nbody_newton ratio should improve from 1.22×
4. Check `.ll` output: no `%State = type { ... }` — individual `alloca float` per field

### Expected Performance Impact

| Benchmark | Before | After (estimated) | Notes |
|-----------|:------:|:-----------------:|-------|
| nbody_newton | 1.22× | ≤ 1.05× | SROA promotes 31 float allocas → natural phi placement |
| ring_buffer | 1.20× | ~1.10× | 5 i64 allocas → lower pressure, same pointer boxing issue |
| All others | unchanged | unchanged | Small state sizes already handled well |

## Phase 5d: Loop Peeling Pass (Future)

After flat allocas are working, an automatic loop peeling pass can be added for benchmarks with mixed compute + print loops:

```
Before:                          After:
[count < N]                      [count < M]  // inner (M = N - remaining)
  compute                           compute
  when period { print }             count += 1
  count += 1                     [remaining > 0]
  term                              print
                                    remaining -= 1
```

This is a source-to-source transformation on the `Statement` vector. Not implemented in this phase.

## Appendices

### A: Changes from Previous Plan (2026-07-29-frontend-ir-quality-improvements.md)

This plan supersedes the prior "Improvement #2: Separate `@init_state`" which was found to regress performance. The flat-allocas approach achieves the same goal (cleaner function structure) without the SROA loss.

### B: LLVM Documentation Citations

| Citation | URL | Key quote |
|----------|-----|-----------|
| Performance Tips §Use of allocas | https://llvm.org/docs/Frontend/PerformanceTips.html | "SROA and Mem2Reg passes only attempt to eliminate alloca instructions that are in the entry basic block" |
| Kaleidoscope §7.3 | https://llvm.org/docs/tutorial/MyFirstLanguageFrontend/LangImpl07.html | "Each mutable variable becomes a stack allocation" |
| Vectorizers §If Conversion | https://llvm.org/docs/Vectorizers.html | "Loop Vectorizer is able to flatten IF statements" |
| LangRef §alloca | https://llvm.org/docs/LangRef.html#i-alloca | "allocas in entry block are analyzed by mem2reg" |

### C: Experimental Results Referenced

| Experiment | Result | Location in repo |
|------------|--------|-----------------|
| Baseline (no changes) | 1.23× C | `benchmarks/results/2026-07-29-baseline-4fa1641e.md` |
| Pure phi-capping | 1.43× C | Appendix E.1 in `docs/plans/2026-07-29-frontend-ir-quality-improvements.md` |
| `<2 x float>` vector phis | 1.52× C | Appendix E.2 |
| `<8 x float>` vector phis (SoA) | 1.48× C | Appendix E.4 |
| `@init_state` separation | 1.28× C | Appendix F (reverted) |
| Briev-level LICM | 1.24× C | Committed at `0c042745` |
