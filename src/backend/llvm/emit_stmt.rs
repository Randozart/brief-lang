use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use crate::features::traits::*;
use std::collections::HashMap;
use std::fmt::Write;

/// 2026-07-04: Maximum fields per %State sub-alloca.  LLVM's SROA pass
/// decomposes structs with up to ~16 elements.  Chunks of <16 ensure
/// SROA can decompose each chunk into scalars for alias analysis and
/// vectorization.  Must match the chunk size used in declare_state_type.
pub(crate) const MAX_FIELDS_PER_ALLLOCA: usize = 15;

impl LlvmBackend {
    /// 2026-07-03: Invalidate cache slots for a field by writing i8 0 to
    /// each valid-bit slot.  Used after both SSA insertvalue and memory stores.
    fn invalidate_field_caches(&mut self, out: &mut String, indent: &str, fname: &str, mut ssa_reg: String) -> String {
        if let Some(targets) = self.ctx.cache_slots.get(fname) {
            for (_target, &(_cache_idx, valid_idx)) in targets {
                let inv = format!("%civ{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = insertvalue %State {}, i8 0, {}", indent, inv, ssa_reg, valid_idx).ok();
                ssa_reg = inv;
            }
        }
        ssa_reg
    }

    /// 2026-07-03: Emit a memory-mode field store: GEP + typed store +
    /// ssa_old tracking + pending_phi_backedge + cache invalidation.
    /// Used by emit_stmt when ssa_state_reg is None (memory mode).
    fn emit_memory_field_store(
        &mut self,
        out: &mut String,
        indent: &str,
        fname: &str,
        idx: usize,
        val: &TypedRegister,
        is_volatile: bool,
    ) {
        // 2026-07-05: Vector group handling — instead of GEP store, emit
        // insertelement to build the vector backedge.  The latch's phi
        // backedge for vector groups is the accumulated vector value.
        for (vec_phi, members) in &self.fun.vector_phi_groups {
            if let Some(comp_idx) = members.iter().position(|m| m == fname) {
                if is_volatile { break; }
                let cur_vec = self.fun.vector_phi_current.get(vec_phi)
                    .cloned().unwrap_or_else(|| vec_phi.clone());
                let ins = format!("%iv{}_{}", self.fun.txn_counter, &vec_phi[1..]);
                self.fun.txn_counter += 1;
                writeln!(out, "{} {} = insertelement <4 x float> {}, float {}, i32 {}",
                    indent, ins, cur_vec, val, comp_idx).ok();
                self.fun.vector_phi_current.insert(vec_phi.clone(), ins.clone());
                self.fun.pending_phi_backedge.insert(fname.to_string(), ins.clone());
                self.fun.pending_phi_native_backedge.insert(fname.to_string(), ins);
                return;
            }
        }
        let ty = self.ctx.field_types[idx].clone();
        let sr = self.fun.state_reg_name.clone();
        let p = self.emit_state_gep(out, indent, "ap", &sr, idx);
        let vol_str = if is_volatile { " volatile" } else { "" };
        let ty_str = ty.as_str();
        let is_native_float = ty_str == "float" || ty_str == "double";
        if !is_volatile && !is_native_float {
            // Integer/pointer types: box to i64, then unbox to target type for store.
            // The box→unbox is needed because pending_phi_backedge stores i64 values
            // for the integer phi backedge path.
            let val_boxed = self.adapt_to_i64(out, indent, val);
            let tn = crate::backend::llvm::tbaa_node(&ty, self.ctx.type_universe.as_ref());
            let brief_ty = self.ctx.field_brief_types.get(idx).cloned();
            let typed_val = self.ensure_typed_value(out, indent, ty_str, &val_boxed, brief_ty, Some(&val.ty));
            // 2026-07-04: Gate the store on both needs_state_stores_in_body
            // and per-field done: liveness.  When done_needs_fields is
            // non-empty, only store fields that the done: block reads.
            // 2026-07-05: rotation_fields override — body stores must be
            // emitted for rotation fields so the latch can GEP-reload them
            // (breaking the circular phi chain for SCEV analysis).
            // Also override for counter_field_name (the latch's overflow
            // guard needs to GEP-reload the counter to check bound).
            let is_counter = self.fun.counter_field_name.as_deref() == Some(fname);
            if (self.fun.needs_state_stores_in_body || self.fun.rotation_fields.contains(fname) || is_counter)
                && (self.fun.done_needs_fields.is_empty() || self.fun.done_needs_fields.contains(fname) || self.fun.rotation_fields.contains(fname) || is_counter) {
                writeln!(out, "{}store{} {} {}, ptr {}, align {}, !tbaa !{}",
                    indent, vol_str, ty, typed_val, p, self.align_of(&ty), tn).ok();
            }
            // 2026-07-04: When parallel_safe_body, keep old (phi) register
            // in ssa_old caches so all computations use old values and become
            // independent — enabling SIMD vectorization.  The ssa_old caches
            // are NOT updated because subsequent reads of this field should
            // still see the pre-iteration value, not the newly computed one.
            // Exceptions:
            //   - counter_field_name: the induction variable, always updated
            //     so guard conditions like [count % N == 0] see new count
            //   - parallel_safe_exempt_fields: fields that guard conditions
            //     read (periodic print guards need latest values)
            let is_counter = self.fun.counter_field_name.as_deref() == Some(fname);
            let is_exempt = is_counter || self.fun.parallel_safe_exempt_fields.contains(fname);
            if !self.fun.parallel_safe_body || is_exempt {
                if ty_str == "i8*" || ty_str == "ptr" || (ty_str != "float" && ty_str != "double") {
                    self.fun.ssa_old_int_regs.insert(fname.to_string(), val_boxed.clone());
                }
            }
            self.fun.pending_phi_backedge.insert(fname.to_string(), val_boxed);
            self.fun.pending_phi_native_backedge.insert(fname.to_string(), typed_val);
        } else if !is_volatile && is_native_float {
            // 2026-07-03: Native float/double: store directly, skip box→unbox roundtrip.
            // val.name is already a float-typed register (from emit_expr). Storing it
            // directly is bit-identical to the box→unbox result but saves 4 instructions.
            let typed_val = val.to_string();
            let tn = crate::backend::llvm::tbaa_node(&ty, self.ctx.type_universe.as_ref());
            // 2026-07-04: Gate the store on both needs_state_stores_in_body
            // and per-field done: liveness.  2026-07-05: rotation_fields + counter override.
            let is_counter = self.fun.counter_field_name.as_deref() == Some(fname);
            if (self.fun.needs_state_stores_in_body || self.fun.rotation_fields.contains(fname) || is_counter)
                && (self.fun.done_needs_fields.is_empty() || self.fun.done_needs_fields.contains(fname) || self.fun.rotation_fields.contains(fname) || is_counter) {
                writeln!(out, "{}store{} {} {}, ptr {}, align {}, !tbaa !{}",
                    indent, vol_str, ty, typed_val, p, self.align_of(&ty), tn).ok();
            }
            // 2026-07-04: When parallel_safe_body, keep old (phi) register
            // in ssa_old caches so all computations use old values.
            // Exemptions: counter field + fields that guard conditions read.
            let is_counter = self.fun.counter_field_name.as_deref() == Some(fname);
            let is_exempt = is_counter || self.fun.parallel_safe_exempt_fields.contains(fname);
            if !self.fun.parallel_safe_body || is_exempt {
                self.fun.ssa_old_float_regs.insert(fname.to_string(), typed_val.clone());
            }
            // pending_phi_backedge key marks this field as modified (latch uses
            // pending_phi_native_backedge for the actual backedge value).
            self.fun.pending_phi_backedge.insert(fname.to_string(), typed_val.clone());
            self.fun.pending_phi_native_backedge.insert(fname.to_string(), typed_val);
        } else {
            // Volatile store (MMIO etc.): passthrough.
            // NOT gated on needs_state_stores_in_body — volatile stores have
            // observable side effects and must always be emitted.
            let val_raw = if is_native_float { val.to_string() } else { self.adapt_to_i64(out, indent, val) };
            writeln!(out, "{}store{} {} {}, ptr {}, align {}", indent, vol_str, ty, val_raw, p, self.align_of(&ty)).ok();
            self.fun.pending_phi_backedge.insert(fname.to_string(), val_raw.clone());
            self.fun.pending_phi_native_backedge.insert(fname.to_string(), val_raw);
        }
        // 2026-07-04: Cache invalidation gated on same conditions:
        // needs_state_stores_in_body + per-field done: liveness.
        if self.fun.needs_state_stores_in_body && (self.fun.done_needs_fields.is_empty() || self.fun.done_needs_fields.contains(fname)) {
            if let Some(targets) = self.ctx.cache_slots.get(fname) {
                for (_target, &(_cache_idx, valid_idx)) in targets {
                    let inv_gep = format!("%civ{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                        indent, inv_gep, valid_idx).ok();
                    writeln!(out, "{}store i8 0, ptr {}, align 1", indent, inv_gep).ok();
                }
            }
        }
    }

    /// 2026-07-04: Ensure a value has the right LLVM type for storage.
    /// Returns the register name usable as the typed value (trunc, bitcast,
    /// or identity). Callers are responsible for the store instruction.
    /// If `brief_ty` is provided (cloned), the type universe is queried
    /// for the correct unbox operation — enabling custom-type-aware codegen
    /// rather than hardcoded LLVM type string matching.
    pub(super) fn ensure_typed_value(&mut self, out: &mut String, indent: &str,
        ty: &str, val: &str, brief_ty: Option<crate::ast::Type>, src_ty: Option<&Type>) -> String
    {
        // 2026-07-10: If source is already the target type, return as-is.
        if let Some(src) = src_ty {
            let is_native_float = *src == Type::Custom("Float".to_string()) && ty == "float";
            let is_native_double = *src == Type::Custom("Float64".to_string()) && ty == "double";
            if is_native_float || is_native_double {
                return val.to_string();
            }
        }
        // 2026-07-04: Try universe-driven unbox first.
        // Clone unbox_op to avoid simultaneous immutable/mutable borrow of self.
        let universe_unbox = brief_ty.as_ref().and_then(|brief| {
            let u = self.ctx.type_universe.as_ref()?;
            let rt = u.get_by_type(brief)?;
            let op = rt.unbox_op.clone()?;
            if op.is_empty() || op == "identity#" { return None; }
            Some(op)
        });
        if let Some(ref op) = universe_unbox {
            return self.emit_unbox_param(out, indent, ty, val, op);
        }
        // Fallback: hardcoded LLVM type string matching.
        match ty {
            "i8" => {
                let tr = format!("%tr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val).ok();
                tr
            }
            "i32" => {
                let tr = format!("%tri{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val).ok();
                tr
            }
            "float" => self.native_float_or_box(out, indent, val),
            "double" => {
                let fl = format!("%nffl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i64 {} to double", indent, fl, val).ok();
                fl
            }
            s if s == "i8*" || s == "ptr" => {
                let fp = format!("%fp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, fp, val).ok();
                fp
            }
            _ => val.to_string(),
        }
    }
    /// 2026-07-03: Emit a GEP into %State for a given field index.
    /// Returns the register name holding the field pointer.
    /// pub(super): shared across the llvm backend.
    /// 2026-07-04: Emit the reverse of emit_box_param — convert an i64-boxed
    /// value to its native LLVM type using the `unbox_op` intrinsic name.
    /// Returns the native-typed register name.
    fn emit_unbox_param(&mut self, out: &mut String, indent: &str,
        ty: &str, val: &str, unbox_op: &str) -> String
    {
        let r = format!("%ub{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        match unbox_op {
            // Bool: trunc i64 to i8 (was widened i8 → i64 via zext)
            "zext.i1.to.i64#" => {
                writeln!(out, "{}{} = trunc i64 {} to i8", indent, r, val).ok();
            }
            // Char / UInt32: trunc i64 to i32 (was widened i32 → i64 via zext)
            "zext.i32.to.i64#" => {
                writeln!(out, "{}{} = trunc i64 {} to i32", indent, r, val).ok();
            }
            // String/Data: inttoptr i64 → ptr (was converted ptr → i64 via ptrtoint)
            "ptrtoint#" => {
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, r, val).ok();
            }
            // Float: trunc i64 to i32, then bitcast i32 to float
            // (was bitcast float→i32, then zext i32→i64)
            "bitcast.f32.to.i64#" | "bitcast.i64.to.f32#" => {
                let m = format!("%uf{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i64 {} to i32", indent, m, val).ok();
                writeln!(out, "{}{} = bitcast i32 {} to float", indent, r, m).ok();
            }
            // Float64: bitcast i64 to double (same width, direct bitcast)
            "bitcast.f64.to.i64#" => {
                writeln!(out, "{}{} = bitcast i64 {} to double", indent, r, val).ok();
            }
            // Signed/unsigned fixed-width: trunc i64 to native size
            op if op.starts_with("sext.") || op.starts_with("zext.") => {
                let llvm_ty = match op {
                    "sext.i8.to.i64#" | "zext.i8.to.i64#" => "i8",
                    "sext.i16.to.i64#" | "zext.i16.to.i64#" => "i16",
                    _ => "i32",
                };
                writeln!(out, "{}{} = trunc i64 {} to {}", indent, r, val, llvm_ty).ok();
            }
            // Unknown unbox_op — keep as-is.
            _ => {
                self.fun.txn_counter -= 1; // didn't use the register
                return val.to_string();
            }
        }
        r
    }

    pub(super) fn emit_state_gep(&mut self, out: &mut String, indent: &str,
        prefix: &str, state_ptr: &str, idx: usize) -> String
    {
        // 2026-07-04: Route to chunk allocas when emitting @main (main_body=true).
        // @init_state and other non-main functions use the monolithic %State type
        // with a single %state pointer (function parameter or standalone alloca).
        let (ptr, struct_ty, sub_idx) = if state_ptr == "%state" && self.fun.main_body {
            let chunk = idx / MAX_FIELDS_PER_ALLLOCA;
            (format!("%state_{}", chunk), format!("%StateChunk{}", chunk), idx % MAX_FIELDS_PER_ALLLOCA)
        } else {
            (state_ptr.to_string(), "%State".to_string(), idx)
        };
        let p = format!("%{}_{}", prefix, self.fun.txn_counter);
        self.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}", indent, p, struct_ty, ptr, sub_idx).ok();
        p
    }

    /// 2026-07-10: Type-aware store to a state field.
    /// Looks up the field's LLVM type, adapts the value via ensure_typed_value,
    /// and emits a correctly-typed store. All LHS store paths should route
    /// through this to avoid the recurring `store i64` bug class.
    pub(crate) fn emit_typed_store(
        &mut self,
        out: &mut String,
        indent: &str,
        name: &str,
        val: &TypedRegister,
    ) {
        // 2026-07-10: Vector phi group — build vector via insertelement.
        for (vec_phi, members) in &self.fun.vector_phi_groups {
            if let Some(comp_idx) = members.iter().position(|m| m == name) {
                let cur_vec = self.fun.vector_phi_current.get(vec_phi)
                    .cloned().unwrap_or_else(|| vec_phi.clone());
                let ins = format!("%iv{}_{}", self.fun.txn_counter, &vec_phi[1..]);
                self.fun.txn_counter += 1;
                writeln!(out, "{} {} = insertelement <4 x float> {}, float {}, i32 {}",
                    indent, ins, cur_vec, val.name, comp_idx).ok();
                self.fun.vector_phi_current.insert(vec_phi.clone(), ins.clone());
                self.fun.pending_phi_backedge.insert(name.to_string(), ins.clone());
                self.fun.pending_phi_native_backedge.insert(name.to_string(), ins);
                return;
            }
        }
        let Some(&idx) = self.ctx.field_index_map.get(name) else { return; };
        let ty = self.ctx.field_types[idx].clone();
        let sr = self.fun.state_reg_name.clone();
        let p = self.emit_state_gep(out, indent, "ts", &sr, idx);
        let brief_ty = self.ctx.field_brief_types.get(idx).cloned();
        let tv = self.ensure_typed_value(out, indent, &ty, &val.name, brief_ty, Some(&val.ty));
        // 2026-07-10: TBAA metadata enables LLVM to prove field stores don't
        // alias each other. Without it, every store blocks load hoisting, store
        // forwarding, and SROA — causing ~54% slowdown (fannkuch regression).
        let tn = crate::backend::llvm::tbaa_node(&ty, self.ctx.type_universe.as_ref());
        let is_counter = self.fun.counter_field_name.as_deref() == Some(name);
        if (self.fun.needs_state_stores_in_body || self.fun.rotation_fields.contains(name) || is_counter)
            && (self.fun.done_needs_fields.is_empty() || self.fun.done_needs_fields.contains(name)
                || self.fun.rotation_fields.contains(name) || is_counter)
        {
            writeln!(out, "{}store {} {}, ptr {}, align {}, !tbaa !{}",
                indent, ty, tv, p, self.align_of(&ty), tn).ok();
        }
        // 2026-07-10: Update SSA tracking maps so subsequent reads in the same
        // tick (guard expressions, phi backedge) see the computed value, not the
        // stale phi register. Without this, guards like [ops % 5000000 == 0] read
        // the old phi value (0) instead of the new body-computed value.
        let ty_str = ty.as_str();
        let is_native_float = ty_str == "float" || ty_str == "double";
        if !is_native_float {
            let val_boxed = self.adapt_to_i64(out, indent, val);
            let is_counter = self.fun.counter_field_name.as_deref() == Some(name);
            let is_exempt = is_counter || self.fun.parallel_safe_exempt_fields.contains(name);
            if !self.fun.parallel_safe_body || is_exempt {
                self.fun.ssa_old_int_regs.insert(name.to_string(), val_boxed.clone());
            }
            self.fun.pending_phi_backedge.insert(name.to_string(), val_boxed);
            self.fun.pending_phi_native_backedge.insert(name.to_string(), tv);
        } else {
            // Native float/double: track in float regs cache instead.
            let is_counter = self.fun.counter_field_name.as_deref() == Some(name);
            let is_exempt = is_counter || self.fun.parallel_safe_exempt_fields.contains(name);
            if !self.fun.parallel_safe_body || is_exempt {
                self.fun.ssa_old_float_regs.insert(name.to_string(), tv.clone());
            }
            self.fun.pending_phi_backedge.insert(name.to_string(), tv.clone());
            self.fun.pending_phi_native_backedge.insert(name.to_string(), tv);
        }
    }

    /// 2026-07-04: Emit chunk allocas for all %State sub-structs.
    /// Each chunk has ≤MAX_FIELDS_PER_CHUNK fields so SROA can decompose
    /// each chunk into scalar registers for alias analysis and vectorization.
    /// Also emits a monolithic %state alloca for backward compat — helper
    /// functions emit raw %State GEPs (pre_load_all_fields, emit_trg_init,
    /// prealloc, etc.) that reference %state directly.  The chunk allocas
    /// are used by emit_state_gep (routed path); the monolithic %state is
    /// used by the raw GEP path.  Both point to the same logical fields.
    /// pub(super): shared across the llvm backend.
    pub(super) fn emit_state_allocas(&mut self, out: &mut String) {
        let num = self.ctx.field_types.len();
        if num == 0 {
            writeln!(out, "  %state = alloca %State, align 8").ok();
            return;
        }
        // Chunk allocas for the routed path (emit_state_gep → SROA-friendly)
        let chunks = (num + MAX_FIELDS_PER_ALLLOCA - 1) / MAX_FIELDS_PER_ALLLOCA;
        for i in 0..chunks {
            writeln!(out, "  %state_{} = alloca %StateChunk{}, align 8", i, i).ok();
        }
        // Monolithic %state for backward compat with raw GEP paths
        writeln!(out, "  %state = alloca %State, align 8").ok();
    }

    /// Store a native-typed value to the i64 result slot, boxing if needed.
    fn store_i64_result(&mut self, out: &mut String, indent: &str, r: &TypedRegister, rs: &str) {
        let adapted = self.adapt_to_i64(out, indent, r);
        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, adapted, rs).ok();
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
        // 2026-06-29: Phase 7A — query universe box_op instead of matching on Type.
        // The universe stores the canonical boxing intrinsic for each type.
        // Falls back to the old type match when universe is not available.
        let box_op = self.ctx.type_universe.as_ref()
            .and_then(|u| u.get_by_type(&r.ty))
            .and_then(|rt| rt.box_op.clone())  // clone to avoid borrow conflict
            .unwrap_or_default();

        if box_op.is_empty() {
            // Universe not available — fallback to old type-based dispatch
            self.adapt_to_i64_fallback(out, indent, r)
        } else {
            self.adapt_via_box_op(out, indent, r, &box_op)
        }
    }

    /// Box a value via its universe-declared box_op intrinsic.
    /// 2026-06-29: Phase 7A — replaces per-type match arms.
    fn adapt_via_box_op(&mut self, out: &mut String, indent: &str, r: &TypedRegister, box_op: &str) -> String {
        match box_op {
            // Already i64 — no conversion needed
            _ if r.ty == Type::Custom("Char".to_string()) => r.name.clone(),

            // Bool: zext i1 to i64
            "zext.i1.to.i64#" => {
                let z = format!("%rz{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, z, r.name).ok();
                z
            }

            // String/Data: ptrtoint ptr to i64 (but check if already boxed)
            "ptrtoint#" => {
                let is_boxed = r.name.starts_with("%t") || r.name.starts_with("%d");
                if is_boxed {
                    r.name.clone()
                } else {
                    let p = format!("%rp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, p, r.name).ok();
                    p
                }
            }

            // Float: bitcast float -> i32 -> zext -> i64 (with reg cache)
            "bitcast.f32.to.i64#" => {
                let cached = self.fun.reg_float_cache.get(&r.name);
                let fl = if let Some(cached) = cached { cached.clone() } else { r.name.clone() };
                let bi = format!("%rbi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, fl).ok();
                let ze = format!("%rze{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                ze
            }

            // Float64: bitcast double to i64 directly (same width)
            "bitcast.f64.to.i64#" => {
                let bi = format!("%rbi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast double {} to i64", indent, bi, r.name).ok();
                bi
            }

            // Signed fixed-width: sext i8/i16/i32 to i64
            op if op.starts_with("sext.") => {
                let llvm_ty = match op {
                    "sext.i8.to.i64#" => "i8",
                    "sext.i16.to.i64#" => "i16",
                    "sext.i32.to.i64#" => "i32",
                    _ => unreachable!(),
                };
                let ex = format!("%rex{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = sext {} {} to i64", indent, ex, llvm_ty, r.name).ok();
                ex
            }

            // Unsigned fixed-width: zext i8/i16/i32 to i64
            op if op.starts_with("zext.") => {
                let llvm_ty = match op {
                    "zext.i8.to.i64#" => "i8",
                    "zext.i16.to.i64#" => "i16",
                    "zext.i32.to.i64#" => "i32",
                    _ => unreachable!(),
                };
                let ex = format!("%rex{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext {} {} to i64", indent, ex, llvm_ty, r.name).ok();
                ex
            }

            // Fallback: treat as already i64
            _ => r.name.clone(),
        }
    }

    /// Fallback boxing when universe is not available (unit tests).
    /// 2026-06-29: Will be removed once all tests go through the full pipeline.
    fn adapt_to_i64_fallback(&mut self, out: &mut String, indent: &str, r: &TypedRegister) -> String {
        if r.ty == Type::Custom("Bool".to_string()) {
            let z = format!("%rz{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = zext i1 {} to i64", indent, z, r.name).ok();
            z
        } else if r.ty == Type::Custom("Char".to_string()) {
            r.name.clone()
        } else if r.ty == Type::Custom("String".to_string()) || r.ty == Type::Custom("Data".to_string()) {
            let is_boxed = r.name.starts_with("%t") || r.name.starts_with("%d");
            if is_boxed { r.name.clone() } else {
                let p = format!("%rp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, p, r.name).ok();
                p
            }
        } else if r.ty == Type::Custom("Float".to_string()) {
            let cached = self.fun.reg_float_cache.get(&r.name);
            let fl = if let Some(cached) = cached { cached.clone() } else { r.name.clone() };
            let bi = format!("%rbi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, fl).ok();
            let ze = format!("%rze{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
            ze
        } else if r.ty == Type::Custom("Float64".to_string()) {
            let bi = format!("%rbi{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = bitcast double {} to i64", indent, bi, r.name).ok();
            bi
        } else if r.ty == Type::Custom("Int8".to_string()) {
            let ex = format!("%rex{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = sext i8 {} to i64", indent, ex, r.name).ok();
            ex
        } else if r.ty == Type::Custom("UInt8".to_string()) {
            let ex = format!("%rex{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = zext i8 {} to i64", indent, ex, r.name).ok();
            ex
        } else if r.ty == Type::Custom("Int16".to_string()) {
            let ex = format!("%rex{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = sext i16 {} to i64", indent, ex, r.name).ok();
            ex
        } else if r.ty == Type::Custom("UInt16".to_string()) {
            let ex = format!("%rex{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = zext i16 {} to i64", indent, ex, r.name).ok();
            ex
        } else if r.ty == Type::Custom("Int32".to_string()) {
            let ex = format!("%rex{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = sext i32 {} to i64", indent, ex, r.name).ok();
            ex
        } else if r.ty == Type::Custom("UInt32".to_string()) {
            let ex = format!("%rex{}", self.fun.txn_counter); self.fun.txn_counter += 1;
            writeln!(out, "{}{} = zext i32 {} to i64", indent, ex, r.name).ok();
            ex
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
                let c = self.fun.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(swan) = swan_song {
                    self.emit_stmt(out, swan, indent);
                }
                // in_callable_txn: set by emit_definition (defn) and
                // emit_callable_txn. When true, Term emits a ret with the
                // computed value and sets terminated=true. When false, Term
                // is a no-op — the caller's outer fallback ret handles it.
                if self.fun.in_callable_txn {
                    // Store value to result slot, branch to post label
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        // Phase 3: Decay chimera return value at term boundary
                        let r = self.emit_decay(out, &r, None, indent);
                        if self.fun.fn_ret_ty == "i32" {
                            if r.ty == Type::Custom("Bool".to_string()) {
                                let z = format!("%rz{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = zext i1 {} to i32", indent, z, r.name).ok();
                                writeln!(out, "{}ret i32 {}", indent, z).ok();
                            } else if r.ty == Type::Custom("Char".to_string()) {
                                writeln!(out, "{}ret i32 {}", indent, r).ok();
                            } else {
                                let tr = format!("%tr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, r.name).ok();
                                writeln!(out, "{}ret i32 {}", indent, tr).ok();
                            }
                        } else if self.fun.fn_ret_ty == "float" {
                            let fl = self.ensure_float_reg(out, indent, &r);
                            writeln!(out, "{}ret float {}", indent, fl).ok();
                        } else if self.fun.fn_ret_ty == "i64" {
                            let adapted = self.adapt_to_i64(out, indent, &r);
                            writeln!(out, "{}ret i64 {}", indent, adapted).ok();
                        } else {
                            let adapted = self.adapt_to_i64(out, indent, &r);
                            writeln!(out, "{}ret i64 {}", indent, adapted).ok();
                        }
                    } else if self.fun.fn_ret_ty == "i32" {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else if self.fun.fn_ret_ty == "float" {
                        writeln!(out, "{}ret float 0.0", indent).ok();
                    } else if self.fun.returns_i64 {
                        writeln!(out, "{}ret i64 0", indent).ok();
                    } else if self.fun.main_body {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else {
                        writeln!(out, "{}ret void", indent).ok();
                    }
                    self.fun.terminated = true;
                }
            }
            Statement::TermBang { values, swan_song, .. } => {
                // Async/await barrier: wait for all pending async_await calls
                if self.pending_async_await_count > 0 {
                    writeln!(out, "{}call void @__barrier_wait__()", indent).ok();
                }
                let c = self.fun.pending_cleanup.clone();
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
                if self.fun.in_callable_txn {
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        // Phase 3: Decay chimera before storing to state
                        let r = self.emit_decay(out, &r, None, indent);
                        self.store_i64_result(out, indent, &r, "%state");
                    }
                    if let Some(ref loop_exit) = self.fun.loop_exit_label {
                        writeln!(out, "{}br label %{}", indent, loop_exit).ok();
                    } else if let Some(ref pl) = self.fun.callable_txn_post_label {
                        writeln!(out, "{}br label %{}", indent, pl).ok();
                    }
                    self.fun.terminated = true;
                } else {
                    if let Some(Some(v)) = values.first() {
                        let r = self.emit_expr(out, v, indent);
                        if self.fun.fn_ret_ty == "i32" {
                            let tr = format!("%tr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, r).ok();
                            writeln!(out, "{}ret i32 {}", indent, tr).ok();
                        } else if self.fun.fn_ret_ty == "i64" {
                            writeln!(out, "{}ret i64 {}", indent, r).ok();
                        } else if self.ctx.is_embedded {
                            writeln!(out, "{}store i64 {}, ptr {}", indent, r, self.fun.state_reg_name).ok();
                            writeln!(out, "{}call void asm sideeffect \"wfi\", \"\"()", indent).ok();
                            writeln!(out, "{}ret void", indent).ok();
                        } else if self.fun.main_body {
                            writeln!(out, "{}ret i32 0", indent).ok();
                        } else {
                            writeln!(out, "{}ret void", indent).ok();
                        }
                    } else if self.fun.fn_ret_ty == "i32" {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else if self.fun.returns_i64 {
                        writeln!(out, "{}ret i64 0", indent).ok();
                    } else if self.ctx.is_embedded {
                        writeln!(out, "{}call void asm sideeffect \"wfi\", \"\"()", indent).ok();
                        writeln!(out, "{}ret void", indent).ok();
                    } else if self.fun.main_body {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else {
                        writeln!(out, "{}ret void", indent).ok();
                    }
                    self.fun.terminated = true;
                }
            }
            Statement::Escape(e) => {
                let c = self.fun.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if self.fun.in_callable_txn {
                    if let Some(v) = e {
                        let r = self.emit_expr(out, v, indent);
                        if let Some(rs) = self.fun.callable_txn_result.clone() {
                            self.store_i64_result(out, indent, &r, &rs);
                        }
                    }
                    if let Some(ref pl) = self.fun.callable_txn_post_label {
                        writeln!(out, "{}br label %{}", indent, pl).ok();
                    }
                } else {
                    if let Some(v) = e {
                        let r = self.emit_expr(out, v, indent);
                        if self.fun.fn_ret_ty == "i32" {
                            let tr = format!("%tr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, r).ok();
                            writeln!(out, "{}ret i32 {}", indent, tr).ok();
                        } else {
                            writeln!(out, "{}ret i64 {}", indent, r).ok();
                        }
                    } else if self.fun.fn_ret_ty == "i32" {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else if self.fun.returns_i64 {
                        writeln!(out, "{}ret i64 0", indent).ok();
                    } else if self.fun.main_body {
                        writeln!(out, "{}ret i32 0", indent).ok();
                    } else {
                        writeln!(out, "{}ret void", indent).ok();
                    }
                    self.fun.terminated = true;
                }
            }
            Statement::Let { name, expr, ty, address_expr, constraint, .. } => {
                // Handle TupleDestructure: extract tuple elements and bind each name
                if let Some(Expr::TupleDestructure(names, tuple_expr)) = expr {
                    let tuple_val = self.emit_expr(out, tuple_expr, indent);
                    let hp = format!("%tdh{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, tuple_val.name).ok();
                    for (i, n) in names.iter().enumerate() {
                        let ep = format!("%tde{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                        let val = format!("%tdr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, val, ep).ok();
                        self.fun.let_bindings.insert(n.clone(), val.clone());
                    }
                    return;
                }
                if let Some(e) = expr {
                    let r = self.emit_expr(out, e, indent);
                    // 2026-06-17: Emit type conversion when annotation differs from emitted type.
                    // e.g. `let c: Char = s[pos]` — s[pos] loads i64 (Type::Custom("Int".to_string())) but annotation
                    // is Type::Custom("Char".to_string()) (i32 native). Without the trunc, adapt_to_i64 would double-
                    // zext i64→zext i32 i64, producing invalid LLVM IR.
                    if let Some(ann_ty) = ty.as_ref() {
                        if *ann_ty != r.ty {
                            match (ann_ty, &r.ty) {
                                (Type::Custom(__t), Type::Custom(__s)) if __t == "Char" && (__s == "Int" || __s == "UInt") => {
                                    let cv = format!("%clv{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, cv, r.name).ok();
                                    self.fun.let_bindings.insert(name.clone(), cv.clone());
                                    self.fun.let_binding_types.insert(name.clone(), ann_ty.clone());
                                    writeln!(out, "{}; let {} = {}", indent, name, cv).ok();
                                    return;
                                }
                                _ => {}
                            }
                        }
                    }
                    self.fun.let_bindings.insert(name.clone(), r.name.clone());
                    let resolved_ty = ty.clone().unwrap_or_else(|| r.ty.clone());
                    self.fun.let_binding_types.insert(name.clone(), resolved_ty);
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
                        let guards: Vec<crate::ast::Expr> = self.ctx.type_universe.as_ref()
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
                    Expr::Identifier(n) => n.clone(),
                    Expr::AddrOf(inner) => {
                        // 2026-07-10: Simple &field = value → typed store.
                        // For complex inner expressions (AddrOf(ListIndex(...)),
                        // AddrOf(FieldAccess(...))), fall through to match the
                        // inner expression as the LHS directly.
                        if let Some(name) = inner.as_var_name() {
                            self.emit_typed_store(out, indent, name, &val);
                            return;
                        }
                        let lhs = inner.as_ref();
                        let val = val;
                        match lhs {
                            Expr::ListIndex(list_expr, index_expr) => {
                                let val_reg = val.name.clone();
                                let list_name = match &**list_expr {
                                    Expr::Identifier(n) => n.clone(),
                                    _ => { writeln!(out, "{}; assign list[idx] = {}", indent, val_reg).ok(); return; }
                                };
                                let idx_val = self.emit_expr(out, index_expr, indent);
                                let list_ptr: Option<String> =
                                    if let Some(ref ssa_reg) = self.fun.ssa_state_reg.clone() {
                                        if let Some(&field_idx) = self.ctx.field_index_map.get(&list_name) {
                                            let ev = format!("%lev{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                            writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, field_idx).ok();
                                            Some(ev)
                                        } else if let Some(reg) = self.fun.let_bindings.get(&list_name).cloned() {
                                            Some(reg)
                                        } else {
                                            None
                                        }
                                    } else if let Some(reg) = self.fun.let_bindings.get(&list_name).cloned() {
                                        Some(reg)
                                    } else if let Some(&field_idx) = self.ctx.field_index_map.get(&list_name) {
                                        let sr = self.fun.state_reg_name.clone();
                                        let p = self.emit_state_gep(out, indent, "lgp", &sr, field_idx);
                                        let ld = format!("%lld{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ld, p).ok();
                                        Some(ld)
                                    } else {
                                        None
                                    };
                                let Some(list_ptr) = list_ptr else {
                                    writeln!(out, "{}; assign list[idx] = {} (unknown list '{}')", indent, val_reg, list_name).ok();
                                    return;
                                };
                                let hp = format!("%lhp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, list_ptr).ok();
                                let dp = format!("%ldp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, hp).ok();
                                let de = format!("%lde{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, de, dp).ok();
                                let idx_boxed = self.adapt_to_i64(out, indent, &idx_val);
                                let ep = format!("%lep{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, de, idx_boxed).ok();
                                writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, val_reg, ep).ok();
                                return;
                            }
                            _ => {
                                writeln!(out, "{}; assign unknown LHS", indent).ok();
                                return;
                            }
                        }
                    }
                    Expr::Deref(ptr) => {
                        // 2026-07-10: Deref LHS — evaluate the pointer, store through it.
                        // Use the value's type from the TypedRegister (pointee type).
                        let ptr_reg = self.emit_expr(out, ptr, indent);
                        let pointee_ty = crate::type_universe::pointee_type(&ptr_reg.ty);
                        let Some(inner_ty) = pointee_ty else {
                            writeln!(out, "{}; cannot dereference non-pointer type", indent).ok();
                            return;
                        };
                        let llvm_ty = match inner_ty {
                            Type::Custom(ref s) if s == "Bool" => "i1".to_string(),
                            Type::Custom(ref s) if s == "Char" => "i32".to_string(),
                            Type::Custom(ref s) if s == "Int" => "i64".to_string(),
                            Type::Custom(ref s) if s == "Float" => "float".to_string(),
                            Type::Custom(ref s) if s == "Float64" => "double".to_string(),
                            _ => "i64".to_string(),
                        };
                        let tv = self.ensure_typed_value(out, indent, &llvm_ty, &val.name, Some(inner_ty.clone()), Some(&val.ty));
                        writeln!(out, "{}store {} {}, ptr {}, align {}", indent, llvm_ty, tv, ptr_reg.name, self.align_of(&llvm_ty)).ok();
                        return;
                    }
                    Expr::ListIndex(list_expr, index_expr) => {
                        let val_reg = val.name.clone();
                        let list_name = match &**list_expr {
                            Expr::Identifier(n) => n.clone(),
                            _ => { writeln!(out, "{}; assign list[idx] = {}", indent, val_reg).ok(); return; }
                        };
                        let idx_val = self.emit_expr(out, index_expr, indent);
                        let list_ptr: Option<String> =
                            if let Some(ref ssa_reg) = self.fun.ssa_state_reg.clone() {
                                if let Some(&field_idx) = self.ctx.field_index_map.get(&list_name) {
                                    let ev = format!("%lev{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, field_idx).ok();
                                    Some(ev)
                                } else if let Some(reg) = self.fun.let_bindings.get(&list_name).cloned() {
                                    Some(reg)
                                } else {
                                    None
                                }
                            } else if let Some(reg) = self.fun.let_bindings.get(&list_name).cloned() {
                                Some(reg)
                            } else if let Some(&field_idx) = self.ctx.field_index_map.get(&list_name) {
                                let sr = self.fun.state_reg_name.clone();
                                let p = self.emit_state_gep(out, indent, "lgp", &sr, field_idx);
                                let ld = format!("%lld{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ld, p).ok();
                                Some(ld)
                            } else {
                                None
                            };
                        let Some(list_ptr) = list_ptr else {
                            writeln!(out, "{}; assign list[idx] = {} (unknown list '{}')", indent, val_reg, list_name).ok();
                            return;
                        };
                        let hp = format!("%lhp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, list_ptr).ok();
                        let dp = format!("%ldp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, hp).ok();
                        let de = format!("%lde{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, de, dp).ok();
                        let ep = format!("%lep{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, de, idx_val.name).ok();
                        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, val_reg, ep).ok();
                        return;
                    }
                    Expr::TupleDestructure(names, _) => {
                        let hp = format!("%tdh{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, val).ok();
                        for (i, name) in names.iter().enumerate() {
                            let ep = format!("%tde{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                            let elem = format!("%tdr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, elem, ep).ok();
                            if let Some(ref ssa_reg) = self.fun.ssa_state_reg.clone() {
                                if let Some(&idx) = self.ctx.field_index_map.get(name) {
                                    let new_reg = format!("%in{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = insertvalue %State {}, i64 {}, {}", indent, new_reg, ssa_reg, elem, idx).ok();
                                    self.fun.ssa_state_reg = Some(new_reg);
                                    continue;
                                }
                            }
                            if let Some(&addr) = self.ctx.mmio_fields.get(name) {
                                let p = format!("%mio{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, addr).ok();
                                writeln!(out, "{}store volatile i64 {}, ptr {}, align 1", indent, elem, p).ok();
                                continue;
                            }
                            if let Some(trg) = self.ctx.triggers.get(name) {
                                if trg.is_const {
                                    writeln!(out, "{}; error: cannot write to const trigger '{}'", indent, name).ok();
                                    continue;
                                }
                            }
                            if let Some(&idx) = self.ctx.field_index_map.get(name) {
                                let ty = self.ctx.field_types[idx].clone();
                                let sr = self.fun.state_reg_name.clone();
                                let p = self.emit_state_gep(out, indent, "ap", &sr, idx);
                                let brief_ty = self.ctx.field_brief_types.get(idx).cloned();
                                let tv = self.ensure_typed_value(out, indent, &ty.as_str(), &elem.to_string(), brief_ty, None);
                                writeln!(out, "{}store {} {}, ptr {}, align {}", indent, ty, tv, p, self.align_of(&ty)).ok();
                            } else if let Some(slot) = self.fun.param_slots.get(name) {
                                writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, elem, slot).ok();
                                self.fun.let_bindings.insert(name.clone(), elem.clone());
                            } else {
                                self.fun.let_bindings.insert(name.clone(), elem.clone());
                                writeln!(out, "{}; tuple elem assign {} to {}", indent, elem, name).ok();
                            }
                        }
                        return;
                    }
                    _ => { writeln!(out, "{}; assign {}", indent, val).ok(); return; }
                };
                let is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                if let Some(trg) = self.ctx.triggers.get(&fname) {
                    if trg.is_const {
                        writeln!(out, "{}; error: cannot write to const trigger '{}'", indent, fname).ok();
                        return;
                    }
                }
                if let Some(ssa_reg) = self.fun.ssa_state_reg.clone() {
                    if let Some(&idx) = self.ctx.field_index_map.get(&fname) {
                        if !is_volatile {
                            let ty = self.ctx.field_types[idx].clone();
                            let new_reg = format!("%in{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                            let val_boxed = self.adapt_to_i64(out, indent, &val);
                            match ty.as_str() {
                                "i8" => {
                                    let tr = format!("%tr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val_boxed).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i8 {}, {}", indent, new_reg, ssa_reg, tr, idx).ok();
                                }
                                "i32" => {
                                    let tr = format!("%tri{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val_boxed).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i32 {}, {}", indent, new_reg, ssa_reg, tr, idx).ok();
                                }
                                "float" => {
                                    let fl = self.native_float_or_box(out, indent, &val.to_string());
                                    writeln!(out, "{}{} = insertvalue %State {}, float {}, {}", indent, new_reg, ssa_reg, fl, idx).ok();
                                }
                                // 2026-06-29: Float64 fields store double directly in %State
                                "double" => {
                                    let fl = self.ensure_float_reg(out, indent, &val);
                                    writeln!(out, "{}{} = insertvalue %State {}, double {}, {}", indent, new_reg, ssa_reg, fl, idx).ok();
                                }
                                "i8*" => {
                                    let p = format!("%fp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, val_boxed).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, ptr {}, {}", indent, new_reg, ssa_reg, p, idx).ok();
                                }
                                _ => {
                                    writeln!(out, "{}{} = insertvalue %State {}, i64 {}, {}", indent, new_reg, ssa_reg, val_boxed, idx).ok();
                                }
                            }
                            // 2026-06-29: Float64 (double) fields need SSA float reg tracking too
                            if ty != "float" && ty != "double" {
                                let re = format!("%re_{}_{}", fname, self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, re, new_reg, idx).ok();
                            self.fun.ssa_old_int_regs.insert(fname.clone(), re);
                            }
                            // Phase 2: Invalidate ALL cache targets on SSA field store
                            let ssa_result = self.invalidate_field_caches(out, indent, &fname, new_reg.clone());
                            self.fun.ssa_state_reg = Some(ssa_result);
                            return;
                        }
                    }
                }
                if let Some(&addr) = self.ctx.mmio_fields.get(&fname) {
                    let p = format!("%mio{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, addr).ok();
                    writeln!(out, "{}store volatile i64 {}, ptr {}, align 1", indent, val, p).ok();
                    return;
                }
                if let Some(&idx) = self.ctx.field_index_map.get(&fname) {
                    self.emit_memory_field_store(out, indent, &fname, idx, &val, is_volatile);
                } else if let Some(slot) = self.fun.param_slots.get(&fname).cloned() {
                    let val_boxed = self.adapt_to_i64(out, indent, &val);
                    writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, val_boxed, slot).ok();
                    self.fun.let_bindings.insert(fname.clone(), val_boxed.clone());
                    self.fun.let_binding_types.insert(fname.clone(), Type::Custom("Int".to_string()));
                } else {
                    self.fun.let_bindings.insert(fname.clone(), val.name.clone());
                    self.fun.let_binding_types.insert(fname.clone(), val.ty.clone());
                    writeln!(out, "{}; assign {} to {}", indent, val, fname).ok();
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                let cond = self.emit_expr(out, condition, indent);
                let i1 = if cond.ty == Type::Custom("Bool".to_string()) {
                    cond.name.clone()
                } else {
                    let i1 = format!("%gc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
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
                if statements.len() == 1 && self.fun.ssa_state_reg.is_none() {
                    if let Statement::Assignment { lhs, expr, modifiers, .. } = &statements[0] {
                        if let Expr::Identifier(n) = lhs {
                            if let Some(&idx) = self.ctx.field_index_map.get(n) {
                                let g_is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                                let gvol = if g_is_volatile { " volatile" } else { "" };
                                let p = format!("%gp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                let av = self.emit_expr(out, expr, indent);
                                let ty = self.ctx.field_types[idx].clone();
                                writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, p, self.fun.state_reg_name, idx).ok();
                                let se = format!("%gs{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                match ty.as_str() {
                                    "i8" => {
                                        let ld = format!("%gl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                        writeln!(out, "{}{} = load i8, ptr {}, align {}", indent, ld, p, self.align_of(&ty)).ok();
                                        let av_boxed = self.adapt_to_i64(out, indent, &av);
                                        let av_tr = format!("%gatr{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, av_tr, av_boxed).ok();
                                        writeln!(out, "{}{} = select i1 {}, i8 {}, i8 {}", indent, se, i1, av_tr, ld).ok();
                                        writeln!(out, "{}store{} i8 {}, ptr {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
                                    }
                                    "float" => {
                                        let ld = format!("%gl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                        writeln!(out, "{}{} = load float, ptr {}, align {}", indent, ld, p, self.align_of(&ty)).ok();
                                        let av_fl = self.native_float_or_box(out, indent, &av.to_string());
                                        writeln!(out, "{}{} = select i1 {}, float {}, float {}", indent, se, i1, av_fl, ld).ok();
                                        writeln!(out, "{}store{} float {}, ptr {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
                                    }
                                    _ => {
                                        let ld = format!("%gl{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ld, p).ok();
                                        // 2026-06-17: Box float to i64 for uniform i64 store
                                        let av_i64 = if av.ty == Type::Custom("Float".to_string()) {
                                            self.adapt_to_i64(out, indent, &av)
                                        } else {
                                            av.name.clone()
                                        };
                                        writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}", indent, se, i1, av_i64, ld).ok();
                                        writeln!(out, "{}store{} i64 {}, ptr {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
                                    }
                                }
                                // Phase 2: Invalidate ALL cache targets on select store
                                if let Some(targets) = self.ctx.cache_slots.get(n) {
                                    for (_target, &(_cache_idx, valid_idx)) in targets {
                                        let inv_gep = format!("%civs{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                                            indent, inv_gep, valid_idx).ok();
                                        writeln!(out, "{}store i8 0, ptr {}, align 1", indent, inv_gep).ok();
                                    }
                                }
                                return;
                    }
                }
            }
        }

                // Standard guarded block with unique labels
                let gid = format!("g{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let then_l = format!("{}_t", gid);
                let end_l = format!("{}_e", gid);
                let prev_terminated = self.fun.terminated;
                self.fun.terminated = false;

                // SSA mode: wrap guard in a named entry block so phi at merge
                // has a known predecessor label for the skip path.
                let ssa_pre_reg = self.fun.ssa_state_reg.clone();
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
                if let Some(ref profile) = self.ctx.pgo_profile {
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
                // 2026-07-05: Save/restore pending_phi_backedge and
                // pending_phi_native_backedge across guard boundaries.
                // Stores inside the guard body populate these with registers
                // from the guard's then-block. Without save/restore, the latch
                // (emit_countable_latch) reads registers that are defined in a
                // non-dominating block, causing "Instruction does not dominate
                // all uses" errors (mandelbrot bug).
                let saved_bindings = self.fun.let_bindings.clone();
                let saved_types = self.fun.let_binding_types.clone();
                let saved_old_int = self.fun.ssa_old_int_regs.clone();
                let saved_old_float = self.fun.ssa_old_float_regs.clone();
                let saved_pending_backedge = self.fun.pending_phi_backedge.clone();
                let saved_pending_native = self.fun.pending_phi_native_backedge.clone();
                for s in statements { self.emit_stmt(out, s, &format!("{}  ", indent)); }
                self.fun.let_bindings = saved_bindings;
                self.fun.let_binding_types = saved_types;
                self.fun.ssa_old_int_regs = saved_old_int;
                self.fun.ssa_old_float_regs = saved_old_float;
                self.fun.pending_phi_backedge = saved_pending_backedge;
                self.fun.pending_phi_native_backedge = saved_pending_native;
                if !self.fun.terminated {
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
                if !self.fun.terminated {
                    // SSA mode: phi merge at guard — the guard body may have
                    // modified state via insertvalue (only on the then path).
                    // Without a phi, the insertvalue result from %then_l would
                    // be undefined on the skip path. Use then_exit as predecessor.
                    if let Some(ref pre_reg) = ssa_pre_reg {
                        if let Some(ref post_reg) = self.fun.ssa_state_reg {
                            if post_reg != pre_reg {
                                let then_exit = format!("{}_tx", gid);
                                let merge = format!("%me{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                                writeln!(out, "  {} = phi %State [ {}, %{} ], [ {}, %{} ]",
                                    merge, post_reg, then_exit, pre_reg, entry_l);
                                self.fun.ssa_state_reg = Some(merge);
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
                        self.fun.ssa_old_int_regs.clear();
                        self.fun.ssa_old_float_regs.clear();
                    }
                    self.fun.terminated = prev_terminated;
                } else {
                    // Then-path terminated (e.g. term! → program exit).
                    // The else path at end_l continues the loop naturally —
                    // do NOT emit ret here. Restore prev_terminated so
                    // callers emit the continuation (br to loop back-edge).
                    self.fun.terminated = prev_terminated;
                }}
            Statement::SyncBlock { body } => {
                for s in body { self.emit_stmt(out, s, indent); }
            }
            Statement::Unification { name, variant, fields, expr } => {
                // Save/restore bindings — pattern variable bindings from the arm
                // block must not leak past the merge block.
                let saved_bindings = self.fun.let_bindings.clone();
                let saved_types = self.fun.let_binding_types.clone();
                let val = self.emit_expr(out, &Expr::Identifier(name.clone()), indent);
                let disc = format!("%ud{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, 255", indent, disc, val).ok();
                let arm_l = format!("ua{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let def_l = format!("ud{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                let merge_l = format!("um{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                // 2026-06-17: Look up the VARIANT name (Ok, Err) in variant_disc,
                // not the VALUE name (json_res). Using name (the value) always
                // returned 0 (Ok's disc), so Err was never matched.
                let target = self.ctx.variant_disc.get(variant)
                    .map(|(_, d, _)| *d)
                    .unwrap_or(0);
                writeln!(out, "{}switch i64 {}, label %{} [ i64 {}, label %{} ]", indent, disc, def_l, target, arm_l).ok();
                writeln!(out, "{}{}:", indent, arm_l).ok();
                let pay = format!("%up{}", self.fun.txn_counter); self.fun.txn_counter += 1;
                writeln!(out, "{}{} = lshr i64 {}, 8", indent, pay, val).ok();
                // Bind pattern fields to the payload register
                bind_pattern_fields(&mut self.fun.let_bindings, &mut self.fun.let_binding_types, fields, &pay);
                let prev_terminated = self.fun.terminated;
                let _ = self.emit_expr(out, expr, indent);
                if !self.fun.terminated {
                    writeln!(out, "{}br label %{}", indent, merge_l).ok();
                } else {
                    self.fun.terminated = false;
                    if !self.fun.returns_i64 {
                        writeln!(out, "{}br label %{}", indent, merge_l).ok();
                    }
                }
                self.fun.terminated = prev_terminated;
                writeln!(out, "{}{}:", indent, def_l).ok();
                writeln!(out, "{}  br label %{}", indent, merge_l).ok();
                writeln!(out, "{}{}:", indent, merge_l).ok();
                self.fun.let_bindings = saved_bindings;
                self.fun.let_binding_types = saved_types;
            }
            Statement::Expression(e) => { let _ = self.emit_expr(out, e, indent); }
            Statement::InlineAsm { asm_string, .. } => { writeln!(out, "{}{}", indent, asm_string).ok(); }
            Statement::Foreach { item, list, body, modifiers } => {
                crate::features::stmt::foreach::ForeachStmt {
                    item: item.clone(),
                    list: list.clone(),
                    body: body.clone(),
                    modifiers: modifiers.clone(),
                }.emit_llvm(self, out, &mut {
                    crate::backend::llvm::LLVMBuilder::new()
                }, &StmtDispatch, indent, &mut |_backend: &mut crate::backend::llvm::LlvmBackend,
                                                      _out: &mut String,
                                                      _builder: &mut crate::backend::llvm::LLVMBuilder,
                                                      _expr: &crate::ast::Expr,
                                                      _indent: &str| {
                    crate::backend::llvm::TypedRegister { name: "%stub".into(), ty: crate::ast::Type::Custom("Int".to_string()) }
                });
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
                    let temp_name = format!("%__await_result_{}", self.fun.txn_counter);
                    self.fun.txn_counter += 1;
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
                let reg = format!("%t{}", self.fun.txn_counter);
                self.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, reg, val.name).ok();
                self.fun.let_bindings.insert(name.clone(), reg);
                if let Some(ty) = self.fun.let_binding_types.get(&val.name).cloned() {
                    self.fun.let_binding_types.insert(name.clone(), ty);
                } else {
                    self.fun.let_binding_types.insert(name.clone(), Type::Custom("Int".to_string()));
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
        let Some(reg) = self.fun.let_bindings.get(var_name).cloned() else { return };
        let prior_ = self.fun.let_bindings.get("_").cloned();
        let prior_ty = self.fun.let_binding_types.get("_").cloned();
        self.fun.let_bindings.insert("_".to_string(), reg);
        if let Some(ty) = self.fun.let_binding_types.get(var_name).cloned() {
            self.fun.let_binding_types.insert("_".to_string(), ty);
        }
        let ok = self.emit_expr(out, guard, indent);
        let i1 = self.as_bool_reg(out, indent, &ok);
        let cc = format!("%cc{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        let cp = format!("%cp{}", self.fun.txn_counter); self.fun.txn_counter += 1;
        self.fun.txn_counter += 2;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, cc, cp).ok();
        writeln!(out, "{}{}:", indent, cp).ok();
        writeln!(out, "{}  call void @llvm.trap()", indent).ok();
        writeln!(out, "{}  unreachable", indent).ok();
        writeln!(out, "{}{}:", indent, cc).ok();
        match prior_ {
            Some(r) => { self.fun.let_bindings.insert("_".to_string(), r); }
            None => { self.fun.let_bindings.remove("_"); }
        }
        match prior_ty {
            Some(t) => { self.fun.let_binding_types.insert("_".to_string(), t); }
            None => { self.fun.let_binding_types.remove("_"); }
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
                let_binding_types.insert(name.clone(), Type::Custom("Int".to_string()));
            }
            crate::ast::Pattern::Tuple(subfields) => {
                for sub in subfields {
                    if let crate::ast::Pattern::Var(name) = sub {
                        let_bindings.insert(name.clone(), payload_reg.to_string());
                        let_binding_types.insert(name.clone(), Type::Custom("Int".to_string()));
                    }
                }
            }
            _ => {}
        }
    }
}

// ── EXPRESSIONS ───────────────────────────────────────────
