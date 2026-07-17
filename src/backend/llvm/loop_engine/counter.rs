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
//   3. HYBRID COUNTER-PHI + MEMORY FIELDS (emit_countable_main, EmitHybridCounterPhi):
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
        // 2026-07-17: Pre-generate backedge register name (forward reference
        // from phi header to latch definition — valid in LLVM IR).
        let next = self.fun.next_reg_with_prefix("fln");
        let exit_label = format!("{}.end", label_prefix);
        writeln!(out, "  br label %{}.header", label_prefix).ok();
        writeln!(out, "{}.header:", label_prefix).ok();
        let counter_name = self.fun.next_reg_with_prefix("flc");
        let done_reg = self.fun.next_reg_with_prefix("fld");
        if is_decreasing {
            writeln!(out, "  {} = phi i64 [ {}, %entry ], [ {}, %{}.latch ]",
                counter_name, init_name, next, label_prefix).ok();
            // 2026-07-17: Fixed comparison direction. For decreasing counters we
            // want `counter > 0` (continue while still above the bound), not
            // `counter < bound` (which would exit immediately for decreasing).
            writeln!(out, "  {} = icmp sgt i64 {}, {}", done_reg, counter_name, bound_reg).ok();
        } else {
            writeln!(out, "  {} = phi i64 [ {}, %entry ], [ {}, %{}.latch ]",
                counter_name, init_name, next, label_prefix).ok();
            // 2026-07-17: Fixed comparison direction. For increasing counters we
            // want `counter < bound` (continue while below the bound), not
            // `counter > bound` (which would exit immediately).
            writeln!(out, "  {} = icmp slt i64 {}, {}", done_reg, counter_name, bound_reg).ok();
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

    /// Emit a foldable loop using memory-based counter (EmitMemoryCounter path).
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
    // Strategy 3: Hybrid Countable Loop (EmitHybridCounterPhi)
    // ═══════════════════════════════════════════════════════════════

    /// Emit a countable main() with per-field phi nodes (EmitPerFieldPhi/EmitHybridCounterPhi).
    ///
    /// 2026-07-17: Each state field in write_set gets its own phi node in
    /// the loop header. The body reads from phi registers and writes to
    /// pending_phi_backedge. The latch computes the backedge value for each
    /// field (identity for unwritten, written value for modified).
    ///
    /// Path A (no post-loop hoists): Zero stores in the hot loop body — phi
    /// registers carry all values. Enables LLVM SROA to decompose the loop
    /// into closed-SSA form.
    ///
    /// Path B (post-loop hoists exist): GEP+store emitted for fields the
    /// done: block reads. Ensures hoisted post-loop prints see final values.
    ///
    /// 2026-07-13: Extracted into a single function with max 2-level
    /// nesting. Loop setup, body, and latch are delegated to helpers.
    pub(crate) fn emit_countable_main(
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

        // 2026-07-17: Pre-load all field initial values from state for per-field phis.
        // Sort deterministically to avoid HashMap iteration non-determinism.
        let mut sorted_fields: Vec<&String> = write_set.iter().collect();
        sorted_fields.sort();
        let mut phi_field_init: HashMap<String, String> = HashMap::new();
        for fname in &sorted_fields {
            if let Some(&idx) = self.ctx.field_index_map.get(fname.as_str()) {
                let igep = self.fun.next_reg_with_prefix("cmi");
                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                    igep, idx).ok();
                let init_f = self.fun.next_reg_with_prefix("cmf");
                writeln!(out, "  {} = load i64, ptr {}, align 8", init_f, igep).ok();
                phi_field_init.insert((*fname).clone(), init_f);
            }
        }

        // 2026-07-17: Pre-generate backedge register names for per-field phis
        // (forward reference from header phi to latch definition).
        let next = self.fun.next_reg_with_prefix("cmn");
        let mut be_field_regs: HashMap<String, String> = HashMap::new();
        for fname in &sorted_fields {
            let be_f = self.fun.next_reg_with_prefix("pbf");
            be_field_regs.insert((*fname).clone(), be_f);
        }

        let exit_label = format!(".cm_end_{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "  br label %.cm_header").ok();
        writeln!(out, ".cm_header:").ok();
        let counter_name = self.fun.next_reg_with_prefix("cmc");
        let done_reg = self.fun.next_reg_with_prefix("cmd");

        // Counter phi
        if is_decreasing {
            writeln!(out, "  {} = phi i64 [ {}, %entry ], [ {}, %.cm_latch ]",
                counter_name, init_name, next).ok();
        } else {
            writeln!(out, "  {} = phi i64 [ {}, %entry ], [ {}, %.cm_latch ]",
                counter_name, init_name, next).ok();
        }

        // 2026-07-17: Per-field phi nodes — one per written field.
        self.fun.phi_field_regs.clear();
        self.fun.backedge_field_regs.clear();
        for fname in &sorted_fields {
            let phi_f = self.fun.next_reg_with_prefix("ppf");
            let be_f = be_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| format!("%be_{}", fname));
            let init_f = phi_field_init.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            writeln!(out, "  {} = phi i64 [ {}, %entry ], [ {}, %.cm_latch ]",
                phi_f, init_f, be_f).ok();
            self.fun.phi_field_regs.insert((*fname).clone(), phi_f);
            self.fun.backedge_field_regs.insert((*fname).clone(), be_f);
        }

        // 2026-07-17: Exit check. For increasing counters we want
        // `counter < bound` (continue while below the bound); for decreasing
        // counters we want `counter > 0` (continue while above the bound).
        // The br branches to .cm_body if done_reg is true.
        if is_decreasing {
            writeln!(out, "  {} = icmp sgt i64 {}, {}", done_reg, counter_name, bound_reg).ok();
        } else {
            writeln!(out, "  {} = icmp slt i64 {}, {}", done_reg, counter_name, bound_reg).ok();
        }
        writeln!(out, "  br i1 {}, label %.cm_body, label %{}", done_reg, exit_label).ok();
        writeln!(out, ".cm_body:").ok();

        // 2026-07-17: Initialize pending_phi_backedge with identity values.
        // Body writes will overwrite entries for modified fields.
        self.fun.pending_phi_backedge.clear();
        for fname in &sorted_fields {
            if let Some(phi_f) = self.fun.phi_field_regs.get(fname.as_str()) {
                self.fun.pending_phi_backedge.insert((*fname).clone(), phi_f.clone());
            }
        }

        // 2026-07-17: Determine store gating — Path A (stores suppressed) vs
        // Path B (stores emitted for post-loop hoisted prints).
        self.fun.needs_state_stores_in_body = !self.fun.pending_post_hoist.is_empty();

        // 2026-07-17: pending_post_hoist (set by hoist_terminating_guard) is
        // emitted AFTER the loop closes, not inside the body. The hoisted
        // swan song reads final accumulator values from %State (stored by
        // Path B — needs_state_stores_in_body). Clone to satisfy borrow
        // checker (self.emit_expr needs &mut self; pending_post_hoist is
        // behind &self.fun).
        let hoist = self.fun.pending_post_hoist.clone();
        let mut empty = Vec::new();
        self.emit_countable_body(out, body, write_set, &mut empty);
        writeln!(out, "  br label %.cm_latch").ok();
        writeln!(out, ".cm_latch:").ok();
        if is_decreasing {
            writeln!(out, "  {} = add i64 {}, -1", next, counter_name).ok();
        } else {
            writeln!(out, "  {} = add i64 {}, 1", next, counter_name).ok();
        }

        // 2026-07-17: Per-field backedges. Modified fields use the written value;
        // unwritten fields use identity (phi self-ref). LLVM peephole eliminates
        // the `add i64 0, %val` copy in both cases.
        for fname in &sorted_fields {
            if let Some(be_f) = self.fun.backedge_field_regs.get(fname.as_str()) {
                let val = self.fun.pending_phi_backedge.get(fname.as_str())
                    .cloned().unwrap_or_else(|| {
                        self.fun.phi_field_regs.get(fname.as_str())
                            .cloned().unwrap_or_else(|| "0".to_string())
                    });
                writeln!(out, "  {} = add i64 0, {}", be_f, val).ok();
            }
        }

        writeln!(out, "  br label %.cm_header").ok();
        writeln!(out, "{}:", exit_label).ok();
        // 2026-07-17: Emit hoisted post-loop prints (swan song) AFTER the loop
        // closes, so they read the final accumulator values from %State. The
        // guard condition was hoist_terminating_guard-removed; at this point
        // the loop postcondition guarantees it holds.
        let hoist = self.fun.pending_post_hoist.clone();
        self.emit_hoisted_post_loop_prints(out, &hoist);
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
            // 2026-07-17: Resolve bound from compile-time constant value first.
            // The tcn is the bound variable name. It may be a const (like
            // `const total: Int = 500`) rather than a state field — in which
            // case we read the literal from `self.ctx.constants`.
            if let Some((_, Expr::Decimal(val))) = self.ctx.constants.get(tcn) {
                writeln!(out, "  {} = add i64 0, {}", bound_reg, val).ok();
            } else if let Some(&idx) = self.ctx.field_index_map.get(tcn) {
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
                    self.fun.last_val_temps.insert(name.clone(), reg.name.clone());
                    self.fun.last_val_types.insert(name.clone(), reg.ty);
                }
                Statement::Assign(lhs, expr) => {
                    let lhs_name = Self::assign_target_name(lhs);
                    let val = self.emit_expr(out, expr, "  ");
                    if let Some(ref n) = lhs_name {
                        if write_set.contains(n) {
                            // 2026-07-17: Box the value to i64 for the phi backedge.
                            // Phi registers track all fields as i64 (the %State
                            // representation). Float/double values must be boxed.
                            let boxed = self.adapt_to_i64(out, "  ", &val);
                            self.fun.pending_phi_backedge.insert(n.clone(), boxed);
                        }
                        // 2026-07-17: When post-loop hoisted prints need final values,
                        // emit state stores for ALL fields, not just phi-tracked ones.
                        // Without this, fields outside the capped write_set (max 6)
                        // silently lose their values between iterations — the body
                        // computes the new value, but it's never stored back to %State.
                        if self.fun.needs_state_stores_in_body {
                            if let Some(&idx) = self.ctx.field_index_map.get(n) {
                                let boxed = self.adapt_to_i64(out, "  ", &val);
                                let gep = self.fun.next_reg_with_prefix("cms");
                                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                                    gep, idx).ok();
                                writeln!(out, "  store i64 {}, ptr {}, align 8", boxed, gep).ok();
                            }
                        }
                        self.fun.last_val_temps.insert(n.clone(), val.name.clone());
                        self.fun.last_val_types.insert(n.clone(), val.ty.clone());
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

}
