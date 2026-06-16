# Thorough Fixes: setvbuf Policy + Analysis-Time Guard Hoisting

**Date:** 2026-06-16  
**Status:** Plan — implementation in progress

## Fix 1: Global stdout Buffering Policy

### Problem
Every print intrinsic (`print_int#`, `print_float#`, `putchar#`, `println#`) calls
`fflush(stdout)` individually. For `putchar#` at 50M iterations, this is 50M syscalls
— 99x slowdown vs C.

### Root Cause
Phase I cargo-culted the `fflush` pattern from `print_int#` onto `putchar#` without
considering the per-character performance impact.

### Thorough Fix
1. **Emit `call void @setvbuf(ptr @stdout, ptr null, i32 1, i64 0)` at program startup**
   (`_IOLBF = 1` = line-buffered mode). This is a single declarative policy.
2. **Remove `fflush(stdout)` from `putchar#`** — line-buffered mode means `\n`
   auto-flushes; single chars buffer until full (fast).
3. **Remove `fflush(stdout)` from `print_int#` and `print_float#`** — they already
   end with `\n`, which auto-flushes in line-buffered mode.
4. **Keep `fflush(stdout)` in `println#`** — it prints arbitrary strings that may
   not end with `\n`. Needed for TUI/officina-cli real-time responsiveness.

**Why this is future-proof:** A single buffering policy at program init. No per-intrinsic
flush logic to maintain. Works for all output types.

### Files
- `src/backend/llvm/mod.rs` — emit `setvbuf` call in module header
- `src/backend/llvm/emit_expr.rs` — remove `fflush` from print_int, print_float, putchar
- `src/backend/llvm/loop_engine.rs` — remove `fflush` from `emit_post_print`

---

## Fix 2: Analysis-Time Guard Hoisting → Folded SSA Path

### Problem
The dispatch decision at `mod.rs:1179-1189` chooses GEP+store memory path when the
body has guards. GEP+store breaks SSA reduction chains — LLVM can't identify
`checksum` as a reduction, blocking vectorization.

### Root Cause
The dispatch analysis (`is_uniform_body_group`, `has_guards`) runs on the ORIGINAL
body, which includes terminating guards like `[count == N-1] { term! -> print_int#(...) }`.
Phase III hoists these at EMIT time, but the analysis already chose the GEP+store path.

### Thorough Fix
Hoist terminating guards at ANALYSIS time, before the dispatch decision. The guard-free
body passes the `has_guards` check, and the dispatch chooses the SSA insertvalue path.

**Implementation approach:**
1. Extract the guard hoisting logic from `emit_ssa_main` (loop_engine.rs) into a
   standalone function `hoist_terminating_guards(body) -> (body_stmts, post_hoist_info)`
2. Call this function at dispatch time (mod.rs:~1179) to produce `hoisted_body`
3. Pass `hoisted_body` to the dispatch decision instead of `txn.body`
4. When `emit_ssa_main` calls the hoisting again, it finds nothing left to hoist
   (already done) and runs efficiently

### Impact
Folded SSA insertvalue path enables LLVM to identify reductions → loop vectorization →
fannkuch 1.15x → ~1.00x, mandelbrot 1.11x → ~1.00x.

### Files
- `src/backend/llvm/mod.rs` — call guard hoisting before dispatch decision
- `src/backend/llvm/loop_engine.rs` — extract hoisting into reusable function

## Priority

1. Fix 1 (setvbuf) — immediate impact, small change
2. Fix 2 (analysis-time hoisting) — deeper change, higher impact
