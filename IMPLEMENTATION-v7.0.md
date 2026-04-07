# Brief v7.0 Implementation Summary

**Date:** 2026-04-07
**Status:** Implementation In Progress

---

## Quick Reference

| Feature | File | SPEC Section |
|---------|------|--------------|
| Implicit `term true;` | desugarer.rs | 5.3.1 |
| Multi-field FFI outputs | parser.rs | 7.4.1 |
| Multi-return validation | typechecker.rs | Part 11 |
| FFI error warning | typechecker.rs | 7.7 |
| R-Brief syntax fix | SPEC.md, refs | 9.2 |
| Reactor throttling | SPEC.md | 8.4 |
| Mutual exclusion fix | SPEC.md | 8.3 |

---

## Completed Implementations

### 1. Implicit `term true;` Desugaring
**File:** `src/desugarer.rs`
**SPEC Section:** 5.3.1 Implicit `term true;`

Transforms `term;` to `term true;` when postcondition is literal `true`.

```brief
txn activate [ready][true] {
    term;  // becomes: term true;
};
```

**Commits:** `5616fa1`

---

### 2. Multi-Field FFI Success Output Parsing
**File:** `src/parser.rs`
**SPEC Section:** 7.4.1 Multi-Field Success Outputs

Added support for tuple syntax in FFI success types:
```brief
frgn divide(a: Int, b: Int) -> Result<(quotient: Int, remainder: Int), MathError> from "lib/math.toml";
```

**Commits:** `5616fa1`

---

### 3. Multi-Return Validation
**File:** `src/typechecker.rs`
**SPEC Section:** Part 11 (Multi-Return Functions)

Added `check_statement_with_outputs()` to validate that term outputs match definition output types.

**Commits:** `5616fa1`

---

### 4. FFI Error Enforcement (Partial)
**File:** `src/typechecker.rs`
**SPEC Section:** 7.7 Error Handling Requirements

Added warning when FFI result is assigned without error handling:
```
F101: FFI call result not handled
```

```brief
let result = read_file(path);  // Warning: should use is_ok()/is_err()
```

**Commits:** `a1277fc`

**Note:** Full enforcement requires tracking variable state through guards. Current implementation provides a warning foundation.

---

### 5. R-Brief Syntax Corrections
**Files:** `spec/SPEC.md`, `spec/LANGUAGE-REFERENCE.md`, `spec/RENDERED-BRIEF-GUIDE.md`

- Fixed rstruct syntax: HTML is inline using `<tag>` inside rstruct
- Added `render` standalone view component documentation
- CSS imported at file top with standard `import` statement

**Commits:** `a6929ad`

---

### 6. Reactor Throttling Documentation
**Files:** `spec/SPEC.md`, `spec/RENDERED-BRIEF-GUIDE.md`
**SPEC Section:** 8.4 Reactor Throttling

Documented `@Hz` declarations:
```brief
reactor @10Hz;  // File-level default
rct txn fast [c][p] { ... } @60Hz;  // Per-transaction override
```

**Commits:** `a6929ad`

---

### 7. Mutual Exclusion Clarification
**File:** `spec/SPEC.md`
**SPEC Section:** 8.3 Async Transactions

Clarified that preconditions only need to be mutually exclusive when they write to overlapping state. Reading-only or writing to different variables is fine.

**Commits:** `a6929ad`

---

## Pending Implementations

### 1. `term functionCall();` Verification
**SPEC Section:** 5.3.2 `term functionCall();`

Verify that function call output satisfies postcondition:
```brief
txn increment [count < 100][count == @count + 1] {
    term addOne(@count);  // Compiler verifies: addOne(@count) == @count + 1
};
```

**Status:** Not started

---

### 2. Complete FFI Error Enforcement
**SPEC Section:** 7.7

Track Result variables and enforce:
- `.value` only accessible after `.is_ok()` or `.is_err()` check
- Compiler should reject unsafe access, not just warn

**Status:** Partial (warning only)

---

### 3. Dynamic FFI Registry
**SPEC Section:** 7

Replace hardcoded builtins in interpreter with dynamic registry loaded from TOML.

**Status:** Not started

---

## Documentation Updates

### Version: 7.0

All documentation updated to v7.0:
- `spec/SPEC.md`
- `spec/LANGUAGE-REFERENCE.md`
- `spec/LANGUAGE-TUTORIAL.md`
- `spec/FFI-GUIDE.md`
- `spec/RENDERED-BRIEF-GUIDE.md`
- `spec/QUICK-REFERENCE.md`

**Commits:** `7d0d513`, `a6929ad`

---

## Git History

```
c0b52a0 docs: add v7.0 implementation summary
a1277fc impl: add FFI error enforcement warning
5616fa1 impl: Phase 1 core language features
a6929ad docs: fix rstruct syntax and add reactor throttling
7d0d513 docs: update documentation to v7.0 with new features
```
