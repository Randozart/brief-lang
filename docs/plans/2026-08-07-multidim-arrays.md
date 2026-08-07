# Multi-dimensional arrays — the `Matrix<T, Rows, Cols>` enabler

**Date:** 2026-08-07 · **Phase:** 7 (§16.6 const generics) · **Status:** Shipped (multi-dim arrays; Matrix<T,Rows,Cols> obj-member const-param substitution is a follow-up)

## Goal

SPEC §16.6: `Matrix<T, Rows, Cols>` — compile-time dimension params. The
const-generic VALUE machinery already exists (`Stack<T, N>` works: `N` is a
dimension and a bound). The missing piece is **multi-dimensional arrays**:
`data: T[Rows][Cols]` fails to parse (`expected identifier, found '['`), and
the codegen only handles 1-dim vectors.

## Scope (3 slices)

### A. Multi-dim TYPE parse + layout
- **Parser** (`types.rs`): `T[M][N]` → `Type::Vector(inner, [M, N])` — loop
  the `[...]` suffix instead of returning after the first.
- **push_field_type** (`mod.rs:1138`): the Vector arm's `dims.len() == 1`
  gate → build the nested LLVM type `[M x [N x T]]` recursively.
- `vector_element_count` already multiplies dims (`dims.iter().product()`).

### B. Multi-dim indexing (codegen + interpreter + typechecker)
- `data[i][j]` = `Index(Index(Identifier(data), i), j)`.
- **Codegen**: the INNER `data[i]` on a multi-dim field returns a ROW VIEW —
  a GEP into `%State` (a ptr to `[N x T]`) typed `Vector(inner, [N])`. The
  OUTER `[j]` on a Vector-typed row-view GEPs `[N x T]` at `j` + loads `T`.
  (The existing vector-field paths require a field identifier — a new arm
  handles a Vector-typed register that IS a row ptr.)
- **Interpreter**: a nested `Product` (list-of-lists) index already works via
  `eval_index`; the row is a sub-Product and `[j]` indexes it. Verify with a
  test (the interpreter reference).
- **Typechecker**: `Index(row, j)` where row: `Vector(inner, [N])` types to
  `inner` (the existing element-type resolution already handles a Vector obj
  — confirm).

### C. `Matrix<T, Rows, Cols>` end-to-end
- An `obj Matrix<T, Rows, Cols> { data: T[Rows][Cols]; ... }` with A+B in
  place resolves the const params (like `Stack<Int, 256>`) and indexes
  `data[r][c]`.

## Tests
- Parser: `T[M][N]` → `Vector(inner, [M, N])` (AST shape).
- Codegen: a 2-dim field lays out as `[M x [N x T]]`; `data[i][j]` emits the
  nested GEP.
- End-to-end: `Matrix<Int, 3, 4>`, `m.set(1, 2, 42)`, read `m.data[1][2]`.
- Interpreter: nested product index.

## Boundaries
- More than 2 dims (recursive — follows the same pattern, not gated).
- `Matrix<T, Rows, Cols>` bounds proofs during specialization (contract
  `[row < Rows]` already works like `Stack`'s `[len < N]`).

## Docs
- `docs/plans/2026-08-05-spec-implementation-status.md` §16: mark const
  generics (multi-dim arrays) shipped.

## Update 2026-08-07 (evening): const-param member substitution

`resolve_field_type` (emit_expr.rs) now substitutes an Applied generic obj's
member type against the instance's concrete args — `data: T[Rows][Cols]` for
`Matrix<Int, 3, 4>` resolves to `Vector(Int, [Anonymous(3), Anonymous(4)])`
(the same map `ensure_mono` uses for struct_types). Verified: 1654 tests, 75
MATCH + 1 PASS, no regression.

The FULL `Matrix<T, Rows, Cols>` end-to-end remains blocked by a PRE-EXISTING
generic-obj MEMBER-ARRAY access gap (verified at the baseline `46f4f741`): a
direct `obj.member[i]` read generates invalid IR there, and a direct member
txn call (`m.set(...)`) segfaults. Member access via the `<-` ops (the Stack's
self-slot path) works. A dedicated member-array-access fix (struct member-array
layout + Field read + call dispatch) is the remaining Phase 7 work.
