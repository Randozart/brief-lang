# Finalize: Inop <T> Generics, Contract Hygiene, Byte Type, Example Cleanup

**Date:** 2026-06-25
**Status:** Active

## Scope

Four interrelated changes that together make the inop system correct,
generic, and verifiable without `--skip-proof`.

---

## Phase 0: Generic Type Parameters on Inops

**Goal:** Make `inop sl_insert<T>(list: SkipList<T>, val: T) -> SkipList<T>` work.

### Parser (`src/parser.rs`, `parse_inop_decl`)

After parsing the inop name token, check for `<` and parse type parameter
list identically to how `parse_definition` does it. Store in a new
`type_params: Vec<String>` field on `InopDeclaration`.

### AST (`src/ast.rs`)

Add `type_params: Vec<String>` to `InopDeclaration`. This struct already
has `params`, `outputs`, `contract`, etc. — this is one more field with
no special behavior in most backends.

### Type Checker (`src/typechecker.rs:2180`)

In the `Intrinsic::UserDefined(name)` arm, after looking up the inop
declaration, perform type variable substitution: match the call-site
type arguments against the inop's type params, bind `T → ConcreteType`,
and substitute into `outputs`. This uses the same logic as defn type
substitution already present in the typechecker.

### Interpreter

No change — the inop fallback is already evaluated with concrete types
at runtime. The `T` is resolved at compile time.

### LLVM Backend

No change — all types are i64 at the LLVM level.

---

## Phase 1: `format_expr` Fix

Add missing match arms at `src/proof_engine.rs:1220`:
- `Expr::Projection { source, target }`
- `Expr::FieldAccess { source, field }`
- `Expr::Term`
- `Expr::ListIndex { source, index }`
- `Expr::OwnedRef(name)`
- `Expr::PriorState(name)`
- `Expr::Sub(..)`, `Expr::Shr(..)`, `Expr::Shl(..)`
- `Expr::BitAnd`, `Expr::BitOr`, `Expr::BitXor`, `Expr::BitNot`

These are the contract expression types the proof engine encounters
but cannot display, causing every P008 error to show `<expr>`.

---

## Phase 2: Byte Type Definition

New file `lib/std/types.bv`:

```briev
type Byte : Bits @/0..7 {
    IsZero = self == 0;
    IsOne  = self == 1;
};
```

Update `src/typechecker.rs` `is_cast_valid()` to add Byte ↔ Int
cast support so that `as Ptr<Byte>` and `as Byte` compile.

---

## Phase 3: Rename Inops to Intrinsic Convention

`lib/std/skiplist.bv` + `lib/std/core/skiplist.bv`:
- `_sl_insert` → `sl_insert`
- `_sl_remove` → `sl_remove`
- `sl_remove_fallback` (no underscore)
- TypeDef bindings update accordingly

`lib/std/core/skiplist.bv` — same changes.

---

## Phase 4: Contract Cleanup

| File | Change |
|------|--------|
| `lib/std/skiplist.bv` | Remove trivial `[true]` from defns; use `[[term .#Size > list .#Size]` for inop post; `[i < list .#Size][i == list .#Size]` for txn convergence |
| `lib/std/atomic.bv` | `[ptr as Int >= 0]]` for pre-only contracts |
| `lib/std/state.bv` | `[[term > 0]` for post-only |
| `examples/inop-uart-mmap.bv` | `[in_range(...)]]` syntax; fix txn convergence |
| `examples/inop-isr-table.bv` | Use `Byte` type; fix inop contract |
| `examples/inop-ring-buffer.bv` | Remove `[true]` from defns |

---

## Phase 5: Verify

```bash
cargo test --lib
cargo run --bin briev-compiler -- check lib/std/atomic.bv
cargo run --bin briev-compiler -- check lib/std/skiplist.bv
cargo run --bin briev-compiler -- check examples/inop-*.bv
```

No flags. No `--skip-proof`. Every inop and txn has a verified contract.
