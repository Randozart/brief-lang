use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::{float_to_llvm_hex, find_perfect_hash, sparsity_ratio, FoldParam, LlvmBackend};
use crate::analysis::dependency_graph::DependencyGraph;
use std::collections::HashMap;
use std::fmt::Write;

impl LlvmBackend {
    /// Recursively evaluate a boolean expression for the exit condition check.
    /// All values are emitted as `i64` for uniformity; comparisons are zext'd from `i1`.
    pub(crate) fn emit_exit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> String {
        // Leaf expressions: delegate integer/bool to emit_expr for constant
        // inlining. Keep Identifier/OwnedRef local because exit conditions
        // Access %state pointer (passed as parameter or via alloca in main)
        match expr {
            Expr::Integer(_) | Expr::Bool(_) | Expr::Float(_) | Expr::Neg(_) => {
                return self.emit_expr(out, expr, indent).name;
            }
            _ => {}
        }
        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        match expr {
            Expr::Identifier(name) => {
                if let Some(&idx) = self.field_index_map.get(name) {
                    let p = format!("%gep_exit_{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, p).ok();
                } else if self.constants.contains_key(name) {
                    writeln!(out, "{}{} = load i64, i64* @{}, align 8", indent, v, name).ok();
                } else if self.trigger_names.contains(name) {
                    if let Some(t) = self.triggers.get(name).cloned() {
                        self.emit_trg_load(out, indent, &v, &t.address, &t.ty);
                    } else {
                        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    }
                } else {
                    writeln!(out, "{}{} = add i64 0, 0 ; unknown id '{}'", indent, v, name).ok();
                }
                v
            }
            Expr::OwnedRef(name) => {
                return self.emit_exit_expr(out, &Expr::Identifier(name.clone()), indent);
            }
            Expr::Eq(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Ne(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Lt(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Le(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Gt(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Ge(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::And(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                writeln!(out, "{}{} = and i64 {}, {}", indent, v, lv, rv).ok();
                v
            }
            Expr::Or(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                writeln!(out, "{}{} = or i64 {}, {}", indent, v, lv, rv).ok();
                v
            }
            Expr::Not(e) => {
                let inner = self.emit_exit_expr(out, e, indent);
                writeln!(out, "{}{} = xor i64 {}, 1", indent, v, inner).ok();
                v
            }
            _ => {
                writeln!(out, "{}{} = add i64 0, 0 ; unsupported exit expr", indent, v).ok();
                v
            }
        }
    }

    // ── MAIN FUNCTION ─────────────────────────────────────────
    pub(crate) fn emit_main(&mut self, out: &mut String, has_wake_triggers: bool) {
        self.fn_ret_ty = "i32".to_string();
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#3")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        if self.has_async_txns && !self.is_lightweight_async {
            let count = self.async_txn_names.len() as i32;
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x void (%State*)*]* @thread_pool_fns to i8**", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @brief_thread_pool_init(i32 {}, i8** %tp_fn_ptr)", count).ok();
        }
        // Line-buffer stdout so \n auto-flushes (matching default TTY behavior)
        writeln!(out, "  %so_init = load ptr, ptr @stdout").ok();
        writeln!(out, "  call i32 @setvbuf(ptr %so_init, ptr null, i32 1, i64 0)").ok();
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "  tick:").ok();
        if self.has_async_txns && !self.is_lightweight_async {
            self.emit_async_phase(out);
        } else {
            writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
        }
        let has_exit = self.exit_condition.is_some();
        if has_exit {
            let cond = self.exit_condition.clone().unwrap();
            let val = self.emit_exit_expr(out, &cond, "  ");
            let tr = format!("%t{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
            if has_wake_triggers {
                writeln!(out, "  br i1 {}, label %done, label %wait", tr).ok();
                writeln!(out, "  wait:").ok();
                emit_trg_event_epoll_wait(self, out);
                writeln!(out, "  br label %tick").ok();
            } else {
                writeln!(out, "  br i1 {}, label %done, label %tick", tr).ok();
            }
            writeln!(out, "  done:").ok();
            writeln!(out, "  ret i32 0").ok();
        } else {
            if has_wake_triggers {
                emit_trg_event_epoll_wait(self, out);
            }
            writeln!(out, "  br label %tick").ok();
        }
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Pre-extract all float fields from the current SSA state register
    /// into named old-value registers. Body statements that read float
    /// fields will use these old-value registers, making all float
    /// operations within the iteration independent — LLVM's scheduler can
    /// then fill all CPU float execution ports simultaneously.
    pub(crate) fn pre_extract_float_fields(&mut self, out: &mut String) {
        let ssa_reg = match self.ssa_state_reg.clone() {
            Some(r) => r,
            None => return,
        };
        self.ssa_old_float_regs.clear();
        for (field_name, &field_idx) in &self.field_index_map {
            if self.field_types[field_idx] == "float" {
                let old_reg = format!("%{}_old_{}", field_name, self.txn_counter);
                self.txn_counter += 1;
                writeln!(out, "  {} = extractvalue %State {}, {}", old_reg, ssa_reg, field_idx).ok();
                self.ssa_old_float_regs.insert(field_name.clone(), old_reg);
            }
        }
    }

    /// Pre-extract all non-Float state fields into SSA registers before the body.
    /// Mirrors `pre_extract_float_fields` for Int fields. This eliminates the
    /// per-reference extractvalue-from-insertvalue-chain pattern that inflates
    /// the SSA body by ~5× for Int-heavy benchmarks.
    pub(crate) fn pre_extract_int_fields(&mut self, out: &mut String) {
        let ssa_reg = match self.ssa_state_reg.clone() {
            Some(r) => r,
            None => return,
        };
        self.ssa_old_int_regs.clear();
        for (field_name, &field_idx) in &self.field_index_map {
            if self.field_types[field_idx] != "float" {
                let old_reg = format!("%{}_old_{}", field_name, self.txn_counter);
                self.txn_counter += 1;
                writeln!(out, "  {} = extractvalue %State {}, {}", old_reg, ssa_reg, field_idx).ok();
                self.ssa_old_int_regs.insert(field_name.clone(), old_reg);
            }
        }
    }

    /// Load all state fields into old-value registers via GEP loads.
    /// Used by emit_ssa_main when ssa_state_reg is None (per-field GEP mode).
    /// Mirrors pre_extract_float/int_fields but loads from memory instead of
    /// extractvalue from the SSA %State register.
    fn pre_load_all_fields(&mut self, out: &mut String, state_ptr: &str) {
        self.ssa_old_float_regs.clear();
        self.ssa_old_int_regs.clear();
        for (field_name, &field_idx) in &self.field_index_map {
            let ty_str = &self.field_types[field_idx];
            let gc = self.txn_counter; self.txn_counter += 1;
            let gep = format!("%gep_{}_{}", field_name, gc);
            writeln!(out, "  {} = getelementptr inbounds %State, %State* {}, i32 0, i32 {}", gep, state_ptr, field_idx).ok();
            let old_reg = format!("%{}_old_{}", field_name, self.txn_counter);
            self.txn_counter += 1;
            writeln!(out, "  {} = load {}, {}* {}, align {}", old_reg, ty_str, ty_str, gep, self.align_of(ty_str)).ok();
            if ty_str == "float" {
                self.ssa_old_float_regs.insert(field_name.clone(), old_reg);
            } else {
                self.ssa_old_int_regs.insert(field_name.clone(), old_reg);
            }
        }
    }

    /// Emit the folded while-loop body (without `@init_state()` or the enclosing
    /// `define` / `ret`).  Used by both `emit_folded_main` and the enum dispatch path.
    ///
    /// When `use_phi = true`, the counter lives in an SSA phi node (register)
    /// instead of being loaded/stored through %state every iteration.
    /// Only valid when the txn body is pure (just counter++).
    ///
    /// When `use_phi = false` and `body = Some(stmts)`, the txn body is emitted
    /// inline with struct-SSA (load `%State` once, insertvalue chains, store once).
    /// When `use_phi = false` and `body = None`, calls the txn function as before.
    pub(crate) fn emit_folded_loop(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        label_prefix: &str,
        use_phi: bool,
        body: Option<&[Statement]>,
        unroll_factor: usize,
        is_decreasing: bool,
        bound_literal: Option<i64>,
    ) {
        let c0 = self.txn_counter;
        if use_phi {
            let entry_label = format!("{}_phi_entry", label_prefix);
            let hdr_label = format!("{}_hdr", label_prefix);
            let body_label = format!("{}_body", label_prefix);
            let done_label = format!("{}_done", label_prefix);
            writeln!(out, "{}:", entry_label).ok();
            // Load bound once
            if let Some(ti) = total_idx {
                writeln!(out, "  %gt_{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0, ti).ok();
                writeln!(out, "  %lt_{}_{} = load i64, i64* %gt_{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt_{}_{} = load i64, i64* @{}, align 8", label_prefix, c0, cn).ok();
            } else {
                writeln!(out, "  %lt_{}_{} = add i64 0, 0", label_prefix, c0).ok();
            }
            // Load counter once, precompute remaining iterations
            writeln!(out, "  %gcnt_{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0, counter_idx).ok();
            writeln!(out, "  %init_{}_{} = load i64, i64* %gcnt_{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
            // Counted-down loop: remaining = bound - initial, count down to 0.
            // This eliminates the cmp instruction (sub sets ZF for jne) and
            // matches what clang emits for C for-loops.
            writeln!(out, "  %rem_{}_{} = sub i64 %lt_{}_{}, %init_{}_{}", label_prefix, c0 + 1, label_prefix, c0, label_prefix, c0).ok();
            writeln!(out, "  br label %{}", hdr_label).ok();
            writeln!(out, "{}:", hdr_label).ok();
            writeln!(out, "  %i_{}_{} = phi i64 [ %rem_{}_{}, %{} ], [ %dec_{}_{}, %{} ]", label_prefix, c0 + 2, label_prefix, c0 + 1, entry_label, label_prefix, c0 + 2, body_label).ok();
            writeln!(out, "  %cp_{}_{} = icmp sgt i64 %i_{}_{}, 0", label_prefix, c0 + 3, label_prefix, c0 + 2).ok();
            writeln!(out, "  br i1 %cp_{}_{}, label %{}, label %{}", label_prefix, c0 + 3, body_label, done_label).ok();
            writeln!(out, "{}:", body_label).ok();
            writeln!(out, "  %dec_{}_{} = sub i64 %i_{}_{}, 1", label_prefix, c0 + 2, label_prefix, c0 + 2).ok();
            writeln!(out, "  br label %{}", hdr_label).ok();
            writeln!(out, "{}:", done_label).ok();
            // Final counter value is always the bound after counted-down loop
            writeln!(out, "  store i64 %lt_{}_{}, i64* %gcnt_{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
        } else if let Some(stmts) = body {
            // SSA mode: load once, phi in header, inline unrolled body with extract/insert, store once
            if let Some(bl) = bound_literal {
                writeln!(out, "  %lt{}_{} = add i64 0, {}", label_prefix, c0, bl).ok();
            } else if let Some(ti) = total_idx {
                writeln!(out, "  %gt{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0, ti).ok();
                writeln!(out, "  %lt{}_{} = load i64, i64* %gt{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt{}_{} = load i64, i64* @{}, align 8", label_prefix, c0, cn).ok();
            } else {
                writeln!(out, "  %lt{}_{} = add i64 0, 0", label_prefix, c0).ok();
            }
            let phi_reg = format!("%ssa_phi_{}", label_prefix);
            let unroll = unroll_factor.max(1);
            let unroll_minus_1 = unroll - 1;

            // --- body4: unrolled loop body ---
            let mut body4_buf = String::new();
            if unroll > 1 {
                writeln!(body4_buf, "{}_body4:", label_prefix).ok();
                let mut cur = phi_reg.clone();
                for _ in 0..unroll {
                    self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
                    self.terminated = false;
                    self.returns_i64 = false;
                    self.ssa_state_reg = Some(cur);
                    // Pre-extract all float fields from the entering state
                    // so body field reads use old values — all float ops
                    // become independent, filling all CPU execution ports.
                    self.pre_extract_float_fields(&mut body4_buf);
                    self.pre_extract_int_fields(&mut body4_buf);
                    for stmt in stmts.iter().filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. })) {
                        self.emit_stmt(&mut body4_buf, stmt, "  ");
                    }
                    self.ssa_old_float_regs.clear();
                    self.ssa_old_int_regs.clear();
                    cur = self.ssa_state_reg.take().unwrap_or(phi_reg.clone());
                }
                let backedge4 = cur;
                writeln!(body4_buf, "  store %State {}, %State* %slot_{}, align 8", backedge4, label_prefix).ok();
                writeln!(body4_buf, "  br label %{}_hdr", label_prefix).ok();
            }

            // --- body1: remainder loop (single iteration) ---
            let mut body1_buf = String::new();
            writeln!(body1_buf, "{}_body1:", label_prefix).ok();
            self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            self.ssa_state_reg = Some(phi_reg.clone());
            self.pre_extract_float_fields(&mut body1_buf);
            self.pre_extract_int_fields(&mut body1_buf);
            for stmt in stmts.iter().filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. })) {
                self.emit_stmt(&mut body1_buf, stmt, "  ");
            }
            let backedge_val = self.ssa_state_reg.take().unwrap_or(phi_reg.clone());
            writeln!(body1_buf, "  store %State {}, %State* %slot_{}, align 8", backedge_val, label_prefix).ok();
            writeln!(body1_buf, "  br label %{}_hdr", label_prefix).ok();

            // Build initial %State from known constants
            writeln!(out, "  br label %{}_pre", label_prefix).ok();
            writeln!(out, "{}_pre:", label_prefix).ok();
            let mut cur_init = "zeroinitializer".to_string();
            let mut fields: Vec<(String, usize, String)> = self.field_index_map.iter()
                .map(|(name, &idx)| (name.clone(), idx, self.field_types[idx].clone()))
                .collect();
            fields.sort_by_key(|&(_, idx, _)| idx);
            for (name, idx, ty) in &fields {
                let init = self.field_initializers.get(name).and_then(|e| e.as_ref());
                match init {
                    Some(Expr::Float(f)) => {
                        let h = float_to_llvm_hex(*f);
                        let bc = format!("%fbc{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = bitcast i32 {} to float", bc, h).ok();
                        let iv = format!("%fiv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, float {}, {}", iv, cur_init, bc, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Integer(n)) => {
                        let iv = format!("%iiv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i64 {}, {}", iv, cur_init, n, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Bool(b)) => {
                        let v = if *b { 1 } else { 0 };
                        let iv = format!("%biv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i8 {}, {}", iv, cur_init, v, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Neg(inner)) => {
                        let s = match inner.as_ref() {
                            Expr::Float(f) => float_to_llvm_hex(-*f),
                            Expr::Integer(n) => format!("-{}", n),
                            _ => "0".to_string(),
                        };
                        if ty == "float" {
                            let bc = format!("%nbc{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "  {} = bitcast i32 {} to float", bc, s).ok();
                            let iv = format!("%niv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "  {} = insertvalue %State {}, float {}, {}", iv, cur_init, bc, idx).ok();
                            cur_init = iv;
                        } else {
                            let iv = format!("%niv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "  {} = insertvalue %State {}, i64 {}, {}", iv, cur_init, s, idx).ok();
                            cur_init = iv;
                        }
                    }
                    Some(Expr::String(_)) => {
                        let iv = format!("%siv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i8* null, {}", iv, cur_init, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Char(c)) => {
                        let v = *c as i32;
                        let iv = format!("%civ{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i32 {}, {}", iv, cur_init, v, idx).ok();
                        cur_init = iv;
                    }
                    _ => {
                        let gep = format!("%gep{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", gep, idx).ok();
                        let ld = format!("%ld{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = load {}, {}* {}, align {}", ld, ty, ty, gep, self.align_of(&ty)).ok();
                        let iv = format!("%liv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, {} {}, {}", iv, cur_init, ty, ld, idx).ok();
                        cur_init = iv;
                    }
                }
            }
            let slot = format!("%slot_{}", label_prefix);
            writeln!(out, "  {} = alloca %State, align 8", slot).ok();
            writeln!(out, "  store %State {}, %State* {}, align 8", cur_init, slot).ok();
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();

            // Header: extract counter, compare with adjusted/un-adjusted bounds
            writeln!(out, "{}_hdr:", label_prefix).ok();
            writeln!(out, "  {} = load %State, %State* {}, align 8", phi_reg, slot).ok();
            writeln!(out, "  %ex{}_{} = extractvalue %State {}, {}", label_prefix, self.txn_counter, phi_reg, counter_idx).ok();
            let ex_reg = format!("%ex{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;

            if unroll > 1 {
                let adj = format!("%adj{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                if is_decreasing {
                    writeln!(out, "  {} = add i64 %lt{}_{}, {}", adj, label_prefix, c0, unroll_minus_1).ok();
                } else {
                    writeln!(out, "  {} = add i64 %lt{}_{}, -{}", adj, label_prefix, c0, unroll_minus_1).ok();
                }
                let cp4 = format!("%cp{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                if is_decreasing {
                    writeln!(out, "  {} = icmp sgt i64 {}, {}", cp4, ex_reg, adj).ok();
                } else {
                    writeln!(out, "  {} = icmp slt i64 {}, {}", cp4, ex_reg, adj).ok();
                }
                writeln!(out, "  br i1 {}, label %{}_body4, label %{}_rem", cp4, label_prefix, label_prefix).ok();
                writeln!(out, "{}_rem:", label_prefix).ok();
            }
            let cp1 = format!("%cp{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
            if is_decreasing {
                writeln!(out, "  {} = icmp sgt i64 {}, %lt{}_{}", cp1, ex_reg, label_prefix, c0).ok();
            } else {
                writeln!(out, "  {} = icmp slt i64 {}, %lt{}_{}", cp1, ex_reg, label_prefix, c0).ok();
            }
            writeln!(out, "  br i1 {}, label %{}_body1, label %{}_done", cp1, label_prefix, label_prefix).ok();

            if unroll > 1 {
                out.push_str(&body4_buf);
            }
            out.push_str(&body1_buf);

            let final_reg = format!("%final_{}", label_prefix);
            writeln!(out, "{}_done:", label_prefix).ok();
            writeln!(out, "  {} = load %State, %State* %slot_{}, align 8", final_reg, label_prefix).ok();
            writeln!(out, "  store %State {}, %State* %state, align 8", final_reg).ok();
        } else {
            if let Some(bl) = bound_literal {
                writeln!(out, "  %lt{}_{} = add i64 0, {}", label_prefix, c0, bl).ok();
            } else if let Some(ti) = total_idx {
                writeln!(out, "  %gt{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0, ti).ok();
                writeln!(out, "  %lt{}_{} = load i64, i64* %gt{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt{}_{} = load i64, i64* @{}, align 8", label_prefix, c0, cn).ok();
            } else {
                writeln!(out, "  %lt{}_{} = add i64 0, 0", label_prefix, c0).ok();
            }
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();
            writeln!(out, "{}_hdr:", label_prefix).ok();
            writeln!(out, "  %gp{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0 + 1, counter_idx).ok();
            writeln!(out, "  %lp{}_{} = load i64, i64* %gp{}_{}, align 8", label_prefix, c0 + 1, label_prefix, c0 + 1).ok();
            let cmp_reg = format!("%cp{}_{}", label_prefix, c0 + 2);
            if is_decreasing {
                writeln!(out, "  {} = icmp sgt i64 %lp{}_{}, %lt{}_{}", cmp_reg, label_prefix, c0 + 1, label_prefix, c0).ok();
            } else {
                writeln!(out, "  {} = icmp slt i64 %lp{}_{}, %lt{}_{}", cmp_reg, label_prefix, c0 + 1, label_prefix, c0).ok();
            }
            writeln!(out, "  br i1 {}, label %{}_body, label %{}_done", cmp_reg, label_prefix, label_prefix).ok();
            writeln!(out, "{}_body:", label_prefix).ok();
            writeln!(out, "  call void @{}(%State* %state)", txn_name).ok();
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();
            writeln!(out, "{}_done:", label_prefix).ok();
        }
    }

    pub(crate) fn emit_folded_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        use_phi: bool,
        body: Option<&[Statement]>,
    ) {
        self.fn_ret_ty = "i32".to_string();
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        // Legacy phi-mode: uses
        if use_phi {
            writeln!(out, "  br label %case_phi_entry").ok();
        }
        let uf = if !use_phi && body.is_some() { 4 } else { 1 };
        self.emit_folded_loop(out, txn_name, counter_idx, total_idx, total_const_name, "case", use_phi, body, uf, false, None);
        let saved = std::mem::take(&mut self.pending_post_hoist);
        self.emit_hoisted_post_loop_prints(out, &saved);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a counted-loop main() that uses per-field GEP loads/stores (no SSA
    /// insertvalue chain). Used when the body has branching control flow (Guarded
    /// statements) and linearity cannot be proven — avoids phi %State dominance issues.
    ///
    /// 2026-06-13: A005b — memory path for non-linear bodies.
    pub(crate) fn emit_folded_memory_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        body: &[Statement],
    ) {
        self.fn_ret_ty = "i32".to_string();
        let attr = self.slp_attr("main", "#0");
        let c0 = self.txn_counter;
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        // Bound loading — use numbered positional args ({0}, {1}) to avoid
        // LLVM IR brace chars being parsed as named format placeholders.
        if let Some(ti) = total_idx {
            writeln!(out, "  %gt{0}_{1} = getelementptr inbounds %State, %State* %state, i32 0, i32 {1}", c0, ti).ok();
            writeln!(out, "  %lt{0}_{0} = load i64, i64* %gt{0}_{0}, align 8", c0).ok();
        } else if let Some(cn) = total_const_name {
            writeln!(out, "  %lt{0}_{0} = load i64, i64* @{1}, align 8", c0, cn).ok();
        } else {
            writeln!(out, "  %lt{0}_{0} = add i64 0, 0", c0).ok();
        }
        writeln!(out, "  br label %_hdr").ok();
        writeln!(out, "_hdr:").ok();
        writeln!(out, "  %gp{0}_{1} = getelementptr inbounds %State, %State* %state, i32 0, i32 {1}", c0 + 1, counter_idx).ok();
        writeln!(out, "  %lp{0}_{0} = load i64, i64* %gp{0}_{1}, align 8", c0 + 1, counter_idx).ok();
        let cmp_reg = format!("%cp{}", c0 + 2);
        writeln!(out, "  {0} = icmp slt i64 %lp{1}_{1}, %lt{2}_{2}", cmp_reg, c0 + 1, c0).ok();
        writeln!(out, "  br i1 {}, label %_body, label %_done", cmp_reg).ok();
        writeln!(out, "_body:").ok();
        self.ssa_state_reg = None; // memory mode: writes go through GEP+store
        self.returns_i64 = false;
        self.pre_load_all_fields(out, "%state");
        for s in body {
            if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                self.emit_stmt(out, s, "  ");
            }
        }
        self.ssa_old_float_regs.clear();
        self.ssa_old_int_regs.clear();
        // Increment counter via GEP+store
        let inc = format!("%inc{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "  {0} = add i64 %lp{1}_{1}, 1", inc, c0 + 1).ok();
        let sg = format!("%sg{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "  {0} = getelementptr inbounds %State, %State* %state, i32 0, i32 {1}", sg, counter_idx).ok();
        writeln!(out, "  store i64 {}, i64* {}, align 8", inc, sg).ok();
        writeln!(out, "  br label %_hdr").ok();
        writeln!(out, "_done:").ok();
        let saved = std::mem::take(&mut self.pending_post_hoist);
        self.emit_hoisted_post_loop_prints(out, &saved);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a `main()` that uses per-field GEP loads/stores for all-convergent
    /// programs. Loads each field via GEP at tick entry, runs each reactive txn's
    /// precondition and body inline with direct GEP stores for modifications,
    /// avoiding the wide %State load/store + extractvalue/insertvalue pattern.
    /// Handles trigger sampling inline (via lazy emit_trg_load in emit_expr),
    /// and the wake path (__rt_wait) when has_wake_triggers is set.
    pub(crate) fn emit_ssa_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        has_wake_triggers: bool,
    ) {
        self.fn_ret_ty = "i32".to_string();
        let attr = self.slp_attr("main", "#3");
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        // Detect canonical loop pattern: simple single counter [count < bound]
        let has_canonical_loop = txns.len() == 1 && {
            let pre = &txns[0].1.contract.pre_condition;
            match pre {
                Expr::Lt(lhs, rhs) => {
                    matches!(lhs.as_ref(), Expr::Identifier(_))
                        && (matches!(rhs.as_ref(), Expr::Identifier(_)) || matches!(rhs.as_ref(), Expr::Integer(_)))
                }
                _ => false
            }
        };
        if has_canonical_loop {
            let txn = &txns[0].1;
            let counter_name = if let Expr::Lt(lhs, _) = &txn.contract.pre_condition {
                if let Expr::Identifier(name) = lhs.as_ref() { Some(name.clone()) } else { None }
            } else { None };
            if let Some(ref cname) = counter_name {
                let bound_name = if let Expr::Lt(_, rhs) = &txn.contract.pre_condition {
                    if let Expr::Identifier(name) = rhs.as_ref() { Some(name.clone()) }
                    else if let Expr::Integer(n) = rhs.as_ref() { Some(n.to_string()) }
                    else { None }
                } else { None };
                // Load bound once before loop
                if let Some(ref bname) = bound_name {
                    if let Some(&b_idx) = self.field_index_map.get(bname) {
                        let b_gep = format!("%gep_bn{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", b_gep, b_idx).ok();
                        let b_val = format!("%val_bn{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = load i64, i64* {}, align 8", b_val, b_gep).ok();
                        // Check if bound is compile-time or runtime
                        let bound_imm = if bname.parse::<i64>().is_ok() { bname.clone() } else { b_val.clone() };
                        let pi_name = format!("%pi_{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  br label %phdr").ok();
                        writeln!(out, "  phdr:").ok();
                        let pn_name = format!("%pn_{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = phi i64 [ 0, %entry ], [ {}, %platch ]", pi_name, pn_name).ok();
                        let pc_name = format!("%pc_{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = icmp slt i64 {}, {}", pc_name, pi_name, bound_imm).ok();
                        writeln!(out, "  br i1 {}, label %ptick, label %pdoneloop", pc_name).ok();
                        writeln!(out, "  ptick:").ok();
                        // The old tick label is skipped — we use ptick instead
                        self.phi_induction_reg = Some((cname.clone(), pi_name.clone(), pn_name.clone()));
                    }
                }
            }
        }
        if !has_canonical_loop {
            writeln!(out, "  br label %tick").ok();
            writeln!(out, "  tick:").ok();
        }
        self.ssa_state_reg = None;
        for (name, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let pre = &txn.contract.pre_condition;
            
            // Detect terminating final guards and replace them with
            // post-loop field-based prints. A "terminating final guard" is a
            // Guarded statement whose body ends with term! (program exit).
            // Replacing it with a post-loop field load removes the per-iteration
            // branch, enabling LLVM to identify reductions and vectorize.
            let (body_stmts, post_hoist): (Vec<&Statement>, Vec<(String, String)>) = {
                let mut stmts: Vec<&Statement> = txn.body.iter()
                    .filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }))
                    .collect();
                let mut hoist: Vec<(String, String)> = Vec::new(); // (field_name, intrinsic_name)
                // Build mapping from let-binding names to field names (e.g., nchecksum -> checksum)
                // by scanning assignment statements like &checksum = nchecksum;
                let mut let_to_field: HashMap<String, String> = HashMap::new();
                for stmt in &txn.body {
                    if let Statement::Assignment { lhs: Expr::OwnedRef(fname), expr, .. } = stmt {
                        if self.field_index_map.contains_key(fname) {
                            // Try to extract identifier from expr Box<Expr> using string hack
                            let s = format!("{:?}", expr);
                            if let Some(let_name) = s.strip_prefix("Identifier(\"").and_then(|s| s.split('"').next()) {
                                let_to_field.insert(let_name.to_string(), fname.clone());
                            }
                        }
                    }
                }
                'outer: while let Some(last_idx) = stmts.len().checked_sub(1) {
                    if let Statement::Guarded { statements, .. } = &stmts[last_idx] {
                        let is_terminating = statements.iter().any(|s| matches!(s, Statement::TermBang { .. }));
                        if !is_terminating { break; }
                        // Extract the print intrinsic from the guard body
                        for s in statements {
                            if let Statement::Expression(Expr::IntrinsicCall { intrinsic, args }) = s {
                                let intrinsic_name = intrinsic.name();
                                if let Some(Expr::Identifier(fname)) = args.first() {
                                    if self.field_index_map.contains_key(fname) {
                                        hoist.push((fname.clone(), intrinsic_name.to_string()));
                                    }
                                }
                            }
                            if let Statement::TermBang { values, swan_song, .. } = s {
                                // Check values (outputs before ->)
                                for v in values {
                                    if let Some(Expr::IntrinsicCall { intrinsic, args }) = v {
                                        let intrinsic_name = intrinsic.name();
                                        if let Some(Expr::Identifier(fname)) = args.first() {
                                            if self.field_index_map.contains_key(fname) {
                                                hoist.push((fname.clone(), intrinsic_name.to_string()));
                                            }
                                        }
                                    }
                                }
                                // Check swan_song (the expression after ->)
                                if let Some(ss) = swan_song {
                                    if let Statement::Expression(Expr::IntrinsicCall { intrinsic, args }) = ss.as_ref() {
                                        let intrinsic_name = intrinsic.name();
                                        if let Some(Expr::Identifier(fname)) = args.first() {
                                            if self.field_index_map.contains_key(fname) {
                                                hoist.push((fname.clone(), intrinsic_name.to_string()));
                                            } else if let Some(mapped_field) = let_to_field.get(fname) {
                                                hoist.push((mapped_field.clone(), intrinsic_name.to_string()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if !hoist.is_empty() {
                            stmts.pop();
                        }
                        break 'outer;
                    } else { break; }
                }
                (stmts, hoist)
            };
            
            if self.phi_induction_reg.is_some() {
                // Canonical loop: phi induction variable already guarantees precondition.
                // Skip precondition check — body runs unconditionally.
                self.pre_load_all_fields(out, "%state");
                if let Some((ref cname, ref pi_reg, _)) = self.phi_induction_reg {
                    self.ssa_old_int_regs.insert(cname.clone(), pi_reg.clone());
                }
                self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
                self.terminated = false;
                self.returns_i64 = false;
                self.loop_exit_label = Some("done".into());
                for s in body_stmts { self.emit_stmt(out, s, "  "); }
                self.loop_exit_label = None;
                self.ssa_old_float_regs.clear();
                self.ssa_old_int_regs.clear();
                // Store post_hoist for emission after loop exit (in pdoneloop)
                self.pending_post_hoist = post_hoist.clone();
            } else if !matches!(pre, Expr::Bool(true)) {
                self.pre_load_all_fields(out, "%state");
                let cond = self.emit_expr(out, pre, "  ");
                let i1 = if cond.ty == Type::Bool {
                    cond.name.clone()
                } else {
                    let i1 = format!("%pi{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
                    i1
                };
                let body_l = format!("b_{}", name);
                let skip_l = format!("s_{}", name);
                let done_l = format!("done_{}", name);
                writeln!(out, "  br i1 {}, label %{}, label %{}", i1, body_l, done_l).ok();
                writeln!(out, "  {}:", body_l).ok();
                self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
                self.terminated = false;
                self.returns_i64 = false;
                self.pre_load_all_fields(out, "%state");
                self.loop_exit_label = Some("done".into());
                for s in body_stmts { self.emit_stmt(out, s, "  "); }
                self.loop_exit_label = None;
                self.ssa_old_float_regs.clear();
                self.ssa_old_int_regs.clear();
                writeln!(out, "  br label %{}", skip_l).ok();
                writeln!(out, "  {}:", done_l).ok();
                // Post-loop: emit hoisted field-based prints, then chain to next txn
                self.emit_hoisted_post_loop_prints(out, &post_hoist);
                if self.phi_induction_reg.is_some() {
                    writeln!(out, "  br label %platch").ok();
                } else {
                    writeln!(out, "  br label %{}", skip_l).ok();
                }
                writeln!(out, "  {}:", skip_l).ok();
            } else {
                self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
                self.terminated = false;
                self.returns_i64 = false;
                self.pre_load_all_fields(out, "%state");
                // Override counter field with phi induction register if available
                if let Some((ref cname, ref pi_reg, _)) = self.phi_induction_reg {
                    self.ssa_old_int_regs.insert(cname.clone(), pi_reg.clone());
                }
                self.loop_exit_label = Some("done".into());
                for s in body_stmts { self.emit_stmt(out, s, "  "); }
                self.loop_exit_label = None;
                self.ssa_old_float_regs.clear();
                self.ssa_old_int_regs.clear();
                // Post-loop: emit hoisted field-based prints
                self.emit_hoisted_post_loop_prints(out, &post_hoist);
            }
        }
        if let Some((_, ref pi_reg, ref pn_reg)) = self.phi_induction_reg.clone() {
            // Canonical loop: emit latch and done labels
            writeln!(out, "  br label %platch").ok();
            writeln!(out, "  platch:").ok();
            writeln!(out, "  {} = add i64 {}, 1", pn_reg, pi_reg).ok();
            writeln!(out, "  br label %phdr").ok();
            writeln!(out, "  pdoneloop:").ok();
            // Emit post-loop prints after loop exit
            let saved = std::mem::take(&mut self.pending_post_hoist);
            self.emit_hoisted_post_loop_prints(out, &saved);
            writeln!(out, "  ret i32 0").ok();
        } else if let Some(ref cond) = self.exit_condition.clone() {
            let val = self.emit_exit_expr(out, cond, "  ");
            let tr = format!("%t{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
            if has_wake_triggers {
                writeln!(out, "  br i1 {}, label %done, label %wait", tr).ok();
                writeln!(out, "  wait:").ok();
                writeln!(out, "  call void @__rt_wait()").ok();
                writeln!(out, "  br label %tick").ok();
            } else {
                writeln!(out, "  br i1 {}, label %done, label %tick", tr).ok();
            }
            writeln!(out, "  done:").ok();
        } else if has_wake_triggers {
            writeln!(out, "  call void @__rt_wait()").ok();
            writeln!(out, "  br label %tick").ok();
        } else {
            writeln!(out, "  br label %tick").ok();
        }
        self.phi_induction_reg = None;
        self.pending_post_hoist.clear();
        if self.exit_condition.is_none() && self.phi_induction_reg.is_none() {
            writeln!(out, "  done:").ok();
        }
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit post-loop field-based prints for hoisted terminating guards.
    /// After the loop exits, load fields from %state and print their final values.
    fn emit_hoisted_post_loop_prints(&mut self, out: &mut String, hoisted: &[(String, String)]) {
        for (fname, intrinsic_name) in hoisted {
            if let Some(&idx) = self.field_index_map.get(fname) {
                let ty = self.field_types[idx].clone();
                let gep = format!("%gep_pl_{}", self.txn_counter); self.txn_counter += 1;
                let val = format!("%val_pl_{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}",
                    gep, idx).ok();
                match ty.as_str() {
                    "float" => {
                        writeln!(out, "  {} = load float, float* {}, align 4", val, gep).ok();
                        self.emit_post_print(out, intrinsic_name, &val, "float", "  ");
                    }
                    _ => {
                        writeln!(out, "  {} = load i64, i64* {}, align 8", val, gep).ok();
                        self.emit_post_print(out, intrinsic_name, &val, "i64", "  ");
                    }
                }
            }
        }
    }

    /// Emit a single print intrinsic call from raw register names (no TypedRegister).
    fn emit_post_print(&mut self, out: &mut String, name: &str, reg: &str, ty: &str, indent: &str) {
        let so = format!("%ppl_a{}", self.txn_counter); self.txn_counter += 1;
        let fmt_reg = format!("%ppl_c{}", self.txn_counter); self.txn_counter += 1;
        let res = format!("%ppl_r{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
        match name {
            "print_int" => {
                writeln!(out, "{}{} = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0", indent, fmt_reg).ok();
                writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, i64 {})", indent, res, so, fmt_reg, reg).ok();
            }
            "print_float" => {
                let fd = format!("%ppl_f{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = fpext float {} to double", indent, fd, reg).ok();
                writeln!(out, "{}{} = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0", indent, fmt_reg).ok();
                writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, double {})",
                    indent, res, so, fmt_reg, fd).ok();
            }
            "putchar" => {
                let ct = format!("%ppl_g{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = trunc i64 {} to i32", indent, ct, reg).ok();
                writeln!(out, "{}{} = call i32 @fputc(i32 {}, ptr {})", indent, res, ct, so).ok();
            }
            "println" => {
                let so2 = format!("%ppl_b{}", self.txn_counter); self.txn_counter += 1;
                let res_ff = format!("%ppl_e{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so2).ok();
                writeln!(out, "{}{} = getelementptr [4 x i8], [4 x i8]* @FMT_STR, i64 0, i64 0", indent, fmt_reg).ok();
                writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, ptr {})",
                    indent, res, so, fmt_reg, reg).ok();
                writeln!(out, "{}{} = call i32 @fflush(ptr {})", indent, res_ff, so2).ok();
            }
            _ => {}
        }
    }

    /// Emit a `main()` that folds ALL reactive transactions into a single
    /// register-pipeline loop.  Each txn gets an SSA phi node for its counter;
    /// the loop terminates when all counters reach their bounds.
    /// Assumes all txns are pure/effectively-pure with bounded_pre + increments.
    /// After the entry setup, performs enum trigger dispatch and switch-based
    /// execution (merged from the original emit_enum_main design).
    pub(crate) fn emit_folded_multi_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        enum_sizes: &[(String, Option<u64>)],
        enum_keys: &HashMap<String, Vec<i64>>,
        fold_params: &HashMap<String, FoldParam>,
        fold_pure: &HashMap<String, (bool, Option<i64>)>,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        composed_fn: Option<&str>,
        composed_trig_map: Option<&HashMap<String, Vec<(i64, String)>>>,
        all_internal_map: Option<&HashMap<String, (usize, i64)>>,
        has_wake: bool,
    ) {
        let c0 = self.txn_counter;
        // Deduplicate by counter index: multiple txns may share the same counter
        let mut uniq: Vec<(usize, String)> = Vec::new();
        let mut seen_idxs: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut first_tidx: Option<usize> = None;
        for (_, fp) in fold_params.iter() {
            if seen_idxs.insert(fp.counter_idx) {
                uniq.push((fp.counter_idx, format!("c{}", fp.counter_idx)));
                if first_tidx.is_none() {
                    first_tidx = fp.bound_field_idx;
                }
            }
        }
        self.fn_ret_ty = "i32".to_string();
        let main_attr = self.slp_attr("main", if has_wake { "#3" } else { "#0" });
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", main_attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        if self.has_async_txns && !self.is_lightweight_async {
            let count = self.async_txn_names.len() as i32;
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x void (%State*)*]* @thread_pool_fns to i8**", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @brief_thread_pool_init(i32 {}, i8** %tp_fn_ptr)", count).ok();
        }
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "tick:").ok();

        // Sample triggers (clone trigger data to avoid borrow conflict)
        let trigger_data: Vec<(String, crate::ast::LinkRef, crate::ast::Type)> = enum_sizes.iter()
            .filter_map(|(tn, _)| {
                self.triggers.get(tn).map(|t| {
                    let rn = format!("%sz_{}", tn);
                    let addr = &t.address;
                    (rn, addr.clone(), t.ty.clone())
                })
            })
            .collect();
        for (rn, addr, ty) in &trigger_data {
            self.emit_trg_load(out, "  ", rn, addr, ty);
        }

        // Build switch dispatch
        let txn_name = composed_fn.unwrap_or(
            txns.first().map(|(n, _)| n.as_str()).unwrap_or("__missing")
        );

        // Build per-trigger-value composed function lookup (for chain branching)
        let root_txn = txns.first().map(|(n, _)| n.as_str()).unwrap_or("");
        let mut trig_to_fn: HashMap<i64, String> = HashMap::new();
        if let Some(ctm) = composed_trig_map {
            if let Some(entries) = ctm.get(root_txn) {
                for (val, fname) in entries {
                    trig_to_fn.insert(*val, fname.clone());
                }
            }
        }

        let total_combos: u64 = enum_sizes.iter().map(|(_, s)| s.unwrap_or(1)).product();

        // Helper: check if a function name maps to an all-internal
        // (pure counter) case and return its (ci, total_val) if so.
        let all_internal_lookup = |fn_name: &str| -> Option<(usize, i64)> {
            all_internal_map.and_then(|m| m.get(fn_name).copied())
        };

        // "Done" label for each branch — in wake mode this is either exit_check
        // (when #!exit is declared), async_phase (when async txns exist), or do_wait.
        // In one-shot mode this is "exit" (ret i32 0).
        // All case arms branch to done_label; done_label routes through the
        // exit condition check (if present) before reaching the wait loop.
        let done_label = if has_wake {
            if self.exit_condition.is_some() { "exit_check" }
            else if self.has_async_txns && !self.is_lightweight_async { "async_phase" }
            else { "do_wait" }
        } else { "exit" };
        if !has_wake { writeln!(out, "  br label %dispatch").ok(); writeln!(out, "dispatch:").ok(); }

        /// Emit one or more per-txn folded loops for a case arm.
        /// When fold_params contains entries, emits one loop per entry;
        /// otherwise falls back to the legacy single-txn params.
        let emit_case_folded_loops = |this: &mut LlvmBackend,
                                      out: &mut String,
                                      prefix: &str,
                                      fn_name: &str,
                                      ci: usize,
                                      ti: Option<usize>,
                                      tcn: Option<&str>|
        {
            if !fold_params.is_empty() {
                // Multi-txn: emit one folded loop per bounded-counter txn
                for (ptxn_name, fp) in fold_params.iter() {
                    let sub_prefix = format!("{}_{}", prefix, ptxn_name);
                    if let Some(&(pure, tv)) = fold_pure.get(ptxn_name) {
                        if pure {
                            if let Some(tv) = tv {
                                writeln!(out, "  %pc_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", sub_prefix, fp.counter_idx).ok();
                                writeln!(out, "  store i64 {}, i64* %pc_{}, align 8", tv, sub_prefix).ok();
                                continue;
                            } else {
                                let ptcn_ref = fp.bound_const_name.as_deref();
                                this.emit_folded_loop(out, ptxn_name, fp.counter_idx, fp.bound_field_idx, ptcn_ref, &sub_prefix, true, None, 1, fp.is_decreasing, fp.bound_literal);
                                continue;
                            }
                        }
                    }
                    let ptcn_ref = fp.bound_const_name.as_deref();
                    let body = txns.iter().find(|(n, _)| n == ptxn_name).map(|(_, t)| t.body.as_slice());
                    this.emit_folded_loop(out, ptxn_name, fp.counter_idx, fp.bound_field_idx, ptcn_ref, &sub_prefix, false, body, 4, fp.is_decreasing, fp.bound_literal);
                }
            } else {
                let body = txns.iter().find(|(n, _)| n == fn_name).map(|(_, t)| t.body.as_slice());
                this.emit_folded_loop(out, fn_name, ci, ti, tcn, prefix, false, body, 4, false, None);
            }
        };

        if total_combos == 1 && enum_sizes.len() == 1 {
            // Single-value trigger: just fall through to the loop
            let fn_name = trig_to_fn.get(&0).map(|s| s.as_str()).unwrap_or(txn_name);
            if let Some((ci, tv)) = all_internal_lookup(fn_name) {
                writeln!(out, "  %pc_sc = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", ci).ok();
                writeln!(out, "  store i64 {}, i64* %pc_sc, align 8", tv).ok();
            } else {
                emit_case_folded_loops(self, out, "sc", fn_name, counter_idx, total_idx, total_const_name);
            }
            if has_wake {
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  ret i32 0").ok();
            }
        } else if enum_sizes.len() == 1 {
            // Single enumerable trigger — one switch axis
            let tn = &enum_sizes[0].0;
            let n = enum_sizes[0].1.unwrap_or(2);
            let native_name = txn_name.to_string();
            // Use extracted keys when available, otherwise fall back to dense 0..n
            let keys: Vec<i64> = enum_keys.get(tn).cloned().unwrap_or_else(|| (0..n as i64).collect());

            // Check if all case arms produce identical code (uniform-body skip).
            // When trig_to_fn maps all keys to the same function and all have
            // the same all-internal status, the switch dispatch is redundant.
            let uniform_body = keys.len() > 1 && {
                let first_fn = trig_to_fn.get(&keys[0]).map(|s| s.as_str()).unwrap_or(&native_name);
                let first_ai = all_internal_lookup(first_fn);
                keys[1..].iter().all(|k| {
                    let fn_name = trig_to_fn.get(k).map(|s| s.as_str()).unwrap_or(&native_name);
                    fn_name == first_fn && all_internal_lookup(fn_name) == first_ai
                })
            };

            if uniform_body {
                // All case arms identical — skip the switch, emit one body
                let fn_name = trig_to_fn.get(&keys[0]).map(|s| s.as_str()).unwrap_or(&native_name);
                if let Some((ci, tv)) = all_internal_lookup(fn_name) {
                    writeln!(out, "  %pc_uni = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", ci).ok();
                    writeln!(out, "  store i64 {}, i64* %pc_uni, align 8", tv).ok();
                } else {
                    emit_case_folded_loops(self, out, "uni", fn_name, counter_idx, total_idx, total_const_name);
                }
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    writeln!(out, "  ret i32 0").ok();
                }
                // Residual label for safety (unreachable for fully-covered enums)
                writeln!(out, "{}_residual:", tn).ok();
                writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    writeln!(out, "  br label %{}_residual_loop", tn).ok();
                    writeln!(out, "{}_residual_loop:", tn).ok();
                    writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                    writeln!(out, "  br label %{}_residual_loop", tn).ok();
                }
            } else {
            let key_count = keys.len();
            // Try perfect hashing for sparse key sets (gap ratio > 4).
            let (use_hash, multiplier, hash_shift): (bool, u64, u32) =
                if sparsity_ratio(&keys) > 4.0 {
                    if let Some((m, s)) = find_perfect_hash(&keys) {
                        (true, m, s)
                    } else { (false, 0, 0) }
                } else { (false, 0, 0) };
            let dispatch_val = if use_hash {
                // Emit perfect hash: h(k) = (k * M) >> S
                writeln!(out, "  %hm_{} = mul i64 %sz_{}, {}", c0, tn, multiplier).ok();
                writeln!(out, "  %hs_{} = lshr i64 %hm_{}, {}", c0, c0, hash_shift).ok();
                format!("%hs_{}", c0)
            } else {
                format!("%sz_{}", tn)
            };
            writeln!(out, "  switch i64 {}, label %{}_residual [", dispatch_val, tn).ok();
            for (idx, _key) in keys.iter().enumerate() {
                let label = format!("{}_{}", tn, idx);
                writeln!(out, "    i64 {}, label %{}_case_{}", idx, tn, idx).ok();
            }
            writeln!(out, "  ]").ok();
            for (idx, key) in keys.iter().enumerate() {
                let prefix = format!("{}_{}", tn, idx);
                writeln!(out, "{}_case_{}:", tn, idx).ok();
                // For hashed dispatch, verify the original key matches (safety guard)
                if use_hash {
                    writeln!(out, "  %vg_{}_{} = icmp eq i64 %sz_{}, {}", c0, idx, tn, key).ok();
                    writeln!(out, "  br i1 %vg_{}_{}, label %{}_safe_{}, label %{}_residual", c0, idx, tn, idx, tn).ok();
                    writeln!(out, "{}_safe_{}:", tn, idx).ok();
                }
                let fn_name = trig_to_fn.get(key).map(|s| s.as_str()).unwrap_or(&native_name);
                if let Some((ci, tv)) = all_internal_lookup(fn_name) {
                    writeln!(out, "  %pc_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", prefix, ci).ok();
                    writeln!(out, "  store i64 {}, i64* %pc_{}, align 8", tv, prefix).ok();
                } else {
                    emit_case_folded_loops(self, out, &prefix, fn_name, counter_idx, total_idx, total_const_name);
                }
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    writeln!(out, "  ret i32 0").ok();
                }
            }
            writeln!(out, "{}_residual:", tn).ok();
            writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
            if has_wake {
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  br label %{}_residual_loop", tn).ok();
                writeln!(out, "{}_residual_loop:", tn).ok();
                writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                writeln!(out, "  br label %{}_residual_loop", tn).ok();
            }
            }
        } else {
            // Multi-trigger case: just fall through to standard reactor
            if has_wake {
                writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  br label %residual_entry").ok();
                writeln!(out, "residual_entry:").ok();
                self.emit_inline_init_stores(out, "%state");
                writeln!(out, "  br label %residual_loop").ok();
                writeln!(out, "residual_loop:").ok();
                writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                writeln!(out, "  br label %residual_loop").ok();
            }
        }

        if has_wake {
            let has_exit = self.exit_condition.is_some();
            if has_exit {
                let cond = self.exit_condition.clone().unwrap();
                writeln!(out, "exit_check:").ok();
                let val = self.emit_exit_expr(out, &cond, "  ");
                let tr = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
                if self.has_async_txns && !self.is_lightweight_async {
                    writeln!(out, "  br i1 {}, label %done, label %async_phase", tr).ok();
                } else {
                    writeln!(out, "  br i1 {}, label %done, label %do_wait", tr).ok();
                }
            }
            if self.has_async_txns && !self.is_lightweight_async {
                writeln!(out, "async_phase:").ok();
                self.emit_async_phase(out);
                writeln!(out, "  br label %do_wait").ok();
            }
            writeln!(out, "do_wait:").ok();
            writeln!(out, "  call void @__rt_wait()").ok();
            writeln!(out, "  br label %tick").ok();
            if has_exit {
                writeln!(out, "done:").ok();
                writeln!(out, "  ret i32 0").ok();
            }
        }

        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    pub(crate) fn emit_folded_pure_counter(&mut self, out: &mut String, counter_idx: usize, total_value: i64) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        writeln!(out, "  %gp = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", counter_idx).ok();
        writeln!(out, "  store i64 {}, i64* %gp, align 8", total_value).ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a `step()` function for the trg reactive dirty-flag system.
    /// Reads trigger variables via volatile load (liveness anchor),
    /// checks dirty flags, and recomputes dependent variables in topological order.
    /// The step() function is called when any trigger fires.
    #[allow(unused_variables)]
    pub(crate) fn emit_trg_step(
        &mut self,
        out: &mut String,
        dep_graph: &DependencyGraph,
        trigger_names: &[String],
    ) {
        let mut tc = self.txn_counter;
        writeln!(out, "define void @step(%State* noalias nocapture %state, i64 %dirty_in) local_unnamed_addr #0 {{").ok();
        // Load dirty flags into a mutable alloca
        let dirty_slot = format!("%dirty_{}", tc); tc += 1;
        writeln!(out, "  {} = alloca i64, align 8", dirty_slot).ok();
        writeln!(out, "  store i64 %dirty_in, i64* {}, align 8", dirty_slot).ok();
        // Volatile-load all trigger variables (liveness anchor + value observation)
        for trg_name in trigger_names {
            if let Some(&idx) = self.field_index_map.get(trg_name) {
                let gep = format!("%gtrg_{}", tc); tc += 1;
                writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", gep, idx).ok();
                let ld = format!("%ltrg_{}", tc); tc += 1;
                writeln!(out, "  {} = load volatile i64, i64* {}, align 8", ld, gep).ok();
                // Volatile store back — liveness anchor prevents elimination
                writeln!(out, "  store volatile i64 {}, i64* {}, align 8", ld, gep).ok();
            }
        }
        // For each non-trg variable in topological order:
        // if any dependency is dirty, recompute
        for var_name in &dep_graph.topo_order {
            if dep_graph.is_trg.contains(var_name) { continue; }
            let deps = match dep_graph.dependencies.get(var_name) {
                Some(d) if !d.is_empty() => d,
                _ => continue,
            };
            let idx = match self.field_index_map.get(var_name) {
                Some(&i) => i,
                None => continue,
            };
            // Build dirty check: for each dep, check if its bit is set
            let mut checks = Vec::new();
            for dep_name in deps {
                if let Some(&bit) = dep_graph.bit_index.get(dep_name) {
                    let mask = 1u64 << bit;
                    let ld = format!("%ld_{}", tc); tc += 1;
                    writeln!(out, "  {} = load i64, i64* {}, align 8", ld, dirty_slot).ok();
                    let and = format!("%and_{}", tc); tc += 1;
                    writeln!(out, "  {} = and i64 {}, {}", and, ld, mask).ok();
                    let cmp = format!("%cmp_{}", tc); tc += 1;
                    writeln!(out, "  {} = icmp ne i64 {}, 0", cmp, and).ok();
                    checks.push(cmp);
                }
            }
            if checks.is_empty() { continue; }
            // Combine with OR
            let cond = if checks.len() == 1 {
                checks[0].clone()
            } else {
                let mut cur = checks[0].clone();
                for c in &checks[1..] {
                    let or = format!("%or_{}", tc); tc += 1;
                    writeln!(out, "  {} = or i1 {}, {}", or, cur, c).ok();
                    cur = or;
                }
                cur
            };
            // Branch: body (recompute) or skip
            let body_label = format!("%step_body_{}", var_name);
            let skip_label = format!("%step_skip_{}", var_name);
            writeln!(out, "  br i1 {}, label %{}, label %{}", cond, body_label, skip_label).ok();
            writeln!(out, "{}:", body_label).ok();
            // Load all dependency values and store as placeholder
            // (full recomputation requires tracking each StateDecl's expr)
            for dep_name in deps {
                if let Some(&dep_idx) = self.field_index_map.get(dep_name) {
                    let gdep = format!("%gdep_{}", tc); tc += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", gdep, dep_idx).ok();
                    let ldep = format!("%ldep_{}", tc); tc += 1;
                    writeln!(out, "  {} = load i64, i64* {}, align 8", ldep, gdep).ok();
                    let _ = ldep; // consumed by future recompute expr
                }
            }
            // Store first dependency value as proxy (placeholder for recomputation)
            if let Some(first_dep) = deps.first() {
                if let Some(&first_idx) = self.field_index_map.get(first_dep) {
                    let gsrc = format!("%gsrc_{}", tc); tc += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", gsrc, first_idx).ok();
                    let lsrc = format!("%lsrc_{}", tc); tc += 1;
                    writeln!(out, "  {} = load i64, i64* {}, align 8", lsrc, gsrc).ok();
                    let gdst = format!("%gdst_{}", tc); tc += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", gdst, idx).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8 ; recompute {}", lsrc, gdst, var_name).ok();
                }
            }
            writeln!(out, "  br label %{}", skip_label).ok();
            writeln!(out, "{}:", skip_label).ok();
        }
        // Clear all dirty flags
        writeln!(out, "  store i64 0, i64* {}, align 8", dirty_slot).ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
        self.txn_counter = tc;
    }
}

/// Emit epoll_wait + per-trigger read + dirty-bit-set for the event loop.
/// Called instead of @__rt_wait() when the program has built-in triggers.
pub(crate) fn emit_trg_event_epoll_wait(backend: &mut LlvmBackend, out: &mut String) {
    let tc = backend.txn_counter; backend.txn_counter += 20;
    let epfd_idx = match backend.field_index_map.get("__trg_epfd") {
        Some(&i) => i,
        None => return,
    };
    let evt = format!("%evt_{}", tc);
    writeln!(out, "  {} = alloca i8, i64 16, align 8", evt).ok();
    let ep_gep = format!("%epg_{}", tc);
    writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", ep_gep, epfd_idx).ok();
    let ep_ld = format!("%epl_{}", tc);
    writeln!(out, "  {} = load i32, i32* {}, align 4", ep_ld, ep_gep).ok();
    let n_ev = format!("%nev_{}", tc);
    writeln!(out, "  {} = call i32 @epoll_wait(i32 {}, i8* {}, i32 1, i32 -1)", n_ev, ep_ld, evt).ok();
    let ev_cmp = format!("%evc_{}", tc);
    writeln!(out, "  {} = icmp sgt i32 {}, 0", ev_cmp, n_ev).ok();
    let ev_body = format!("ev_body_{}", tc);
    let ev_done = format!("ev_done_{}", tc);
    writeln!(out, "  br i1 {}, label %{}, label %{}", ev_cmp, ev_body, ev_done).ok();
    writeln!(out, "{}:", ev_body).ok();
    let ev_data = format!("%evd_{}", tc);
    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 8", ev_data, evt).ok();
    let ev_data_u64 = format!("%evdu_{}", tc);
    writeln!(out, "  {} = bitcast i8* {} to i64*", ev_data_u64, ev_data).ok();
    let ev_bit = format!("%evb_{}", tc);
    writeln!(out, "  {} = load i64, i64* {}, align 8", ev_bit, ev_data_u64).ok();
    for (name, trg) in &backend.triggers {
        let bit = backend.dep_graph.bit_index.get(name).copied().unwrap_or(0);
        let bit_check = format!("%bc_{}_{}", tc, name);
        writeln!(out, "  {} = icmp eq i64 {}, {}", bit_check, ev_bit, bit).ok();
        let t_body = format!("tb_{}_{}", tc, name);
        let t_skip = format!("ts_{}_{}", tc, name);
        writeln!(out, "  br i1 {}, label %{}, label %{}", bit_check, t_body, t_skip).ok();
        writeln!(out, "{}:", t_body).ok();
        match &trg.address {
            crate::ast::LinkRef::Stdin => {
                let ch_slot = format!("%ch_{}_{}", tc, name);
                writeln!(out, "  {} = alloca i8, i64 1, align 1", ch_slot).ok();
                writeln!(out, "  %rd_{}_{} = call i64 @read(i32 0, i8* {}, i64 1)", tc, name, ch_slot).ok();
                if let Some(&idx) = backend.field_index_map.get(name) {
                    let sge = format!("%sge_{}_{}", tc, name);
                    writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", sge, idx).ok();
                    let ch_ld = format!("%chld_{}_{}", tc, name);
                    writeln!(out, "  {} = load i8, i8* {}, align 1", ch_ld, ch_slot).ok();
                    let ch_z = format!("%chz_{}_{}", tc, name);
                    writeln!(out, "  {} = zext i8 {} to i64", ch_z, ch_ld).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8", ch_z, sge).ok();
                }
            }
            crate::ast::LinkRef::Timer(_hz) => {
                if let Some(&idx) = backend.field_index_map.get(name) {
                    let sge = format!("%sge_{}_{}", tc, name);
                    writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", sge, idx).ok();
                    let cur = format!("%cur_{}_{}", tc, name);
                    writeln!(out, "  {} = load i64, i64* {}, align 8", cur, sge).ok();
                    let inc = format!("%inc_{}_{}", tc, name);
                    writeln!(out, "  {} = add i64 {}, 1", inc, cur).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8", inc, sge).ok();
                }
            }
            crate::ast::LinkRef::Signal(_sig) => {
                if let Some(&idx) = backend.field_index_map.get(name) {
                    let sge = format!("%sge_{}_{}", tc, name);
                    writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", sge, idx).ok();
                    writeln!(out, "  store i64 1, i64* {}, align 8", sge).ok();
                }
            }
            _ => {}
        }
        let drx = format!("%drx_{}_{}", tc, name);
        writeln!(out, "  {} = add i64 {}, {}", drx, 1u64 << bit, bit).ok();
        writeln!(out, "  call void @step(%State* %state, i64 {})", drx).ok();
        writeln!(out, "  br label %{}", t_skip).ok();
        writeln!(out, "{}:", t_skip).ok();
    }
    writeln!(out, "  br label %{}", ev_done).ok();
    writeln!(out, "{}:", ev_done).ok();
}
