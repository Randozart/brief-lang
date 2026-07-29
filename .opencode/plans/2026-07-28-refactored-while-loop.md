# Nbody Refactoring: Happy Path Strategies Catalog

**Date:** 2026-07-28
**Goal:** List every confirmed working optimization strategy and its role in the refactored while-loop.

## The Goal

Reproduce all all-time bests simultaneously by making the 5 independent decision axes
chooseable per-program based on measured properties, not hardcoded thresholds.

## The 5 Decision Axes

Every optimization strategy is a combination of 5 independent choices:

| Axis | Choices | Determines | Affected benchmarks |
|------|---------|-----------|-------------------|
| **A. Dispatch** | while-loop / per-field phi / EmitInlineSsa / pure-counter fold | How state fields flow between iterations | ALL |
| **B. Attributes** | `memory(rdwr)` / `argmem:rdwr` / `argmem:rdwr+willreturn` | Whether SROA fires on the hot loop | ALL with FFI |
| **C. SLP** | our SLP / LLVM-only SLP / none | Whether vector ops are emitted | nbody sqrt/newton, kalman |
| **D. Inlining** | alwaysinline / noinline / no txn function | Which function's attributes govern the loop | ALL with separate txn fn |
| **E. Metadata** | none / `!range` / `!prof` | Whether annotations alter LLVM's pass behavior | ALL with field loads/branches |

## Benchmark × Strategy Matrix

| Benchmark | Best | A: Dispatch | B: Attributes | C: SLP | D: Inlining | E: Metadata |
|----------|------|-------------|--------------|--------|-------------|------------|
| ring_buffer | 0.99x | while-loop (#9) | memory(write) | none | no txn fn | none |
| nbody_newton | 0.75x | while-loop (#9) | memory(write) | LLVM SLP | alwaysinline into @main | none |
| nbody_sqrt_idio | 0.67x | per-field phi (#9) | argmem:rdwr | none | alwaysinline into @main | none |
| float_math | 0.81x | per-field phi (#9) | memory(write) | none | alwaysinline | none |
| kalman | 0.95x | per-field phi (#9) | memory(write) | none | alwaysinline | none |
| sparse_dispatch | 0.09x | folded | N/A | N/A | N/A | N/A |
| queue_drain | 0.01x | folded | N/A | N/A | N/A | N/A |
| interval_step | 0.01x | folded | N/A | N/A | N/A | N/A |

## Working Strategies (Happy Paths)

### H1. Flat GEP+load+store (NO per-field phis)

Era 5's hot loop loaded every field via `getelementptr + load`, computed the new value,
and stored it back via `getelementptr + store`. NO phi nodes. This creates a flat
instruction sequence that LLVM's SLP vectorizer can merge across bodies.

**In the refactored while-loop:** `emit_while_main` already does this. Keep it.

### H2. `#9 = memory(readwrite)` on @main

Era 5's @main used `#9 = memory(readwrite)`. The -283 reduction was found WITHOUT
argmem:readwrite on @main. The flat sequence alone was sufficient.

**Keep `#9` unchanged.** Do NOT try to use `#12` or `#8` for @main.

### H3. NO `!range` or `!prof` metadata on hot loop loads/branches

Era 5 had no `!range` metadata on field loads and no `!prof` metadata on guard branches.
Our current `emit_while_main` DOES emit these because `load_field_type()` appends `!range`
and the guard emission adds `!prof`. These metadata nodes change LLVM's optimization.

**In the refactored while-loop:** Conditionally skip `!range`/`!prof` emission when the
while-loop is selected. This is a ~2-line change: gate the `!range` append on a flag,
and skip `!prof` emission when `should_vec` is false (the while-loop doesn't use SLP).

### H4. NO `noundef` on the %state alloca (implicit)

Era 5's `@main` has no state parameter — it uses `%state = alloca %State`. The alloca
is always defined. No `noundef` is emitted for the alloca itself.

**Our `emit_while_main` already does this correctly** — `@main()` parameter is `void`,
and `%state = alloca %State` is emitted without `noundef`. The `noundef` on txn function
params (Step 2) doesn't affect @main's alloca.

### H5. `alwaysinline` NOT needed (body is in @main directly)

Era 5 used `alwaysinline` on the txn function to inline the body into @main. With the
while-loop, the body is ALREADY in @main — no txn function is needed. This eliminates
the `alwaysinline` requirement entirely.

**Our refactored while-loop emits the body directly into @main** — no separate txn
function, no `alwaysinline` needed. This avoids the attribute conflict issue.

### H6. `is_reduction_pattern` (per-lane consumer check)

Blocks SLP vectorization for reduction patterns (mag_sq: dsq01 = dx01² + dy01² + dz01²)
while allowing accumulation patterns (velocity update: vx0 -= dx01*mag01 - dx02*mag02).

**Keep in the refactoring.** Already working.

### H7. `estimate_template_depth` with actual `tree_depth()`

Former stub returned `if width > 3 { 2 } else { 1 }`. Now returns actual expression
depth, making chain_pass_ok's cost-gain model accurate.

**Keep in the refactoring.** Already working.

### H8. `post_hoist_read_set` (swan-song field tracking)

Tracks only the fields the swan song reads, avoiding blanket stores of ALL 33 fields.
Zero gain for current benchmarks but correct.

**Keep in the refactoring for future programs.**

## Failed Experiments (Anti-Patterns)

These are confirmed WRONG or HARMFUL for nbody. DO NOT include in the refactoring:

| Experiment | Why it failed | Evidence |
|-----------|--------------|----------|
| `#12` on @main by field count | Never activated for nbody (33>14) | nbody stayed at #9 |
| `willreturn` on `#11` | LLVM auto-vectorizer harmed kalman | kalman 3.51x |
| While-loop for ALL `has_body_ffi` | while-loop harms scattered access patterns | kalman 3.51x |
| `#9` attribute by `has_unguarded_ffi` | False positives from sqrt() as FFI | Never activated |
| chain_pass_ok threshold `*4` | mag_sq groups still passed | No change |
| Peak register pressure dispatch | while-loop in CURRENT compiler ≠ Era 5's | 1.41x vs 1.09x |

## The Refactored While-Loop Specification

```llvm
define i32 @main() local_unnamed_addr #9 {
entry:
  %state = alloca %State, align 8
  ; init all fields (emit_inline_init_stores)
  br label %.loop

.loop:
  ; count < bound? (GEP+load+icmp)
  br i1 %cond, label %.body, label %.exit

.body:
  ; GEP+load %state.COUNTER
  ; compute dx01, dy01, dz01: GEP+load, arithmetic → GEP+store
  ; compute mag_sq, dist: same pattern
  ; compute velocity accum: CHAINED fsub pattern
  ; compute position update: same pattern
  ; GEP+store result back
  ; -- NO !range metadata on loads --
  ; -- NO !prof metadata on branches --
  br label %.guard_check

.guard_check:
  ; when count % N == 0? (pure arithmetic, computed directly)
  br i1 %guard_fired, label %.guard_body, label %.latch

.guard_body:
  ; __print_float(energy)  (FFI — but inside guard, NOT in hot path)
  br label %.latch

.latch:
  ; count = GEP+load → add 1 → GEP+store  (NO !range on load)
  br label %.loop

.exit:
  ret i32 0
}
```

Key constraint: **the hot path (.loop → .body → .guard_check → .latch) must have NO
metadata annotations.** Load_field_type must NOT emit `!range` for while-loop-selected
programs. Guard branch must NOT emit `!prof`.

## Implementation

**Change `emit_while_main`** in `counter.rs` (or move to a new function) to:
1. Accept a `bool skip_metadata` parameter
2. When `skip_metadata`, skip `!range` emission in `load_field_type` (via a flag on
   `FunctionContext` or by calling a modified version)
3. When `skip_metadata`, skip `!prof` emission on guard branches (via the same flag)
4. Everything else stays the same — the flat GEP+load+store sequence is already there

## Dispatch Rule (in mod.rs:2736)

```rust
// Use while-loop when:
//   has_body_ffi (required for any reactive txn with prints)
//   AND (total_fields < 5  — trivially small, ring_buffer
//        OR (total_fields >= 20  — large state, nbody
//            && write_density >= 0.8  — dense writes, nbody 33/33=1.0
//            && has_body_ffi))         — FFI in guards
if has_body_ffi && (total_fields < 5
    || (total_fields >= 20 && write_density >= 0.8)) {
    self.emit_while_main(out, ..., &body_stmts, /* skip_metadata = */ true);
```

The `write_density >= 0.8` gate excludes kalman (~0.67 density, scattered matrix
access). The `total_fields >= 20` gate excludes float_math (15 fields). Combined,
only nbody-family benchmarks (33 fields, 1.0 density) use the metadata-free
while-loop.

## Verification

1. `cargo test --lib` — all pass
2. nbody_newton with metadata-free while-loop → SLP should find -283 reduction
3. `opt -O3 -pass-remarks=slp-vectorizer` → `Vectorized horizontal reduction with cost -283`
4. Full benchmark suite: nbody improves, kalman stable, float_math stable
