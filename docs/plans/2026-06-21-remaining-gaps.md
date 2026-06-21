# Remaining Gaps — Survey Results (2026-06-21)

Systematic survey of all `todo!()`, `unimplemented!()`, stub returns,
placeholder strings, and degraded code paths across the compiler.

## Critical (blocks functionality)

### #1: Webstack backend — transaction body emits placeholder ✅ FIXED (2026-06-21)
- `src/backend/webstack.rs:271-277` — Transaction body generates `true` placeholder → replaced with real `statement_to_rust()` / `expr_to_rust()` codegen
- TS path: intrinsic handler expanded from 4 to 25+ variants (Math.*, String(), Date.now(), etc.)
- All Statement types emit real TS code (no more `// statement omitted`)
- ARM bare-metal path emits native Rust code for all Statement/Expr variants

### #2: CIRCT backend — Expr::Call returns None
- `src/backend/circt.rs:349` — `Expr::Call(name, _args) => None`
- Most expression types return `None` (unsupported)

## High (incomplete features)

### #3: Pattern B AssignmentStmt
- `src/features/stmt/assignment.rs:28-34` — All 4 trait impls are empty stubs
- `evaluate` returns `Err("not yet dispatched")`, `emit_llvm` is empty body

### #4: Macro expansion
- `src/features/macros/macro_.rs:5-11` — Parsing done, expansion returns error
- Phase 1b AST/parser implemented but actual expansion stubbed

### #5: Crypto/HTTP FFI functions
- `src/ffi/registry.rs:1118-1252` — md5, sha1, sha256, sha512, uuid_v4, http::get, http::post
- All return `Value::String("not_implemented")` placeholder strings

### #6: `bytes` projection in interpreter
- `src/interpreter.rs:1759` — Not implemented for `Value::Instance` / `Value::Float`

### #7: GPU intrinsics in interpreter
- `src/interpreter.rs:3752-3764` — `get_global_id#`, `get_local_id#`, etc. return 0/1 stubs

## Medium (small, self-contained)

### #8: atomic_store/fence/halt LLVM stubs
- `src/backend/llvm/emit_expr.rs:1408,1440,2332,2398`
- Void-returning intrinsics emit `add i64 0, 0` as dummy value

### #9: Exit expression LLVM stubs
- `src/backend/llvm/loop_engine.rs:60,68,71,144`
- Unknown fields/identifiers in exit expressions return 0

## Principle
- Use intrinsics (`#`-syntax, `Intrinsic` enum) instead of `frgn` declarations
  where possible. Check `src/ast.rs` for existing `Intrinsic` variants before
  adding new FFI paths.
