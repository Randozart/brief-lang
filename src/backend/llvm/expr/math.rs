// ── Arithmetic & Bitwise Expression Codegen ────────────────────────
//
// Handles emission of arithmetic (Add, Sub, Mul, Div, Mod, Neg) and
// bitwise (And, Or, Xor, Not, Shl, Shr, BitAnd, BitOr, BitXor, BitNot)
// expressions.
//
// 2026-06-29: Extracted from emit_expr.rs lines 373-440.
// Each function does its own register allocation rather than using the
// `v` pre-allocated by emit_expr, because several delegate to emit_binop
// which also allocates its own registers.

use crate::ast::{Expr, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

macro_rules! emit_binop_dispatch {
    ($backend:expr, $out:expr, $expr:expr, $variant:ident, $int_op:expr, $float_op:expr, $indent:expr) => {{
        match $expr {
            Expr::$variant(l, r) => $backend.emit_binop($out, $indent, l, r, $int_op, $float_op),
            _ => emit_zero($backend, $out, $indent),
        }
    }};
}

pub fn emit_add(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    emit_binop_dispatch!(backend, out, expr, Add, "add", "fadd", indent)
}

pub fn emit_sub(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    emit_binop_dispatch!(backend, out, expr, Sub, "sub", "fsub", indent)
}

pub fn emit_mul(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    emit_binop_dispatch!(backend, out, expr, Mul, "mul", "fmul", indent)
}

pub fn emit_div(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    emit_binop_dispatch!(backend, out, expr, Div, "sdiv", "fdiv", indent)
}

pub fn emit_mod(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::Mod(l, r) => {
            let a = backend.emit_expr(out, l, indent);
            let b = backend.emit_expr(out, r, indent);
            let v = alloc_reg(backend, out, indent);
            writeln!(out, "{}{} = srem i64 {}, {}", indent, v, a.name, b.name).ok();
            TypedRegister { name: v, ty: Type::Int }
        }
        _ => emit_zero(backend, out, indent),
    }
}

pub fn emit_neg(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::Neg(e) => {
            let inner = backend.emit_expr(out, e, indent);
            let v = alloc_reg(backend, out, indent);
            if inner.ty == Type::Float {
                writeln!(out, "{}{} = fneg float {}", indent, v, inner.name).ok();
            } else {
                writeln!(out, "{}{} = sub i64 0, {}", indent, v, inner.name).ok();
            }
            TypedRegister { name: v, ty: inner.ty }
        }
        _ => emit_zero(backend, out, indent),
    }
}

// ── Bitwise Operations ────────────────────────────────────────────

pub fn emit_bitand(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::BitAnd(l, r) => {
            let (a, b) = (backend.emit_expr(out, l, indent), backend.emit_expr(out, r, indent));
            let v = alloc_reg(backend, out, indent);
            writeln!(out, "{}{} = and i64 {}, {}", indent, v, a.name, b.name).ok();
            TypedRegister { name: v, ty: Type::Int }
        }
        _ => emit_zero(backend, out, indent),
    }
}

pub fn emit_bitor(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::BitOr(l, r) => {
            let (a, b) = (backend.emit_expr(out, l, indent), backend.emit_expr(out, r, indent));
            let v = alloc_reg(backend, out, indent);
            writeln!(out, "{}{} = or i64 {}, {}", indent, v, a.name, b.name).ok();
            TypedRegister { name: v, ty: Type::Int }
        }
        _ => emit_zero(backend, out, indent),
    }
}

pub fn emit_bitxor(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::BitXor(l, r) => {
            let (a, b) = (backend.emit_expr(out, l, indent), backend.emit_expr(out, r, indent));
            let v = alloc_reg(backend, out, indent);
            writeln!(out, "{}{} = xor i64 {}, {}", indent, v, a.name, b.name).ok();
            TypedRegister { name: v, ty: Type::Int }
        }
        _ => emit_zero(backend, out, indent),
    }
}

pub fn emit_bitnot(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::BitNot(e) => {
            let inner = backend.emit_expr(out, e, indent);
            let v = alloc_reg(backend, out, indent);
            writeln!(out, "{}{} = xor i64 {}, -1", indent, v, inner.name).ok();
            TypedRegister { name: v, ty: Type::Int }
        }
        _ => emit_zero(backend, out, indent),
    }
}

pub fn emit_shl(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::Shl(l, r) => {
            let (a, b) = (backend.emit_expr(out, l, indent), backend.emit_expr(out, r, indent));
            let v = alloc_reg(backend, out, indent);
            writeln!(out, "{}{} = shl i64 {}, {}", indent, v, a.name, b.name).ok();
            TypedRegister { name: v, ty: Type::Int }
        }
        _ => emit_zero(backend, out, indent),
    }
}

pub fn emit_shr(backend: &mut LlvmBackend, out: &mut String, _v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::Shr(l, r) => {
            let (a, b) = (backend.emit_expr(out, l, indent), backend.emit_expr(out, r, indent));
            let v = alloc_reg(backend, out, indent);
            writeln!(out, "{}{} = lshr i64 {}, {}", indent, v, a.name, b.name).ok();
            TypedRegister { name: v, ty: Type::Int }
        }
        _ => emit_zero(backend, out, indent),
    }
}

// ── Helpers ────────────────────────────────────────────────────────

/// Allocate a new temporary register (for operations that cannot delegate to emit_binop).
fn alloc_reg(backend: &mut LlvmBackend, out: &mut String, indent: &str) -> String {
    let v = format!("%t{}", backend.fun.txn_counter);
    backend.fun.txn_counter += 1;
    v
}

fn emit_zero(backend: &mut LlvmBackend, out: &mut String, indent: &str) -> TypedRegister {
    let v = alloc_reg(backend, out, indent);
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    TypedRegister { name: v, ty: Type::Int }
}
