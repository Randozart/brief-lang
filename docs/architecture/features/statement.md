# Statement Features — Pattern B Architecture

**Date:** 2026-06-09  
**Phase:** 2  
**Status:** Feature files exist with stub trait impls; dispatch not yet migrated

## Design

13 Statement variants are extracted into individual feature files under
`src/features/stmt/`. Each file contains a Pattern B struct definition and
5 trait implementations (StmtTypecheck, StmtEval, StmtCodegenLLVM,
StmtCodegenVHDL, StmtCodegenWebstack).

## File Layout

```
src/features/stmt/
  mod.rs              — Module declarations for all 13 features
  assignment.rs       — Statement::Assignment { lhs, expr, timeout, modifiers }
  let_binding.rs      — Statement::Let { name, ty, expr, address, bit_range, ... }
  guarded.rs          — Statement::Guarded { condition, statements }
  term.rs             — Statement::Term { values, swan_song, modifiers }
                        Statement::TermBang { values, swan_song, modifiers }
  escape.rs           — Statement::Escape(Option<Expr>)
  expression.rs       — Statement::Expression(Expr)
  unification.rs      — Statement::Unification { name, variant, fields, expr }
  inline_asm.rs       — Statement::InlineAsm { asm_string, clobbers, span }
  local_trigger.rs    — Statement::LocalTrigger { name, ty, expr, span }
  alka.rs             — Statement::Alka(AlkaBlock)
  on_exit.rs          — Statement::OnExit { body, span }
  sync_block.rs       — Statement::SyncBlock { body }
```

## Traits

Defined in `src/features/traits.rs`:

| Trait | Method | Pass |
|-------|--------|------|
| `StmtTypecheck` | `typecheck(&self, ctx: &mut TypeChecker, dispatch: &StmtDispatch)` | Typechecker |
| `StmtEval` | `evaluate(&self, ctx: &mut Interpreter, dispatch: &StmtDispatch)` | Interpreter |
| `StmtCodegenLLVM` | `emit_llvm(&self, ctx, out, dispatch, indent)` | LLVM backend |
| `StmtCodegenVHDL` | `emit_vhdl(&self, ctx, out, dispatch, indent)` | VHDL backend |
| `StmtCodegenWebstack` | `emit_js(&self, ctx, out, dispatch)` | Webstack backend |

`StmtDispatch` is the handle for recursive sub-statement dispatch.

## Tuple Destructuring Assignment `&(a, b) = expr`

**Date:** 2026-06-11  
**Status:** Implemented directly in parser + interpreter + typechecker

The `&(a, b) = expr;` syntax destructures a `Value::Tuple` or `Value::List`
into named variables, binding each element to the corresponding name. This
is the mutable reassignment form of `let (a, b) = expr;`.

### Parsing

In `parser.rs`, the `&` unary prefix handler now checks for `LParen` after
the `&`. If found, it parses a comma-separated list of identifiers followed
by `)`, and produces `Expr::TupleDestructure(names, Box::new(Expr::Term))`.
The `Expr::Term` inner expression is a dummy — it is never evaluated in
the assignment context (the RHS comes from the `Statement::Assignment`'s
`expr` field).

### Interpreter

In `interpreter.rs` `exec_stmt`, the `Statement::Assignment` LHS handler
includes an `Expr::TupleDestructure(names, _)` arm that:
1. Evaluates the RHS expression to a `Value`
2. Matches on `Value::Tuple(items)` or `Value::List(items)`
3. Inserts each element into `self.state` by the corresponding name

### Typechecker

In `typechecker.rs` `check_statement`, the `Statement::Assignment` handler
has a special `Expr::TupleDestructure` branch that:
1. Infers the RHS type
2. Expects `Type::Tuple(elem_types)` — emits `TypeMismatch` otherwise
3. For each name, looks up the declared variable type and checks
   compatibility with the corresponding tuple element type

### Backend coverage

| Backend | Status |
|---------|--------|
| Interpreter  | ✅ Full implementation |
| Typechecker  | ✅ Full implementation |
| LLVM         | ⚠️ Comment stub (tuple codegen incomplete) |
| Webstack     | ✅ Falls through to `_ =>` wildcard — safe no-op |
| VHDL         | ✅ No `Statement::Assignment` match — safe no-op |
| Rust         | ✅ Falls through to `_ => return` — safe no-op |

### Limitations

- Only handles top-level destructuring (no nested `&(a, (b, c)) = expr`)
- LLVM backend emits only a comment — actual tuple codegen is a known gap

## Migration Status

All feature files are stubs — the actual dispatch still uses the old
Statement enum variants directly in the pass files (`exec_stmt` in
interpreter.rs, `emit_stmt` in llvm.rs, etc.). The dual-path transition
(adding new Pattern B Statement variants alongside old ones) is deferred
to Phase 4.

## Kani

All Kani harnesses for Statement features are gated behind
`#[cfg(all(kani, feature = "kani_full"))]` because the struct definitions
use `Vec`, `Box`, and `Option` types that violate the fast-group
no-heap-allocation rule.
