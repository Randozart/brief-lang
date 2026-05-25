# Praetor Compliance Plan

**Date:** 2026-05-25
**Status:** IN PROGRESS — fixing 1,675 pre-existing diagnostics across codebase

## Scope

Praetor's `validate --warn` finds 1,675 unproven diagnostics, primarily `[Intent Required]` (missing doc comments) and some `[Datalog Rule 1]` (private data access without auth).

## Strategy

### Phase A: Configure hook to target `./src` only

Generated/3rd-party directories (`out_test/`, `landing-build/`, `page-build/`, `examples/`) are not part of the compiler itself. The pre-commit hook will be modified to run `praetor validate --warn --target ./src`.

### Phase B: Fix all `src/` diagnostics

Roughly ~400 functions in `src/` need intent comments. Pattern:
```rust
/// Intent: describes what this function does, its pre/post conditions, and side effects
```

### Phase C: Fix Datalog Rule 1 violations

The Datalog Rule 1 violations (`// praetor/datalog-rule-1`) involve functions accessing private data without authentication. These need `authenticate()` calls or access logging added.

### Phase D: Extend to other directories

After `src/` is clean, extend the hook's target to include other directories gradually.

## Progress

### 2026-05-25
- Created this plan
- Modified pre-commit hook to target `./src`
- Started fixing `src/backend/` intent comments
