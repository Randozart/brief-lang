# Inop/BILD Audit Fixes & Extensions

**Date:** 2026-06-25
**Status:** Mostly complete — see implementation order below

## Scope

Addresses 7 post-commit audit items from prior session plus 3 high-impact
inop extensions identified during skiplist work.

---

## Part 1 — Audit Fixes

### A-1. `lib/std/syscall.bv` — Default arch arms wrong (HIGH)

**Bug:** The `default:` arm in every `asm target {}` references x86_64 registers
(`rax`, `rdi`, `rsi`) even when the target architecture is not x86_64. This
produces LLVM IR with mismatched register constraints on AArch64/RISC-V.

**Fix:** `default:` arms must not reference any arch-specific registers. Use
`ud2` as the default (fault on unsupported arch) with no output constraints.

**Files:** `lib/std/syscall.bv` (6 inops, ~80 lines total to fix)

---

### A-2. `#!cfg` — Unknown keys silently skip code (HIGH)

**Bug:** `CfgCondition::evaluate` at `src/ast.rs:2365` uses `_ => return false`
when the key string doesn't match `target_os`, `target_arch`, or `board`. A
typo like `#!cfg(target_os == "linx")` silently discards the guarded code.

**Fix:** Change return type to `Result<bool, String>` so `flatten_cfg` can
emit a warning or error for unknown keys.

**Files:** `src/ast.rs` (CfgCondition::evaluate), `src/parser.rs` (flatten_cfg)

---

### A-3. `examples/volatile-io.bv` — Contract boilerplate (MEDIUM)

**Problem:** Each function repeats 4 contract lines for register bounds.

**Fix:** Add a `in_range` helper.

**Files:** `examples/volatile-io.bv`

---

### A-4. Fn* lens naming — Not idiomatic (MEDIUM)

**Problem:** `FnPtr`, `FnName` etc. use a `Fn` namespace inconsistent with
Brief's lens conventions (`Size`, `IsEmpty`, `Contains`, `Keys`).

**Proposal:** Rename `FnPtr` -> `Address`, `FnName` -> `Name`, etc.

**Files:** Multiple (AST, parser, typechecker, interpreter, LLVM backend)

---

### A-5. `#section` — No test coverage (LOW)

**Problem:** Zero tests for the `section` attribute on inop declarations.

**Fix:** Add one test.

---

### A-6. `examples/bild-asm-target.bv` — Redundant `fast_syscall` (LOW)

**Fix:** Remove the `fast_syscall` inop (duplicates `lib/std/syscall.bv`).

---

## Part 2 — Inop Extensions

### B-1. Atomic operations (HIGH)

`atomic_cas`, `atomic_fetch_add`, `atomic_fetch_sub`, `atomic_fetch_and`,
`atomic_fetch_or`, `atomic_fetch_xor`, `atomic_load`, `atomic_store`.

**Files:** `lib/std/atomic.bv` (new), `lib/std/core/atomic.bv` (copy)

---

### B-2. True skiplist BILD (HIGH)

Replace `{ term %res; }` stubs in `lib/std/skiplist.bv` with real LLVM IR:
random level generation, forward pointer walk, splice, buffer growth.

**Files:** `lib/std/skiplist.bv` (update BILD bodies)

---

### B-3. Stateful inop pattern (MEDIUM)

One concrete `(%state)` example with test coverage.

**Files:** `lib/std/state.bv` (new), backend tests

---

## Implementation Order

1. ✅ A-1 syscall default arms (quick fix, real bug)
2. ✅ A-2 cfg unknown key warning (quick fix, high DX impact)
3. ✅ B-1 Atomic inops (new capability)
4. ✅ B-2 True skiplist BILD (performance)
5. ✅ A-3 volatile-io helper (medium)
6. ✅ B-3 Stateful inop example (medium)
7. ✅ A-4 Fn* rename (medium, cosmetic)
8. ✅ A-5 #section test (low)
9. ✅ A-6 remove fast_syscall (low)
