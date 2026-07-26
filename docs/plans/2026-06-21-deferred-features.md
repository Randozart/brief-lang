# Deferred Features — Phase 1.5+ Implementation (2026-06-21)

## Scope

Implement select Phase 1.5+ deferred items from REFACTOR_PLAN.md.
Target: close D-6, D-5, D-4 — three self-contained items that add
real value with minimal risk.

## Items by Priority

### D-6: `#volatile` / `#atomic` Pragma Desugaring

**What**: TypeDef body pragmas `#volatile;` / `#atomic;` desugar to
`Volatile = true;` / `Atomic = true;` bindings. No new AST — the
desugarer or type_universe checks for known modifier names and
injects the binding.

**Files**:
- `src/type_universe.rs` — In `resolve_type_def()`, scan body stmts
  for `#volatile` / `#atomic` and inject bindings. Or do it in
  `apply_binding()` by recognizing modifier-like patterns.
- (Alternative) `src/features/toplevel/typedef.rs` — Add a
  desugaring step before resolution.
- `src/ast.rs` — No changes; Volatile/Atomic bindings already exist.
- Tests: `src/features/toplevel/typedef.rs` or `type_universe.rs`.

**Scope**: Small — ~20 lines of resolution logic + ~3 tests.

### D-5: `.#Size` Uniformity for Scalars

**What**: `Int .#Size` currently errors with "requires List or
collection type". For scalars, `Size` should return 1 (a single
element). `Bool .#Size` → 1, `Char .#Size` → 1, `Float .#Size` → 1.
`String .#Size` returns byte count (kept as-is for now — codec-
defined projections like `:> Runes` / `:> Graphemes` are deferred).

**Files**:
- `src/features/projection.rs` — `ProjectionTarget::Size` match arm:
  add Int, Float, Bool, Char cases returning 1.
- `src/interpreter.rs` — `Intrinsic::Size` match arm: same additions.
- `src/typechecker.rs` — `ProjectionTarget::Size` type resolution:
  add Int/Float/Bool/Char → Type::Int.
- `src/backend/llvm/emit_expr.rs` — LLVM `ProjectionTarget::Size`:
  add scalar cases.
- Tests: ~4 new tests across projection and intrinsic forms.

**Scope**: Small — ~15 lines + tests. Must keep interpreter and
backends in sync.

### D-4: Deprecation Warnings for AsStack/AsQueue Projections

**What**: When `val :> AsStack` or `val :> AsQueue` is used, emit a
compiler warning suggesting migration to type metadata
(`InsertAt`/`ExtractFrom`/`AllowIndex`).

**Files**:
- `src/typechecker.rs` or `src/features/projection.rs` — Detect
  `ProjectionTarget::AsStack` / `AsQueue` usage and emit a warning.
- Warning message: `"W001: AsStack is deprecated, use
  InsertAt/ExtractFrom type metadata instead"`.

**Scope**: Trivial — ~5 lines warning emission + ~2 tests.

## Not Implemented This Session

- **D-1** (expression type params): Needs design work.
- **D-2** (codec validation): Needs duck-typing infrastructure.
- **D-3** (InsertAt/ExtractFrom synthesis): Needs strategy codegen.
- **D-7** (runtime constraint guards): Needs LLVM codegen synthesis.
- **D-8** (field access in `<:` queries): Already works.

## Testing

- `cargo test --lib` after each item.
- New tests for each item (at minimum 2 per item).
- Verify no regressions.

## Error/Warnings Added

- D-4: Warning `W001` — `"AsStack is deprecated, use
  InsertAt/ExtractFrom type metadata instead"`
- D-6: Minor — silently ignored unknown pragmas in TypeDef bodies
  stay silent (forward compat).
- D-5: No new errors — existing error paths for unsupported types
  are unchanged.
