use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use crate::features::traits::*;
use std::collections::HashMap;
use std::fmt::Write;

impl LlvmBackend {
    /// Store a native-typed value to the i64 result slot, boxing if needed.
    fn store_i64_result(&mut self, out: &mut String, indent: &str, r: &TypedRegister, rs: &str) {
        let adapted = self.adapt_to_i64(out, indent, r);
        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, adapted, rs).ok();
    }

    /// Box a native-typed value to i64 for return/store, returning the adapted SSA name.
    pub(super) fn adapt_to_i64(&mut self, out: &mut String, indent: &str, r: &TypedRegister) -> String {
        if r.ty == Type::Bool {
            let z = format!("%rz{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = zext i1 {} to i64", indent, z, r.name).ok();
            z
        } else if r.ty == Type::Char {
            // All Char registers from emit_expr are already i64 (boxed).
            // No zext needed — the register is already the right width.
            r.name.clone()
        } else if r.ty == Type::String || r.ty == Type::Data {
            let p = format!("%rp{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, p, r.name).ok();
            p
        // 2026-06-20: Check reg_float_cache before bitcasting — guarantees correctness if
        // the register name is i64-boxed but Type::Float (e.g. intrinsic float returns,
        // callable txn param marshaling). The cache maps i64 register names to their native
        // float counterpart. Without this check, bitcast float %i64_reg causes LLVM verifier
        // errors. See docs/plans/2026-06-20-float-boxing-dual-path-plan.md.
        } else if r.ty == Type::Float {
            if let Some(cached) = self.reg_float_cache.get(&r.name) {
                let bi = format!("%rbi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, cached).ok();
                let ze = format!("%rze{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                ze
            } else {
                let bi = format!("%rbi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, r.name).ok();
                let ze = format!("%rze{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                ze
            }
        } else {
            r.name.clone()
        }
    }

    pub(crate) fn emit_stmt(&mut self, out: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Term { values, swan_song, .. } => {
                // Async/await barrier: wait for all pending async_await calls
                if self.pending_async_await_count > 0 {
                    writeln!(out, "{}call void @__barrier_wait__()", indent).ok();
                }
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(swan) = swan_song {
                    self.emit_stmt(out, swan, indent);
                }
                if self.in_callable_txn {
                    // Store value to result slot, branch to post label
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        if let Some(rs) = self.callable_txn_result.clone() {
                            self.store_i64_result(out, indent, &r, &rs);
                        }
                    }
                    if let Some(ref pl) = self.callable_txn_post_label {
                        writeln!(out, "{}br label %{}", indent, pl).ok();
                    }
                } else {
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        if self.fn_ret_ty == "i32" {
                            if r.ty == Type::Bool {
                                let z = format!("%rz{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = zext i1 {} to i32", indent, z, r.name).ok();
                                writeln!(out, "{}ret i32 {}", indent, z).ok();
                            } else if r.ty == Type::Char {
                                writeln!(out, "{}ret i32 {}", indent, r).ok();
                            } else {
                                let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, r.name).ok();
                                writeln!(out, "{}ret i32 {}", indent, tr).ok();
                            }
                        } else if self.fn_ret_ty == "float" {
                            let fl = self.ensure_float_reg(out, indent, &r);
                            writeln!(out, "{}ret float {}", indent, fl).ok();
                        } else if self.fn_ret_ty == "i64" {
                            let adapted = self.adapt_to_i64(out, indent, &r);
                            writeln!(out, "{}ret i64 {}", indent, adapted).ok();
                        } else {
                            let adapted = self.adapt_to_i64(out, indent, &r);
                            writeln!(out, "{}ret i64 {}", indent, adapted).ok();
                        }
                    } else if self.fn_ret_ty == "i32" {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else if self.fn_ret_ty == "float" {
                        writeln!(out, "{}ret float 0.0", indent).ok();
                    } else if self.returns_i64 {
                        writeln!(out, "{}ret i64 0", indent).ok();
                    } else if self.main_body {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else {
                        writeln!(out, "{}ret void", indent).ok();
                    }
                    self.terminated = true;
                }
            }
            Statement::TermBang { values, swan_song, .. } => {
                // Async/await barrier: wait for all pending async_await calls
                if self.pending_async_await_count > 0 {
                    writeln!(out, "{}call void @__barrier_wait__()", indent).ok();
                }
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(swan) = swan_song {
                    self.emit_stmt(out, swan, indent);
                }
                if self.in_callable_txn {
                    // Store value to result slot, branch to post label
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        if let Some(rs) = self.callable_txn_result.clone() {
                            self.store_i64_result(out, indent, &r, &rs);
                        }
                    }
                    if let Some(ref pl) = self.callable_txn_post_label {
                        writeln!(out, "{}br label %{}", indent, pl).ok();
                    }
                } else if let Some(exit_label) = self.loop_exit_label.clone() {
                    // Inside a reactive transaction loop — branch to exit label
                    // instead of ret, so LLVM can unroll the loop.
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        self.store_i64_result(out, indent, &r, "%state");
                    }
                    writeln!(out, "{}br label %{}", indent, exit_label).ok();
                    self.terminated = true;
                } else {
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        if self.fn_ret_ty == "i32" {
                            let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, r).ok();
                            writeln!(out, "{}ret i32 {}", indent, tr).ok();
                        } else if self.fn_ret_ty == "i64" {
                            writeln!(out, "{}ret i64 {}", indent, r).ok();
                        } else if self.is_embedded {
                            writeln!(out, "{}store i64 {}, ptr %state", indent, r).ok();
                            writeln!(out, "{}call void asm sideeffect \"wfi\", \"\"()", indent).ok();
                            writeln!(out, "{}ret void", indent).ok();
                        } else if self.main_body {
                            writeln!(out, "{}ret i32 0", indent).ok();
                        } else {
                            writeln!(out, "{}ret void", indent).ok();
                        }
                    } else if self.fn_ret_ty == "i32" {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else if self.returns_i64 {
                        writeln!(out, "{}ret i64 0", indent).ok();
                    } else if self.is_embedded {
                        writeln!(out, "{}call void asm sideeffect \"wfi\", \"\"()", indent).ok();
                        writeln!(out, "{}ret void", indent).ok();
                    } else if self.main_body {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else {
                        writeln!(out, "{}ret void", indent).ok();
                    }
                    self.terminated = true;
                }
            }
            Statement::Escape(e) => {
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if self.in_callable_txn {
                    if let Some(v) = e {
                        let r = self.emit_expr(out, v, indent);
                        if let Some(rs) = self.callable_txn_result.clone() {
                            self.store_i64_result(out, indent, &r, &rs);
                        }
                    }
                    if let Some(ref pl) = self.callable_txn_post_label {
                        writeln!(out, "{}br label %{}", indent, pl).ok();
                    }
                } else {
                    if let Some(v) = e {
                        let r = self.emit_expr(out, v, indent);
                        if self.fn_ret_ty == "i32" {
                            let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, r).ok();
                            writeln!(out, "{}ret i32 {}", indent, tr).ok();
                        } else {
                            writeln!(out, "{}ret i64 {}", indent, r).ok();
                        }
                    } else if self.fn_ret_ty == "i32" {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else if self.returns_i64 {
                        writeln!(out, "{}ret i64 0", indent).ok();
                    } else if self.main_body {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else {
                        writeln!(out, "{}ret void", indent).ok();
                    }
                    self.terminated = true;
                }
            }
            Statement::Let { name, expr, ty, address_expr, .. } => {
                // Handle TupleDestructure: extract tuple elements and bind each name
                if let Some(Expr::TupleDestructure(names, tuple_expr)) = expr {
                    let tuple_val = self.emit_expr(out, tuple_expr, indent);
                    let hp = format!("%tdh{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, tuple_val.name).ok();
                    for (i, n) in names.iter().enumerate() {
                        let ep = format!("%tde{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                        let val = format!("%tdr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, val, ep).ok();
                        self.let_bindings.insert(n.clone(), val.clone());
                    }
                    return;
                }
                if let Some(e) = expr {
                    let r = self.emit_expr(out, e, indent);
                    // 2026-06-17: Emit type conversion when annotation differs from emitted type.
                    // e.g. `let c: Char = s[pos]` — s[pos] loads i64 (Type::Int) but annotation
                    // is Type::Char (i32 native). Without the trunc, adapt_to_i64 would double-
                    // zext i64→zext i32 i64, producing invalid LLVM IR.
                    if let Some(ann_ty) = ty.as_ref() {
                        if *ann_ty != r.ty {
                            match (ann_ty, &r.ty) {
                                (Type::Char, Type::Int | Type::UInt) => {
                                    let cv = format!("%clv{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, cv, r.name).ok();
                                    self.let_bindings.insert(name.clone(), cv.clone());
                                    self.let_binding_types.insert(name.clone(), ann_ty.clone());
                                    writeln!(out, "{}; let {} = {}", indent, name, cv).ok();
                                    return;
                                }
                                _ => {}
                            }
                        }
                    }
                    self.let_bindings.insert(name.clone(), r.name.clone());
                    let resolved_ty = ty.clone().unwrap_or_else(|| r.ty.clone());
                    self.let_binding_types.insert(name.clone(), resolved_ty);
                    writeln!(out, "{}; let {} = {}", indent, name, r).ok();
                } else {
                    writeln!(out, "{}; let {} = undef", indent, name).ok();
                }
            }
            Statement::Assignment { lhs, expr, modifiers, .. } => {
                let val = self.emit_expr(out, expr, indent);
                let fname = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                    Expr::ListIndex(list_expr, index_expr) => {
                        let val_reg = val.name.clone();
                        let list_name = match &**list_expr {
                            Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                            _ => { writeln!(out, "{}; assign list[idx] = {}", indent, val_reg).ok(); return; }
                        };
                        let idx_val = self.emit_expr(out, index_expr, indent);
                        // Resolve the list pointer from state (SSA or non-SSA) or let bindings
                        let list_ptr: Option<String> =
                            if let Some(ref ssa_reg) = self.ssa_state_reg.clone() {
                                if let Some(&field_idx) = self.field_index_map.get(&list_name) {
                                    let ev = format!("%lev{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, field_idx).ok();
                                    Some(ev)
                                } else if let Some(reg) = self.let_bindings.get(&list_name).cloned() {
                                    Some(reg)
                                } else {
                                    None
                                }
                            } else if let Some(reg) = self.let_bindings.get(&list_name).cloned() {
                                Some(reg)
                            } else if let Some(&field_idx) = self.field_index_map.get(&list_name) {
                                let p = format!("%lgp{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, field_idx).ok();
                                let ld = format!("%lld{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, p).ok();
                                Some(ld)
                            } else {
                                None
                            };
                        let Some(list_ptr) = list_ptr else {
                            writeln!(out, "{}; assign list[idx] = {} (unknown list '{}')", indent, val_reg, list_name).ok();
                            return;
                        };
                        let hp = format!("%lhp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_ptr).ok();
                        let dp = format!("%ldp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                        let de = format!("%lde{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                        let ep = format!("%lep{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, de, idx_val.name).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, val_reg, ep).ok();
                        return;
                    }
                    Expr::TupleDestructure(names, _) => {
                        let hp = format!("%tdh{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, val).ok();
                        for (i, name) in names.iter().enumerate() {
                            let ep = format!("%tde{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                            let elem = format!("%tdr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, elem, ep).ok();
                            // Store to variable — same patterns as single-variable assignment
                            if let Some(ref ssa_reg) = self.ssa_state_reg.clone() {
                                if let Some(&idx) = self.field_index_map.get(name) {
                                    let new_reg = format!("%in{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = insertvalue %State {}, i64 {}, {}", indent, new_reg, ssa_reg, elem, idx).ok();
                                    self.ssa_state_reg = Some(new_reg);
                                    continue;
                                }
                            }
                            if let Some(&addr) = self.mmio_fields.get(name) {
                                let p = format!("%mio{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                                writeln!(out, "{}store volatile i64 {}, i64* {}, align 1", indent, elem, p).ok();
                                continue;
                            }
                            if let Some(trg) = self.triggers.get(name) {
                                if trg.is_const {
                                    writeln!(out, "{}; error: cannot write to const trigger '{}'", indent, name).ok();
                                    continue;
                                }
                            }
                            if let Some(&idx) = self.field_index_map.get(name) {
                                let ty = self.field_types[idx].clone();
                                let p = format!("%ap{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                                match ty.as_str() {
                                    "i8" => {
                                        let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, elem).ok();
                                        writeln!(out, "{}store i8 {}, i8* {}, align {}", indent, tr, p, self.align_of(&ty)).ok();
                                    }
                                    "float" => {
                                        let fl = self.native_float_or_box(out, indent, &elem.to_string());
                                        writeln!(out, "{}store float {}, float* {}, align {}", indent, fl, p, self.align_of(&ty)).ok();
                                    }
                                    _ => {
                                        writeln!(out, "{}store {} {}, {}* {}, align {}", indent, ty, elem, ty, p, self.align_of(&ty)).ok();
                                    }
                                }
                            } else if let Some(slot) = self.param_slots.get(name) {
                                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, elem, slot).ok();
                                self.let_bindings.insert(name.clone(), elem.clone());
                            } else {
                                self.let_bindings.insert(name.clone(), elem.clone());
                                writeln!(out, "{}; tuple elem assign {} to {}", indent, elem, name).ok();
                            }
                        }
                        return;
                    }
                    _ => { writeln!(out, "{}; assign {}", indent, val).ok(); return; }
                };
                let is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                // const trg check: reject writes to const triggers
                if let Some(trg) = self.triggers.get(&fname) {
                    if trg.is_const {
                        writeln!(out, "{}; error: cannot write to const trigger '{}'", indent, fname).ok();
                        return;
                    }
                }
                // SSA mode: use insertvalue instead of GEP + store
                if let Some(ssa_reg) = self.ssa_state_reg.clone() {
                    if let Some(&idx) = self.field_index_map.get(&fname) {
                        if !is_volatile {
                            let ty = self.field_types[idx].clone();
                            let new_reg = format!("%in{}", self.txn_counter); self.txn_counter += 1;
                            let val_boxed = self.adapt_to_i64(out, indent, &val);
                            match ty.as_str() {
                                "i8" => {
                                    let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val_boxed).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i8 {}, {}", indent, new_reg, ssa_reg, tr, idx).ok();
                                }
                                "i32" => {
                                    let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val_boxed).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i32 {}, {}", indent, new_reg, ssa_reg, tr, idx).ok();
                                }
                                "float" => {
                                    let fl = self.native_float_or_box(out, indent, &val.to_string());
                                    writeln!(out, "{}{} = insertvalue %State {}, float {}, {}", indent, new_reg, ssa_reg, fl, idx).ok();
                                }
                                "i8*" => {
                                    let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, val_boxed).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i8* {}, {}", indent, new_reg, ssa_reg, p, idx).ok();
                                }
                                _ => {
                                    writeln!(out, "{}{} = insertvalue %State {}, i64 {}, {}", indent, new_reg, ssa_reg, val_boxed, idx).ok();
                                }
                            }
                            // 2026-06-17: Re-extract written field so intra-body reads
                            // see the updated value. Without this, subsequent reads of
                            // a field written earlier in the same body use the pre-tick
                            // value from pre_extract_int_fields, not the freshly stored
                            // value — causes incorrect results when a field is both
                            // written and read in the same txn body (e.g. fannkuch_redux_sym:
                            // &seed = p0 → checksum + seed % 13 reads p0_old, not seed_old).
                            if ty != "float" {
                                let re = format!("%re_{}_{}", fname, self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, re, new_reg, idx).ok();
                                self.ssa_old_int_regs.insert(fname.clone(), re);
                            }
                            self.ssa_state_reg = Some(new_reg);
                            return;
                        }
                    }
                }
                if let Some(&addr) = self.mmio_fields.get(&fname) {
                    let p = format!("%mio{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                    writeln!(out, "{}store volatile i64 {}, i64* {}, align 1", indent, val, p).ok();
                    return;
                }
                if let Some(&idx) = self.field_index_map.get(&fname) {
                    let ty = self.field_types[idx].clone();
                    let p = format!("%ap{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    let vol_str = if is_volatile { " volatile" } else { "" };
                    let val_boxed = self.adapt_to_i64(out, indent, &val);
                    let tn = crate::backend::llvm::tbaa_node(&ty);
                    match ty.as_str() {
                        "i8" => {
                            let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val_boxed).ok();
                            writeln!(out, "{}store{} i8 {}, i8* {}, align {}, !tbaa !{}", indent, vol_str, tr, p, self.align_of(&ty), tn).ok();
                        }
                        "i32" => {
                            let tr = format!("%tri{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val_boxed).ok();
                            writeln!(out, "{}store{} i32 {}, i32* {}, align {}, !tbaa !{}", indent, vol_str, tr, p, self.align_of(&ty), tn).ok();
                        }
                        "float" => {
                            let fl = self.native_float_or_box(out, indent, &val.to_string());
                            writeln!(out, "{}store{} float {}, float* {}, align {}, !tbaa !{}", indent, vol_str, fl, p, self.align_of(&ty), tn).ok();
                        }
                        s if s == "i8*" || s == "ptr" => {
                            let fp = format!("%fp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, fp, val_boxed).ok();
                            writeln!(out, "{}store{} i8* {}, i8** {}, align {}, !tbaa !{}", indent, vol_str, fp, p, self.align_of(&ty), tn).ok();
                        }
                        _ => {
                            writeln!(out, "{}store{} {} {}, {}* {}, align {}, !tbaa !{}", indent, vol_str, ty, val_boxed, ty, p, self.align_of(&ty), tn).ok();
                        }
                    }
                } else if let Some(slot) = self.param_slots.get(&fname).cloned() {
                    let val_boxed = self.adapt_to_i64(out, indent, &val);
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, val_boxed, slot).ok();
                    // Update let_bindings so subsequent reads see the boxed i64 value
                    self.let_bindings.insert(fname.clone(), val_boxed.clone());
                    self.let_binding_types.insert(fname.clone(), Type::Int);
                } else {
                    self.let_bindings.insert(fname.clone(), val.name.clone());
                    self.let_binding_types.insert(fname.clone(), val.ty.clone());
                    writeln!(out, "{}; assign {} to {}", indent, val, fname).ok();
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                let cond = self.emit_expr(out, condition, indent);
                let i1 = if cond.ty == Type::Bool {
                    cond.name.clone()
                } else {
                    let i1 = format!("%gc{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();
                    i1
                };

                // Guard→select if single assignment (not in SSA mode — branch-based path handles insertvalue)
                if statements.len() == 1 && self.ssa_state_reg.is_none() {
                    if let Statement::Assignment { lhs, expr, modifiers, .. } = &statements[0] {
                        if let Expr::Identifier(n) | Expr::OwnedRef(n) = lhs {
                            if let Some(&idx) = self.field_index_map.get(n) {
                                let g_is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                                let gvol = if g_is_volatile { " volatile" } else { "" };
                                let p = format!("%gp{}", self.txn_counter); self.txn_counter += 1;
                                let av = self.emit_expr(out, expr, indent);
                                let ty = self.field_types[idx].clone();
                                writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                                let se = format!("%gs{}", self.txn_counter); self.txn_counter += 1;
                                match ty.as_str() {
                                    "i8" => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load i8, i8* {}, align {}", indent, ld, p, self.align_of(&ty)).ok();
                                        let av_boxed = self.adapt_to_i64(out, indent, &av);
                                        let av_tr = format!("%gatr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, av_tr, av_boxed).ok();
                                        writeln!(out, "{}{} = select i1 {}, i8 {}, i8 {}", indent, se, i1, av_tr, ld).ok();
                                        writeln!(out, "{}store{} i8 {}, i8* {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
                                    }
                                    "float" => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load float, float* {}, align {}", indent, ld, p, self.align_of(&ty)).ok();
                                        let av_fl = self.native_float_or_box(out, indent, &av.to_string());
                                        writeln!(out, "{}{} = select i1 {}, float {}, float {}", indent, se, i1, av_fl, ld).ok();
                                        writeln!(out, "{}store{} float {}, float* {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
                                    }
                                    _ => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, p).ok();
                                        // 2026-06-17: Box float to i64 for uniform i64 store
                                        let av_i64 = if av.ty == Type::Float {
                                            self.adapt_to_i64(out, indent, &av)
                                        } else {
                                            av.name.clone()
                                        };
                                        writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}", indent, se, i1, av_i64, ld).ok();
                                        writeln!(out, "{}store{} i64 {}, i64* {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
                                    }
                                }
                                return;
                    }
                }
            }
        }

                // Standard guarded block with unique labels
                let gid = format!("g{}", self.txn_counter); self.txn_counter += 1;
                let then_l = format!("{}_t", gid);
                let end_l = format!("{}_e", gid);
                let prev_terminated = self.terminated;
                self.terminated = false;

                // SSA mode: wrap guard in a named entry block so phi at merge
                // has a known predecessor label for the skip path.
                let ssa_pre_reg = self.ssa_state_reg.clone();
                let entry_l: String;
                if ssa_pre_reg.is_some() {
                    entry_l = format!("{}_ge", gid);
                    writeln!(out, "{}br label %{}", indent, entry_l).ok();
                    writeln!(out, "{}{}:", indent, entry_l).ok();
                } else {
                    entry_l = String::new();
                }

                let guard_id = format!("guard_{}", self.pgo_guard_idx);
                self.pgo_guard_idx += 1;
                if let Some(ref profile) = self.pgo_profile {
                    if let Some(prof) = crate::analysis::pgo::emit_branch_weights(profile, &guard_id) {
                        writeln!(out, "{}br i1 {}, label %{}, label %{}, {}", indent, i1, then_l, end_l, prof).ok();
                    } else {
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, then_l, end_l).ok();
                    }
                } else {
                    writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, then_l, end_l).ok();
                }
                writeln!(out, "{}{}:", indent, then_l).ok();
                // Save let bindings + types — values defined in the then-path
                // use SSA registers local to %then_l and don't dominate %end_l.
                let saved_bindings = self.let_bindings.clone();
                let saved_types = self.let_binding_types.clone();
                for s in statements { self.emit_stmt(out, s, &format!("{}  ", indent)); }
                self.let_bindings = saved_bindings;
                self.let_binding_types = saved_types;
                if !self.terminated {
                    // Emit a sentinel then-exit block so the phi at end_l:
                    // (a) has a single predecessor from the then-path (not then_l
                    //     directly — nested guards inside the body terminate then_l
                    //     before reaching end_l), and
                    // (b) the phi predecessor matches the actual last block.
                    let then_exit = format!("{}_tx", gid);
                    writeln!(out, "{}  br label %{}", indent, then_exit).ok();
                    writeln!(out, "{}{}:", indent, then_exit).ok();
                    writeln!(out, "{}  br label %{}", indent, end_l).ok();
                }
                writeln!(out, "{}{}:", indent, end_l).ok();
                if !self.terminated {
                    // SSA mode: phi merge at guard — the guard body may have
                    // modified state via insertvalue (only on the then path).
                    // Without a phi, the insertvalue result from %then_l would
                    // be undefined on the skip path. Use then_exit as predecessor.
                    if let Some(ref pre_reg) = ssa_pre_reg {
                        if let Some(ref post_reg) = self.ssa_state_reg {
                            if post_reg != pre_reg {
                                let then_exit = format!("{}_tx", gid);
                                let merge = format!("%me{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "  {} = phi %State [ {}, %{} ], [ {}, %{} ]",
                                    merge, post_reg, then_exit, pre_reg, entry_l);
                                self.ssa_state_reg = Some(merge);
                            }
                        }
                    }
                    // Clear stale old-value caches — guard may have modified state
                    // via insertvalue; pre-guard cached values are now incorrect.
                    self.ssa_old_int_regs.clear();
                    self.ssa_old_float_regs.clear();
                    self.terminated = prev_terminated;
                } else {
                    // Then-path terminated (e.g. term! → program exit).
                    // The else path at end_l continues the loop naturally —
                    // do NOT emit ret here. Restore prev_terminated so
                    // callers emit the continuation (br to loop back-edge).
                    self.terminated = prev_terminated;
                }}
            Statement::SyncBlock { body } => {
                for s in body { self.emit_stmt(out, s, indent); }
            }
            Statement::Unification { name, variant, fields, expr } => {
                // Save/restore bindings — pattern variable bindings from the arm
                // block must not leak past the merge block.
                let saved_bindings = self.let_bindings.clone();
                let saved_types = self.let_binding_types.clone();
                let val = self.emit_expr(out, &Expr::Identifier(name.clone()), indent);
                let disc = format!("%ud{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, 255", indent, disc, val).ok();
                let arm_l = format!("ua{}", self.txn_counter); self.txn_counter += 1;
                let def_l = format!("ud{}", self.txn_counter); self.txn_counter += 1;
                let merge_l = format!("um{}", self.txn_counter); self.txn_counter += 1;
                // 2026-06-17: Look up the VARIANT name (Ok, Err) in variant_disc,
                // not the VALUE name (json_res). Using name (the value) always
                // returned 0 (Ok's disc), so Err was never matched.
                let target = self.variant_disc.get(variant)
                    .map(|(_, d, _)| *d)
                    .unwrap_or(0);
                writeln!(out, "{}switch i64 {}, label %{} [ i64 {}, label %{} ]", indent, disc, def_l, target, arm_l).ok();
                writeln!(out, "{}{}:", indent, arm_l).ok();
                let pay = format!("%up{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = lshr i64 {}, 8", indent, pay, val).ok();
                // Bind pattern fields to the payload register
                bind_pattern_fields(&mut self.let_bindings, &mut self.let_binding_types, fields, &pay);
                let prev_terminated = self.terminated;
                let _ = self.emit_expr(out, expr, indent);
                if !self.terminated {
                    writeln!(out, "{}br label %{}", indent, merge_l).ok();
                } else {
                    self.terminated = false;
                    if !self.returns_i64 {
                        writeln!(out, "{}br label %{}", indent, merge_l).ok();
                    }
                }
                self.terminated = prev_terminated;
                writeln!(out, "{}{}:", indent, def_l).ok();
                writeln!(out, "{}  br label %{}", indent, merge_l).ok();
                writeln!(out, "{}{}:", indent, merge_l).ok();
                self.let_bindings = saved_bindings;
                self.let_binding_types = saved_types;
            }
            Statement::Expression(e) => { let _ = self.emit_expr(out, e, indent); }
            Statement::LocalTrigger { .. } => { writeln!(out, "{}; trg!", indent).ok(); }
            Statement::OnExit { body, .. } => { self.pending_cleanup.extend(body.iter().cloned()); }
            Statement::Alka(b) => { for l in b.content.lines() { let _ = writeln!(out, "{}{}", indent, l); } }
            Statement::InlineAsm { asm_string, .. } => { writeln!(out, "{}{}", indent, asm_string).ok(); }
            Statement::Foreach { item, list, body, modifiers } => {
                crate::features::stmt::foreach::ForeachStmt {
                    item: item.clone(),
                    list: list.clone(),
                    body: body.clone(),
                    modifiers: modifiers.clone(),
                }.emit_llvm(self, out, &StmtDispatch, indent);
            }
            Statement::Oracle { body, handler, .. } => {
                for s in body {
                    self.emit_stmt(out, s, indent);
                }
            }
            Statement::Await { expr, .. } => {
                // Emit call expression and capture result for subsequent use
                let reg = self.emit_expr(out, expr, indent);
                // Store result in a temp SSA value that subsequent statements can reference
                // The TypedRegister from emit_expr already points to the result value.
                // If the backend needs to reference it later via a named alloca:
                if !reg.name.is_empty() {
                    let temp_name = format!("%__await_result_{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(out, "{} = alloca i64, align 8", temp_name).ok();
                    writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, reg, temp_name).ok();
                }
            }
            Statement::Async { body, .. } => {
                // Fire-and-forget: emit body but discard any return value
                self.emit_stmt(out, body, indent);
            }
            Statement::AsyncAwait { body, lhs, .. } => {
                // Fork-join: emit body, optionally capture result, track barrier
                self.emit_stmt(out, body, indent);
                if let Some(name) = lhs {
                    writeln!(out, "{}; %{} = alloca i64, align 8", indent, name).ok();
                }
                self.pending_async_await_count += 1;
            }
        }
    }
}

/// Bind pattern fields from a Unification pattern to let_bindings.
fn bind_pattern_fields(
    let_bindings: &mut std::collections::HashMap<String, String>,
    let_binding_types: &mut std::collections::HashMap<String, Type>,
    fields: &[crate::ast::Pattern],
    payload_reg: &str,
) {
    for field in fields {
        match field {
            crate::ast::Pattern::Var(name) => {
                let_bindings.insert(name.clone(), payload_reg.to_string());
                let_binding_types.insert(name.clone(), Type::Int);
            }
            crate::ast::Pattern::Tuple(subfields) => {
                // For tuple patterns, bind each subfield to the payload
                for sub in subfields {
                    if let crate::ast::Pattern::Var(name) = sub {
                        let_bindings.insert(name.clone(), payload_reg.to_string());
                        let_binding_types.insert(name.clone(), Type::Int);
                    }
                }
            }
            _ => {} // Wildcard, literals — no binding
        }
    }
}

// ── EXPRESSIONS ───────────────────────────────────────────
