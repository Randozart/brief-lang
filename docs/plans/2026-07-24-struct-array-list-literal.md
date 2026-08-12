# 2026-07-24: Struct Array List Literal Codegen

## Problem

When a list literal `[PyMethodDef{...}, ...]` has all elements of the same
C-compatible struct type, `emit_heap_seq` emits a heap-allocated Briev list
(malloc + header + element slots as `i64` values). C code expects a contiguous
`PyMethodDef*` array. The GLUE bridge generator emits `let methods = [PyMethodDef{...}]`
but the generated code cannot be passed to C functions expecting `PyMethodDef*`.

## Solution

Detect when all elements of a list literal are `Expr::StructLiteral` with the
same known struct type. In that case, emit a contiguous stack-allocated array
instead of a heap-allocated Briev list. The `&var` on such a list naturally
produces a `ptrtoint ptr %alloca to i64` — exactly what C expects.

## Design

### Detection: `detect_struct_list`

Check three conditions before taking the struct-array path:
1. All elements are `Expr::StructLiteral`
2. All elements share the same `type_name`
3. That `type_name` is registered in `self.ctx.struct_types`

Returns `Option<String>` with the common type name.

### Emission: `emit_struct_array`

```
alloca i8, i64 <N * element_size>
for each element at position i:
  for each field:
    evaluate field expression
    GEP to (i * elem_size) + field_offset
    store field value
ptrtoint ptr %alloca to i64
```

Records the alloca in `struct_literal_allocas` keyed by the result register,
so the let-binding handler transfers it to the variable name, and `&var`
retrieves the stack address.

### Integration

Insert the struct-array check BEFORE the existing SVO and heap_seq paths in
`Expr::List` handler:

```rust
Expr::List(exprs) => {
    if let Some(elem_ty) = self.detect_struct_list(exprs) {
        return self.emit_struct_array(out, v, exprs, &elem_ty, indent);
    }
    if self.feature_svo && exprs.len() <= 3 {
        return self.emit_svo_list(out, v, exprs, indent);
    }
    self.emit_heap_seq(out, v, exprs, indent)
}
```

## Files Changed

1. `src/backend/llvm/emit_expr.rs`: Add `detect_struct_list` and
   `emit_struct_array` methods. Wire into `Expr::List`.

2. `src/backend/llvm/tests.rs`: Add `test_struct_array_list_literal` test.

## Tests

- **test_struct_array_list_literal**: Two-element struct array, verify
  contiguous alloca (size = 2 * element_size), all elements stored at correct
  offsets, `ptrtoint` emitted, no `malloc` call present.
- **test_struct_array_addr_of**: Struct array via `let` + `&var` passed
  to `Ptr`-typed frgn param. Verify `inttoptr` is emitted for the param.
