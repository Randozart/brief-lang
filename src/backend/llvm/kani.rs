use super::*;
use crate::features::literal::LiteralExpr;

#[kani::proof]
fn verify_llvm_emit_expr_literal_integer() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::int());
    assert!(out.contains("add i64 0, 42"));
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_bool() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::bool_());
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_term() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Term));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::int());
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_float() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Float(1.5)));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::float());
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_string() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::String("s".to_string())));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::string());
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_char() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Char('A')));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::char_());
}

#[kani::proof]
fn verify_llvm_emit_guard_check_trap() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    // Set up let_bindings with a variable for guard_check to find
    backend
        .fun
        .let_bindings
        .insert("x".to_string(), Reg::int("%xval"));
    backend
        .fun
        .let_binding_types
        .insert("x".to_string(), Type::int());
    // Guard: _ > 0 (where _ is bound to x's value %xval)
    let guard = Expr::Gt(
        Box::new(Expr::Identifier("_".to_string())),
        Box::new(Expr::Decimal(0)),
    );
    backend.emit_guard_check(&mut out, "", "x", &guard);
    assert!(
        out.contains("@llvm.trap"),
        "emit_guard_check must emit @llvm.trap. Got:\n{}",
        out
    );
    assert!(
        out.contains("unreachable"),
        "emit_guard_check must emit unreachable. Got:\n{}",
        out
    );
    assert!(
        out.contains("br i1"),
        "emit_guard_check must emit conditional branch. Got:\n{}",
        out
    );
}

#[kani::proof]
fn verify_llvm_emit_guard_check_saves_prior_underscore() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    // Bind _ first, then x
    backend
        .fun
        .let_bindings
        .insert("_".to_string(), Reg::int("%prior"));
    backend
        .fun
        .let_binding_types
        .insert("_".to_string(), Type::int());
    backend
        .fun
        .let_bindings
        .insert("x".to_string(), Reg::int("%xval"));
    backend
        .fun
        .let_binding_types
        .insert("x".to_string(), Type::int());
    let guard = Expr::Gt(
        Box::new(Expr::Identifier("_".to_string())),
        Box::new(Expr::Decimal(0)),
    );
    backend.emit_guard_check(&mut out, "", "x", &guard);
    // After emit_guard_check, _ should be restored to %prior
    assert_eq!(
        backend.fun.let_bindings.get("_"),
        Some(&Reg::int("%prior")),
        "_ must be restored after guard check"
    );
}
