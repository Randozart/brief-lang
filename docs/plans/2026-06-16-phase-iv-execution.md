# Phase IV Execution Plan — Backend Optimization + officina-cli Compatibility

**Date:** 2026-06-16  
**Status:** Plan — implementation in progress

## Guiding Principles

1. **Benchmarks must improve** — target <1.05x for all benchmarks vs C
2. **officina-cli must compile** — if it uses wrong syntax, add clear error messages
3. **Anti-patterns are compiler bugs** — fix the compiler, don't work around them
4. **No regressions** — existing 909 tests must always pass

## Step 1: Fix A — Remove `add i64 0, %reg` Boxing

**Files:** `src/backend/llvm/emit_expr.rs` (~line 61-65), `emit_stmt.rs`

Every expression result is wrapped in `add i64 0, %source`. This is an SSA no-op that
`-O3` eliminates anyway — removing it at emission time saves LLVM work.

**Change:** Return the source register directly instead of creating a new one.

**Risk:** None — cosmetic. Verified by `opt -O3` producing identical optimized IR.

## Step 2: Compile officina-cli (baseline)

```bash
cd ~/Desktop/Projects/officina-cli
../briev-compiler/target/release/briev-compiler llvm officina.bv --out /tmp/officina_test --prod
```

If it fails, document the error. If it succeeds, compile with clang and test.

## Step 3: Fix C — Canonical Loop Induction Variable

**File:** `src/backend/llvm/loop_engine.rs` (`emit_ssa_main`)

For `[count < bound][count == bound]` transactions, emit a `phi i64` induction variable
in canonical LLVM loop form instead of the current GEP+extractvalue pattern.

This enables LLVM to determine the loop trip count, unlocking unrolling and vectorization.

**Approach:**
- Scan contract for simple `[count < bound]` pattern
- If found + body uses `&count = count + 1`, emit phi + canonical loop
- Only affects `emit_ssa_main` path (no async/triggers)
- Existing body emission reused unchanged

**Risk:** Low — additive change, coexists with existing codegen paths.

## Step 4: Fix B — Native Pointer SSA (conditional)

**Files:** `emit_expr.rs`, `emit_stmt.rs`, `mod.rs`

Only attempted if officina-cli compiles but has string corruption, or if benchmarks
still show significant gap after Steps 1-3.

**Approach:** Keep `i8*` (ptr) for `Type::String` in SSA registers. Only `ptrtoint` to
`i64` at the FFI call boundary.

**Risk:** Medium-High — affects all string operations (concat, index, slice, compare).

## Verification Steps

After each step:
1. `cargo test --lib` — all 909 pass
2. Benchmark timing (knucleotide, fannkuch_redux, mandelbrot, print_loop)
3. Inspect `.ll` for optimization remarks: `opt -O3 -pass-remarks-missed=loop-vectorize`
4. Compile officina-cli and check for errors

## Target Results

| Benchmark | Current | Step 1 | Step 3 | Step 4 | C |
|---|---|---|---|---|---|
| knucleotide | 1.033x | ~1.03x | ~1.01x | ~0.98x | 1.00x |
| fannkuch_redux | 1.104x | ~1.09x | ~1.02x | ~1.00x | 1.00x |
| mandelbrot | 1.104x | ~1.09x | ~1.02x | ~1.00x | 1.00x |
| print_loop | 1.247x | ~1.20x | ~1.20x | ~1.15x | 1.00x |
