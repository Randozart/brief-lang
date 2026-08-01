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

/// Configuration for batch-loop mode.
/// When set, the loop is split into an outer structural loop and an inner
/// pure-compute loop. The inner loop runs for `batch_size` iterations without
/// any branch guards, enabling LLVM's if-conversion. The outer loop handles
/// the guard checks (prints, termination) between batches.
///
/// See docs/plans/2026-07-29-loop-peeling-automatic.md
//
// 2026-07-31: BatchInfo removed — the composite-node decomposition
// (emit_version_dag_main) supersedes the heuristic batch-loop. See
// docs/plans/2026-07-30-flat-node-decomposition.md §11.

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
        let store_val = format!("{}", total_value);
        self.emit_state_store_i64_by_idx(out, "  ", counter_idx, &store_val);
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
        let (init_name, _) = self.emit_state_load_i64_by_idx(out, "  ", counter_idx);
        // 2026-07-18: Preallocate push targets before the loop body.
        // Collects all Assign(Ident, _) targets from the body and allocates
        // (bound + 2) * 8 bytes per target from the arena (or @malloc if
        // no arena active). This converts per-iteration push overhead from
        // O(N) malloc+memcpy to O(1) direct store.
        if let Some(stmts) = body {
            let mut push_targets: Vec<String> = Vec::new();
            crate::backend::llvm::collect_push_targets(stmts, &mut push_targets);
            push_targets.sort();
            push_targets.dedup();
            if !push_targets.is_empty() {
                self.emit_prealloc_for_targets(out, "  ", &push_targets, &bound_reg);
            }
        }
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
            writeln!(out, "  {} = sub nuw nsw i64 {}, 1", next, counter_name).ok();
        } else {
            writeln!(out, "  {} = add nuw nsw i64 {}, 1", next, counter_name).ok();
        }
        emit_loop_metadata(out, "  ", &format!("{}.header", label_prefix),
            &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
        writeln!(out, "{}:", exit_label).ok();
        self.emit_state_store_i64_by_idx(out, "  ", counter_idx, &counter_name);
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
        self.emit_main_header(out, "#0", true);
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_folded_loop(out, txn_name, counter_idx, total_idx, total_const_name,
            ".fmain", use_phi, body, 1, false, None);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // 2026-07-29: emit_folded_memory_main and emit_while_main removed — dead code
    // after Phase 4 dispatch simplification. PerFieldPhi (emit_countable_main)
    // handles all cases with better SROA characteristics.

    // ═══════════════════════════════════════════════════════════════
    // Strategy 3: Hybrid Countable Loop (EmitHybridCounterPhi)
    // ═══════════════════════════════════════════════════════════════

/// Configuration for batch-loop mode.
/// When set, the loop is split into an outer structural loop and an inner
/// pure-compute loop. The inner loop runs for `batch_size` iterations without
/// any branch guards, enabling LLVM's if-conversion. The outer loop handles
/// the guard checks (prints, termination) between batches.
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
    /// 2026-07-29: Batch mode (batch_info is Some) emits a nested loop
    /// structure with two levels of phi nodes. The outer phis track values
    /// across batches; the inner phis track values within a single batch.
    /// The inner loop has no branches or function calls, enabling LLVM's
    /// if-conversion.
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
        counter_var: Option<&str>,
        watchdog: Option<&crate::ast::top::WatchdogSpec>,
    ) {
        self.emit_main_header(out, "#0", true);
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let c0 = self.fun.txn_counter;
        let bound_reg = self.fun.next_reg_with_prefix("cmb");
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);
        let (init_name, _) = self.emit_state_load_i64_by_idx(out, "  ", counter_idx);

        // 2026-07-17: Pre-load all field initial values from state for per-field phis.
        // Sort deterministically to avoid HashMap iteration non-determinism.
        let mut sorted_fields: Vec<&String> = write_set.iter().collect();
        sorted_fields.sort();
        let mut phi_field_init: HashMap<String, String> = HashMap::new();
        for fname in &sorted_fields {
            let idx = match self.ctx.field_index_map.get(fname.as_str()) {
                Some(&i) => i,
                None => continue,
            };
            let (init_f, _) = self.emit_state_load_i64_by_idx(out, "  ", idx);
            phi_field_init.insert((*fname).clone(), init_f);
        }

        // 2026-07-29: Clear vector phi state — disabled inside emit_countable_main.
        // The dispatch-level detection in mod.rs still checks for vector phi groups,
        // but the actual emission is deferred until the vector phi infrastructure
        // handles all edge cases (duplicate fields, let-binding groups, power-of-2
        // widths, backedge register naming conflicts).
        self.fun.active_vector_groups.clear();
        self.fun.field_to_phi.clear();
        self.fun.field_to_lane.clear();
        self.fun.vector_phi_current.clear();

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

        // Counter phi — use the field's native LLVM type from field_types.
        let counter_ty = self.ctx.field_types.get(counter_idx)
            .cloned().unwrap_or_else(|| "i64".to_string());
        if is_decreasing {
            writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %.cm_latch ]",
                counter_name, counter_ty, init_name, next).ok();
        } else {
            writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %.cm_latch ]",
                counter_name, counter_ty, init_name, next).ok();
        }

        // 2026-07-17: Per-field phi nodes — one per written field.
        // 2026-07-26: Phi type is read from field_types to match the native
        // LLVM type stored by push_field_type (float/double for #Float types,
        // iN for exact ints, i64 for flexible Int and everything else).
        self.fun.phi_field_regs.clear();
        self.fun.backedge_field_regs.clear();

        for fname in &sorted_fields {
            // Check if this field duplicates the counter variable
            if let Some(cv) = counter_var {
                if fname.as_str() == cv {
                    self.fun.phi_field_regs.insert((*fname).clone(), counter_name.clone());
                    self.fun.backedge_field_regs.insert((*fname).clone(), next.clone());
                    continue;
                }
            }
            let phi_f = self.fun.next_reg_with_prefix("ppf");
            let be_f = be_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| format!("%be_{}", fname));
            let init_f = phi_field_init.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            let phi_ty = self.ctx.field_index_map.get(fname.as_str())
                .and_then(|idx| self.ctx.field_types.get(*idx))
                .cloned().unwrap_or_else(|| "i64".to_string());
            writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %.cm_latch ]",
                phi_f, phi_ty, init_f, be_f).ok();
            self.fun.phi_field_regs.insert((*fname).clone(), phi_f);
            self.fun.backedge_field_regs.insert((*fname).clone(), be_f);
        }

        // 2026-07-17: Exit check. For increasing counters we want
        // `counter < bound` (continue while below the bound); for decreasing
        // counters we want `counter > 0` (continue while above the bound).
        // The br branches to .cm_body if done_reg is true.
        // 2026-07-26: Counter may be narrower than 64 (native int width from
        // field_types). sext to i64 for comparison with bound (always i64).
        let cmp_counter = if counter_ty != "i64" {
            let w = self.fun.next_reg_with_prefix("cmw");
            writeln!(out, "  {} = sext {} {} to i64", w, counter_ty, counter_name).ok();
            w
        } else {
            counter_name.clone()
        };
        if is_decreasing {
            writeln!(out, "  {} = icmp sgt i64 {}, {}", done_reg, cmp_counter, bound_reg).ok();
        } else {
            writeln!(out, "  {} = icmp slt i64 {}, {}", done_reg, cmp_counter, bound_reg).ok();
        }
        // 2026-08-01 (C2/C3): liveliness watchdog for the memory-counter loop —
        // continue while `?[condition]` holds; on false, fire the handler with
        // the last computed value and exit (mirrors the countdown path).
        if let Some(wd) = watchdog {
            writeln!(out, "  br i1 {}, label %.cmwd_{}, label %{}", done_reg, self.fun.txn_counter, exit_label).ok();
            let wd_c0 = self.fun.txn_counter;
            self.fun.txn_counter += 1;
            writeln!(out, ".cmwd_{}:", wd_c0).ok();
            self.fun.cur_block = Some(format!(".cmwd_{}", wd_c0));
            let cond_reg = self.emit_expr(out, &wd.condition, "  ");
            let bool_reg = self.as_bool_reg(out, "  ", &cond_reg);
            writeln!(out, "  br i1 {}, label %.cm_body, label %.cmwdf_{}", bool_reg, wd_c0).ok();
            writeln!(out, ".cmwdf_{}:", wd_c0).ok();
            self.fun.cur_block = Some(format!(".cmwdf_{}", wd_c0));
            if let Some(on_fire) = &wd.on_fire {
                let call_reg = self.fun.gen_reg();
                let args: Vec<crate::ast::Expr> = match &on_fire.arg {
                    Some(name) => vec![crate::ast::Expr::Identifier(name.clone())],
                    None => Vec::new(),
                };
                self.emit_user_call(out, &call_reg, &on_fire.handler, &args, "  ");
            } else if wd.is_required {
                writeln!(out, "  call void @__watchdog_fail()").ok();
            }
            writeln!(out, "  br label %{}", exit_label).ok();
        } else {
            writeln!(out, "  br i1 {}, label %.cm_body, label %{}", done_reg, exit_label).ok();
        }
        writeln!(out, ".cm_body:").ok();

        // 2026-07-17: Initialize pending_phi_backedge with identity values.
        // Body writes will overwrite entries for modified fields.
        self.fun.pending_phi_backedge.clear();
        for fname in &sorted_fields {
            if let Some(phi_f) = self.fun.phi_field_regs.get(fname.as_str()) {
                self.fun.pending_phi_backedge.insert((*fname).clone(), phi_f.clone());
            }
        }

        // 2026-07-17: Path B (stores in body) for post-loop hoisted prints.
        // 2026-07-21: Also enabled by dispatch when phi-capped fields need stores
        // (float_math_nonzero p22 fix). Use OR to preserve pre-existing value.
        if !self.fun.pending_post_hoist.is_empty() {
            self.fun.needs_state_stores_in_body = true;
        }

        // 2026-07-17: pending_post_hoist (provided by the frontend swan-song
        // hoist, analysis/swan_song.rs) is emitted AFTER the loop closes, not
        // inside the body. The hoisted swan song reads final accumulator values
        // from %State (stored by Path B — needs_state_stores_in_body). Clone to
        // satisfy borrow checker (self.emit_expr needs &mut self;
        // pending_post_hoist is behind &self.fun).
        let hoist = self.fun.pending_post_hoist.clone();
        let mut empty = Vec::new();
        self.emit_countable_body(out, body, write_set, &mut empty);
        writeln!(out, "  br label %.cm_latch").ok();
        writeln!(out, ".cm_latch:").ok();
        // 2026-07-26: Counter increment uses the field's native type, not i64.
        if is_decreasing {
            writeln!(out, "  {} = sub nuw nsw {} {}, 1", next, counter_ty, counter_name).ok();
        } else {
            writeln!(out, "  {} = add nuw nsw {} {}, 1", next, counter_ty, counter_name).ok();
        }

        // 2026-07-17: Per-field scalar backedges. Modified fields use the written value;
        // unwritten fields use identity (phi self-ref). LLVM peephole eliminates
        // the `add i64 0, %val` copy in both cases.
        // 2026-07-21: Skip the counter variable — its backedge is already the
        // latch increment (next). Creating another identity would redefine next.
        // For float fields, the backedge value is already the native float type
        // (skip adapt_to_i64), so the identity would be a type mismatch.
        for fname in sorted_fields.iter().filter(|f| {
            counter_var.map_or(true, |cv| f.as_str() != cv)
        }) {
            if let Some(be_f) = self.fun.backedge_field_regs.get(fname.as_str()) {
                let val = self.fun.pending_phi_backedge.get(fname.as_str())
                    .cloned().unwrap_or_else(|| {
                        self.fun.phi_field_regs.get(fname.as_str())
                            .cloned().unwrap_or_else(|| "0".to_string())
                    });
                // Check if this field is a float type — skip i64 identity
                let field_ty = self.ctx.field_index_map.get(fname.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .cloned().unwrap_or_else(|| "i64".to_string());
                // 2026-07-26: The backedge identity must match the phi type.
                // Float fields use fadd, integer fields use the field's native width.
                if field_ty == "float" || field_ty == "double" {
                    writeln!(out, "  {} = fadd {} 0.0, {}", be_f, field_ty, val).ok();
                } else {
                    writeln!(out, "  {} = add {} 0, {}", be_f, field_ty, val).ok();
                }
            }
        }

        emit_loop_metadata(out, "  ", ".cm_header",
            &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
        writeln!(out, "{}:", exit_label).ok();
        // 2026-07-17: Emit hoisted post-loop prints (swan song) AFTER the loop
        // closes, so they read the final accumulator values from %State. The
        // guard condition was removed by the frontend swan-song hoist
        // (analysis/swan_song.rs); at this point the loop postcondition
        // guarantees it holds.
        // Clear the float cache to prevent reusing fpext registers from the
        // loop body (which may be defined in non-dominating conditional blocks
        // like periodic prints). Without this, the swan song reuses a register
        // defined inside a conditional that never fires for small BOUND values,
        // producing 0.0 from LLVM's undefined value handling — nbody_newton bug.
        self.fun.reg_float_cache.clear();
        // 2026-07-19: Clear last-value temps before hoisted post-loop prints.
        // Without this, identifier resolution uses SSA registers from the loop
        // body which don't dominate the exit block — SSA dominance violation.
        // The hoisted prints must resolve via phi registers or %State loads.
        self.fun.last_val_temps.clear();
        self.fun.last_val_types.clear();
        let hoist = self.fun.pending_post_hoist.clone();
        self.emit_hoisted_post_loop_prints(out, &hoist);
        self.emit_state_store_i64_by_idx(out, "  ", counter_idx, &counter_name);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ── Batch-Loop Emission ───────────────────────────────────────────
    //
    // 2026-07-31: Rebuilt from the Phase-6-removed emit_countable_batched_main
    // (docs/plans/2026-07-30-flat-node-decomposition.md §4), now consuming the
    // frontend BatchShape (analysis/batch_shape.rs) instead of the
    // extract_batch_size / split_hoistable heuristics. The io boundary is the
    // guard precondition's interval (`count % N == 0`), derived structurally.
    //
    // Only POST-increment guards are batched (the counter is incremented BEFORE
    // the guard, e.g. kalman/float_math) — for them the structure is EXACT:
    // the inner loop runs `batch_size` pure-compute iterations and the guard
    // fires at the boundary after the same number of computes as the composite.
    //
    // Structure (one @main):
    //   entry → .oh (outer header: phis + next boundary) → .inner (inner
    //   header: phis + exit check) → .il (inner pure body + latch) → inner_exit
    //   (fire io guard, store to %State) → .ox (bound check) → .done (post-loop
    //   hoist, ret) / .ol (outer latch: reload, loop).
    pub(crate) fn emit_countable_batched_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        write_set: &HashSet<String>,
        is_decreasing: bool,
        counter_var: &str,
        batch: &crate::analysis::batch_shape::BatchShape,
    ) {
        let batch_size = batch.batch_size as i64;
        self.emit_main_header(out, "#0", true);
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let c0 = self.fun.txn_counter;
        self.fun.txn_counter += 1;
        let bound_reg = self.fun.next_reg_with_prefix("obb");
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);

        // Pre-load initial values from %State for the outer phis.
        let mut sorted_fields: Vec<&String> = write_set.iter().collect();
        sorted_fields.sort();
        let mut phi_field_init: HashMap<String, String> = HashMap::new();
        for fname in &sorted_fields {
            if let Some(&idx) = self.ctx.field_index_map.get(fname.as_str()) {
                let (init_f, _) = self.emit_state_load_i64_by_idx(out, "  ", idx);
                phi_field_init.insert((*fname).clone(), init_f);
            }
        }
        let init_name = phi_field_init.get(counter_var)
            .cloned().unwrap_or_else(|| "0".to_string());

        let exit_label = format!(".oexit_{}", c0);
        let inner_exit_label = format!(".inner_exit_{}", c0);
        writeln!(out, "  br label %.oh_{}", c0).ok();

        // ── Outer Header ──────────────────────────────────────────
        // Phis track values across batches. Updated from entry (first batch)
        // or from the outer latch (subsequent batches).
        writeln!(out, ".oh_{}:", c0).ok();
        let oh_counter = self.fun.next_reg_with_prefix("ohc");
        let oh_bound = self.fun.next_reg_with_prefix("ohb");
        let next_oh = self.fun.next_reg_with_prefix("ohn");
        let counter_ty = self.ctx.field_types.get(counter_idx)
            .cloned().unwrap_or_else(|| "i64".to_string());
        writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %.ol_{} ]",
            oh_counter, counter_ty, init_name, next_oh, c0).ok();
        writeln!(out, "  {} = phi i64 [ {}, %entry ], [ {}, %.ol_{} ]",
            oh_bound, bound_reg, bound_reg, c0).ok();

        let mut oh_field_regs: HashMap<String, String> = HashMap::new();
        let mut oh_latch_regs: HashMap<String, String> = HashMap::new();
        for fname in &sorted_fields {
            if fname.as_str() == counter_var {
                continue;
            }
            let oh_f = self.fun.next_reg_with_prefix("ohs");
            let ol_f = self.fun.next_reg_with_prefix("olf");
            let init_f = phi_field_init.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            let phi_ty = self.ctx.field_index_map.get(fname.as_str())
                .and_then(|idx| self.ctx.field_types.get(*idx))
                .cloned().unwrap_or_else(|| "i64".to_string());
            writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %.ol_{} ]",
                oh_f, phi_ty, init_f, ol_f, c0).ok();
            oh_field_regs.insert((*fname).clone(), oh_f);
            oh_latch_regs.insert((*fname).clone(), ol_f);
        }

        // inner_end = min(bound, next_print_boundary)
        // next_print_boundary = ((counter / batch_size) + 1) * batch_size
        let bsize_reg = self.fun.next_reg_with_prefix("bsz");
        writeln!(out, "  {} = add i64 0, {}", bsize_reg, batch_size).ok();
        let div_reg = self.fun.next_reg_with_prefix("bdi");
        writeln!(out, "  {} = udiv i64 {}, {}", div_reg, oh_counter, bsize_reg).ok();
        let add_reg = self.fun.next_reg_with_prefix("bad");
        writeln!(out, "  {} = add i64 {}, 1", add_reg, div_reg).ok();
        let mul_reg = self.fun.next_reg_with_prefix("bmu");
        writeln!(out, "  {} = mul i64 {}, {}", mul_reg, add_reg, bsize_reg).ok();
        let inner_end = self.fun.next_reg_with_prefix("bie");
        writeln!(out, "  {} = call i64 @llvm.umin.i64(i64 {}, i64 {})",
            inner_end, oh_bound, mul_reg).ok();
        writeln!(out, "  br label %.inner_{}", c0).ok();

        // ── Inner Header ──────────────────────────────────────────
        // Phis fed by the outer phis (first iteration) then the inner latch.
        writeln!(out, ".inner_{}:", c0).ok();
        let i_counter = self.fun.next_reg_with_prefix("icc");
        let next_i = self.fun.next_reg_with_prefix("icn");
        writeln!(out, "  {} = phi {} [ {}, %.oh_{} ], [ {}, %.il_{} ]",
            i_counter, counter_ty, oh_counter, c0, next_i, c0).ok();

        self.fun.phi_field_regs.clear();
        self.fun.backedge_field_regs.clear();
        self.fun.phi_field_regs.insert(counter_var.to_string(), i_counter.clone());
        self.fun.backedge_field_regs.insert(counter_var.to_string(), next_i.clone());

        for fname in &sorted_fields {
            if fname.as_str() == counter_var {
                continue;
            }
            let i_f = self.fun.next_reg_with_prefix("ifs");
            let be_f = self.fun.next_reg_with_prefix("ibf");
            let init_f = oh_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            let phi_ty = self.ctx.field_index_map.get(fname.as_str())
                .and_then(|idx| self.ctx.field_types.get(*idx))
                .cloned().unwrap_or_else(|| "i64".to_string());
            writeln!(out, "  {} = phi {} [ {}, %.oh_{} ], [ {}, %.il_{} ]",
                i_f, phi_ty, init_f, c0, be_f, c0).ok();
            self.fun.phi_field_regs.insert((*fname).clone(), i_f);
            self.fun.backedge_field_regs.insert((*fname).clone(), be_f);
        }

        // Inner exit check — continue while counter < inner_end.
        let cmp_i_counter = if counter_ty != "i64" {
            let w = self.fun.next_reg_with_prefix("icw");
            writeln!(out, "  {} = sext {} {} to i64", w, counter_ty, i_counter).ok();
            w
        } else {
            i_counter.clone()
        };
        let exit_reg = self.fun.next_reg_with_prefix("iex");
        if is_decreasing {
            writeln!(out, "  {} = icmp sgt i64 {}, {}", exit_reg, cmp_i_counter, inner_end).ok();
        } else {
            writeln!(out, "  {} = icmp slt i64 {}, {}", exit_reg, cmp_i_counter, inner_end).ok();
        }
        writeln!(out, "  br i1 {}, label %.il_{}, label %{}", exit_reg, c0, inner_exit_label).ok();

        // ── Inner Body + Latch ────────────────────────────────────
        // Pure compute (guard removed). Field reads resolve to the inner phis;
        // writes go to pending_phi_backedge (no per-iteration %State traffic).
        writeln!(out, ".il_{}:", c0).ok();
        self.fun.pending_phi_backedge.clear();
        for fname in &sorted_fields {
            let init_val = self.fun.phi_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            self.fun.pending_phi_backedge.insert((*fname).clone(), init_val.clone());
        }
        let mut empty = Vec::new();
        self.emit_countable_body(out, &batch.inner_body, write_set, &mut empty);
        // Counter increment (native width) — the counter's backedge.
        if is_decreasing {
            writeln!(out, "  {} = sub nuw nsw {} {}, 1", next_i, counter_ty, i_counter).ok();
        } else {
            writeln!(out, "  {} = add nuw nsw {} {}, 1", next_i, counter_ty, i_counter).ok();
        }
        self.fun.pending_phi_backedge.insert(counter_var.to_string(), next_i.clone());
        // Field backedges (skip the counter — its backedge is next_i above).
        for fname in sorted_fields.iter().filter(|f| f.as_str() != counter_var) {
            if let Some(be_f) = self.fun.backedge_field_regs.get(fname.as_str()) {
                let val = self.fun.pending_phi_backedge.get(fname.as_str())
                    .cloned().unwrap_or_else(|| {
                        self.fun.phi_field_regs.get(fname.as_str())
                            .cloned().unwrap_or_else(|| "0".to_string())
                    });
                let field_ty = self.ctx.field_index_map.get(fname.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .cloned().unwrap_or_else(|| "i64".to_string());
                if field_ty == "float" || field_ty == "double" {
                    writeln!(out, "  {} = fadd {} 0.0, {}", be_f, field_ty, val).ok();
                } else {
                    writeln!(out, "  {} = add {} 0, {}", be_f, field_ty, val).ok();
                }
            }
        }
        writeln!(out, "  br label %.inner_{}", c0).ok();

        // ── Inner Exit ────────────────────────────────────────────
        // The inner loop completed one batch. Fire the io guard — re-evaluating
        // `count % N == 0` here reads the counter phi (a boundary multiple), so
        // it is true and the body runs. Let-bindings referenced by the guard are
        // remapped to their stored state fields (they are not live here).
        writeln!(out, "{}:", inner_exit_label).ok();
        let mut let_to_field: HashMap<String, String> = HashMap::new();
        for stmt in &batch.inner_body {
            if let Statement::Assign(lhs, Expr::Identifier(let_name)) = stmt {
                if let Some(field_name) = lhs.as_var_name() {
                    // 2026-08-01 (A9b): a field-to-field assignment (`queue = count`,
                    // the `<-` push's lowered AST) is NOT a let-alias — `count` is a
                    // field, so the guard must keep reading the count field, not the
                    // queue. Only a genuine non-field local (`field = local`, the
                    // `sum = acc` hoist pattern) creates an alias.
                    if self.ctx.field_index_map.contains_key(field_name)
                        && !self.ctx.field_index_map.contains_key(let_name)
                    {
                        let_to_field.insert(let_name.clone(), field_name.to_string());
                    }
                }
            }
        }
        let mut guard = batch.guard.clone();
        crate::analysis::swan_song::remap_stmt_identifiers(&mut guard, &let_to_field);
        self.fun.last_val_temps.clear();
        self.fun.last_val_types.clear();
        let mut empty2 = Vec::new();
        self.emit_countable_body(out, std::slice::from_ref(&guard), write_set, &mut empty2);
        // Store final values to %State for the outer latch (per-batch — once per
        // `batch_size` iterations, negligible). Use the inner HEADER phis (they
        // dominate inner_exit; the latch backedge registers do not). The guard
        // above reads these same phis, so the stored values reflect the batch's
        // final state.
        for fname in &sorted_fields {
            let val = self.fun.phi_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            if let Some(&idx) = self.ctx.field_index_map.get(fname.as_str()) {
                self.emit_state_store_i64_by_idx(out, "  ", idx, &val);
            }
        }
        // The counter's header phi is the boundary value (e.g. 5M) at inner_exit.
        let final_counter = i_counter.clone();
        self.emit_state_store_i64_by_idx(out, "  ", counter_idx, &final_counter);
        writeln!(out, "  br label %.ox_{}", c0).ok();

        // ── Outer Body (Termination Check) ────────────────────────
        writeln!(out, ".ox_{}:", c0).ok();
        let (final_count_load, _) = self.emit_state_load_i64_by_idx(out, "  ", counter_idx);
        let done_reg = self.fun.next_reg_with_prefix("odn");
        if is_decreasing {
            writeln!(out, "  {} = icmp sle i64 {}, {}", done_reg, final_count_load, oh_bound).ok();
        } else {
            writeln!(out, "  {} = icmp sge i64 {}, {}", done_reg, final_count_load, oh_bound).ok();
        }
        writeln!(out, "  br i1 {}, label %.done_{}, label %.ol_{}", done_reg, c0, c0).ok();

        // ── Done / Exit ───────────────────────────────────────────
        writeln!(out, ".done_{}:", c0).ok();
        self.fun.reg_float_cache.clear();
        self.fun.last_val_temps.clear();
        self.fun.last_val_types.clear();
        let pending: Vec<Vec<Statement>> = self.fun.pending_post_hoist.clone();
        if !pending.is_empty() {
            for group in &pending {
                let mut empty3 = Vec::new();
                self.emit_countable_body(out, group, &HashSet::new(), &mut empty3);
            }
        }
        writeln!(out, "  ret i32 0").ok();

        // ── Outer Latch ───────────────────────────────────────────
        writeln!(out, ".ol_{}:", c0).ok();
        writeln!(out, "  {} = add {} 0, {}", next_oh, counter_ty, final_counter).ok();
        for fname in &sorted_fields {
            if fname.as_str() == counter_var {
                continue;
            }
            let ol_f = oh_latch_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| self.fun.next_reg_with_prefix("olf"));
            if let Some(&idx) = self.ctx.field_index_map.get(fname.as_str()) {
                let (val, _) = self.emit_state_load_i64_by_idx(out, "  ", idx);
                let field_ty = self.ctx.field_index_map.get(fname.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .cloned().unwrap_or_else(|| "i64".to_string());
                if field_ty == "float" || field_ty == "double" {
                    writeln!(out, "  {} = fadd {} 0.0, {}", ol_f, field_ty, val).ok();
                } else {
                    writeln!(out, "  {} = add {} 0, {}", ol_f, field_ty, val).ok();
                }
            }
        }
        writeln!(out, "  br label %.oh_{}", c0).ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
        let _ = txn_name;
    }

    // 2026-07-29: emit_countable_memory_main removed — dead code after Phase 4.
    // PerFieldPhi (emit_countable_main) handles all cases.

    // ── Version-DAG Emission ─────────────────────────────────────────
    //
    // ── Countdown-Loop Emission ───────────────────────────────────────
    //
    // 2026-07-31: Single tight loop for periodic post-increment io guards
    // (`when count % N == 0` AFTER count++). Instead of the batch's outer/inner
    // structure, a loop-carried `%rem` counter decrements each iteration; when
    // it reaches 0, a COLD guard block prints and resets `%rem = N`.
    //
    // WHY this shape (plan 2026-07-31-fmn-countdown-vs-batch-and-new-benchmarks):
    // the version-DAG's guard-in-loop costs a modulo + body-split (~5 extra
    // instructions vs C) AND the batch's PURE inner loop lets LLVM's vectorizer
    // mis-vectorize cross-indexed matrix bodies (fmn: 14 shuffle-heavy
    // instructions, slower than 29 scalar). The countdown keeps the loop in ONE
    // block (no body-split), replaces the modulo with `sub;cmp` (2 instructions),
    // and its `%fire` conditional naturally blocks the bad vectorization.
    //
    // Structure:
    //   entry → .cd (header: phis %count/%rem/%fields, bound check)
    //         → .cdb (body ONE block: compute + count++ + rem--)
    //         → .cdg (COLD guard block: print, rem = N)  /  .cdl (latch)
    //         → .cde (done: post-loop hoist, ret)
    pub(crate) fn emit_countable_countdown_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        write_set: &HashSet<String>,
        counter_var: &str,
        batch: &crate::analysis::batch_shape::BatchShape,
        watchdog: Option<&crate::ast::top::WatchdogSpec>,
        free_after: &[String],
    ) {
        let batch_size = batch.batch_size as i64;
        self.emit_main_header(out, "#0", true);
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let c0 = self.fun.txn_counter;
        self.fun.txn_counter += 1;
        let bound_reg = self.fun.next_reg_with_prefix("cdb");
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);

        // Pre-load initial values for the header phis.
        // 2026-07-31 (A4): aggregate (array) fields are excluded from phis —
        // they are memory-resident and accessed via the %State GEP path.
        let mut sorted_fields: Vec<&String> = write_set
            .iter()
            .filter(|f| !self.is_aggregate_field(f))
            .collect();
        sorted_fields.sort();
        let mut phi_field_init: HashMap<String, String> = HashMap::new();
        for fname in &sorted_fields {
            if let Some(&idx) = self.ctx.field_index_map.get(fname.as_str()) {
                let (init_f, _) = self.emit_state_load_i64_by_idx(out, "  ", idx);
                phi_field_init.insert((*fname).clone(), init_f);
            }
        }
        let init_name = phi_field_init.get(counter_var)
            .cloned().unwrap_or_else(|| "0".to_string());
        let counter_ty = self.ctx.field_types.get(counter_idx)
            .cloned().unwrap_or_else(|| "i64".to_string());
        writeln!(out, "  br label %.cd_{}", c0).ok();

        // ── Header ──────────────────────────────────────────────
        writeln!(out, ".cd_{}:", c0).ok();
        let c_counter = self.fun.next_reg_with_prefix("cdc");
        let c_next = self.fun.next_reg_with_prefix("cdn");
        let c_rem = self.fun.next_reg_with_prefix("cdr");
        let c_rem_latch = self.fun.next_reg_with_prefix("cdl");
        writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %.cdl_{} ]",
            c_counter, counter_ty, init_name, c_next, c0).ok();
        writeln!(out, "  {} = phi i64 [ {}, %entry ], [ {}, %.cdl_{} ]",
            c_rem, batch_size, c_rem_latch, c0).ok();

        self.fun.phi_field_regs.clear();
        self.fun.backedge_field_regs.clear();
        self.fun.phi_field_regs.insert(counter_var.to_string(), c_counter.clone());
        self.fun.backedge_field_regs.insert(counter_var.to_string(), c_next.clone());
        for fname in &sorted_fields {
            if fname.as_str() == counter_var {
                continue;
            }
            let f_reg = self.fun.next_reg_with_prefix("cdf");
            let f_be = self.fun.next_reg_with_prefix("cbe");
            let init_f = phi_field_init.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            let phi_ty = self.ctx.field_index_map.get(fname.as_str())
                .and_then(|idx| self.ctx.field_types.get(*idx))
                .cloned().unwrap_or_else(|| "i64".to_string());
            writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %.cdl_{} ]",
                f_reg, phi_ty, init_f, f_be, c0).ok();
            self.fun.phi_field_regs.insert((*fname).clone(), f_reg);
            self.fun.backedge_field_regs.insert((*fname).clone(), f_be);
        }

        // Exit check — continue while count < bound.
        let cmp_counter = if counter_ty != "i64" {
            let w = self.fun.next_reg_with_prefix("cdw");
            writeln!(out, "  {} = sext {} {} to i64", w, counter_ty, c_counter).ok();
            w
        } else {
            c_counter.clone()
        };
        let done_reg = self.fun.next_reg_with_prefix("cdd");
        writeln!(out, "  {} = icmp slt i64 {}, {}", done_reg, cmp_counter, bound_reg).ok();
        // 2026-08-01 (C2/C3): liveliness watchdog — the loop continues while
        // `?[condition]` holds; when it stops, fire the on-fire handler with
        // the last computed value and exit. The check sits between the header
        // and the body (per-iteration), branching to a cold `.wdf_` fire block.
        if let Some(wd) = watchdog {
            writeln!(out, "  br i1 {}, label %.cdw_{}, label %.cde_{}", done_reg, c0, c0).ok();
            writeln!(out, ".cdw_{}:", c0).ok();
            self.fun.cur_block = Some(format!(".cdw_{}", c0));
            let cond_reg = self.emit_expr(out, &wd.condition, "  ");
            let bool_reg = self.as_bool_reg(out, "  ", &cond_reg);
            writeln!(out, "  br i1 {}, label %.cdb_{}, label %.wdf_{}", bool_reg, c0, c0).ok();
            // ── Watchdog fired (COLD) ──────────────────────────
            writeln!(out, ".wdf_{}:", c0).ok();
            self.fun.cur_block = Some(format!(".wdf_{}", c0));
            if let Some(on_fire) = &wd.on_fire {
                let call_reg = self.fun.gen_reg();
                let args: Vec<crate::ast::Expr> = match &on_fire.arg {
                    Some(name) => vec![crate::ast::Expr::Identifier(name.clone())],
                    None => Vec::new(),
                };
                self.emit_user_call(out, &call_reg, &on_fire.handler, &args, "  ");
            } else if wd.is_required {
                // Required watchdog with no handler: error exit.
                writeln!(out, "  call void @__watchdog_fail()").ok();
            }
            writeln!(out, "  br label %.cde_{}", c0).ok();
        } else {
            writeln!(out, "  br i1 {}, label %.cdb_{}, label %.cde_{}", done_reg, c0, c0).ok();
        }

        // ── Body (ONE block) ────────────────────────────────────
        writeln!(out, ".cdb_{}:", c0).ok();
        self.fun.cur_block = Some(format!(".cdb_{}", c0));
        self.fun.pending_phi_backedge.clear();
        for fname in &sorted_fields {
            let init_val = self.fun.phi_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            self.fun.pending_phi_backedge.insert((*fname).clone(), init_val.clone());
        }
        let mut empty = Vec::new();
        self.emit_countable_body(out, &batch.inner_body, write_set, &mut empty);
        // Countdown: remaining-- (loop-carried, independent of the counter).
        // 2026-08-01 (B): the rem/fire instructions land in the inner body's
        // FINAL block (cur_block) — an if-ended body leaves the emitter in the
        // if's merge block, so `.cdb_`'s terminator is the if's br. The latch
        // phis below use that block as the `.cdb_` predecessor.
        let body_final = self.fun.cur_block.clone()
            .unwrap_or_else(|| format!(".cdb_{}", c0));
        let c_rem_next = self.fun.next_reg_with_prefix("cdm");
        writeln!(out, "  {} = sub i64 {}, 1", c_rem_next, c_rem).ok();
        let fire = self.fun.next_reg_with_prefix("cdf");
        writeln!(out, "  {} = icmp eq i64 {}, 0", fire, c_rem_next).ok();
        writeln!(out, "  br i1 {}, label %.cdg_{}, label %.cdl_{}", fire, c0, c0).ok();

        // Fields the guard WRITES (e.g. accumulator_flush's `sum = 0` reset)
        // need a latch phi merging the body's value (.cdb) with the guard's
        // (.cdg) — the guard's write register does not dominate the latch.
        // Print-only guards have an empty set and take the plain backedge path.
        let mut guard_writes: HashSet<String> = HashSet::new();
        for stmt in &batch.guard_body {
            if let Statement::Assign(lhs, _) = stmt {
                if let Some(field_name) = lhs.as_var_name() {
                    if self.ctx.field_index_map.contains_key(field_name) {
                        guard_writes.insert(field_name.to_string());
                    }
                }
            }
        }
        // Save the body's per-field values BEFORE the guard overwrites them, so
        // the latch's .cdb phi entry sees the body's compute.
        let body_backedges = self.fun.pending_phi_backedge.clone();

        // ── Guard (COLD — 1 in N iterations) ────────────────────
        // The io guard fires here (remaining == 0 is known true), so only the
        // guard BODY is emitted — no conditional branch structure, keeping
        // .cdl's predecessors exactly {.cdb, .cdg}. The body reads the header
        // phis (post-compute state); let-bindings referenced by the guard are
        // remapped to their stored state fields.
        writeln!(out, ".cdg_{}:", c0).ok();
        self.fun.cur_block = Some(format!(".cdg_{}", c0));
        let mut let_to_field: HashMap<String, String> = HashMap::new();
        for stmt in &batch.inner_body {
            if let Statement::Assign(lhs, Expr::Identifier(let_name)) = stmt {
                if let Some(field_name) = lhs.as_var_name() {
                    // 2026-08-01 (A9b): a field-to-field assignment (`queue = count`,
                    // the `<-` push's lowered AST) is NOT a let-alias — `count` is a
                    // field, so the guard must keep reading the count field, not the
                    // queue. Only a genuine non-field local (`field = local`, the
                    // `sum = acc` hoist pattern) creates an alias.
                    if self.ctx.field_index_map.contains_key(field_name)
                        && !self.ctx.field_index_map.contains_key(let_name)
                    {
                        let_to_field.insert(let_name.clone(), field_name.to_string());
                    }
                }
            }
        }
        let mut guard_body = batch.guard_body.clone();
        for s in &mut guard_body {
            crate::analysis::swan_song::remap_stmt_identifiers(s, &let_to_field);
        }

        // 2026-07-31: Do NOT clear last_val_temps here. The guard fires mid-loop
        // (before the latch), so the header phis still hold the PRE-body values;
        // the current iteration's computed state lives in last_val_temps (the
        // body's assigns, defined in .cdb which dominates .cdg). Clearing it
        // would make the guard print the previous iteration's state — the
        // 5M+1-compute bug (kalman printed 8.188e12 instead of 8.139e12).
        // 2026-08-01 (B): the rem reset is emitted BEFORE the guard body so it
        // is defined in .cdg_ and dominates the guard's control flow. A guard
        // body that ends in a `when` leaves the emitter in the when's
        // next_label; the latch br below lands there, and the latch phis use
        // cur_block (the final block) as the guard predecessor — hardcoding
        // .cdg_ broke the phi's predecessor set for when-ended guards.
        let rem_reset = self.fun.next_reg_with_prefix("cdz");
        writeln!(out, "  {} = add i64 0, {}", rem_reset, batch_size).ok();
        let mut empty2 = Vec::new();
        self.emit_countable_body(out, &guard_body, write_set, &mut empty2);
        writeln!(out, "  br label %.cdl_{}", c0).ok();

        // ── Latch ──────────────────────────────────────────────
        // %rem_latch = phi [remaining-1, body], [N, guard]. All phis (rem +
        // guard-written fields) are grouped at the TOP of the block per LLVM
        // rules; non-phi backedges follow. The guard predecessor is the
        // guard's FINAL block (cur_block) — a when-ended guard branches to
        // .cdl_ from its next_label, not from .cdg_ itself.
        let guard_pred = self.fun.cur_block.clone()
            .unwrap_or_else(|| format!(".cdg_{}", c0));
        writeln!(out, ".cdl_{}:", c0).ok();
        writeln!(out, "  {} = phi i64 [ {}, %{} ], [ {}, %{} ]",
            c_rem_latch, c_rem_next, body_final, rem_reset, guard_pred).ok();
        for fname in sorted_fields.iter().filter(|f| {
            f.as_str() != counter_var && guard_writes.contains(f.as_str())
        }) {
            if let Some(be_f) = self.fun.backedge_field_regs.get(fname.as_str()) {
                let field_ty = self.ctx.field_index_map.get(fname.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .cloned().unwrap_or_else(|| "i64".to_string());
                let body_val = body_backedges.get(fname.as_str())
                    .cloned().unwrap_or_else(|| {
                        self.fun.phi_field_regs.get(fname.as_str())
                            .cloned().unwrap_or_else(|| "0".to_string())
                    });
                let guard_val = self.fun.pending_phi_backedge.get(fname.as_str())
                    .cloned().unwrap_or_else(|| body_val.clone());
                writeln!(out, "  {} = phi {} [ {}, %{} ], [ {}, %{} ]",
                    be_f, field_ty, body_val, body_final, guard_val, guard_pred).ok();
            }
        }
        // Counter increment (native width) — the counter's backedge.
        writeln!(out, "  {} = add nuw nsw {} {}, 1", c_next, counter_ty, c_counter).ok();
        self.fun.pending_phi_backedge.insert(counter_var.to_string(), c_next.clone());
        // Non-guard-written field backedges (skip the counter — its backedge is
        // c_next above).
        for fname in sorted_fields.iter().filter(|f| {
            f.as_str() != counter_var && !guard_writes.contains(f.as_str())
        }) {
            if let Some(be_f) = self.fun.backedge_field_regs.get(fname.as_str()) {
                let val = self.fun.pending_phi_backedge.get(fname.as_str())
                    .cloned().unwrap_or_else(|| {
                        self.fun.phi_field_regs.get(fname.as_str())
                            .cloned().unwrap_or_else(|| "0".to_string())
                    });
                let field_ty = self.ctx.field_index_map.get(fname.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .cloned().unwrap_or_else(|| "i64".to_string());
                if field_ty == "float" || field_ty == "double" {
                    writeln!(out, "  {} = fadd {} 0.0, {}", be_f, field_ty, val).ok();
                } else {
                    writeln!(out, "  {} = add {} 0, {}", be_f, field_ty, val).ok();
                }
            }
        }
        writeln!(out, "  br label %.cd_{}", c0).ok();

        // ── Done / Exit ─────────────────────────────────────────
        writeln!(out, ".cde_{}:", c0).ok();
        self.fun.reg_float_cache.clear();
        self.fun.last_val_temps.clear();
        self.fun.last_val_types.clear();
        // Store final phi values so a post-loop hoist can read them from %State.
        for fname in &sorted_fields {
            let val = self.fun.phi_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            if let Some(&idx) = self.ctx.field_index_map.get(fname.as_str()) {
                self.emit_state_store_i64_by_idx(out, "  ", idx, &val);
            }
        }
        let pending: Vec<Vec<Statement>> = self.fun.pending_post_hoist.clone();
        if !pending.is_empty() {
            for group in &pending {
                let mut empty3 = Vec::new();
                self.emit_countable_body(out, group, &HashSet::new(), &mut empty3);
            }
        }
        // 2026-08-01 (D2): garbage scheduling — emit the `Free#` for each
        // heap-backed state field whose reactor-ordered last consumer is this
        // countdown transaction. The free fires exactly once, after the whole
        // loop completes (a per-iteration free would be a use-after-free). The
        // handle is the field's STORED value (the ptrtoint of the allocation),
        // loaded from %State — re-evaluating the initializer would re-malloc.
        for f in free_after {
            let Some(&fidx) = self.ctx.field_index_map.get(f) else { continue; };
            let (handle, _) = self.emit_state_load_i64_by_idx(out, "  ", fidx);
            let ptr = self.fun.gen_reg();
            writeln!(out, "  {} = inttoptr i64 {} to ptr", ptr, handle).ok();
            writeln!(out, "  call void @free(ptr {})", ptr).ok();
        }
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
        let _ = txn_name;
    }

    // ── Version-DAG Emission ─────────────────────────────────────────
    //
    // 2026-07-31: Emit the composite-node decomposition for a transaction
    // body containing ONE runtime `when` guard. The body is split at the
    // guard into [pre], [guard], [post] (see analysis/node_decompose.rs).
    // Two versions are emitted:
    //
    //   guard-absent loop:  [pre] → check predicate → [post] (self-terminating)
    //   guard-present block: [pre] → [guard] → [post] (fires when predicate holds)
    //
    // The guard predicate is evaluated BETWEEN [pre] and [post], at the split
    // point — this captures whether the guard observes the counter pre- or
    // post-increment naturally (no position scanning, no counter-name matching).
    //
    // Returns false if the body has no runtime guard or more than one — the
    // caller falls back to PerFieldPhi (emit_countable_main).
    //
    // See docs/plans/2026-07-30-flat-node-decomposition.md §11.
    pub(crate) fn emit_version_dag_main(
        &mut self,
        out: &mut String,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        body: &[Statement],
        write_set: &HashSet<String>,
        is_decreasing: bool,
        counter_var: Option<&str>,
    ) -> bool {
        use crate::analysis::node_decompose::{PredicateClass, Segment, split_into_segments};
        let segments = split_into_segments(body);

        // Locate the single runtime guard and collect [pre] / [post] statements.
        let mut pre: Vec<Statement> = Vec::new();
        let mut post: Vec<Statement> = Vec::new();
        let mut runtime_guard: Option<(&Expr, &Vec<Statement>)> = None;
        let mut seen_guard = false;
        for seg in &segments {
            match seg {
                Segment::Compute(stmts) => {
                    if runtime_guard.is_none() {
                        pre.extend(stmts.clone());
                    } else {
                        post.extend(stmts.clone());
                    }
                }
                Segment::Guard { condition, body, classification, .. } => {
                    if seen_guard {
                        return false; // multiple guards — fall back to PerFieldPhi
                    }
                    seen_guard = true;
                    match classification {
                        PredicateClass::Runtime => {
                            runtime_guard = Some((condition, body));
                        }
                        // 2026-07-31: Static predicates are handled by inlining
                        // (always-true) or dropping (always-false). Both mean no
                        // runtime version split is needed — the guard body is
                        // either always executed or never, so we fold it into
                        // [pre]/[post] and let PerFieldPhi emit the single loop.
                        PredicateClass::AlwaysTrue | PredicateClass::AlwaysFalse => {
                            return false;
                        }
                    }
                }
            }
        }
        let Some((guard_cond, guard_body)) = runtime_guard else {
            return false; // no runtime guard — PerFieldPhi
        };

        // ── Emit @main with the guard-absent loop + guard-present block ──
        let c0 = self.fun.txn_counter;
        let vd_prefix = format!("vd{}", c0);
        self.fun.txn_counter += 1;
        let header_label = format!(".{}_header", vd_prefix);
        let absent_label = format!(".{}_absent", vd_prefix);
        let latch_label = format!(".{}_latch", vd_prefix);
        let present_label = format!(".{}_present", vd_prefix);
        let end_label = format!(".{}_end", vd_prefix);

        self.emit_main_header(out, "#0", true);
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let bound_reg = self.fun.next_reg_with_prefix("vdb");
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);
        let (init_name, _) = self.emit_state_load_i64_by_idx(out, "  ", counter_idx);

        let mut sorted_fields: Vec<&String> = write_set.iter().collect();
        sorted_fields.sort();
        let mut phi_field_init: HashMap<String, String> = HashMap::new();
        for fname in &sorted_fields {
            let idx = match self.ctx.field_index_map.get(fname.as_str()) {
                Some(&i) => i,
                None => continue,
            };
            let (init_f, _) = self.emit_state_load_i64_by_idx(out, "  ", idx);
            phi_field_init.insert((*fname).clone(), init_f);
        }

        // Pre-generate backedge register names: one set for the latch, one for
        // the present block (both are header predecessors). The counter's
        // backedge registers live in these maps too (indexed by counter_var).
        let mut be_latch_regs: HashMap<String, String> = HashMap::new();
        let mut be_present_regs: HashMap<String, String> = HashMap::new();
        for fname in &sorted_fields {
            be_latch_regs.insert((*fname).clone(), self.fun.next_reg_with_prefix("bl"));
            be_present_regs.insert((*fname).clone(), self.fun.next_reg_with_prefix("bp"));
        }

        let counter_ty = self.ctx.field_types.get(counter_idx)
            .cloned().unwrap_or_else(|| "i64".to_string());
        writeln!(out, "  br label %{}", header_label).ok();

        // ── Header: per-field phis ───────────────────────────────────
        // 2026-07-31: Minimal-state classification (Phase 7). Fields never
        // written in the loop are hoisted (no phi); fields written but never
        // read are dropped. Only loop-carried fields get phis. The body
        // includes the guard, so a field read only by the guard is carried.
        let hoist_flat: Vec<Statement> = self.fun.pending_post_hoist.iter()
            .flat_map(|g| g.clone()).collect();
        let observables: Vec<&[Statement]> = vec![guard_body, &hoist_flat];
        let field_classes = crate::analysis::loop_carried::classify_fields(
            &write_set, body, &[], &observables,
        );
        writeln!(out, "{}:", header_label).ok();
        self.fun.phi_field_regs.clear();
        self.fun.backedge_field_regs.clear();
        let counter_name = self.fun.next_reg_with_prefix("vdc");
        let counter_key = counter_var.map(|s| s.to_string()).unwrap_or_else(|| "count".to_string());
        let be_l_count = be_latch_regs.get(&counter_key)
            .cloned().unwrap_or_else(|| self.fun.next_reg_with_prefix("bl"));
        let be_p_count = be_present_regs.get(&counter_key)
            .cloned().unwrap_or_else(|| self.fun.next_reg_with_prefix("bp"));
        writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %{} ], [ {}, %{} ]",
            counter_name, counter_ty, init_name, be_l_count, latch_label,
            be_p_count, present_label).ok();
        for fname in &sorted_fields {
            if let Some(cv) = counter_var {
                if fname.as_str() == cv {
                    self.fun.phi_field_regs.insert((*fname).clone(), counter_name.clone());
                    continue;
                }
            }
            let phi_f = self.fun.next_reg_with_prefix("vdf");
            let be_l = be_latch_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| self.fun.next_reg_with_prefix("bl"));
            let be_p = be_present_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| self.fun.next_reg_with_prefix("bp"));
            let init_f = phi_field_init.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            let phi_ty = self.ctx.field_index_map.get(fname.as_str())
                .and_then(|idx| self.ctx.field_types.get(*idx))
                .cloned().unwrap_or_else(|| "i64".to_string());
            // 2026-07-31: Minimal-state — loop-invariant fields are hoisted
            // (their entry load is the hoisted value, no phi); dead fields are
            // dropped; loop-carried fields get a phi.
            match field_classes.get(fname.as_str()) {
                Some(crate::analysis::loop_carried::FieldClass::LoopInvariant) => {
                    self.fun.phi_field_regs.insert((*fname).clone(), init_f);
                }
                Some(crate::analysis::loop_carried::FieldClass::Dead) => {
                    // Skipped — no phi, no backedge, body writes dropped.
                }
                _ => {
                    writeln!(out, "  {} = phi {} [ {}, %entry ], [ {}, %{} ], [ {}, %{} ]",
                        phi_f, phi_ty, init_f, be_l, latch_label, be_p, present_label).ok();
                    self.fun.phi_field_regs.insert((*fname).clone(), phi_f);
                }
            }
        }
        // 2026-07-31: Save the header phi registers — the present block must
        // read them (they dominate it), not the absent body's post-[pre] regs.
        let header_phi_regs: HashMap<String, String> = self.fun.phi_field_regs.clone();
        // 2026-07-31: Exit check AT THE HEADER — evaluated before the guard
        // predicate, so the present block never fires at count == bound.
        // count < bound → absent_body; count >= bound → end.
        let cmp_counter_h = if counter_ty != "i64" {
            let w = self.fun.next_reg_with_prefix("vdw");
            writeln!(out, "  {} = sext {} {} to i64", w, counter_ty, counter_name).ok();
            w
        } else {
            counter_name.clone()
        };
        let done_h = self.fun.next_reg_with_prefix("vdd");
        if is_decreasing {
            writeln!(out, "  {} = icmp sgt i64 {}, {}", done_h, cmp_counter_h, bound_reg).ok();
        } else {
            writeln!(out, "  {} = icmp slt i64 {}, {}", done_h, cmp_counter_h, bound_reg).ok();
        }
        writeln!(out, "  br i1 {}, label %{}, label %{}", done_h, absent_label, end_label).ok();

        // ── Guard-absent body: [pre], predicate check ───────────────
        writeln!(out, "{}:", absent_label).ok();
        self.fun.pending_phi_backedge.clear();
        for fname in &sorted_fields {
            let init_val = self.fun.phi_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            self.fun.pending_phi_backedge.insert((*fname).clone(), init_val);
        }
        self.emit_countable_body(out, &pre, write_set, &mut vec![]);
        // Update phi_field_regs to post-[pre] values so the predicate reads them.
        for fname in &sorted_fields {
            if let Some(v) = self.fun.pending_phi_backedge.get(fname.as_str()) {
                self.fun.phi_field_regs.insert((*fname).clone(), v.clone());
            }
        }
        // Evaluate the guard predicate at the split point.
        let pred_reg = self.emit_expr(out, guard_cond, "  ");
        let pred_bool = self.fun.next_reg_with_prefix("vdb");
        writeln!(out, "  {} = trunc i8 {} to i1", pred_bool, pred_reg.name).ok();
        writeln!(out, "  br i1 {}, label %{}, label %{}",
            pred_bool, present_label, latch_label).ok();

        // ── Latch: [post] + backedge to header ──────────────────────
        writeln!(out, "{}:", latch_label).ok();
        self.emit_countable_body(out, &post, write_set, &mut vec![]);
        // 2026-07-31: The counter increment lives in [pre] or [post] (the
        // source's `count = count + 1` statement). After [post],
        // pending_phi_backedge[count] holds the incremented value. The loop
        // below emits an identity copy to be_latch_regs[count] — the register
        // the header's counter phi references from the latch predecessor.
        // Emit latch backedges for fields (including the counter).
        for fname in &sorted_fields {
            if let Some(be_f) = be_latch_regs.get(fname.as_str()) {
                let val = self.fun.pending_phi_backedge.get(fname.as_str())
                    .cloned().unwrap_or_else(|| {
                        self.fun.phi_field_regs.get(fname.as_str())
                            .cloned().unwrap_or_else(|| "0".to_string())
                    });
                let field_ty = self.ctx.field_index_map.get(fname.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .cloned().unwrap_or_else(|| "i64".to_string());
                if field_ty == "float" || field_ty == "double" {
                    writeln!(out, "  {} = fadd {} 0.0, {}", be_f, field_ty, val).ok();
                } else {
                    writeln!(out, "  {} = add {} 0, {}", be_f, field_ty, val).ok();
                }
            }
        }
        // 2026-07-31: Exit check is at the header; the latch just backs to it.
        writeln!(out, "  br label %{}", header_label).ok();

        // ── Guard-present block: [pre] [guard] [post] ───────────────
        writeln!(out, "{}:", present_label).ok();
        // 2026-07-31: Restore phi_field_regs to the header phis so the
        // present block's [pre] reads the loop-carried values (which dominate
        // it), not the absent body's post-[pre] registers (sibling block).
        self.fun.phi_field_regs = header_phi_regs.clone();
        self.fun.last_val_temps.clear();
        self.fun.last_val_types.clear();
        self.fun.pending_phi_backedge.clear();
        for fname in &sorted_fields {
            let init_val = self.fun.phi_field_regs.get(fname.as_str())
                .cloned().unwrap_or_else(|| "0".to_string());
            self.fun.pending_phi_backedge.insert((*fname).clone(), init_val);
        }
        self.emit_countable_body(out, &pre, write_set, &mut vec![]);
        self.emit_countable_body(out, guard_body, write_set, &mut vec![]);
        self.emit_countable_body(out, &post, write_set, &mut vec![]);
        // Present-backedges to the header. The counter increment is in
        // [pre]/[post]; the loop below emits be_present_regs[count] from
        // pending_phi_backedge — the register the header phi references
        // from the present predecessor.
        for fname in &sorted_fields {
            if let Some(be_f) = be_present_regs.get(fname.as_str()) {
                let val = self.fun.pending_phi_backedge.get(fname.as_str())
                    .cloned().unwrap_or_else(|| {
                        self.fun.phi_field_regs.get(fname.as_str())
                            .cloned().unwrap_or_else(|| "0".to_string())
                    });
                let field_ty = self.ctx.field_index_map.get(fname.as_str())
                    .and_then(|idx| self.ctx.field_types.get(*idx))
                    .cloned().unwrap_or_else(|| "i64".to_string());
                if field_ty == "float" || field_ty == "double" {
                    writeln!(out, "  {} = fadd {} 0.0, {}", be_f, field_ty, val).ok();
                } else {
                    writeln!(out, "  {} = add {} 0, {}", be_f, field_ty, val).ok();
                }
            }
        }
        writeln!(out, "  br label %{}", header_label).ok();

        // ── End: post-loop prints ───────────────────────────────────
        writeln!(out, "{}:", end_label).ok();
        // 2026-07-31: Materialize ALL written fields' final values to %State.
        // The end block is a successor of the header, so it references the
        // header phi registers (which dominate it), NOT the absent body's
        // post-[pre] registers (sibling block). The post-loop swan song reads
        // these from %State — boundary-only fields (e.g. `escapes`) must be
        // stored here or the print reads the initial value.
        self.fun.phi_field_regs = header_phi_regs;
        for fname in &sorted_fields {
            if let Some(&idx) = self.ctx.field_index_map.get(fname.as_str()) {
                let phi = self.fun.phi_field_regs.get(fname.as_str())
                    .cloned().unwrap_or_else(|| "0".to_string());
                self.emit_state_store_i64_by_idx(out, "  ", idx, &phi);
            }
        }
        let hoist = self.fun.pending_post_hoist.clone();
        if !hoist.is_empty() {
            self.fun.phi_field_regs.clear();
            self.fun.last_val_temps.clear();
            for group in &hoist {
                self.emit_countable_body(out, group, &HashSet::new(), &mut vec![]);
            }
        }
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
        true
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
        // 2026-07-20: Pre-allocated bound_reg — use hand-rolled GEP+load
        // because the centralized helper creates its own register name.
        if let Some(ti) = total_idx {
            let gep = self.fun.next_reg_with_prefix("clb");
            writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                gep, ti).ok();
            writeln!(out, "  {} = load i64, ptr {}, align 8", bound_reg, gep).ok();
        } else if let Some(tcn) = total_const_name {
            // 2026-07-17: Resolve bound from compile-time constant value first.
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
    /// 2026-07-29: SLP gating removed — proven counterproductive. LLVM's SLP
    /// vectorizer has its own cost model. See docs/plans/2026-07-29-full-recovery-plan.md §7.
    fn emit_countable_body(
        &mut self,
        out: &mut String,
        body: &[Statement],
        write_set: &HashSet<String>,
        hoisted: &mut Vec<Vec<Statement>>,
    ) {
        let mut i = 0;
        while i < body.len() {
            let stmt = &body[i];
            match stmt {
                Statement::Let { name, expr: Some(e), .. } => {
                    let reg = self.emit_expr(out, e, "  ");
                    self.fun.last_val_temps.insert(name.clone(), reg.name.clone());
                    self.fun.last_val_types.insert(name.clone(), reg.ty);
                }
                Statement::Assign(lhs, expr) => {
                    let lhs_name = Self::assign_target_name(lhs);
                     let val = self.emit_expr(out, expr, "  ");
                      // 2026-08-01 (A9b): `<-` op dispatch — when the LHS is a
                      // collection with an InsertAt op binding (`queue <- count`),
                      // emit the self-bound member call (push) instead of a scalar
                      // field backedge. The collection field is aggregate (excluded
                      // from the phis); its data write is memory-resident in %State.
                      let insert_strat = self.find_insert_strategy(lhs).cloned();
                      if let Some(op_def) = &insert_strat {
                          if !super::emit_stmt::emit_strategy_member_call(self, out, "  ", lhs, op_def, Some(&val.name)) {
                              super::emit_stmt::emit_strategy_fn_call(self, out, "  ", lhs, op_def, Some(&val.name));
                          }
                          i += 1;
                          continue;
                      }
                       if let Some(ref n) = lhs_name {
                         if write_set.contains(n) {
                              // 2026-07-29: Vector phi routing — if field belongs to
                              // a vector group, record_field_update instead of scalar backedge.
                              // Clone lookup data to avoid borrow conflicts with &mut self.fun.
                              let is_vector_grouped = self.fun.field_to_phi.contains_key(n.as_str());
                              let (groups_clone, lane_map_clone) = if is_vector_grouped {
                                  (Some(self.fun.active_vector_groups.clone()),
                                   Some(self.fun.field_to_lane.clone()))
                              } else {
                                  (None, None)
                              };
                              if let (Some(ref g), Some(ref l)) = (groups_clone, lane_map_clone) {
                                  crate::backend::llvm::vector_phi::record_field_update(
                                      &mut self.fun, n, &val.name, g, l,
                                  );
                              } else {
                                  // 2026-07-21: Float fields use native type in backend —
                                  // skip adapt_to_i64 and store the float value directly.
                                  let field_ty = self.ctx.field_index_map.get(n)
                                      .and_then(|idx| self.ctx.field_types.get(*idx))
                                      .cloned().unwrap_or_else(|| "i64".to_string());
                                  if field_ty == "float" || field_ty == "double" {
                                      self.fun.pending_phi_backedge.insert(n.clone(), val.name.clone());
                                  } else {
                                      let boxed = self.adapt_to_i64(out, "  ", &val);
                                      self.fun.pending_phi_backedge.insert(n.clone(), boxed);
                                  }
                              }
                          }
                        // 2026-07-17: When post-loop hoisted prints need final values,
                        // emit state stores for ALL fields, not just phi-tracked ones.
                        // Without this, fields outside the capped write_set (max 6)
                        // silently lose their values between iterations — the body
                        // computes the new value, but it's never stored back to %State.
                        if self.fun.needs_state_stores_in_body {
                            if let Some(&idx) = self.ctx.field_index_map.get(n) {
                                // 2026-07-20: Intentionally hand-rolled — needs adapt_to_i64
                                // fallback when val_ty != field_ty (float→i64 store).
                                // 2026-07-19: Store with native type for %State struct
                                // compatibility. Phi backedge uses i64, but the state
                                // store matches the field's LLVM type (float/double).
                                let field_ty = &self.ctx.field_types[idx];
                                let val_ty = self.llvm_type(&val.ty);
                                let gep = self.fun.next_reg_with_prefix("cms");
                                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                                    gep, idx).ok();
                                if val_ty == *field_ty {
                                    writeln!(out, "  store {} {}, ptr {}, align 8", field_ty, val.name, gep).ok();
                                } else {
                                    let boxed = self.adapt_to_i64(out, "  ", &val);
                                    writeln!(out, "  store i64 {}, ptr {}, align 8", boxed, gep).ok();
                                }
                            }
                        }
                        self.fun.last_val_temps.insert(n.clone(), val.name.clone());
                        self.fun.last_val_types.insert(n.clone(), val.ty.clone());
                    }
                    // 2026-07-21: Handle pointer-indexed stores (data[idx] = val)
                    // and deref stores (*ptr = val) inside convergence loops.
                    // Without this, emit_countable_body silently drops these
                    // assignments (assign_target_name returns None for non-Ident).
                    match lhs {
                        Expr::Index(obj, idx) => {
                            // 2026-08-01 (B): array-state field store
                            // (`f[i] = v` for Float[16]) — route through
                            // emit_array_state_store like the normal path. The
                            // countdown previously only handled Ptr-indexed
                            // stores, silently DROPPING array-state writes
                            // (the seed + the loop's f[j]=n[j] both vanished).
                            if super::emit_stmt::emit_array_state_store(self, out, "  ", obj, idx, &val) {
                                i += 1;
                                continue;
                            }
                            let obj_reg = self.emit_expr(out, obj, "  ");
                            if matches!(obj_reg.ty, Type::Ptr(_)) {
                                let idx_reg = self.emit_expr(out, idx, "  ");
                                let ptr = self.fun.gen_reg();
                                writeln!(out, "  {} = inttoptr i64 {} to ptr", ptr, obj_reg.name).ok();
                                let gep = self.fun.gen_reg();
                                let offset = self.fun.gen_reg();
                                // Only List/tuple literals have a length header at slot 0.
                                if matches!(obj.as_ref(), Expr::List(_) | Expr::Tuple(_)) {
                                    writeln!(out, "  {} = add i64 {}, 1", offset, idx_reg.name).ok();
                                } else {
                                    writeln!(out, "  {} = add i64 {}, 0", offset, idx_reg.name).ok();
                                }
                                writeln!(out, "  {} = getelementptr i64, ptr {}, i64 {}", gep, ptr, offset).ok();
                                writeln!(out, "  store i64 {}, ptr {}", val.name, gep).ok();
                            }
                        }
                        Expr::Deref(inner) => {
                            let ptr_reg = self.emit_expr(out, inner, "  ");
                            // 2026-07-30: Ptr values are stored as i64 internally;
                            // convert back to LLVM ptr before storing through.
                            let store_ptr = if matches!(ptr_reg.ty, Type::Ptr(_)) {
                                let p = self.fun.gen_reg();
                                writeln!(out, "  {} = inttoptr i64 {} to ptr", p, ptr_reg.name).ok();
                                p.to_string()
                            } else {
                                ptr_reg.name.clone()
                            };
                            writeln!(out, "  store i64 {}, ptr {}", val.name, store_ptr).ok();
                        }
                        _ => {}
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
                    self.fun.cur_block = Some(merge_label);
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
                    self.fun.cur_block = Some(next_label);
                }
                Statement::Block(stmts) => {
                    self.emit_countable_body(out, stmts, write_set, hoisted);
                }
                Statement::Expression(e) => {
                    // 2026-08-01 (A10): `<- &collection` discard — dispatch the
                    // ExtractFrom member call (self-bound pop), not just emit the
                    // address. Without it the pop never runs: a Stack's len never
                    // decrements and the next push overflows the buffer.
                    if let Expr::AddrOf(source) = e {
                        let strat = self.find_extract_strategy(source)
                            .or_else(|| self.find_extract_strategy(e)).cloned();
                        if let Some(op_def) = &strat {
                            if !super::emit_stmt::emit_strategy_member_call(self, out, "  ", source, op_def, None) {
                                super::emit_stmt::emit_strategy_fn_call(self, out, "  ", source, op_def, None);
                            }
                        }
                        let _ = self.fun.gen_reg();
                    } else {
                        self.emit_expr(out, e, "  ");
                    }
                }
                Statement::Return(Some(e)) => {
                    let val = self.emit_expr(out, e, "  ");
                    writeln!(out, "  ret i64 {}", val.name).ok();
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// Emit a single guard statement (when cond { body } or term! -> print)
    /// as a simple if-block, without phi backedge tracking or field write sets.
    /// Suitable for outer loop guards and post-loop termination prints.
    fn emit_guard_block(&mut self, out: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Guarded(cond, body) => {
                let cond_reg = self.emit_expr(out, cond, indent);
                let bool_reg = self.as_bool_reg(out, indent, &cond_reg);
                let body_label = format!(".ogb{}", self.fun.txn_counter);
                let next_label = format!(".ogn{}", self.fun.txn_counter);
                self.fun.txn_counter += 1;
                writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, bool_reg, body_label, next_label).ok();
                writeln!(out, "{}:", body_label).ok();
                for s in body {
                    self.emit_guard_body_stmt(out, s, indent);
                }
                writeln!(out, "{}br label %{}", indent, next_label).ok();
                writeln!(out, "{}:", next_label).ok();
            }
            Statement::Term(Some(e)) | Statement::Expression(e) | Statement::TermBang(Some(e)) => {
                self.emit_expr(out, e, indent);
            }
            _ => {}
        }
    }

    /// Emit a single statement inside a guard body (Let → compute, Expression → call).
    fn emit_guard_body_stmt(&mut self, out: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Let { name, expr: Some(e), .. } => {
                let reg = self.emit_expr(out, e, indent);
                self.fun.last_val_temps.insert(name.clone(), reg.name.clone());
                self.fun.last_val_types.insert(name.clone(), reg.ty);
            }
            Statement::Expression(e) => {
                self.emit_expr(out, e, indent);
            }
            Statement::Guarded(cond, body) => {
                self.emit_guard_block(out, stmt, indent);
            }
            Statement::Term(Some(e)) | Statement::TermBang(Some(e)) => {
                self.emit_expr(out, e, indent);
            }
            Statement::Assign(lhs, expr) => {
                let val = self.emit_expr(out, expr, indent);
                if let Expr::Identifier(n) = lhs {
                    self.fun.last_val_temps.insert(n.clone(), val.name.clone());
                    self.fun.last_val_types.insert(n.clone(), val.ty.clone());
                }
            }
            _ => {}
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
