# 2026-06-02 Code Review Findings & Fix Plan

Two independent reviews arrived simultaneously:
- **Review A**: AI-agent evaluation of `LlvmBackend` (7 optimization categories)
- **Review B**: Manual audit of shared modules (7 bugs + 1 benchmark suggestion)

This document consolidates both, prioritizes them, and tracks implementation.

---

## Review A — LLVM Backend Optimization (from `2026-06-02-llvm-backend-optimization-phases.md`)

| # | Category | Priority | Status | Notes |
|---|----------|----------|--------|-------|
| A1 | **alloca+SROA** — replace `phi %State` with alloca+load/store | HIGH | ✅ **DONE** | float_math 41× improvement |
| A2 | **fast-math flags** — `fast` on all fadd/fmul/fsub/fdiv/fcmp | MEDIUM | ✅ **DONE** | Compounds with SROA |
| A3 | **SLP hazard fix** — union+cross-op cap | MEDIUM | ✅ **DONE** | Revised formula |
| A4 | **Typed SSA** — remove i64 boxing for floats | HIGH | ⏳ PENDING | Still worth for correctness; SROA already eliminates boxing in opt |
| A5 | **Pointer provenance** — no ptrtoint/inttoptr | LOW | ⏳ PENDING | For string-heavy programs |
| A6 | **Commutativity pattern bug** — extract_trigger_keys | LOW | ⏳ PENDING | Minor redundant pattern |
| A7 | **fastcc + per-function SLP** | LOW | ⏳ PENDING | Marginal impact |

### Key Finding (A4)
float_math_nonzero 2.32× gap is NOT from boxing — SROA+opt already eliminates all
trunc/bitcast/zext. Gap is instruction scheduling from phi structure vs local registers.
Phase B (typed SSA) still worth doing for correctness (eliminates `is_float_expr` guess)
but won't close this gap.

---

## Review B — Shared Module Bugs

### B1 — UTF-8 Slicing Panic in FFI Helpers
**File**: `lib/ffi/native/src/lib.rs`
**Severity**: CRITICAL — runtime panic on non-ASCII input.
**Functions affected**:
- `__contains_at` (line 353): `haystack[start as usize..]` — byte-index panics on multi-byte chars
- `__find_from` (line 361): `s[start_idx..].find(&needle)` — same issue
- `__UTF8_len` (line 389): returns `s.len()` (byte count) not character count

**Fix**:
```rust
pub fn __contains_at(haystack: String, needle: String, start: i64) -> Result<bool, String> {
    if start < 0 || start as usize > haystack.len() {
        return Ok(false);
    }
    let start_usize = start as usize;
    if !haystack.is_char_boundary(start_usize) {
        return Ok(false);
    }
    Ok(haystack[start_usize..].contains(&needle))
}
```
For `__UTF8_len`: `Ok(s.chars().count() as i64)` instead of `Ok(s.len() as i64)`.

### B2 — Entry-Point Analysis Returns Presence Not Value
**File**: `src/analysis/entry_point.rs:99-108`
**Severity**: CRITICAL — incorrect compile-time precondition evaluation.
**Root cause**: `get_initial_value` returns `Some(decl.expr.is_some())` (true if
any initializer exists) instead of evaluating the actual value.
`let limit: Int = 100` → evaluates to `1`, not `100`.
Any precondition like `[count < limit]` evaluates `0 < 1` instead of `0 < 100`.

**Fix**: Replace with `get_initial_value_numeric`:
```rust
fn get_initial_value_numeric(name: &str, program: &Program) -> Option<i64> {
    for item in &program.items {
        if let TopLevel::StateDecl(decl) = item {
            if decl.name == name {
                return match &decl.expr {
                    Some(Expr::Integer(n)) => Some(*n),
                    Some(Expr::Bool(b)) => Some(if *b { 1 } else { 0 }),
                    _ => None,
                };
            }
        }
    }
    None
}
```

### B3 — Assertion Verifier Ignores Guard's False Path
**File**: `src/assertion_verify.rs:81-103`
**Severity**: CRITICAL — soundness hole in formal verification.
**Root cause**: `Statement::Guarded` only checks the `true` branch (guarded statements).
The `false` path is not explored. A function:
```
sig always_succeeds -> true;
[x > 0] { term true; };
```
is declared verified even though `x <= 0` causes the function to end without
producing `true`.

**Fix**: After checking the guarded branch, continue checking remaining statements
for the false path. If no term is found after the guard, the false path fails.

### B4 — Overlap Detection Checks Only First Declaration
**File**: `src/analysis/cross_reference.rs:119-134`
**Severity**: MEDIUM — missed error detection.
**Root cause**: `if let Some(first) = decls.first() { if first.1.is_none() { ... } }`
only checks the first declaration's bit range. Declarations:
```
let low: UInt @ 0x8000A000 /0..3 = 0;     // has range
let high: UInt @ 0x8000A000 = 0;           // no range → NOT detected
```

**Fix**: Change to `decls.iter().any(|d| d.1.is_none())`.

### B5 — Loop Overshoot Check (Informational)
**File**: `src/proof_engine.rs:1285-1295`
**Severity**: INFORMATIONAL — not a bug.
**Comment at line 1291**: `dist <= 0` → precondition is initially false →
transaction never fires → convergence vacuous. This is correct design.
Cross-txn interference (another txn modifying the counter) is a known
limitation of per-txn analysis.

### B6 — Duplicated Parser Match Blocks
**File**: `src/parser.rs:~4617`
**Severity**: LOW — maintainability.
**Root cause**: The same ~20-arm keyword-to-identifier match block is
copy-pasted in multiple locations. Adding a keyword requires updating
all copies.

**Fix**: Extract `is_keyword_identifier(token: &Token) -> bool` helper.

### B7 — Hardcoded FPGA Address Boundaries
**File**: `src/analysis/address_space.rs:42-68`
**Severity**: MEDIUM — portability.
**Root cause**: Magic addresses like `0x40A80000`, `0x8000A000` hardcoded.
Binds compiler to AMD/Xilinx Zynq UltraScale+.

**Fix**: Load address ranges from `hardware.toml` via `target_spec` or
`MemoryMapping` metadata. Add `address_space` field to memory map entries.

### B8 — Kalman Benchmark: test/jnz Loop Structure
Suggested optimization for the C reference: using `--BOUND/--total` directly
in the loop condition allows `test + jnz` instead of `cmp + jne`.
- Brief already uses `icmp eq` for the folded loop exit (matches `test + jnz`)
- C reference: `while (count < bound)` → `cmp + jne` on x86
- Measured improvement: ~0.01s averaged over 100 runs
- `-march=native` already passed in build_and_bench.sh lines 59, 72
- Brief compile: `opt -O2` + `llc -O2` + `clang -O3 -march=native` (link step)

---

## Consolidated Priority Matrix

| Priority | ID | Bug | File | Effort | Risk |
|----------|----|-----|------|--------|------|
| 🔴 P0 | B1 | UTF-8 slicing panic | `lib/ffi/native/src/lib.rs` | 10 min | Low |
| 🔴 P0 | B2 | Entry-point value != presence | `src/analysis/entry_point.rs` | 15 min | Low |
| 🔴 P0 | B3 | Assertion false-path unsound | `src/assertion_verify.rs` | 20 min | Medium |
| 🟡 P1 | A4 | Typed SSA (correctness) | `src/backend/llvm.rs` | High | Medium |
| 🟡 P1 | B4 | Overlap detection | `src/analysis/cross_reference.rs` | 5 min | Low |
| 🟡 P1 | B7 | Hardcoded address spaces | `src/analysis/address_space.rs` | 30 min | Medium |
| 🟢 P2 | B6 | Parser duplication | `src/parser.rs` | 10 min | Low |
| 🟢 P2 | A5 | Pointer provenance | `src/backend/llvm.rs` | Medium | Low |
| 🟢 P2 | A6 | Commutativity pattern | `src/backend/llvm.rs` | 5 min | Low |
| 🟢 P2 | A7 | fastcc + per-fn SLP | `src/backend/llvm.rs` | 15 min | Low |
| 🔵 P3 | B5 | Cross-txn interference note | `src/proof_engine.rs` | Comment | None |

---

## Current Benchmark Table (post Phase A + C + SLP fix)

| Benchmark | Brief | C | Ratio | Status |
|-----------|-------|---|-------|--------|
| float_math | 0.011s | 0.052s | **Brief 4.5×** | ✅ O(1), beats C |
| float_math_nonzero | 0.380s | 0.165s | **2.32×** | 🔶 Phi scheduling gap |
| iir_filter | 0.172s | 0.119s | 1.44× | ✅ Fields eliminated |
| precompute_sum | 0.002s | 0.002s | tie | ✅ O(1) |
| ring_buffer | 0.007s | 0.002s | 3.3× | 🔶 Startup noise |
| async_counters | 0.004s | 0.005s | ~tie | ✅ |
| sparse_dispatch | 0.077s | 0.002s | startup | 🔶 Redesign needed |
| const_heavy | 0.006s | 0.044s | **Brief 7×** | ✅ |
| kalman_filter | 0.71s | 0.75s | **Brief beats C** | ✅ SLP disabled |

---

## What to Fix Now (P0 items, build mode)

In priority order:

1. **B1 — UTF-8 slicing fix** in `lib/ffi/native/src/lib.rs`
   - `__contains_at`: add `haystack.is_char_boundary(start_usize)` check
   - `__find_from`: same fix
   - `__UTF8_len`: change `s.len()` to `s.chars().count()`

2. **B2 — Entry-point value bug** in `src/analysis/entry_point.rs`
   - New `get_initial_value_numeric()` that evaluates `decl.expr` to i64
   - Update `evaluate_to_constant` to use it instead of `get_initial_value`
   - Update `is_initially_true` for `Expr::Identifier(name)` to evaluate numeric

3. **B3 — Assertion false-path** in `src/assertion_verify.rs`
   - After processing a `Guarded { condition, statements }`, continue checking
     subsequent statements for the false path
   - Both branches must independently produce `term true`

4. **B4 — Overlap detection** in `src/analysis/cross_reference.rs`
   - Change `decls.first()` to `decls.iter().any(|d| d.1.is_none())`

Then resume the LLVM optimization track (A4/A5/A6/A7).
