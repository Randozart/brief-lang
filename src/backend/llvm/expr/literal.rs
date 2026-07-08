// ── Literal Expression Codegen ─────────────────────────────────────
//
// Handles emission of literal expressions: integers, floats, bools,
// strings, chars, and Term.
//
// 2026-06-29: Extracted from emit_expr.rs lines 37-77. Each function
// handles one Expr variant and returns a TypedRegister.
//
// Each function receives a pre-allocated result register `v` that was
// created by the caller (emit_expr). Additional registers may be
// allocated inline as needed.

use crate::ast::{Expr, Type};
use crate::backend::llvm::{float64_to_llvm_hex, float_to_llvm_hex, LlvmBackend, TypedRegister};
use std::fmt::Write;

pub fn emit_integer(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    if let Expr::Integer(n) = expr {
        writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
    } else {
        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    }
    TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
}

pub fn emit_integer_suffixed(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::IntegerSuffixed(n, ty) => {
            let llvm_ty = backend.llvm_type(ty);
            writeln!(out, "{}{} = add {} 0, {}", indent, v, llvm_ty, n).ok();
            TypedRegister { name: v.to_string(), ty: ty.clone() }
        }
        _ => emit_integer(backend, out, v, expr, indent),
    }
}

pub fn emit_bool(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::Bool(b) => {
            if *b {
                writeln!(out, "{}{} = and i1 true, true", indent, v).ok();
            } else {
                writeln!(out, "{}{} = xor i1 true, true", indent, v).ok();
            }
            TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) }
        }
        _ => emit_integer(backend, out, v, expr, indent),
    }
}

pub fn emit_float64(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::Float64(f) => {
            let bits = float64_to_llvm_hex(*f);
            writeln!(out, "{}{} = bitcast i64 {} to double", indent, v, bits).ok();
            TypedRegister { name: v.to_string(), ty: Type::Custom("Float64".to_string()) }
        }
        _ => emit_integer(backend, out, v, expr, indent),
    }
}

pub fn emit_float(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    match expr {
        Expr::Float(f) => {
            let bits = float_to_llvm_hex(*f);
            writeln!(out, "{}{} = bitcast i32 {} to float", indent, v, bits).ok();
            backend.fun.reg_float_cache.insert(v.to_string(), v.to_string());
            TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) }
        }
        _ => emit_integer(backend, out, v, expr, indent),
    }
}

pub fn emit_string(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    let s = match expr {
        Expr::String(s) | Expr::RegexLiteral(s) => s,
        _ => return emit_integer(backend, out, v, expr, indent),
    };
    let si = backend.ctx.string_constants.iter().position(|x| x == s).unwrap_or(0);
    let g = format!("@str.{}", si);
    let bp = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = bitcast <{{ i64, i64, [{} x i8] }}>* {} to ptr", indent, bp, s.len() + 1, g).ok();
    // Tag static string pointers with bit 0 (=1) so concat can distinguish
    // them from heap-allocated strings and avoid freeing static data.
    let pi = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, pi, bp).ok();
    let ori = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = or i64 {}, 1", indent, ori, pi).ok();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, v, ori).ok();
    TypedRegister { name: v.to_string(), ty: Type::Custom("String".to_string()) }
}

pub fn emit_char(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    if let Expr::Char(c) = expr {
        writeln!(out, "{}{} = add i64 0, {}", indent, v, *c as i32).ok();
    } else {
        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    }
    TypedRegister { name: v.to_string(), ty: Type::Custom("Char".to_string()) }
}

pub fn emit_term(backend: &mut LlvmBackend, out: &mut String, v: &str, indent: &str) -> TypedRegister {
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
}
