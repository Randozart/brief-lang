# Phase 2 Briev: Contract Optimization (!range + @llvm.assume + nuw nsw)

**Date:** 2026-05-29  
**Spec Reference:** `05-CONTRACT-TO-METADATA.md`  
**Prerequisite:** Phase 1 complete (let/term/guarded emission)  
**Estimated Effort:** 2 days  

## Goal

Every `%State*` parameter gets `noalias` + `nocapture` (done). Preconditions inject `!range` metadata and `@llvm.assume` calls. Bounded arithmetic gets `nuw nsw`. Single-assignment guards become `select` instructions.

## Deliverables

### 1. !range Metadata on Field Loads

When a precondition contains `[x < N]` or `[x >= 0]` & `[x < N]`, attach `!range !{ 0, N }` to the corresponding `load` instruction.

**Analysis:** The precondition expression tree is parsed for `Expr::And(Lt(Ident("x"), Integer(N)), ...)` patterns. The bound `[lower, upper)` is attached as metadata.

```llvm
; Before: no !range
%count = load i64, i64* %ptr, align 8

; After: !range tells LLVM count ∈ [0, 100)
%count = load i64, i64* %ptr, align 8, !range !0
; ...
!0 = !{ i64 0, i64 100 }
```

**Key rule** (from spec audit): Signed bounds use `2^63` (`9223372036854775808`) as the upper bound for `[x >= 0]`, not `-1` (which wraps to `2^64-1`).

### 2. @llvm.assume for Complex Preconditions

When a precondition can't be expressed as simple range metadata (multiple variables, relationships), emit a debug-mode runtime check or release-mode `@llvm.assume`.

**Debug mode** (default):
```llvm
%c1 = icmp slt i64 %count, 100
br i1 %c1, label %safe, label %panic
panic:
  call void @__panic(i8* "...")
  unreachable
safe:
```

**Release mode** (`--release` flag):
```llvm
%c1 = icmp slt i64 %count, 100
call void @llvm.assume(i1 %c1)
```

### 3. nuw nsw for Bounded Arithmetic

When `x` has a proven upper bound (e.g., `[x < 100]`) and we compute `x + 1` or `x - 1`, emit `nuw nsw` on the arithmetic instruction:

```llvm
; count ∈ [0, 100), so count + 1 cannot overflow
%new = add nuw nsw i64 %count, 1
```

### 4. Guard → select Conversion

When a `[cond] { &x = val; }` guarded block contains exactly one assignment, emit `select` instead of `br`:

```llvm
; Instead of:
; %i1 = icmp ne i64 %cond, 0
; br i1 %i1, label %then, label %end
; then: store i64 42, i64* %ptr; br label %end
; end:

; Emit:
%old = load i64, i64* %ptr
%i1 = icmp ne i64 %cond, 0
%new = select i1 %i1, i64 42, i64 %old
store i64 %new, i64* %ptr
```

**Inhibition:** MMIO volatile stores, multiple statements, nested guards.

## New Test Fixtures

| Fixture | Tests |
|---------|-------|
| `range_contract.bv` | `[x < 100]` precondition → !range metadata |
| `complex_pre.bv` | `[x > 0 && y < 100]` → @llvm.assume |
| `bounded_arith.bv` | `[x < 100]` with `x + 1` → `nuw nsw add` |
| `guard_select.bv` | Single-assignment guard → `select i1` |

## Acceptance Criteria

```bash
for f in tests/fixtures/phase2/*.bv; do
  briev-compiler llvm "$f" --out /tmp/p2/
  llc /tmp/p2/$(basename "$f" .bv).ll -o /dev/null  # Must succeed
done
grep "!range" /tmp/p2/range_contract.ll              # !range metadata present
grep "llvm.assume" /tmp/p2/complex_pre.ll             # @llvm.assume present
grep "nuw nsw" /tmp/p2/bounded_arith.ll               # nuw nsw on arithmetic
grep "select" /tmp/p2/guard_select.ll                  # select instruction present
```

## Implementation Checklist

- [ ] Parse precondition tree for `Lt(Ident, Int)` → extract `!range` bounds
- [ ] Attach `!range` metadata to field `load` instructions during `generate_statement`
- [ ] Emit `@llvm.assume` or panic branch based on `release` flag
- [ ] `!0 = !{ i64 0, i64 N }` metadata node at module level
- [ ] `nuw nsw` on `add`/`sub`/`mul` when operands have bounded ranges
- [ ] Guard→select: detect single-assignment blocks, emit `select` + no branch
- [ ] Phase 0 + Phase 1 regression fixtures still pass