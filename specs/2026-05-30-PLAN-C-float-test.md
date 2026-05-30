# Plan C: T11 — Local float binding test

> Created: 2026-05-30T14:15Z
> Status: Draft — ready for implementation
> Depends on: Nothing

## Problem

The B2 bug fix (Phase 0) ensures that:
1. `Expr::Float` registers its type in `register_types`
2. `is_float_expr` checks `let_bindings` → `register_types` → falls back to `field_index_map`

But there is **no test** that constructs a program with `Expr::Float` values and verifies the backend emits correct float LLVM IR (`fadd float`, `bitcast float`, etc.). This was deferred as T11 — "pending (AST construction complex)" — in the test plan.

## Goal

Add a backend unit test in `src/backend/llvm.rs` that creates a program with float state variables and float operations, generates LLVM IR, and verifies:
- `fadd float` is emitted (not `add i64`)
- `bitcast float` is emitted (type conversion)
- `float` type annotations appear in the IR

## Implementation

### File: `src/backend/llvm.rs`
### Location: End of the `#[cfg(test)] mod tests { ... }` block (after the Phase 5 tests)

### Test 1: Direct float assignment

```rust
#[test]
fn test_local_float_binding() {
    let mut backend = LlvmBackend::new();
    let program = Program {
        items: vec![
            TopLevel::StateDecl(StateDecl {
                name: "x".to_string(),
                ty: Type::Float,
                expr: Some(Expr::Float(1.5)),
                address: None, bit_range: None,
                is_override: false, os_mode: false,
                span: None, attrs: vec![],
            }),
            TopLevel::Transaction(Transaction {
                name: "t".to_string(),
                parameters: vec![],
                contract: Contract {
                    pre_condition: Expr::Bool(true),
                    post_condition: Expr::Bool(true),
                    span: None, watchdog: None,
                },
                body: vec![
                    Statement::Assignment {
                        lhs: Expr::Identifier("x".to_string()),
                        expr: Expr::Float(2.0),
                        timeout: None, modifiers: vec![],
                    },
                    Statement::Term { values: vec![], modifiers: vec![] },
                ],
                is_async: false, is_reactive: false,
                reactor_speed: None, span: None,
                is_lambda: false, dependencies: vec![],
                attrs: vec![], modifiers: vec![],
                variant_bodies: vec![],
            }),
        ],
        comments: vec![],
        reactor_speed: None,
        attrs: Vec::new(),
        ffi: None,
        strict_mode: StrictMode::Off,
        dispatch_mode: Default::default(),
    };
    let output = backend.generate(&program);

    // Should emit bitcast float → i32 → zext i64 for the float literal
    assert!(output.contains("bitcast float"),
        "Float expression should emit bitcast float to i32");
    // Should NOT call fadd (no binary operation on floats in this test)
    // but should have the float literal pattern
    assert!(output.contains("float 2.0"),
        "Float literal 2.0 should appear in IR");
}
```

### Test 2: Float binary operation

```rust
#[test]
fn test_float_binary_add() {
    let mut backend = LlvmBackend::new();
    let program = Program {
        items: vec![
            TopLevel::StateDecl(StateDecl {
                name: "x".to_string(),
                ty: Type::Float,
                expr: Some(Expr::Float(1.0)),
                address: None, bit_range: None,
                is_override: false, os_mode: false,
                span: None, attrs: vec![],
            }),
            TopLevel::Transaction(Transaction {
                name: "t".to_string(),
                parameters: vec![],
                contract: Contract {
                    pre_condition: Expr::Bool(true),
                    post_condition: Expr::Bool(true),
                    span: None, watchdog: None,
                },
                body: vec![
                    Statement::Assignment {
                        lhs: Expr::Identifier("x".to_string()),
                        expr: Expr::Add(
                            Box::new(Expr::Identifier("x".to_string())),
                            Box::new(Expr::Float(2.0)),
                        ),
                        timeout: None, modifiers: vec![],
                    },
                    Statement::Term { values: vec![], modifiers: vec![] },
                ],
                is_async: false, is_reactive: false,
                reactor_speed: None, span: None,
                is_lambda: false, dependencies: vec![],
                attrs: vec![], modifiers: vec![],
                variant_bodies: vec![],
            }),
        ],
        comments: vec![],
        reactor_speed: None,
        attrs: Vec::new(),
        ffi: None,
        strict_mode: StrictMode::Off,
        dispatch_mode: Default::default(),
    };
    let output = backend.generate(&program);

    // Float + Float should emit fadd, not add
    assert!(output.contains("fadd float"),
        "Float binary add should emit fadd float");
    // Should NOT emit integer add for this expression
    assert!(!output.contains("add i64"),
        "Float add should NOT emit integer add i64");
}
```

### Key Assertions

| Assertion | Why |
|-----------|-----|
| `bitcast float` | B2 fix: `Expr::Float` registers type, emission uses `bitcast float to i32` |
| `fadd float` | `is_float_expr` returns true, `emit_binop` picks `fadd` |
| NOT `add i64` | Integer ops should not appear for float-typed expressions |
| `float 2.0` | Literal float value appears in IR |
| `float 1.0` | Initial state value appears in IR |

### Dependencies on B2 fix

These tests verify that the Phase 0 B2 fix is correctly wired:

1. `Expr::Float` → `register_types.insert(v, Type::Float)` (line 778 area)
2. `is_float_expr` → checks `register_types` before `field_index_map` (line 1390 area)
3. `emit_binop` → selects `fadd`/`fsub`/`fmul`/`fdiv` when `is_float_expr` (line 834 area)