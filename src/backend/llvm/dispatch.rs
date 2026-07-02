use crate::ast::{Expr, Program, Statement, TopLevel, Type};
use crate::backend::llvm::{find_perfect_hash, sparsity_ratio, FoldParam, FunctionGuard, LlvmBackend};
use std::collections::HashMap;
use std::fmt::Write;

impl LlvmBackend {
    // ── REACTOR LOOP ──────────────────────────────────────────
    //
    // Why two dispatch modes (sequential + parallel):
    //
    // Sequential is the fallback for any program where we cannot prove
    // conflict-freedom. It evaluates ALL preconditions first, then fires
    // each txn body. Ordering preconditions before bodies means that if
    // one txn fires and modifies state, subsequent txns still see the
    // pre-tick state for their pre-checks — a txn's decision to fire is
    // based on the state at tick start, not mid-tick modifications.
    //
    // Parallel adds a %fired_mask (bitmask) to track which fields have
    // been written by any previously-fired txn. Before firing, a txn
    // checks (fired_mask & write_mask) == 0 — if any of its written fields
    // were already written by a prior parallel txn, it skips. This avoids
    // the write-after-write hazard without locking.
    //
    // Both modes emit @cell_persistent_ticks(ptr %state) at the end of
    // every tick. This is a single call site for all dispatch paths that
    // ensures cell instances with persistent bodies tick once per reactor
    // cycle. See emit_toplevel.rs.
    pub(crate) fn emit_reactor(&mut self, out: &mut String, txns: &[(String, &crate::ast::Transaction)], fusable: &[(String, String)]) {
        self.fused_to_first.clear();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            self.fused_to_first.insert(fn_, a.clone());
        }
        let mut used_fused: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut dispatch: Vec<String> = Vec::new();
        let mut fused_txns: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            if used_fused.contains(&fn_) { continue; }
            used_fused.insert(fn_.clone());
            fused_txns.insert(a.clone()); fused_txns.insert(b.clone());
            dispatch.push(fn_);
        }
        for (n, t) in txns { if !fused_txns.contains(n) && t.is_reactive { dispatch.push(n.clone()); } }

        writeln!(out, "define void @reactor_tick(ptr noalias nocapture %state) local_unnamed_addr #2 {{").ok();
        writeln!(out, "  entry:").ok();
        // 2026-06-27: Clear ssa_old regs at reactor_tick entry — they may
        // contain stale entries from the main function emit (e.g., from
        // emit_ssa_main's per-field phi setup or emit_stmt's ssa_old reg
        // update). Without this, inline txn body identifier lookups find
        // stale register names not defined in this function.
        self.fun.ssa_old_int_regs.clear();
        self.fun.ssa_old_float_regs.clear();
        // Arena init: shared arena for all txns in this tick.
        // Previously each @txn_name had its own 64KB arena (Approach 2),
        // but inlining shares one arena across all txns, saving memory.
        self.emit_arena_init(out, "  ");
        self.sampled_triggers.clear();
        let trigger_snapshot: Vec<(String, crate::ast::TriggerDeclaration)> = self.ctx.trigger_names
            .iter()
            .filter_map(|tn| self.ctx.triggers.get(tn).map(|t| (tn.clone(), t.clone())))
            .collect();
        for (tn, t) in &trigger_snapshot {
            let sz = format!("%sz_{}", tn);
            self.emit_trg_load(out, "  ", &sz, &t.address, &t.ty);
            self.sampled_triggers.insert(tn.clone(), sz);
        }

        if dispatch.is_empty() {
            self.emit_arena_fini(out, "  ");
            writeln!(out, "  call void @cell_persistent_ticks(ptr %state)").ok();
            writeln!(out, "  ret void").ok();
        } else if fusable.is_empty()
            && dispatch.len() >= 2
            && crate::analysis::transition_graph::is_uniform_body_group(txns)
        {
            self.emit_inline_txn_body(out, "  ", txns, &dispatch[0]);
            writeln!(out, "  call void @cell_persistent_ticks(ptr %state)").ok();
            self.emit_arena_fini(out, "  ");
            writeln!(out, "  ret void").ok();
        } else {
            let mut pre_regs: Vec<String> = Vec::with_capacity(dispatch.len());
            for (i, txn_name) in dispatch.iter().enumerate() {
                let has_pre = self.dispatch_has_pre(txns, txn_name);
                if has_pre {
                    let reg = format!("%pr{}", i);
                    let txn = self.resolve_dispatch_first_txn(txn_name);
                    writeln!(out, "  {} = call i1 @pre_{}(ptr %state)", reg, txn).ok();
                    pre_regs.push(reg);
                } else {
                    pre_regs.push("true".to_string());
                }
            }

            for (i, txn_name) in dispatch.iter().enumerate() {
                let b = format!("b{}", i);
                let c = format!("ck{}", i);
                let pr = &pre_regs[i];
                writeln!(out, "  br i1 {}, label %{}, label %{}", pr, b, c).ok();
                writeln!(out, "{}:", b).ok();
                // Inline txn body instead of `call @txn_name` — shares the
                // arena across all txns in the tick and avoids function-call
                // overhead. The body inherits the arena allocated above.
                self.emit_inline_txn_body(out, "  ", txns, txn_name);
                writeln!(out, "  br label %{}", c).ok();
                writeln!(out, "{}:", c).ok();
            }
            // Tick persistent cell! instances every reactor cycle.
            // The @cell_persistent_ticks function is emitted unconditionally
            // in generate() (see emit_toplevel.rs). Single tick point for
            // all dispatch paths that use @reactor_tick.
            self.emit_arena_fini(out, "  ");
            writeln!(out, "  call void @cell_persistent_ticks(ptr %state)").ok();
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ── RANGE EXTRACTION ──────────────────────────────────────
    //
    // Extracts [lo, hi) range bounds from precondition expressions for
    // !range metadata annotation. Only handles simple patterns:
    //   And(Lt(x, N), Ge(x, 0))  →  x: (0, N)
    //   Gt(x, M)                 →  x: (M+1, MAX)
    //
    // Why this lives in dispatch.rs instead of emit_toplevel.rs with the
    // !range emission: the range extraction happens during the same pass
    // that builds the dispatch mask, and both share the field_index_map.
    // The ranges are consumed by emit_precondition_check in emit_toplevel.rs.
    pub(crate) fn extract_ranges(pre: &Expr) -> HashMap<String, (i64, i64)> {
        let mut r = HashMap::new();
        Self::extract_ranges_inner(pre, &mut r);
        r
    }
    pub(crate) fn extract_ranges_inner(expr: &Expr, r: &mut HashMap<String, (i64, i64)>) {
        match expr {
            Expr::And(l, rgt) => { Self::extract_ranges_inner(l, r); Self::extract_ranges_inner(rgt, r); }
            Expr::Lt(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if *v < e.1 { e.1 = *v; } } } }
            Expr::Ge(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if *v > e.0 { e.0 = *v; } } } }
            Expr::Gt(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if v + 1 > e.0 { e.0 = v + 1; } } } }
            _ => {}
        }
    }

    // ── DISPATCH HELPERS ─────────────────────────────────────
    pub(crate) fn resolve_dispatch_first_txn(&self, name: &str) -> String {
        self.fused_to_first.get(name).cloned().unwrap_or_else(|| name.to_string())
    }
    pub(crate) fn dispatch_has_pre(&self, txns: &[(String, &crate::ast::Transaction)], name: &str) -> bool {
        let first = self.resolve_dispatch_first_txn(name);
        txns.iter().find(|(n, _)| n == &first).map(|(_, t)| !matches!(t.contract.pre_condition, Expr::Bool(true))).unwrap_or(false)
    }

    // ── WRITE MASKS (Parallel Dispatch) ──────────────────────
    //
    // Precomputes a 64-bit bitmask per transaction where bit N is set if
    // the txn writes to field at index N in field_index_map.
    //
    // Why 64 bits: field_index_map is partitioned so state fields get the
    // lowest indices (0..N). Cache slots and internal variables get higher
    // indices. A u64 mask is cheap to and/or/test and saturates any practical
    // state size. If >64 fields are needed, this should switch to u128.
    pub(crate) fn build_write_masks(&mut self, program: &Program) {
        self.txn_write_masks.clear();
        for item in &program.items {
            if let TopLevel::Transaction(t) = item {
                let writes = crate::backend::collect_assigned_identifiers(&t.body);
                let mut mask = 0u64;
                for w in &writes {
                    if let Some(&idx) = self.ctx.field_index_map.get(w.as_str()) {
                        if idx < 64 { mask |= 1u64 << idx; }
                    }
                }
                self.txn_write_masks.insert(t.name.clone(), mask);
            }
        }
    }

    // ── PARALLEL DISPATCH REACTOR ────────────────────────────
    //
    // Emits a reactor_tick that fires conflict-free transactions in parallel.
    // Unlike the sequential reactor (which chains every txn unconditionally),
    // this version tracks which fields have been written via a %fired_mask.
    //
    // Why a mask instead of locks or transactions: the compiler has complete
    // knowledge of every transaction's write set at compile time. A simple
    // bitmask check (load-and-test) is cheaper than any lock acquire or
    // transactional memory instruction. The mask is only 8 bytes on the stack
    // and the check is 3 ALU ops per txn.
    pub(crate) fn emit_parallel_reactor(&mut self, out: &mut String, txns: &[(String, &crate::ast::Transaction)],
                             fusable: &[(String, String)]) {
        self.fused_to_first.clear();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            self.fused_to_first.insert(fn_, a.clone());
        }
        let mut used_fused: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut dispatch: Vec<String> = Vec::new();
        let mut fused_txns: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            if used_fused.contains(&fn_) { continue; }
            used_fused.insert(fn_.clone());
            fused_txns.insert(a.clone()); fused_txns.insert(b.clone());
            dispatch.push(fn_);
        }
        for (n, t) in txns { if !fused_txns.contains(n) && t.is_reactive { dispatch.push(n.clone()); } }

        writeln!(out, "define void @reactor_tick(ptr noalias nocapture %state) local_unnamed_addr #2 {{").ok();
        writeln!(out, "  entry:").ok();
        // 2026-07-01: When the thread pool is active (async txns), the worker
        // threads execute the bodies in parallel on the correct state snapshot.
        // The main thread's reactor_tick must be a no-op — the workers handle
        // everything, synchronized via __barrier_release__/__barrier_wait__.
        if self.has_async_txns && !self.is_lightweight_async {
            writeln!(out, "  ret void").ok();
            writeln!(out, "}}").ok();
            writeln!(out).ok();
            return;
        }
        // 2026-06-27: Clear ssa_old regs at reactor_tick entry (same
        // rationale as the sequential reactor counterpart).
        self.fun.ssa_old_int_regs.clear();
        self.fun.ssa_old_float_regs.clear();
        // Arena init for parallel reactor — shared across all parallel txns.
        self.emit_arena_init(out, "  ");
        self.sampled_triggers.clear();
        let trigger_snapshot: Vec<(String, crate::ast::TriggerDeclaration)> = self.ctx.trigger_names
            .iter()
            .filter_map(|tn| self.ctx.triggers.get(tn).map(|t| (tn.clone(), t.clone())))
            .collect();
        for (tn, t) in &trigger_snapshot {
            let sz = format!("%sz_{}", tn);
            self.emit_trg_load(out, "  ", &sz, &t.address, &t.ty);
            self.sampled_triggers.insert(tn.clone(), sz);
        }

        writeln!(out, "  %fired_mask = alloca i64, align 8").ok();
        writeln!(out, "  store i64 0, i64* %fired_mask").ok();

        if dispatch.is_empty() {
            self.emit_arena_fini(out, "  ");
            writeln!(out, "  ret void").ok();
        } else {
            let n = dispatch.len();
            for (i, txn_name) in dispatch.iter().enumerate() {
                let has_pre = self.dispatch_has_pre(txns, txn_name);
                if has_pre {
                    let first_txn = self.resolve_dispatch_first_txn(txn_name);
                    writeln!(out, "  %pr{} = call i1 @pre_{}(ptr %state)", i, first_txn).ok();
                } else {
                    writeln!(out, "  %pr{} = add i1 0, 1", i).ok();
                }
            }

            for i in 0..n {
                let txn_name = &dispatch[i];
                let b = format!("b{}", i);
                let next_c = format!("ck{}", i + 1);

                if i == 0 {
                    writeln!(out, "  br i1 %pr0, label %b0, label %ck1").ok();
                } else {
                    let c = format!("ck{}", i);
                    writeln!(out, "{}:", c).ok();
                    let wm = self.txn_write_masks.get(txn_name).copied().unwrap_or(0);
                    if wm == 0 {
                        writeln!(out, "  br i1 %pr{}, label %{}, label %{}", i, b, next_c).ok();
                    } else {
                        let fm = format!("%fm{}", i);
                        let ca = format!("%ca{}", i);
                        let nc = format!("%nc{}", i);
                        writeln!(out, "  {} = load i64, i64* %fired_mask", fm).ok();
                        writeln!(out, "  {} = and i64 {}, {}", ca, fm, wm).ok();
                        writeln!(out, "  {} = icmp eq i64 {}, 0", nc, ca).ok();
                        writeln!(out, "  %can{} = and i1 %pr{}, {}", i, i, nc).ok();
                        writeln!(out, "  br i1 %can{}, label %{}, label %{}", i, b, next_c).ok();
                    }
                }
            }

            for i in 0..n {
                let txn_name = &dispatch[i];
                let b = format!("b{}", i);
                let next_c = format!("ck{}", i + 1);
                let wm = self.txn_write_masks.get(txn_name).copied().unwrap_or(0);
                writeln!(out, "{}:", b).ok();
                // Inline txn body — shares arena across parallel txns.
                self.emit_inline_txn_body(out, "  ", txns, txn_name);
                if wm != 0 {
                    let fm = format!("%fm{}a", i);
                    let fmu = format!("%fm{}b", i);
                    writeln!(out, "  {} = load i64, i64* %fired_mask", fm).ok();
                    writeln!(out, "  {} = or i64 {}, {}", fmu, fm, wm).ok();
                    writeln!(out, "  store i64 {}, i64* %fired_mask", fmu).ok();
                }
                writeln!(out, "  br label %{}", next_c).ok();
            }

            writeln!(out, "ck{}:", n).ok();
            self.emit_arena_fini(out, "  ");
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ── INLINE TXN BODY HELPER ──────────────────────────────────
    //
    // Emits a txn body inline within the reactor_tick function instead of
    // calling it as a separate @txn_name function. Uses FunctionGuard RAII
    // to save/restore all FunctionContext fields, preventing cross-txn
    // state contamination without manual per-field clone/restore.
    //
    // 2026-06-29: Replaced manual 7-field save/restore with FunctionGuard.
    // The guard clones the entire FunctionContext at scope entry and restores
    // it on drop. This ensures new FunctionContext fields are automatically
    // protected without editing this function.
    fn emit_inline_txn_body(&mut self, out: &mut String, indent: &str,
                             txns: &[(String, &crate::ast::Transaction)],
                             txn_name: &str) {
        let first_name = self.resolve_dispatch_first_txn(txn_name);
        if let Some((_, txn)) = txns.iter().find(|(n, _)| n == &first_name) {
            // 2026-06-29: FunctionGuard snapshots ALL FunctionContext state;
            // restore() at the end puts it back. No more forgetting fields.
            let guard = FunctionGuard::new(&self.fun);

            self.fun.terminated = false;
            self.fun.returns_i64 = false;

            // Emit precondition assume (for LLVM opt) — the br instruction
            // already guards execution, so this is just for metadata.
            if !matches!(txn.contract.pre_condition, crate::ast::Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, indent);
            }
            for s in &txn.body {
                if self.fun.terminated { break; }
                self.emit_stmt(out, s, indent);
            }

            // 2026-07-01: Use restore_preserve_counters — SSA register counters
            // (txn_counter, arena_counter) must stay monotonically increasing
            // across inlined bodies to prevent duplicate %t{N}/%dab{N}/%aa{N}
            // registers. Full restore() would rewind counters.
            guard.restore_preserve_counters(&mut self.fun);
        }
    }

    // ── EXIT CONDITION EXPRESSION ────────────────────────────
    //
    // Validates that every identifier in a #!exit condition references an
    // existing state field, constant, or trigger. This prevents silent
    // reference-to-nowhere bugs where the exit condition always evaluates
    // to false because the field name was mistyped.
    //
    // Why this lives in dispatch.rs: the exit condition is evaluated in the
    // same pass as the main dispatch decision — both need the same
    // field_index_map and trigger_names visibility. Separating it would
    // require duplicating the lookup tables.
    pub(crate) fn check_exit_condition_idents(&self, expr: &Expr) -> Vec<String> {
        let mut errors = Vec::new();
        self.check_exit_condition_idents_inner(expr, &mut errors);
        errors
    }

    pub(crate) fn check_exit_condition_idents_inner(&self, expr: &Expr, errors: &mut Vec<String>) {
        match expr {
            Expr::Identifier(name) => {
                if !self.ctx.field_index_map.contains_key(name)
                    && !self.ctx.constants.contains_key(name)
                    && !self.ctx.trigger_names.contains(name)
                {
                    errors.push(format!(
                        "error: #!exit references unknown variable '{}'\n  note: '{}' is not a state field, constant, or trigger",
                        name, name
                    ));
                }
            }
            Expr::BinaryOp(bop) => {
                self.check_exit_condition_idents_inner(&bop.left, errors);
                self.check_exit_condition_idents_inner(&bop.right, errors);
            }
            Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
            | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
                self.check_exit_condition_idents_inner(l, errors);
                self.check_exit_condition_idents_inner(r, errors);
            }
            Expr::Not(e) => self.check_exit_condition_idents_inner(e, errors),
            _ => {}
        }
    }
}
