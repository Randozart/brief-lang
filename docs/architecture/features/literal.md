# LiteralExpr Feature (Phase 1.1)

**Date**: 2026-06-09  
**Commit**: (to be added after commit)

## Design

`LiteralExpr` is the first Pattern B feature module, migrating integer, float, string, char, bool, and term literal expressions from direct `Expr` enum variants into a separate struct with co-located trait implementations.

### Enum

```
LiteralExpr
├── Integer(i64)
├── Float(f64)
├── String(String)
├── Char(char)
├── Bool(bool)
└── Term
```

### Dual-Path Safety Net

The `Expr` enum retains both old and new variants:

```rust
pub enum Expr {
    Literal(Box<LiteralExpr>),  // NEW — Pattern B
    Integer(i64),               // OLD — kept as safety net
    Float(f64),                 // OLD
    String(String),             // OLD
    Char(char),                 // OLD
    Bool(bool),                 // OLD
    Term,                       // OLD
    // ...
}
```

All match arms handle both variants identically. Router arms for the new variant delegate to `LiteralExpr` trait impls. Old variants remain as fallback.

### Helper Methods on `Expr`

Five dual-path helper methods provide unified access regardless of variant encoding:

| Method | Returns | Handles Old | Handles New |
|--------|---------|-------------|-------------|
| `as_integer()` | `Option<i64>` | `Expr::Integer(n)` | `Expr::Literal(LiteralExpr::Integer(n))` |
| `as_bool()` | `Option<bool>` | `Expr::Bool(b)` | `Expr::Literal(LiteralExpr::Bool(b))` |
| `as_float()` | `Option<f64>` | `Expr::Float(f)` | `Expr::Literal(LiteralExpr::Float(f))` |
| `as_string()` | `Option<&String>` | `Expr::String(s)` | `Expr::Literal(LiteralExpr::String(s))` |
| `is_term()` | `bool` | `Expr::Term` | `Expr::Literal(LiteralExpr::Term)` |

### Trait Implementations

| Trait | Method | Backend |
|-------|--------|---------|
| `ExprTypecheck` | `typecheck(ctx, dispatch) -> Type` | TypeChecker |
| `ExprEval` | `evaluate(ctx, dispatch) -> Value` | Interpreter |
| `ExprCodegenLLVM` | `emit_llvm(ctx, out, dispatch) -> TypedRegister` | LLVM |
| `ExprCodegenVHDL` | `emit_vhdl(ctx, dispatch) -> String` | VHDL |
| `ExprCodegenWebstack` | `emit_js(ctx, dispatch) -> String` | Webstack |

### Display/Formatting

`LiteralExpr::format()` converts each variant to a display string, used by `format_expr()` in the annotator, proof_engine, and parser diagnostics.

### Kani Formal Verification

- **Fast group** (`cargo kani --lib`): 5 harnesses proving dual-path equivalence for `as_integer`, `as_bool`, `is_term` and fallback paths (None for non-matching types). Runs in ~2.5s.
- **Full group** (`cargo kani --lib --features kani_full`): Format/float/string tests that involve `Display`/`to_string()` internals.

## Router Arms

The following files received a single `Expr::Literal(lit)` route arm:

| File | Function | Dispatch |
|------|----------|----------|
| `interpreter.rs` | `eval_expr` | `lit.evaluate(self, &ExprDispatch)` |
| `typechecker.rs` | `infer_expression` | Direct destructure over `LiteralExpr` variants |
| `annotator.rs` | `format_expr` | Direct destructure |
| `symbolic.rs` | `eval_symbolic` | Direct destructure |
| `backend/vhdl.rs` | `expr_to_string` | `lit.emit_vhdl(self, &ExprDispatch)` |
| `backend/webstack.rs` | `expr_to_js_value` | `lit.emit_js(self, &ExprDispatch)` |
| `backend/llvm.rs` | `emit_expr` | `lit.emit_llvm(self, out, &ExprDispatch)` |
| `analysis/dataflow.rs` | `extract_ids_recursive` | Leaf catch-all |
| `analysis/transition_graph.rs` | `collect_identifiers` | Leaf catch-all |
| `proof_engine.rs` | 6 functions | Various destructure arms |

## Files Touched (16)

| File | Status | Changes |
|------|--------|---------|
| `src/features/literal.rs` | Created | LiteralExpr enum + 5 trait impls + format() |
| `src/features/traits.rs` | Modified | VHDL/Webstack traits changed to &self |
| `src/features/mod.rs` | Modified | `pub mod literal;` uncommented |
| `src/ast.rs` | Modified | `Expr::Literal(Box<LiteralExpr>)` variant + 5 helper methods |
| `src/interpreter.rs` | Modified | Router arm + imports |
| `src/annotator.rs` | Modified | Router arm + imports |
| `src/typechecker.rs` | Modified | Router arm + imports |
| `src/parser.rs` | Modified | `parse_primary`/`parse_match_expr` produce LiteralExpr; contract validation uses `as_bool()` |
| `src/proof_engine.rs` | Modified | 6 functions updated for dual-path; all 22 tests pass |
| `src/backend/vhdl.rs` | Modified | Router arm |
| `src/backend/webstack.rs` | Modified | Router arm |
| `src/backend/llvm.rs` | Modified | Router arm (was missing — added during Phase 1.1) |
| `src/analysis/dataflow.rs` | Modified | Leaf catch-all arm |
| `src/analysis/transition_graph.rs` | Modified | Leaf catch-all arm |
| `src/symbolic.rs` | Modified | Router arm + imports |
| `AGENTS.md` | Modified | Kani coverage requirement, Per-Commit Checklist |
| `Cargo.toml` | Modified | Added `kani_full` feature |
| `scripts/verify.sh` | Created | CI verification script |

## Verification

- 713 tests pass (`cargo test --lib`)
- 0 new Praetor diagnostics across all 16 touched files
- 14 Kani harnesses prove dual-path equivalence (fast group, 2.5s)
- 110 Kani harnesses total (full group with `--features kani_full`)
- `cargo build` compiles without errors (17 pre-existing warnings)
