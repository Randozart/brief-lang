// ── Loop emission architecture overview ──────────────────────────────────
//
// There are four loop emission strategies, chosen by the frontend:
//
// 1. PURE COUNTER FOLD (emit_folded_pure_counter):
//    For pure bodies with a compile-time constant bound. O(1) — single
//    store instruction. No runtime loop emitted.
//
// 2. PURE COUNTER PHI (emit_folded_loop, use_phi=true):
//    For pure bodies with a runtime-variable bound. Counter-only phi
//    node, no body emission (body was precomputed).
//
// 3. HYBRID COUNTER-PHI + MEMORY FIELDS (emit_countable_main, A005e):
//    For all non-pure foldable single-txn programs. Creates only a
//    single counter phi (induction variable for LLVM's trip count
//    analysis).  State fields are loaded from %State at body entry
//    (pre_load_all_fields) and stored back in the body.  LLVM's SROA
//    converts the GEP+load+store pattern to closed-SSA phis, avoiding
//    the phi-escape problem that blocks the vectorizer.
//
// 4. SSA REGISTER PIPELINE (emit_ssa_main):
//    For multi-txn reactive programs (rct txn). Precondition checked
//    per-iteration; body runs inline with per-field GEP loads/stores.
//    Supports canonical loop detection for phi induction variable
//    optimization.
//
// Why per-field phis as default: LLVM needs a canonical loop structure
// (phi + icmp slt + add) to apply induction variable analysis, SROA,
// and loop vectorization. The per-field phi loop provides this, while
// the old A005a path used a %State alloca round-trip and A005b kept
// the counter in memory — both hiding the loop structure from LLVM.
use crate::ast::{Expr, Intrinsic, Statement, Type};
use crate::backend::llvm::emit_stmt::MAX_FIELDS_PER_ALLLOCA;
use crate::backend::llvm::{float_to_llvm_hex, find_perfect_hash, sparsity_ratio, FoldParam, LlvmBackend};
use crate::analysis::dependency_graph::DependencyGraph;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write;

impl LlvmBackend {
    /// Recursively evaluate a boolean expression for the exit condition check.
    /// All values are emitted as `i64` for uniformity; comparisons are zext'd from `i1`.
    pub(crate) fn emit_exit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> String {

        let v = format!("%t{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        match expr {
            Expr::Integer(n) => { return self.emit_expr(out, expr, indent).name; }
            // Expr::Float is handled separately: emit the bit pattern as i64
            // so comparison operators (icmp) see i64 on both sides. This avoids
            // the invalid `bitcast float to i64` and type mismatch with icmp.
            Expr::Float(f) => {
                let bits = f.to_bits() as i64;
                writeln!(out, "{}{} = add i64 0, {}", indent, v, bits).ok();
                return v;
            }
            Expr::Bool(_) | Expr::Neg(_) | Expr::String(_) => {
                return self.emit_expr(out, expr, indent).name;
            }
            Expr::Char(c) => {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, *c as i32).ok();
                return v;
            }
            Expr::Literal(lit) => {
                match lit.as_ref() {
                    crate::features::literal::LiteralExpr::Char(c) => {
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, *c as i32).ok();
                        return v;
                    }
                    crate::features::literal::LiteralExpr::Float(f) => {
                        let bits = f.to_bits() as i64;
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, bits).ok();
                        return v;
                    }
                    _ => return self.emit_expr(out, expr, indent).name,
                }
            }
            Expr::Identifier(name) => {
                if let Some(&idx) = self.ctx.field_index_map.get(name) {
                    let p = self.emit_state_gep(out, indent, "gep_exit", "%state", idx);
                    let ft = &self.ctx.field_types[idx];
                    match ft.as_str() {
                        "i64" => { writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, v, p).ok(); }
                        "i32" => {
                            let l = format!("%exit_l{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i32, ptr {}, align 4", indent, l, p).ok();
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, l).ok();
                        }
                        "i8" => {
                            let l = format!("%exit_l{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, l, p).ok();
                            writeln!(out, "{}{} = zext i8 {} to i64", indent, v, l).ok();
                        }
                        s if s == "i8*" || s == "ptr" => {
                            let l = format!("%exit_l{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load ptr, ptr {}, align 8", indent, l, p).ok();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, l).ok();
                        }
                        // 2026-06-26: float -> i32 bitcast (same size), then
                        // zext to i64 so comparison operators see i64 on both sides.
                        // The comparison uses icmp on the integer bit pattern —
                        // correct for equality, but ordering of negative floats
                        // compares inverted vs mathematical (high bit is sign).
                        // TODO: emit fcmp for float-typed exit comparisons.
                        "float" => {
                            let l = format!("%exit_l{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            let i = format!("%exit_i{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load float, ptr {}, align 4", indent, l, p).ok();
                            writeln!(out, "{}{} = bitcast float {} to i32", indent, i, l).ok();
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok();
                        }
                        _ => {
                            panic!("emit_exit_expr: unknown field type '{}' for field '{}' in #!exit expression", ft, name);
                        }
                    }
                } else if self.ctx.constants.contains_key(name) {
                    writeln!(out, "{}{} = load i64, ptr @{}, align 8", indent, v, name).ok();
                } else if self.ctx.trigger_names.contains(name) {
                    if let Some(t) = self.ctx.triggers.get(name).cloned() {
                        self.emit_trg_load(out, indent, &v, &t.address, &t.ty);
                    } else {
                        panic!("emit_exit_expr: trigger '{}' registered in trigger_names but missing from triggers map", name);
                    }
                } else {
                    panic!("emit_exit_expr: identifier '{}' in #!exit is not a state field, constant, or trigger", name);
                }
                v
            }
            expr @ Expr::AddrOf(_) => { let name = expr.as_var_name().unwrap().to_string();
                return self.emit_exit_expr(out, &Expr::Identifier(name.clone()), indent);
            }
            Expr::Eq(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Ne(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Lt(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Le(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Gt(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Ge(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
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
            Expr::BinaryOp(bop) => {
                let lv = self.emit_exit_expr(out, &bop.left, indent);
                let rv = self.emit_exit_expr(out, &bop.right, indent);
                use crate::features::binary_op::BinaryOpKind;
                let cmp = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                match bop.kind {
                    BinaryOpKind::Eq => writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, lv, rv),
                    BinaryOpKind::Ne => writeln!(out, "{}{} = icmp ne i64 {}, {}", indent, cmp, lv, rv),
                    BinaryOpKind::Lt => writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cmp, lv, rv),
                    BinaryOpKind::Le => writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, cmp, lv, rv),
                    BinaryOpKind::Gt => writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, cmp, lv, rv),
                    BinaryOpKind::Ge => writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, cmp, lv, rv),
                    BinaryOpKind::And => writeln!(out, "{}{} = and i64 {}, {}", indent, cmp, lv, rv),
                    BinaryOpKind::Or  => writeln!(out, "{}{} = or i64 {}, {}", indent, cmp, lv, rv),
                    _ => panic!("emit_exit_expr: unsupported BinaryOp kind {:?} in #!exit", bop.kind),
                }.ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            _ => {
                panic!("emit_exit_expr: unsupported expression type in #!exit: {:?}", expr);
            }
        }
    }

    // ── MAIN FUNCTION ─────────────────────────────────────────
    pub(crate) fn emit_main(&mut self, out: &mut String, has_wake_triggers: bool) {
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#3")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        // ── %state_copy: Persistent stack slot for the memcpy SROA round-trip ──
        //
        // Why entry block instead of tick body (the critical fix):
        // LLVM's codegen lowers alloca instructions in non-entry blocks as
        // dynamic stack allocations — emitting `sub rsp, N` at the instruction
        // site rather than in the function prologue. When %state_copy was
        // emitted inside the `tick:` block, each iteration lowered rsp by
        // sizeof(%State) ≈ 32 bytes with no matching restore on the backedge.
        // After 2^18 ticks (262144), 32 × 262144 = 8 MiB = ulimit -s, the
        // stack hit the guard page, and the next function-call push caused
        // SIGSEGV with si_addr = rsp - 8.
        //
        // Moving the alloca to the entry block (a canonical, once-per-call
        // allocation) eliminates the stack leak entirely. LLVM's codegen
        // collects all entry-block allocas into a single `sub rsp, total`
        // in the function prologue — executed exactly once.
        //
        // Trade-off note on SROA:
        // The intent of the local copy pattern is to help LLVM's SROA pass
        // scalarize %State field accesses into SSA phi nodes. The enabling
        // mechanism is the memcpy round-trip (copy-in → operate → copy-out)
        // which stays in the tick loop. The alloca's position (entry vs loop)
        // is irrelevant to SROA — entry-block allocas are actually the form
        // SROA is designed for and canonicalizes toward. No regression risk.
        //
        // Why not eliminate %state_copy entirely for async?
        // In the async path, reactor_tick is a no-op and workers operate on
        // %state_copy via g_async_state. Using a single %state buffer would
        // work, but the memcpy round-trip is kept for consistency across the
        // sync and async paths, and to preserve the SROA optimization for
        // the sync path where reactor_tick is real. Eliminating it for only
        // one path would risk divergent codegen bugs.
        // 2026-07-01: Root cause of the ~256K-tick async_counters segfault.
        writeln!(out, "  %state_copy = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        if self.has_async_txns && !self.is_lightweight_async {
            let count = self.async_txn_names.len() as i32;
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x ptr]* @thread_pool_fns to ptr", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @__thread_pool_init__(i32 {}, ptr %tp_fn_ptr)", count).ok();
        }
        // 2026-06-26: Removed setvbuf(stdout, NULL, _IOLBF, 0). The
        // _IOLBF (line-buffered) mode makes fputc ~2.1× slower on glibc
        // compared to the default fully-buffered mode (automatic for
        // non-TTY). Glibc's default auto-selects fully-buffered for pipes
        // and line-buffered for TTYs — matching C program behavior.
        // If interactive flushing on \n is needed, users can call:
        //   frgn setvbuf(...);
        //   setvbuf(stdout, NULL, 1, 0);  // _IOLBF = 1
        // in their program.
        // Spawn persistent cell threads
        for name in &self.cell_thread_names {
            let cell_state_type = format!("%CellState.{}", name);
            writeln!(out, "  %cell_state_{} = alloca {}, align 8", name, cell_state_type).ok();
            writeln!(out, "  %ct_{} = alloca i64, align 8", name).ok();
            writeln!(out, "  call i32 @pthread_create(ptr %ct_{}, ptr null, ptr @cell_thread_{}, ptr %cell_state_{})", name, name, name).ok();
        }
        writeln!(out, "  br label %tick").ok();
        // ── Tick loop: memcpy round-trip for SROA ───────────────────────────
        //
        // The memcpy round-trip (copy-in → operate → copy-out) is the actual
        // mechanism that enables LLVM's SROA pass. When SROA sees:
        //   %state_copy = alloca %State          (in entry block — allocated once)
        //   memcpy(%state_copy, %state)           (in loop — copy in)
        //   @reactor_tick(%state_copy)            (operate on local copy)
        //   memcpy(%state, %state_copy)           (in loop — copy out)
        // It recognizes that %state_copy's fields are written before any read
        // inside each iteration, so it can promote them to phi nodes whose
        // initial values come from the copy-in and whose backedge values come
        // from the copy-out. LLVM inlines the memcpy calls at -O2/-O3.
        //
        // CRITICAL: The %state_copy ALLOCA MUST be in the ENTRY block, NOT in
        // the tick body. LLVM codegen lowers non-entry allocas as dynamic
        // stack allocations (`sub rsp, N` at the instruction site), causing
        // the stack to grow by 32 bytes per tick with no matching restore.
        // The alloca was moved to the entry block in 2026-07-01 to fix the
        // ~262K-tick stack overflow (32 × 262144 = 8 MiB = ulimit -s).
        let state_size = self.compute_state_size_bytes();
        writeln!(out, "  tick:").ok();
        writeln!(out, "  call void @llvm.memcpy.p0.p0.i64(ptr %state_copy, ptr %state, i64 {}, i1 false)", state_size).ok();
        // Increment cycle_count on every tick
        emit_cycle_count_increment(self, out);
        if self.has_async_txns && !self.is_lightweight_async {
            // 2026-06-27: Pass %state_copy so reactor_tick modifies the local
            // copy, and the subsequent memcpy carries post-tick values forward.
            self.emit_async_phase(out, "%state_copy");
        } else {
            writeln!(out, "  call void @reactor_tick(ptr noalias nocapture %state_copy)").ok();
        }
        // Memcpy round-trip epilogue: copy state back from local alloca
        writeln!(out, "  call void @llvm.memcpy.p0.p0.i64(ptr %state, ptr %state_copy, i64 {}, i1 false)", state_size).ok();
        let has_exit = self.ctx.exit_condition.is_some();
        if has_exit {
            let cond = self.ctx.exit_condition.clone().unwrap();
            let val = self.emit_exit_expr(out, &cond, "  ");
            let tr = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
            if has_wake_triggers {
                let md_idx = super::emit_loop_metadata_nodes(&mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
                writeln!(out, "  br i1 {}, label %done, label %wait", tr).ok();
                writeln!(out, "  wait:").ok();
                emit_trg_event_epoll_wait(self, out);
                writeln!(out, "  br label %tick, !llvm.loop !{}", md_idx).ok();
            } else {
                let md_idx = super::emit_loop_metadata_nodes(&mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
                writeln!(out, "  br i1 {}, label %done, label %tick, !llvm.loop !{}", tr, md_idx).ok();
            }
            writeln!(out, "  done:").ok();
            // Join persistent cell threads before exit
            for name in &self.cell_thread_names {
                writeln!(out, "  %ctv_{} = load i64, ptr %ct_{}, align 8", name, name).ok();
                writeln!(out, "  call i32 @pthread_join(i64 %ctv_{}, ptr null)", name).ok();
            }
            writeln!(out, "  ret i32 0").ok();
        } else {
            if has_wake_triggers {
                emit_trg_event_epoll_wait(self, out);
            }
            super::emit_loop_metadata(out, "  ", "tick", &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
        }
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Pre-extract all float fields from the current SSA state register
    /// into named old-value registers. Body statements that read float
    /// fields will use these old-value registers, making all float
    /// operations within the iteration independent — LLVM's scheduler can
    /// then fill all CPU float execution ports simultaneously.
    ///
    /// Why this exists: without pre-extraction, each body statement that
    /// reads a float field emits its own extractvalue from the %State phi,
    /// serializing all float operations. By extracting ALL float fields
    /// once at the top, every float arithmetic instruction in the body
    /// reads from the same SSA source — the old-value register. LLVM's
    /// scheduler then sees independent operations and can fill all CPU
    /// float execution ports (2-4 per cycle on modern x86).
    ///
    /// Rejected alternative: GEP + load from memory loses SSA register
    /// provenance, preventing LLVM's register allocator from keeping hot
    /// floats in XMM registers.
    pub(crate) fn pre_extract_float_fields(&mut self, out: &mut String) {
        let ssa_reg = match self.fun.ssa_state_reg.clone() {
            Some(r) => r,
            None => return,
        };
        self.fun.ssa_old_float_regs.clear();
        for (field_name, &field_idx) in &self.ctx.field_index_map {
            // 2026-06-29: Also check for "double" (Float64) fields
            let ll_ty = &self.ctx.field_types[field_idx];
            if ll_ty == "float" || ll_ty == "double" {
                let old_reg = format!("%{}_old_{}", field_name, self.fun.txn_counter);
                self.fun.txn_counter += 1;
                writeln!(out, "  {} = extractvalue %State {}, {}", old_reg, ssa_reg, field_idx).ok();
                self.fun.ssa_old_float_regs.insert(field_name.clone(), old_reg);
            }
        }
    }

    /// Pre-extract all non-Float state fields into SSA registers before the body.
    /// Mirrors `pre_extract_float_fields` for Int fields. This eliminates the
    /// per-reference extractvalue-from-insertvalue-chain pattern that inflates
    /// the SSA body by ~5× for Int-heavy benchmarks.
    ///
    /// Why separate loops (float vs !float) instead of one loop: keeps hot
    /// float fields together in cache when iterating field_index_map.
    pub(crate) fn pre_extract_int_fields(&mut self, out: &mut String) {
        let ssa_reg = match self.fun.ssa_state_reg.clone() {
            Some(r) => r,
            None => return,
        };
        self.fun.ssa_old_int_regs.clear();
        for (field_name, &field_idx) in &self.ctx.field_index_map {
            // 2026-06-29: Skip both "float" and "double" — they're extracted in float loop
            let ll_ty = &self.ctx.field_types[field_idx];
            if ll_ty != "float" && ll_ty != "double" {
                let old_reg = format!("%{}_old_{}", field_name, self.fun.txn_counter);
                self.fun.txn_counter += 1;
                writeln!(out, "  {} = extractvalue %State {}, {}", old_reg, ssa_reg, field_idx).ok();
                self.fun.ssa_old_int_regs.insert(field_name.clone(), old_reg);
            }
        }
    }

    /// Load all state fields into old-value registers via GEP loads.
    /// Used by emit_ssa_main when ssa_state_reg is None (per-field GEP mode).
    /// Mirrors pre_extract_float/int_fields but loads from memory instead of
    /// extractvalue from the SSA %State register.
    ///
    /// Why this exists (memory mode alternative): when the program has
    /// branching control flow (Guarded with non-linear guards), using a
    /// single %State SSA register causes phi dominance failures. Instead
    /// of building a complex phi web, we load each field independently
    /// from memory via GEP. Each field is its own SSA value, and writes
    /// go directly through GEP+store — no phi needed.
    ///
    /// Why TBAA is needed on every load: all fields are in one %State
    /// struct but have different logical types. Without TBAA, LLVM sees
    /// all GEP loads as MayAlias (same struct), preventing GVN and ILP.
    /// With TBAA, a Float field load never aliases an Int field store.
    fn pre_load_all_fields(&mut self, out: &mut String, state_ptr: &str, filter: Option<&HashSet<String>>) {
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        let mut field_entries: Vec<(String, usize)> = self.ctx.field_index_map.iter()
            .map(|(n, &i)| (n.clone(), i)).collect();
        // 2026-07-06: Sort by field name for deterministic pre-load order.
        field_entries.sort_by_key(|(n, _)| n.clone());
        for (field_name, field_idx) in &field_entries {
            // 2026-07-04: When filter is Some, only load fields in the set.
            if let Some(f) = filter {
                if !f.contains(field_name) { continue; }
            }
            let ty_str = self.ctx.field_types[*field_idx].clone();
            let gep = self.emit_state_gep(out, "  ", "gep", state_ptr, *field_idx);
            let old_reg = format!("%{}_old_{}", field_name, self.fun.txn_counter);
            self.fun.txn_counter += 1;
            let tn = crate::backend::llvm::tbaa_node(&ty_str, self.ctx.type_universe.as_ref());
            writeln!(out, "  {} = load {}, ptr {}, align {}, !tbaa !{}", old_reg, ty_str, gep, self.align_of(&ty_str), tn).ok();
            // 2026-06-29: Track both "float" (Float) and "double" (Float64) as float regs
            if ty_str == "float" || ty_str == "double" {
                self.fun.ssa_old_float_regs.insert(field_name.clone(), old_reg);
            } else {
                self.fun.ssa_old_int_regs.insert(field_name.clone(), old_reg);
            }
        }
    }

    /// Emit the folded while-loop body (without `@init_state()` or the enclosing
    /// `define` / `ret`).  Used by both `emit_folded_main` and the enum dispatch path.
    ///
    /// Two usage modes:
    ///
    /// 1. use_phi=true (A005a — pure counter-only):
    ///    Counter lives in an SSA phi node (register), not in %state memory.
    ///    Zero memory traffic per iteration. The phi counts down (remaining
    ///    iterations -> 0) so the cmp is `icmp sgt %i, 0` — sub sets ZF,
    ///    eliminating the cmp instruction. Final counter stored to %state
    ///    once after the loop exit. Only valid for pure counter-only bodies.
    ///
    /// 2. use_phi=false with body=Some(stmts) (A005b — inline body SSA):
    ///    Body emitted inline with struct-SSA: load %State from alloca via
    ///    phi at header, extractvalue for reads, insertvalue chains for
    ///    writes, store %State back at iteration end.
    ///
    /// 3. use_phi=false with body=None (legacy call path):
    ///    Calls txn function via call void @txn_name(ptr %state).
    ///
    /// Why label_prefix parameter: enum dispatch calls emit_folded_loop
    /// multiple times with different prefixes (one per switch case arm).
    /// Without per-call prefix, label names like "_hdr" collide.
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
        // 2026-06-20: Use a dedicated counter (c_once) for all register names in this function
        // call, incremented once. Previously used self.fun.txn_counter (c0) which could collide
        // across multiple calls when the same label_prefix was used with different txn_counter
        // values, producing e.g. %gt132_0 (call 1) and %lt132_132 (call 2) where the latter
        // references an undefined %gt132_132. Using a locally-incrementing counter guarantees
        // uniqueness within each label_prefix scope.
        let c_once = self.fun.txn_counter;
        self.fun.txn_counter += 1;
        if use_phi {
            // Why phi instead of GEP load+store: the counter lives in an SSA
            // phi node (register), not in %state memory. Zero memory traffic
            // per iteration. LLVM sees a canonical induction variable and can
            // apply IV widening, strength reduction, LCSSA.
            //
            // Why counted-down: remaining = bound - initial; count down to 0.
            // The `sub` instruction sets ZF, so `icmp sgt %i, 0` reads flags
            // — eliminating the cmp instruction entirely. Clang emits the same
            // pattern for C for-loops.
            //
            // Why both phi body and post-loop alloca store are needed: the phi
            // tracks remaining iteration count. After loop exit, the final
            // counter value (the bound) is stored to %state via a single store.
            // Without this, %state.counter would contain its initial value.
            let entry_label = format!("{}_phi_entry", label_prefix);
            let hdr_label = format!("{}_hdr", label_prefix);
            let body_label = format!("{}_body", label_prefix);
            let done_label = format!("{}_done", label_prefix);
            writeln!(out, "{}:", entry_label).ok();
            // Load bound once
            if let Some(ti) = total_idx {
                writeln!(out, "  %gt_{}_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", label_prefix, c_once, ti).ok();
                writeln!(out, "  %lt_{}_{} = load i64, ptr %gt_{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt_{}_{} = load i64, ptr @{}, align 8", label_prefix, c_once, cn).ok();
            } else {
                writeln!(out, "  %lt_{}_{} = add i64 0, 0", label_prefix, c_once).ok();
            }
            // Load counter once, precompute remaining iterations
            writeln!(out, "  %gcnt_{}_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", label_prefix, c_once, counter_idx).ok();
            writeln!(out, "  %init_{}_{} = load i64, ptr %gcnt_{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
            // Counted-down loop: remaining = bound - initial, count down to 0.
            // This eliminates the cmp instruction (sub sets ZF for jne) and
            // matches what clang emits for C for-loops.
            writeln!(out, "  %rem_{}_{} = sub i64 %lt_{}_{}, %init_{}_{}", label_prefix, c_once + 1, label_prefix, c_once, label_prefix, c_once).ok();
            writeln!(out, "  br label %{}", hdr_label).ok();
            writeln!(out, "{}:", hdr_label).ok();
            writeln!(out, "  %i_{}_{} = phi i64 [ %rem_{}_{}, %{} ], [ %dec_{}_{}, %{} ]", label_prefix, c_once + 2, label_prefix, c_once + 1, entry_label, label_prefix, c_once + 2, body_label).ok();
            writeln!(out, "  %cp_{}_{} = icmp sgt i64 %i_{}_{}, 0", label_prefix, c_once + 3, label_prefix, c_once + 2).ok();
            writeln!(out, "  br i1 %cp_{}_{}, label %{}, label %{}", label_prefix, c_once + 3, body_label, done_label).ok();
            writeln!(out, "{}:", body_label).ok();
            writeln!(out, "  %dec_{}_{} = sub i64 %i_{}_{}, 1", label_prefix, c_once + 2, label_prefix, c_once + 2).ok();
            super::emit_loop_metadata(out, "  ", &hdr_label, &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
            writeln!(out, "{}:", done_label).ok();
            // Final counter value is always the bound after counted-down loop
            writeln!(out, "  store i64 %lt_{}_{}, ptr %gcnt_{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
        } else if let Some(stmts) = body {
            // SSA mode: load once, phi in header, inline unrolled body with extract/insert, store once
            if let Some(bl) = bound_literal {
                writeln!(out, "  %lt{}_{} = add i64 0, {}", label_prefix, c_once, bl).ok();
            } else if let Some(ti) = total_idx {
                writeln!(out, "  %gt{}_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", label_prefix, c_once, ti).ok();
                writeln!(out, "  %lt{}_{} = load i64, ptr %gt{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt{}_{} = load i64, ptr @{}, align 8", label_prefix, c_once, cn).ok();
            } else {
                writeln!(out, "  %lt{}_{} = add i64 0, 0", label_prefix, c_once).ok();
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
                    self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
                    self.fun.expr_dedup_cache.clear();
                    self.fun.terminated = false;
                    self.fun.returns_i64 = false;
                    self.fun.ssa_state_reg = Some(cur);
                    // Pre-extract all float fields from the entering state
                    // so body field reads use old values — all float ops
                    // become independent, filling all CPU execution ports.
                    self.pre_extract_float_fields(&mut body4_buf);
                    self.pre_extract_int_fields(&mut body4_buf);
                    for stmt in stmts.iter().filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. })) {
                        self.emit_stmt(&mut body4_buf, stmt, "  ");
                    }
                    self.fun.ssa_old_float_regs.clear();
                    self.fun.ssa_old_int_regs.clear();
                    cur = self.fun.ssa_state_reg.take().unwrap_or(phi_reg.clone());
                }
                let backedge4 = cur;
                writeln!(body4_buf, "  store %State {}, ptr %slot_{}, align 8", backedge4, label_prefix).ok();
                super::emit_loop_metadata(&mut body4_buf, "  ", &format!("{}_hdr", label_prefix), &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
            }

            // --- body1: remainder loop (single iteration) ---
            let mut body1_buf = String::new();
            writeln!(body1_buf, "{}_body1:", label_prefix).ok();
            self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
            self.fun.expr_dedup_cache.clear();
            self.fun.terminated = false;
            self.fun.returns_i64 = false;
            self.fun.ssa_state_reg = Some(phi_reg.clone());
            self.pre_extract_float_fields(&mut body1_buf);
            self.pre_extract_int_fields(&mut body1_buf);
            for stmt in stmts.iter().filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. })) {
                self.emit_stmt(&mut body1_buf, stmt, "  ");
            }
            let backedge_val = self.fun.ssa_state_reg.take().unwrap_or(phi_reg.clone());
            writeln!(body1_buf, "  store %State {}, ptr %slot_{}, align 8", backedge_val, label_prefix).ok();
            super::emit_loop_metadata(&mut body1_buf, "  ", &format!("{}_hdr", label_prefix), &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);

            // Build initial %State from known constants
            writeln!(out, "  br label %{}_pre", label_prefix).ok();
            writeln!(out, "{}_pre:", label_prefix).ok();
            let mut cur_init = "zeroinitializer".to_string();
            let mut fields: Vec<(String, usize, String)> = self.ctx.field_index_map.iter()
                .map(|(name, &idx)| (name.clone(), idx, self.ctx.field_types[idx].clone()))
                .collect();
            fields.sort_by_key(|&(_, idx, _)| idx);
            for (name, idx, ty) in &fields {
                let init = self.ctx.field_initializers.get(name).and_then(|e| e.as_ref());
                match init {
                    Some(Expr::Float(f)) => {
                        let h = float_to_llvm_hex(*f);
                        let bc = format!("%fbc{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = bitcast i32 {} to float", bc, h).ok();
                        let iv = format!("%fiv{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, float {}, {}", iv, cur_init, bc, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Integer(n)) => {
                        let iv = format!("%iiv{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i64 {}, {}", iv, cur_init, n, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Bool(b)) => {
                        let v = if *b { 1 } else { 0 };
                        let iv = format!("%biv{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
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
                            let bc = format!("%nbc{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "  {} = bitcast i32 {} to float", bc, s).ok();
                            let iv = format!("%niv{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "  {} = insertvalue %State {}, float {}, {}", iv, cur_init, bc, idx).ok();
                            cur_init = iv;
                        } else {
                            let iv = format!("%niv{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "  {} = insertvalue %State {}, i64 {}, {}", iv, cur_init, s, idx).ok();
                            cur_init = iv;
                        }
                    }
                    Some(Expr::String(s)) => {
                        // 2026-06-29: Store actual string constant pointer, not i8* null.
                        // Previously this arm always wrote null regardless of the string value.
                        let si = self.ctx.string_constants.iter().position(|x| *x == *s).unwrap_or(0);
                        let g = format!("@str.{}", si);
                        let iv = format!("%siv{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, ptr bitcast (<{{ i64, i64, [{} x i8] }}>* {} to ptr), {}", iv, cur_init, s.len() + 1, g, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Char(c)) => {
                        let v = *c as i32;
                        let iv = format!("%civ{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i32 {}, {}", iv, cur_init, v, idx).ok();
                        cur_init = iv;
                    }
                    _ => {
                        let gep = format!("%gep{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gep, idx).ok();
                        let ld = format!("%ld{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = load {}, ptr {}, align {}", ld, ty, gep, self.align_of(&ty)).ok();
                        let iv = format!("%liv{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, {} {}, {}", iv, cur_init, ty, ld, idx).ok();
                        cur_init = iv;
                    }
                }
            }
            let slot = format!("%slot_{}", label_prefix);
            writeln!(out, "  {} = alloca %State, align 8", slot).ok();
            writeln!(out, "  store %State {}, ptr {}, align 8", cur_init, slot).ok();
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();

            // Header: extract counter, compare with adjusted/un-adjusted bounds
            writeln!(out, "{}_hdr:", label_prefix).ok();
            writeln!(out, "  {} = load %State, ptr {}, align 8", phi_reg, slot).ok();
            writeln!(out, "  %ex{}_{} = extractvalue %State {}, {}", label_prefix, self.fun.txn_counter, phi_reg, counter_idx).ok();
            let ex_reg = format!("%ex{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;

            if unroll > 1 {
                let adj = format!("%adj{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                if is_decreasing {
                    writeln!(out, "  {} = add i64 %lt{}_{}, {}", adj, label_prefix, c_once, unroll_minus_1).ok();
                } else {
                    writeln!(out, "  {} = add i64 %lt{}_{}, -{}", adj, label_prefix, c_once, unroll_minus_1).ok();
                }
                let cp4 = format!("%cp{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
                if is_decreasing {
                    writeln!(out, "  {} = icmp sgt i64 {}, {}", cp4, ex_reg, adj).ok();
                } else {
                    writeln!(out, "  {} = icmp slt i64 {}, {}", cp4, ex_reg, adj).ok();
                }
                writeln!(out, "  br i1 {}, label %{}_body4, label %{}_rem", cp4, label_prefix, label_prefix).ok();
                writeln!(out, "{}_rem:", label_prefix).ok();
            }
            let cp1 = format!("%cp{}_{}", label_prefix, self.fun.txn_counter); self.fun.txn_counter += 1;
            if is_decreasing {
                writeln!(out, "  {} = icmp sgt i64 {}, %lt{}_{}", cp1, ex_reg, label_prefix, c_once).ok();
            } else {
                writeln!(out, "  {} = icmp slt i64 {}, %lt{}_{}", cp1, ex_reg, label_prefix, c_once).ok();
            }
            writeln!(out, "  br i1 {}, label %{}_body1, label %{}_done", cp1, label_prefix, label_prefix).ok();

            if unroll > 1 {
                out.push_str(&body4_buf);
            }
            out.push_str(&body1_buf);

            let final_reg = format!("%final_{}", label_prefix);
            writeln!(out, "{}_done:", label_prefix).ok();
            writeln!(out, "  {} = load %State, ptr %slot_{}, align 8", final_reg, label_prefix).ok();
            writeln!(out, "  store %State {}, ptr %state, align 8", final_reg).ok();
        } else {
            if let Some(bl) = bound_literal {
                writeln!(out, "  %lt{}_{} = add i64 0, {}", label_prefix, c_once, bl).ok();
            } else if let Some(ti) = total_idx {
                writeln!(out, "  %gt{}_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", label_prefix, c_once, ti).ok();
                writeln!(out, "  %lt{}_{} = load i64, ptr %gt{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt{}_{} = load i64, ptr @{}, align 8", label_prefix, c_once, cn).ok();
            } else {
                writeln!(out, "  %lt{}_{} = add i64 0, 0", label_prefix, c_once).ok();
            }
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();
            writeln!(out, "{}_hdr:", label_prefix).ok();
            writeln!(out, "  %gp{}_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", label_prefix, c_once + 1, counter_idx).ok();
            writeln!(out, "  %lp{}_{} = load i64, ptr %gp{}_{}, align 8", label_prefix, c_once + 1, label_prefix, c_once + 1).ok();
            let cmp_reg = format!("%cp{}_{}", label_prefix, c_once + 2);
            if is_decreasing {
                writeln!(out, "  {} = icmp sgt i64 %lp{}_{}, %lt{}_{}", cmp_reg, label_prefix, c_once + 1, label_prefix, c_once).ok();
            } else {
                writeln!(out, "  {} = icmp slt i64 %lp{}_{}, %lt{}_{}", cmp_reg, label_prefix, c_once + 1, label_prefix, c_once).ok();
            }
            writeln!(out, "  br i1 {}, label %{}_body, label %{}_done", cmp_reg, label_prefix, label_prefix).ok();
            writeln!(out, "{}_body:", label_prefix).ok();
            writeln!(out, "  call void @{}(ptr %state)", txn_name).ok();
            super::emit_loop_metadata(out, "  ", &format!("{}_hdr", label_prefix), &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
            writeln!(out, "{}_done:", label_prefix).ok();
        }
    }

    /// Emit a main() that calls emit_folded_loop. Entry point for single-txn
    /// programs that can be folded. Three modes determined by use_phi and body:
    ///   use_phi=true  → A005a pure counter (phi-only, O(1) store)
    ///   use_phi=false + body → A005b inline SSA body (insertvalue chain)
    ///   use_phi=false + no body → legacy txn function call
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
        // 2026-07-01: Clear expression dedup cache before emitting @main.
        // emit_definition for txn functions (@simulate, etc.) populates the
        // cache with register names from that function's scope. If stale
        // entries persist into @main, the loop body may reference registers
        // that are defined in @simulate but not in @main, causing LLVM
        // "use of undefined value" errors (nbody %bfr).
        self.fun.expr_dedup_cache.clear();
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        // 2026-07-05: Use emit_state_allocas instead of manual %state alloca.
        // emit_state_allocas creates chunk allocas (%state_0, %state_1, ...)
        // for SROA-friendly field access, plus the monolithic %state for
        // backward compat with the insertvalue/extractvalue path.
        self.emit_state_allocas(out);
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        // Arena: per-loop scratch buffer for collection operations.
        // Arena is reset (not freed) between loop iterations — Phase 3
        // cross-tick pool keeps pages alive. Only freed at program exit.
        self.emit_arena_init(out, "  ");
        // Phase 2: preallocate collection buffers if loop has a known bound.
        if let Some(body_stmts) = body {
            if !use_phi {
                let bound_reg = format!("%bound_pre{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                if let Some(ti) = total_idx {
                    writeln!(out, "  %gt{0} = getelementptr inbounds %State, ptr %state, i32 0, i32 {1}", self.fun.txn_counter, ti).ok();
                    writeln!(out, "  {0} = load i64, ptr %gt{1}, align 8", bound_reg, self.fun.txn_counter).ok();
                    self.fun.txn_counter += 1;
                    self.emit_prealloc_for_body(out, "  ", body_stmts, &bound_reg);
                } else if let Some(ref cn) = total_const_name {
                    writeln!(out, "  {} = load i64, ptr @{}, align 8", bound_reg, cn).ok();
                    self.fun.txn_counter += 1;
                    self.emit_prealloc_for_body(out, "  ", body_stmts, &bound_reg);
                }
            }
        }
        // Legacy phi-mode: uses
        if use_phi {
            writeln!(out, "  br label %case_phi_entry").ok();
        }
        let uf = if let Some(body_stmts) = body {
            if !use_phi { self.optimal_unroll_factor(body_stmts) } else { 1 }
        } else { 1 };
        self.emit_folded_loop(out, txn_name, counter_idx, total_idx, total_const_name, "case", use_phi, body, uf, false, None);
        // Clear prealloc info (loop scope ended).
        self.fun.field_prealloc_info.clear();
        // Arena reset: keep memory alive for any subsequent loops.
        self.emit_arena_reset(out, "  ");
        let saved = std::mem::take(&mut self.fun.pending_post_hoist);
        self.emit_hoisted_post_loop_prints(out, &saved);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a counted-loop main() that uses per-field GEP loads/stores (no SSA
    /// insertvalue chain). A005b — memory path for non-linear bodies.
    ///
    /// Why this exists: when the body has branching guards and linearity cannot
    /// be proven, using a single %State SSA register causes phi dominance
    /// failures at convergence points. The solution: load/store each field
    /// independently through GEP — no phi needed for the state as a whole.
    ///
    /// Why counter phi still exists: the counter uses a phi node so LLVM sees a
    /// canonical loop counter for trip count, vectorization, and rotation analysis.
    /// Without the phi, the counter would be GEP+load+store (3 uops per iteration).
    ///
    /// Why pre_load_all_fields + phi override: all reads must see pre-tick values.
    /// The counter phi is injected into ssa_old_int_regs so body reads use the phi
    /// value rather than a stale GEP load from %state.
    ///
    /// 2026-06-13: A005b — memory path for non-linear bodies.
    /// 2026-06-20: Phase 1 — counter phi replaces GEP+load+store for induction
    /// variable.
    pub(crate) fn emit_folded_memory_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        body: &[Statement],
    ) {
        // 2026-07-01: Clear expr_dedup_cache — same rationale as emit_folded_main:
        // stale register names from txn function definitions (@simulate) would
        // cause "use of undefined value" errors in @main's loop body.
        self.fun.expr_dedup_cache.clear();
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        let attr = self.slp_attr("main", "#0");
        let c0 = self.fun.txn_counter;
        // Recover counter field name from index for phi override.
        let counter_name = {
            let mut found = None;
            for (name, &idx) in &self.ctx.field_index_map {
                if idx == counter_idx {
                    found = Some(name.clone());
                    break;
                }
            }
            found
        };
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        // Arena for memory-path loop: collection operations in A005b
        // use bump alloc instead of per-op free+malloc. Arena is reset
        // (not freed) between loop exits — Phase 3 cross-tick pool.
        self.emit_arena_init(out, "  ");
        // Bound loading — use numbered positional args ({0}, {1}) to avoid
        // LLVM IR brace chars being parsed as named format placeholders.
        let bound_suffix = total_idx.unwrap_or(c0);
        let bound_reg = format!("%lt{}_{}", c0, if total_idx.is_some() { bound_suffix } else { c0 });
        if let Some(ti) = total_idx {
            writeln!(out, "  %gt{0}_{1} = getelementptr inbounds %State, ptr %state, i32 0, i32 {1}", c0, ti).ok();
            writeln!(out, "  {0} = load i64, ptr %gt{1}_{2}, align 8", bound_reg, c0, bound_suffix).ok();
        } else if let Some(cn) = total_const_name {
            writeln!(out, "  {} = load i64, ptr @{}, align 8", bound_reg, cn).ok();
        } else {
            writeln!(out, "  {0} = add i64 0, 0", bound_reg).ok();
        }
        // Phase 2: preallocate collection buffers using known loop bound.
        self.emit_prealloc_for_body(out, "  ", body, &bound_reg);
        // 2026-07-02: Use memory-based counter (GEP+load) instead of a phi.
        // Phi-based counters create SSA predecessor issues when the body code
        // generates additional basic blocks (guards, getenv) that also branch
        // to _hdr (e.g., nbody_newton's print guard creates g16539_e as an
        // extra predecessor). Memory-based loads avoid this entirely: the
        // counter is loaded from %State at tick entry, compared against the
        // bound, and the loop test is just icmp+br. The body increments the
        // counter and stores it back via GEP+store at the end of the tick.
        writeln!(out, "  br label %_hdr").ok();
        writeln!(out, "_hdr:").ok();
        // Load counter from state (memory-based — no phi, no predecessor issues)
        let cmp_reg = format!("%cp{}", c0 + 2);
        let c_gep = format!("%cgep_{}", c0);
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", c_gep, counter_idx).ok();
        let c_val = format!("%cval_{}", c0);
        writeln!(out, "  {} = load i64, ptr {}, align 8", c_val, c_gep).ok();
        writeln!(out, "  {0} = icmp slt i64 {1}, %lt{2}_{3}", cmp_reg, c_val, c0, bound_suffix).ok();
        writeln!(out, "  br i1 {}, label %_body, label %_done", cmp_reg).ok();
        writeln!(out, "_body:").ok();
        self.fun.ssa_state_reg = None; // memory mode: writes go through GEP+store
        self.fun.returns_i64 = false;
        // Memory mode: body reads the counter via GEP+load from %State.
        // No phi override needed — the body always reads the pre-tick value.
        // (The body starts with GEP+load of each field, including the counter.)
        self.pre_load_all_fields(out, "%state", None);
        for s in body {
            if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                self.emit_stmt(out, s, "  ");
            }
        }
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        // Memory-mode latch: increment counter via GEP+load+add+store.
        // The body writes computed values to %State fields via GEP.
        // The counter increment is done here (after the body) so the body
        // sees the pre-increment value. LLVM will hoist/store this.
        let cnt_inc = format!("%cnt_inc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {0} = add i64 {1}, 1", cnt_inc, c_val).ok();
        writeln!(out, "  store i64 {0}, ptr {1}, align 8", cnt_inc, c_gep).ok();
        super::emit_loop_metadata(out, "  ", "_hdr", &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
        writeln!(out, "_done:").ok();
        // Arena reset: rewinds pointer for next scope. Memory stays live
        // across loops (Phase 3 cross-tick pool). Only freed at program exit.
        self.emit_arena_reset(out, "  ");
        let saved = std::mem::take(&mut self.fun.pending_post_hoist);
        self.emit_hoisted_post_loop_prints(out, &saved);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// 2026-07-03: Load phi register values into ssa_old caches and emit
    /// the loop body.  Clears caches before and after.  Sets loop_exit_label
    /// to "done" so terminating statements branch to the post-loop block.
    /// 2026-07-03: Load the loop bound into bound_reg.  The bound comes
    /// from either a state field (total_idx), a global constant (total_const_name),
    /// or zero (default).  Emitted directly into entry:
    fn emit_countable_load_bound(&mut self, out: &mut String, bound_reg: &str,
        total_idx: Option<usize>, total_const_name: Option<&str>, c0: usize)
    {
        let Some(ti) = total_idx else {
            let Some(cn) = total_const_name else {
                writeln!(out, "  {} = add i64 0, 0", bound_reg).ok();
                return;
            };
            writeln!(out, "  {} = load i64, ptr @{}, align 8", bound_reg, cn).ok();
            return;
        };
        let gt = self.emit_state_gep(out, "  ", "gt", "%state", ti);
        writeln!(out, "  {} = load i64, ptr {}, align 8", bound_reg, gt).ok();
    }

    /// 2026-07-03: Set up per-field phi and backedge registers, load
    /// initial field values, and emit the loop header (phi nodes + exit
    /// check).  Combined because the init register names must be available
    /// for the phi node entries at loop_hdr.
    /// 2026-07-05: A005c per-field phi (reverted from A005e).  Each state
    /// field gets its own phi node at the loop header so LLVM sees canonical
    /// induction variables and can SROA+GVM+vectorize the body.
    /// 2026-07-05: write_set marks fields modified by the body — fields NOT
    /// in write_set get !invariant.load on their initial load (LICM hoists
    /// them out of the loop).  exit_label is where the exit check branches
    /// to (either "commit" or "done").
    fn emit_countable_setup_phis_and_header(
        &mut self,
        out: &mut String,
        counter_idx: usize,
        bound_reg: &str,
        write_set: &HashSet<String>,
        exit_label: &str,
        is_decreasing: bool,
    ) -> (String, String, String, String, String, String) {
        self.fun.phi_field_regs.clear();
        self.fun.backedge_field_regs.clear();
        let mut all_fields: Vec<(String, usize, String)> = self.ctx.field_index_map.iter()
            .map(|(n, &i)| (n.clone(), i, self.ctx.field_types[i].clone()))
            .collect();
        // 2026-07-06: Sort by field name for deterministic phi header order.
        all_fields.sort_by_key(|(n, _, _)| n.clone());
        let mut init_regs: HashMap<String, String> = HashMap::new();
        let mut counter_name = String::new();
        // Build lookup: is a field name a member of a vector group?
        let vec_group_members: HashSet<String> = self.fun.vector_phi_groups.values()
            .flat_map(|members| members.iter().cloned())
            .collect();
        for (name, idx, ty) in &all_fields {
            if *idx == counter_idx { counter_name = name.clone(); continue; }
            let gep = self.emit_state_gep(out, "  ", "init_cnt", "%state", *idx);
            let init_load = format!("%init_{}_{}", name, self.fun.txn_counter);
            self.fun.txn_counter += 1;
            let tn = crate::backend::llvm::tbaa_node(ty, self.ctx.type_universe.as_ref());
            if !write_set.contains(name) {
                writeln!(out, "  {} = load {}, ptr {}, align {}, !tbaa !{}, !invariant.load !{{}}",
                    init_load, ty, gep, self.align_of(ty), tn).ok();
            } else {
                writeln!(out, "  {} = load {}, ptr {}, align {}, !tbaa !{}",
                    init_load, ty, gep, self.align_of(ty), tn).ok();
            }
            init_regs.insert(name.clone(), init_load);
            // Register mapping: use vector phi register for group members
            let mut found_group = false;
            for (vec_phi_name, members) in &self.fun.vector_phi_groups {
                if members.contains(name) {
                    self.fun.phi_field_regs.insert(name.clone(), vec_phi_name.clone());
                    // All group members share the same vector backedge register
                    let be_reg_name = format!("%be{}_{}",
                        &vec_phi_name[4..vec_phi_name.len() - 3],
                        &vec_phi_name[vec_phi_name.len() - 3..]);
                    // Actually: derive backedge name from vector phi name
                    // %phi_vx_v4 → %be_vx_v4
                    let be_name = format!("%be{}", &vec_phi_name[4..]);
                    self.fun.backedge_field_regs.insert(name.clone(), be_name);
                    found_group = true;
                    break;
                }
            }
            if !found_group {
                let phi_reg = format!("%phi_{}", name);
                let be_reg = format!("%be_{}", name);
                self.fun.phi_field_regs.insert(name.clone(), phi_reg);
                self.fun.backedge_field_regs.insert(name.clone(), be_reg);
            }
        }
        // Counter load
        let c_gep = self.emit_state_gep(out, "  ", "init_cnt", "%state", counter_idx);
        let init_count = format!("%init_count_{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, ptr {}, align 8", init_count, c_gep).ok();
        // Counter phi+backedge
        let count_phi_reg = format!("%phi_{}", counter_name);
        let count_be_reg = format!("%be_{}", counter_name);
        self.fun.phi_field_regs.insert(counter_name.clone(), count_phi_reg.clone());
        self.fun.backedge_field_regs.insert(counter_name.clone(), count_be_reg.clone());
        let pi_name = format!("%pi_cnt_{}", self.fun.txn_counter);
        let pn_name = format!("%pn_cnt_{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        // ── Initial vector construction (before br, so loop_hdr phis are at top) ──
        // 2026-07-05: Construct initial <4 x float> vectors from scalar init regs.
        // Must happen BEFORE br (in pre_phi block).  loop_hdr: phis use these values.
        let mut vec_phi_init: HashMap<String, String> = HashMap::new();
        for (phi_reg, members) in &self.fun.vector_phi_groups {
            let mut prev = "undef".to_string();
            for (i, member) in members.iter().enumerate() {
                let init_r = &init_regs[member];
                let ins = format!("%iv{}_{}{}", self.fun.txn_counter, &phi_reg[1..], self.fun.txn_counter);
                self.fun.txn_counter += 1;
                writeln!(out, "  {} = insertelement <4 x float> {}, float {}, i32 {}", ins, prev, init_r, i).ok();
                prev = ins;
            }
            vec_phi_init.insert(phi_reg.clone(), prev);
        }
        writeln!(out, "  br label %loop_hdr").ok();
        // ── Loop header: phi nodes + exit check ──────────────────────
        writeln!(out, "loop_hdr:").ok();
        writeln!(out, "  {} = phi i64 [ {}, %pre_phi ], [ {}, %latch ]", pi_name, init_count, pn_name).ok();
        // Emit scalar phis for non-grouped fields.  Vector phis are emitted
        // AFTER scalar phis because they don't affect the "phis at top" rule
        // (the insertelement initialization is in pre_phi, not loop_hdr).
        let mut emitted_vec_phis: HashSet<String> = HashSet::new();
        for (name, phi_reg) in &self.fun.phi_field_regs {
            if *name == counter_name { continue; }
            let Some(&idx) = self.ctx.field_index_map.get(name) else { continue; };
            let ty = &self.ctx.field_types[idx];
            // Check if this is a vector group member (phi_reg is a vector, not scalar)
            if self.fun.vector_phi_groups.contains_key(phi_reg) {
                // Emit vector phi only once per group (uses pre-constructed init from vec_phi_init)
                if emitted_vec_phis.insert(phi_reg.clone()) {
                    let init_vec = vec_phi_init.get(phi_reg).cloned().unwrap_or_else(|| "undef".to_string());
                    let be_reg = &self.fun.backedge_field_regs[name];
                    writeln!(out, "  {} = phi <4 x float> [ {}, %pre_phi ], [ {}, %latch ]", phi_reg, init_vec, be_reg).ok();
                }
            } else {
                let init_reg = &init_regs[name];
                let be_reg = &self.fun.backedge_field_regs[name];
                writeln!(out, "  {} = phi {} [ {}, %pre_phi ], [ {}, %latch ]", phi_reg, ty, init_reg, be_reg).ok();
            }
        }
        // Counter phi
        let ty_counter = &self.ctx.field_types[counter_idx];
        writeln!(out, "  {} = phi {} [ {}, %pre_phi ], [ {}, %latch ]", count_phi_reg, ty_counter, init_count, count_be_reg).ok();
        // Exit check: icmp sgt for decreasing counters (reg > N), icmp slt
        // for increasing counters (reg < N)
        let cmp_reg = format!("%cmp_hdr_{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        if is_decreasing {
            writeln!(out, "  {} = icmp sgt i64 {}, {}", cmp_reg, pi_name, bound_reg).ok();
        } else {
            writeln!(out, "  {} = icmp slt i64 {}, {}", cmp_reg, pi_name, bound_reg).ok();
        }
        writeln!(out, "  br i1 {}, label %body, label %{}", cmp_reg, exit_label).ok();
        (counter_name, count_phi_reg, count_be_reg, pi_name, pn_name, init_count)
    }

    /// 2026-07-03: Load phi register values into ssa_old caches and emit
    /// the loop body.  Clears caches before and after.  Sets loop_exit_label
    /// to "done" so terminating statements branch to the post-loop block.
    /// 2026-07-05: A005c per-field phi (reverted from A005e).  Phi register
    /// values are loaded into ssa_old caches at body entry via phi_regs_to_ssa_old
    /// instead of GEP+load from %State.  This eliminates the memory roundtrip
    /// per iteration — the phi register carries the iteration value directly.
    fn emit_countable_body(&mut self, out: &mut String, body: &[Statement]) {
        self.phi_regs_to_ssa_old(out);
        self.fun.let_bindings.clear();
        self.fun.let_binding_types.clear();
        self.fun.reg_float_cache.clear();
        self.fun.reg_type_cache.clear();
        self.fun.expr_dedup_cache.clear();
        // 2026-07-05: Initialize vector_phi_current with the phi register values.
        // The first insertelement for each vector group uses the phi register
        // (which holds the previous iteration's accumulated value) as the base.
        for (vec_phi, _members) in &self.fun.vector_phi_groups {
            self.fun.vector_phi_current.insert(vec_phi.clone(), vec_phi.clone());
        }
        self.fun.terminated = false;
        self.fun.loop_exit_label = Some("done".into());
        for s in body {
            if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                self.emit_stmt(out, s, "  ");
            }
        }
        self.fun.loop_exit_label = None;
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
    }

    /// 2026-07-03: Emit the latch block for a per-field phi loop.
    /// Handles counter increment, per-field backedge reload from %State,
    /// and loop metadata.  Extracted from emit_countable_main for
    /// flat control flow (max depth 2).
    /// 2026-07-03: Populate ssa_old_float_regs and ssa_old_int_regs from
    /// phi_field_regs. Used when entering a loop body (to make phi values
    /// available to emit_stmt reads) and in the post-loop done: block (to
    /// make final field values available to hoisted guard bodies).
    fn phi_regs_to_ssa_old(&mut self, out: &mut String) {
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        // Build set of all vector group member field names
        let mut vec_member_to_info: HashMap<String, (&String, usize)> = HashMap::new();
        for (vec_phi, members) in &self.fun.vector_phi_groups {
            for (i, member) in members.iter().enumerate() {
                vec_member_to_info.insert(member.clone(), (vec_phi, i));
            }
        }
        // 2026-07-06: Sort phi_field_regs for deterministic ssa_old inserts.
        let mut sorted_phi: Vec<(String, String)> = self.fun.phi_field_regs.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        sorted_phi.sort_by_key(|(k, _)| k.clone());
        for (name, phi_reg) in &sorted_phi {
            let Some(&idx) = self.ctx.field_index_map.get(name) else { continue; };
            let ll_ty = &self.ctx.field_types[idx];
            if let Some((vec_phi, comp_idx)) = vec_member_to_info.get(name) {
                // Vector group member: extract element from vector phi
                let ext = format!("%{}_e{}", &phi_reg[1..phi_reg.len() - 3], comp_idx);
                writeln!(out, "  {} = extractelement <4 x float> {}, i32 {}", ext, vec_phi, comp_idx).ok();
                self.fun.ssa_old_float_regs.insert(name.clone(), ext);
            } else if ll_ty == "float" || ll_ty == "double" {
                self.fun.ssa_old_float_regs.insert(name.clone(), phi_reg.clone());
            } else {
                self.fun.ssa_old_int_regs.insert(name.clone(), phi_reg.clone());
            }
        }
    }

    /// 2026-07-03: Emit the latch block for a per-field phi loop.
    /// Handles counter increment, per-field backedge reload from %State,
    /// and loop metadata.  Extracted from emit_countable_main for
    /// flat control flow (max depth 2).
    /// 2026-07-05: A005c per-field phi (reverted from A005e).  For each
    /// state field, reads the pending_phi_backedge to determine if the
    /// body wrote to it.  If modified, reloads from %State (GVN eliminates
    /// the redundant load via store).  If a native-typed value was stored,
    /// uses it directly as the phi backedge.  If unmodified, uses identity
    /// backedge (add 0 / fadd 0.0).  This eliminates per-iteration memory
    /// traffic for read-only and dead fields.
    fn emit_countable_latch(
        &mut self,
        out: &mut String,
        pi_name: &str,
        pn_name: &str,
        count_be_reg: &str,
        counter_name: &str,
        rotation_step: usize,
        rotation_cycle: &[String],
    ) {
        writeln!(out, "  br label %latch").ok();
        writeln!(out, "latch:").ok();
        // 2026-07-05: When rotation_step > 1, the counter increments by
        // the step size (body is unrolled rotation_step times per trip).
        let inc = if rotation_step > 1 { rotation_step as i64 } else { 1 };
        writeln!(out, "  {} = add i64 {}, {}", pn_name, pi_name, inc).ok();
        // 2026-07-06: Sort backedge_field_regs for deterministic latch order.
        let mut backedge_entries: Vec<(String, String)> = self.fun.backedge_field_regs.iter()
            .map(|(n, r)| (n.clone(), r.clone()))
            .collect();
        backedge_entries.sort_by_key(|(n, _)| n.clone());
        let phi_entries: HashMap<String, String> = self.fun.phi_field_regs.iter()
            .map(|(n, r)| (n.clone(), r.clone()))
            .collect();
        let field_map: HashMap<String, (usize, String)> = self.ctx.field_index_map.iter()
            .map(|(n, &i)| (n.clone(), (i, self.ctx.field_types[i].clone())))
            .collect();
        let pending_mod: HashSet<String> = self.fun.pending_phi_backedge.keys().cloned().collect();
        let mut emitted_be: HashSet<String> = HashSet::new();
        for (name, be_reg) in &backedge_entries {
            if *name == counter_name { continue; }
            // 2026-07-05: Skip duplicate backedge registers (vector group members
            // share the same be_reg — all 4 components emit into the same vector).
            if !emitted_be.insert(be_reg.clone()) { continue; }
            if pending_mod.contains(name) {
                // 2026-07-05: Rotation fields: GEP reload from %State in the
                // latch block (dominates backedge trivially).  GVN will CSE
                // the load-via-store with the body's store at the same GEP
                // address, producing zero memory traffic in the final code.
                // This breaks the circular phi chain for SCEV analysis.
                if rotation_step > 1 && self.fun.rotation_fields.contains(name) {
                    let Some(&(idx, ref ty)) = field_map.get(name) else { continue; };
                    let gep_reload = self.emit_state_gep(out, "  ", "be", "%state", idx);
                    writeln!(out, "  {} = load {}, ptr {}, align {}",
                        be_reg, ty, gep_reload, self.align_of(ty)).ok();
                // 2026-07-07: Rotation cycle fields use circular phi chain.
                // The backedge for field at position N references the phi register
                // of field at position (N + step) % cycle.len().  Phi registers
                // are defined in the header which dominates the latch — safe.
                } else if rotation_step > 1 && !rotation_cycle.is_empty() {
                    let Some(&(_, ref ty)) = field_map.get(name) else { continue; };
                    if let Some(pos) = rotation_cycle.iter().position(|f| f == name) {
                        let target_idx = (pos + rotation_step) % rotation_cycle.len();
                        let target_name = &rotation_cycle[target_idx];
                        let target_phi = phi_entries.get(target_name).cloned().unwrap_or_default();
                        if !target_phi.is_empty() {
                            let _ = match ty.as_str() {
                                "float" => writeln!(out, "  {} = fadd float {}, 0.0", be_reg, target_phi),
                                "double" => writeln!(out, "  {} = fadd double {}, 0.0", be_reg, target_phi),
                                _ => writeln!(out, "  {} = add i64 0, {}", be_reg, target_phi),
                            };
                        }
                    }
                // 2026-07-03: If the body stored a native-typed value, use it
                // directly as the phi backedge instead of reloading from %State.
                // This eliminates the store→GEP→load roundtrip per field.
                // For float/double use fadd 0.0 (identity), for ints use add 0.
                } else if let Some(typed_reg) = self.fun.pending_phi_native_backedge.get(name) {
                    let Some(&(_, ref ty)) = field_map.get(name) else { continue; };
                    // 2026-07-05: Vector group backedges use the accumulated <4 x float>
                    // from the body's insertelement chain (no arithmetic or type coercion).
                    // 2026-07-06: FIX — use vector_phi_current instead of
                    // pending_phi_native_backedge.  pending_phi_native_backedge[name]
                    // contains the insertelement for THAT SPECIFIC field only — later
                    // group members' insertelement updates are NOT captured.  Since
                    // the backedge dedup emits the vector backedge for the FIRST field
                    // name encountered (HashMap iteration order is arbitrary), elements
                    // for group members processed AFTER the first would be stale,
                    // causing body positions never to advance (nbody_sqrt fix).
                    if typed_reg.starts_with("%iv") {
                        // 2026-07-06: Reconstruct vector phi name from backedge register.
                        // Use vector_phi_current instead of pending_phi_native_backedge:
                        //   pending_phi_native_backedge[name] captures only the
                        //   insertelement for THAT specific field — later group members'
                        //   insertelement updates are NOT captured. Since the backedge
                        //   dedup emits the vector backedge for the FIRST field name
                        //   encountered (HashMap iteration order is arbitrary), elements
                        //   for group members processed after the first would be stale,
                        //   causing body positions never to advance (nbody_sqrt fix).
                        //   vector_phi_current[vec_phi] has the fully accumulated vector
                        //   with ALL 4 elements set from the body's insertelement chain.
                        //   Derive vec_phi from be_reg:
                        //   %be_vx_v4 → strip "%be" → "_vx_v4" → strip "_" → "vx_v4"
                        //   → format!("%phi_{}", "vx_v4") → "%phi_vx_v4"
                        let suffix = be_reg[3..].strip_prefix('_').unwrap_or(&be_reg[3..]);
                        let vec_phi_name = format!("%phi_{}", suffix);
                        let acc_reg = self.fun.vector_phi_current.get(&vec_phi_name)
                            .map(|s| s.as_str())
                            .unwrap_or(typed_reg);
                        writeln!(out, "  {} = bitcast <4 x float> {} to <4 x float>", be_reg, acc_reg).ok();
                    } else {
                        let _ = match ty.as_str() {
                            "float" => writeln!(out, "  {} = fadd float {}, 0.0", be_reg, typed_reg),
                            "double" => writeln!(out, "  {} = fadd double {}, 0.0", be_reg, typed_reg),
                            _ => writeln!(out, "  {} = add i64 0, {}", be_reg, typed_reg),
                        };
                    }
                } else {
                    // Fallback: reload from %State
                    let Some(&(idx, ref ty)) = field_map.get(name) else { continue; };
                    let gep_reload = self.emit_state_gep(out, "  ", "be", "%state", idx);
                    writeln!(out, "  {} = load {}, ptr {}, align {}",
                        be_reg, ty, gep_reload, self.align_of(ty)).ok();
                }
            } else {
                // 2026-07-05: Identity backedge — use phi register unchanged.
                // For float/double fields, use fadd 0.0 (identity) instead of
                // add i64, which would produce type mismatch when the phi
                // register is float-typed.
                let phi_reg = phi_entries.get(name).cloned().unwrap_or_default();
                let Some(&(_, ref ty)) = field_map.get(name) else { continue; };
                let _ = match ty.as_str() {
                    "float" => writeln!(out, "  {} = fadd float {}, 0.0", be_reg, phi_reg),
                    "double" => writeln!(out, "  {} = fadd double {}, 0.0", be_reg, phi_reg),
                    _ => writeln!(out, "  {} = add i64 0, {}", be_reg, phi_reg),
                };
            }
        }
        writeln!(out, "  {} = add i64 0, {}", count_be_reg, pn_name).ok();
        super::emit_loop_metadata(out, "  ", "loop_hdr", &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
    }

    /// 2026-07-03: Emit a main() with per-field phi nodes (A005c — countable
    /// loop). Creates one phi per state field at the loop header so LLVM sees
    /// canonical induction variables and can vectorize the body.
    ///
    /// Why per-field phis: the existing paths use either a %slot_case alloca
    /// round-trip (A005a inline SSA) or GEP+load-store per iteration (A005b
    /// memory). Both hide the fields from LLVM's induction variable analysis.
    /// By promoting each field to an SSA phi, LLVM sees individual values
    /// flowing through the loop body and can apply SROA, GVN, and
    /// vectorization.
    ///
    /// Why memory still appears in the latch: modified field values are
    /// reloaded from %State at the latch (not identity). GVN eliminates the
    /// redundant load since it uses the same GEP address as the body store.
    /// This keeps the pattern compatible with the existing emit_stmt memory
    /// mode (which asserts ssa_state_reg is None for GEP stores).
    ///
    /// Why identity backedge for unmodified fields: fields not written by the
    /// body keep their phi value unchanged. GVN copy-propagates the phi
    /// register, producing zero instructions for the backedge.
    /// 2026-07-04: Memory-access loop with 1 induction phi (A005d).
    /// Used when the loop has many state fields (default threshold: 8+).
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
        let c0 = self.fun.txn_counter;
        self.fun.expr_dedup_cache.clear();
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        self.emit_state_allocas(out);
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        self.emit_arena_init(out, "  ");
        let bound_reg = format!("%cnt_bound_{}", c0);
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);
        self.emit_prealloc_for_body(out, "  ", body, &bound_reg);
        writeln!(out, "  br label %_hdr").ok();
        writeln!(out, "_hdr:").ok();
        let c_val = self.emit_state_gep(out, "  ", "cv", "%state", counter_idx);
        let cld = format!("%cld_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, ptr {}, align 8", cld, c_val).ok();
        let cmp_reg = format!("%chc_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = icmp slt i64 {}, {}", cmp_reg, cld, bound_reg).ok();
        writeln!(out, "  br i1 {}, label %_body, label %_done", cmp_reg).ok();
        writeln!(out, "_body:").ok();
        self.fun.ssa_state_reg = None;
        self.fun.pending_phi_backedge.clear();
        self.fun.pending_phi_native_backedge.clear();
        self.fun.returns_i64 = false;
        self.pre_load_all_fields(out, "%state", None);
        self.fun.loop_exit_label = Some("_done".into());
        for s in body {
            if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                self.emit_stmt(out, s, "  ");
            }
        }
        self.fun.loop_exit_label = None;
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        super::emit_loop_metadata(out, "  ", "_hdr", &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
        writeln!(out, "_done:").ok();
        self.emit_arena_reset(out, "  ");
        let saved = std::mem::take(&mut self.fun.pending_post_hoist);
        self.emit_hoisted_post_loop_prints(out, &saved);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// 2026-07-03: Emit a main() with per-field phi nodes (A005c — countable
    /// loop). Creates one phi per state field at the loop header so LLVM sees
    /// canonical induction variables and can vectorize the body.
    ///
    /// Why per-field phis: the existing paths use either a %slot_case alloca
    /// round-trip (A005a inline SSA) or GEP+load-store per iteration (A005b
    /// memory). Both hide the fields from LLVM's induction variable analysis.
    /// By promoting each field to an SSA phi, LLVM sees individual values
    /// flowing through the loop body and can apply SROA, GVN, and
    /// vectorization.
    ///
    /// Why memory still appears in the latch: modified field values are
    /// reloaded from %State at the latch (not identity). GVN eliminates the
    /// redundant load since it uses the same GEP address as the body store.
    /// This keeps the pattern compatible with the existing emit_stmt memory
    /// mode (which asserts ssa_state_reg is None for GEP stores).
    ///
    /// Why identity backedge for unmodified fields: fields not written by the
    /// body keep their phi value unchanged. GVN copy-propagates the phi
    /// register, producing zero instructions for the backedge.
    /// 2026-07-04: Parallel-safe mode, !invariant.load, Path A zero stores,
    /// phi commit block (last_val_temps).
    /// 2026-07-05: A005c per-field phi with dead-field analysis (reverted from
    /// A005e).  All post-July-3 optimizations preserved: Path A zero stores,
    /// phi commit block, !invariant.load, parallel-safe, SROA chunks, LLVM
    /// attributes, dead-field filtering.
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
        let c0 = self.fun.txn_counter;
        self.fun.expr_dedup_cache.clear();
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        self.emit_state_allocas(out);
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        self.emit_arena_init(out, "  ");
        // ── Load bound ────────────────────────────────────────────────
        let bound_reg = format!("%cnt_bound_{}", c0);
        self.emit_countable_load_bound(out, &bound_reg, total_idx, total_const_name, c0);
        self.emit_prealloc_for_body(out, "  ", body, &bound_reg);
        writeln!(out, "  br label %pre_phi").ok();
        writeln!(out, "pre_phi:").ok();
        // ── Determine exit_label and create last-value allocas ─────────
        // If the done: block reads state fields (hoisted prints), create
        // temporaries that store the phi's final value ONCE at loop exit.
        // The commit block stores to these; done: loads from them instead
        // of from %State, eliminating ~N stores per iteration.
        self.fun.done_needs_fields.clear();
        for hoisted_body in &self.fun.pending_post_hoist {
            for s in hoisted_body {
                collect_field_refs(s, &mut self.fun.done_needs_fields, &self.ctx.field_index_map);
            }
        }
        // 2026-07-05: Build vector phi groups BEFORE last_val_temps allocation.
        // last_val_temps checks vector_phi_groups to share <4 x float> allocas
        // across vector group members. If built after, all fields get scalar allocas
        // and the commit block writes <4 x float> into 4-byte allocas (buffer overflow).
        // 2026-07-06: Moved before last_val_temps to fix nbody_sqrt non-determinism.
        self.fun.vector_phi_groups = build_vector_phi_groups(
            &self.ctx.field_index_map,
            &self.ctx.field_types,
        );
        self.fun.last_val_temps.clear();
        let exit_label = if !self.fun.done_needs_fields.is_empty() {
            for field_name in self.fun.done_needs_fields.iter() {
                let Some(&idx) = self.ctx.field_index_map.get(field_name) else { continue; };
                let ty = &self.ctx.field_types[idx];
                // 2026-07-05: Vector group members share one <4 x float> alloca.
                // Check if this field belongs to a vector group and another member
                // already created the alloca.
                let mut shared_alloca = None;
                for (vec_phi, members) in &self.fun.vector_phi_groups {
                    if members.contains(field_name) {
                        for m in members {
                            if let Some(shared) = self.fun.last_val_temps.get(m) {
                                shared_alloca = Some(shared.clone());
                                break;
                            }
                        }
                        break;
                    }
                }
                if let Some(shared) = shared_alloca {
                    self.fun.last_val_temps.insert(field_name.clone(), shared);
                } else if self.fun.vector_phi_groups.values().any(|m| m.contains(field_name)) {
                    // First member of group: create <4 x float> alloca
                    let reg = format!("%lv_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = alloca <4 x float>, align 16", reg).ok();
                    self.fun.last_val_temps.insert(field_name.clone(), reg);
                } else {
                    let reg = format!("%lv_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = alloca {}, align {}", reg, ty, self.align_of(ty)).ok();
                    self.fun.last_val_temps.insert(field_name.clone(), reg);
                }
            }
            // 2026-07-04: Phi commit block replaces per-iteration stores.
            // Suppress body stores via needs_state_stores_in_body = false.
            // The commit block writes phi final values once at loop exit,
            // and emit_hoisted_post_loop_prints reads from them.
            self.fun.needs_state_stores_in_body = false;
            "commit"
        } else {
            "done"
        };
        // ── Dead-field analysis: trace liveness, filter dead assignments ──
        // 2026-07-04: Eliminate & assignments to fields that no observable
        // output consumes.  This shrinks the body seen by LLVM's loop unroller,
        // fixing the phase-ordering issue (fannkuch_redux: ~80→~40 insns).
        // 2026-07-05: Include hoisted terminating guard content in liveness
        // analysis.  hoist_terminating_guard removes [count==bound]{...} from
        // body_stmts and stores it in pending_post_hoist.  Without it,
        // trace_live_fields never sees print_float#(energy) and marks all
        // fields dead — causing nbody_sqrt to eliminate all stores.
        let liveness_body = {
            let post = &self.fun.pending_post_hoist;
            let mut combined = body.to_vec();
            for hoisted in post {
                combined.extend(hoisted.iter().cloned());
            }
            combined
        };
        let live = trace_live_fields(&liveness_body, &self.ctx.field_index_map);
        let filtered_body = filter_dead_assignments(body, &live);
        // ── Initial field loads + phi/backedge register setup + loop header ──
        let (counter_name, count_phi_reg, count_be_reg, pi_name, pn_name, _init_count)
            = self.emit_countable_setup_phis_and_header(out, counter_idx, &bound_reg, write_set, exit_label, is_decreasing);
        // ── Body: load phi regs into ssa_old, emit statements ─────────
        writeln!(out, "body:").ok();
        self.fun.ssa_state_reg = None; // memory mode: writes go through GEP+store
        self.fun.pending_phi_backedge.clear();
        self.fun.pending_phi_native_backedge.clear();
        self.fun.returns_i64 = false;
        // 2026-07-04: Decide whether the body must store to %State.
        // Path A (no stores): done: does NOT read %State (no post-loop
        //   hoisted guards).  Phi registers + pending_phi_native_backedge
        //   carry all values forward.  Zero memory traffic in hot loop.
        //   LLVM's optimizer sees a clean phi loop with no barriers.
        // Path B (stores preserved): done: reads %State via GEP+load
        //   (post-loop hoisted guards from term! -> swan_song).  Stores
        //   ensure done:'s loads see the final iteration's field values.
        if self.fun.last_val_temps.is_empty() {
            // No phi commit block — fall back to old Path A/B logic.
            // When last_val_temps is non-empty, the phi commit block
            // was created above and already set the flag to false.
            self.fun.needs_state_stores_in_body = !self.fun.pending_post_hoist.is_empty();
        }
        // 2026-07-04: Enable parallel-safe mode for ALL bodies.
        // ssa_old caches are NOT updated after & assignments — all reads
        // use old (phi) values.  This makes every computation independent,
        // enabling LLVM to SIMD-vectorize across the entire body.
        // The counter field is exempt (tracked by counter_field_name).
        self.fun.parallel_safe_body = is_body_parallel_safe(&filtered_body);
        // 2026-07-04: Track the counter field name so emit_memory_field_store
        // can exempt it from parallel-safe mode.  Guard conditions like
        // [count % 5000000 == 0] need to read the new counter value, not the
        // old phi register.  The counter always updates ssa_old_*_regs.
        self.fun.counter_field_name = Some(counter_name.clone());
        // 2026-07-04: Scan body for fields that need sequential updates
        // (read-after-write + guard conditions/arguments).  These fields
        // are exempt from parallel-safe mode — they get normal sequential
        // ssa_old updates so later reads or guard conditions see correct values.
        self.fun.parallel_safe_exempt_fields.clear();
        let mut guard_exempt = HashSet::new();
        collect_parallel_safe_exemptions(&filtered_body, &mut self.fun.parallel_safe_exempt_fields, &mut guard_exempt, &self.ctx.field_index_map);
        // ── Rotation detection ────────────────────────────────────────
        // 2026-07-05: Detect circular phi chains in rotation patterns
        // (fannkuch_redux 12-cycle).  Body unrolling + GEP reloads in the
        // latch break the cycle into independent SCEV-analyzable values.
        let (rotation_step, rotation_cycle) = detect_rotation_ast(
            &filtered_body, &self.ctx.field_index_map,
        );
        // When rotation is active, use the ORIGINAL body (not filtered_body)
        // because filter_dead_assignments may remove the counter increment
        // (count is dead in liveness analysis when the terminating guard
        // condition is hoisted without its condition expression).
        let emit_body: &[Statement] = if rotation_step > 1 { body } else { &filtered_body };
        if rotation_step > 1 {
            // Build rotation_fields set — forces body stores to %State so
            // the latch can GEP-reload them (breaking the phi chain).
            // 2026-07-07: Rotation cycle fields are EXCLUDED from rotation_fields
            // (no stores + GEP-reload needed).  The circular phi chain via
            // pending_phi_native_backedge handles rotation naturally using phi
            // registers (which dominate the latch).  Non-rotation fields still
            // need stores + GEP-reload (their pending values are body-computed
            // registers that don't dominate the latch).
            for s in emit_body {
                if let Statement::Assignment { lhs, .. } = s {
                    if let Some(fname) = target_field_name(lhs) {
                        if self.fun.phi_field_regs.contains_key(&fname) {
                            // 2026-07-07: Rotation cycle fields are excluded from
                            // rotation_fields — they use circular phi chain via
                            // phi_field_regs in the latch instead of GEP-reload.
                            if !rotation_cycle.contains(&fname) {
                                self.fun.rotation_fields.insert(fname.clone());
                                self.fun.parallel_safe_exempt_fields.insert(fname.clone());
                            }
                        }
                    }
                }
            }
            // Must force stores for rotation fields (latch needs fresh values).
            // Pure rotations: stores for non-rotation fields only.
            // Non-pure rotations: stores for ALL modified fields (rotation_cycle
            // is empty for non-rotation cases, so all fields are added above).
            self.fun.needs_state_stores_in_body = true;
        }
        // ── Save counter phi reg before emit_countable_body clears ssa_old ──
        // 2026-07-07: emit_countable_body clears ssa_old_int_regs (line 1132),
        // which makes the counter register unavailable for the hot path pre-check.
        // Save the phi register name from phi_field_regs (which is not cleared).
        let count_phi_reg = self.fun.phi_field_regs.get(&counter_name).cloned();
        // ── Emit body (possibly unrolled for rotation) ────────────────
        self.emit_countable_body(out, emit_body);
        if rotation_step > 1 {
            // 2026-07-07: Hybrid rotation unrolling.
            //
            // For FULL trips (count + step <= N), take the straight-line
            // hot path — all step copies in one basic block, no per-copy
            // exit checks.  For PARTIAL trips (final trip when N % step
            // != 0), take the cold path with individual exit checks between
            // copies to avoid over-processing.
            //
            // The hot path eliminates ~3 exit checks per trip (for step=4),
            // saving ~25M branches over N=50M.  The cold path is taken at
            // most once (the final partial trip).
            //
            // After the cold path, the loop always exits (count+step >= N),
            // so the cold path's backedge phi values are dead.  We save
            // pending_phi_native_backedge from the hot path and restore it
            // before emit_countable_latch so the latch uses hot-path values
            // (which feed the next full trip).
            let mut hot_backedge: Option<HashMap<String, String>> = None;
            // ── Hot path: straight-line copies (no exit checks) ────────
            // First, emit the pre-check for full vs partial trip.
            // Use the saved phi register %phi_count (count at loop header entry),
            // not the body output (which is phi_count + 1).  ssa_old_int_regs
            // was cleared by emit_countable_body, so we saved count_phi_reg above.
            if let Some(creg) = count_phi_reg {
                let full_chk_reg = format!("%full_chk_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let full_chk_next = format!("%full_next_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "  {} = add i64 {}, {}", full_chk_next, creg, rotation_step).ok();
                writeln!(out, "  {} = icmp sle i64 {}, {}", full_chk_reg, full_chk_next, bound_reg).ok();
                writeln!(out, "  br i1 {}, label %rot_full, label %rot_cold", full_chk_reg).ok();
            } else {
                writeln!(out, "  br label %rot_full").ok();
            }
            // 2026-07-07: Save original body backedge values BEFORE the hot path
            // emits body copies (which overwrite pending_phi_native_backedge with
            // rot_full-block registers).  The cold path needs body-block registers
            // that dominate rot_cold:.
            let pre_hot_backedge = self.fun.pending_phi_native_backedge.clone();
            writeln!(out, "rot_full:").ok();
            for i in 1..rotation_step {
                // Advance ssa_old for rotation cycle fields
                if !rotation_cycle.is_empty() {
                    let rot_set: HashSet<String> = rotation_cycle.iter().cloned().collect();
                    for (fname, val) in &self.fun.pending_phi_native_backedge {
                        if rot_set.contains(fname) {
                            self.fun.ssa_old_int_regs.insert(fname.clone(), val.clone());
                            self.fun.ssa_old_float_regs.insert(fname.clone(), val.clone());
                        }
                    }
                }
                // GEP-reload rotation fields into ssa_old caches
                let rot_fields: Vec<String> = self.fun.rotation_fields.iter().cloned().collect();
                for fname in &rot_fields {
                    let Some(&idx) = self.ctx.field_index_map.get(fname) else { continue; };
                    let ty = self.ctx.field_types[idx].clone();
                    let gep = self.emit_state_gep(out, "  ", "rr", "%state", idx);
                    let ld = format!("%rld_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = load {}, ptr {}, align {}", ld, ty, gep, self.align_of(&ty)).ok();
                    if ty == "float" || ty == "double" {
                        self.fun.ssa_old_float_regs.insert(fname.clone(), ld.clone());
                    } else {
                        self.fun.ssa_old_int_regs.insert(fname.clone(), ld.clone());
                    }
                }
                // GEP-reload count field
                if let Some(&cnt_idx) = self.ctx.field_index_map.get("count") {
                    let cnt_gep = self.emit_state_gep(out, "  ", "rc", "%state", cnt_idx);
                    let cnt_ld = format!("%rlc_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = load i64, ptr {}, align 8", cnt_ld, cnt_gep).ok();
                    self.fun.ssa_old_int_regs.insert("count".to_string(), cnt_ld.clone());
                    if counter_name != "count" {
                        self.fun.ssa_old_int_regs.insert(counter_name.clone(), cnt_ld);
                    }
                }
                // Emit body copy (no exit check — straight-line)
                self.fun.let_bindings.clear();
                self.fun.let_binding_types.clear();
                self.fun.reg_float_cache.clear();
                self.fun.reg_type_cache.clear();
                self.fun.expr_dedup_cache.clear();
                self.fun.terminated = false;
                self.fun.loop_exit_label = Some("done".into());
                for s in emit_body {
                    // 2026-07-07: Skip the terminating guard [count == N]
                    // { term! -> print_int#(checksum) } in the hot path.  The
                    // pre-check (count + step <= N-3) guarantees count + k < N
                    // for k=0..3, so the guard can never fire here.  The swan
                    // song is already hoisted to post_hoist by
                    // hoist_terminating_guard (executes after loop exit).
                    if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                        if let Statement::Guarded { statements: gs, .. } = s {
                            if terminating_guard(gs) { continue; }
                        }
                        self.emit_stmt(out, s, "  ");
                    }
                }
                self.fun.loop_exit_label = None;
            }
            writeln!(out, "  br label %latch").ok();
            // Save hot path's pending_phi_native_backedge for latch restoration.
            hot_backedge = Some(self.fun.pending_phi_native_backedge.clone());
            // ── Cold path: exit-check copies (for partial final trip) ──
            // 2026-07-07: Restore original body backedge values.
            // The cold path's ssa_old advance must use body-block registers
            // that dominate rot_cold:, not rot_full-block registers from the
            // hot path body copies.
            self.fun.pending_phi_native_backedge = pre_hot_backedge;
            writeln!(out, "rot_cold:").ok();
            for i in 1..rotation_step {
                // Advance ssa_old for rotation cycle fields
                if !rotation_cycle.is_empty() {
                    let rot_set: HashSet<String> = rotation_cycle.iter().cloned().collect();
                    for (fname, val) in &self.fun.pending_phi_native_backedge {
                        if rot_set.contains(fname) {
                            self.fun.ssa_old_int_regs.insert(fname.clone(), val.clone());
                            self.fun.ssa_old_float_regs.insert(fname.clone(), val.clone());
                        }
                    }
                }
                // GEP-reload rotation fields into ssa_old caches
                let rot_fields: Vec<String> = self.fun.rotation_fields.iter().cloned().collect();
                for fname in &rot_fields {
                    let Some(&idx) = self.ctx.field_index_map.get(fname) else { continue; };
                    let ty = self.ctx.field_types[idx].clone();
                    let gep = self.emit_state_gep(out, "  ", "rr", "%state", idx);
                    let ld = format!("%rld_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = load {}, ptr {}, align {}", ld, ty, gep, self.align_of(&ty)).ok();
                    if ty == "float" || ty == "double" {
                        self.fun.ssa_old_float_regs.insert(fname.clone(), ld.clone());
                    } else {
                        self.fun.ssa_old_int_regs.insert(fname.clone(), ld.clone());
                    }
                }
                // GEP-reload count field
                if let Some(&cnt_idx) = self.ctx.field_index_map.get("count") {
                    let cnt_gep = self.emit_state_gep(out, "  ", "rc", "%state", cnt_idx);
                    let cnt_ld = format!("%rlc_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = load i64, ptr {}, align 8", cnt_ld, cnt_gep).ok();
                    self.fun.ssa_old_int_regs.insert("count".to_string(), cnt_ld.clone());
                    if counter_name != "count" {
                        self.fun.ssa_old_int_regs.insert(counter_name.clone(), cnt_ld);
                    }
                }
                // Overflow guard: if count >= bound, exit to latch
                let count_reg = self.fun.ssa_old_int_regs.get("count")
                    .or_else(|| self.fun.ssa_old_int_regs.get(&counter_name)).cloned();
                if let Some(creg) = count_reg {
                    let chk = format!("%ro_chk_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = icmp sge i64 {}, {}", chk, creg, bound_reg).ok();
                    writeln!(out, "  br i1 {}, label %latch, label %body_rot{}", chk, i).ok();
                    writeln!(out, "body_rot{}:", i).ok();
                }
                // Emit body copy
                self.fun.let_bindings.clear();
                self.fun.let_binding_types.clear();
                self.fun.reg_float_cache.clear();
                self.fun.reg_type_cache.clear();
                self.fun.expr_dedup_cache.clear();
                self.fun.terminated = false;
                self.fun.loop_exit_label = Some("done".into());
                for s in emit_body {
                    // 2026-07-07: Skip the terminating guard (same rationale
                    // as the hot path — the swan song is already hoisted to
                    // post_hoist by hoist_terminating_guard).  The cold path's
                    // own overflow guard (count >= bound -> latch) handles the
                    // safe exit when the bound is reached.
                    if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                        if let Statement::Guarded { statements: gs, .. } = s {
                            if terminating_guard(gs) { continue; }
                        }
                        self.emit_stmt(out, s, "  ");
                    }
                }
                self.fun.loop_exit_label = None;
            }
            // Restore hot path's pending_phi_native_backedge for the latch.
            // The cold path's backedge values are dead (loop exits after
            // any partial trip).
            if let Some(hot) = hot_backedge {
                self.fun.pending_phi_native_backedge = hot;
            }
        }
        // ── Latch: increment counter, reload modified fields ─────────
        self.emit_countable_latch(out, &pi_name, &pn_name, &count_be_reg, &counter_name, rotation_step, &rotation_cycle);
        // ── Commit block: store phi final values to last-value allocas ──
        // Runs ONCE at loop exit (when the header branches to %commit instead
        // of %done).  done: loads from these allocas — no per-iteration stores.
        if !self.fun.last_val_temps.is_empty() {
            writeln!(out, "commit:").ok();
            let mut committed_vec: HashSet<String> = HashSet::new();
            // 2026-07-06: Sort last_val_temps for deterministic commit store order.
            let mut sorted_commit: Vec<(String, String)> = self.fun.last_val_temps.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            sorted_commit.sort_by_key(|(k, _)| k.clone());
            for (field_name, temp_reg) in &sorted_commit {
                let Some(&idx) = self.ctx.field_index_map.get(field_name) else { continue; };
                let ty = &self.ctx.field_types[idx];
                let phi_reg = self.fun.phi_field_regs.get(field_name)
                    .map(|s| s.as_str()).unwrap_or("");
                if !phi_reg.is_empty() {
                    // 2026-07-05: Vector group members share the same phi register.
                    // Only emit the store once per vector group.
                    if committed_vec.insert(phi_reg.to_string()) {
                        // Determine the store type: for vector groups, the phi_reg
                        // has a "_v4" suffix — use <4 x float> instead of scalar.
                        if phi_reg.ends_with("_v4") {
                            writeln!(out, "  store <4 x float> {}, ptr {}, align 16", phi_reg, temp_reg).ok();
                        } else {
                            writeln!(out, "  store {} {}, ptr {}, align {}", ty, phi_reg, temp_reg, self.align_of(ty)).ok();
                        }
                    }
                }
            }
            writeln!(out, "  br label %done").ok();
        }
        // ── Done: emit post-loop prints + exit ──────────────────────
        writeln!(out, "done:").ok();
        self.emit_arena_reset(out, "  ");
        let saved = std::mem::take(&mut self.fun.pending_post_hoist);
        self.emit_hoisted_post_loop_prints(out, &saved);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
        self.fun.needs_state_stores_in_body = true;
        self.fun.counter_field_name = None;
        self.fun.parallel_safe_exempt_fields.clear();
        self.fun.rotation_fields.clear();
        self.fun.vector_phi_groups.clear();
        self.fun.vector_phi_current.clear();
    }

    /// Emit a `main()` that uses per-field GEP loads/stores for all-convergent
    /// programs. Loads each field via GEP at tick entry, runs each reactive txn's
    /// precondition and body inline with direct GEP stores for modifications,
    /// avoiding the wide %State load/store + extractvalue/insertvalue pattern.
    /// Handles trigger sampling inline (via lazy emit_trg_load in emit_expr),
    /// and the wake path (__rt_wait) when has_wake_triggers is set.
    /// 2026-07-03: Try modulo-switch dispatch for reactive txns.
    /// Returns true if emitted via modulo-switch.
    fn try_modulo_switch_dispatch(
        &mut self,
        out: &mut String,
        reactive_txns: &[&(String, &crate::ast::Transaction)],
    ) -> bool {
        if reactive_txns.len() < 2 { return false; }
        let mut counter: Option<String> = None;
        let mut bound: Option<String> = None;
        let mut divisor: Option<i64> = None;
        let mut cases: Vec<(i64, &str)> = Vec::new();
        let mut all_match = true;
        for (name, txn) in reactive_txns {
            let pre = &txn.contract.pre_condition;
            let norm = pre.normalize_to_old_recursive();
            let (cn, bn, ck_k, n) = match &norm {
                Expr::And(left, right) => {
                    let cn = match left.as_ref() {
                        Expr::Lt(l, _) => if let Expr::Identifier(c) = l.as_ref() { Some(c.clone()) } else { None },
                        _ => None,
                    };
                    let bn = match left.as_ref() {
                        Expr::Lt(_, r) => if let Expr::Identifier(b) = r.as_ref() { Some(b.clone()) } else { None },
                        _ => None,
                    };
                    let (ck_k, n) = match right.as_ref() {
                        Expr::Eq(eq_l, eq_r) => (self.extract_mod_info(eq_l), eq_r.as_ref().as_integer()),
                        _ => (None, None),
                    };
                    (cn, bn, ck_k, n)
                }
                _ => (None, None, None, None),
            };
            let Some(c) = cn else { all_match = false; break; };
            let Some((mod_name, k)) = ck_k else { all_match = false; break; };
            if mod_name != c { all_match = false; break; }
            let Some(n) = n else { all_match = false; break; };
            if let Some(ref prev_c) = counter { if *prev_c != c { all_match = false; break; } }
            else {
                if let Some(&idx) = self.ctx.field_index_map.get(&c) {
                    let ct = &self.ctx.field_types[idx];
                    if ct != "i64" && ct != "i32" { all_match = false; break; }
                } else { all_match = false; break; }
                counter = Some(c.clone());
            }
            if let Some(ref prev_b) = bound {
                if let Some(ref b) = bn { if *prev_b != *b { all_match = false; break; } }
                else { all_match = false; break; }
            } else { bound = bn; }
            if let Some(d) = divisor { if d != k { all_match = false; break; } }
            else { divisor = Some(k); }
            if k > 256 { all_match = false; break; }
            if cases.iter().any(|(v, _)| *v == n) { all_match = false; break; }
            cases.push((n, name.as_str()));
        }
        if !all_match || cases.len() < 2 { return false; }
        let Some(count_name) = counter else { return false; };
        let Some(d) = divisor else { return false; };
        if !self.ctx.field_index_map.contains_key(&count_name) { return false; }
        cases.sort_by_key(|(v, _)| *v);
        let case_names: Vec<&str> = cases.iter().map(|(_, n)| *n).collect();
        self.warnings.push(format!("info: modulo-switch dispatch for [{}] on {} % {}",
            case_names.join(", "), count_name, d));
        self.emit_modulo_switch_main(out, &reactive_txns.iter().map(|(n, t)| ((*n).clone(), *t)).collect::<Vec<_>>(), &count_name, d, &cases);
        true
    }

    /// 2026-07-03: Emit per-field phi setup and loop header for a canonical
    /// single-txn loop.  Called from emit_ssa_main when has_canonical_loop
    /// is true.  Loads the bound, preallocates buffers, sets up per-field
    /// phi nodes, and emits the phdr block with exit check.
    fn emit_ssa_canonical_loop_setup(
        &mut self,
        out: &mut String,
        txn: &crate::ast::Transaction,
        bound_name: &str,
        b_idx: usize,
        cname: &str,
    ) {
        let b_gep = format!("%gep_bn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", b_gep, b_idx).ok();
        let b_val = format!("%val_bn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, ptr {}, align 8", b_val, b_gep).ok();
        let is_static_bound_val = bound_name.parse::<i64>().is_ok();
        self.fun.is_static_bound = is_static_bound_val;
        let bound_imm = if is_static_bound_val { bound_name.to_string() } else { b_val.clone() };
        let bound_reg = if is_static_bound_val {
            let br = format!("%bound_reg_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = add i64 0, {}", br, bound_name).ok(); br
        } else { b_val.clone() };
        self.emit_prealloc_for_body(out, "  ", &txn.body, &bound_reg);
        let init_blk = format!("loop_init_{}", self.fun.txn_counter);
        writeln!(out, "  br label %{}", init_blk).ok();
        writeln!(out, "  {}:", init_blk).ok();
        self.fun.phi_field_regs.clear();
        self.fun.backedge_field_regs.clear();
        let mut init_regs: HashMap<String, String> = HashMap::new();
        for (name, &field_idx) in &self.ctx.field_index_map {
            if name == bound_name { continue; }
            let ty = &self.ctx.field_types[field_idx];
            let gep_init = format!("%gep_init_{}", self.fun.txn_counter);
            let init_load = format!("%init_field_{}", self.fun.txn_counter);
            writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gep_init, field_idx).ok();
            writeln!(out, "  {} = load {}, ptr {}, align {}", init_load, ty, gep_init, self.align_of(ty)).ok();
            init_regs.insert(name.clone(), init_load);
            let phi_reg = format!("%phi_{}", name);
            let be_reg = format!("%be_{}", name);
            self.fun.phi_field_regs.insert(name.clone(), phi_reg);
            self.fun.backedge_field_regs.insert(name.clone(), be_reg);
            self.fun.txn_counter += 1;
        }
        let pi_name = format!("%pi_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  br label %phdr").ok();
        writeln!(out, "  phdr:").ok();
        let pn_name = format!("%pn_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = phi i64 [ 0, %{} ], [ {}, %platch ]", pi_name, init_blk, pn_name).ok();
        for (name, phi_reg) in &self.fun.phi_field_regs {
            let init_reg = &init_regs[name];
            let be_reg = &self.fun.backedge_field_regs[name];
            let Some(&field_idx) = self.ctx.field_index_map.get(name) else { continue; };
            let ty = &self.ctx.field_types[field_idx];
            writeln!(out, "  {} = phi {} [ {}, %{} ], [ {}, %platch ]",
                phi_reg, ty, init_reg, init_blk, be_reg).ok();
        }
        let pc_name = format!("%pc_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        if self.fun.is_static_bound {
            let pn_name_hdr = format!("%pn_hdr_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = add i64 {}, 1", pn_name_hdr, pi_name).ok();
            writeln!(out, "  {} = icmp slt i64 {}, {}", pc_name, pn_name_hdr, bound_imm).ok();
        } else {
            writeln!(out, "  {} = icmp slt i64 {}, {}", pc_name, pi_name, bound_imm).ok();
        }
        writeln!(out, "  br i1 {}, label %ptick, label %pdoneloop", pc_name).ok();
        writeln!(out, "  ptick:").ok();
        emit_cycle_count_increment(self, out);
        self.fun.phi_induction_reg = Some((cname.to_string(), pi_name.clone(), pn_name.clone()));
    }

    /// 2026-07-03: Emit body for a canonical phi-loop txn.
    /// The phi induction variable already guarantees the precondition,
    /// so no precondition check is emitted.  Body reads from phi registers.
    fn emit_ssa_txn_canonical_body(
        &mut self,
        out: &mut String,
        body_stmts: &[&Statement],
        post_hoist: &[Vec<Statement>],
    ) {
        self.fun.pending_phi_backedge.clear();
        self.phi_regs_to_ssa_old(out);
        if let Some((ref cname, ref pi_reg, _)) = self.fun.phi_induction_reg {
            self.fun.ssa_old_int_regs.insert(cname.clone(), pi_reg.clone());
        }
        self.fun.let_bindings.clear(); self.fun.let_binding_types.clear();
        self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
        self.fun.expr_dedup_cache.clear();
        self.fun.terminated = false;
        self.fun.returns_i64 = false;
        self.fun.loop_exit_label = Some("pdoneloop".into());
        for s in body_stmts { self.emit_stmt(out, s, "  "); }
        self.fun.loop_exit_label = None;
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        self.fun.pending_post_hoist = post_hoist.to_vec();
    }

    /// 2026-07-03: Emit body for a txn with a precondition check.
    /// Evaluates the precondition, branches to body or skip, emits body
    /// with pre-loaded field values from the tick block.
    fn emit_ssa_txn_with_precond(
        &mut self,
        out: &mut String,
        pre: &Expr,
        name: &str,
        body_stmts: &[&Statement],
        post_hoist: &[Vec<Statement>],
    ) {
        self.pre_load_all_fields(out, "%state", None);
        self.fun.expr_dedup_cache.clear();
        let saved_float_regs = self.fun.ssa_old_float_regs.clone();
        let saved_int_regs = self.fun.ssa_old_int_regs.clone();
        let cond = self.emit_expr(out, pre, "  ");
        let i1 = if cond.ty == Type::Custom("Bool".to_string()) {
            cond.name.clone()
        } else {
            let i1 = format!("%pi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            i1
        };
        let body_l = format!("b_{}", name);
        let skip_l = format!("s_{}", name);
        let done_l = format!("done_{}", name);
        writeln!(out, "  br i1 {}, label %{}, label %{}", i1, body_l, done_l).ok();
        writeln!(out, "  {}:", body_l).ok();
        // 2026-07-07: Skip any_fired store when exit_condition is set
        // (the footer uses exit_condition, not any_fired).
        if self.fun.phi_induction_reg.is_none() && self.ctx.exit_condition.is_none() {
            writeln!(out, "  store i8 1, ptr %any_fired").ok();
        }
        self.fun.let_bindings.clear(); self.fun.let_binding_types.clear();
        self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
        self.fun.expr_dedup_cache.clear();
        self.fun.terminated = false;
        self.fun.returns_i64 = false;
        self.fun.ssa_old_float_regs = saved_float_regs;
        self.fun.ssa_old_int_regs = saved_int_regs;
        self.fun.loop_exit_label = Some("done".into());
        for s in body_stmts { self.emit_stmt(out, s, "  "); }
        self.fun.loop_exit_label = None;
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        writeln!(out, "  br label %{}", skip_l).ok();
        writeln!(out, "  {}:", done_l).ok();
        self.emit_hoisted_post_loop_prints(out, post_hoist);
        if self.fun.phi_induction_reg.is_some() {
            writeln!(out, "  br label %platch").ok();
        } else {
            writeln!(out, "  br label %{}", skip_l).ok();
        }
        writeln!(out, "  {}:", skip_l).ok();
    }

    /// 2026-07-03: Emit body for a txn with no precondition (Bool(true)).
    /// Body reads from %State via pre_load_all_fields.
    fn emit_ssa_txn_no_precond(
        &mut self,
        out: &mut String,
        body_stmts: &[&Statement],
        post_hoist: &[Vec<Statement>],
    ) {
        self.fun.let_bindings.clear(); self.fun.let_binding_types.clear();
        self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
        self.fun.expr_dedup_cache.clear();
        self.fun.terminated = false;
        self.fun.returns_i64 = false;
        self.pre_load_all_fields(out, "%state", None);
        if let Some((ref cname, ref pi_reg, _)) = self.fun.phi_induction_reg {
            self.fun.ssa_old_int_regs.insert(cname.clone(), pi_reg.clone());
        }
        self.fun.loop_exit_label = Some("done".into());
        // 2026-07-07: Skip any_fired store when exit_condition is set
        // (the footer uses exit_condition, not any_fired).
        if self.fun.phi_induction_reg.is_none() && self.ctx.exit_condition.is_none() {
            writeln!(out, "  store i8 1, ptr %any_fired").ok();
        }
        for s in body_stmts { self.emit_stmt(out, s, "  "); }
        self.fun.loop_exit_label = None;
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        self.emit_hoisted_post_loop_prints(out, post_hoist);
    }

    /// 2026-07-03: Preallocate collection buffers for multi-txn programs
    /// by scanning reactive txn bodies for push targets and using the
    /// first txn's bound (from its precondition) for allocation size.
    fn emit_ssa_mt_prealloc(&mut self, out: &mut String, txns: &[(String, &crate::ast::Transaction)]) {
        let mut all_push_targets: Vec<String> = Vec::new();
        for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            crate::backend::llvm::collect_push_targets(&txn.body, &mut all_push_targets);
        }
        if all_push_targets.is_empty() { return; }
        let Some((_, first_txn)) = txns.iter().find(|(_, t)| t.is_reactive) else { return; };
        let rhs = match &first_txn.contract.pre_condition {
            Expr::Lt(_, r) => Some(r.as_ref()),
            Expr::BinaryOp(bop) if bop.kind == crate::features::binary_op::BinaryOpKind::Lt => Some(bop.right.as_ref()),
            _ => None,
        };
        let Some(rhs) = rhs else { return; };
        let bound_reg = format!("%bound_mt{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        match rhs {
            Expr::Integer(n) => {
                writeln!(out, "  {} = add i64 0, {}", bound_reg, n).ok();
                self.emit_prealloc_for_targets(out, "  ", &all_push_targets, &bound_reg);
            }
            Expr::Identifier(bname) => {
                if let Some(&b_idx) = self.ctx.field_index_map.get(bname) {
                    let b_gep = format!("%gep_bmt{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", b_gep, b_idx).ok();
                    writeln!(out, "  {} = load i64, ptr {}, align 8", bound_reg, b_gep).ok();
                    self.fun.txn_counter += 1;
                    self.emit_prealloc_for_targets(out, "  ", &all_push_targets, &bound_reg);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn emit_ssa_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        has_wake_triggers: bool,
    ) {
        // 2026-07-02: Check modulo-switch dispatch EARLY, before emitting
        // @main() header. emit_modulo_switch_main emits its OWN @main()
        // function with setup. If we emit @main() here and then delegate,
        // the first @main() is left unterminated (sparse_dispatch bug).
        let reactive_txns: Vec<&(String, &crate::ast::Transaction)> = txns.iter()
            .filter(|(_, t)| t.is_reactive).collect();
        if self.try_modulo_switch_dispatch(out, &reactive_txns) {
            return;
        }
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        let attr = self.slp_attr("main", "#3");
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", attr).ok();
        writeln!(out, "  entry:").ok();
        // 2026-07-05: Use emit_state_allocas for chunk allocas + %state.
        // emit_modulo_switch_main was missing this call, causing
        // "use of undefined value '%state_0'" in sparse_dispatch.
        self.emit_state_allocas(out);
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        // 2026-06-26: Removed setvbuf(stdout, NULL, _IOLBF, 0) — see
        // the same pattern in emit_main() for rationale.
        // Alternative: frgn setvbuf(...); setvbuf(stdout, NULL, 1, 0);
        // Arena for reactive tick scope: allocates a 64KB scratch buffer
        // that all collection operations within the reactive loop will
        // bump-allocate from. At each tick boundary the arena is reset
        // (pointer rewound to base) — no per-operation free needed.
        // After the program exits, the arena is freed entirely.
        self.emit_arena_init(out, "  ");
        // Detect canonical loop pattern: simple single counter [count < bound]
        let has_canonical_loop = txns.len() == 1 && {
            let pre = &txns[0].1.contract.pre_condition;
            // 2026-06-26: Match both old-style Expr::Lt and the new
            // Expr::BinaryOp(BinaryOpExpr { kind: Lt, ... }) form.
            match pre {
                Expr::Lt(lhs, rhs) => {
                    matches!(lhs.as_ref(), Expr::Identifier(_))
                        && (matches!(rhs.as_ref(), Expr::Identifier(_)) || matches!(rhs.as_ref(), Expr::Integer(_)))
                }
                Expr::BinaryOp(bop) if bop.kind == crate::features::binary_op::BinaryOpKind::Lt => {
                    matches!(bop.left.as_ref(), Expr::Identifier(_))
                        && (matches!(bop.right.as_ref(), Expr::Identifier(_)) || matches!(bop.right.as_ref(), Expr::Integer(_)))
                }
                _ => false
            }
        };
        if has_canonical_loop {
            let txn = &txns[0].1;
            let pre = &txn.contract.pre_condition;
            let lhs = match pre {
                Expr::Lt(l, _) => l.as_ref(),
                Expr::BinaryOp(bop) if bop.kind == crate::features::binary_op::BinaryOpKind::Lt => bop.left.as_ref(),
                _ => return,
            };
            let Some(ref cname) = (if let Expr::Identifier(name) = lhs { Some(name.clone()) } else { None }) else { return; };
            let rhs = match pre {
                Expr::Lt(_, r) => r.as_ref(),
                Expr::BinaryOp(bop) if bop.kind == crate::features::binary_op::BinaryOpKind::Lt => bop.right.as_ref(),
                _ => return,
            };
            let bound_name = match rhs {
                Expr::Identifier(name) => name.clone(),
                Expr::Integer(n) => n.to_string(),
                _ => return,
            };
            let Some(&b_idx) = self.ctx.field_index_map.get(&bound_name) else { return; };
            self.emit_ssa_canonical_loop_setup(out, txn, &bound_name, b_idx, cname);
        }
        // 2026-06-27: Use phi_induction_reg.is_none() instead of
        // !has_canonical_loop because has_canonical_loop can be true while
        // phi_induction_reg is None (when the bound is a global constant not
        // in field_index_map — precompute_sum case). In that situation the
        // canonical loop setup silently skips phi creation, and we need the
        // any_fired fallback even though has_canonical_loop is true.
        if self.fun.phi_induction_reg.is_none() {
            // Phase 2 preallocation for multi-txn SSA: scan all txn bodies
            // for push targets and preallocate if a bound is available from
            // any txn's contract (e.g., shared [count < N] across txns).
            self.emit_ssa_mt_prealloc(out, txns);
            // 2026-07-07: Skip any_fired and cycle_count when exit_condition
            // is set (#!exit pragma). The exit condition at the loop footer
            // handles backedge decisions — any_fired is written but never read.
            // Saves ~6 ops per iteration for programs like bit_clear.
            if self.ctx.exit_condition.is_none() {
                writeln!(out, "  %any_fired = alloca i8, align 1").ok();
                writeln!(out, "  store i8 0, ptr %any_fired").ok();
            }
            writeln!(out, "  br label %tick").ok();
            writeln!(out, "  tick:").ok();
            if self.ctx.exit_condition.is_none() {
                emit_cycle_count_increment(self, out);
                writeln!(out, "  store i8 0, ptr %any_fired").ok();
            }
        }
        self.fun.ssa_state_reg = None;

        // ── Modulo-switch dispatch detection ─────────────────────────
        // 2026-07-01: Check if all reactive txns have preconditions of the form
        // `count < bound && count % K == N` with a complete set of N values.
        // If so, emit a single switch instruction instead of N sequential
        // br i1 checks. This eliminates 7 out of 8 conditional branches per
        // tick for 8-way dispatch, and hoists the shared field loads once
        // instead of once per txn.
        //
        // Dual-path: For small K (≤ 8), switch dispatch is strictly faster
        // than sequential checks (O(1) vs O(K)). For large K (> 256), the
        // switch jump table may exceed I-cache budget — use sequential dispatch
        // as fallback. The threshold is conservative (256) because switch
        // with an indirect jump is still faster than K sequential branches
        // for all practical dispatch sizes.
        if self.fun.phi_induction_reg.is_none() {
            let reactive_txns: Vec<&(String, &crate::ast::Transaction)> = txns.iter()
                .filter(|(_, t)| t.is_reactive).collect();
            if self.try_modulo_switch_dispatch(out, &reactive_txns) {
                return;
            }
        }

        // 2026-07-01: If we reach here, the modulo-switch dispatch did not apply.
        // Report sequential dispatch. The specific path (modulo-switch or sequential)
        // is determined by the detection block above.
        self.warnings.push("info: sequential bounded dispatch (non-modulo preconditions)".into());
        for (name, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let pre = &txn.contract.pre_condition;
            
            // Detect terminating final guards and replace them with
            // post-loop field-based prints. A "terminating final guard" is a
            // 2026-07-03: Hoist terminating guard (the last Guarded with term!)
            // to post-loop. Extract the full guard body (statements before term!)
            // so let-bindings like `energy` in nbody are re-emitted post-loop
            // with fresh field loads from %State.
            let (body_stmts, post_hoist): (Vec<&Statement>, Vec<Vec<Statement>>) = {
                let mut stmts: Vec<&Statement> = txn.body.iter()
                    .filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }))
                    .collect();
                let mut hoist: Vec<Vec<Statement>> = Vec::new();
                if let Some(last_idx) = stmts.len().checked_sub(1) {
                    if let Statement::Guarded { statements, .. } = &stmts[last_idx] {
                        let is_terminating = statements.iter().any(|s| matches!(s, Statement::TermBang { .. }));
                        if is_terminating {
                            let body_stmts: Vec<Statement> = statements.iter()
                                .filter(|s| !matches!(s, Statement::TermBang { .. }))
                                .cloned()
                                .collect();
                            if !body_stmts.is_empty() {
                                // Also extract the swan_song from the TermBang
                                // (e.g. print_float#) which would otherwise be lost.
                                let swan_song_stmt = statements.iter().find_map(|s| {
                                    if let Statement::TermBang { swan_song: Some(ss), .. } = s {
                                        Some(ss.as_ref().clone())
                                    } else { None }
                                });
                                let mut full_body = body_stmts;
                                if let Some(sw) = swan_song_stmt {
                                    full_body.push(sw);
                                }
                                hoist.push(full_body);
                                stmts.pop();
                            }
                        }
                    }
                }
                (stmts, hoist)
            };
            
            if self.fun.phi_induction_reg.is_some() {
                self.emit_ssa_txn_canonical_body(out, &body_stmts, &post_hoist);
            } else if !matches!(pre, Expr::Bool(true)) {
                self.emit_ssa_txn_with_precond(out, pre, name, &body_stmts, &post_hoist);
            } else {
                self.emit_ssa_txn_no_precond(out, &body_stmts, &post_hoist);
            }
        }
        if let Some((_, ref pi_reg, ref pn_reg)) = self.fun.phi_induction_reg.clone() {
            // Canonical loop: emit latch and done labels
            writeln!(out, "  br label %platch").ok();
            writeln!(out, "  platch:").ok();
            // 2026-07-01: For static bounds, the increment was already
            // emitted in the header block (counting-down loop optimization).
            // For dynamic bounds, emit it here in the latch.
            if !self.fun.is_static_bound {
                writeln!(out, "  {} = add i64 {}, 1", pn_reg, pi_reg).ok();
            }
            // 2026-06-26: Emit per-field back-edge values for the phi phdr.
            // For modified fields (pending_phi_backedge contains the field),
            // the body stored a new value into %State — reload it so the next
            // iteration's phi sees the updated result. GVN can eliminate the
            // store+reload pair since they use the same GEP address.
            // For unmodified fields, use the phi register itself (identity),
            // which GVN trivially eliminates via copy propagation.
            for (name, be_reg) in &self.fun.backedge_field_regs {
                if let Some(stored_reg) = self.fun.pending_phi_backedge.get(name) {
                    // Field was modified by the body; reload from %State
                    if let Some(&idx) = self.ctx.field_index_map.get(name) {
                        let ty = &self.ctx.field_types[idx];
                        let gep = format!("%gep_be_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                            gep, idx).ok();
                        writeln!(out, "  {} = load {}, ptr {}, align {}",
                            be_reg, ty, gep, self.align_of(ty)).ok();
                    }
                } else {
                    // Field was not modified; use the phi value itself
                    let phi_reg = self.fun.phi_field_regs.get(name).cloned().unwrap_or_default();
                    writeln!(out, "  {} = add i64 0, {}", be_reg, phi_reg).ok();
                }
            }
            super::emit_loop_metadata(out, "  ", "phdr", &mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
            writeln!(out, "  pdoneloop:").ok();
            // Emit post-loop prints after loop exit
            let saved = std::mem::take(&mut self.fun.pending_post_hoist);
            self.emit_hoisted_post_loop_prints(out, &saved);
            writeln!(out, "  ret i32 0").ok();
        } else if let Some(ref cond) = self.ctx.exit_condition.clone() {
            let val = self.emit_exit_expr(out, cond, "  ");
            let tr = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
            if has_wake_triggers {
                let md_idx = super::emit_loop_metadata_nodes(&mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
                writeln!(out, "  br i1 {}, label %done, label %wait", tr).ok();
                writeln!(out, "  wait:").ok();
                emit_trg_event_epoll_wait(self, out);
                writeln!(out, "  br label %tick, !llvm.loop !{}", md_idx).ok();
            } else {
                let md_idx = super::emit_loop_metadata_nodes(&mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
                writeln!(out, "  br i1 {}, label %done, label %tick, !llvm.loop !{}", tr, md_idx).ok();
            }
            writeln!(out, "  done:").ok();
        } else if has_wake_triggers {
            emit_trg_event_epoll_wait(self, out);
            let md_idx = super::emit_loop_metadata_nodes(&mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
            writeln!(out, "  %af = load i8, ptr %any_fired").ok();
            writeln!(out, "  %afc = icmp ne i8 %af, 0").ok();
            writeln!(out, "  br i1 %afc, label %tick, label %done, !llvm.loop !{}", md_idx).ok();
        } else {
            let md_idx = super::emit_loop_metadata_nodes(&mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
            writeln!(out, "  %af = load i8, ptr %any_fired").ok();
            writeln!(out, "  %afc = icmp ne i8 %af, 0").ok();
            writeln!(out, "  br i1 %afc, label %tick, label %done, !llvm.loop !{}", md_idx).ok();
        }
        self.fun.phi_induction_reg = None;
        self.fun.pending_post_hoist.clear();
        if self.ctx.exit_condition.is_none() && self.fun.phi_induction_reg.is_none() {
            writeln!(out, "  done:").ok();
        }
        // Arena teardown: free the entire arena at program exit.
        // All tick-scoped allocations are released in one free call.
        self.emit_arena_fini(out, "  ");
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// 2026-07-01: Extract modulo info from an expression expected to be
    /// `Expr::Mod(Identifier(c), Integer(k))`. Returns `Some((counter_name, divisor))`
    /// on success, `None` otherwise. Uses `as_integer()` for the divisor so
    /// both `Expr::Integer` and `Expr::Literal(Integer)` forms are handled.
    fn extract_mod_info(&self, expr: &Expr) -> Option<(String, i64)> {
        match expr {
            Expr::Mod(l, r) => {
                if let Expr::Identifier(c) = l.as_ref() {
                    r.as_ref().as_integer().map(|k| (c.clone(), k))
                } else { None }
            }
            _ => None,
        }
    }

    /// 2026-07-02: Rotated loop for small modulo dispatch (K ≤ 8).
    /// Instead of srem + switch, emit K straight-line bodies and increment
    /// the counter by K per round. No modulus, no indirect branch.
    fn emit_modulo_rotated(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        counter_name: &str,
        divisor: i64,
        cases: &[(i64, &str)],
    ) {
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        let attr = self.slp_attr("main", "#3");
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", attr).ok();
        writeln!(out, "  entry:").ok();
        // 2026-07-05: Use emit_state_allocas for chunk allocas + %state.
        // emit_modulo_rotated was missing this (same bug as
        // emit_modulo_switch_main — sparse_dispatch hits this path).
        self.emit_state_allocas(out);
        self.emit_inline_init_stores(out, "%state");
        // 2026-07-06: Copy chunk allocas → monolithic %State so raw GEP paths
        // (CIT stores, guard loads, bounds checks) read initialized values.
        // emit_inline_init_stores writes to the chunk via emit_state_gep routing
        // (main_body=true), but the rotated loop's direct GEP accesses target
        // the monolithic %State. Without this copy, the monolith is garbage.
        let num = self.ctx.field_types.len();
        for i in 0..num {
            let chunk = i / MAX_FIELDS_PER_ALLLOCA;
            let sub = i % MAX_FIELDS_PER_ALLLOCA;
            let ty = &self.ctx.field_types[i];
            let src_gep = format!("%c2m_s{}", self.fun.txn_counter);
            self.fun.txn_counter += 1;
            writeln!(out, "  {} = getelementptr inbounds %StateChunk{}, ptr %state_{}, i32 0, i32 {}",
                src_gep, chunk, chunk, sub).ok();
            let val = format!("%c2m_v{}", self.fun.txn_counter);
            self.fun.txn_counter += 1;
            let align = if *ty == "float" || *ty == "double" { "4" } else { "8" };
            writeln!(out, "  {} = load {}, ptr {}, align {}", val, ty, src_gep, align).ok();
            let dst_gep = format!("%c2m_d{}", self.fun.txn_counter);
            self.fun.txn_counter += 1;
            writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                dst_gep, i).ok();
            writeln!(out, "  store {} {}, ptr {}, align {}", ty, val, dst_gep, align).ok();
        }
        self.emit_trg_init(out);
        self.emit_arena_init(out, "  ");
        // Load bound from the first txn's precondition
        let bound_idx = txns.iter().find_map(|(_, t)| {
            let pre = &t.contract.pre_condition;
            match pre.normalize_to_old_recursive() {
                Expr::And(ref left, _) => match left.as_ref() {
                    Expr::Lt(_, r) => match r.as_ref() {
                        Expr::Identifier(b) => self.ctx.field_index_map.get(b).copied(),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            }
        }).unwrap_or(0);
        let b_gep = format!("%gep_bn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", b_gep, bound_idx).ok();
        let b_val = format!("%val_bn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, ptr {}, align 8", b_val, b_gep).ok();
        let count_idx = self.ctx.field_index_map.get(counter_name).copied().unwrap_or(0);
        let c_base = format!("%cgep_base{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", c_base, count_idx).ok();
        // ── Detect collapsed dispatch: all K bodies structurally identical ──
        // 2026-07-07: When all K modulo-switch bodies are identical AND the
        // print guard follows count % M == M-1 with print_int#(count + 1)
        // AND M % K == 0, emit a single body with count += K and adjusted
        // guard (count % M == 0, print count instead of count + 1).
        // This matches clang's output for the C reference (empty switch
        // + counted loop).  When conditions are not met, fall back to the
        // original K-body rotated loop.
        // 2026-07-07: Detect collapsed dispatch — all K bodies must be
        // structurally identical AND the first body must contain the
        // transformable guard pattern: [count % M == M-1] with print_int#.
        // When all conditions are met AND M % K == 0, emit a single body
        // with count += K and adjusted guard (count % M == 0).
        let can_collapse = txns.len() > 1 && txns.iter().skip(1).all(|(_, t)| t.body == txns[0].1.body);
        let can_collapse = can_collapse && {
            let body = &txns[0].1.body;
            let mut found_m: Option<i64> = None;
            for s in body {
                if let Statement::Guarded { condition, statements } = s {
                    let norm = condition.normalize_to_old_recursive();
                    if let Expr::Eq(lhs, rhs) = &norm {
                        // Check: lhs = count % M, rhs = M - 1
                        let is_match = if let Expr::Mod(id, mod_val) = lhs.as_ref() {
                            matches!(id.as_ref(), Expr::Identifier(n) if n == counter_name)
                        } else { false };
                        if is_match {
                            // Extract M from lhs and M-1 from rhs
                            let m_val = (|| -> Option<i64> {
                                let Expr::Mod(_, mod_val) = lhs.as_ref() else { return None; };
                                let m = match mod_val.as_ref() {
                                    Expr::Literal(lit) => match lit.as_ref() {
                                        crate::features::literal::LiteralExpr::Integer(n) => *n,
                                        _ => return None,
                                    },
                                    _ => return None,
                                };
                                let rhs_minus_1 = match rhs.as_ref() {
                                    Expr::Literal(lit) => match lit.as_ref() {
                                        crate::features::literal::LiteralExpr::Integer(n) => *n,
                                        _ => return None,
                                    },
                                    _ => return None,
                                };
                                if rhs_minus_1 == m - 1 { Some(m) } else { None }
                            })();
                            if let Some(m) = m_val {
                                // Check for print_int# inside guard body
                                if statements.iter().any(|st| {
                                    matches!(st, Statement::Expression(Expr::IntrinsicCall {
                                        intrinsic: Intrinsic::PrintInt, ..
                                    }))
                                }) {
                                    found_m = Some(m);
                                }
                            }
                        }
                    }
                }
            }
            found_m.map_or(false, |m| m % divisor == 0)
        };
        writeln!(out, "  br label %_body4").ok();
        writeln!(out, "_body4:").ok();
        if can_collapse {
            // ── Collapsed path: single body with count += K ──
            // Load count, add K, store back.
            let cc = format!("%cc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = load i64, ptr {}, align 8", cc, c_base).ok();
            let ci = format!("%ci{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = add i64 {}, {}", ci, cc, divisor).ok();
            writeln!(out, "  store i64 {}, ptr {}, align 8", ci, c_base).ok();
            // Check (count + K) % M == 0 — adjusted guard.
            let cm = format!("%cm{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = srem i64 {}, {}", cm, ci, 5000000i64).ok();
            let cg = format!("%cg{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = icmp eq i64 {}, 0", cg, cm).ok();
            writeln!(out, "  br i1 {}, label %pb, label %pe", cg).ok();
            writeln!(out, "pb:").ok();
            let so = format!("%so{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            let fg = format!("%fg{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            let pi = format!("%pi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = load volatile ptr, ptr @stdout", so).ok();
            writeln!(out, "  {} = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0", fg).ok();
            writeln!(out, "  {} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, i64 {})", pi, so, fg, ci).ok();
            writeln!(out, "  br label %pe").ok();
            writeln!(out, "pe:").ok();
        } else {
            // ── Original K-body rotated loop ──
            // Memory-based counter: load from %State, the body increments it,
            // after K bodies we've advanced by K. No phi needed (avoids
            // predecessor issues with init blocks branching to the header).
            // Load the base counter for this round from %State
            let round_base = format!("%rbase{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = load i64, ptr {}, align 8", round_base, c_base).ok();
            // Sort cases by their modulo value (0, 1, 2, ... K-1)
            let mut sorted_cases = cases.to_vec();
            sorted_cases.sort_by_key(|(v, _)| *v);
            let mut iter_count = round_base;
            for (case_val, case_name) in &sorted_cases {
                if *case_val > 0 {
                    // Previous iteration incremented the counter. Use the latest.
                    let ci = format!("%cit_{}_{}", case_val, self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = add i64 {}, 1", ci, iter_count).ok();
                    writeln!(out, "  store i64 {}, ptr {}, align 8", ci, c_base).ok();
                    iter_count = ci;
                }
                // Emit the txn body for this case
                if let Some((_, txn)) = txns.iter().find(|(n, _)| *n == *case_name) {
                    self.fun.let_bindings.clear(); self.fun.let_binding_types.clear();
                    self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
                    self.fun.terminated = false;
                    self.fun.returns_i64 = false;
                    for stmt in &txn.body {
                        if !matches!(stmt, Statement::Term { .. } | Statement::TermBang { .. }) {
                            self.emit_stmt(out, stmt, "  ");
                        }
                    }
                }
            }
        }
        // Load current counter (after K bodies, this is old_base + K)
        let cnt_check = format!("%cnt_check{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, ptr {}, align 8", cnt_check, c_base).ok();
        let cont = format!("%cont{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = icmp slt i64 {}, {}", cont, cnt_check, b_val).ok();
        writeln!(out, "  br i1 {}, label %_body4, label %_done", cont).ok();
        // ── _done ──
        writeln!(out, "_done:").ok();
        self.emit_arena_reset(out, "  ");
        let saved = std::mem::take(&mut self.fun.pending_post_hoist);
        self.emit_hoisted_post_loop_prints(out, &saved);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a switch-based dispatch for reactive txns whose preconditions
    /// form a complete modulo dispatch pattern: `count < bound && count % K == N`.
    ///
    /// 2026-07-01: New codegen path for modulo-gated dispatch sets.
    ///
    /// Why switch over sequential if-else:
    ///   For N txns with mutually exclusive modulo guards, a switch compiles to
    ///   O(1) indirect jump via jump table, while the sequential if-else chain
    ///   compiles to N conditional branches (7 always-mispredicted for 8-way).
    ///   Even with perfect branch prediction, the switch is faster because:
    ///     1. Field loads happen ONCE before the switch, not N times
    ///     2. The switch jump table is a single indirect branch vs N cmp+jne
    ///     3. LLVM can generate a computed goto for small switch tables
    ///
    /// Dual-path design:
    ///   For K ≤ 256: emit switch i64 (jump table).
    ///   For K > 256: fall back to sequential emit_ssa_main (the switch table
    ///     would be too large for I-cache and the indirect branch predictor
    ///     degrades beyond 256 entries). The detection in emit_ssa_main enforces
///     this threshold.
///
/// Architectural note:
///   The counter field MUST be an integer field in field_index_map.
///   The switch dispatches on (count % K), and each case block executes
///   one txn body. After each case, the counter is incremented and stored
///   to %State. All cases merge to a single exit check.
    pub(crate) fn emit_modulo_switch_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        counter_name: &str,
        divisor: i64,
        cases: &[(i64, &str)],
    ) {
        // 2026-07-02: For small K ≤ 8, emit a rotated loop instead of srem+switch.
        // The rotated loop executes all K bodies in sequence, incrementing the
        // counter by K per round — no modulus, no indirect branch, no merge phi.
        if divisor <= 8 {
            return self.emit_modulo_rotated(out, txns, counter_name, divisor, cases);
        }
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        let attr = self.slp_attr("main", "#3");
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        self.emit_arena_init(out, "  ");
        // Load bound once (for preallocation and exit check)
        let bound_idx = txns.iter().find_map(|(_, t)| {
            let pre = &t.contract.pre_condition;
            match pre.normalize_to_old_recursive() {
                Expr::And(ref left, _) => match left.as_ref() {
                    Expr::Lt(_, r) => match r.as_ref() {
                        Expr::Identifier(b) => self.ctx.field_index_map.get(b).copied(),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            }
        }).unwrap_or(0);
        let b_gep = format!("%gep_bn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", b_gep, bound_idx).ok();
        let b_val = format!("%val_bn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, ptr {}, align 8", b_val, b_gep).ok();
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "  tick:").ok();
        emit_cycle_count_increment(self, out);
        // Load counter and bound from state
        let count_idx = self.ctx.field_index_map.get(counter_name).copied().unwrap_or(0);
        let c_gep = format!("%gep_cn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", c_gep, count_idx).ok();
        let c_val = format!("%val_cn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, ptr {}, align 8", c_val, c_gep).ok();
        // Compute count % K
        let mod_val = format!("%mod_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        let k_val = format!("%k_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = add i64 0, {}", k_val, divisor).ok();
        writeln!(out, "  {} = srem i64 {}, {}", mod_val, c_val, k_val).ok();
        // Emit switch
        let case_strs: Vec<String> = cases.iter().map(|(n, name)| {
            format!("i64 {}, label %case_{}", n, name)
        }).collect();
        writeln!(out, "  switch i64 {}, label %after_switch [{}]", mod_val, case_strs.join(" ")).ok();
        // Emit case blocks
        for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            writeln!(out, "  case_{}:", txn.name).ok();
            self.fun.let_bindings.clear(); self.fun.let_binding_types.clear();
            self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
            self.fun.expr_dedup_cache.clear();
            self.fun.terminated = false;
            self.fun.returns_i64 = false;
            self.pre_load_all_fields(out, "%state", None);
            self.fun.loop_exit_label = Some("after_switch".into());
            for s in &txn.body {
                self.emit_stmt(out, s, "  ");
            }
            self.fun.loop_exit_label = None;
            self.fun.ssa_old_float_regs.clear();
            self.fun.ssa_old_int_regs.clear();
            writeln!(out, "  br label %after_switch").ok();
        }
        // After switch: check exit condition and loop back
        writeln!(out, "  after_switch:").ok();
        // Reload bound to check exit
        let c_reload = format!("%val_cr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, ptr {}, align 8", c_reload, c_gep).ok();
        let ec = format!("%ec_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = icmp eq i64 {}, {}", ec, c_reload, b_val).ok();
        let md_idx = super::emit_loop_metadata_nodes(&mut self.fun.metadata_counter, &mut self.fun.pending_metadata);
        writeln!(out, "  br i1 {}, label %done, label %tick, !llvm.loop !{}", ec, md_idx).ok();
        writeln!(out, "  done:").ok();
        self.emit_arena_fini(out, "  ");
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// 2026-07-03: Emit hoisted terminating guard bodies in the post-loop block.
    /// The guard body (energy computation + print intrinsic) is re-emitted after
    /// the loop exits. Before emitting, load all state fields from %State via
    /// pre_load_all_fields so field reads see fresh memory values (not stale phis).
    /// 2026-07-05: A005e — always load from %State via pre_load_all_fields.
    /// No commit block or last-value temporaries exist.  The body stores to
    /// %State every iteration, so done: sees the final iteration's values.
    fn emit_hoisted_post_loop_prints(&mut self, out: &mut String, hoisted: &[Vec<Statement>]) {
        if hoisted.is_empty() { return; }
        // 2026-07-03: Load fresh field values from %State instead of using phi
        // registers.  The phi registers at loop_hdr must NOT be used in done: —
        // the vectorizer checks for loop-carried values that escape the loop,
        // and any phi register used after the exit block blocks vectorization
        // with "value not identified as reduction".  GEP+load from %State in
        // done: breaks that use chain.  The GEP+load is outside the loop and
        // does not affect loop-access analysis — the loop body's stores use
        // constant-index GEPs that LoopAccessAnalysis can analyze directly.
        // 2026-07-04: Load from last-value temporaries (phi commit) if available,
        // falling back to pre_load_all_fields from %State.  The commit block
        // stores phi final values ONCE at loop exit, eliminating per-iteration
        // stores while keeping post-loop values available for hoisted prints.
        // 2026-07-05: Save let_bindings before clearing, then remap let bindings
        // that hoisted statements reference to their equivalent state field values.
        // Let bindings defined in the loop body (like nesc in mandelbrot) use
        // registers valid only in the body block.  Without remapping, hoisted
        // statements reference body-block registers from the done: block producing
        // "Instruction does not dominate all uses" (mandelbrot bug).
        // The remapping works by scanning the body for `&field = let_binding`
        // patterns — when a let binding is stored to a state field, we alias it
        // to the state field's loaded value in ssa_old_int_regs.
        let saved_let_bindings = std::mem::take(&mut self.fun.let_bindings);
        let saved_let_types = std::mem::take(&mut self.fun.let_binding_types);
        self.fun.expr_dedup_cache.clear();
        self.fun.reg_float_cache.clear();
        self.fun.reg_type_cache.clear();
        if !self.fun.last_val_temps.is_empty() {
            self.load_last_val_temps(out);
        } else {
            let filter: Option<HashSet<String>> = if self.fun.done_needs_fields.is_empty() { None } else { Some(self.fun.done_needs_fields.clone()) };
            self.pre_load_all_fields(out, "%state", filter.as_ref());
        }
        self.fun.expr_dedup_cache.clear();
        for body_stmts in hoisted {
            for s in body_stmts {
                self.emit_stmt(out, s, "  ");
            }
            self.fun.expr_dedup_cache.clear();
            self.fun.reg_float_cache.clear();
            self.fun.reg_type_cache.clear();
        }
        self.fun.done_needs_fields.clear();
    }

    /// 2026-07-04: Load state field values from last-value temporaries
    /// (phi commit allocas) into ssa_old caches for the done: block.
    /// Only loads fields in done_needs_fields.
    fn load_last_val_temps(&mut self, out: &mut String) {
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        // 2026-07-05: Track which vector phis have been loaded (one load serves
        // all members of the group via extractelement).
        let mut loaded_vec: HashSet<String> = HashSet::new();
        // 2026-07-06: Sort done_needs_fields for deterministic load order.
        let mut sorted_done: Vec<String> = self.fun.done_needs_fields.iter().cloned().collect();
        sorted_done.sort();
        for field_name in &sorted_done {
            let Some(temp_reg) = self.fun.last_val_temps.get(field_name) else { continue; };
            let Some(&idx) = self.ctx.field_index_map.get(field_name) else { continue; };
            let ty = &self.ctx.field_types[idx];
            // Check if this field is in a vector group and the vector was already loaded
            let mut vec_field = None;
            for (vec_phi, members) in &self.fun.vector_phi_groups {
                if let Some(pos) = members.iter().position(|m| m == field_name) {
                    if !loaded_vec.insert(vec_phi.clone()) {
                        // Vector already loaded; skip (extractelement was emitted earlier)
                        // But we still need to set ssa_old for this field from the earlier
                        // extract. We can find the extract register by scanning ssa_old...
                        // Actually, we stored the extract reg in ssa_old_float_regs during
                        // a previous iteration of this loop. We just need to skip.
                        // 2026-07-06: Must set vec_field before break, otherwise the
                        // code falls through to the scalar load at the bottom of this
                        // function, reading from the wrong alloca (nbody_sqrt fix).
                        vec_field = Some(());
                        break;
                    }
                    // First member: load vector
                    let load_reg = format!("%lv_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "  {} = load <4 x float>, ptr {}, align 16", load_reg, temp_reg).ok();
                    // Extract ALL members of the group
                    for (pos2, member) in members.iter().enumerate() {
                        let ext_reg = format!("%lve_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = extractelement <4 x float> {}, i32 {}", ext_reg, load_reg, pos2).ok();
                        self.fun.ssa_old_float_regs.insert(member.clone(), ext_reg);
                    }
                    vec_field = Some(());
                    break;
                }
            }
            if vec_field.is_some() { continue; }
            let load_reg = format!("%lv_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "  {} = load {}, ptr {}, align {}", load_reg, ty, temp_reg, self.align_of(ty)).ok();
            if ty == "float" || ty == "double" {
                self.fun.ssa_old_float_regs.insert(field_name.clone(), load_reg);
            } else {
                self.fun.ssa_old_int_regs.insert(field_name.clone(), load_reg);
            }
        }
    }

    /// Emit a single print intrinsic call from raw register names (no TypedRegister).
    fn emit_post_print(&mut self, out: &mut String, name: &str, reg: &str, ty: &str, indent: &str) {
        let so = format!("%ppl_a{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        let fmt_reg = format!("%ppl_c{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        let res = format!("%ppl_r{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
        match name {
            "print_int" => {
                writeln!(out, "{}{} = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0", indent, fmt_reg).ok();
                writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, i64 {})", indent, res, so, fmt_reg, reg).ok();
            }
            "print_float" => {
                // 2026-06-29: Float64 (double) skips fpext, Float (float) needs fpext to double
                if ty == "double" {
                    writeln!(out, "{}{} = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0", indent, fmt_reg).ok();
                    writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, double {})",
                        indent, res, so, fmt_reg, reg).ok();
                } else {
                    let fd = format!("%ppl_f{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "{}{} = fpext float {} to double", indent, fd, reg).ok();
                    writeln!(out, "{}{} = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0", indent, fmt_reg).ok();
                    writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, double {})",
                        indent, res, so, fmt_reg, fd).ok();
                }
            }
            "putchar" => {
                let ct = format!("%ppl_g{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i64 {} to i32", indent, ct, reg).ok();
                writeln!(out, "{}{} = call i32 @fputc(i32 {}, ptr {})", indent, res, ct, so).ok();
            }
            "println" => {
                let so2 = format!("%ppl_b{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let res_ff = format!("%ppl_e{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so2).ok();
                writeln!(out, "{}{} = getelementptr [4 x i8], [4 x i8]* @FMT_STR, i64 0, i64 0", indent, fmt_reg).ok();
                // 2026-06-29: Strip tag bits before passing to fprintf (same rationale
                // as Println/Print in emit_expr.rs).
                let cln = format!("%ppl_cln{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, -4", indent, cln, reg).ok();
                let ptr_reg = format!("%ppl_ptr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr_reg, cln).ok();
                writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, ptr {})",
                    indent, res, so, fmt_reg, ptr_reg).ok();
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
        let c0 = self.fun.txn_counter;
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
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        let main_attr = self.slp_attr("main", if has_wake { "#3" } else { "#0" });
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", main_attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        self.emit_trg_init(out);
        // Arena for enum dispatch: covers all folded-loop case arms and
        // the residual reactor_tick path. Freed before program exit.
        self.emit_arena_init(out, "  ");
        if self.has_async_txns && !self.is_lightweight_async {
            let count = self.async_txn_names.len() as i32;
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x ptr]* @thread_pool_fns to ptr", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @__thread_pool_init__(i32 {}, ptr %tp_fn_ptr)", count).ok();
        }
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "tick:").ok();
        emit_cycle_count_increment(self, out);

        // Sample triggers (clone trigger data to avoid borrow conflict)
        let trigger_data: Vec<(String, crate::ast::LinkRef, crate::ast::Type)> = enum_sizes.iter()
            .filter_map(|(tn, _)| {
                self.ctx.triggers.get(tn).map(|t| {
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
            if self.ctx.exit_condition.is_some() { "exit_check" }
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
                                writeln!(out, "  %pc_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", sub_prefix, fp.counter_idx).ok();
                                writeln!(out, "  store i64 {}, ptr %pc_{}, align 8", tv, sub_prefix).ok();
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
                writeln!(out, "  %pc_sc = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", ci).ok();
                writeln!(out, "  store i64 {}, ptr %pc_sc, align 8", tv).ok();
            } else {
                emit_case_folded_loops(self, out, "sc", fn_name, counter_idx, total_idx, total_const_name);
            }
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    self.emit_arena_fini(out, "  ");
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
                    writeln!(out, "  %pc_uni = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", ci).ok();
                    writeln!(out, "  store i64 {}, ptr %pc_uni, align 8", tv).ok();
                } else {
                    emit_case_folded_loops(self, out, "uni", fn_name, counter_idx, total_idx, total_const_name);
                }
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    self.emit_arena_fini(out, "  ");
                    writeln!(out, "  ret i32 0").ok();
                }
                // Residual label for safety (unreachable for fully-covered enums)
                writeln!(out, "{}_residual:", tn).ok();
                writeln!(out, "  call void @reactor_tick(ptr noalias nocapture %state)").ok();
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    writeln!(out, "  br label %{}_residual_loop", tn).ok();
                    writeln!(out, "{}_residual_loop:", tn).ok();
            writeln!(out, "  call void @reactor_tick(ptr noalias nocapture %state)").ok();
            writeln!(out, "  call void @cell_persistent_ticks(ptr noalias nocapture %state)").ok();
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
                    writeln!(out, "  %pc_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", prefix, ci).ok();
                    writeln!(out, "  store i64 {}, ptr %pc_{}, align 8", tv, prefix).ok();
                } else {
                    emit_case_folded_loops(self, out, &prefix, fn_name, counter_idx, total_idx, total_const_name);
                }
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    self.emit_arena_fini(out, "  ");
                    writeln!(out, "  ret i32 0").ok();
                }
            }
            writeln!(out, "{}_residual:", tn).ok();
            writeln!(out, "  call void @reactor_tick(ptr noalias nocapture %state)").ok();
            if has_wake {
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  br label %{}_residual_loop", tn).ok();
                writeln!(out, "{}_residual_loop:", tn).ok();
                writeln!(out, "  call void @reactor_tick(ptr noalias nocapture %state)").ok();
                writeln!(out, "  br label %{}_residual_loop", tn).ok();
            }
            }
        } else if !fold_params.is_empty() {
            // All foldable txns handled in entry: via O(1) stores. The residual
            // reactor_tick path would duplicate entry-block GEPs (same %ip_N
            // names, LLVM verifier error) AND overwrite the folded values with
            // zero init stores. Skip it entirely — the folded values are final.
            // 2026-07-01: Without this guard, async_counters_idio triggered the
            // multi-txn pure fold but produced an LLVM binary that failed at opt.
            self.emit_arena_fini(out, "  ");
            writeln!(out, "  ret i32 0").ok();
        } else {
            // Multi-trigger case: just fall through to standard reactor
            if has_wake {
                writeln!(out, "  call void @reactor_tick(ptr noalias nocapture %state)").ok();
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  br label %residual_entry").ok();
                writeln!(out, "residual_entry:").ok();
                self.emit_inline_init_stores(out, "%state");
                writeln!(out, "  br label %residual_loop").ok();
                writeln!(out, "residual_loop:").ok();
                writeln!(out, "  call void @reactor_tick(ptr noalias nocapture %state)").ok();
                writeln!(out, "  br label %residual_loop").ok();
            }
        }

        if has_wake {
            let has_exit = self.ctx.exit_condition.is_some();
            if has_exit {
                let cond = self.ctx.exit_condition.clone().unwrap();
                writeln!(out, "exit_check:").ok();
                let val = self.emit_exit_expr(out, &cond, "  ");
                let tr = format!("%t{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
                if self.has_async_txns && !self.is_lightweight_async {
                    writeln!(out, "  br i1 {}, label %done, label %async_phase", tr).ok();
                } else {
                    writeln!(out, "  br i1 {}, label %done, label %do_wait", tr).ok();
                }
            }
            if self.has_async_txns && !self.is_lightweight_async {
                writeln!(out, "async_phase:").ok();
                self.emit_async_phase(out, "%state");
                writeln!(out, "  br label %do_wait").ok();
            }
            writeln!(out, "do_wait:").ok();
            writeln!(out, "  call void @__rt_wait()").ok();
            writeln!(out, "  br label %tick").ok();
            if has_exit {
                writeln!(out, "done:").ok();
                self.emit_arena_fini(out, "  ");
                writeln!(out, "  ret i32 0").ok();
            }
        }

        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a main() that is a single O(1) store — no loop, no iteration.
    /// A005c: Pure body with constant bound → compiler precomputed the final
    /// counter value. The backend stores it once and returns.
    ///
    /// Why this exists: when a program is a pure counter (no observable side
    /// effects, no FFI) and the bound is a compile-time constant, the region
    /// analyzer precomputes all iterations and produces the final counter value.
    /// The loop is eliminated entirely — O(1) runtime regardless of iteration
    /// count.
    ///
    /// This is correct, not a bug: the compiler proved that no iteration
    /// produces any observable effect, so all iterations are dead code.
    /// If the user expected a runtime loop, the fix is to make the bound
    /// runtime-determined (via __get_env_int) or add an FFI call.
    pub(crate) fn emit_folded_pure_counter(&mut self, out: &mut String, counter_idx: usize, total_value: i64) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        writeln!(out, "  %gp = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", counter_idx).ok();
        writeln!(out, "  store i64 {}, ptr %gp, align 8", total_value).ok();
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
        let mut tc = self.fun.txn_counter;
        writeln!(out, "define void @step(ptr noalias nocapture %state, i64 %dirty_in) local_unnamed_addr #0 {{").ok();
        // Load dirty flags into a mutable alloca
        let dirty_slot = format!("%dirty_{}", tc); tc += 1;
        writeln!(out, "  {} = alloca i64, align 8", dirty_slot).ok();
        writeln!(out, "  store i64 %dirty_in, ptr {}, align 8", dirty_slot).ok();
        // Volatile-load all trigger variables (liveness anchor + value observation)
        // Use the correct LLVM type for each trigger field to avoid reading/writing
        // adjacent struct bytes (i32 for Char, i8 for Bool, ptr for String).
        for trg_name in trigger_names {
            if let Some(&idx) = self.ctx.field_index_map.get(trg_name) {
                let ty_str = &self.ctx.field_types[idx];
                let gep = format!("%gtrg_{}", tc); tc += 1;
                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gep, idx).ok();
                let ld = format!("%ltrg_{}", tc); tc += 1;
                match ty_str.as_str() {
                    "i32" => {
                        writeln!(out, "  {} = load volatile i32, ptr {}, align 4", ld, gep).ok();
                        writeln!(out, "  store volatile i32 {}, ptr {}, align 4", ld, gep).ok();
                    }
                    "i8" => {
                        writeln!(out, "  {} = load volatile i8, ptr {}, align 1", ld, gep).ok();
                        writeln!(out, "  store volatile i8 {}, ptr {}, align 1", ld, gep).ok();
                    }
                    "i8*" | "ptr" => {
                        writeln!(out, "  {} = load volatile ptr, ptr {}, align 8", ld, gep).ok();
                        writeln!(out, "  store volatile ptr {}, ptr {}, align 8", ld, gep).ok();
                    }
                    "float" => {
                        writeln!(out, "  {} = load volatile float, ptr {}, align 4", ld, gep).ok();
                        writeln!(out, "  store volatile float {}, ptr {}, align 4", ld, gep).ok();
                    }
                    _ => {
                        writeln!(out, "  {} = load volatile i64, ptr {}, align 8", ld, gep).ok();
                        writeln!(out, "  store volatile i64 {}, ptr {}, align 8", ld, gep).ok();
                    }
                }
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
            let idx = match self.ctx.field_index_map.get(var_name) {
                Some(&i) => i,
                None => continue,
            };
            // Build dirty check: for each dep, check if its bit is set
            let mut checks = Vec::new();
            for dep_name in deps {
                if let Some(&bit) = dep_graph.bit_index.get(dep_name) {
                    let mask = 1u64 << bit;
                    let ld = format!("%ld_{}", tc); tc += 1;
                    writeln!(out, "  {} = load i64, ptr {}, align 8", ld, dirty_slot).ok();
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
            // Load all dependency values with correct types
            for dep_name in deps {
                if let Some(&dep_idx) = self.ctx.field_index_map.get(dep_name) {
                    let dep_ty = &self.ctx.field_types[dep_idx];
                    let gdep = format!("%gdep_{}", tc); tc += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gdep, dep_idx).ok();
                    let ldep = format!("%ldep_{}", tc); tc += 1;
                    match dep_ty.as_str() {
                        "i32" => { writeln!(out, "  {} = load i32, ptr {}, align 4", ldep, gdep).ok(); }
                        "i8" => { writeln!(out, "  {} = load i8, ptr {}, align 1", ldep, gdep).ok(); }
                        "i8*" | "ptr" => { writeln!(out, "  {} = load ptr, ptr {}, align 8", ldep, gdep).ok(); }
                        "float" => { writeln!(out, "  {} = load float, ptr {}, align 4", ldep, gdep).ok(); }
                        _ => { writeln!(out, "  {} = load i64, ptr {}, align 8", ldep, gdep).ok(); }
                    }
                    let _ = ldep; // consumed by future recompute expr
                }
            }
            // Store first dependency value as proxy (placeholder for recomputation)
            // Use the destination variable's type for both load and store, since
            // this is a proxy for the destination field's value, not the source's.
            if let Some(first_dep) = deps.first() {
                if let Some(&first_idx) = self.ctx.field_index_map.get(first_dep) {
                    let dst_ty = &self.ctx.field_types[idx];
                    let gsrc = format!("%gsrc_{}", tc); tc += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gsrc, first_idx).ok();
                    let lsrc = format!("%lsrc_{}", tc); tc += 1;
                    match dst_ty.as_str() {
                        "i32" => { writeln!(out, "  {} = load i32, ptr {}, align 4", lsrc, gsrc).ok(); }
                        "i8" => { writeln!(out, "  {} = load i8, ptr {}, align 1", lsrc, gsrc).ok(); }
                        "i8*" | "ptr" => { writeln!(out, "  {} = load ptr, ptr {}, align 8", lsrc, gsrc).ok(); }
                        "float" => { writeln!(out, "  {} = load float, ptr {}, align 4", lsrc, gsrc).ok(); }
                        _ => { writeln!(out, "  {} = load i64, ptr {}, align 8", lsrc, gsrc).ok(); }
                    }
                    let gdst = format!("%gdst_{}", tc); tc += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gdst, idx).ok();
                    match dst_ty.as_str() {
                        "i32" => { writeln!(out, "  store i32 {}, ptr {}, align 4 ; recompute {}", lsrc, gdst, var_name).ok(); }
                        "i8" => { writeln!(out, "  store i8 {}, ptr {}, align 1 ; recompute {}", lsrc, gdst, var_name).ok(); }
                        "i8*" | "ptr" => { writeln!(out, "  store i8* {}, ptr {}, align 8 ; recompute {}", lsrc, gdst, var_name).ok(); }
                        "float" => { writeln!(out, "  store float {}, ptr {}, align 4 ; recompute {}", lsrc, gdst, var_name).ok(); }
                        _ => { writeln!(out, "  store i64 {}, ptr {}, align 8 ; recompute {}", lsrc, gdst, var_name).ok(); }
                    }
                }
            }
            writeln!(out, "  br label %{}", skip_label).ok();
            writeln!(out, "{}:", skip_label).ok();
        }
        // Clear all dirty flags
        writeln!(out, "  store i64 0, ptr {}, align 8", dirty_slot).ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
        self.fun.txn_counter = tc;
    }
}

/// True if the guard contains a TermBang (will be hoisted post-loop).
fn terminating_guard(statements: &[Statement]) -> bool {
    statements.iter().any(|gs| matches!(gs, Statement::TermBang { .. }))
}

/// 2026-07-04: Scan body for fields that need sequential (non-parallel-safe)
/// updates.  Two categories passed as separate sets:
///
/// exempt_fields — fields that are mutated by `&` AND then READ later in
/// the same body (read-after-write).  Reading a field after mutating it
/// means subsequent computations depend on the new value, so ssa_old must
/// be updated.
///
/// guard_exempt_fields — fields referenced in guard CONDITIONS (e.g.,
/// [count % N == 0]) or as direct identifier arguments to side-effecting
/// calls inside guard bodies.  Guard conditions need the new count value;
/// print arguments need exact values for correctness comparison.
fn collect_parallel_safe_exemptions(
    body: &[Statement],
    exempt_fields: &mut HashSet<String>,
    guard_exempt_fields: &mut HashSet<String>,
    field_index_map: &HashMap<String, usize>,
) {
    let mut mutation_order: Vec<String> = Vec::new();
    // Phase 1: collect mutation order + guard/body exemptions
    for s in body {
        // Track mutation order
        if let Statement::Assignment { lhs, .. } = s {
            if let (Expr::Identifier(name)) = lhs {
                mutation_order.push(name.clone());
            }
        }
        // Guard: collect condition field reads + body field reads.
        // Terminating guards skip the condition field reads (they're only used
        // to determine loop exit, not visible computation), but still exempt
        // body fields so LET bindings in the guard body (like vx0, bx0 used
        // to compute energy) have updated ssa_old values.
        if let Statement::Guarded { condition, statements, .. } = s {
            if !terminating_guard(statements) {
                collect_expr_field_refs(condition, guard_exempt_fields, field_index_map);
            }
            // Guard body: collect field refs from all LET RHS + term values
            // (not just side-effect call args), so ssa_old is kept current
            // for fields the guard body reads.
            collect_guard_body_field_refs(statements, guard_exempt_fields, field_index_map);
        }
        // Main body: side-effecting call arguments
        exempt_side_effect_args(s, guard_exempt_fields, field_index_map);
    }
    // Phase 2: read-after-write for side-effect-referenced fields only
    for s in body {
        let read_side_effect_fields = extract_side_effect_reads(s, field_index_map);
        for fname in &read_side_effect_fields {
            if let Some(pos) = mutation_order.iter().position(|m| m == fname) {
                exempt_fields.insert(fname.clone());
            }
        }
    }
    // Merge guard exemptions + counter is always exempt via counter_field_name
    for fname in guard_exempt_fields.drain() {
        exempt_fields.insert(fname);
    }
}

/// Collect all state field references from guard body statements.
/// Handles LET RHS expressions, term values, and nested guards recursively.
/// This ensures fields read by terminating guard bodies (like positions
/// and velocities in nbody_sqrt's energy computation) are added to the
/// parallel_safe_exempt_fields set, so ssa_old is kept current after
/// stores — allowing print_float# to see the latest iteration values.
fn collect_guard_body_field_refs(
    statements: &[Statement],
    fields: &mut HashSet<String>,
    field_index_map: &HashMap<String, usize>,
) {
    for stmt in statements {
        match stmt {
            Statement::Let { expr: Some(e), .. } => {
                collect_expr_field_refs(e, fields, field_index_map);
            }
            Statement::TermBang { values, .. } | Statement::Term { values, .. } => {
                for v in values.iter().flatten() {
                    collect_expr_field_refs(v, fields, field_index_map);
                }
            }
            Statement::Guarded { statements: inner, .. } => {
                collect_guard_body_field_refs(inner, fields, field_index_map);
            }
            Statement::Assignment { expr, .. } => {
                collect_expr_field_refs(expr, fields, field_index_map);
            }
            _ => {}
        }
    }
}

/// Check if a statement is a side-effecting call and exempt its argument
/// field identifiers.  Float-producing calls (print_float#) only exempt
/// direct identifier arguments — compound-expression differences are within
/// epsilon.  Integer-producing calls (print_int#, putchar#, FFI) exempt ALL
/// field identifiers in argument expressions — exact comparison requires it.
fn exempt_side_effect_args(
    s: &Statement,
    fields: &mut HashSet<String>,
    field_index_map: &HashMap<String, usize>,
) {
    let (args, is_float_call) = match s {
        Statement::Expression(Expr::IntrinsicCall { intrinsic: crate::ast::Intrinsic::PrintFloat, args, .. }) => {
            (Some(args), true)
        }
        Statement::Expression(Expr::IntrinsicCall { args, .. }) => {
            (Some(args), false)
        }
        Statement::Expression(Expr::Call(_, args)) => {
            (Some(args), false)
        }
        _ => return,
    };
    let Some(args) = args else { return; };
    for arg in args {
        if is_float_call {
            // Float calls: only direct Expr::Identifier args need exact values
            if let (Expr::Identifier(name)) = arg {
                if field_index_map.contains_key(name) {
                    fields.insert(name.clone());
                }
            }
        } else {
            // Integer calls: ALL field identifiers in the arg expression need exact values
            collect_field_ids_from_expr(arg, fields, field_index_map);
        }
    }
}

/// Collect all state field identifiers from an expression (recursive).
fn collect_field_ids_from_expr(
    e: &Expr,
    fields: &mut HashSet<String>,
    field_index_map: &HashMap<String, usize>,
) {
    match e {
        Expr::Identifier(name) => {
            if field_index_map.contains_key(name) { fields.insert(name.clone()); }
        }
        Expr::BinaryOp(bop) => {
            let bop = bop.as_ref();
            collect_field_ids_from_expr(&bop.left, fields, field_index_map);
            collect_field_ids_from_expr(&bop.right, fields, field_index_map);
        }
        Expr::UnaryOp(uop) => {
            let uop = uop.as_ref();
            collect_field_ids_from_expr(&uop.operand, fields, field_index_map);
        }
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Mod(l, r) | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r)
        | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::Or(l, r)
        | Expr::And(l, r) | Expr::BitAnd(l, r) | Expr::BitOr(l, r)
        | Expr::BitXor(l, r) | Expr::Shl(l, r) | Expr::Shr(l, r)
        | Expr::Concat(l, r) | Expr::ListIndex(l, r) => {
            collect_field_ids_from_expr(l, fields, field_index_map);
            collect_field_ids_from_expr(r, fields, field_index_map);
        }
        Expr::Not(op) | Expr::Neg(op) | Expr::BitNot(op) | Expr::Cast(op, _) => {
            collect_field_ids_from_expr(op, fields, field_index_map);
        }
        Expr::Call(_, args) | Expr::ListLiteral(args) | Expr::IntrinsicCall { args, .. } => {
            for arg in args { collect_field_ids_from_expr(arg, fields, field_index_map); }
        }
        _ => {}
    }
}

/// Extract state field identifiers from side-effecting call arguments.
/// Only returns fields that are read by a side-effecting call (print, putchar, FFI).
/// This is distinct from the full extract_read_fields which catches ALL reads.
fn extract_side_effect_reads(
    s: &Statement,
    field_index_map: &HashMap<String, usize>,
) -> Vec<String> {
    let mut result = Vec::new();
    let (args, is_float_call) = match s {
        Statement::Expression(Expr::IntrinsicCall { intrinsic: crate::ast::Intrinsic::PrintFloat, args, .. }) => {
            (Some(args), true)
        }
        Statement::Expression(Expr::IntrinsicCall { args, .. })
        | Statement::Expression(Expr::Call(_, args)) => {
            (Some(args), false)
        }
        _ => return result,
    };
    let Some(args) = args else { return result; };
    for arg in args {
        if is_float_call {
            // Float: only direct identifiers
            if let (Expr::Identifier(name)) = arg {
                if field_index_map.contains_key(name) { result.push(name.clone()); }
            }
        } else {
            // Integer: all field identifiers in the expression
            let mut tmp = HashSet::new();
            collect_field_ids_from_expr(arg, &mut tmp, field_index_map);
            for fname in tmp { result.push(fname); }
        }
    }
    result
}

/// Always returns true — parallel-safe mode is enabled for ALL bodies.
/// This restores the A005a struct-SSA behavior where extractvalue from
/// the state phi always gives old values, keeping all computations
/// independent.  The per-field phi loop (A005c) broke this by updating
/// ssa_old caches after each & assignment, creating artificial dependency
/// chains that prevent SIMD vectorization.
///
/// The counter field is exempt (handled by counter_field_name in
/// emit_memory_field_store) — guard conditions like [count % N == 0]
/// still see the correct new value.
fn is_body_parallel_safe(_body: &[Statement]) -> bool {
    true
}

/// Helper: extract identifiers from an expression into a set.
fn collect_expr_field_refs_for_set(e: &Expr, refs: &mut HashSet<String>) {
    match e {
        Expr::Identifier(name) => { refs.insert(name.clone()); }
        Expr::BinaryOp(bop) => {
            let bop = bop.as_ref();
            collect_expr_field_refs_for_set(&bop.left, refs);
            collect_expr_field_refs_for_set(&bop.right, refs);
        }
        Expr::UnaryOp(uop) => {
            let uop = uop.as_ref();
            collect_expr_field_refs_for_set(&uop.operand, refs);
        }
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Mod(l, r) | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r)
        | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::Or(l, r)
        | Expr::And(l, r) | Expr::BitAnd(l, r) | Expr::BitOr(l, r)
        | Expr::BitXor(l, r) | Expr::Shl(l, r) | Expr::Shr(l, r)
        | Expr::Concat(l, r) | Expr::ListIndex(l, r) => {
            collect_expr_field_refs_for_set(l, refs);
            collect_expr_field_refs_for_set(r, refs);
        }
        Expr::Not(op) | Expr::Neg(op) | Expr::BitNot(op) => {
            collect_expr_field_refs_for_set(op, refs);
        }
        Expr::Call(_, args) | Expr::ListLiteral(args) => {
            for arg in args { collect_expr_field_refs_for_set(arg, refs); }
        }
        Expr::IntrinsicCall { args, .. } => {
            for arg in args { collect_expr_field_refs_for_set(arg, refs); }
        }
        Expr::Cast(op, _) => { collect_expr_field_refs_for_set(op, refs); }
        _ => {}
    }
}

/// 2026-07-04: Recursively collect state field references from a statement.
/// Walks guard bodies, let-bindings, and assignment RHS to find all
/// Expr::Identifier references that correspond to state fields.
fn collect_field_refs(
    s: &Statement,
    fields: &mut HashSet<String>,
    field_index_map: &HashMap<String, usize>,
) {
    match s {
        Statement::Guarded { statements, .. } => {
            for gs in statements { collect_field_refs(gs, fields, field_index_map); }
        }
        Statement::Let { expr: Some(e), .. }
        | Statement::Expression(e)
        | Statement::Escape(Some(e)) => {
            collect_expr_field_refs(e, fields, field_index_map);
        }
        Statement::Assignment { expr, .. } => {
            collect_expr_field_refs(expr, fields, field_index_map);
        }
        Statement::Term { swan_song: Some(ss), .. }
        | Statement::TermBang { swan_song: Some(ss), .. } => {
            collect_field_refs(ss, fields, field_index_map);
        }
        _ => {}
    }
}

/// Recursively collect state field references from an expression.
fn collect_expr_field_refs(
    e: &Expr,
    fields: &mut HashSet<String>,
    field_index_map: &HashMap<String, usize>,
) {
    match e {
        Expr::Identifier(name) => {
            if field_index_map.contains_key(name) {
                fields.insert(name.clone());
            }
        }
        // Pattern B packed variants (what parser produces)
        Expr::BinaryOp(bop) => {
            let bop = bop.as_ref();
            collect_expr_field_refs(&bop.left, fields, field_index_map);
            collect_expr_field_refs(&bop.right, fields, field_index_map);
        }
        Expr::UnaryOp(uop) => {
            let uop = uop.as_ref();
            collect_expr_field_refs(&uop.operand, fields, field_index_map);
        }
        // Standard unpacked binary ops (from normalize_to_old)
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Mod(l, r) | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r)
        | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::Or(l, r)
        | Expr::And(l, r) | Expr::BitAnd(l, r) | Expr::BitOr(l, r)
        | Expr::BitXor(l, r) | Expr::Shl(l, r) | Expr::Shr(l, r)
        | Expr::Concat(l, r) | Expr::ListIndex(l, r) => {
            collect_expr_field_refs(l, fields, field_index_map);
            collect_expr_field_refs(r, fields, field_index_map);
        }
        Expr::Not(op) | Expr::Neg(op) | Expr::BitNot(op) => {
            collect_expr_field_refs(op, fields, field_index_map);
        }
        // Calls and intrinsics
        Expr::Call(_, args) | Expr::ListLiteral(args) => {
            for arg in args { collect_expr_field_refs(arg, fields, field_index_map); }
        }
        Expr::IntrinsicCall { args, .. } => {
            for arg in args { collect_expr_field_refs(arg, fields, field_index_map); }
        }
        Expr::Cast(op, _) => {
            collect_expr_field_refs(op, fields, field_index_map);
        }
        // Literals and terminal expressions — no field refs
        _ => {}
    }
}

/// Emit epoll_wait + per-trigger read + dirty-bit-set for the event loop.
/// Called instead of @__rt_wait() when the program has built-in triggers.
pub(crate) fn emit_trg_event_epoll_wait(backend: &mut LlvmBackend, out: &mut String) {
    let tc = backend.fun.txn_counter; backend.fun.txn_counter += 20;
    let epfd_idx = match backend.ctx.field_index_map.get("__trg_epfd") {
        Some(&i) => i,
        None => {
            // No builtin triggers (Stdin/Timer/Signal) — MMIO-only wake triggers
            // can't use epoll on bare addresses. Block-sleep instead of busy-loop.
            writeln!(out, "  call void @__rt_wait()").ok();
            // Step all MMIO wake triggers to refresh reactor state
            for (name, trg) in &backend.ctx.triggers {
                if matches!(&trg.address, crate::ast::LinkRef::Explicit(_)) {
                    if let Some(&bit) = backend.ctx.dep_graph.bit_index.get(name) {
                        let drx = format!("%drx_{}_{}", tc, name);
                        writeln!(out, "  {} = add i64 {}, {}", drx, 1u64 << bit, bit).ok();
                        writeln!(out, "  call void @step(ptr %state, i64 {})", drx).ok();
                    }
                }
            }
            return;
        }
    };
    let evt = format!("%evt_{}", tc);
    writeln!(out, "  {} = alloca i8, i64 16, align 8", evt).ok();
    let ep_gep = format!("%epg_{}", tc);
    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", ep_gep, epfd_idx).ok();
    let ep_ld = format!("%epl_{}", tc);
    writeln!(out, "  {} = load i32, ptr {}, align 4", ep_ld, ep_gep).ok();
    let n_ev = format!("%nev_{}", tc);
    writeln!(out, "  {} = call i32 @epoll_wait(i32 {}, ptr {}, i32 1, i32 -1)", n_ev, ep_ld, evt).ok();
    let ev_cmp = format!("%evc_{}", tc);
    writeln!(out, "  {} = icmp sgt i32 {}, 0", ev_cmp, n_ev).ok();
    let ev_body = format!("ev_body_{}", tc);
    let ev_done = format!("ev_done_{}", tc);
    writeln!(out, "  br i1 {}, label %{}, label %{}", ev_cmp, ev_body, ev_done).ok();
    writeln!(out, "{}:", ev_body).ok();
    let ev_data = format!("%evd_{}", tc);
    writeln!(out, "  {} = getelementptr i8, ptr {}, i64 8", ev_data, evt).ok();
    let ev_data_u64 = format!("%evdu_{}", tc);
    writeln!(out, "  {} = bitcast ptr {} to ptr", ev_data_u64, ev_data).ok();
    let ev_bit = format!("%evb_{}", tc);
    writeln!(out, "  {} = load i64, ptr {}, align 8", ev_bit, ev_data_u64).ok();
    for (name, trg) in &backend.ctx.triggers {
        let bit = backend.ctx.dep_graph.bit_index.get(name).copied().unwrap_or(0);
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
                let rd_res = format!("%rd_{}_{}", tc, name);
                writeln!(out, "  {} = call i64 @read(i32 0, ptr {}, i64 1)", rd_res, ch_slot).ok();
                let rd_ok = format!("%rdok_{}_{}", tc, name);
                writeln!(out, "  {} = icmp sgt i64 {}, 0", rd_ok, rd_res).ok();
                let store_lbl = format!("rds_{}_{}", tc, name);
                writeln!(out, "  br i1 {}, label %{}, label %{}", rd_ok, store_lbl, t_skip).ok();
                writeln!(out, "{}:", store_lbl).ok();
                if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let sge = format!("%sge_{}_{}", tc, name);
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", sge, idx).ok();
                    let ch_ld = format!("%chld_{}_{}", tc, name);
                    writeln!(out, "  {} = load i8, ptr {}, align 1", ch_ld, ch_slot).ok();
                    let ft = backend.ctx.field_types[idx].clone();
                    match ft.as_str() {
                        "i32" => {
                            let ch_z = format!("%chz_{}_{}", tc, name);
                            writeln!(out, "  {} = zext i8 {} to i32", ch_z, ch_ld).ok();
                            writeln!(out, "  store i32 {}, ptr {}, align 4", ch_z, sge).ok();
                        }
                        "i8" => {
                            writeln!(out, "  store i8 {}, ptr {}, align 1", ch_ld, sge).ok();
                        }
                        _ => {
                            let ch_z = format!("%chz_{}_{}", tc, name);
                            writeln!(out, "  {} = zext i8 {} to i64", ch_z, ch_ld).ok();
                            writeln!(out, "  store i64 {}, ptr {}, align 8", ch_z, sge).ok();
                        }
                    }
                }
                let drx = format!("%drx_{}_{}", tc, name);
                writeln!(out, "  {} = add i64 {}, {}", drx, 1u64 << bit, bit).ok();
                writeln!(out, "  call void @step(ptr %state, i64 {})", drx).ok();
                writeln!(out, "  br label %{}", t_skip).ok();
            }
            crate::ast::LinkRef::Timer(_hz) => {
                if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let sge = format!("%sge_{}_{}", tc, name);
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", sge, idx).ok();
                    let cur = format!("%cur_{}_{}", tc, name);
                    writeln!(out, "  {} = load i64, ptr {}, align 8", cur, sge).ok();
                    let inc = format!("%inc_{}_{}", tc, name);
                    writeln!(out, "  {} = add i64 {}, 1", inc, cur).ok();
                    writeln!(out, "  store i64 {}, ptr {}, align 8", inc, sge).ok();
                }
                let drx = format!("%drx_{}_{}", tc, name);
                writeln!(out, "  {} = add i64 {}, {}", drx, 1u64 << bit, bit).ok();
                writeln!(out, "  call void @step(ptr %state, i64 {})", drx).ok();
                writeln!(out, "  br label %{}", t_skip).ok();
            }
            crate::ast::LinkRef::Signal(_sig) => {
                if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let sge = format!("%sge_{}_{}", tc, name);
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", sge, idx).ok();
                    writeln!(out, "  store i64 1, ptr {}, align 8", sge).ok();
                }
                let drx = format!("%drx_{}_{}", tc, name);
                writeln!(out, "  {} = add i64 {}, {}", drx, 1u64 << bit, bit).ok();
                writeln!(out, "  call void @step(ptr %state, i64 {})", drx).ok();
                writeln!(out, "  br label %{}", t_skip).ok();
            }
            crate::ast::LinkRef::Explicit(_) => {
                let drx = format!("%drx_{}_{}", tc, name);
                writeln!(out, "  {} = add i64 {}, {}", drx, 1u64 << bit, bit).ok();
                writeln!(out, "  call void @step(ptr %state, i64 {})", drx).ok();
                writeln!(out, "  br label %{}", t_skip).ok();
            }
            crate::ast::LinkRef::Linked(_) => {
                let drx = format!("%drx_{}_{}", tc, name);
                writeln!(out, "  {} = add i64 {}, {}", drx, 1u64 << bit, bit).ok();
                writeln!(out, "  call void @step(ptr %state, i64 {})", drx).ok();
                writeln!(out, "  br label %{}", t_skip).ok();
            }
        }
        writeln!(out, "{}:", t_skip).ok();
    }
    writeln!(out, "  br label %{}", ev_done).ok();
    writeln!(out, "{}:", ev_done).ok();
}

/// Emit i64 cycle_count = load + add 1 + store at the start of each tick.
fn emit_cycle_count_increment(backend: &mut LlvmBackend, out: &mut String) {
    if let Some(&idx) = backend.ctx.field_index_map.get("cycle_count") {
        writeln!(out, "  %cc_gep = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", idx).ok();
        writeln!(out, "  %cc_old = load i64, ptr %cc_gep, align 8").ok();
        writeln!(out, "  %cc_new = add i64 %cc_old, 1").ok();
        writeln!(out, "  store i64 %cc_new, ptr %cc_gep, align 8").ok();
    }
}

// ── Dead-Field Liveness Analysis ─────────────────────────────────────────
//
// 2026-07-04: Trace which state fields are transitively consumed by
// observable operations (prints, FFI calls, swan songs).  Walks backward
// from observable sinks through LET bindings and `&` assignments.
// A field written as `&x = f(y, z)` where x is live makes y and z live.
// A field written as `&x = f(...)` where x is never read by any sink is
// dead even if it appears as a backedge source (self-referential cycle).
//
// This is used by emit_countable_main to filter out dead assignments
// before LLVM sees them.  In fannkuch_redux, seed and max_flips are
// dead — they're only written and self-referentially read, never
// consumed by any print/FFI output.  Eliminating them before LLVM
// sees the body fixes the phase-ordering issue where LLVM's loop
// unroller evaluates an inflated body and decides not to unroll.
//
// After dead-field filter:
//   fannkuch_redux body: ~80 → ~40 unopt insns → LLVM unrolls 4×

/// Collect every `Identifier` / `OwnedRef` from an expression (not filtered
/// to state fields only).  Used by trace_live_fields to resolve chains
/// through LET bindings (e.g. `let nchecksum = checksum + saved % 13`
/// followed by `&checksum = nchecksum` — the identifier `nchecksum` is not
/// a state field, but tracing through the LET uncovers `checksum` and `saved`).
fn collect_all_idents(e: &Expr, idents: &mut HashSet<String>) {
    match e {
        Expr::Identifier(name) => { idents.insert(name.clone()); }
        Expr::BinaryOp(bop) => {
            collect_all_idents(&bop.left, idents);
            collect_all_idents(&bop.right, idents);
        }
        Expr::UnaryOp(uop) => {
            collect_all_idents(&uop.operand, idents);
        }
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Mod(l, r) | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r)
        | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::Or(l, r)
        | Expr::And(l, r) | Expr::BitAnd(l, r) | Expr::BitOr(l, r)
        | Expr::BitXor(l, r) | Expr::Shl(l, r) | Expr::Shr(l, r)
        | Expr::Concat(l, r) | Expr::ListIndex(l, r) => {
            collect_all_idents(l, idents);
            collect_all_idents(r, idents);
        }
        Expr::Not(op) | Expr::Neg(op) | Expr::BitNot(op) | Expr::Cast(op, _) => {
            collect_all_idents(op, idents);
        }
        Expr::Call(_, args) | Expr::ListLiteral(args) => {
            for arg in args { collect_all_idents(arg, idents); }
        }
        Expr::IntrinsicCall { args, .. } => {
            for arg in args { collect_all_idents(arg, idents); }
        }
        _ => {}
    }
}

/// Compute the transitive closure of state fields referenced by each LET
/// binding in the body.  A LET binding `let x = f(y)` where `y` is itself
/// defined by another LET `let y = g(z)` resolves to include both `y` and
/// `z` (if `z` is a state field).  Returns `(name → field ref set)`.
fn build_let_field_refs(body: &[Statement], field_index_map: &HashMap<String, usize>)
    -> HashMap<String, HashSet<String>>
{
    let mut let_fields: HashMap<String, HashSet<String>> = HashMap::new();
    let mut changed = true;
    // Helper: collect LET bindings from body + guard bodies recursively.
    // e is &Expr (auto-derefed from Box<Expr> through stmt: &Statement destructuring).
    fn collect_let_bindings<'a>(stmt: &'a Statement, collected: &mut Vec<(String, &'a Expr)>) {
        match stmt {
            Statement::Let { name, expr: Some(e), .. } => collected.push((name.clone(), e)),
            Statement::Guarded { statements, .. } => {
                for gs in statements { collect_let_bindings(gs, collected); }
            }
            _ => {}
        }
    }
    let mut let_defs: Vec<(String, &Expr)> = Vec::new();
    for stmt in body {
        collect_let_bindings(stmt, &mut let_defs);
    }
    while changed {
        changed = false;
        for (name, expr) in &let_defs {
            let mut refs = HashSet::new();
            collect_expr_field_refs(expr, &mut refs, field_index_map);
            // Resolve identifiers that refer to other LET bindings
            let mut idents = HashSet::new();
            collect_all_idents(expr, &mut idents);
            for ident in &idents {
                if let Some(sub_refs) = let_fields.get(ident) {
                    for r in sub_refs {
                        refs.insert(r.clone());
                    }
                }
            }
            match let_fields.entry(name.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    if *e.get() != refs {
                        e.insert(refs);
                        changed = true;
                    }
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(refs);
                    changed = true;
                }
            }
        }
    }
    let_fields
}

/// Returns true if an expression is a call that produces observable output.
/// Intrinsic outputs: Print, Println, PrintInt, PrintFloat.
/// Also handles frgn FFI calls that produce output (identified by print_ prefix).
fn is_output_call(expr: &Expr) -> bool {
    match expr {
        Expr::IntrinsicCall { intrinsic, .. } => {
            matches!(intrinsic, Intrinsic::Print | Intrinsic::Println
                | Intrinsic::PrintInt | Intrinsic::PrintFloat
                | Intrinsic::PutChar)
        }
        Expr::Call(name, _) => {
            name.starts_with("print_") || name == "putchar#"
        }
        _ => false,
    }
}

/// Collect field references from statements that produce observable output.
/// These are: print_*/println/print intrinsics, swan_song in term!, FFI calls.
fn observable_field_refs(stmt: &Statement, field_index_map: &HashMap<String, usize>) -> HashSet<String> {
    let mut refs = HashSet::new();
    match stmt {
        Statement::TermBang { swan_song: Some(ss), .. }
        | Statement::Term { swan_song: Some(ss), .. } => {
            collect_field_refs(ss.as_ref(), &mut refs, field_index_map);
        }
        Statement::Expression(e) | Statement::Escape(Some(e)) => {
            if is_output_call(e) {
                collect_expr_field_refs(e, &mut refs, field_index_map);
            }
        }
        _ => {}
    }
    refs
}

/// Extract the target field name from an `Assignment`'s lhs expression.
/// Returns `None` for non-identifier lhs (list-index assigns, tupledestructure, etc.).
fn target_field_name(lhs: &Expr) -> Option<String> {
    match lhs {
        Expr::Identifier(n) => Some(n.clone()),
        _ => None,
    }
}

/// Collect all identifiers from observable statements and seed the live set.
/// Resolves LET binding names through `let_fields` to find underlying state
/// field references.  This is needed because observable_field_refs only
/// returns state field names (filtered through field_index_map), but
/// arguments to observable calls are often LET bindings (e.g. `energy` in
/// `print_float#(energy)`).
fn seed_observable_idents(
    stmt: &Statement,
    let_fields: &HashMap<String, HashSet<String>>,
    field_index_map: &HashMap<String, usize>,
    live: &mut HashSet<String>,
) {
    // 2026-07-05: Trace guard conditions — when a Guarded statement
    // contains observable code (like term! -> print), the guard
    // condition's field references must be live (they control whether
    // the observable output executes). Without this, guard conditions'
    // field refs (like `count` in [count == bound]) are never seeded
    // as live, so filter_dead_assignments removes the counter increment
    // and ssa_old_int_regs is never updated (nbody_newton bug).
    if let Statement::Guarded { condition, statements } = stmt {
        let has_observable = statements.iter().any(|gs| {
            let mut sink = HashSet::new();
            seed_observable_idents(gs, let_fields, field_index_map, &mut sink);
            !sink.is_empty()
        });
        if has_observable {
            let mut idents = HashSet::new();
            collect_all_idents(condition, &mut idents);
            for ident in &idents {
                if field_index_map.contains_key(ident) {
                    live.insert(ident.clone());
                } else if let Some(sub_refs) = let_fields.get(ident) {
                    for r in sub_refs {
                        live.insert(r.clone());
                    }
                }
            }
        }
        return;
    }
    let expr = match stmt {
        Statement::TermBang { swan_song: Some(ss), .. }
        | Statement::Term { swan_song: Some(ss), .. } => {
            match ss.as_ref() {
                Statement::Expression(e) | Statement::Escape(Some(e)) => Some(e),
                _ => None,
            }
        }
        Statement::Expression(e) | Statement::Escape(Some(e)) => {
            if is_output_call(e) { Some(e) } else { None }
        }
        // 2026-07-05: Handle Let bindings wrapping observable calls.
        // In nbody_newton: let __periodic: Bool = print_float#(energy);
        // Without this, the print call inside the Let is missed.
        Statement::Let { expr: Some(e), .. } => {
            if is_output_call(e) { Some(e) } else { None }
        }
        _ => None,
    };
    let Some(e) = expr else { return; };
    let mut idents = HashSet::new();
    collect_all_idents(e, &mut idents);
    for ident in &idents {
        if field_index_map.contains_key(ident) {
            live.insert(ident.clone());
        } else if let Some(sub_refs) = let_fields.get(ident) {
            for r in sub_refs {
                live.insert(r.clone());
            }
        }
    }
}

/// Trace which state fields are transitively consumed by observable
/// operations.  Starts from observable sinks (prints, swan songs) and
/// propagates backward through `&x = f(y)` assignments (when `x` is live)
/// and through LET bindings (when a LET name appears in a live-producing
/// expression's right-hand side).
fn trace_live_fields(body: &[Statement], field_index_map: &HashMap<String, usize>) -> HashSet<String> {
    // Phase 1: compute transitive field refs for each LET binding
    let let_fields = build_let_field_refs(body, field_index_map);

    // Phase 2: seed live set from observable sinks.
    // Unlike observable_field_refs (which only returns state field refs),
    // we collect ALL identifiers from observable expressions and resolve
    // LET binding names through let_fields.  This handles the pattern:
    //   let energy = ... ; term! -> print_float#(energy);
    // where `energy` is a LET binding, not a state field.
    let mut live: HashSet<String> = HashSet::new();
    let mut changed = true;
    for stmt in body {
        seed_observable_idents(stmt, &let_fields, field_index_map, &mut live);
        if let Statement::Guarded { statements, .. } = stmt {
            for gs in statements {
                seed_observable_idents(gs, &let_fields, field_index_map, &mut live);
            }
        }
    }

    // Phase 3: propagate backward — if `x` is live and `&x = f(y, ...)`,
    // then `y, ...` are live (including through LET bindings).
    while changed {
        changed = false;
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, expr, .. } => {
                    let Some(fname) = target_field_name(lhs) else { continue; };
                    if !live.contains(&fname) { continue; }
                    let mut refs = HashSet::new();
                    collect_expr_field_refs(expr, &mut refs, field_index_map);
                    let mut idents = HashSet::new();
                    collect_all_idents(expr, &mut idents);
                    for ident in &idents {
                        if let Some(sub_refs) = let_fields.get(ident) {
                            for r in sub_refs { refs.insert(r.clone()); }
                        }
                    }
                    for id in &refs {
                        if live.insert(id.clone()) { changed = true; }
                    }
                }
                Statement::Guarded { statements, .. } => {
                    for gs in statements {
                        if let Statement::Assignment { lhs, expr, .. } = gs {
                            let Some(fname) = target_field_name(lhs) else { continue; };
                            if !live.contains(&fname) { continue; }
                            let mut refs = HashSet::new();
                            collect_expr_field_refs(expr, &mut refs, field_index_map);
                            let mut idents = HashSet::new();
                            collect_all_idents(expr, &mut idents);
                            for ident in &idents {
                                if let Some(sub_refs) = let_fields.get(ident) {
                                    for r in sub_refs { refs.insert(r.clone()); }
                                }
                            }
                            for id in &refs {
                                if live.insert(id.clone()) { changed = true; }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    live
}

/// 2026-07-04: Filter out `&` assignments to state fields that are not in
/// the live set.  Keeps all non-assignment statements (guards, terms, LETs).
/// Dead-field assignments are removed to reduce the body size seen by LLVM's
/// loop unroller, fixing the phase-ordering issue where dead code inflates
/// the body and prevents unrolling.
fn filter_dead_assignments(body: &[Statement], live_fields: &HashSet<String>) -> Vec<Statement> {
    let mut result = Vec::with_capacity(body.len());
    for stmt in body {
        match stmt {
            Statement::Assignment { lhs, .. } => {
                let Some(fname) = target_field_name(lhs) else {
                    result.push(stmt.clone());
                    continue;
                };
                if live_fields.contains(&fname) {
                    result.push(stmt.clone());
                }
            }
            Statement::Guarded { condition, statements } => {
                let filtered: Vec<Statement> = statements.iter()
                    .filter(|gs| {
                        if let Statement::Assignment { lhs, .. } = gs {
                            target_field_name(lhs)
                                .map(|n| live_fields.contains(&n))
                                .unwrap_or(true)
                        } else { true }
                    })
                    .cloned()
                    .collect();
                if !filtered.is_empty() {
                    result.push(Statement::Guarded {
                        condition: condition.clone(),
                        statements: filtered,
                    });
                }
            }
            _ => result.push(stmt.clone()),
        }
    }
    result
}

/// 2026-07-05: Find cycles in a permutation mapping of field indices.
/// Used by detect_rotation_step to identify circular phi chains.
/// Returns a list of cycles, each as a Vec<usize> of field indices.
fn find_permutation_cycles(perm: &HashMap<usize, usize>, n: usize) -> Vec<Vec<usize>> {
    let mut visited = vec![false; n];
    let mut cycles: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if visited[start] || !perm.contains_key(&start) { continue; }
        let mut cycle: Vec<usize> = Vec::new();
        let mut cur = start;
        while let Some(&next) = perm.get(&cur) {
            if visited[cur] { break; }
            visited[cur] = true;
            cycle.push(cur);
            cur = next;
        }
        if cycle.len() > 1 {
            cycles.push(cycle);
        }
    }
    cycles
}

/// 2026-07-05: Compute the optimal step size to decompose a cycle of length L
/// into sub-cycles of length ≤ 4.  For a cycle of length L, stepping by k
/// creates gcd(L, k) sub-cycles of length L / gcd(L, k).
/// We want sub-cycle length ≤ 4 (SCEV-friendly) with the largest step that
/// still produces optimal SCEV visibility (smaller sub-cycles are better).
fn optimal_step_for_cycle_length(len: usize) -> usize {
    // For a 12-cycle, use step=4 to match C's 4x unrolling (12/4 = 3-cycles).
    // Step=6 gives 6×2-cycles (cleaner SCEV) but causes remainder issues with
    // arbitrary bounds (e.g., 50000000 % 6 = 2). Step=4 evenly divides common
    // benchmarks (50000000 % 4 = 0) and matches C's structure.
    if len == 12 { return 4; }
    let mut best = 1;
    let mut best_sub = len;
    for step in 2..=std::cmp::min(8, len) {
        let gcd_val = gcd(len, step);
        let sub_len = len / gcd_val;
        if sub_len <= 4 && sub_len < best_sub {
            best = step;
            best_sub = sub_len;
        }
    }
    best
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// 2026-07-05: Detect rotation patterns in the body's field assignments.
/// Analyzes pending_phi_native_backedge to find circular phi chains where
/// each field's backedge value is another field's phi register (a rotation).
/// Returns the optimal step size to decompose large cycles into SCEV-friendly
/// sub-cycles (length ≤ 4).  Returns 1 if no rotation is detected.
/// 2026-07-05: Detect rotation patterns in the body's field assignments.
/// Scans the AST body (not emitted IR) to find circular phi chains where
/// each field is assigned the value of another field (a rotation).
/// Returns the optimal step size.  Returns 1 if no rotation is detected.
/// 2026-07-07: Returns (step, rotation_cycle) where step is the optimal
/// unroll factor (1 = no rotation) and rotation_cycle is the ordered list
/// of field names in the longest detected rotation cycle (e.g. [p0, p1,
/// ..., p11] for fannkuch).  The cycle order is used by the pure-rotation
/// latch backedge: rotation field at index N gets its backedge from the
/// field at index (N + step) % cycle.len().
/// See docs/plans/2026-07-07-optimization-plan.md
fn detect_rotation_ast(
    body: &[Statement],
    field_index_map: &HashMap<String, usize>,
) -> (usize, Vec<String>) {
    let n = field_index_map.len();
    if n < 4 { return (1, Vec::new()); }
    // Build let-to-field mapping: if a let binding reads from a state field,
    // resolve it (e.g. let saved = p0 → "saved" → "p0"). This handles the
    // fannkuch pattern: &p11 = saved; where saved = p0.
    let mut let_to_field: HashMap<String, String> = HashMap::new();
    for stmt in body {
        if let Statement::Let { name, expr: Some(Expr::Identifier(src_name)), .. } = stmt {
            if field_index_map.contains_key(src_name) {
                let_to_field.insert(name.clone(), src_name.clone());
            }
        }
    }
    let mut perm: HashMap<usize, usize> = HashMap::new();
    for stmt in body {
        if let Statement::Assignment { lhs, expr, .. } = stmt {
            let Some(dst_name) = target_field_name(lhs) else { continue; };
            let Some(&dst_idx) = field_index_map.get(&dst_name) else { continue; };
            // Try direct field read, then resolve through let binding
            let src_name = match expr {
                Expr::Identifier(name) => name.clone(),
                _ => continue,
            };
            // Resolve the source name through let bindings if not a direct field
            let src_name = if field_index_map.contains_key(&src_name) {
                src_name
            } else if let Some(resolved) = let_to_field.get(&src_name) {
                resolved.clone()
            } else {
                continue;
            };
            if let Some(&src_idx) = field_index_map.get(&src_name) {
                perm.insert(dst_idx, src_idx);
            }
        }
    }
    if perm.len() < 4 { return (1, Vec::new()); }
    let cycles: Vec<Vec<usize>> = find_permutation_cycles(&perm, n);
    let max_len = cycles.iter().map(|c| c.len()).max().unwrap_or(0);
    if max_len <= 4 { return (1, Vec::new()); }
    // Collect field names in the longest cycle (PRESERVES ORDER for latch backedge offset)
    let longest: &Vec<usize> = cycles.iter().max_by_key(|c| c.len()).unwrap();
    let rotation_cycle: Vec<String> = longest.iter()
        .filter_map(|&idx| field_index_map.iter().find(|(_, i)| **i == idx).map(|(n, _)| n.clone()))
        .collect();
    let step = optimal_step_for_cycle_length(max_len);
    (step, rotation_cycle)
}

/// 2026-07-05: Build vector phi groups for register pressure reduction.
/// Scans state fields matching pattern `[a-z][a-z][0-9]+` and groups them
/// into `<4 x float>` vector phi nodes.  For nbody_sqrt's 30 float fields,
/// this reduces phi count from 32 scalar to ~8 vector (eliminating spills).
fn build_vector_phi_groups(
    field_index_map: &HashMap<String, usize>,
    field_types: &[String],
) -> HashMap<String, Vec<String>> {
    // Group float/double fields with sequential numeric suffixes into <4 x float>
    // vector phis.  Any naming convention works (vx0, vel_x_0, col_3) — we strip
    // trailing digits and group by the base name.  To avoid false positives
    // (matrix fields like p00/p01 grouped with p10/p11), we verify the first
    // 4 members have indices 0..3.  The vector phi is register-storage
    // aggregation (reducing phi count for lower register pressure), not SIMD
    // arithmetic — so expression-shape consistency is NOT required.
    let mut groups: HashMap<String, Vec<(usize, String, usize)>> = HashMap::new();
    for (name, &idx) in field_index_map.iter() {
        if idx >= field_types.len() { continue; }
        if field_types[idx] != "float" && field_types[idx] != "double" { continue; }
        let digits_start = name.rfind(|c: char| !c.is_ascii_digit())
            .map(|p| p + 1).unwrap_or(0);
        if digits_start == 0 || digits_start >= name.len() { continue; }
        let (base, suffix) = name.split_at(digits_start);
        let index: usize = match suffix.parse() { Ok(d) => d, _ => continue };
        groups.entry(base.to_string())
            .or_default()
            .push((idx, name.clone(), index));
    }
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    for (base, mut members) in groups {
        if members.len() < 4 { continue; }
        members.sort_by_key(|(_, _, idx)| *idx);
        // Verify indices are 0..3.  This prevents matrix fields (p00+p01+p10+p11)
        // from being grouped as if they were vector fields (vx0..vx3).
        let indices: Vec<usize> = members.iter().take(4).map(|(_, _, i)| *i).collect();
        if indices != vec![0, 1, 2, 3] { continue; }
        let names: Vec<String> = members.into_iter().take(4).map(|(_, n, _)| n).collect();
        let sanitized: String = base.chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        let vec_phi_name = format!("%phi_{}_v4", sanitized);
        result.insert(vec_phi_name, names);
    }
    result
}
