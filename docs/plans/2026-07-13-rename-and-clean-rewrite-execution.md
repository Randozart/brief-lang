# Rename ast_new → ast + Clean Rewrite Execution

## Context

We renamed `src/ast_new/` → `src/ast/` and replaced all `crate::ast_new::` references
with `crate::ast::` across the codebase. 2948 errors remain — all from old code that
matches on legacy `Expr` variants (`Expr::Add`, `Expr::Sub`, etc.) and legacy
`Statement` variants (`Statement::Assignment { lhs, expr, .. }`, etc.) that no longer
exist in the clean new AST.

## Goal

Zero compilation errors. Every file rewritten to use the new AST types with max 2
levels of indentation.

## The Single Mechanical Transformation

Every file that walks the AST contains functions like `expr_refs_name`,
`collect_expr_identifiers`, `rewrite_cell_identifiers`, etc. These all share the
same pattern:

```rust
// OLD: 17 arms for binary ops, 3 for unary, plus IntrinsicCall, Block(stmts, last), etc.
match expr {
    Expr::Add(l, r) | Expr::Sub(l, r) | ... | Expr::Shr(l, r) => { f(l); f(r); }
    Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => { f(e); }
    Expr::IntrinsicCall { intrinsic, args } => { for a in args { f(a); } }
    Expr::Block(stmts, last) => { for s in stmts { .. }; f(last); }
    Expr::StructInstance(_, fields) => { for (_, e) in fields { f(e); } }
    Expr::ListLiteral(items) => { for item in items { f(item); } }
    // ... 30+ more arms
}

// NEW: unified variants, far fewer arms
match expr {
    Expr::BinaryOp(_, l, r) => { f(l); f(r); }
    Expr::UnaryOp(_, e) => { f(e); }
    Expr::Call(_, args) => { for a in args { f(a); } }
    Expr::Block(stmts) => { for s in stmts { process_stmt(s); } }
    Expr::Tuple(items) | Expr::List(items) => { for item in items { f(item); } }
    Expr::If(cond, then, else_opt) => {
        f(cond); f(then);
        if let Some(el) = else_opt { f(el); }
    }
    Expr::Field(..) | Expr::Index(..) => { /* recurse into children */ }
    Expr::Lambda(params, body) => f(body),
    Expr::Cast(e, _) | Expr::IsType(e, _) => f(e),
    Expr::Within(body, fallback) => { f(body); f(fallback); }
    // Leaves
    Expr::Decimal(_) | Expr::Bool(_) | Expr::Float(_) | Expr::Quoted(_) | Expr::Identifier(_) => {}
}
// Maximum 2 nesting levels. Use ? and early returns.
```

## Statement matching

```rust
// OLD
Statement::Assignment { lhs, expr, timeout, modifiers } => { ... }
Statement::Let { name, ty, expr, address, address_expr, bit_range, constraint, is_override, modifiers } => { ... }
Statement::Guarded { condition, statements, metadata } => { ... }
Statement::Term { values, swan_song, modifiers } => { ... }

// NEW
Statement::Assign(lhs, expr) => { ... }
Statement::Let { name, ty, expr, modifiers } => { ... }
Statement::Guarded(condition, statements) => { ... }
Statement::Term(expr) => { ... }
```

## Steps

### Step 0: Pre-execution
1. Write this plan
2. Git commit current state
3. `git pull` to get user's changes from elsewhere

### Step 1: Revert legacy-variant hack from `ast/expr.rs`
Remove lines 115-172 (the `// Legacy backward-compatible variants` block inside the
`Expr` enum). These reference undeclared types (`ProjectionTarget`, `PatternField`,
`ArrowDir`, `SubtypeOp`, `SharedMem`, `PipeChain`, `Statement` circular ref) and
were a shortcut that should never have been added.

File: `src/ast/expr.rs`

### Step 2: Rewrite `backend/llvm/` (1681 errors)

8 files, ~17,000 lines total.

| File | Lines | Errors | Approach |
|------|-------|--------|----------|
| `loop_engine.rs` | 4398 | 373 | Split into 3 files (loop_body, schedule, phi). Combine binary ops → 1 arm. |
| `mod.rs` | 3673 | 282 | Split: LlvmBackend struct in mod.rs; `generate()` extracted. |
| `helpers.rs` | 1920 | 237 | 50+ match arms → 8. Keep emit_cast_convert, emit_inline_concat. |
| `gpu.rs` | 1912 | 176 | Fix Expr matching same pattern. |
| `emit_toplevel.rs` | 2317 | 138 | Fix Statement matching. |
| `hazard.rs` | 710 | 67 | expr_refs_name + modifies_name — combine binary/unary ops. |
| `dispatch.rs` | ~500 | 36 | Dispatch selection. |
| `reorder.rs` | ~500 | 37 | Instruction reorder. |

### Step 3: Rewrite `analysis/` (863 errors)

20 files, ~10,000 lines total.

| File | Lines | Errors | Notes |
|------|-------|--------|-------|
| `region.rs` | 2626 | 274 | ~10 collect_* functions, each matching 30+ old variants |
| `transition_graph.rs` | 2023 | 263 | Reactor transition graph + fusion detection |
| `watchdog.rs` | 764 | 52 | Watchdog analysis |
| `dataflow.rs` | 496 | 49 | Dataflow analysis |
| `dependency_graph.rs` | 714 | 48 | Dependency graph building |
| `gpu_cost.rs` | ~400 | 47 | GPU cost model |
| `provenance.rs` | ~500 | 34 | Provenance tracking |
| Rest | 300-400 each | ~100 | call_graph, range, entry_point, etc. |

### Step 4: Rewrite other backends (296 errors)

- `backend/mod.rs` (999 lines, 141 errors) — validate_hashtags, detect_fusable_pairs, etc.
- `webstack.rs` (1963 lines, 108 errors) — JS/WASM codegen
- `circt.rs` (1208 lines, 47 errors) — MLIR/hardware codegen

### Step 5: Rewrite misc files (~400 errors)

- `interpreter/` (77 errors) — eval, casts, cells
- `ffi/` (127 errors) — registry, loader, types, etc.
- `import_resolver.rs` (103 errors), `annotator.rs` (43), `symbolic.rs` (37)
- `lsp.rs` (36), `fuzz_checker/` (48), `reactor.rs` (21), etc.

### Step 6: Dead backends (~50 errors)

- verilog, vhdl, c, rust, cobol, x86_64, aarch64, wasm, tcl_generator
- MINIMAL: `#[allow(unused)]`, `_ => {}`, `todo!()` with comment `// dead backend`

### Step 7: Verify

```
cargo check        # zero errors
cargo test --lib   # all tests pass
```
