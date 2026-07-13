// ── Statement Codegen ──────────────────────────────────────────────────
// 2026-07-12: Phase 4 — Emit LLVM IR for all Statement variants.
//
// 2026-07-04: MAX_FIELDS_PER_ALLLOCA=15 ensures LLVM's SROA can decompose
// %State chunks into scalars for alias analysis and vectorization.

use crate::ast_new::{Expr, Statement, Type};
use crate::backend::llvm::TypedRegister;
use crate::backend::llvm::helpers::LlvmBackend;
use std::fmt::Write;

pub(crate) const MAX_FIELDS_PER_ALLLOCA: usize = 15;

/// Emit LLVM IR for a statement. Returns the last expression's register.
pub fn emit_statement(backend: &mut LlvmBackend, out: &mut String, stmt: &Statement, indent: &str) -> TypedRegister {
    match stmt {
        Statement::Let { name, ty, expr, .. } => {
            let val = match expr {
                Some(e) => backend.emit_expr(out, e, indent),
                None => {
                    let v = backend.fun.gen_reg();
                    let llvm_ty = ty.as_ref().map(|t| crate::backend::llvm::types::lower_type(t)).unwrap_or("i64".into());
                    writeln!(out, "{}{} = alloca {}", indent, v, llvm_ty).ok();
                    TypedRegister { name: v, ty: ty.clone().unwrap_or(Type::int()) }
                }
            };
            backend.fun.let_bindings.insert(name.clone(), val.name.clone());
            backend.fun.let_binding_types.insert(name.clone(), val.ty.clone());
            TypedRegister { name: val.name, ty: Type::void() }
        }
        Statement::Assign(lhs, rhs) => {
            let val = backend.emit_expr(out, rhs, indent);
            if let Expr::Identifier(name) = lhs {
                if let Some(reg) = backend.get_local(name) {
                    writeln!(out, "{}store i64 {}, ptr {}", indent, val.name, reg).ok();
                }
            }
            TypedRegister { name: val.name, ty: Type::void() }
        }
        Statement::Expression(expr) => backend.emit_expr(out, expr, indent),
        Statement::Term(val) | Statement::TermBang(val) => {
            if let Some(val) = val {
                let reg = backend.emit_expr(out, val, indent);
                writeln!(out, "{}ret i64 {}", indent, reg.name).ok();
                backend.fun.terminated = true;
            }
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        Statement::Return(val) => {
            if let Some(val) = val {
                let reg = backend.emit_expr(out, val, indent);
                writeln!(out, "{}ret i64 {}", indent, reg.name).ok();
                backend.fun.terminated = true;
            }
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        Statement::Guarded(cond, body) => {
            let cond_reg = backend.emit_expr(out, cond, indent);
            let then_lbl = format!("guard.then{}", backend.fun.gen_reg());
            let end_lbl = format!("guard.end{}", backend.fun.gen_reg());
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cond_reg.name, then_lbl, end_lbl).ok();
            writeln!(out, "{}:", indent, then_lbl).ok();
            for stmt in body {
                emit_statement(backend, out, stmt, indent);
            }
            writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            writeln!(out, "{}:", indent, end_lbl).ok();
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        Statement::If(cond, then, else_) => {
            let cond_reg = backend.emit_expr(out, cond, indent);
            let then_lbl = format!("if.then{}", backend.fun.gen_reg());
            let else_lbl = format!("if.else{}", backend.fun.gen_reg());
            let end_lbl = format!("if.end{}", backend.fun.gen_reg());
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cond_reg.name, then_lbl, else_lbl).ok();
            writeln!(out, "{}:", indent, then_lbl).ok();
            for stmt in then {
                emit_statement(backend, out, stmt, indent);
            }
            writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            writeln!(out, "{}:", indent, else_lbl).ok();
            for stmt in else_ {
                emit_statement(backend, out, stmt, indent);
            }
            writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            writeln!(out, "{}:", indent, end_lbl).ok();
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        Statement::Block(stmts) => {
            let mut last = TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() };
            for stmt in stmts {
                last = emit_statement(backend, out, stmt, indent);
            }
            last
        }
        Statement::Escape(_) => {
            writeln!(out, "{}ret i64 0", indent).ok();
            backend.fun.terminated = true;
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        _ => {
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
    }
}
