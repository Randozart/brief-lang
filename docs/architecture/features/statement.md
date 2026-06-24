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
  alka.rs             — Statement::Alka(AlkaBlock) — **PERMANENTLY ABANDONED**
  on_exit.rs          — Statement::OnExit { body, span } — **PERMANENTLY ABANDONED**
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

## `foreach(item in list) { body }` — Bounded Iteration

**Date:** 2026-06-15  
**Status:** Implemented (interpreter, LLVM backend with SIMD hint, parser; other backends stubs)

`foreach` is a statement-level bounded loop. It iterates over a `List<T>`,
binding `item: T` in each iteration. Unlike `rct txn` loops, `foreach`
does NOT require a convergence contract — termination is structural (the
list is finite).

### Syntax

```
foreach (item in list) { body };
```

Only valid inside `defn`/`txn`/`rct txn` bodies. Top-level `foreach`
produces a parse error.

### Desugaring

Not desugared — `foreach` is a first-class statement. The interpreter
implements it directly as:

```rust
Statement::Foreach { item, list, body } => {
    let list_val = self.eval_expr(list)?;
    if let Value::List(items) = list_val {
        for elem in items {
            self.state.insert(item.clone(), elem);
            for stmt in body {
                self.exec_stmt(stmt)?;
            }
        }
    }
}
```

### Backend coverage

| Backend | Status |
|---------|--------|
| Interpreter  | ✅ Direct implementation |
| LLVM         | ✅ Real loop IR (phi-less alloca-based index) + `!llvm.loop.vectorize.enable` metadata |
| Webstack     | ⚠️ Comment stub |
| C / COBOL / Verilog / VHDL / Wasm / aarch64 / x86_64 | ⚠️ Zero-fix dead backends |

### LLVM IR (2026-06-15)

The LLVM backend emits a counted loop using an alloca-based index variable
(avoids phi-block naming issues that plagued the SSA path):

```llvm
; list_val is i64 (ptrtoint of the 2-slot-header buffer)
; Slot 0: data_ptr (i64), Slot 1: length (i64), Slot 2+: elements
%fe_hp_N = inttoptr i64 %list_val to i64*
%fe_dp_gep_N = getelementptr i64, i64* %fe_hp_N, i64 0
%fe_dp_N = load i64, i64* %fe_dp_gep_N
%fe_ep_N = inttoptr i64 %fe_dp_N to i64*
%fe_len_gep_N = getelementptr i64, i64* %fe_hp_N, i64 1
%fe_len_N = load i64, i64* %fe_len_gep_N
%fe_idx_slot_N = alloca i64
store i64 0, i64* %fe_idx_slot_N
br label %fe_hdr_N

fe_hdr_N:
%fe_cur_N = load i64, i64* %fe_idx_slot_N
%fe_cmp_N = icmp slt i64 %fe_cur_N, %fe_len_N
br i1 %fe_cmp_N, label %fe_body_N, label %fe_done_N

fe_body_N:
%fe_elem_gep_N = getelementptr i64, i64* %fe_ep_N, i64 %fe_cur_N
%fe_elem_N = load i64, i64* %fe_elem_gep_N
; element bound to 'item' in let_bindings, body emitted here
%fe_next_N = add i64 %fe_cur_N, 1
store i64 %fe_next_N, i64* %fe_idx_slot_N
br label %fe_hdr_N, !llvm.loop !M

; LLVM Loop Vectorizer metadata — LLVM only vectorizes when the
; body has no cross-iteration dependencies.
!M = !{!M, !N}
!N = !{!"llvm.loop.vectorize.enable", i1 true}

fe_done_N:
```

### Feature file (2026-06-15)

The Foreach implementation lives in `src/features/stmt/foreach.rs` as
a `ForeachStmt` struct with trait impls (`StmtTypecheck`, `StmtEval`,
`StmtCodegenLLVM`, `StmtCodegenWebstack`), following the `sync_block.rs`
migration pattern. The central `emit_stmt.rs` delegates to the feature
module rather than containing the loop IR inline.

### Comparison to manual iteration

**Before** (manual `txn` convergence loop — 5 lines of scaffolding):
```
txn filter_fluff(tokens, result, i) [i < tokens:>Size][i == tokens:>Size] -> List<String> {
    [not_fluff(tokens[i])] { &result <- tokens[i]; };
    &i = i + 1;
    term result;
};
```

**After** (`foreach` — 1 line of scaffolding):
```
foreach (tok in tokens) {
    [not_fluff(tok)] { &result <- tok; };
};
```

The contract guarantee is preserved — termination is structural (list is
finite) rather than proven by `check_convergence`, but the same safety
property (no infinite loop) holds.

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
