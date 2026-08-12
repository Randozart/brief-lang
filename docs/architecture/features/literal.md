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

## LLVM Backend Boxing Convention (2026-06-16)

The LLVM backend's `emit_llvm` for `LiteralExpr` returns values in the
**boxed i64 format** — matching the system-wide convention that all SSA
values are `i64` regardless of Briev-level type:

| Literal Variant | LLVM Emission | Register Type | TypedRegister.ty |
|----------------|---------------|---------------|------------------|
| `LiteralExpr::Char(c)` | `add i32 0, c` → `zext i32 to i64` | `i64` | `Type::Int` |
| `LiteralExpr::String(s)` | `getelementptr @str.N` → `ptrtoint to i64` | `i64` | `Type::Int` |
| `LiteralExpr::Bool(b)` | `and i1 true, true` / `xor i1 true, true` | `i1` | `Type::Bool` |
| `LiteralExpr::Integer(n)` | `add i64 0, n` | `i64` | `Type::Int` |
| `LiteralExpr::Float(f)` | `bitcast float f to i32` → `zext i32 to i64` | `i64` | `Type::Float` |

**Important**: `LiteralExpr::Char` and `LiteralExpr::String` return
`Type::Int` (not `Type::Char`/`Type::String`) because the value is
already boxed to `i64`. This prevents `adapt_to_i64` from generating
a bogus `ptrtoint`/`zext` on an already-boxed value.

`LiteralExpr::Bool` returns `Type::Bool` with a native `i1` register.
This is correct — `adapt_to_i64` can zext `i1` to `i64` on demand,
and other code paths (comparisons, `as_bool_reg`) consume `i1` directly.

## Verification

- 713 tests pass (`cargo test --lib`)
- 0 new Praetor diagnostics across all 16 touched files
- 14 Kani harnesses prove dual-path equivalence (fast group, 2.5s)
- 110 Kani harnesses total (full group with `--features kani_full`)
- `cargo build` compiles without errors (17 pre-existing warnings)
