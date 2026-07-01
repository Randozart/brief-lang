// ── Loop emission architecture overview ──────────────────────────────────
//
// There are three main loop emission strategies, chosen by the frontend
// based on the program's structure (see optimizer.rs classification):
//
// 1. FOLDED LOOP (emit_folded_loop + emit_folded_main):
//    For single-txn programs where the body is pure (no branches, no
//    reactive triggers). The counter is either a phi node (use_phi=true,
//    A005a — pure counter-only) or has the body emitted inline with
//    struct-SSA (use_phi=false, A005b — body with provably linear guards).
//
// 2. MEMORY LOOP (emit_folded_memory_main, A005b):
//    For bodies with branching control flow (Guarded statements) where
//    linearity cannot be proven. Uses per-field GEP loads/stores instead
//    of the %State insertvalue chain to avoid phi %State dominance issues.
//
// 3. SSA REGISTER PIPELINE (emit_ssa_main):
//    For multi-txn reactive programs (rct txn). Precondition checked per-
//    iteration; body runs inline with per-field GEP loads/stores. Supports
//    canonical loop detection for phi induction variable optimization.
//
// Why three separate strategies instead of one:
//   - Each eliminates a different category of LLVM IR bloat.
//   - The folded phi loop (A005a) is O(1) — single store, no iteration.
//   - The memory path (A005b) avoids phi %State dominance failures.
//   - The SSA pipeline handles reactive trigger sampling inline.
use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::{float_to_llvm_hex, find_perfect_hash, sparsity_ratio, FoldParam, LlvmBackend};
use crate::analysis::dependency_graph::DependencyGraph;
use std::collections::HashMap;
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
                    let p = format!("%gep_exit_{}", self.fun.txn_counter);
                    self.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, p, idx).ok();
                    let ft = &self.ctx.field_types[idx];
                    match ft.as_str() {
                        "i64" => { writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, p).ok(); }
                        "i32" => {
                            let l = format!("%exit_l{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i32, i32* {}, align 4", indent, l, p).ok();
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, l).ok();
                        }
                        "i8" => {
                            let l = format!("%exit_l{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, l, p).ok();
                            writeln!(out, "{}{} = zext i8 {} to i64", indent, v, l).ok();
                        }
                        s if s == "i8*" || s == "ptr" => {
                            let l = format!("%exit_l{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i8*, i8** {}, align 8", indent, l, p).ok();
                            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, l).ok();
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
                            writeln!(out, "{}{} = load float, float* {}, align 4", indent, l, p).ok();
                            writeln!(out, "{}{} = bitcast float {} to i32", indent, i, l).ok();
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok();
                        }
                        _ => {
                            panic!("emit_exit_expr: unknown field type '{}' for field '{}' in #!exit expression", ft, name);
                        }
                    }
                } else if self.ctx.constants.contains_key(name) {
                    writeln!(out, "{}{} = load i64, i64* @{}, align 8", indent, v, name).ok();
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
            Expr::OwnedRef(name) => {
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
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x void (ptr)*]* @thread_pool_fns to i8**", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @__thread_pool_init__(i32 {}, i8** %tp_fn_ptr)", count).ok();
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
    fn pre_load_all_fields(&mut self, out: &mut String, state_ptr: &str) {
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        for (field_name, &field_idx) in &self.ctx.field_index_map {
            let ty_str = &self.ctx.field_types[field_idx];
            let gc = self.fun.txn_counter; self.fun.txn_counter += 1;
            let gep = format!("%gep_{}_{}", field_name, gc);
            writeln!(out, "  {} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", gep, state_ptr, field_idx).ok();
            let old_reg = format!("%{}_old_{}", field_name, self.fun.txn_counter);
            self.fun.txn_counter += 1;
            let tn = crate::backend::llvm::tbaa_node(ty_str);
            writeln!(out, "  {} = load {}, {}* {}, align {}, !tbaa !{}", old_reg, ty_str, ty_str, gep, self.align_of(ty_str), tn).ok();
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
                writeln!(out, "  %lt_{}_{} = load i64, i64* %gt_{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt_{}_{} = load i64, i64* @{}, align 8", label_prefix, c_once, cn).ok();
            } else {
                writeln!(out, "  %lt_{}_{} = add i64 0, 0", label_prefix, c_once).ok();
            }
            // Load counter once, precompute remaining iterations
            writeln!(out, "  %gcnt_{}_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", label_prefix, c_once, counter_idx).ok();
            writeln!(out, "  %init_{}_{} = load i64, i64* %gcnt_{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
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
            writeln!(out, "  store i64 %lt_{}_{}, i64* %gcnt_{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
        } else if let Some(stmts) = body {
            // SSA mode: load once, phi in header, inline unrolled body with extract/insert, store once
            if let Some(bl) = bound_literal {
                writeln!(out, "  %lt{}_{} = add i64 0, {}", label_prefix, c_once, bl).ok();
            } else if let Some(ti) = total_idx {
                writeln!(out, "  %gt{}_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", label_prefix, c_once, ti).ok();
                writeln!(out, "  %lt{}_{} = load i64, i64* %gt{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt{}_{} = load i64, i64* @{}, align 8", label_prefix, c_once, cn).ok();
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
                        writeln!(out, "  {} = insertvalue %State {}, i8* bitcast (<{{ i64, i64, [{} x i8] }}>* {} to i8*), {}", iv, cur_init, s.len() + 1, g, idx).ok();
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
                        writeln!(out, "  {} = load {}, {}* {}, align {}", ld, ty, ty, gep, self.align_of(&ty)).ok();
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
                writeln!(out, "  %lt{}_{} = load i64, i64* %gt{}_{}, align 8", label_prefix, c_once, label_prefix, c_once).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt{}_{} = load i64, i64* @{}, align 8", label_prefix, c_once, cn).ok();
            } else {
                writeln!(out, "  %lt{}_{} = add i64 0, 0", label_prefix, c_once).ok();
            }
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();
            writeln!(out, "{}_hdr:", label_prefix).ok();
            writeln!(out, "  %gp{}_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", label_prefix, c_once + 1, counter_idx).ok();
            writeln!(out, "  %lp{}_{} = load i64, i64* %gp{}_{}, align 8", label_prefix, c_once + 1, label_prefix, c_once + 1).ok();
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
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
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
                    writeln!(out, "  {0} = load i64, i64* %gt{1}, align 8", bound_reg, self.fun.txn_counter).ok();
                    self.fun.txn_counter += 1;
                    self.emit_prealloc_for_body(out, "  ", body_stmts, &bound_reg);
                } else if let Some(ref cn) = total_const_name {
                    writeln!(out, "  {} = load i64, i64* @{}, align 8", bound_reg, cn).ok();
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
            writeln!(out, "  {0} = load i64, i64* %gt{1}_{2}, align 8", bound_reg, c0, bound_suffix).ok();
        } else if let Some(cn) = total_const_name {
            writeln!(out, "  {} = load i64, i64* @{}, align 8", bound_reg, cn).ok();
        } else {
            writeln!(out, "  {0} = add i64 0, 0", bound_reg).ok();
        }
        // Phase 2: preallocate collection buffers using known loop bound.
        self.emit_prealloc_for_body(out, "  ", body, &bound_reg);
        writeln!(out, "  br label %_hdr").ok();
        writeln!(out, "_hdr:").ok();
        // Counter phi: initial value is 0 (first tick); subsequent values
        // come from the latch (counter_next). The counter is stored back to
        // %state at the end of the body via the body's GEP+store, but the
        // phi-driven induction variable is what LLVM sees as the loop counter.
        let phi_reg = format!("%phi{}", c0);
        let next_reg = format!("%cnt_next{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {0} = phi i64 [ 0, %entry ], [ {1}, %_body ]", phi_reg, next_reg).ok();
        let cmp_reg = format!("%cp{}", c0 + 2);
        writeln!(out, "  {0} = icmp slt i64 {1}, %lt{2}_{3}", cmp_reg, phi_reg, c0, bound_suffix).ok();
        writeln!(out, "  br i1 {}, label %_body, label %_done", cmp_reg).ok();
        writeln!(out, "_body:").ok();
        self.fun.ssa_state_reg = None; // memory mode: writes go through GEP+store
        self.fun.returns_i64 = false;
        // Override counter field with phi register so body reads use the
        // pre-tick value rather than a stale GEP load from %state.
        if let Some(ref cname) = counter_name {
            self.fun.ssa_old_int_regs.insert(cname.clone(), phi_reg.clone());
        }
        self.pre_load_all_fields(out, "%state");
        for s in body {
            if !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
                self.emit_stmt(out, s, "  ");
            }
        }
        self.fun.ssa_old_float_regs.clear();
        self.fun.ssa_old_int_regs.clear();
        // Phi latch: increment phi counter.
        // The body's GEP+store for the counter field stores the same value
        // (via ssa_old_int_regs → body sees phi, writes back phi+1).
        writeln!(out, "  {0} = add i64 {1}, 1", next_reg, phi_reg).ok();
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
        self.fun.fn_ret_ty = "i32".to_string();
        self.fun.main_body = true;
        let attr = self.slp_attr("main", "#3");
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
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
            let counter_name = {
                let pre = &txn.contract.pre_condition;
                let lhs = match pre {
                    Expr::Lt(l, _) => l.as_ref(),
                    Expr::BinaryOp(bop) if bop.kind == crate::features::binary_op::BinaryOpKind::Lt => bop.left.as_ref(),
                    _ => return,
                };
                if let Expr::Identifier(name) = lhs { Some(name.clone()) } else { None }
            };
            if let Some(ref cname) = counter_name {
                let bound_name = {
                    let pre = &txn.contract.pre_condition;
                    let rhs = match pre {
                        Expr::Lt(_, r) => r.as_ref(),
                        Expr::BinaryOp(bop) if bop.kind == crate::features::binary_op::BinaryOpKind::Lt => bop.right.as_ref(),
                        _ => return,
                    };
                    match rhs {
                        Expr::Identifier(name) => Some(name.clone()),
                        Expr::Integer(n) => Some(n.to_string()),
                        _ => None,
                    }
                };
                // Load bound once before loop
                if let Some(ref bname) = bound_name {
                    if let Some(&b_idx) = self.ctx.field_index_map.get(bname) {
                        let b_gep = format!("%gep_bn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", b_gep, b_idx).ok();
                        let b_val = format!("%val_bn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = load i64, i64* {}, align 8", b_val, b_gep).ok();
                        // Check if bound is compile-time or runtime
                        let bound_imm = if bname.parse::<i64>().is_ok() { bname.clone() } else { b_val.clone() };
                        // Phase 2: preallocate collection buffers using the loop bound.
                        // The bound is either a literal (already a string like "100")
                        // or a loaded register (b_val). For the literal case we need
                        // a register; for the register case we pass it directly.
                        let bound_reg = if bname.parse::<i64>().is_ok() {
                            let br = format!("%bound_reg_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "  {} = add i64 0, {}", br, bname).ok();
                            br
                        } else {
                            b_val.clone()
                        };
                        self.emit_prealloc_for_body(out, "  ", &txn.body, &bound_reg);
                        // 2026-06-26: Emit a named block before the per-field
                        // init loads so we have a stable predecessor label for
                        // the phi nodes at phdr. The `br label` terminates the
                        // preceding init block (genv_af32 / etc.) which has no
                        // native terminator.
                        let init_blk = format!("loop_init_{}", self.fun.txn_counter);
                        writeln!(out, "  br label %{}", init_blk).ok();
                        writeln!(out, "  {}:", init_blk).ok();
                        // Per-field phi nodes for ALL scalar state fields so
                        // values flow through SSA registers, not GEP+load/store
                        // round-trips. Load initial values from %State in this
                        // block, then phi at phdr.
                        self.fun.phi_field_regs.clear();
                        self.fun.backedge_field_regs.clear();
                        let mut init_regs: HashMap<String, String> = HashMap::new();
                        for (name, &idx) in &self.ctx.field_index_map {
                            if let Some(ref bname) = bound_name {
                                if name == bname || *name == *bname { continue; }
                            }
                            let ty = &self.ctx.field_types[idx];
                            let gep_init = format!("%gep_init_{}", self.fun.txn_counter);
                            let init_load = format!("%init_field_{}", self.fun.txn_counter);
                            writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                                gep_init, idx).ok();
                            writeln!(out, "  {} = load {}, {}* {}, align {}",
                                init_load, ty, ty, gep_init, self.align_of(ty)).ok();
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
                        // 2026-06-26: Per-field phis for all scalar state fields.
                        // Each phi selects between the init-block initial load
                        // and the latch back-edge value (reloaded from %State for
                        // modified fields, identity for unchanged).
                        for (name, phi_reg) in &self.fun.phi_field_regs {
                            let init_reg = &init_regs[name];
                            let be_reg = &self.fun.backedge_field_regs[name];
                            let ty = &self.ctx.field_types[*self.ctx.field_index_map.get(name).unwrap()];
                            writeln!(out, "  {} = phi {} [ {}, %{} ], [ {}, %platch ]",
                                phi_reg, ty, init_reg, init_blk, be_reg).ok();
                        }
                        let pc_name = format!("%pc_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "  {} = icmp slt i64 {}, {}", pc_name, pi_name, bound_imm).ok();
                        writeln!(out, "  br i1 {}, label %ptick, label %pdoneloop", pc_name).ok();
                        writeln!(out, "  ptick:").ok();
                        emit_cycle_count_increment(self, out);
                        // The old tick label is skipped — we use ptick instead
                        self.fun.phi_induction_reg = Some((cname.clone(), pi_name.clone(), pn_name.clone()));
                    }
                }
            }
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
            let mut all_push_targets: Vec<String> = Vec::new();
            for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
                crate::backend::llvm::collect_push_targets(&txn.body, &mut all_push_targets);
            }
            if !all_push_targets.is_empty() {
                // Try to extract bound from the first txn's precondition
                if let Some((_, first_txn)) = txns.iter().find(|(_, t)| t.is_reactive) {
                    let rhs = match &first_txn.contract.pre_condition {
                        Expr::Lt(_, r) => Some(r.as_ref()),
                        Expr::BinaryOp(bop) if bop.kind == crate::features::binary_op::BinaryOpKind::Lt => Some(bop.right.as_ref()),
                        _ => None,
                    };
                    if let Some(rhs) = rhs {
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
                                    writeln!(out, "  {} = load i64, i64* {}, align 8", bound_reg, b_gep).ok();
                                    self.fun.txn_counter += 1;
                                    self.emit_prealloc_for_targets(out, "  ", &all_push_targets, &bound_reg);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            writeln!(out, "  %any_fired = alloca i8, align 1").ok();
            writeln!(out, "  store i8 0, ptr %any_fired").ok();
            writeln!(out, "  br label %tick").ok();
            writeln!(out, "  tick:").ok();
            emit_cycle_count_increment(self, out);
            writeln!(out, "  store i8 0, ptr %any_fired").ok();
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
            if reactive_txns.len() >= 2 {
                // Extract (K, N) from each precondition
                let mut counter: Option<String> = None;
                let mut bound: Option<String> = None;
                let mut divisor: Option<i64> = None;
                let mut cases: Vec<(i64, &str)> = Vec::new();
                let mut all_match = true;
                for (name, txn) in &reactive_txns {
                    let pre = &txn.contract.pre_condition;
                    // Match: And(Lt(counter, bound), Eq(Mod(counter, K), N))
                    // 2026-07-01: Bind normalized expr to local to avoid
                    // temporary-borrow-dropped errors from normalize_to_old_recursive.
                    let norm = pre.normalize_to_old_recursive();
                    match &norm {
                        Expr::And(left, right) => {
                            // Extract counter name from Lt(counter, bound)
                            let cn = match left.as_ref() {
                                Expr::Lt(l, _) => {
                                    if let Expr::Identifier(c) = l.as_ref() { Some(c.clone()) } else { None }
                                }
                                _ => None,
                            };
                            let bn = match left.as_ref() {
                                Expr::Lt(_, r) => {
                                    if let Expr::Identifier(b) = r.as_ref() { Some(b.clone()) } else { None }
                                }
                                _ => None,
                            };
                            // 2026-07-01: Use as_integer() to handle both
                            // Expr::Integer(n) and Expr::Literal(LiteralExpr::Integer(n)).
                            // The parser creates Literal-wrapped integers; normalize_to_old
                            // does not convert them back to Expr::Integer.
                            if let Expr::Eq(eq_l, eq_r) = right.as_ref() {
                                if let (Some((c, k)), Some(n)) = (self.extract_mod_info(eq_l),
                                    eq_r.as_ref().as_integer())
                                {
                                    if let Some(ref prev_c) = counter {
                                                if *prev_c != c { all_match = false; }
                                            } else {
                                                // For modulo dispatch, the counter must be an
                                                // integer field. String/bool are rejected because
                                                // srem only works on integers.
                                                if let Some(&idx) = self.ctx.field_index_map.get(&c) {
                                                    let ct = &self.ctx.field_types[idx];
                                                    if ct != "i64" && ct != "i32" { all_match = false; }
                                                } else { all_match = false; }
                                                counter = Some(c);
                                            }
                                    if let Some(ref prev_b) = bound {
                                        if let Some(ref b) = bn {
                                            if *prev_b != *b { all_match = false; }
                                        } else { all_match = false; }
                                    } else {
                                        bound = bn;
                                    }
                                    if let Some(d) = divisor {
                                        if d != k { all_match = false; }
                                    } else {
                                        divisor = Some(k);
                                    }
                                    if k > 256 {
                                        all_match = false;
                                    }
                                    if cases.iter().any(|(v, _)| *v == n) {
                                        all_match = false;
                                    }
                                    cases.push((n, name.as_str()));
                                } else { all_match = false; }
                            } else { all_match = false; }
                        }
                        _ => { all_match = false; }
                    }
                    if !all_match { break; }
                }
                if all_match && cases.len() >= 2 && counter.is_some() {
                    cases.sort_by_key(|(v, _)| *v);
                    let count_name = counter.take().unwrap();
                    if self.ctx.field_index_map.contains_key(&count_name) {
                        // Emit modulo-switch dispatch
                        let case_names: Vec<&str> = cases.iter().map(|(_, n)| *n).collect();
                        self.warnings.push(format!("info: modulo-switch dispatch for [{}] on {} % {}",
                            case_names.join(", "), count_name, divisor.unwrap()));
                        self.emit_modulo_switch_main(out, txns, &count_name, divisor.unwrap(), &cases);
                        return;
                    }
                }
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
                        if self.ctx.field_index_map.contains_key(fname) {
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
                                    if self.ctx.field_index_map.contains_key(fname) {
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
                                            if self.ctx.field_index_map.contains_key(fname) {
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
                                            if self.ctx.field_index_map.contains_key(fname) {
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
            
            if self.fun.phi_induction_reg.is_some() {
                // Canonical loop: phi induction variable already guarantees precondition.
                // Skip precondition check — body runs unconditionally.
                // 2026-06-26: Use per-field phi registers instead of
                // pre_load_all_fields (GEP+load). The phi registers are defined
                // at the phdr block and dominate the ptick body block. This
                // eliminates a load+store round-trip per field per iteration.
                // pending_phi_backedge is populated by emit_stmt when it
                // processes &field = expr assignments.
                self.fun.pending_phi_backedge.clear();
                // 2026-06-27: Classify phi regs by field type — float fields
                // go into ssa_old_float_regs so body lookups find the correct
                // register. Previously all phi regs went to ssa_old_int_regs,
                // causing float field reads to fall back to "0.0" (nbody bug).
                // 2026-06-29: Also check for "double" (Float64) fields.
                for (name, phi_reg) in &self.fun.phi_field_regs {
                    if let Some(&idx) = self.ctx.field_index_map.get(name) {
                        let ll_ty = &self.ctx.field_types[idx];
                        if ll_ty == "float" || ll_ty == "double" {
                            self.fun.ssa_old_float_regs.insert(name.clone(), phi_reg.clone());
                        } else {
                            self.fun.ssa_old_int_regs.insert(name.clone(), phi_reg.clone());
                        }
                    } else {
                        self.fun.ssa_old_int_regs.insert(name.clone(), phi_reg.clone());
                    }
                }
                if let Some((ref cname, ref pi_reg, _)) = self.fun.phi_induction_reg {
                    self.fun.ssa_old_int_regs.insert(cname.clone(), pi_reg.clone());
                }
                self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
                // 2026-07-01: Clear expression dedup cache at body entry so each
                // body emission gets a fresh scope. The cache persists across
                // let-bindings within the body to catch cross-statement redundancy
                // (e.g., dxe23*dxe23 appearing in multiple energy computations).
                self.fun.expr_dedup_cache.clear();
                self.fun.terminated = false;
                self.fun.returns_i64 = false;
                self.fun.loop_exit_label = Some("pdoneloop".into());
                for s in body_stmts { self.emit_stmt(out, s, "  "); }
                self.fun.loop_exit_label = None;
                self.fun.ssa_old_float_regs.clear();
                self.fun.ssa_old_int_regs.clear();
                // Store post_hoist for emission after loop exit (in pdoneloop)
                self.fun.pending_post_hoist = post_hoist.clone();
            } else if !matches!(pre, Expr::Bool(true)) {
                // 2026-06-26: pre_load_all_fields loads ALL state fields into
                // ssa_old_float_regs / ssa_old_int_regs so the precondition
                // check can reference them.  Save these registers and restore
                // them in the body block instead of calling pre_load_all_fields
                // again — the tick block dominates the body block, so the
                // original SSA values are available without reloading.
                self.pre_load_all_fields(out, "%state");
                // 2026-07-01: Clear dedup cache so per-txn body emission starts
                // with a fresh scope (registers from precondition evaluation and
                // other txns are not cached across the dispatch chain).
                self.fun.expr_dedup_cache.clear();
                let saved_float_regs = self.fun.ssa_old_float_regs.clone();
                let saved_int_regs = self.fun.ssa_old_int_regs.clone();
                let cond = self.emit_expr(out, pre, "  ");
                let i1 = if cond.ty == Type::Bool {
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
                // 2026-06-27: Only emit any_fired when no phi induction reg;
                // canonical phi loop uses counter for exit, not any_fired.
                if self.fun.phi_induction_reg.is_none() { writeln!(out, "  store i8 1, ptr %any_fired").ok(); }
                self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
                self.fun.expr_dedup_cache.clear();
                self.fun.terminated = false;
                self.fun.returns_i64 = false;
                // 2026-06-26: Use the tick's pre-loaded registers instead of
                // reloading — the tick block dominates b_body, so the original
                // GEP+load SSA values are available without memory traffic.
                self.fun.ssa_old_float_regs = saved_float_regs;
                self.fun.ssa_old_int_regs = saved_int_regs;
                self.fun.loop_exit_label = Some("done".into());
                for s in body_stmts { self.emit_stmt(out, s, "  "); }
                self.fun.loop_exit_label = None;
                self.fun.ssa_old_float_regs.clear();
                self.fun.ssa_old_int_regs.clear();
                writeln!(out, "  br label %{}", skip_l).ok();
                writeln!(out, "  {}:", done_l).ok();
                // Post-loop: emit hoisted field-based prints, then chain to next txn
                self.emit_hoisted_post_loop_prints(out, &post_hoist);
                if self.fun.phi_induction_reg.is_some() {
                    writeln!(out, "  br label %platch").ok();
                } else {
                    writeln!(out, "  br label %{}", skip_l).ok();
                }
                writeln!(out, "  {}:", skip_l).ok();
            } else {
                self.fun.let_bindings.clear(); self.fun.let_binding_types.clear(); self.fun.reg_float_cache.clear(); self.fun.reg_type_cache.clear();
                self.fun.expr_dedup_cache.clear();
                self.fun.terminated = false;
                self.fun.returns_i64 = false;
                self.pre_load_all_fields(out, "%state");
                // Override counter field with phi induction register if available
                if let Some((ref cname, ref pi_reg, _)) = self.fun.phi_induction_reg {
                    self.fun.ssa_old_int_regs.insert(cname.clone(), pi_reg.clone());
                }
                self.fun.loop_exit_label = Some("done".into());
                // 2026-06-27: Only emit any_fired when no phi induction reg
                // (same rationale as the precondition body branch above).
                if self.fun.phi_induction_reg.is_none() { writeln!(out, "  store i8 1, ptr %any_fired").ok(); }
                for s in body_stmts { self.emit_stmt(out, s, "  "); }
                self.fun.loop_exit_label = None;
                self.fun.ssa_old_float_regs.clear();
                self.fun.ssa_old_int_regs.clear();
                // Post-loop: emit hoisted field-based prints
                self.emit_hoisted_post_loop_prints(out, &post_hoist);
            }
        }
        if let Some((_, ref pi_reg, ref pn_reg)) = self.fun.phi_induction_reg.clone() {
            // Canonical loop: emit latch and done labels
            writeln!(out, "  br label %platch").ok();
            writeln!(out, "  platch:").ok();
            writeln!(out, "  {} = add i64 {}, 1", pn_reg, pi_reg).ok();
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
                        writeln!(out, "  {} = load {}, {}* {}, align {}",
                            be_reg, ty, ty, gep, self.align_of(ty)).ok();
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
        writeln!(out, "  {} = load i64, i64* {}, align 8", b_val, b_gep).ok();
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "  tick:").ok();
        emit_cycle_count_increment(self, out);
        // Load counter and bound from state
        let count_idx = self.ctx.field_index_map.get(counter_name).copied().unwrap_or(0);
        let c_gep = format!("%gep_cn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", c_gep, count_idx).ok();
        let c_val = format!("%val_cn{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        writeln!(out, "  {} = load i64, i64* {}, align 8", c_val, c_gep).ok();
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
            self.pre_load_all_fields(out, "%state");
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
        writeln!(out, "  {} = load i64, i64* {}, align 8", c_reload, c_gep).ok();
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

    /// Emit post-loop field-based prints for hoisted terminating guards.
    /// After the loop exits, load fields from %state and print their final values.
    fn emit_hoisted_post_loop_prints(&mut self, out: &mut String, hoisted: &[(String, String)]) {
        for (fname, intrinsic_name) in hoisted {
            if let Some(&idx) = self.ctx.field_index_map.get(fname) {
                let ty = self.ctx.field_types[idx].clone();
                let gep = format!("%gep_pl_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let val = format!("%val_pl_{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                    gep, idx).ok();
                match ty.as_str() {
                    "float" => {
                        writeln!(out, "  {} = load float, float* {}, align 4", val, gep).ok();
                        self.emit_post_print(out, intrinsic_name, &val, "float", "  ");
                    }
                    // 2026-06-29: Float64 → load double, print as double (skip fpext)
                    "double" => {
                        writeln!(out, "  {} = load double, double* {}, align 8", val, gep).ok();
                        self.emit_post_print(out, intrinsic_name, &val, "double", "  ");
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
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x void (ptr)*]* @thread_pool_fns to i8**", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @__thread_pool_init__(i32 {}, i8** %tp_fn_ptr)", count).ok();
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
                writeln!(out, "  %pc_sc = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", ci).ok();
                writeln!(out, "  store i64 {}, i64* %pc_sc, align 8", tv).ok();
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
                    writeln!(out, "  store i64 {}, i64* %pc_uni, align 8", tv).ok();
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
                    writeln!(out, "  store i64 {}, i64* %pc_{}, align 8", tv, prefix).ok();
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
        let mut tc = self.fun.txn_counter;
        writeln!(out, "define void @step(ptr noalias nocapture %state, i64 %dirty_in) local_unnamed_addr #0 {{").ok();
        // Load dirty flags into a mutable alloca
        let dirty_slot = format!("%dirty_{}", tc); tc += 1;
        writeln!(out, "  {} = alloca i64, align 8", dirty_slot).ok();
        writeln!(out, "  store i64 %dirty_in, i64* {}, align 8", dirty_slot).ok();
        // Volatile-load all trigger variables (liveness anchor + value observation)
        // Use the correct LLVM type for each trigger field to avoid reading/writing
        // adjacent struct bytes (i32 for Char, i8 for Bool, i8* for String).
        for trg_name in trigger_names {
            if let Some(&idx) = self.ctx.field_index_map.get(trg_name) {
                let ty_str = &self.ctx.field_types[idx];
                let gep = format!("%gtrg_{}", tc); tc += 1;
                writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gep, idx).ok();
                let ld = format!("%ltrg_{}", tc); tc += 1;
                match ty_str.as_str() {
                    "i32" => {
                        writeln!(out, "  {} = load volatile i32, i32* {}, align 4", ld, gep).ok();
                        writeln!(out, "  store volatile i32 {}, i32* {}, align 4", ld, gep).ok();
                    }
                    "i8" => {
                        writeln!(out, "  {} = load volatile i8, i8* {}, align 1", ld, gep).ok();
                        writeln!(out, "  store volatile i8 {}, i8* {}, align 1", ld, gep).ok();
                    }
                    "i8*" | "ptr" => {
                        writeln!(out, "  {} = load volatile i8*, i8** {}, align 8", ld, gep).ok();
                        writeln!(out, "  store volatile i8* {}, i8** {}, align 8", ld, gep).ok();
                    }
                    "float" => {
                        writeln!(out, "  {} = load volatile float, float* {}, align 4", ld, gep).ok();
                        writeln!(out, "  store volatile float {}, float* {}, align 4", ld, gep).ok();
                    }
                    _ => {
                        writeln!(out, "  {} = load volatile i64, i64* {}, align 8", ld, gep).ok();
                        writeln!(out, "  store volatile i64 {}, i64* {}, align 8", ld, gep).ok();
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
            // Load all dependency values with correct types
            for dep_name in deps {
                if let Some(&dep_idx) = self.ctx.field_index_map.get(dep_name) {
                    let dep_ty = &self.ctx.field_types[dep_idx];
                    let gdep = format!("%gdep_{}", tc); tc += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gdep, dep_idx).ok();
                    let ldep = format!("%ldep_{}", tc); tc += 1;
                    match dep_ty.as_str() {
                        "i32" => { writeln!(out, "  {} = load i32, i32* {}, align 4", ldep, gdep).ok(); }
                        "i8" => { writeln!(out, "  {} = load i8, i8* {}, align 1", ldep, gdep).ok(); }
                        "i8*" | "ptr" => { writeln!(out, "  {} = load i8*, i8** {}, align 8", ldep, gdep).ok(); }
                        "float" => { writeln!(out, "  {} = load float, float* {}, align 4", ldep, gdep).ok(); }
                        _ => { writeln!(out, "  {} = load i64, i64* {}, align 8", ldep, gdep).ok(); }
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
                        "i32" => { writeln!(out, "  {} = load i32, i32* {}, align 4", lsrc, gsrc).ok(); }
                        "i8" => { writeln!(out, "  {} = load i8, i8* {}, align 1", lsrc, gsrc).ok(); }
                        "i8*" | "ptr" => { writeln!(out, "  {} = load i8*, i8** {}, align 8", lsrc, gsrc).ok(); }
                        "float" => { writeln!(out, "  {} = load float, float* {}, align 4", lsrc, gsrc).ok(); }
                        _ => { writeln!(out, "  {} = load i64, i64* {}, align 8", lsrc, gsrc).ok(); }
                    }
                    let gdst = format!("%gdst_{}", tc); tc += 1;
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", gdst, idx).ok();
                    match dst_ty.as_str() {
                        "i32" => { writeln!(out, "  store i32 {}, i32* {}, align 4 ; recompute {}", lsrc, gdst, var_name).ok(); }
                        "i8" => { writeln!(out, "  store i8 {}, i8* {}, align 1 ; recompute {}", lsrc, gdst, var_name).ok(); }
                        "i8*" | "ptr" => { writeln!(out, "  store i8* {}, i8** {}, align 8 ; recompute {}", lsrc, gdst, var_name).ok(); }
                        "float" => { writeln!(out, "  store float {}, float* {}, align 4 ; recompute {}", lsrc, gdst, var_name).ok(); }
                        _ => { writeln!(out, "  store i64 {}, i64* {}, align 8 ; recompute {}", lsrc, gdst, var_name).ok(); }
                    }
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
        self.fun.txn_counter = tc;
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
                writeln!(out, "  {} = call i64 @read(i32 0, i8* {}, i64 1)", rd_res, ch_slot).ok();
                let rd_ok = format!("%rdok_{}_{}", tc, name);
                writeln!(out, "  {} = icmp sgt i64 {}, 0", rd_ok, rd_res).ok();
                let store_lbl = format!("rds_{}_{}", tc, name);
                writeln!(out, "  br i1 {}, label %{}, label %{}", rd_ok, store_lbl, t_skip).ok();
                writeln!(out, "{}:", store_lbl).ok();
                if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let sge = format!("%sge_{}_{}", tc, name);
                    writeln!(out, "  {} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", sge, idx).ok();
                    let ch_ld = format!("%chld_{}_{}", tc, name);
                    writeln!(out, "  {} = load i8, i8* {}, align 1", ch_ld, ch_slot).ok();
                    let ft = backend.ctx.field_types[idx].clone();
                    match ft.as_str() {
                        "i32" => {
                            let ch_z = format!("%chz_{}_{}", tc, name);
                            writeln!(out, "  {} = zext i8 {} to i32", ch_z, ch_ld).ok();
                            writeln!(out, "  store i32 {}, i32* {}, align 4", ch_z, sge).ok();
                        }
                        "i8" => {
                            writeln!(out, "  store i8 {}, i8* {}, align 1", ch_ld, sge).ok();
                        }
                        _ => {
                            let ch_z = format!("%chz_{}_{}", tc, name);
                            writeln!(out, "  {} = zext i8 {} to i64", ch_z, ch_ld).ok();
                            writeln!(out, "  store i64 {}, i64* {}, align 8", ch_z, sge).ok();
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
                    writeln!(out, "  {} = load i64, i64* {}, align 8", cur, sge).ok();
                    let inc = format!("%inc_{}_{}", tc, name);
                    writeln!(out, "  {} = add i64 {}, 1", inc, cur).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8", inc, sge).ok();
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
                    writeln!(out, "  store i64 1, i64* {}, align 8", sge).ok();
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
