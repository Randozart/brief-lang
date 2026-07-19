# Benchmark Stabilization & Post-Implementation Audit

**Date:** 2026-07-19
**Author:** Compiler agent
**Prerequisite:** July 18 master plan (SSO, SVO, Ptr Level 3, allocation strategy, custom strings) — implemented across ~15 commits, 931 tests passing
**Status:** Plan — ready to implement

---

## Executive Summary

The July 18 feature set (SSO strings, SVO lists, allocation strategy DAG, Ptr Level 3, custom string types) is largely implemented but has two codegen bugs blocking benchmarks and several disconnected code paths. This plan:

1. **Fixes the terminator bug** blocking `precompute_sum` and `async_counters_idio` (the only benchmark blocker)
2. **Runs baseline benchmarks** to measure current performance against C
3. **Audits remaining gaps** from the July 18 plan and fixes those affecting benchmarks
4. **Runs post-fix benchmarks** and compares against baseline
5. **Documents what improved, what hasn't, and why**

### Key principle: fix the platform, not the benchmark

We fix general compiler correctness bugs. We do NOT:
- Add benchmark-specific match arms or special cases
- Weaken existing optimization paths to make benchmarks "pass"
- Tune constants or heuristics to match specific benchmark patterns

A benchmark that fails due to a missing feature is a signal to implement that feature generally. A benchmark that regresses after a fix means the fix broke something.

---

## Current State Audit

### Committed (all from July 18 plan):

| Area | Commit | Status |
|------|--------|--------|
| SSO Phase B flag, literal emission, 2-slot state, `{i64,i64}` type, `extractvalue`, AND-8 tag | (multiple) | ✅ Implemented |
| SSO concat (`emit_sso_concat` in helpers.rs) | (committed) | ⚠️ **Orphaned** — never called from dispatch |
| Utf8View, StaticString, SmallString64 decls + pure-Brief ops | (committed) | ✅ Implemented |
| SVO S0-S4: flag, `is_vector_like`, `push_field_type`, literal, index | `aded093` | ✅ Implemented |
| SVO S5-S6: heap path, index fix | `d079134` | ⚠️ **push/pop via `<-` always heap** |
| Ptr Level 3: `is_local_provenance` fix, `PtrConst`, `(Type, Provenance)`, `is_mutable_location`, compile pipeline | `8d0e4d9`, `7cb6083` | ✅ 6/10 items |
| Allocation strategy DAG + escape detection | `690304b`, `b86a5a2` | ✅ Implemented |
| Arena + CrosswordArena stdlib + thread-safe arena | `a484b9e`, `7e3597e` | ✅ Implemented |
| Natural convergence exit | `50caa29`, `56e7f76` | ✅ Implemented |

### Benchmark status:

| Benchmark | Tag | Builds? | Runs? | Correct? |
|-----------|-----|---------|-------|----------|
| iir_filter | optimizer | ✅ | ✅ (precomputed) | ✅ |
| **precompute_sum** | optimizer | ❌ **terminator bug** | — | — |
| **async_counters_idio** | optimizer | ❌ **terminator bug** | — | — |
| const_heavy | optimizer | ✅ builds | ✅ (precomputed) | ✅ |
| utf8_ops | runtime | ? | ? | ? |
| ring_buffer | runtime | ✅ builds | ? | ? |
| float_math | runtime | ? | ? | ? |
| float_math_nonzero | runtime | ? | ? | ? |
| sparse_dispatch | runtime | ? | ? | ? |
| print_loop | runtime | ? | ? | ? |
| nbody_newton | runtime | ? | ? | ? |
| nbody_sqrt | runtime | ? | ? | ? |
| nbody_sqrt_idio | runtime | ? | ? | ? |
| fasta | runtime | ? | ? | ? |
| fannkuch_redux | runtime | ? | ? | ? |
| mandelbrot | runtime | ? | ? | ? |
| kalman_filter_runtime | runtime | ? | ? | ? |
| knucleotide | runtime | ? | ? | ? |
| cancel_math | runtime | ? | ? | ? |
| bit_clear | runtime | ? | ? | ? |
| queue_drain | runtime | ? | ? | ? |
| queue_drain_sym | runtime | ? | ? | ? |
| queue_drain_idio | runtime | ? | ? | ? |
| interval_step | runtime | ? | ? | ? |

---

## Phase 1: Fix Terminator Bug

### Root cause

`src/backend/llvm/emit_stmt.rs:151-174`: `Statement::Term` handler sets `backend.fun.terminated = true` without emitting an LLVM terminator for void reactive txns (no `callable_txn_result`, `fn_ret_ty = "void"`).

### Fix

Add `writeln!(out, "{}ret void", indent).ok();` in two `else` branches before `backend.fun.terminated = true;`:

1. **Line ~170** — `Statement::Term(val) | Statement::TermBang(val)` with `Some(val)`, void txn path
2. **Line ~173** — Same, `None` (valueless `term;`), void txn path

**Why `ret void` is correct:** In a void reactive txn, `term`/`term!` means "this txn's work is done for this tick — return to the main loop." The main loop calls the txn function as `@txn_fn(ptr %state)` and expects it to return. `ret void` is the correct LLVM terminator.

**Why it's safe:** After `ret void`, any subsequent labels or code are in dead basic blocks. LLVM's optimizer eliminates these. The function epilogue's own `ret void` becomes dead code — no duplication.

**Files:** `src/backend/llvm/emit_stmt.rs` (2 lines added)

### Verification

```bash
cargo build && \
  BOUND=5 ./target/release/brief-compiler build benchmarks/precompute_sum.bv --out benchmarks && \
  ./benchmarks/precompute_sum
cargo test --lib
```

### Tests

Add a unit test: compile a reactive txn with `[guard] { term! -> Print#(x); };` in the body, assert the `.ll` output parses with `llc` (or at minimum contains `ret void` after the guard body and no empty label blocks).

---

## Phase 2: Baseline Benchmarks

### Command

```bash
cargo build --release && bash benchmarks/build_and_bench.sh --runtime --optimizer
```

### Expected output table

| Benchmark | Brief (s) | C (s) | Ratio | Correctness |
|-----------|-----------|-------|-------|-------------|
| ring_buffer | | | | |
| float_math | | | | |
| float_math_nonzero | | | | |
| sparse_dispatch | | | | |
| print_loop | | | | |
| nbody_newton | | | | |
| nbody_sqrt | | | | |
| ... | | | | |

This is the post-SSO, post-allocation-strategy baseline. Compare against the July 18 baseline in `master-overview.md`.

### Success criteria

- All benchmarks compile and run
- No benchmark produces `__FAIL__` (crash/timeout)
- Correctness: all benchmarks MATCH C reference
- Runtime benchmarks: Brief is within 2× of C (acceptable for initial SSO/SVO rollout)

---

## Phase 3: Fix Remaining Gaps (Triage by Benchmark Impact)

### Gap assessment

| Gap | Benchmark impact | Fix effort | Priority |
|-----|-----------------|------------|----------|
| **SSO concat disconnected** (`emit_inline_concat` orphaned) | Affects any string `++` — falls through to `add i64`. Benchmarks using string concat will produce wrong results. | 1 file, 3 lines | **HIGH** |
| **SVO push/pop always heap** | SVO list benchmarks get no inline benefit. Correct but suboptimal. | 1 file, ~20 lines | MEDIUM |
| `Value::Ptr` missing | Interpreter correctness for Ptr programs. No benchmark impact. | 2 files | LOW |
| `memory_mode_fields` missing | SSA fallback for borrowed fields. Affects micro-optimization only. | 2 files | LOW |
| No SSO/SVO tests | Missing regression coverage. No benchmark impact. | 2 files | LOW |

### HIGH: Wire SSO concat

**File:** `src/backend/llvm/emit_expr.rs`

In `emit_binary_op` (or equivalent dispatch function), add a match arm for `BinaryOpKind::Concat` that calls `self.emit_inline_concat(out, indent, l, r)`.

The function `emit_inline_concat` exists at `helpers.rs:753` and has complete SSO-aware logic (short path ≤6 bytes, heap path otherwise). It's just never called.

**Test:** Compile `"hello " + "world"` with `--feature sso-strings`, check output is correct.

### MEDIUM: SVO push/pop awareness

**File:** `src/backend/llvm/emit_stmt.rs`

The `<-` dispatch at lines 34-148 calls the strategy function (e.g., `ring_push`). For SVO types with inline capacity remaining, this should store inline instead. Since SVO is off by default (`feature_svo = false`), this is a correctness issue only when SVO is explicitly enabled.

**Deferred to followup** — the SVO push/pop gap is correct (falls through to heap), just not optimized. No benchmark currently uses `--feature svo`.

---

## Phase 4: Post-Fix Benchmarks

Same command as Phase 2:

```bash
cargo build --release && bash benchmarks/build_and_bench.sh --runtime --optimizer
```

---

## Phase 5: Analysis

### Compare Phase 2 vs Phase 4 tables

| Question | How to answer |
|----------|---------------|
| Did SSO concat fix affect any benchmark? | Check if string-heavy benchmarks changed |
| Did the terminator fix affect performance? | No — it only adds `ret void` in dead paths |
| How does Brief compare to C? | Ratio column |
| Which benchmarks improved vs regressed? | Compare Phase 2 vs July 18 baseline |
| Are there any correctness regressions? | Check correctness column against July 18 |

### Expected analysis

- **All benchmarks compile and run** (terminator fix)
- **SSO concat fix** enables correct string operations (currently `++` produces garbage)
- **Brief vs C ratios** should be similar to July 18 baseline for non-string benchmarks
- **String-heavy benchmarks** may show improvement from SSO (or regression from 2-register ABI)

---

## Phase 6: Document Remaining Gaps

After all benchmarks are complete, update `docs/plans/2026-07-18-master-overview.md` with:
- Current benchmark results table
- List of implemented items with status
- List of remaining gaps (SVO push/pop, `Value::Ptr`, etc.) with priority

---

## Files Changed (Summary)

| File | Phase | Change |
|------|-------|--------|
| `src/backend/llvm/emit_stmt.rs` | 1 | Add `ret void` in `Statement::Term` void-reactive-txn branches |
| `src/backend/llvm/emit_expr.rs` | 3 | Wire `BinaryOpKind::Concat` to `emit_inline_concat` |
| `docs/plans/2026-07-19-benchmark-stabilization.md` | 0 | This plan |
| `docs/plans/2026-07-18-master-overview.md` | 6 | Update with results |

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `ret void` in term handler breaks other code paths (callable txns, non-void returns) | Low | High | Check both conditionals before adding: only add `ret void` in the void-reactive-txn `else` branch, not in the callable or non-void branches |
| SSO concat fix touches wrong dispatch path | Low | Medium | Add as a new match arm before `_ =>` fallthrough — additive only |
| Benchmark results are noisy | Medium | Low | 5 iterations per benchmark, use `TIMER_BIN` fork+exec harness. Document range (min/max) |
| HashMap iteration order affects IR determinism | Low | Medium | All IR-emitting HashMaps are already sorted per AGENTS.md convention |
