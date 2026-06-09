use super::*;
use crate::features::literal::LiteralExpr;

#[kani::proof]
fn verify_llvm_emit_expr_literal_integer() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::Int);
    assert!(out.contains("add i64 0, 42"));
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_bool() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::Bool);
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_term() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Term));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::Int);
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_float() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Float(1.5)));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::Float);
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_string() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::String("s".to_string())));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::String);
}

#[kani::proof]
fn verify_llvm_emit_expr_literal_char() {
    let mut backend = LlvmBackend::new();
    let mut out = String::new();
    let expr = Expr::Literal(Box::new(LiteralExpr::Char('A')));
    let reg = backend.emit_expr(&mut out, &expr, "");
    assert_eq!(reg.ty, Type::Char);
}
