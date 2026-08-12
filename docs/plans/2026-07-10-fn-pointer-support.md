# Function Pointers: `Ptr<fn(...)>` Support

## Goal

Make `&my_func` produce `Ptr<Fn(Tuple(params), ret)>` (a proper function pointer
type) instead of `Ptr<Custom("my_func")>`, enabling:

```briev
let fp = &my_func;     // fp: Ptr<Fn((Int), Int)>
let r = (*fp)(42);     // dereference + indirect call
```

## Changes needed

### 1. Typechecker: resolve function identifiers (typechecker.rs:2072)

When `Expr::Identifier(name)` encounters an unknown name, check the function
definition tables before returning `Type::Custom(name)`:

```rust
let lookup = self.lookup_variable(name);
if let Some(ty) = lookup { ty }
else if self.definitions.contains_key(name) || self.transactions.contains_key(name) {
    // Known function → return Fn type
    let params = self.defn_params.get(name).cloned().unwrap_or_default();
    let ret = self.defn_return_types.get(name).cloned().unwrap_or(Type::Custom("Void".to_string()));
    Type::Applied("Fn".to_string(), vec![params, ret])
} else {
    Type::Custom(name.clone())
}
```

### 2. Codegen: function identifier values (identifier.rs:347)

When `emit_identifier` encounters a name in `defn_params`, emit `ptrtoint`:

```rust
} else if backend.ctx.defn_params.contains_key(name)
    || backend.ctx.defn_return_types.contains_key(name)
{
    writeln!(out, "{}{} = ptrtoint @{} to i64", indent, v, name).ok();
    let fn_ty = Type::Applied("Fn".to_string(), vec![
        backend.ctx.defn_params.get(name).cloned().unwrap_or_default(),
        backend.ctx.defn_return_types.get(name).cloned().unwrap_or(Type::Custom("Void".to_string())),
    ]);
    return TypedRegister { name: v.to_string(), ty: fn_ty };
}
```

### 3. Codegen: `emit_addr_of` for functions (identifier.rs:363)

When `expr` is an `Identifier` naming a function, return the label as a ptr:

```rust
Expr::Identifier(name) if backend.ctx.defn_params.contains_key(name) => {
    Ok(format!("%{}", name))  // function label as ptr register
}
```

### 4. Codegen: Deref of `Ptr<Fn(...)>` (rest.rs:1224)

Add match arm for `Applied("Fn", ...)` pointee type — load as `ptr`:

```rust
Type::Applied(name, _) if name == "Fn" => {
    ("ptr".to_string(), inner_ty)
}
```

### 5. Codegen: indirect call from `ptr` (call.rs:271-327)

`try_fn_ptr_call` currently expects an `i64` that it `inttoptr`s. When the
register type is already `ptr`, skip the `inttoptr`.

```rust
// Before: %fptr = inttoptr i64 %reg to ptr
// After:  %fptr is the register itself (already a ptr)
let fn_ptr = if fn_reg.ty is ptr-like { fn_reg.name } else { inttoptr i64 fn_reg.name to ptr };
```

### 6. `pointee_type` support for `LayoutPtr` (type_universe.rs)

Extend `pointee_type` to handle `LayoutPtr` in addition to `Ptr`/`PtrConst`

## Files changed

| File | What | Lines |
|------|------|-------|
| `src/typechecker.rs` | Resolve function names to `Fn` type | ~8 |
| `src/backend/llvm/expr/identifier.rs` | Emit `ptrtoint` for function identifiers + `emit_addr_of` for fn labels | ~25 |
| `src/backend/llvm/expr/rest.rs` | Add `Fn` match arm in Deref codegen | ~4 |
| `src/backend/llvm/expr/call.rs` | Handle `ptr`-typed fn pointers in `try_fn_ptr_call` | ~8 |
| `src/type_universe.rs` | Add `LayoutPtr` to `pointee_type` | ~4 |

## Verification

1. `cargo test --lib` — all tests pass
2. `let fp = &my_func; (*fp)(42)` compiles and runs correctly
3. `let fp = my_func; fp(42)` also works (identifier type resolves to `Fn(...)`)
