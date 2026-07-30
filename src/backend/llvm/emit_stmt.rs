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
                    let llvm_ty = ty.as_ref().map(|t| backend.llvm_type(t)).unwrap_or("i64".into());
                    writeln!(out, "{}{} = alloca {}", indent, v, llvm_ty).ok();
                    TypedRegister { name: v, ty: ty.clone().unwrap_or(Type::int()) }
                }
            };
            // 2026-07-18: Track alloca bindings so identifier codegen loads values.
            if expr.is_none() {
                backend.fun.let_binding_allocas.insert(val.name.clone());
            }
            // 2026-07-24: Transfer struct literal alloca tracking from result
            // register to variable name, so &let_var retrieves the stack address.
            if let Some(alloca) = backend.fun.struct_literal_allocas.remove(&val.name) {
                backend.fun.struct_literal_allocas.insert(name.clone(), alloca);
            }
            backend.fun.let_bindings.insert(name.clone(), val.name.clone());
            backend.fun.let_binding_types.insert(name.clone(), val.ty.clone());
            TypedRegister { name: val.name, ty: Type::void() }
        }
        Statement::Assign(lhs, rhs) => {
            // 2026-07-17: Pop: `x <- &queue` → Assign(Identifier(x), AddrOf(source)).
            // Detect this pattern BEFORE emitting the RHS (which would get the
            // address, not the popped value). Emit the ring buffer pop directly.
            // 2026-07-18: Pop — emit call @fn_name(handle), store result to lhs.
            // 2026-07-20: Uses find_extract_strategy (reads OperatorDef from context).
            if let Expr::AddrOf(source) = rhs {
                let strat = backend.find_extract_strategy(source)
                    .or_else(|| backend.find_extract_strategy(rhs));
                let Some(op_def) = strat else {
                    return TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() };
                };
                let Some(result) = emit_strategy_fn_call(backend, out, indent, source, &op_def.clone(), None) else {
                    return TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() };
                };
                // Store popped result to the LHS variable.
                let Expr::Identifier(name) = lhs else {
                    return TypedRegister { name: result, ty: Type::int() };
                };
                if let Some(reg) = backend.fun.let_bindings.get(name) {
                    writeln!(out, "{}store i64 {}, ptr {}", indent, result, reg).ok();
                } else if backend.ctx.field_index_map.contains_key(name) {
                    backend.emit_state_store_i64(out, indent, name, &result);
                }
                return TypedRegister { name: result, ty: Type::int() };
            }

            let val = backend.emit_expr(out, rhs, indent);
            match lhs {
                Expr::Identifier(name) => {
                    // 2026-07-18: Push — emit call @fn_name(handle, val).
                    // 2026-07-20: Uses find_insert_strategy (reads OperatorDef from context).
                    let insert_strat = backend.find_insert_strategy(lhs).cloned();
                    if let Some(op_def) = &insert_strat {
                        emit_strategy_fn_call(backend, out, indent, lhs, op_def, Some(&val.name));
                    } else if let Some(reg) = backend.fun.let_bindings.get(name).cloned() {
                        // 2026-07-18: If the binding is a value register (not an alloca),
                        // the variable is being mutated — create an alloca and redirect.
                        let is_alloca = backend.fun.let_binding_allocas.contains(&reg)
                            || backend.fun.param_slots.values().any(|s| s == &reg);
                        let slot = if is_alloca {
                            reg
                        } else {
                            let slot = backend.fun.gen_reg();
                            writeln!(out, "{}{} = alloca i64, align 8", indent, slot).ok();
                            writeln!(out, "{}store i64 {}, ptr {}", indent, reg, slot).ok();
                            backend.fun.let_bindings.insert(name.clone(), slot.clone());
                            backend.fun.let_binding_allocas.insert(slot.clone());
                            slot
                        };
                        let store_ty = backend.llvm_type(&val.ty);
                        writeln!(out, "{}store {} {}, ptr {}", indent, store_ty, val.name, slot).ok();
                    // 2026-07-14: Handle MMIO and regular state field assignments
                    } else if let Some(&addr) = backend.ctx.mmio_fields.get(name) {
                        let ptr = backend.fun.gen_reg();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
                        // 2026-07-14: volatile store type must match val.ty — hardcoded i64 breaks MMIO bools
                        let store_ty = backend.llvm_type(&val.ty);
                        writeln!(out, "{}store volatile {} {}, ptr {}", indent, store_ty, val.name, ptr).ok();
                    } else if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                        let ptr = backend.emit_state_gep(out, indent, "as", "%state", idx);
                        // 2026-07-19: Store with native type from field_types.
                        // When the value's LLVM type matches the field type, store
                        // directly (no boxing). Otherwise box via adapt_to_i64.
                        let field_ty = &backend.ctx.field_types[idx];
                        let val_ty = backend.llvm_type(&val.ty);
                        if val_ty == *field_ty {
                            writeln!(out, "{}store {} {}, ptr {}", indent, field_ty, val.name, ptr).ok();
                        } else {
                            let boxed = backend.adapt_to_i64(out, indent, &val);
                            writeln!(out, "{}store i64 {}, ptr {}", indent, boxed, ptr).ok();
                        }
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
                    let store_ty = backend.llvm_type(&val.ty);
                    // 2026-07-30: Ptr values are stored as i64 internally;
                    // convert back to LLVM ptr before storing through.
                    let store_ptr = if matches!(ptr_reg.ty, Type::Ptr(_)) {
                        let p = backend.fun.gen_reg();
                        backend.emit_inttoptr(out, indent, &p, &ptr_reg.name);
                        p.to_string()
                    } else {
                        ptr_reg.name.clone()
                    };
                    writeln!(out, "{}store {} {}, ptr {}", indent, store_ty, val.name, store_ptr).ok();
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
            // 2026-07-20: Uses find_extract_strategy (reads OperatorDef from context).
            if let Expr::AddrOf(source) = expr {
                let strat = backend.find_extract_strategy(source)
                    .or_else(|| backend.find_extract_strategy(expr));
                if let Some(op_def) = strat {
                    emit_strategy_fn_call(backend, out, indent, source, &op_def.clone(), None);
                }
                TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
            } else {
                backend.emit_expr(out, expr, indent)
            }
        }
        Statement::Term(val) | Statement::TermBang(val) => {
            // 2026-07-26: Phase 4 — webstack flush at term.
            // Emit __web_flush_state call before the return/branch so the
            // JS shim applies DOM updates before the transaction completes.
            // Phase 6 will wire the actual flush buffer with modified fields.
            if backend.ctx.webstack_enabled {
                writeln!(out, "{}call void @__web_flush_state(i32 0, i32 0)", indent).ok();
            }
            if let Some(val) = val {
                let reg = backend.emit_expr(out, val, indent);
                if backend.fun.callable_txn_result.is_some() {
                    // 2026-07-18: In a callable txn, term stores to %result and
                    // branches to post (convergence loop). The 'ret' is at done:.
                    let val_ty = backend.llvm_type(&reg.ty);
                    let store_name = if val_ty != backend.fun.fn_ret_ty {
                        if val_ty == "i64" && backend.fun.fn_ret_ty == "ptr" {
                            let c = backend.fun.gen_reg();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, c, reg.name).ok();
                            c
                        } else if val_ty == "ptr" && backend.fun.fn_ret_ty == "i64" {
                            let c = backend.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, c, reg.name).ok();
                            c
                        } else {
                            reg.name
                        }
                    } else {
                        reg.name
                    };
                    if let Some(ref result_slot) = backend.fun.callable_txn_result {
                        writeln!(out, "{}store {} {}, ptr {}", indent, backend.fun.fn_ret_ty, store_name, result_slot).ok();
                    }
                    if let Some(ref post_label) = backend.fun.callable_txn_post_label {
                        writeln!(out, "{}br label %{}", indent, post_label).ok();
                    }
                    backend.fun.terminated = true;
                } else if backend.fun.fn_ret_ty != "void" {
                    // 2026-07-26: Use actual expression LLVM type, not hardcoded "i64".
                    // Frgn calls may return ptr (for String/Data in C ABI).
                    let val_ty = backend.llvm_type(&reg.ty);
                    let final_name = if val_ty != backend.fun.fn_ret_ty {
                        // 2026-07-20: Insert type conversion when the expression type doesn't
                        // match the function's declared return type (e.g., SysCall# returns i64
                        // but function returns ptr → need inttoptr).
                        if val_ty == "i64" && backend.fun.fn_ret_ty == "ptr" {
                            let c = backend.fun.gen_reg();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, c, reg.name).ok();
                            c
                        } else if val_ty == "ptr" && backend.fun.fn_ret_ty == "i64" {
                            let c = backend.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, c, reg.name).ok();
                            c
                        } else {
                            reg.name
                        }
                    } else {
                        reg.name
                    };
                    writeln!(out, "{}ret {} {}", indent, backend.fun.fn_ret_ty, final_name).ok();
                    backend.fun.terminated = true;
                } else {
                    backend.fun.terminated = true;
                }
            } else {
                backend.fun.terminated = true;
            }
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        Statement::Return(val) => {
            if let Some(val) = val {
                let reg = backend.emit_expr(out, val, indent);
                let val_ty = backend.llvm_type(&reg.ty);
                let final_name = if val_ty != backend.fun.fn_ret_ty {
                    if val_ty == "i64" && backend.fun.fn_ret_ty == "ptr" {
                        let c = backend.fun.gen_reg();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, c, reg.name).ok();
                        c
                        } else if val_ty == "ptr" && backend.fun.fn_ret_ty == "i64" {
                            let c = backend.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, c, reg.name).ok();
                            c
                        } else {
                        reg.name
                    }
                } else {
                    reg.name
                };
                writeln!(out, "{}ret {} {}", indent, backend.fun.fn_ret_ty, final_name).ok();
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
            backend.fun.terminated = false;
            for stmt in body {
                emit_statement(backend, out, stmt, indent);
            }
            // 2026-07-19: Always emit br to end label — the guard body always
            // converges to guard.endN regardless of term! or term; inside it.
            // Previously this was conditional on !terminated, but term! inside
            // a guard body sets terminated=true without emitting a real LLVM
            // terminator, leaving the guard.thenN block dangling. The branch
            // is harmless dead code if the body already has a ret.
            writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            writeln!(out, "{}{}:", indent, end_lbl).ok();
            backend.fun.terminated = false;
            backend.fun.terminated = false;
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
            backend.fun.terminated = false;
            for stmt in then {
                emit_statement(backend, out, stmt, indent);
            }
            if !backend.fun.terminated {
                writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            }
            writeln!(out, "{}{}:", indent, else_lbl).ok();
            backend.fun.terminated = false;
            for stmt in else_ {
                emit_statement(backend, out, stmt, indent);
            }
            // 2026-07-18: Always emit end label (referenced by br i1 false branch
            // and/or then->end and else->end branches).
            if !backend.fun.terminated {
                writeln!(out, "{}br label %{}", indent, end_lbl).ok();
            }
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
        Statement::Gate(cond) => {
            // 2026-07-26: Convergence gate — if cond is true, continue;
            // otherwise branch to convergence_target (the loop header for retry).
            let cond_reg = backend.emit_expr(out, cond, indent);
            let label_n = backend.fun.txn_counter;
            backend.fun.txn_counter += 1;
            let pass_lbl = format!("gate.pass{}", label_n);
            // 2026-07-30: In a defn (no convergence target), assertions that fail
            // trap via unreachable. In a txn, they branch back to the loop header.
            let has_convergence = backend.fun.convergence_target.is_some();
            let fail_target = if has_convergence {
                backend.fun.convergence_target.as_ref().unwrap().clone()
            } else {
                format!("gate.fail{}", label_n)
            };
            let cond_i1 = if cond_reg.ty == Type::bool_() {
                let b = backend.fun.gen_reg();
                writeln!(out, "{}{} = trunc i8 {} to i1", indent, b, cond_reg.name).ok();
                b
            } else {
                let i1_name = format!("%gi1_{}", label_n);
                writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1_name, cond_reg.name).ok();
                i1_name
            };
            writeln!(out, "{0}br i1 {1}, label %{2}, label %{3}",
                indent, cond_i1, pass_lbl, fail_target).ok();
            if !has_convergence {
                // Defn body: assertion failure traps via unreachable
                writeln!(out, "{}{}:", indent, fail_target).ok();
                writeln!(out, "{}  unreachable", indent).ok();
            }
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cond_i1, pass_lbl, target).ok();
            writeln!(out, "{}{}:", indent, pass_lbl).ok();
            backend.fun.terminated = false;
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
        _ => {
            TypedRegister { name: backend.fun.gen_reg(), ty: Type::void() }
        }
    }
}

/// Resolve a strategy property value to a function name and argument markers,
/// compute the handle (pointer to the collection storage), and emit a
/// generic call @fn_name(arg1, arg2, ...) where args are resolved from markers.
/// 2026-07-18: Generic dispatch — no hardcoded function names.
/// 2026-07-20: Handle both ringbuf-inline types (via ringbuf_inline data_idx) and
///   non-ringbuf types (via field_index_map or let_binding slot). Any type declaring
///   InsertAt/ExtractFrom in operator_defs gets the same <- behavior.
/// Supports: PropertyValue::Identifier("ring_push") for convention-based dispatch,
///   and PropertyValue::List([Identifier("ring_push"), HashL, HashR]) for
///   explicit marker-based dispatch like InsertAt <~ ring_push(#L, #R).
fn emit_strategy_fn_call(backend: &mut LlvmBackend, out: &mut String, indent: &str,
    target: &Expr, op_def: &crate::ast::top::OperatorDef, value: Option<&str>) -> Option<String> {
    let pv = op_def.impl_args.as_ref()?;
    let (fn_name, markers): (&str, &[crate::ast::PropertyValue]) = match pv {
        crate::ast::PropertyValue::Identifier(s) => {
            const EMPTY: &[crate::ast::PropertyValue] = &[];
            (s.as_str(), EMPTY)
        }
        crate::ast::PropertyValue::List(items) => {
            let fn_ident = match items.first()? {
                crate::ast::PropertyValue::Identifier(f) => f.as_str(),
                _ => return None,
            };
            (fn_ident, &items[1..])
        }
        _ => return None,
    };
    let var_name = target.as_var_name()?;

    // 2026-07-20: Compute handle as ptrtoint of the variable's storage location.
    // For RingBuf-inline types, use the data buffer field. For all other types,
    // derive the handle from the state field or let-binding alloca.
    let handle = if let Some(rbi) = backend.ctx.ringbuf_inline.get(var_name) {
        let gep = backend.emit_state_gep(out, indent, "hnd", "%state", rbi.data_idx);
        let h = backend.fun.gen_reg();
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, h, gep).ok();
        h
    } else if let Some(&idx) = backend.ctx.field_index_map.get(var_name) {
        let gep = backend.emit_state_gep(out, indent, "hnd", "%state", idx);
        let h = backend.fun.gen_reg();
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, h, gep).ok();
        h
    } else if let Some(slot) = backend.fun.let_bindings.get(var_name).cloned() {
        // Let-binding — use alloca pointer address as handle.
        let h = backend.fun.gen_reg();
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, h, slot).ok();
        h
    } else {
        return None; // No storage location found — can't compute handle
    };

    // Resolve markers to argument registers. Convention-based dispatch (no markers)
    // passes (handle, value) for push and (handle) for pop. Marker-based dispatch
    // resolves each marker to the corresponding register.
    let args: Vec<String> = if markers.is_empty() {
        // Convention-based: push = (handle, value), pop = (handle)
        match value {
            Some(val) => vec![handle.clone(), val.to_string()],
            None => vec![handle.clone()],
        }
    } else {
        // Marker-based: resolve #L, #R, #T to actual registers
        markers.iter().map(|m| match m {
            crate::ast::PropertyValue::HashL => handle.clone(),
            crate::ast::PropertyValue::HashR => value.map(|v| v.to_string()).unwrap_or(handle.clone()),
            crate::ast::PropertyValue::HashT => "1".to_string(), // placeholder — element type
            _ => handle.clone(),
        }).collect()
    };

    let args_str = args.join(", ");
    let result = backend.fun.gen_reg();
    writeln!(out, "{}{} = call i64 @{}({})", indent, result, fn_name, args_str).ok();
    Some(result)
}
