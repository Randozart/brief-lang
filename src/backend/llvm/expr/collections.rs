// ── Collection Expression Codegen ─────────────────────────────────
//
// Handles emission of list and tuple literal expressions.
// 2026-06-29: Extracted from emit_expr.rs lines 2673-2738+.

use crate::ast::{Expr, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

pub fn emit_list_literal(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    if let Expr::ListLiteral(items) = expr {
        emit_list_or_tuple_body(backend, out, v, items, "l", indent)
    } else {
        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
        TypedRegister { name: v.to_string(), ty: Type::int() }
    }
}

pub fn emit_tuple(backend: &mut LlvmBackend, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
    if let Expr::Tuple(items) = expr {
        emit_list_or_tuple_body(backend, out, v, items, "t", indent)
    } else {
        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
        TypedRegister { name: v.to_string(), ty: Type::int() }
    }
}

fn emit_list_or_tuple_body(backend: &mut LlvmBackend, out: &mut String, v: &str, items: &[Expr], prefix: &str, indent: &str) -> TypedRegister {
    if items.is_empty() {
        // Empty list/tuple → global rodata sentinel @ll_empty_list
        writeln!(out, "{}{} = ptrtoint {{ i64, i64 }}* @ll_empty_list to i64", indent, v).ok();
        return TypedRegister { name: v.to_string(), ty: Type::int() };
    }
    // Non-empty → malloc + populate (safe for %State persistence)
    let n = items.len() as i64;
    let total = n + 2;
    let ai = format!("%{}ai{}", prefix, backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = call ptr @malloc(i64 {})", indent, ai, total * 8).ok();
    let cast = format!("%{}ac{}", prefix, backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, cast, ai).ok();
    let dp_ptr = format!("%{}dp{}", prefix, backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 2", indent, dp_ptr, cast).ok();
    let dp_val = format!("%{}dv{}", prefix, backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, dp_val, dp_ptr).ok();
    let s0 = format!("%{}s0{}", prefix, backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, s0, cast).ok();
    writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, dp_val, s0).ok();
    let s1 = format!("%{}s1{}", prefix, backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, s1, cast).ok();
    writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, n, s1).ok();
    for (i, item) in items.iter().enumerate() {
        let iv = backend.emit_expr(out, item, indent);
        let adapted = backend.adapt_to_i64(out, indent, &iv);
        let ep = format!("%{}ep{}", prefix, backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, cast, (i as i64) + 2).ok();
        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, adapted, ep).ok();
    }
    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, cast).ok();
    TypedRegister { name: v.to_string(), ty: Type::int() }
}
