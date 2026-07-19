// ── Loop Emission: SSA Register Pipeline ───────────────────────
//
// 2026-07-13: Extracted from monolithic loop_engine.rs (4398 lines).
// Implements Strategy 4 (SSA Register Pipeline) for multi-txn reactive
// programs, modulo-switch dispatch, and folded multi-txn dispatch.
//
// Strategies:
//   a) SSA canonical loop (single txn, simple bound)
//   b) SSA with precondition check (rct txn with pre/post)
//   c) SSA no precondition (rct txn with [true][post])
//   d) Modulo-rotated dispatch (K ≤ 8 reactive txns)
//   e) Modulo-switch dispatch (K > 8 reactive txns)
//   f) Folded multi-txn (enum-key dispatch for folded transactions)

use crate::backend::llvm::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write;

impl LlvmBackend {
    // ═══════════════════════════════════════════════════════════════
    // Modulo-Switch Dispatch
    // ═══════════════════════════════════════════════════════════════

    /// Try to detect if the reactive transaction set can be dispatched
    /// via modulo-switch on the counter register.
    fn try_modulo_switch_dispatch(
        &mut self,
        out: &mut String,
        reactive_txns: &[&(String, &crate::ast::Transaction)],
    ) -> bool {
        if reactive_txns.len() < 2 {
            return false;
        }
        let first_pre = &reactive_txns[0].1.contract.pre_condition;
        let (counter_name, divisor) = match self.extract_mod_info(first_pre) {
            Some(info) => info,
            None => return false,
        };
        let mut cases: Vec<(i64, &str)> = Vec::new();
        for (name, txn) in reactive_txns {
            if !txn.is_reactive {
                return false;
            }
            let pre = &txn.contract.pre_condition;
            match self.extract_mod_guard(pre, &counter_name, divisor) {
                Some(val) => cases.push((val, name)),
                None => return false,
            }
        }
        if cases.len() != reactive_txns.len() {
            return false;
        }
        if cases.len() <= 8 {
            // 2026-07-17: Use modulo-rotated loop for K ≤ 8 (sparse_dispatch).
            // emit_modulo_switch_main is one-shot (no loop) and doesn't handle
            // the bounded counter — it was designed for the old pre-check path.
            let txns_ref: Vec<(String, &crate::ast::Transaction)> = reactive_txns.iter()
                .map(|(n, t)| (n.clone(), *t)).collect();
            self.emit_modulo_rotated(out, &txns_ref, &counter_name, divisor, &cases);
        } else {
            self.emit_modulo_switch_main(out, reactive_txns, &counter_name, divisor, &cases);
        }
        true
    }

    /// Extract (counter_name, divisor) from a modulo precondition.
    fn extract_mod_info(&self, expr: &Expr) -> Option<(String, i64)> {
        match expr {
            Expr::BinaryOp(kind, l, r) if *kind == crate::ast::BinaryOpKind::Mod => {
                let counter = match l.as_ref() {
                    Expr::Identifier(n) => n.clone(),
                    _ => return None,
                };
                let divisor = match r.as_ref() {
                    Expr::Decimal(d) => *d,
                    _ => return None,
                };
                Some((counter, divisor))
            }
            // 2026-07-17: Handle Eq(Mod(counter, divisor), value) — the common
            // pattern for `count % 8 == N` which wraps Mod inside Eq.
            Expr::BinaryOp(kind, l, r) if *kind == crate::ast::BinaryOpKind::Eq => {
                self.extract_mod_info(l).or_else(|| self.extract_mod_info(r))
            }
            // 2026-07-17: Handle compound AND expressions like
            // `count < total && count % 8 == N` — extract Mod from either side.
            Expr::BinaryOp(kind, l, r) if *kind == crate::ast::BinaryOpKind::And => {
                self.extract_mod_info(l).or_else(|| self.extract_mod_info(r))
            }
            _ => None,
        }
    }

    /// Extract the expected modulo value from a guard expression.
    fn extract_mod_guard(&self, expr: &Expr, counter_name: &str, divisor: i64) -> Option<i64> {
        match expr {
            // 2026-07-17: Handle compound AND — try both sides.
            Expr::BinaryOp(kind, l, r) if *kind == crate::ast::BinaryOpKind::And => {
                self.extract_mod_guard(l, counter_name, divisor)
                    .or_else(|| self.extract_mod_guard(r, counter_name, divisor))
            }
            Expr::BinaryOp(kind, l, r) if *kind == crate::ast::BinaryOpKind::Eq => {
                let inner = match l.as_ref() {
                    Expr::BinaryOp(k, cl, cr) if *k == crate::ast::BinaryOpKind::Mod
                        && matches!(cl.as_ref(), Expr::Identifier(n) if n == counter_name)
                        && matches!(cr.as_ref(), Expr::Decimal(d) if *d == divisor) => l.as_ref(),
                    _ => return None,
                };
                match r.as_ref() {
                    Expr::Decimal(v) => Some(*v),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Emit a modulo-switch dispatch: check counter % divisor and switch
    /// to the matching transaction body.
    fn emit_modulo_switch_main(
        &mut self,
        out: &mut String,
        _txns: &[&(String, &crate::ast::Transaction)],
        counter_name: &str,
        divisor: i64,
        cases: &[(i64, &str)],
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let counter_idx = self.ctx.field_index_map.get(counter_name).copied().unwrap_or(0);
        let c_gep = self.fun.next_reg_with_prefix("msp");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            c_gep, counter_idx).ok();
        let c_val = self.fun.next_reg_with_prefix("msv");
        writeln!(out, "  {} = load i64, ptr {}, align 8", c_val, c_gep).ok();
        let mod_val = self.fun.next_reg_with_prefix("msm");
        writeln!(out, "  {} = srem i64 {}, {}", mod_val, c_val, divisor).ok();
        writeln!(out, "  switch i64 {}, label %.end [", mod_val).ok();
        for (val, name) in cases {
            writeln!(out, "    i64 {}, label %{}", val, name).ok();
        }
        writeln!(out, "  ]").ok();
        for (_val, name) in cases {
            writeln!(out, "{}:", name).ok();
            writeln!(out, "  call void @txn_{}(ptr %state)", name).ok();
            writeln!(out, "  br label %.end").ok();
        }
        writeln!(out, ".end:").ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ═══════════════════════════════════════════════════════════════
    // Modulo-Rotated Dispatch (K ≤ 8)
    // ═══════════════════════════════════════════════════════════════

    /// Emit a modulo-rotated dispatch loop. Used when the number of
    /// reactive transactions is small (K ≤ 8). The loop body checks
    /// `counter % divisor` and branches to the matching case.
    pub(crate) fn emit_modulo_rotated(
        &mut self,
        out: &mut String,
        _txns: &[(String, &crate::ast::Transaction)],
        counter_name: &str,
        divisor: i64,
        cases: &[(i64, &str)],
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        writeln!(out, "  br label %.mr_loop").ok();
        writeln!(out, ".mr_loop:").ok();
        let counter_idx = self.ctx.field_index_map.get(counter_name).copied().unwrap_or(0);
        let c_gep = self.fun.next_reg_with_prefix("mrp");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            c_gep, counter_idx).ok();
        let c_val = self.fun.next_reg_with_prefix("mrv");
        writeln!(out, "  {} = load i64, ptr {}, align 8", c_val, c_gep).ok();
        // 2026-07-17: Bound check — exit when counter >= total.
        // The total field name may not be directly after counter; find by name.
        let total_idx = self.ctx.field_index_map.get("total").copied().unwrap_or(counter_idx + 1);
        let b_gep = self.fun.next_reg_with_prefix("mrb");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            b_gep, total_idx).ok();
        let bound_val = self.fun.next_reg_with_prefix("mrbv");
        writeln!(out, "  {} = load i64, ptr {}, align 8", bound_val, b_gep).ok();
        let done = self.fun.next_reg_with_prefix("mrd");
        writeln!(out, "  {} = icmp sge i64 {}, {}", done, c_val, bound_val).ok();
        writeln!(out, "  br i1 {}, label %.mr_end, label %.mr_cont", done).ok();
        writeln!(out, ".mr_cont:").ok();
        let mod_val = self.fun.next_reg_with_prefix("mrm");
        writeln!(out, "  {} = srem i64 {}, {}", mod_val, c_val, divisor).ok();
        for (i, (val, name)) in cases.iter().enumerate() {
            let eq = self.fun.next_reg_with_prefix("mre");
            let next_label = format!(".mr_next_{}", i);
            writeln!(out, "  {} = icmp eq i64 {}, {}", eq, mod_val, val).ok();
            writeln!(out, "  br i1 {}, label %{}, label %{}", eq, name, next_label).ok();
            writeln!(out, "{}:", name).ok();
            writeln!(out, "  call void @txn_{}(ptr %state)", name).ok();
            writeln!(out, "  br label %.mr_latch").ok();
            writeln!(out, "{}:", next_label).ok();
        }
        writeln!(out, "  br label %.mr_latch").ok();
        writeln!(out, ".mr_latch:").ok();
        let next = self.fun.next_reg_with_prefix("mrn");
        writeln!(out, "  {} = add i64 {}, 1", next, c_val).ok();
        writeln!(out, "  store i64 {}, ptr {}, align 8", next, c_gep).ok();
        writeln!(out, "  br label %.mr_loop").ok();
        writeln!(out, ".mr_end:").ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ═══════════════════════════════════════════════════════════════
    // SSA Canonical Loop Setup (Single Txn)
    // ═══════════════════════════════════════════════════════════════

    /// Set up a canonical SSA loop for a single transaction.
    /// Emits phi nodes for counter + field state, header check, and latch.
    fn emit_ssa_canonical_loop_setup(
        &mut self,
        out: &mut String,
        _txn: &crate::ast::Transaction,
        bound_name: &str,
        b_idx: usize,
        cname: &str,
    ) {
        let c_gep = self.fun.next_reg_with_prefix("ssc");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            c_gep, self.ctx.field_index_map.get(cname).copied().unwrap_or(0)).ok();
        let init = self.fun.next_reg_with_prefix("ssi");
        writeln!(out, "  {} = load i64, ptr {}, align 8", init, c_gep).ok();
        let b_gep = self.fun.next_reg_with_prefix("ssb");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            b_gep, b_idx).ok();
        let bound = self.fun.next_reg_with_prefix("ssv");
        writeln!(out, "  {} = load i64, ptr {}, align 8", bound, b_gep).ok();
        writeln!(out, "  br label %.ss_loop").ok();
        writeln!(out, ".ss_loop:").ok();
        let counter = self.fun.next_reg_with_prefix("ssc");
        writeln!(out, "  {} = phi i64 [ {}, %.ss_loop ], [ {}, %.ss_latch ]",
            counter, init, bound).ok();
        let done = self.fun.next_reg_with_prefix("ssd");
        writeln!(out, "  {} = icmp sgt i64 {}, {}", done, counter, bound).ok();
        writeln!(out, "  br i1 {}, label %.ss_body, label %.ss_end", done).ok();
        writeln!(out, ".ss_body:").ok();
    }

    /// Emit the body of a canonical SSA transaction.
    fn emit_ssa_txn_canonical_body(
        &mut self,
        out: &mut String,
        body_stmts: &[&Statement],
        _post_hoist: &[Vec<Statement>],
    ) {
        for stmt in body_stmts {
            self.emit_statement(out, stmt, "  ");
        }
    }

    /// Emit a transaction body with precondition check.
    fn emit_ssa_txn_with_precond(
        &mut self,
        out: &mut String,
        pre: &Expr,
        _name: &str,
        body_stmts: &[&Statement],
        _post_hoist: &[Vec<Statement>],
    ) {
        let cond_val = self.emit_expr(out, pre, "  ");
        let bool_reg = self.as_bool_reg(out, "  ", &cond_val);
        let body_label = format!(".spb{}", self.fun.txn_counter);
        let next_label = format!(".spn{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "  br i1 {}, label %{}, label %{}", bool_reg, body_label, next_label).ok();
        writeln!(out, "{}:", body_label).ok();
        for stmt in body_stmts {
            self.emit_statement(out, stmt, "  ");
        }
        writeln!(out, "  br label %{}", next_label).ok();
        writeln!(out, "{}:", next_label).ok();
    }

    /// Emit a transaction body without precondition check.
    fn emit_ssa_txn_no_precond(
        &mut self,
        out: &mut String,
        body_stmts: &[&Statement],
        _post_hoist: &[Vec<Statement>],
    ) {
        for stmt in body_stmts {
            self.emit_statement(out, stmt, "  ");
        }
    }

    /// Pre-allocate SSA registers for multi-txn state fields.
    fn emit_ssa_mt_prealloc(
        &mut self,
        out: &mut String,
        _txns: &[(String, &crate::ast::Transaction)],
    ) {
        // Emit alloca for each state field at function entry
        for (name, idx) in &self.ctx.field_index_map {
            let alloca = self.fun.next_reg_with_prefix("sa");
            let ft = &self.ctx.field_types[*idx];
            let llvm_type = match ft.as_str() {
                "float" => "float",
                "i32" => "i32",
                "i8" => "i8",
                s if s == "i8*" || s == "ptr" => "ptr",
                _ => "i64",
            };
            writeln!(out, "  {} = alloca {}, align 8", alloca, llvm_type).ok();
            // 2026-07-17: Do NOT insert the alloca pointer into last_val_temps.
            // Alloca registers are LLVM ptr type. When emit_expr looks up a
            // field in last_val_temps, it expects a value register (i64) but
            // gets a ptr — type mismatch at the icmp/cmp instruction.
            // load_last_val_temps (called later in emit_ssa_main) properly
            // loads state values into last_val_temps.
        }
    }

    /// Emit the main SSA register pipeline for multi-txn reactive programs.
    pub(crate) fn emit_ssa_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        has_wake_triggers: bool,
    ) {
        // 2026-07-17: Early-return for modulo-gated dispatch (sparse_dispatch).
        // Checks if all reactive txns have counter % K == N preconditions and
        // emits an optimized modulo-rotated loop that branches directly to the
        // correct body without evaluating K preconditions per iteration.
        let reactive_txns: Vec<&(String, &crate::ast::Transaction)> = txns.iter()
            .filter(|(_, t)| t.is_reactive).collect();
        if self.try_modulo_switch_dispatch(out, &reactive_txns) {
            return;
        }

        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_ssa_mt_prealloc(out, txns);
        // 2026-07-18: Convergence check — if no wake triggers and no async,
        // the program is one-shot. Exit when all txns have converged
        // (precondition false for all).
        let is_one_shot = !has_wake_triggers && !self.has_async_txns;
        let has_exit_cond = self.ctx.exit_condition.is_some();
        let is_terminating = has_exit_cond || is_one_shot;

        let active_slot = if is_terminating {
            let slot = format!("%any_active_{}", self.fun.txn_counter);
            self.fun.txn_counter += 1;
            Some(slot)
        } else {
            None
        };

        // 2026-07-18: Allocate convergence tracking slot in entry (not in loop)
        if let Some(ref slot) = active_slot {
            writeln!(out, "  {} = alloca i64, align 8", slot).ok();
        }
        writeln!(out, "  br label %.ss_main_loop").ok();
        writeln!(out, ".ss_main_loop:").ok();
        // Reset active flag each iteration
        if let Some(ref slot) = active_slot {
            writeln!(out, "  store i64 0, ptr {}", slot).ok();
        }
        // Check each txn's precondition, execute body if true
        for (name, txn) in txns {
            if txn.is_reactive {
                let pre = &txn.contract.pre_condition;
                let cond_val = self.emit_expr(out, pre, "  ");
                let bool_reg = self.as_bool_reg(out, "  ", &cond_val);
                let body_label = format!(".ssb_{}", name);
                let next_label = format!(".ssn_{}", name);
                self.fun.txn_counter += 1;
                writeln!(out, "  br i1 {}, label %{}, label %{}", bool_reg, body_label, next_label).ok();
                writeln!(out, "{}:", body_label).ok();
                if let Some(ref slot) = active_slot {
                    writeln!(out, "  store i64 1, ptr {}", slot).ok();
                }
                for stmt in &txn.body {
                    self.emit_statement(out, stmt, "  ");
                }
                writeln!(out, "  br label %{}", next_label).ok();
                writeln!(out, "{}:", next_label).ok();
            }
        }
        if let Some(ref slot) = active_slot {
            // Check if any txn ran this iteration
            let check_reg = self.fun.gen_reg();
            writeln!(out, "  {} = load i64, ptr {}", check_reg, slot).ok();
            let done_reg = self.fun.gen_reg();
            writeln!(out, "  {} = icmp eq i64 {}, 0", done_reg, check_reg).ok();
            writeln!(out, "  br i1 {}, label %.end, label %.ss_main_loop", done_reg).ok();
        } else {
            writeln!(out, "  br label %.ss_main_loop").ok();
        }
        writeln!(out, ".end:").ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ═══════════════════════════════════════════════════════════════
    // Folded Multi-Txn Dispatch
    // ═══════════════════════════════════════════════════════════════

    /// Emit a folded multi-txn main — dispatch between multiple folded
    /// transactions via an enum key. Used when multiple transactions
    /// share a single main() and are dispatched on a counter/state field.
    pub(crate) fn emit_folded_multi_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        _enum_sizes: &[(String, Option<u64>)],
        _enum_keys: &HashMap<String, Vec<i64>>,
        _fold_params: &HashMap<String, crate::backend::llvm::FoldParam>,
        _fold_pure: &HashMap<String, (bool, Option<i64>)>,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        _composed_fn: Option<&str>,
        _composed_trig_map: Option<&HashMap<String, Vec<(i64, String)>>>,
        _all_internal_map: Option<&HashMap<String, (usize, i64)>>,
        _has_wake: bool,
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let bound_reg = self.fun.next_reg_with_prefix("fmb");
        if let Some(ti) = total_idx {
            let gep = self.fun.next_reg_with_prefix("fmgb");
            writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                gep, ti).ok();
            writeln!(out, "  {} = load i64, ptr {}, align 8", bound_reg, gep).ok();
        } else if let Some(tcn) = total_const_name {
            if let Some(&idx) = self.ctx.field_index_map.get(tcn) {
                let gep = self.fun.next_reg_with_prefix("fmgb");
                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                    gep, idx).ok();
                writeln!(out, "  {} = load i64, ptr {}, align 8", bound_reg, gep).ok();
            } else {
                writeln!(out, "  {} = add i64 0, 1", bound_reg).ok();
            }
        } else {
            writeln!(out, "  {} = add i64 0, 1", bound_reg).ok();
        }
        // 2026-07-18: Preallocate push targets for all txn bodies in the
        // multi-txn main loop. Each body's push targets are collected via
        // over-approximation and deduplicated, then preallocated once with
        // capacity = bound + 2 per target. Since all txns share the same
        // arena and bound, one preallocation block covers all bodies.
        {
            let mut push_targets: Vec<String> = Vec::new();
            for (_name, txn) in txns {
                crate::backend::llvm::collect_push_targets(&txn.body, &mut push_targets);
            }
            push_targets.sort();
            push_targets.dedup();
            if !push_targets.is_empty() {
                self.emit_prealloc_for_targets(out, "  ", &push_targets, &bound_reg);
            }
        }
        writeln!(out, "  br label %.fm_loop").ok();
        writeln!(out, ".fm_loop:").ok();
        let c_gep = self.fun.next_reg_with_prefix("fmg");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            c_gep, counter_idx).ok();
        let c_val = self.fun.next_reg_with_prefix("fmv");
        writeln!(out, "  {} = load i64, ptr {}, align 8", c_val, c_gep).ok();
        let done = self.fun.next_reg_with_prefix("fmd");
        writeln!(out, "  {} = icmp slt i64 {}, {}", done, c_val, bound_reg).ok();
        writeln!(out, "  br i1 {}, label %.fm_body, label %.fm_end", done).ok();
        writeln!(out, ".fm_body:").ok();
        for (name, txn) in txns {
            writeln!(out, "  call void @txn_{}(ptr %state)", name).ok();
        }
        let next = self.fun.next_reg_with_prefix("fmn");
        writeln!(out, "  {} = add i64 {}, 1", next, c_val).ok();
        writeln!(out, "  store i64 {}, ptr {}, align 8", next, c_gep).ok();
        writeln!(out, "  br label %.fm_loop").ok();
        writeln!(out, ".fm_end:").ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ═══════════════════════════════════════════════════════════════
    // Post-Loop Print Handling
    // ═══════════════════════════════════════════════════════════════

    /// Load last-value temps into registers for post-loop printing.
    pub(crate) fn load_last_val_temps(&mut self, out: &mut String) {
        let mut sorted_keys: Vec<String> = self.fun.last_val_temps.keys().cloned().collect();
        sorted_keys.sort();
        for name in &sorted_keys {
            if let Some(&idx) = self.ctx.field_index_map.get(name) {
                let gep = self.fun.next_reg_with_prefix("lvt");
                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                    gep, idx).ok();
                let val = self.fun.next_reg_with_prefix("lvv");
                writeln!(out, "  {} = load i64, ptr {}, align 8", val, gep).ok();
                self.fun.last_val_temps.insert(name.clone(), val.clone());
                let brief_ty = self.ctx.field_brief_types.get(idx).cloned().unwrap_or(Type::int());
                self.fun.last_val_types.insert(name.clone(), brief_ty);
            }
        }
    }

    /// Emit a single post-loop print statement.
    fn emit_post_print(
        &mut self,
        out: &mut String,
        name: &str,
        reg: &str,
        _ty: &str,
        indent: &str,
    ) {
        writeln!(out, "{}call void @PrintInt#(i64 {})  ; {}", indent, reg, name).ok();
    }

    /// Emit hoisted post-loop print calls.
    pub(crate) fn emit_hoisted_post_loop_prints(
        &mut self,
        out: &mut String,
        hoisted: &[Vec<Statement>],
    ) {
        for block in hoisted {
            for stmt in block {
                match stmt {
                    // 2026-07-17: hoist_terminating_guard wraps the swan song
                    // as Statement::Expression(swan_song_expr). Also handle
                    // TermBang/Term in case other paths produce those variants.
                    Statement::TermBang(Some(e)) | Statement::Term(Some(e)) | Statement::Expression(e) => {
                        self.emit_expr(out, e, "  ");
                    }
                    // 2026-07-19: Emit let bindings inside the hoisted guard
                    // body so downstream identifier lookups resolve correctly.
                    // Without this, let bindings (e.g. nbody's `let energy:
                    // Float32 = ...`) are silently skipped, and the identifier
                    // fallback creates an undefined global reference (@energy).
                    Statement::Let { .. } => {
                        self.emit_statement(out, stmt, "  ");
                    }
                    _ => {}
                }
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Trigger Step Function
    // ═══════════════════════════════════════════════════════════════

    /// Emit the trigger step function: check dirty flags and recompute
    /// transactions whose trigger conditions are met.
    pub(crate) fn emit_trg_step(
        &mut self,
        out: &mut String,
        _dep_graph: &crate::analysis::dependency_graph::DependencyGraph,
        trigger_names: &[String],
    ) {
        writeln!(out, "define void @trg_step(ptr %state) local_unnamed_addr {{").ok();
        for trg_name in trigger_names {
            let trg_name_clone = trg_name.clone();
            if let Some(trg) = self.ctx.triggers.get(&trg_name_clone) {
                let trg_name = trg.name.clone();
                let cond_val = self.emit_expr(out, &Expr::Identifier(trg_name.clone()), "  ");
                let bool_reg = self.as_bool_reg(out, "  ", &cond_val);
                let body_label = format!(".trg_{}", trg_name);
                let next_label = format!(".trg_next_{}", trg_name);
                writeln!(out, "  br i1 {}, label %{}, label %{}", bool_reg, body_label, next_label).ok();
                writeln!(out, "  {}:", body_label).ok();
                writeln!(out, "  call void @txn_{}(ptr %state)", trg_name).ok();
                writeln!(out, "  br label %{}", next_label).ok();
                writeln!(out, "  {}:", next_label).ok();
            }
        }
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a trigger event via epoll_wait.
    pub(crate) fn emit_trg_event_epoll_wait(
        _backend: &mut LlvmBackend,
        _out: &mut String,
    ) {
        // Placeholder: epoll-based trigger wait is platform-specific
    }

    /// Emit a cycle count increment.
    pub(crate) fn emit_cycle_count_increment(
        _backend: &mut LlvmBackend,
        _out: &mut String,
    ) {
        // Placeholder: cycle counting is architecture-specific
    }

}
