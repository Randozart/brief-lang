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
    ///
    /// Why boxing to i64: %State stores all non-float fields as i64 for
    /// uniformity. Bool (native i1) is zext'd, Char (native i32) is kept
    /// as i64, String/Data (native i8*) is ptrtoint'd, and Float (native
    /// float) is bitcast through i32 then zext. The single i64 slot per
    /// field means LLVM's TBAA metadata is the only way to disambiguate
    /// types — there is no runtime type tag.
    ///
    /// The redundancy in the float path (bitcast float→i32→zext i64) is
    /// deliberate: it preserves the float bits through a uniform i64
    /// representation so that TBAA (not the bit pattern) tells LLVM
    /// which operations are valid. Without the bitcast+zext, LLVM would
    /// see a float value stored in an i64 slot and produce invalid
    /// bitcast or pointer-to-int transforms during optimization.
    pub(super) fn adapt_to_i64(&mut self, out: &mut String, indent: &str, r: &TypedRegister) -> String {
        if r.ty == Type::Bool {
            let z = format!("%rz{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = zext i1 {} to i64", indent, z, r.name).ok();
            z
        } else if r.ty == Type::Char {
            // All Char registers from emit_expr are already i64 (boxed).
            // No zext needed — the register is already the right width.
            r.name.clone()
        // 2026-06-28: String/Data registers can be either native i8* (from
        // function params or arg slots) or boxed i64 (from emit_expr's %t{N}
        // registers, ListIndex loads, or %State field loads). Check the
        // register name prefix to decide: %t and %d prefixes are from
        // emit_expr (always i64); other prefixes like %p_ are native i8*.
        // 2026-06-28: String/Data registers can be either native i8* (from
        // function params or arg slots) or boxed i64 (from emit_expr's %t{N}
        // registers, ListIndex loads, or %State field loads). Check the
        // register name prefix to decide: %t and %d prefixes are from
        // emit_expr (always i64); other prefixes like %p_ are native i8*.
        } else if r.ty == Type::String || r.ty == Type::Data {
            let is_boxed = r.name.starts_with("%t") || r.name.starts_with("%d");
            if is_boxed {
                // Already i64 (boxed) — just use as-is
                // This happens with ListIndex loads like rules[i] where the
                // element type is String but the actual register is i64.
                r.name.clone()
            } else {
                let p = format!("%rp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, p, r.name).ok();
                p
            }
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
                // in_callable_txn: set by emit_definition (defn) and
                // emit_callable_txn. When true, Term emits a ret with the
                // computed value and sets terminated=true. When false, Term
                // is a no-op — the caller's outer fallback ret handles it.
                if self.in_callable_txn {
                    // Store value to result slot, branch to post label
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        // Phase 3: Decay chimera return value at term boundary
                        let r = self.emit_decay(out, &r, None, indent);
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
                // term! has three emission paths depending on context:
                //
                // 1. Callable txn: store result to callable_txn_result slot,
                //    branch to post_label (caller picks up the value).
                //    No ret — the caller's post-label handles the return.
                //
                // 2. Reactive txn loop (loop_exit_label is set): store value
                //    to %state, branch to exit label. This lets LLVM see the
                //    loop as countable (the exit branch dominates all exits)
                //    and enables more aggressive unrolling/vectorization
                //    compared to ret + caller loop.
                //
                // 3. Standalone (main_body or plain function): emit ret with
                //    the correct return type. Embedded targets emit wfi
                //    (wait-for-interrupt) instead of ret.
                if self.in_callable_txn {
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        // Phase 3: Decay chimera before storing to state
                        let r = self.emit_decay(out, &r, None, indent);
                        self.store_i64_result(out, indent, &r, "%state");
                    }
                    if let Some(ref loop_exit) = self.loop_exit_label {
                        writeln!(out, "{}br label %{}", indent, loop_exit).ok();
                    } else if let Some(ref pl) = self.callable_txn_post_label {
                        writeln!(out, "{}br label %{}", indent, pl).ok();
                    }
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
                            writeln!(out, "{}store i64 {}, ptr {}", indent, r, self.state_reg_name).ok();
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
            Statement::Let { name, expr, ty, address_expr, constraint, .. } => {
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
                // Inline constraint check: <: [expr]
                if let Some(c) = constraint {
                    self.emit_guard_check(out, indent, name, c);
                }
                // TypeUniverse guard check: TypeDef body constraints on the annotated type
                if let Some(ann_ty) = ty.as_ref() {
                    let ann_ref: &Type = ann_ty;
                    let type_name: &str = match ann_ref {
                        Type::Custom(n) => n.as_str(),
                        _ => "",
                    };
                    if !type_name.is_empty() {
                        let guards: Vec<crate::ast::Expr> = self.type_universe.as_ref()
                            .and_then(|u| u.types.get(type_name))
                            .map(|r| r.guards.clone())
                            .unwrap_or_default();
                        for guard in &guards {
                            self.emit_guard_check(out, indent, name, guard);
                        }
                    }
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
                                writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, p, self.state_reg_name, field_idx).ok();
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
                                writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, p, self.state_reg_name, idx).ok();
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
                if let Some(trg) = self.triggers.get(&fname) {
                    if trg.is_const {
                        writeln!(out, "{}; error: cannot write to const trigger '{}'", indent, fname).ok();
                        return;
                    }
                }
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
                                    let tr = format!("%tri{}", self.txn_counter); self.txn_counter += 1;
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
                            if ty != "float" {
                                let re = format!("%re_{}_{}", fname, self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, re, new_reg, idx).ok();
                            self.ssa_old_int_regs.insert(fname.clone(), re);
                            }
                            // Phase 2: Invalidate ALL cache targets on SSA field store
                            let ssa_result = if let Some(targets) = self.cache_slots.get(&fname) {
                                let mut reg = new_reg.clone();
                                for (_target, &(_cache_idx, valid_idx)) in targets {
                                    let inv = format!("%civssa{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = insertvalue %State {}, i8 0, {}", indent, inv, reg, valid_idx).ok();
                                    reg = inv;
                                }
                                reg
                            } else {
                                new_reg.clone()
                            };
                            self.ssa_state_reg = Some(ssa_result);
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
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, p, self.state_reg_name, idx).ok();
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
                            // 2026-06-27: Update ssa_old_float_regs so subsequent
                            // body reads see the stored float value.
                            self.ssa_old_float_regs.insert(fname.clone(), fl);
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
                    // 2026-06-26: Track the stored value for per-field phi
                    // back-edge. When the canonical loop uses phi nodes for
                    // state fields, the latch needs the updated register value
                    // to feed back into the phi (instead of reloading from
                    // %State, which would add a GEP+load round-trip).
                    self.pending_phi_backedge.insert(fname.clone(), val_boxed.clone());
                    // 2026-06-27: Update ssa_old registers so subsequent body
                    // reads (guards, let-bindings) see the stored value. In the
                    // per-field phi path, ssa_old_int_regs starts with phi regs
                    // (pre-tick values). Without this update, a guard after a
                    // field write reads the pre-write value (ring_buffer bug).
                    // Note: float case updates ssa_old_float_regs inside its
                    // match arm above (fl is local to that arm).
                    if ty != "float" && ty != "i8*" && ty != "ptr" {
                        self.ssa_old_int_regs.insert(fname.clone(), val_boxed.clone());
                    } else if ty == "i8*" || ty == "ptr" {
                        self.ssa_old_int_regs.insert(fname.clone(), val_boxed.clone());
                    }
                    // Phase 2: Invalidate ALL cache targets on field store
                    if let Some(targets) = self.cache_slots.get(&fname) {
                        for (_target, &(_cache_idx, valid_idx)) in targets {
                            let inv_gep = format!("%civ{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}",
                                indent, inv_gep, valid_idx).ok();
                            writeln!(out, "{}store i8 0, i8* {}, align 1", indent, inv_gep).ok();
                        }
                    }
                } else if let Some(slot) = self.param_slots.get(&fname).cloned() {
                    let val_boxed = self.adapt_to_i64(out, indent, &val);
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, val_boxed, slot).ok();
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
                //
                // Why this optimization: a Guarded statement that wraps a single
                // Assignment can be emitted as a `select` instruction instead of
                // a branch + phi. `select` is a single ALU op with no control
                // flow change — the CPU's branch predictor sees no branch, the
                // out-of-order scheduler sees no serialization point, and LLVM's
                // passes (GVN, LICM, SROA) can optimize through select more
                // aggressively than through a conditional branch.
                //
                // This only applies in memory mode (ssa_state_reg.is_none()).
                // In SSA mode, insertvalue chains require a phi to merge the
                // two state values — select on the field value alone is not
                // enough because the rest of %State must also be live.
                //
                // Cache slots are invalidated after the select store, same as
                // the branch-based path.
                if statements.len() == 1 && self.ssa_state_reg.is_none() {
                    if let Statement::Assignment { lhs, expr, modifiers, .. } = &statements[0] {
                        if let Expr::Identifier(n) | Expr::OwnedRef(n) = lhs {
                            if let Some(&idx) = self.field_index_map.get(n) {
                                let g_is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                                let gvol = if g_is_volatile { " volatile" } else { "" };
                                let p = format!("%gp{}", self.txn_counter); self.txn_counter += 1;
                                let av = self.emit_expr(out, expr, indent);
                                let ty = self.field_types[idx].clone();
                                writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, p, self.state_reg_name, idx).ok();
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
                                // Phase 2: Invalidate ALL cache targets on select store
                                if let Some(targets) = self.cache_slots.get(n) {
                                    for (_target, &(_cache_idx, valid_idx)) in targets {
                                        let inv_gep = format!("%civs{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}",
                                            indent, inv_gep, valid_idx).ok();
                                        writeln!(out, "{}store i8 0, i8* {}, align 1", indent, inv_gep).ok();
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
                // 2026-06-28: Save SSA old-int/old-float regs + let bindings.
                // Values defined in the guard body (then_l) use SSA registers
                // local to that block and don't dominate the merge block (end_l).
                // Without saving/restoring, subsequent reads of state fields
                // via ssa_old_int_regs would use registers from the guard body,
                // producing "Instruction does not dominate all uses" errors.
                let saved_bindings = self.let_bindings.clone();
                let saved_types = self.let_binding_types.clone();
                let saved_old_int = self.ssa_old_int_regs.clone();
                let saved_old_float = self.ssa_old_float_regs.clone();
                for s in statements { self.emit_stmt(out, s, &format!("{}  ", indent)); }
                self.let_bindings = saved_bindings;
                self.let_binding_types = saved_types;
                self.ssa_old_int_regs = saved_old_int;
                self.ssa_old_float_regs = saved_old_float;
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
                    // 2026-06-26: Only clear cached pre-tick regs in SSA phi
                    // mode where the guard may have modified state via
                    // insertvalue. In memory mode (ssa_pre_reg.is_none()),
                    // Brief's reactive semantics guarantee all reads within
                    // a tick see pre-tick values — the guard's stores affect
                    // the next tick, not the current one. Clearing here forces
                    // ALL subsequent field references to fall back to GEP+load
                    // from %State, adding a load+store round-trip per field
                    // per iteration. This is the single largest performance gap
                    // vs Clang (which keeps everything in phi nodes).
                    if ssa_pre_reg.is_some() {
                        self.ssa_old_int_regs.clear();
                        self.ssa_old_float_regs.clear();
                    }
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
            Statement::TrgBinding { name, instance, .. } => {
                let val = self.emit_expr(out, instance, indent);
                let reg = format!("%t{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, reg, val.name).ok();
                self.let_bindings.insert(name.clone(), reg);
                if let Some(ty) = self.let_binding_types.get(&val.name).cloned() {
                    self.let_binding_types.insert(name.clone(), ty);
                } else {
                    self.let_binding_types.insert(name.clone(), Type::Int);
                }
            }
        }
    }

    /// Emit a runtime constraint/guard check for a variable bound in this tick.
    /// Temporarily binds `_` to the variable's register, evaluates the expression,
    /// and branches to `@llvm.trap()` on false.
    ///
    /// WHY constraint guards are emitted as separate checks with @llvm.trap() failure:
    ///   Brief's contract system allows per-variable guards in type definitions
    ///   (e.g. `let x: Int[0 < x]`). These guards are not preconditions — they
    ///   apply to individual values within a tick. If the guard fails, the program
    ///   has violated a type invariant, which is unrecoverable (UB). @llvm.trap()
    ///   tells LLVM this path is dead code, enabling DCE of the guarded body and
    ///   any downstream computations that depend on x. Unlike @llvm.assume (which
    ///   is a trust-the-checker hint), @llvm.trap() + unreachable is a hard
    ///   correctness boundary — LLVM can eliminate all code that is only reachable
    ///   through the failed guard.
    ///
    ///   The `_` binding allows guards like `[int_to_str(x) != ""]` where the guard
    ///   expression references x using Brief's `_` convention ("the value being
    ///   constrained"). Without it, guards would need to name x explicitly, which
    ///   would be inconsistent with how `_` works in mask/filter expressions.
    fn emit_guard_check(&mut self, out: &mut String, indent: &str, var_name: &str, guard: &Expr) {
        let Some(reg) = self.let_bindings.get(var_name).cloned() else { return };
        let prior_ = self.let_bindings.get("_").cloned();
        let prior_ty = self.let_binding_types.get("_").cloned();
        self.let_bindings.insert("_".to_string(), reg);
        if let Some(ty) = self.let_binding_types.get(var_name).cloned() {
            self.let_binding_types.insert("_".to_string(), ty);
        }
        let ok = self.emit_expr(out, guard, indent);
        let i1 = self.as_bool_reg(out, indent, &ok);
        let cc = format!("%cc{}", self.txn_counter); self.txn_counter += 1;
        let cp = format!("%cp{}", self.txn_counter); self.txn_counter += 1;
        self.txn_counter += 2;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, cc, cp).ok();
        writeln!(out, "{}{}:", indent, cp).ok();
        writeln!(out, "{}  call void @llvm.trap()", indent).ok();
        writeln!(out, "{}  unreachable", indent).ok();
        writeln!(out, "{}{}:", indent, cc).ok();
        match prior_ {
            Some(r) => { self.let_bindings.insert("_".to_string(), r); }
            None => { self.let_bindings.remove("_"); }
        }
        match prior_ty {
            Some(t) => { self.let_binding_types.insert("_".to_string(), t); }
            None => { self.let_binding_types.remove("_"); }
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
                for sub in subfields {
                    if let crate::ast::Pattern::Var(name) = sub {
                        let_bindings.insert(name.clone(), payload_reg.to_string());
                        let_binding_types.insert(name.clone(), Type::Int);
                    }
                }
            }
            _ => {}
        }
    }
}

// ── EXPRESSIONS ───────────────────────────────────────────
