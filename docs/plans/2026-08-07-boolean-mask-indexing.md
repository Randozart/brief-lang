# Boolean Mask Indexing — `data[mask]`

**Date:** 2026-08-07 · **Phase:** 7 (§16.5 remaining) · **Status:** Shipped
## Goal

`data[mask]` (SPEC §16.5 "Boolean masks use ordinary mask indexing") selects the
elements of `data` at the positions where the Boolean mask is true, in order.
This slice ships the Data/Bits case end-to-end (interpreter + typechecker +
codegen). General containers (`Int[N][mask]`, heap `List[mask]`) are a
follow-up; the codegen hard-errors on them (no silent wrongness).

Scope decision: **mask indexing on `Data` (the length-prefixed byte buffer)
and i64-slot typed vectors (`Int[N]`/`Bool[N]`)**. The byte case ships first;
the typed case (later in this session) produces a heap `List<T>` of the
selected elements via `briev_mask_select64`. General containers (`List[mask]`,
Float vectors — the latter scalarize in the backend with no contiguous
array) are rejected consistently in both.

## Semantics

- `data[mask]` where `data: Data`, `mask: Bool[N]` → a new `Data` of the bytes
  `data[i]` for each `i` with `mask[i] == true`, in ascending `i` order.
- Result length = popcount(mask). `N > data.len` → error (mask out of bounds).
- `N < data.len` → only the first `N` elements are considered (mask governs).
- The mask is a `Bool[N]` vector (a `[true, false, …]` list / a `Bool[N]`
  state field). Interpreter representation: a `Product` of `Atom(Bool)`.
- A chained `data[start:stop][mask]` is two ordinary ops — the slice produces
  a Data, then the mask applies (no special chaining code).

## Interpreter — `src/interpreter/eval.rs` `eval_index` (line 274)

When the index value is a Bool-vector (a `Product` whose fields are all
`Atom(AtomKind::Bool)`), do the masked select instead of the scalar-int path:

- `Value::Bits(bytes)` + Bool mask → new `Value::bits(selected)`.
- mask longer than the source → `RuntimeError` (out of bounds).
- any other source value → `TypeError` ("mask indexing is supported on byte
  data").
- the existing scalar-int index path is untouched (additive arm).

## Typechecker — `src/typechecker/mod.rs` `Expr::Index` (line 751)

- When the index type is a Bool vector (`Type::Vector(Bool, _)`), the result
  type is the obj's container kind:
  - `Data`/`String`/`Bits` → `Data` (byte-buffer result).
  - otherwise → keep the existing element-type resolution (mask indexing on
    non-byte containers is rejected at codegen with a hard error).
- No other behavior change.

## Codegen — `src/backend/llvm/emit_expr.rs` `Expr::Index` (line 630)

New arm: when the index is a Bool-vector register, emit a masked gather:

1. **Compile-time constant mask + constant source** → fold to a constant Data
   (count trues at compile time, emit the `[len][bytes]` constant). Handled by
   the existing constant-folding path if the operand is a literal.
2. **Runtime Data source + constant or runtime mask** → emit a runtime gather
   into a fresh `[len][bytes]` buffer:
   - evaluate the data ptr + length; evaluate the mask (Bool array).
   - count trues → `new_len`; `malloc(8 + new_len)`; store `new_len` header.
   - loop over `i`; if `mask[i]` true, copy `data[i]` into the buffer.
   - `ptrtoint` the buffer → the Data value (i64 handle, matching how Data
     literals are represented).
3. Non-byte containers (`Vector<Int>`, heap `List`, …) → hard compile error:
   "mask indexing is supported on Data; `<container>` is not yet supported".
4. Scalar index + slice paths untouched (additive).

## Tests

- **Interpreter**: `Bits` mask select (mixed true/false), all-true, all-false,
  mask-longer-than-source error, non-Bits source error. Follows the
  `test_match_range_*` style in `eval.rs`.
- **Typechecker**: `Data[mask]` → `Data`; `Int[N][mask]` → hard error at
  codegen (typechecks, codegen rejects).
- **Codegen**: constant mask → constant Data in the IR; runtime Data mask →
  gather loop present. Mirrors `test_match_emits_arm_chain_and_phi` style.
- **End-to-end**: `scratch`/`.smoke` style `.bv` — `data[mask]` compiled +
  run, output verified against the interpreter.
- **Benchmarks**: full suite green (mask indexing is additive; no existing
  path modified).

## Docs

- `docs/plans/2026-08-05-spec-implementation-status.md` §16 row: mark boolean
  mask indexing (Data case) shipped; note general containers + iterable
  ranges + named selectors + const generics remain.
- `spec/SPEC.md` is already normative (no change needed).

## Boundaries / follow-ups

- General container masks (`Int[N][mask]`, heap `List[mask]`) — runtime typed
  gather — follow-up slice.
- Iterable ranges + `foreach` (needs interpreter foreach, currently a stub) —
  separate slice.
- Named selectors + multi-dim slices — separate slice (needs `Slice` dims).
- Const generics — separate slice.
