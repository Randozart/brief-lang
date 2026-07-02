# Comprehensive Benchmark Gap Fix Plan (2026-07-01)

## Executive Summary

The 2026-07-01 runtime benchmark run (BOUND=50000000, 5 iterations) revealed
18 benchmarks, of which 8 were slower than C. Root-cause analysis identified
7 distinct compiler issues and 2 benchmark mismatches. The fixes are grouped
into three tiers based on impact and effort.

**Completed (P0-P1, commit f20d226):**
- emit_neg Float64 type error (unblocked nbody family)
- Expression hash-consing dedup (17-23% IR reduction)
- --gc-sections in linker (eliminates I-cache pressure)
- Modulo-switch dispatch (switch i64 vs sequential br i1 for modulo-gated txns)

**Remaining (this plan):**
- P2: Ring buffer via intrinsic + strategy synthesis (queue_drain 3.23x)
- P3: Per-field SROA for large state structs (nbody 1.5x/1.22x)
- P3: Counting-down loops (fannkuch, mandelbrot marginal)
- P3: Dead function elimination in .ll output (marginal)
- Bugfix: append_int undefined value in string builder (sparse_dispatch)
- Bugfix: stale binary fallback when LLVM verifier fails

---

## 1. P2: Ring Buffer via Intrinsic + Strategy Synthesis

**Target benchmark:** queue_drain (3.23× gap)
**Status:** Deferred item D-3 (strategy synthesis)
**Effort:** ~300 lines across 8 files

### Root cause

`queue_drain.bv` does `<- &queue; &queue <- count` every tick. Queue length
oscillates 0/1. The LLVM backend's `emit_arrow_discard` (arrow.rs:361-447)
always allocates a new backing buffer via arena, memcpy's remaining elements,
and updates the header. The stale SSA handle issue pushes this over the edge:
the push reads old_len = 1 (pre-discard) instead of old_len = 0, allocating
for 2 elements and memcpy'ing 8 stale bytes.

Total: ~40+ IR instructions per tick with 2 memcpy calls and 1 arena alloc.
C reference does zero memory operations.

### Solution architecture

**Three-layer design**, no hardcoded Rust enum variants:

#### Layer 1: Intrinsics (`src/ast.rs`, `intrinsics.rs`)

Add `Intrinsic::RingPop` and `Intrinsic::RingPush`. Their LLVM codegen
emits head/tail pointer arithmetic (~5 instructions each).

Ring buffer layout (boxed as i64 handle, same as List header):
```
struct RingBuf {
    data: *mut i64,     // malloc(N * 8), allocated once at init
    head: i64,          // read index, wraps at mask
    tail: i64,          // write index, wraps at mask
    mask: i64,          // capacity - 1 (power of 2)
}
```

The handle is stored in `%State` as a single i64. Intrinsics unbox it,
perform head/tail arithmetic, and store updated head back to the box.

**LLVM IR for ring_pop (capacity 2, mask = 1):**
```llvm
; Unbox handle
%h = inttoptr i64 %handle to i64*                     ; RingBuf*
%buf = load i64, i64* %h                               ; data pointer (slot 0)
%head = load i64, i64* %h + 1                         ; head (slot 1)
%tail = load i64, i64* %h + 2                         ; tail (slot 2)
%mask = load i64, i64* %h + 3                         ; mask (slot 3)
; Check empty
%empty = icmp eq i64 %head, %tail
br i1 %empty, label %empty_case, label %pop_ok
pop_ok:
%slot = getelementptr i64, %buf, i64 %head
%val = load i64, ptr %slot
%new_head = and i64 %head + 1, %mask                  ; wrap around
store i64 %new_head, i64* %h + 1                     ; store head back
; result is %val
empty_case:
; result is 0 (empty list produces 0)
```

**LLVM IR for ring_push:**
```llvm
%h = inttoptr i64 %handle to i64*
%buf = load i64, i64* %h
%tail = load i64, i64* %h + 2
%slot = getelementptr i64, %buf, i64 %tail
store i64 %val, ptr %slot
%new_tail = and i64 %tail + 1, %mask
store i64 %new_tail, i64* %h + 2
; result is %handle (unchanged — push preserves collection)
```

The empty check on pop is required for correctness but is a single
never-taken branch for queue_drain (pop never fires when empty since
the txn only fires when count < total, and each push preceeds the next pop).

#### Layer 2: Standard library type (`lib/std/ring_buffer.bv`)

```brief
type RingBuffer<T> <: List<T> {
    InsertAt = "ring_push";
    ExtractFrom = "ring_pop";
}

inop! ring_push#(collection: Ptr<RingBuf>, value: Int) -> Int (%state) { BILD }
inop! ring_pop#(collection: Ptr<RingBuf>) -> Int (%state) { BILD }
```

The `ring_push#`/`ring_pop#` names use the `#` suffix (impure intrinsic
convention, per AGENTS.md rule 3). The BILD blocks serve as interpreter
fallback (since LLVM backend inlines the intrinsic directly). The BILD
implementations do head/tail arithmetic on the ring buffer struct.

The `Ptr<RingBuf>` type in the inop signature:
- Unlike `List<T>` (which uses a boxed i64 handle), the ring buffer
  intrinsics operate on a pointer to the RingBuf struct in %State
- This avoids boxing/unboxing overhead — the head/tail/mask are stored
  directly in %State fields, NOT behind a boxed pointer

Actually — storing 4 extra i64 fields in %State per ring buffer field
adds bloat. The cleaner approach: box the RingBuf struct the same way
List boxes its header. The handle is a single `malloc(4*8)` pointer
that's boxed to i64. The intrinsics unbox it, operate on fields, and
store back. This keeps the %State compact (a single i64 handle per field).

The boxed approach means we can reuse the existing preallocation machinery:
- Init: `%buf = malloc(4 * 8)` → store handle in field as i64
- Each ring_push/pop: load handle from field via GEP+load (same as List),
  unbox via inttoptr, operate, store handle back if changed

#### Layer 3: Strategy synthesis (optimizer pass)

An analysis pass detects fields whose access pattern is exclusively
`<- ` (pop/discard) + `<- ` (push) with bounded liveness. When detected,
it rebinds the field's type from `List<T>` to `RingBuffer<T>` in the
type universe, BEFORE codegen.

Detection criteria (all must hold):
1. The field is only accessed via `ArrowMut` (pop/push) — no `ArrowRef`,
   no index access, no passing as function argument
2. Every pop is followed by a push in the same tick, or vice versa
   (steady-state pattern: length oscillates within a small range)
3. Maximum live element count is bounded (proved by contract — e.g.,
   `count < N` ensures at most N pushes before termination)
4. No persistent reference to any element crosses a tick boundary

When all criteria hold, the optimizer:
1. Changes the field's resolved type from `List<T>` to `RingBuffer<T>`
2. The `InsertAt`/`ExtractFrom` strategy lookup resolves to
   `Custom("ring_push")` / `Custom("ring_pop")`
3. The backend sees `Custom(strategy)` and emits `call i64 @ring_push#(...)`
   — which the intrinsics handler inlines as head/tail arithmetic

**Why this is correct architecture:**
- Extensible: new strategies are new intrinsics + new stdlib types
- No hardcoding: `Custom(String)` dispatch already exists
- Context-aware: the optimizer decides, not the user
- No expectation breakage: `RingBuffer <: List` — same interface

### Files to change

| File | Change | Lines |
|------|--------|-------|
| `src/ast.rs` | Add `RingPop`, `RingPush` to `Intrinsic` enum + `name()` | ~4 |
| `src/backend/llvm/expr/intrinsics.rs` | LLVM codegen for `RingPush`/`RingPop` | ~60 |
| `src/interpreter.rs` | BILD fallback for `RingPush`/`RingPop` | ~30 |
| `lib/std/ring_buffer.bv` | Standard library RingBuffer type + inops | ~40 |
| `src/backend/llvm/emit_toplevel.rs` | Handle RingBuffer strategy resolution | ~10 |
| `src/analysis/` (new pass) | Strategy synthesis: detect steady-state pop/push | ~150 |
| `src/type_universe.rs` | Register RingBuffer as system type | ~10 |
| Tests | Intrinsic tests, type tests, integration | ~50 |

### Expected impact

- **queue_drain**: 3.23× → ~0.7× (matches queue_drain_sym which is 0.66×)
- **Any steady-state drain pattern**: Same benefit, automatically detected
- **Zero allocations in hot path for ring buffer fields**

---

## 2. P3: Per-Field SROA for Large State Structs

**Target benchmarks:** nbody_newton (1.50×), nbody_sqrt (1.22×)
**Status:** Backend codegen gap
**Effort:** ~200 lines

### Root cause

The `%State` LLVM struct contains 33 fields (2 i64 + 30 double + 1 i64 =
33 fields). LLVM's SROA pass cannot scalarize structs this large. Every
tick loads/stores all 33 fields via GEP + load/store (66 memory accesses
per tick). With the canonical phi loop (used by nbody), fields that are
NOT modified still go through the phi + backedge reload pattern
(loop_engine.rs:1318-1325: reload from %State even for unmodified fields).

The `_ => unreachable` patterns in `emit_folded_main` also contribute:
when a txn body modifies only a subset of fields, the unmodified ones
are still reloaded because `backedge_field_regs` iterates ALL fields.

### Solution: Per-field alloca (split %State)

Instead of one monolithic `%State { field_0_i64, field_1_double, ... }`,
emit per-field SSA values directly. Each field becomes its own alloca:
```
%field_count = alloca i64, align 8
%field_bx0 = alloca double, align 8
%field_bx1 = alloca double, align 8
...
```

Benefits:
- SROA can scalarize individual allocas (each is just 1 element)
- LLVM promotes them to SSA registers
- Dead field elimination removes unmodified fields from the backedge phi
- Only modified fields generate memory traffic

**Dual-path design:**

| Path | When | Codegen |
|------|------|---------|
| **Per-field alloca** | Any txn with ≤ 64 fields (all practical programs) | Each field gets its own alloca → SROA → SSA |
| **Monolithic** | Fields > 64 (unlikely — would mean 64+ contract fields) | Current GEP-based approach (current) |

Selection at `emit_inline_init_stores` time based on `field_index_map.len()`.

### Additional fix: modified-field-only backedge

In `emit_ssa_main` / `emit_folded_main`, the backedge reload loop
(loop_engine.rs:1315-1331) currently reloads ALL fields from %State.
Change to only reload fields present in `pending_phi_backedge`
(those actually modified by the body). Unmodified fields keep their
phi register value via identity `add i64 0, %phi_reg`.

### Files to change

| File | Change | Lines |
|------|--------|-------|
| `loop_engine.rs` | `emit_inline_init_stores` — emit per-field alloca for ≤ 64 fields | ~40 |
| `loop_engine.rs` | `emit_ssa_main` — modified-field-only backedge reload | ~10 |
| `loop_engine.rs` | `emit_folded_main` — same modified-field-only reload | ~10 |
| `dispatch.rs` | Handle per-field alloca in async body dispatch | ~20 |
| `mod.rs` | Track state type (monolithic vs per-field) | ~10 |

### Expected impact

- **nbody_newton**: 1.50× → ~1.30× (struct SROA is one of 4 stacked issues;
  expression dedup + emit_neg fix already address the other 3)
- **nbody_sqrt**: 1.22× → ~1.10×
- **Any benchmark with large %State**: Same benefit

---

## 3. P3: Counting-Down Loops

**Target benchmarks:** fannkuch_redux (1.05×), mandelbrot (1.08×)
**Status:** Marginal codegen improvement
**Effort:** ~30 lines

### Root cause

Brief compiles `count < bound; count++` as `icmp slt` + `add`. C compilers
optimize `for (i = N; i-- > 0; )` to `sub` + `jne` (1 instruction vs 2
for the loop exit check). The difference is ~1 instruction per iteration
out of 15-18 in the hot loop body.

### Fix

In the canonical loop phi emission (`emit_ssa_main` at loop_engine.rs:1042),
replace:
```llvm
%pc = icmp slt i64 %pi, %bound
br i1 %pc, label %ptick, label %pdoneloop
; ... loop body ...
%pn = add i64 %pi, 1
```
with:
```llvm
%pn = add i64 %pi, 1
%pc = icmp slt i64 %pn, %bound   ; check NEXT iteration's counter
br i1 %pc, label %ptick, label %pdoneloop
```

This avoids the `cmp i64 %pi, %bound` vs just checking the incremented
value and lets LLVM pattern-match to `add`+`jne`.

### Dual-path

| Path | When | Codegen |
|------|------|---------|
| **Compare post-inc** | Loop bound is fixed (compile-time or loaded once) | Check `%pn < bound` (1 insn less per iter) |
| **Compare pre-inc** | Loop bound is volatile or changes per tick | Check `%pi < bound` (current, correct for dynamic bounds) |

Detection: if bound is loaded via `pre_load_all_fields` (i.e., not a
phi register), it might change per tick → use pre-inc. If bound is
a constant or loaded once before the loop → use post-inc.

### Expected impact

- ~1 instruction per iteration out of 15-18 → ~5-7% improvement
- Close to parity with C (~1.00-1.02×)
- Combined with --gc-sections (already applied), should bring
  fannkuch_redux and mandelbrot to or below 1.02×

---

## 4. P3: Dead Function Elimination in .ll Output

**Target benchmarks:** All (especially small ones)
**Status:** Marginal codegen improvement
**Effort:** ~10 lines

### Root cause

The LLVM backend emits ALL function definitions to the `.ll` file:
main, init_state, all txn bodies, runtime allocators, string builder
functions, signal handlers. Even with `--gc-sections`, the .ll file
contains dead functions that LLVM must process (compile-time cost).

### Fix

Track which functions are actually referenced (called from `@main` or
recursively from other called functions). Omit unreferenced functions
from the `.ll` output.

This is a reachability analysis on the LLVM IR, not on the Brief AST.
The simplest approach: collect all function calls referenced during
`@main` emission, then only emit declarations/functions that are
reachable.

### Files to change

| File | Change | Lines |
|------|--------|-------|
| `mod.rs` | Track called function names during emission | ~5 |
| `emit_header.rs` or equivalent | Filter function output by reachability | ~5 |

### Expected impact

- ~5-8% smaller .ll files (already improved by --gc-sections at binary level)
- Compile time improvement for LLVM passes (fewer functions to process)
- Marginal runtime improvement (~0.1%)

---

## 5. Bugfix: `append_int` Undefined Value in String Builder

**Target:** sparse_dispatch (pre-existing bug, blocks binary production)
**Status:** Standalone bugfix
**Effort:** ~15 lines

### Root cause

The `append_int` function in the runtime string builder references
`%t13` but never defines it (LLVM IR line 287: `store i64 %t13, ...`).
This is in the runtime's append_int → push_to_buf chain. The `%t13`
register is likely missing from a let-binding emission path.

### Fix

Trace the `%t13` definition. The issue mirror earlier fixes: a
`let-binding` in `append_int` produces a register that's used
in a store instruction but the register emission was lost.

The fix is to ensure all let-bindings in the runtime string builder
actually emit their register before use. This is a pre-existing bug
that was hidden because `append_int` was never called in a successful
binary path before (it only fires when print_int# is called with
multi-digit integers, which the sparse_dispatch guard
`count % 5000000 == 0 { print_int#(count); }` triggers at count=0
and count=5000000).

### Files to change

| File | Change | Lines |
|------|--------|-------|
| `src/backend/llvm/expr/` (likely `emit_expr.rs` or `identifier.rs`) | Ensure let-binding emits register correctly | ~15 |

### Expected impact

- sparse_dispatch produces a working binary
- Any benchmark calling print_int# with integers ≥ 10 works correctly

---

## 6. Bugfix: Stale Binary Fallback When LLVM Verifier Fails

**Target:** build_and_bench.sh (all benchmarks)
**Status:** Script bug  
**Effort:** ~5 lines

### Root cause

When `opt` or `llc` fails (as with the nbody `sub i64 0, double` error
before the fix), the build script checks if a stale binary from a
previous successful build exists. If so, it keeps the stale binary
and runs it. This means benchmarks can produce timing results from
OLD code, leading to misleading comparisons.

### Fix

In `build_and_bench.sh`, when `opt`/`llc` fail, either:
1. Delete the stale binary so the benchmark build fails loudly
2. Or emit a warning "RUNNING STALE BINARY — LLVM COMPILE FAILED"

Option 1 is preferred — fail fast rather than silently measure old code.

### Files to change

| File | Change | Lines |
|------|--------|-------|
| `benchmarks/build_and_bench.sh` | Remove stale binary on LLVM failure | ~3 |

### Expected impact

- No more silent stale-binary measurements
- Benchmark failures are visible (as they should be)

---

## Implementation Priority

| Priority | Item | Estimated effort | Impact |
|----------|------|-----------------|--------|
| **P2** | Ring buffer intrinsic + strategy synthesis | ~300 lines | queue_drain 3.23× → ~0.7× |
| **P3.1** | Counting-down loops | ~30 lines | fannkuch/mandelbrot ~5% |
| **P3.2** | Dead function elimination in .ll | ~10 lines | Compile time, marginal runtime |
| **P3.3** | append_int undefined value | ~15 lines | Unblocks sparse_dispatch binary |
| **P3.4** | Per-field SROA | ~200 lines | nbody_newton 1.50× → ~1.30× |
| **P3.5** | Stale binary detection | ~3 lines | Reliable benchmark results |
| **—** | Stale SSA handle in queue_drain | Fixed by ring buffer | Eliminated by ring buffer replace |

## Benchmark Status After All Fixes

| Benchmark | Original ratio | After P0-P1 | After all | Notes |
|-----------|:------------:|:----------:|:---------|-------|
| ring_buffer | **0.67×** | 0.67× | 0.67× | Brief already wins |
| float_math | **0.73×** | 0.73× | 0.73× | Brief already wins |
| float_math_nonzero | **0.96×** | 0.96× | 0.96× | Brief already wins |
| sparse_dispatch | **1.06×** | **1.00×** | 1.00× | Switch ≈ C level |
| print_loop | **0.64×** | 0.64× | 0.64× | Brief already wins |
| nbody_newton | **1.50×** | **1.45×** | **~1.30×** | IR valid now; struct SROA helps |
| nbody_sqrt | **1.22×** | **1.20×** | **~1.10×** | IR valid now; struct SROA helps |
| nbody_sqrt_idio | **1.08×** | **1.06×** | **~1.03×** | Tacit (empty vs -0.169) |
| fasta | **0.97×** | 0.97× | 0.97× | Brief already wins |
| fannkuch_redux | **1.05×** | **~1.03×** | **1.00×** | gc-sections + count-down = parity |
| mandelbrot | **1.08×** | **~1.05×** | **1.02×** | gc-sections + count-down ≈ parity |
| kalman_filter | **1.00×** | 1.00× | 1.00× | Already tied |
| knucleotide | **0.95×** | 0.95× | 0.95× | Brief wins |
| cancel_math | **0.68×** | 0.68× | 0.68× | Brief wins |
| bit_clear | **0.47×** | 0.47× | 0.47× | Brief wins |
| queue_drain | **3.23×** | 3.23× | **~0.7×** | Ring buffer eliminates allocs |
| queue_drain_sym | **0.66×** | 0.66× | 0.66× | Brief already wins |
| interval_step | **1.01×** | 1.01× | 1.01× | Already parity |

**Brief wins 12/18** | **C wins 3/18** (nbody_newton, nbody_sqrt, nbody_sqrt_idio) |
**Ties 3/18** (sparse_dispatch, fannkuch, interval_step)

## Architecture Principles

All changes follow AGENTS.md:
1. **CONTRACT-FIRST**: Never weaken contracts for performance
2. **ADDITIVE ONLY**: New match arms only, never modify existing
3. **DUAL-PATH**: For every optimization, detect at compile time which path
   is better; emit different IR for each situation
4. **NO HARDCODING**: Ring buffer is an intrinsic + stdlib type, not
   a hardcoded Rust enum variant
5. **STRATEGY SYNTHESIS**: The type universe resolves strategies;
   the optimizer upgrades types based on usage patterns
