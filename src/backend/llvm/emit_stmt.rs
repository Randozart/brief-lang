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
            // 2026-07-17: Pop: `x <- &queue` → Assign(Identifier(x), AddrOf(source)).
            // Detect this pattern BEFORE emitting the RHS (which would get the
            // address, not the popped value). Emit the ring buffer pop directly.
            // 2026-07-18: Pop — emit call @fn_name(handle), store result to lhs.
            if let Expr::AddrOf(source) = rhs {
                let strat = backend.check_extract_strategy(source)
                    .or_else(|| backend.check_extract_strategy(rhs));
                // Extract fn_name from property value — no hardcoded strings.
                let Some(crate::ast::PropertyValue::Identifier(fn_name)) = &strat else {
                    return TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() };
                };
                let Some(result) = emit_strategy_fn_call(backend, out, indent, source, fn_name, None) else {
                    return TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() };
                };
                // Store popped result to the LHS variable.
                let Expr::Identifier(name) = lhs else {
                    return TypedRegister { name: result, ty: Type::int() };
                };
                if let Some(reg) = backend.fun.let_bindings.get(name) {
                    writeln!(out, "{}store i64 {}, ptr {}", indent, result, reg).ok();
                } else if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let ptr = backend.fun.gen_reg();
                    writeln!(out, "{}{} = getelementptr %State, ptr %state, i32 0, i32 {}", indent, ptr, idx).ok();
                    writeln!(out, "{}store i64 {}, ptr {}", indent, result, ptr).ok();
                }
                return TypedRegister { name: result, ty: Type::int() };
            }

            let val = backend.emit_expr(out, rhs, indent);
            match lhs {
                Expr::Identifier(name) => {
                    // 2026-07-18: Push — emit call @fn_name(handle, val).
                    let strat = backend.check_insert_strategy(lhs);
                    if let Some(crate::ast::PropertyValue::Identifier(fn_name)) = &strat {
                        emit_strategy_fn_call(backend, out, indent, lhs, fn_name, Some(&val.name));
                    } else if let Some(reg) = backend.fun.let_bindings.get(name) {
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
                // 2026-07-17: Push: `&queue <- value` → Assign(AddrOf(target), value).
                // The `&` on the LHS is optional — the type-based check above handles
                // the bare-identifier case. The AddrOf arm is kept for explicit usage.
                // 2026-07-17: Dereference-assign: `*ptr = val`. Compute the
                // pointer address and store the value through it. Supports
                // pointer-offset arithmetic (buf + N) via GEP in emit_expr.
                Expr::Deref(inner) => {
                    let ptr_reg = backend.emit_expr(out, inner, indent);
                    let store_ty = crate::backend::llvm::types::lower_type(&val.ty);
                    writeln!(out, "{}store {} {}, ptr {}", indent, store_ty, val.name, ptr_reg.name).ok();
                }
                // 2026-07-17: Pointer-indexed store — data[idx] = val.
                // Emits inttoptr + GEP + store for Ptr-typed objects.
                // List/tuple literals need idx+1 (slot 0 = length header).
                Expr::Index(obj, idx) => {
                    let obj_reg = backend.emit_expr(out, obj, indent);
                    if matches!(obj_reg.ty, Type::Ptr(_)) {
                        let idx_reg = backend.emit_expr(out, idx, indent);
                        let ptr = backend.fun.gen_reg();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, obj_reg.name).ok();
                        let gep = backend.fun.gen_reg();
                        let offset = backend.fun.gen_reg();
                        if matches!(obj.as_ref(), Expr::List(_) | Expr::Tuple(_)) {
                            writeln!(out, "{}{} = add i64 {}, 1", indent, offset, idx_reg.name).ok();
                        } else {
                            writeln!(out, "{}{} = add i64 {}, 0", indent, offset, idx_reg.name).ok();
                        }
                        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, gep, ptr, offset).ok();
                        writeln!(out, "{}store i64 {}, ptr {}", indent, val.name, gep).ok();
                    }
                }
                _ => {}
            }
            TypedRegister { name: val.name, ty: Type::void() }
        }
        Statement::Expression(expr) => {
            // 2026-07-17: Discard: `<- &queue` → Expression(AddrOf(source)).
            // Pop from collection and discard the result.
            if let Expr::AddrOf(source) = expr {
                let strat = backend.check_extract_strategy(source)
                    .or_else(|| backend.check_extract_strategy(expr));
                if let Some(crate::ast::PropertyValue::Identifier(fn_name)) = &strat {
                    emit_strategy_fn_call(backend, out, indent, source, fn_name, None);
                }
                TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
            } else {
                backend.emit_expr(out, expr, indent)
            }
        }
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

/// Compute the handle (ptrtoint of first RingBuf inline field) and emit a generic
/// call @fn_name(handle[, value]) for strategy-based collection operations.
/// 2026-07-18: Generic dispatch — no hardcoded function names. The fn_name comes
/// from the type property (e.g. "ring_push" from InsertAt <~ ring_push).
fn emit_strategy_fn_call(backend: &mut LlvmBackend, out: &mut String, indent: &str,
    target: &Expr, fn_name: &str, value: Option<&str>) -> Option<String> {
    let name = target.as_var_name()?;
    let rbi = backend.ctx.ringbuf_inline.get(name)?;
    let gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
        indent, gep, rbi.data_idx).ok();
    let handle = backend.fun.gen_reg();
    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, handle, gep).ok();
    match value {
        Some(val) => {
            let result = backend.fun.gen_reg();
            writeln!(out, "{}{} = call i64 @{}(i64 {}, i64 {})", indent, result, fn_name, handle, val).ok();
            Some(result)
        }
        None => {
            let result = backend.fun.gen_reg();
            writeln!(out, "{}{} = call i64 @{}(i64 {})", indent, result, fn_name, handle).ok();
            Some(result)
        }
    }
}
