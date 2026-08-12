# Function Pointers (Phase 5)

**Date:** 2026-07-03  
**Status:** Implemented (typechecker + LLVM backend)

## Design

Function pointers use the existing `.#Ptr` projection syntax rather than a
new `&f` address-of operator (which would conflict with Briev's mutable `&`
syntax).

### Getting a Function Pointer

```briev
let cmp_fn = my_compare .#Ptr;  // produces fn pointer
```

The `.#Ptr` projection on a function identifier emits `ptrtoint @fn_name to i64`
in the LLVM backend. The typechecker returns `Applied("Fn", vec![params, ret])`
for the projection result.

### Indirect Call

```briev
let result = cmp_fn(a, b);  // indirect call through fn pointer
```

`Expr::Call("cmp_fn", args)` is resolved as an indirect call when `cmp_fn`
is a local variable with `Applied("Fn", ...)` type. The LLVM backend emits:
```llvm
%fn_ptr = inttoptr i64 %cmp_fn_reg to ptr
%result = call i64 %fn_ptr(i64 %a, i64 %b)
```

### Type Representation

Function pointer types are `Type::Applied("Fn", vec![params_tuple, return_type])`,
where `params_tuple` is `Type::Tuple(vec![param1_type, param2_type, ...])`.

This is consistent with the existing parser output for `(Int) -> Bool` syntax.

### Implementation

| Component | File | Change |
|-----------|------|--------|
| Typechecker | `typechecker.rs` | `.#Ptr` on fn returns Applied("Fn", ...); `Expr::Call` on fn-ptr var validates args |
| LLVM projection | `projection.rs` | `ProjectionTarget::Ptr` on fn identifier emits `ptrtoint` |
| LLVM call | `call.rs` | `try_fn_ptr_call` emits indirect call through fn-ptr variable |
| Architecture | `fn-ptr.md` | This document |

### Flat Control Flow

The implementation follows the max-2-levels nesting rule:
- `try_fn_ptr_call` uses `?` and `else { return None; }` guard clauses
- The caller uses `if let Some(tr) = try_fn_ptr_call(...)` — 1 level
