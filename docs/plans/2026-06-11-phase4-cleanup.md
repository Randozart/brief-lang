# Phase 4: SLP Hazard Fix + `as intrinsic` Removal + Cleanup

**Date:** 2026-06-11 18:00 UTC

## Overview

Six remaining work items after `name#()` intrinsic implementation.

---

## 1. SLP Hazard Fix

**File:** `src/backend/llvm/hazard.rs`
**Priority:** HIGH — last measurable perf gap

**Problem:** `max_float_temps` counts ALL `let`-bound float temps as simultaneously live
(line 178: `let peak = (... + max_float_temps as usize + ...)`). In nbody_sqrt, ~60 float
temps are defined sequentially (each defined, used once, then next) but counted as 60
simultaneously live, exceeding AVX2 16-float register limit, disabling SLP. This accounts
for the 1.17× gap vs C.

**Fix:** Replace `max_float_temps` with liveness-interval analysis. Walk body statements,
track define/last-use for each float temp. Compute `max_simultaneous_live` — peak number
of float SSA values live at any program point. Bounded analysis through all benchmarks
(~200 stmts max). O(n²) worst case.

**Sub-steps:**
1. Add `fn compute_peak_live_floats(&self, body: &[Statement]) -> u32` to hazard.rs
2. Build a def-last_use map per body, tracking Interval<usize>
3. Sweep program points (statement boundaries within the body), count active intervals
4. Replace `max_float_temps` usage on line 178 with `peak_live`
5. Update SLP hazard tests to reflect correct register counts

---

## 2. Remove `as intrinsic` from Parser + AST + LLVM Backend

**Files:** Multiple (see below)
**Priority:** HIGH — cleanup after `name#()` fully replaces old mechanism

### 2a. Remove parser code
`src/parser.rs:1408-1424` — delete `as intrinsic` parsing block.

### 2b. Remove AST field
`src/ast.rs:235` — delete `intrinsic_name: Option<String>` from `ForeignSignature`.

### 2c. Remove LLVM declare path
`src/backend/llvm/mod.rs:738` — remove `intrinsic_name` branch in declare loop.
Always emit `declare <ret> @<name>(<args>)` using the function name, not intrinsic name.

### 2d. Remove LLVM call path
`src/backend/llvm/emit_expr.rs:237-263` — simplify `Expr::Call` handling:
remove `intrinsic_name` lookup, always use function name as call target.

### 2e. Update all ForeignSignature constructors
Files setting `intrinsic_name: None`:
- `src/ffi/validator.rs` (lines 154, 203)
- `src/hardware_validator.rs` (line 758)
- `src/typechecker.rs` (line 2212)
- `src/backend/llvm/tests.rs` (multiple test functions)

### 2f. Remove as_intrinsic tests
`src/backend/llvm/tests.rs`:
- `test_as_intrinsic_declare`
- `test_as_intrinsic_with_from_copath`
- `test_normal_frgn_no_intrinsic`

---

## 3. Delete `lib/std/llvm.bv`

**File:** `lib/std/llvm.bv`
**Priority:** MEDIUM — only imported by `std/math.bv`

Simply delete the file. No replacement needed — all intrinsics now available via `name#()`.

---

## 4. Delete `lib/std/math.bv` + Update `nbody_sqrt.bv` & `__init__.bv`

**Files:**
- `lib/std/math.bv` — delete
- `benchmarks/nbody_sqrt.bv` — replace `import { sqrt } from "std/math"` with direct
  `sqrt#(x)` calls throughout (~20 sites).
- `lib/ffi/__init__.bv` — remove `import "std/math"`

---

## 5. Update Architecture Docs

**File:** `docs/architecture/intrinsics.md`
- Header: "Design — not yet implemented" → "Implementation complete" + date
- Migration table: mark steps 1–9 as done
- Update code examples to match actual implementation
- Add note about removed `as intrinsic`

---

## 6. Prior-State Semantics Audit + Documentation

**Files:**
- All `benchmarks/*.bv` — audit `node` blocks for chained `&field = ...` assignments
- `docs/architecture/glossary.md` — add "Reactive Transaction Semantics" section

**Audit criteria:** `node` has deferred-write semantics: all state reads within a tick
see pre-tick values. This is correct for reactive/reactive transactions. Sequential
`&field = x; &field = y; &field = z;` chains do NOT accumulate — each reads the same
original pre-tick value. The correct pattern is a single combined expression:
`&field = (field * A + B) % C;` or use a callable `txn`.

**Benchmarks to audit:**
- `fasta.bv` — already fixed
- `nbody_newton.bv` — check node body
- `nbody_newton_sym.bv` — check node body
- `nbody_sqrt.bv` — check node body
- `fannkuch_redux.bv` — check node body
- `fannkuch_redux_sym.bv` — check node body
- `knucleotide.bv` — check node body
- `float_math_nonzero.bv` — check node body
- `kalman.bv` — check node body
