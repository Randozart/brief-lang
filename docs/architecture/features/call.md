# CallExpr — Function Calls

**Date:** 2026-06-09  
**Phase:** 1.3 (struct), 9.4 (ExprEval implementation)  
**Status:** ExprEval implemented; typecheck stub, codegen delegates to old Expr::Call

## Syntax

```brief
fn_name(arg1, arg2, ...)
```

`Call` is the only expression-level dispatch point in Brief. It resolves
the target function through a priority-ordered chain:

1. User definitions (`defn`) → `Interpreter::call_defn()`
2. Callable transactions (`txn` with convergence) → `Interpreter::call_txn()`
3. Dynamically linked FFI (`frgn from "lib.so"`) → `FrgnRegistry::call()`
4. State-registered defn aliases (`let f = defn foo; f()`)
5. Enum variant constructors → `Value::Enum` with field values
6. FFI registry (orchestrator with memory layouts) → `Orchestrator::call()`
7. Raw FFI functions → direct call with result marshaling

## Typechecking

Currently a stub — returns `Type::Int`. Full typechecking would:
- Resolve the function's signature
- Validate argument types against parameter types
- Return the declared return type

## Evaluation

The `CallExpr::evaluate()` method (`features/call.rs`) replicates the
dispatch chain from the interpreter's old `Expr::Call(name, args)` arm.
It accesses interpreter internals via `pub` fields and `pub(crate)` methods
(`call_defn`, `call_txn`, `handle_ffi_result`).

## Codegen

LLVM backend delegates to the old `Expr::Call` emit path:
```rust
ctx.emit_expr(out, &Expr::Call(self.name.clone(), self.args.clone()), "")
```

VHDL and Webstack are stubs returning `'0'` / `JsValue::undefined`.
