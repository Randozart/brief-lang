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
            if let Expr::AddrOf(source) = rhs {
                let strat = backend.check_extract_strategy(source)
                    .or_else(|| backend.check_extract_strategy(rhs));
                if strat.as_deref() == Some("ring_pop") {
                    let val = emit_ring_pop(backend, out, indent, source, None);
                    if let Expr::Identifier(name) = lhs {
                        if let Some(reg) = backend.fun.let_bindings.get(name) {
                            writeln!(out, "{}store i64 {}, ptr {}", indent, val, reg).ok();
                        } else if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                            let ptr = backend.fun.gen_reg();
                            writeln!(out, "{}{} = getelementptr %State, ptr %state, i32 0, i32 {}", indent, ptr, idx).ok();
                            writeln!(out, "{}store i64 {}, ptr {}", indent, val, ptr).ok();
                        }
                    }
                    return TypedRegister { name: val, ty: Type::int() };
                }
            }

            let val = backend.emit_expr(out, rhs, indent);
            match lhs {
                Expr::Identifier(name) => {
                    // 2026-07-17: Check type-based insert strategy. If the target
                    // variable is a RingBuffer (insert_at = "ring_push"), emit a
                    // ring buffer push instead of a regular state store. This is
                    // how `queue <- value` works without requiring `&` on the LHS.
                    let strat = backend.check_insert_strategy(lhs);
                    if strat.as_deref() == Some("ring_push") {
                        emit_ring_push(backend, out, indent, lhs, &val);
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
                Expr::AddrOf(target) => {
                    emit_ring_push(backend, out, indent, target, &val);
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
                // 2026-07-17: Push: `&queue <- value` → Assign(AddrOf(target), value).
                // The LHS is an AddrOf expression marking the collection target.
                Expr::AddrOf(target) => {
                    emit_ring_push(backend, out, indent, target, &val);
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
                if strat.as_deref() == Some("ring_pop") {
                    emit_ring_pop(backend, out, indent, source, None);
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

/// Emit inline RingBuffer push: data[tail & mask] = value; tail = (tail+1) & mask.
/// Uses direct %State GEP access via ringbuf_inline field indices (no inttoptr handle).
fn emit_ring_push(backend: &mut LlvmBackend, out: &mut String, indent: &str,
    target: &Expr, val: &TypedRegister) {
    let name = target.as_var_name().and_then(|n| {
        backend.ctx.ringbuf_inline.get(n).map(|rbi| (n.to_string(), rbi.clone()))
    });
    let Some((_name, rbi)) = name else { return };
    // tail = (tail + 1) & mask
    let t_gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
        indent, t_gep, rbi.tail_idx).ok();
    let t_val = backend.fun.gen_reg();
    writeln!(out, "{}{} = load i64, ptr {}", indent, t_val, t_gep).ok();
    let m_gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
        indent, m_gep, rbi.mask_idx).ok();
    let m_val = backend.fun.gen_reg();
    writeln!(out, "{}{} = load i64, ptr {}", indent, m_val, m_gep).ok();
    let d_gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
        indent, d_gep, rbi.data_idx).ok();
    let d_val = backend.fun.gen_reg();
    writeln!(out, "{}{} = load i64, ptr {}", indent, d_val, d_gep).ok();
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, d_val).ok();
    let idx = backend.fun.gen_reg();
    writeln!(out, "{}{} = and i64 {}, {}", indent, idx, t_val, m_val).ok();
    let gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, gep, ptr, idx).ok();
    writeln!(out, "{}store i64 {}, ptr {}", indent, val.name, gep).ok();
    let nt = backend.fun.gen_reg();
    writeln!(out, "{}{} = add i64 {}, 1", indent, nt, t_val).ok();
    let nw = backend.fun.gen_reg();
    writeln!(out, "{}{} = and i64 {}, {}", indent, nw, nt, m_val).ok();
    writeln!(out, "{}store i64 {}, ptr {}", indent, nw, t_gep).ok();
}

/// Emit inline RingBuffer pop: result = data[head & mask]; head = (head+1) & mask.
/// Returns the register name holding the popped value (0 if empty).
/// If `target` is None, the value is discarded (no store to target variable).
fn emit_ring_pop(backend: &mut LlvmBackend, out: &mut String, indent: &str,
    source: &Expr, _target: Option<&str>) -> String {
    let name = source.as_var_name().and_then(|n| {
        backend.ctx.ringbuf_inline.get(n).map(|rbi| (n.to_string(), rbi.clone()))
    });
    let Some((_name, rbi)) = name else { return "0".to_string() };
    let h_gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
        indent, h_gep, rbi.head_idx).ok();
    let h_val = backend.fun.gen_reg();
    writeln!(out, "{}{} = load i64, ptr {}", indent, h_val, h_gep).ok();
    let t_gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
        indent, t_gep, rbi.tail_idx).ok();
    let t_val = backend.fun.gen_reg();
    writeln!(out, "{}{} = load i64, ptr {}", indent, t_val, t_gep).ok();
    let m_gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
        indent, m_gep, rbi.mask_idx).ok();
    let m_val = backend.fun.gen_reg();
    writeln!(out, "{}{} = load i64, ptr {}", indent, m_val, m_gep).ok();
    let empty = backend.fun.gen_reg();
    writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, empty, h_val, t_val).ok();
    let d_gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
        indent, d_gep, rbi.data_idx).ok();
    let d_val = backend.fun.gen_reg();
    writeln!(out, "{}{} = load i64, ptr {}", indent, d_val, d_gep).ok();
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, d_val).ok();
    let idx = backend.fun.gen_reg();
    writeln!(out, "{}{} = and i64 {}, {}", indent, idx, h_val, m_val).ok();
    let gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, gep, ptr, idx).ok();
    let raw_val = backend.fun.gen_reg();
    writeln!(out, "{}{} = load i64, ptr {}", indent, raw_val, gep).ok();
    let nh = backend.fun.gen_reg();
    writeln!(out, "{}{} = add i64 {}, 1", indent, nh, h_val).ok();
    let nw = backend.fun.gen_reg();
    writeln!(out, "{}{} = and i64 {}, {}", indent, nw, nh, m_val).ok();
    let sel = backend.fun.gen_reg();
    writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}", indent, sel, empty, h_val, nw).ok();
    writeln!(out, "{}store i64 {}, ptr {}", indent, sel, h_gep).ok();
    let result = backend.fun.gen_reg();
    writeln!(out, "{}{} = select i1 {}, i64 0, i64 {}", indent, result, empty, raw_val).ok();
    result
}
