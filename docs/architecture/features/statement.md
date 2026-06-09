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
