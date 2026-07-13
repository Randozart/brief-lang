// ── Loop Emission: Counter-Based Strategies ────────────────────
//
// 2026-07-13: Extracted from monolithic loop_engine.rs (4398 lines).
// Implements strategies 1–3 from the loop emission architecture:
//
//   1. PURE COUNTER FOLD (emit_folded_pure_counter):
//      Pure bodies with compile-time constant bound. O(1) single store.
//
//   2. PURE COUNTER PHI (emit_folded_loop, use_phi=true):
//      Pure bodies with runtime-variable bound. Counter-only phi node,
//      no body emission (body precomputed).
//
//   3. HYBRID COUNTER-PHI + MEMORY FIELDS (emit_countable_main, A005e):
//      Non-pure foldable single-txn programs. Single counter phi + per-field
//      load/store. LLVM SROA converts to closed-SSA phis, avoiding the
//      phi-escape problem that blocks the vectorizer.

use crate::backend::llvm::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write;

impl LlvmBackend {
    // ═══════════════════════════════════════════════════════════════
    // Strategy 1: Pure Counter Fold
    // ═══════════════════════════════════════════════════════════════

    /// Emit a pure counter fold: single `store i64` with the final counter
    /// value. No runtime loop. The body was fully precomputed at compile
    /// time within `--optimize-budget`.
    pub(crate) fn emit_folded_pure_counter(
        &mut self,
        out: &mut String,
        counter_idx: usize,
        total_value: i64,
    ) {
        let gep = self.fun.next_reg_with_prefix("fpc");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            gep, counter_idx).ok();
        writeln!(out, "  store i64 {}, ptr {}, align 8", total_value, gep).ok();
    }

    // ═══════════════════════════════════════════════════════════════
    // Strategy 2: Folded Loop (Counter Phi, Pure)
    // ═══════════════════════════════════════════════════════════════

    /// Emit a folded loop with counter-only phi. When `use_phi=true`, the
    /// body is precomputed — only the counter phi and backedge are emitted.
    /// When `use_phi=false` with a body, the body statements are emitted
    /// inline with SSA registers.
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
        let c0 = self.fun.txn_counter;
        let bound_reg = self.fun.next_reg_with_prefix("flb");
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);
        let gep = self.fun.next_reg_with_prefix("flg");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            gep, counter_idx).ok();
        let init_name = self.fun.next_reg_with_prefix("fli");
        writeln!(out, "  {} = load i64, ptr {}, align 8", init_name, gep).ok();
        let exit_label = format!("{}.end", label_prefix);
        writeln!(out, "  br label %{}.header", label_prefix).ok();
        writeln!(out, "{}.header:", label_prefix).ok();
        let counter_name = self.fun.next_reg_with_prefix("flc");
        let done_reg = self.fun.next_reg_with_prefix("fld");
        if is_decreasing {
            writeln!(out, "  {} = phi i64 [ {}, %{}.header ], [ {}, %{}.latch ]",
                counter_name, init_name, label_prefix, bound_reg, label_prefix).ok();
            writeln!(out, "  {} = icmp slt i64 {}, {}", done_reg, counter_name, bound_reg).ok();
        } else {
            writeln!(out, "  {} = phi i64 [ {}, %{}.header ], [ {}, %{}.latch ]",
                counter_name, init_name, label_prefix, bound_reg, label_prefix).ok();
            writeln!(out, "  {} = icmp sgt i64 {}, {}", done_reg, counter_name, bound_reg).ok();
        }
        writeln!(out, "  br i1 {}, label %{}.body, label %{}", done_reg, label_prefix, exit_label).ok();
        writeln!(out, "{}.body:", label_prefix).ok();

        if use_phi {
            // Pure phi — no body emission, counter only
        } else if let Some(stmts) = body {
            let write_set: HashSet<String> = HashSet::new();
            let mut hoisted = Vec::new();
            self.emit_countable_body(out, stmts, &write_set, &mut hoisted);
        } else {
            writeln!(out, "  call void @txn_{}(ptr %state)", txn_name).ok();
        }

        writeln!(out, "  br label %{}.latch", label_prefix).ok();
        writeln!(out, "{}.latch:", label_prefix).ok();
        let next = self.fun.next_reg_with_prefix("fln");
        if is_decreasing {
            writeln!(out, "  {} = add i64 {}, -1", next, counter_name).ok();
        } else {
            writeln!(out, "  {} = add i64 {}, 1", next, counter_name).ok();
        }
        writeln!(out, "  br label %{}.header", label_prefix).ok();
        writeln!(out, "{}:", exit_label).ok();
        let final_gep = self.fun.next_reg_with_prefix("flg");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            final_gep, counter_idx).ok();
        writeln!(out, "  store i64 {}, ptr {}, align 8", counter_name, final_gep).ok();
    }

    /// Emit a folded loop wrapped in a main() function.
    /// For pure bodies with a known bound: counter-phi loop that stores
    /// the final counter value and returns.
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
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_folded_loop(out, txn_name, counter_idx, total_idx, total_const_name,
            ".fmain", use_phi, body, 1, false, None);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a foldable loop using memory-based counter (A005b path).
    /// Counter is tracked via GEP+load+store rather than SSA phi.
    pub(crate) fn emit_folded_memory_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        body: &[Statement],
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let c0 = self.fun.txn_counter;
        let bound_reg = self.fun.next_reg_with_prefix("fmb");
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);
        writeln!(out, "  br label %.fm_loop").ok();
        writeln!(out, ".fm_loop:").ok();
        let counter_gep = self.fun.next_reg_with_prefix("fmg");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            counter_gep, counter_idx).ok();
        let counter_val = self.fun.next_reg_with_prefix("fmv");
        writeln!(out, "  {} = load i64, ptr {}, align 8", counter_val, counter_gep).ok();
        let done = self.fun.next_reg_with_prefix("fmd");
        writeln!(out, "  {} = icmp slt i64 {}, {}", done, counter_val, bound_reg).ok();
        writeln!(out, "  br i1 {}, label %.fm_body, label %.fm_end", done).ok();
        writeln!(out, ".fm_body:").ok();
        let write_set: HashSet<String> = HashSet::new();
        let mut hoisted = Vec::new();
        self.emit_countable_body(out, body, &write_set, &mut hoisted);
        let next = self.fun.next_reg_with_prefix("fmn");
        writeln!(out, "  {} = add i64 {}, 1", next, counter_val).ok();
        writeln!(out, "  store i64 {}, ptr {}, align 8", next, counter_gep).ok();
        writeln!(out, "  br label %.fm_loop").ok();
        writeln!(out, ".fm_end:").ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ═══════════════════════════════════════════════════════════════
    // Strategy 3: Hybrid Countable Loop (A005e)
    // ═══════════════════════════════════════════════════════════════

    /// Emit a countable main() — single counter phi + per-field load/store
    /// in the body. LLVM SROA converts GEP+load+store to closed-SSA phis.
    ///
    /// 2026-07-13: Extracted into a single function with max 2-level
    /// nesting. Loop setup, body, and latch are delegated to helpers.
    pub(super) fn emit_countable_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        body: &[Statement],
        write_set: &HashSet<String>,
        is_decreasing: bool,
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let c0 = self.fun.txn_counter;
        let bound_reg = self.fun.next_reg_with_prefix("cmb");
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);
        let gep = self.fun.next_reg_with_prefix("cmi");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            gep, counter_idx).ok();
        let init_name = self.fun.next_reg_with_prefix("cmv");
        writeln!(out, "  {} = load i64, ptr {}, align 8", init_name, gep).ok();
        let exit_label = format!(".cm_end_{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "  br label %.cm_header").ok();
        writeln!(out, ".cm_header:").ok();
        let counter_name = self.fun.next_reg_with_prefix("cmc");
        let done_reg = self.fun.next_reg_with_prefix("cmd");
        if is_decreasing {
            writeln!(out, "  {} = phi i64 [ {}, %.cm_header ], [ {}, %.cm_latch ]",
                counter_name, init_name, bound_reg).ok();
            writeln!(out, "  {} = icmp slt i64 {}, {}", done_reg, counter_name, bound_reg).ok();
        } else {
            writeln!(out, "  {} = phi i64 [ {}, %.cm_header ], [ {}, %.cm_latch ]",
                counter_name, init_name, bound_reg).ok();
            writeln!(out, "  {} = icmp sgt i64 {}, {}", done_reg, counter_name, bound_reg).ok();
        }
        writeln!(out, "  br i1 {}, label %.cm_body, label {}", done_reg, exit_label).ok();
        writeln!(out, ".cm_body:").ok();
        self.pre_load_all_fields(out, "%state", Some(write_set));
        let mut hoisted = Vec::new();
        self.emit_countable_body(out, body, write_set, &mut hoisted);
        self.emit_hoisted_post_loop_prints(out, &hoisted);
        writeln!(out, "  br label %.cm_latch").ok();
        writeln!(out, ".cm_latch:").ok();
        let next = self.fun.next_reg_with_prefix("cmn");
        if is_decreasing {
            writeln!(out, "  {} = add i64 {}, -1", next, counter_name).ok();
        } else {
            writeln!(out, "  {} = add i64 {}, 1", next, counter_name).ok();
        }
        writeln!(out, "  br label %.cm_header").ok();
        writeln!(out, "{}:", exit_label).ok();
        let final_gep = self.fun.next_reg_with_prefix("cmg");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            final_gep, counter_idx).ok();
        writeln!(out, "  store i64 {}, ptr {}, align 8", counter_name, final_gep).ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Simpler variant of countable_main that uses memory-based fields.
    /// No counter phi — counter is tracked via GEP+load+store.
    pub(super) fn emit_countable_memory_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        body: &[Statement],
        write_set: &HashSet<String>,
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let c0 = self.fun.txn_counter;
        let bound_reg = self.fun.next_reg_with_prefix("cmmb");
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);
        writeln!(out, "  br label %.cmm_loop").ok();
        writeln!(out, ".cmm_loop:").ok();
        let counter_gep = self.fun.next_reg_with_prefix("cmmg");
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            counter_gep, counter_idx).ok();
        let counter_val = self.fun.next_reg_with_prefix("cmmv");
        writeln!(out, "  {} = load i64, ptr {}, align 8", counter_val, counter_gep).ok();
        let done = self.fun.next_reg_with_prefix("cmmd");
        writeln!(out, "  {} = icmp slt i64 {}, {}", done, counter_val, bound_reg).ok();
        writeln!(out, "  br i1 {}, label %.cmm_body, label %.cmm_end", done).ok();
        writeln!(out, ".cmm_body:").ok();
        let mut hoisted = Vec::new();
        self.emit_countable_body(out, body, write_set, &mut hoisted);
        self.emit_hoisted_post_loop_prints(out, &hoisted);
        let next = self.fun.next_reg_with_prefix("cmmn");
        writeln!(out, "  {} = add i64 {}, 1", next, counter_val).ok();
        writeln!(out, "  store i64 {}, ptr {}, align 8", next, counter_gep).ok();
        writeln!(out, "  br label %.cmm_loop").ok();
        writeln!(out, ".cmm_end:").ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ═══════════════════════════════════════════════════════════════
    // Countable Loop Helpers
    // ═══════════════════════════════════════════════════════════════

    /// Load the loop bound into a register: from a state field, a const
    /// name, or 0.
    fn emit_countable_load_bound(
        &mut self,
        out: &mut String,
        bound_reg: &str,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        _c0: usize,
    ) {
        if let Some(ti) = total_idx {
            let gep = self.fun.next_reg_with_prefix("clb");
            writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                gep, ti).ok();
            writeln!(out, "  {} = load i64, ptr {}, align 8", bound_reg, gep).ok();
        } else if let Some(tcn) = total_const_name {
            if let Some(&idx) = self.ctx.field_index_map.get(tcn) {
                let gep = self.fun.next_reg_with_prefix("clb");
                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                    gep, idx).ok();
                writeln!(out, "  {} = load i64, ptr {}, align 8", bound_reg, gep).ok();
            } else {
                writeln!(out, "  {} = add i64 0, 1", bound_reg).ok();
            }
        } else {
            writeln!(out, "  {} = add i64 0, 1", bound_reg).ok();
        }
    }

    /// Emit the body of a countable loop. Converts each Statement to the
    /// appropriate SSA load + op + store sequence.
    fn emit_countable_body(
        &mut self,
        out: &mut String,
        body: &[Statement],
        write_set: &HashSet<String>,
        hoisted: &mut Vec<Vec<Statement>>,
    ) {
        for stmt in body {
            match stmt {
                Statement::Let { name, expr: Some(e), .. } => {
                    let reg = self.emit_expr(out, e, "  ");
                    self.fun.last_val_temps.insert(name.clone(), reg.name);
                }
                Statement::Assign(lhs, expr) => {
                    let lhs_name = Self::assign_target_name(lhs);
                    let val = self.emit_expr(out, expr, "  ");
                    if let Some(ref n) = lhs_name {
                        if write_set.contains(n) {
                            if let Some(&idx) = self.ctx.field_index_map.get(n) {
                                let gep = self.fun.next_reg_with_prefix("cms");
                                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                                    gep, idx).ok();
                                writeln!(out, "  store i64 {}, ptr {}, align 8", val.name, gep).ok();
                            }
                        }
                        self.fun.last_val_temps.insert(n.clone(), val.name);
                    }
                }
                Statement::Term(Some(e)) | Statement::TermBang(Some(e)) => {
                    let val = self.emit_expr(out, e, "  ");
                    let name = format!("%t{}", self.fun.txn_counter);
                    self.fun.txn_counter += 1;
                    writeln!(out, "  {} = add i64 0, {}", name, val.name).ok();
                }
                Statement::If(cond, then_b, else_b) => {
                    let cond_reg = self.emit_expr(out, cond, "  ");
                    let bool_reg = self.as_bool_reg(out, "  ", &cond_reg);
                    let then_label = format!(".cmit{}", self.fun.txn_counter);
                    let else_label = format!(".cmie{}", self.fun.txn_counter);
                    let merge_label = format!(".cmim{}", self.fun.txn_counter);
                    self.fun.txn_counter += 1;
                    writeln!(out, "  br i1 {}, label %{}, label %{}", bool_reg, then_label, else_label).ok();
                    writeln!(out, "{}:", then_label).ok();
                    self.emit_countable_body(out, then_b, write_set, hoisted);
                    writeln!(out, "  br label %{}", merge_label).ok();
                    writeln!(out, "{}:", else_label).ok();
                    self.emit_countable_body(out, else_b, write_set, hoisted);
                    writeln!(out, "  br label %{}", merge_label).ok();
                    writeln!(out, "{}:", merge_label).ok();
                }
                Statement::Guarded(cond, stmts) => {
                    let cond_reg = self.emit_expr(out, cond, "  ");
                    let bool_reg = self.as_bool_reg(out, "  ", &cond_reg);
                    let body_label = format!(".cmgb{}", self.fun.txn_counter);
                    let next_label = format!(".cmgn{}", self.fun.txn_counter);
                    self.fun.txn_counter += 1;
                    writeln!(out, "  br i1 {}, label %{}, label %{}", bool_reg, body_label, next_label).ok();
                    writeln!(out, "{}:", body_label).ok();
                    self.emit_countable_body(out, stmts, write_set, hoisted);
                    writeln!(out, "  br label %{}", next_label).ok();
                    writeln!(out, "{}:", next_label).ok();
                }
                Statement::Block(stmts) => {
                    self.emit_countable_body(out, stmts, write_set, hoisted);
                }
                Statement::Expression(e) => {
                    self.emit_expr(out, e, "  ");
                }
                Statement::Return(Some(e)) => {
                    let val = self.emit_expr(out, e, "  ");
                    writeln!(out, "  ret i64 {}", val.name).ok();
                }
                _ => {}
            }
        }
    }

    /// Extract the field name from an assignment left-hand side.
    fn assign_target_name(lhs: &Expr) -> Option<String> {
        match lhs {
            Expr::Identifier(n) => Some(n.clone()),
            _ => None,
        }
    }

    /// Emit hoisted post-loop print statements.
    /// These are deferred prints (e.g. `term! -> PrintInt#(result)`)
    /// that execute after the loop exits.
    fn emit_hoisted_post_loop_prints(
        &mut self,
        out: &mut String,
        hoisted: &[Vec<Statement>],
    ) {
        for block in hoisted {
            for stmt in block {
                if let Statement::TermBang(Some(e)) = stmt {
                    self.emit_expr(out, e, "  ");
                }
                if let Statement::Term(Some(e)) = stmt {
                    self.emit_expr(out, e, "  ");
                }
            }
        }
    }
}
