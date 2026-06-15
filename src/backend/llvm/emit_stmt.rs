use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use crate::features::traits::*;
use std::collections::HashMap;
use std::fmt::Write;

impl LlvmBackend {
    pub(crate) fn emit_stmt(&mut self, out: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Term { values, swan_song, .. } => {
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(swan) = swan_song {
                    self.emit_stmt(out, swan, indent);
                }
                if self.in_callable_txn {
                    // Store value to result slot, branch to post label
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        if let Some(ref rs) = self.callable_txn_result {
                            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, r, rs).ok();
                        }
                    }
                    if let Some(ref pl) = self.callable_txn_post_label {
                        writeln!(out, "{}br label %{}", indent, pl).ok();
                    }
                } else {
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        if self.fn_ret_ty == "i32" {
                            let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, r).ok();
                            writeln!(out, "{}ret i32 {}", indent, tr).ok();
                        } else if self.fn_ret_ty == "i64" {
                            writeln!(out, "{}ret i64 {}", indent, r).ok();
                        } else {
                            writeln!(out, "{}ret i64 {}", indent, r).ok();
                        }
                    } else if self.fn_ret_ty == "i32" {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else if self.returns_i64 {
                        writeln!(out, "{}ret i64 0", indent).ok();
                    } else {
                        writeln!(out, "{}ret void", indent).ok();
                    }
                    self.terminated = true;
                }
            }
            Statement::TermBang { values, swan_song, .. } => {
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(swan) = swan_song {
                    self.emit_stmt(out, swan, indent);
                }
                if self.in_callable_txn {
                    // Store value to result slot, branch to post label
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        if let Some(ref rs) = self.callable_txn_result {
                            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, r, rs).ok();
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
                        writeln!(out, "{}store i64 {}, i64* %state, align 8 ; term! value", indent, r).ok();
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
                        } else {
                            writeln!(out, "{}ret void", indent).ok();
                        }
                    } else if self.fn_ret_ty == "i32" {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else if self.returns_i64 {
                        writeln!(out, "{}ret i64 0", indent).ok();
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
                        if let Some(ref rs) = self.callable_txn_result {
                            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, r, rs).ok();
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
                    self.let_bindings.insert(name.clone(), r.name.clone());
                    // Use type annotation if available (preserves Ptr<T> etc), otherwise fall back to emitted type
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
                // SSA mode: use insertvalue instead of GEP + store
                if let Some(ssa_reg) = self.ssa_state_reg.clone() {
                    if let Some(&idx) = self.field_index_map.get(&fname) {
                        if !is_volatile {
                            let ty = self.field_types[idx].clone();
                            let new_reg = format!("%in{}", self.txn_counter); self.txn_counter += 1;
                            match ty.as_str() {
                                "i8" => {
                                    let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i8 {}, {}", indent, new_reg, ssa_reg, tr, idx).ok();
                                }
                                "float" => {
                                    let fl = self.native_float_or_box(out, indent, &val.to_string());
                                    writeln!(out, "{}{} = insertvalue %State {}, float {}, {}", indent, new_reg, ssa_reg, fl, idx).ok();
                                }
                                "i8*" => {
                                    let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, val).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i8* {}, {}", indent, new_reg, ssa_reg, p, idx).ok();
                                }
                                _ => {
                                    writeln!(out, "{}{} = insertvalue %State {}, i64 {}, {}", indent, new_reg, ssa_reg, val, idx).ok();
                                }
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
                    match ty.as_str() {
                        "i8" => {
                            let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val).ok();
                            writeln!(out, "{}store{} i8 {}, i8* {}, align {}", indent, vol_str, tr, p, self.align_of(&ty)).ok();
                        }
                        "float" => {
                            let fl = self.native_float_or_box(out, indent, &val.to_string());
                            writeln!(out, "{}store{} float {}, float* {}, align {}", indent, vol_str, fl, p, self.align_of(&ty)).ok();
                        }
                        s if s == "i8*" || s == "ptr" => {
                            let fp = format!("%fp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, fp, val).ok();
                            writeln!(out, "{}store{} i8* {}, i8** {}, align {}", indent, vol_str, fp, p, self.align_of(&ty)).ok();
                        }
                        _ => {
                            writeln!(out, "{}store{} {} {}, {}* {}, align {}", indent, vol_str, ty, val, ty, p, self.align_of(&ty)).ok();
                        }
                    }
                } else if let Some(slot) = self.param_slots.get(&fname) {
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, val, slot).ok();
                    // Update let_bindings so subsequent reads see the new value
                    self.let_bindings.insert(fname.clone(), val.name.clone());
                    if let Some(ft) = self.let_binding_types.get(&fname) {
                        self.let_binding_types.insert(fname.clone(), ft.clone());
                    }
                } else {
                    self.let_bindings.insert(fname.clone(), val.name.clone());
                    self.let_binding_types.insert(fname.clone(), val.ty.clone());
                    writeln!(out, "{}; assign {} to {}", indent, val, fname).ok();
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                let cond = self.emit_expr(out, condition, indent);
                let i1 = format!("%gc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();

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
                                        let av_tr = format!("%gatr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, av_tr, av).ok();
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
                                        writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}", indent, se, i1, av, ld).ok();
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
                    // Then-path terminated (e.g. term!). The else path at end_l
                    // still needs a terminator. Do NOT restore prev_terminated —
                    // the current block (end_l) has a ret, so callers must not
                    // emit more code after us.
                    if self.returns_i64 {
                        writeln!(out, "  ret i64 0").ok();
                    } else {
                        writeln!(out, "  ret void").ok();
                    }
                }}
            Statement::SyncBlock { body } => {
                for s in body { self.emit_stmt(out, s, indent); }
            }
            Statement::Unification { name, variant, fields: _, expr } => {
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
                let target = self.variant_disc.get(name.as_str())
                    .map(|(_, d, _)| *d)
                    .unwrap_or(0);
                writeln!(out, "{}switch i64 {}, label %{} [ i64 {}, label %{} ]", indent, disc, def_l, target, arm_l).ok();
                writeln!(out, "{}{}:", indent, arm_l).ok();
                let pay = format!("%up{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = lshr i64 {}, 8", indent, pay, val).ok();
                self.let_bindings.insert(variant.clone(), pay.clone());
                let _ = self.emit_expr(out, expr, indent);
                writeln!(out, "{}br label %{}", indent, merge_l).ok();
                writeln!(out, "{}{}:", indent, def_l).ok();
                writeln!(out, "{}  unreachable", indent).ok();
                writeln!(out, "{}{}:", indent, merge_l).ok();
                self.let_bindings = saved_bindings;
                self.let_binding_types = saved_types;
            }
            Statement::Expression(e) => { let _ = self.emit_expr(out, e, indent); }
            Statement::LocalTrigger { .. } => { writeln!(out, "{}; trg!", indent).ok(); }
            Statement::OnExit { body, .. } => { self.pending_cleanup.extend(body.iter().cloned()); }
            Statement::Alka(b) => { for l in b.content.lines() { let _ = writeln!(out, "{}{}", indent, l); } }
            Statement::InlineAsm { asm_string, .. } => { writeln!(out, "{}{}", indent, asm_string).ok(); }
            Statement::Foreach { item, list, body } => {
                crate::features::stmt::foreach::ForeachStmt {
                    item: item.clone(),
                    list: list.clone(),
                    body: body.clone(),
                }.emit_llvm(self, out, &StmtDispatch, indent);
            }
            Statement::Oracle { body, handler, .. } => {
                for s in body {
                    self.emit_stmt(out, s, indent);
                }
            }
        }
    }

    // ── EXPRESSIONS ───────────────────────────────────────────
}
