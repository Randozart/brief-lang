// ── Statement Codegen ──────────────────────────────────────────────────
// 2026-07-12: Phase 4 — Emit LLVM IR for all Statement variants.
//
// 2026-07-04: MAX_FIELDS_PER_ALLLOCA=15 ensures LLVM's SROA can decompose
// %State chunks into scalars for alias analysis and vectorization.

use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
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
                if let Some(reg) = backend.fun.let_bindings.get(name) {
                    // 2026-07-14: store type must match val.ty — hardcoded i64 breaks bool/float assigns
                    let store_ty = crate::backend::llvm::types::lower_type(&val.ty);
                    writeln!(out, "{}store {} {}, ptr {}", indent, store_ty, val.name, reg).ok();
                // 2026-07-14: Handle MMIO and regular state field assignments
                } else if let Some(&addr) = backend.ctx.mmio_fields.get(name) {
                    let ptr = backend.fun.gen_reg();
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
                    // 2026-07-14: volatile store type must match val.ty — hardcoded i64 breaks MMIO bools
                    let store_ty = crate::backend::llvm::types::lower_type(&val.ty);
                    writeln!(out, "{}store volatile {} {}, ptr {}", indent, store_ty, val.name, ptr).ok();
                } else if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let ptr = backend.fun.gen_reg();
                    writeln!(out, "{}{} = getelementptr %State, ptr %state, i32 0, i32 {}", indent, ptr, idx).ok();
                    // 2026-07-17: State stores always i64 — box via adapt_to_i64.
                    let boxed = backend.adapt_to_i64(out, indent, &val);
                    writeln!(out, "{}store i64 {}, ptr {}", indent, boxed, ptr).ok();
                }
            }
            TypedRegister { name: val.name, ty: Type::void() }
        }
        Statement::Expression(expr) => backend.emit_expr(out, expr, indent),
        Statement::Term(val) | Statement::TermBang(val) => {
            if let Some(val) = val {
                let reg = backend.emit_expr(out, val, indent);
                if backend.fun.fn_ret_ty != "void" {
                    let ret_ty = crate::backend::llvm::types::lower_type(&reg.ty);
                    writeln!(out, "{}ret {} {}", indent, ret_ty, reg.name).ok();
                    backend.fun.terminated = true;
                }
                // 2026-07-15: Void functions: just set terminated — caller emits ret at done:
                backend.fun.terminated = true;
            }
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        Statement::Return(val) => {
            if let Some(val) = val {
                let reg = backend.emit_expr(out, val, indent);
                let ret_ty = crate::backend::llvm::types::lower_type(&reg.ty);
                writeln!(out, "{}ret {} {}", indent, ret_ty, reg.name).ok();
                backend.fun.terminated = true;
            }
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        Statement::Guarded(cond, body) => {
            let cond_reg = backend.emit_expr(out, cond, indent);
            // 2026-07-14: labels need a counter without % prefix — gen_reg() returns %tN
            let label_n = backend.fun.txn_counter;
            backend.fun.txn_counter += 1;
            let then_lbl = format!("guard.then{}", label_n);
            let end_lbl = format!("guard.end{}", label_n);
            // 2026-07-14: bool cond is i8 — trunc to i1 for br instruction
            let cond_i1 = if cond_reg.ty == Type::bool_() {
                let b = backend.fun.gen_reg();
                writeln!(out, "{}{} = trunc i8 {} to i1", indent, b, cond_reg.name).ok();
                b
            } else {
                cond_reg.name.clone()
            };
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cond_i1, then_lbl, end_lbl).ok();
            writeln!(out, "{}{}:", indent, then_lbl).ok();
            for stmt in body {
                emit_statement(backend, out, stmt, indent);
            }
            // 2026-07-15: Always emit br to end (even if then body terminated)
            writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            writeln!(out, "{}{}:", indent, end_lbl).ok();
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        Statement::If(cond, then, else_) => {
            let cond_reg = backend.emit_expr(out, cond, indent);
            // 2026-07-14: labels need a counter without % prefix — gen_reg() returns %tN
            let label_n = backend.fun.txn_counter;
            backend.fun.txn_counter += 1;
            let then_lbl = format!("if.then{}", label_n);
            let else_lbl = format!("if.else{}", label_n);
            let end_lbl = format!("if.end{}", label_n);
            // 2026-07-14: bool cond is i8 — trunc to i1 for br instruction
            let cond_i1 = if cond_reg.ty == Type::bool_() {
                let b = backend.fun.gen_reg();
                writeln!(out, "{}{} = trunc i8 {} to i1", indent, b, cond_reg.name).ok();
                b
            } else {
                cond_reg.name.clone()
            };
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cond_i1, then_lbl, else_lbl).ok();
            writeln!(out, "{}{}:", indent, then_lbl).ok();
            for stmt in then {
                emit_statement(backend, out, stmt, indent);
            }
            writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            writeln!(out, "{}{}:", indent, else_lbl).ok();
            for stmt in else_ {
                emit_statement(backend, out, stmt, indent);
            }
            writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            writeln!(out, "{}{}:", indent, end_lbl).ok();
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
