// ── Comparison & Logical Expression Codegen ───────────────────────
//
// Handles emission of comparison (Eq, Ne, Lt, Le, Gt, Ge) and logical
// (And, Or, Not) expressions.
//
// 2026-06-29: Extracted from emit_expr.rs comparison/logical arms.

use crate::ast::Expr;
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

macro_rules! compare_emit {
    ($backend:expr, $out:expr, $expr:expr, $variant:ident, $cond:expr, $indent:expr) => {{
        match $expr {
            Expr::$variant(l, r) => $backend.emit_fcmp($out, $indent, l, r, $cond),
            _ => unreachable!(),
        }
    }};
}

pub fn emit_eq(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    compare_emit!(backend, out, expr, Eq, "oeq", indent)
}

pub fn emit_ne(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    compare_emit!(backend, out, expr, Ne, "one", indent)
}

pub fn emit_lt(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    compare_emit!(backend, out, expr, Lt, "olt", indent)
}

pub fn emit_le(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    compare_emit!(backend, out, expr, Le, "ole", indent)
}

pub fn emit_gt(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    compare_emit!(backend, out, expr, Gt, "ogt", indent)
}

pub fn emit_ge(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    compare_emit!(backend, out, expr, Ge, "oge", indent)
}

pub fn emit_and(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    if let Expr::And(l, r) = expr {
        let a = backend.emit_expr(out, l, indent);
        let b = backend.emit_expr(out, r, indent);
        let an = backend.as_bool_reg(out, indent, &a);
        let bn = backend.as_bool_reg(out, indent, &b);
        writeln!(out, "{}{} = and i1 {}, {}", indent, v, an, bn).ok();
    }
    TypedRegister { name: v.to_string(), ty: crate::ast::Type::Bool }
}

pub fn emit_or(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    if let Expr::Or(l, r) = expr {
        let a = backend.emit_expr(out, l, indent);
        let b = backend.emit_expr(out, r, indent);
        let an = backend.as_bool_reg(out, indent, &a);
        let bn = backend.as_bool_reg(out, indent, &b);
        writeln!(out, "{}{} = or i1 {}, {}", indent, v, an, bn).ok();
    }
    TypedRegister { name: v.to_string(), ty: crate::ast::Type::Bool }
}

pub fn emit_not(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    if let Expr::Not(e) = expr {
        let inner = backend.emit_expr(out, e, indent);
        let name = backend.as_bool_reg(out, indent, &inner);
        writeln!(out, "{}{} = xor i1 {}, true", indent, v, name).ok();
    }
    TypedRegister { name: v.to_string(), ty: crate::ast::Type::Bool }
}
