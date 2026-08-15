// ── Expression Codegen Dispatch ────────────────────────────────────────
// 2026-07-12: Phase 4 — Emit LLVM IR for all unified Expr variants.
// Flat dispatch: each Expr variant mapped to named helper or submodule.
// Contains critical optimization history — handle with care.
//
// 2026-06-13: equality_saturation::simplify() REMOVED — exponential blowup
// on deeply nested || chains (32+ terms = 13M+ calls). LLVM -O3 does the same.
// See patches/2026-06-13-remove-simplify-from-emit-expr.patch for exact removed code.
//
// 2026-06-28: fix empty-indent emit_expr — default indent prevents %t{N} violations.

use crate::ast::*;
use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::emit_stmt;

/// 2026-08-07 (Phase 7): the source of a Boolean mask in `data[mask]` — a
/// compile-time Boolean list literal, or a Bool[N] state field (by its %State
/// field index).
enum MaskSource {
    Constant(Vec<bool>),
    StateField(usize),
}

/// 2026-08-07 (object instance pools): the operands of a mask index — the object
/// expression + register, and the mask source. Bundled so
/// `emit_masked_index` stays under the parameter budget.
struct MaskIndexOperands<'a> {
    obj: &'a Expr,
    obj_reg: &'a TypedRegister,
    source: MaskSource,
}

/// 2026-08-07 (object instance pools): everything a member body needs — the
/// receiver (a boxed struct address OR an unpacked instance prefix), the
/// member top-level, and the bound argument registers. Bundled so
/// `emit_member_body` stays under the parameter budget.
pub(crate) struct MemberInvocation<'a> {
    pub recv_reg: &'a TypedRegister,
    pub type_name: &'a str,
    pub member: &'a crate::ast::TopLevel,
    pub arg_regs: &'a [(String, Type)],
    /// (instance prefix, pool row register) — "0" for a static instance.
    pub prefix: Option<(String, String)>,
}

/// Pack an SVO inline header: `len` and `cap` into disjoint bit ranges with
/// bit 0 as the inline tag.
///
/// Layout (2026-07-31, §8.5-E2):
///   bit 0        — inline tag (1 = inline storage, 0 = heap)
///   bits 1..32   — capacity
///   bits 32..64  — length
///
/// The previous packing `(len << 32) | (cap << 32) | 1` shifted both fields
/// by 32, so `cap` and `len` OR'd into the SAME bits and were unrecoverable.
/// The disjoint layout lets a reader round-trip len/cap/tag from the header.
// 2026-08-15 (coll plan §3.5): pack_svo_header REMOVED — SVO (Small Vector
// Optimization) was never enabled in production; lists construct via the
// coll scaffolded ops.

use crate::backend::llvm::intrinsics::{
    emit_intrinsic_call, template_for_op,
};
use crate::backend::llvm::types;
use crate::backend::llvm::{LlvmBackend, TypedRegister};

use std::fmt::Write;

impl LlvmBackend {
    /// Emit LLVM IR for an expression. Returns the result register and type.
    /// This is the entry point for all expression codegen.
    pub(crate) fn emit_expr(
        &mut self,
        out: &mut String,
        expr: &Expr,
        indent: &str,
    ) -> TypedRegister {
        let expr = expr.clone();
        let v = self.fun.gen_reg();
        let indent = if indent.is_empty() { "  " } else { indent };
        self.emit_expr_inner(out, &v, &expr, indent)
    }

    /// Inner dispatch: one arm per Expr variant.
    pub(crate) fn emit_expr_inner(
        &mut self,
        out: &mut String,
        v: &str,
        expr: &Expr,
        indent: &str,
    ) -> TypedRegister {
        match expr {
            // ── Literals ─────────────────────────────────────────────
            Expr::Consume(inner) => {
                // 2026-08-01 (Phase 3): the consumed operand's value is used
                // normally; its backing is destroyed after the enclosing op —
                // recorded here and freed at the statement boundary.
                let reg = self.emit_expr(out, inner, indent);
                self.fun.pending_consumes.push(reg.name.clone());
                reg
            }
            // 2026-08-09 (Phase 10): `await task` — the task handle already
            // holds the result (deterministic inline execution); await reads it.
            Expr::Await(inner) => self.emit_expr(out, inner, indent),
            Expr::Decimal(n) => {
                self.emit_int(out, v, *n, indent)
            }
            Expr::Char(c) => {
                // 2026-08-01 (audit): a Char literal emits at its NATIVE i32
                // width (the universe declares Char = 32-bit), matching Char
                // state fields and cast results. Boxed Char params are the
                // sole i64 exception (zext at defn entry). The Print#
                // dispatch and adapt_to_i64 widen i32 → i64 for the runtime
                // ABI; the register's ty carries `#Char` so the generic
                // Print# routes it to __print_char (not __print_int).
                writeln!(out, "{}{} = add i32 0, {}", indent, v, *c as i64).ok();
                TypedRegister { name: v.to_string(), ty: Type::char_() }
            }
            Expr::TaggedLiteral(n, _) => {
                self.emit_int(out, v, *n, indent)
            }
            Expr::Float(f) => {
                // 2026-07-29: Direct bitcast from i32 hex bits to float.
                // The `add i32 0, N` + `fadd float 0.0` wrappers were removed
                // — LLVM IR accepts i32 bit patterns directly in bitcast, and
                // the bitcast result is already float-typed without fadd.
                // The hex bits avoid LLVM's verifier rejecting high-precision
                // float literals like "0.001660076642744037" as f32.
                let h = crate::backend::llvm::float_to_llvm_hex(*f);
                writeln!(out, "{}{} = bitcast i32 {} to float", indent, v, h).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::float(),
                }
            }
            Expr::Bool(b) => {
                let val = if *b { 1 } else { 0 };
                writeln!(out, "{}{} = add i8 0, {}", indent, v, val).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            Expr::BeginProgram => {
                // True exactly once at program start (entry-loop). Reads the
                // node's `@briev_begin_<name>` flag; the node's body clears it
                // when its goal is met, so the precondition stops gating after
                // the entry loop completes. Outside a transaction (no txn_name)
                // it evaluates as `true`.
                let flag = format!("@briev_begin_{}", self.fun.txn_name);
                if self.fun.txn_name.is_empty() {
                    writeln!(out, "{}{} = add i8 0, 1", indent, v).ok();
                } else {
                    let loaded = self.fun.gen_reg();
                    writeln!(out, "{}{} = load i1, ptr {}", indent, loaded, flag).ok();
                    writeln!(out, "{}{} = zext i1 {} to i8", indent, v, loaded).ok();
                }
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            Expr::Quoted(bytes) => self.emit_string_literal(out, v, bytes, indent),
            // 2026-08-06 (Phase 7): `#b"..."` is a raw-bytes Data literal —
            // the [len][bytes] constant must carry the EXACT bytes (the lossy
            // UTF-8 string path would turn \x89 into the replacement char).
            Expr::TaggedQuotedLiteral(bytes, prefix) if prefix == "b" => {
                self.emit_byte_literal(out, v, bytes, indent)
            }
            Expr::TaggedQuotedLiteral(bytes, _) => self.emit_string_literal(out, v, bytes, indent),

            // ── Identifier ───────────────────────────────────────────
            Expr::Identifier(name) => {
                // 2026-08-07 (object instance pools): a bare member name in an
                // UNPACKED member body resolves to the instance's top-level
                // slot — `data` inside `st`'s push → the `st.data` field.
                if let Some((prefix, row_reg)) = self.fun.self_prefix.clone() {
                    let slot = format!("{}.{}", prefix, name);

                    if let Some(&idx) = self.ctx.field_index_map.get(&slot) {
                        let (row, row_ty, load_ty) = self.emit_instance_column_row(out, indent, idx, &row_reg);
                        if matches!(&row_ty, Type::Vector(_, _)) {
                            return TypedRegister { name: row, ty: row_ty };
                        }
                        let loaded = self.fun.gen_reg();
                        writeln!(out, "{}{} = load {}, ptr {}", indent, loaded, load_ty, row).ok();
                        return TypedRegister { name: loaded, ty: row_ty };
                    }
                    // 2026-08-09 (Bug 1 / Phase 5): a BOXED/SPILLED member body
                    // has no `{base}.{member}` field slot (the instance is a
                    // per-heap block, not a pooled column) — resolve the member
                    // through the boxed_offsets layout directly: inttoptr the
                    // handle + GEP the byte offset.
                    if let Some(offsets) = self.ctx.boxed_offsets.get(prefix.as_str()) {
                        if let Some((off, mty)) = offsets.get(name) {
                            let ptr = self.fun.gen_reg();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, row_reg).ok();
                            let gep = self.fun.gen_reg();
                            writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, gep, ptr, off).ok();
                            let llvm_ty = if matches!(mty, Type::Ptr(_)) {
                                "i64".to_string()
                            } else {
                                self.llvm_type(mty)
                            };
                            let loaded = self.fun.gen_reg();
                            writeln!(out, "{}{} = load {}, ptr {}", indent, loaded, llvm_ty, gep).ok();
                            return TypedRegister { name: loaded, ty: mty.clone() };
                        }
                    }
                }
    /// 2026-08-07 (Phase 7): a let MUTATED via `x = ...` is
                /// redirected to an alloca slot by the assign — reads must load
                // from the slot (fresh across loop iterations), NOT resolve a
                // stale `last_val_temps` register that is not dominated in a
                // loop (accumulating `acc = acc + i` in a foreach summed to
                // zero). Checked before last_val_temps.
                let slot_opt: Option<String> = self
                    .fun
                    .let_bindings
                    .get(name)
                    .filter(|r| self.fun.let_binding_allocas.contains(*r))
                    .cloned();
                if let Some(slot) = slot_opt {
                    let briev_ty = self.get_local_type(name);
                    let llvm_ty = self.llvm_type(&briev_ty);
                    let loaded = self.fun.gen_reg();
                    writeln!(out, "{}{} = load {}, ptr {}, align 8", indent, loaded,
                        llvm_ty, slot).ok();
                    return TypedRegister {
                        name: loaded,
                        ty: briev_ty,
                    };
                }
                // 2026-08-06 (fix): a closure-let identifier reads its env
                // block address (a real first-class value) — resolved by the
                // normal let-binding path below.
                // 2026-07-29: Accumulation chaining — check last_val_temps FIRST.
                // When a field is written multiple times in one iteration, the second
                // read must return the just-computed value, not the loop-header phi,
                // so the first write forms a live dependency chain (not dead code).
                if let Some(reg) = self.fun.last_val_temps.get(name) {
                    let briev_ty = self
                        .fun
                        .last_val_types
                        .get(name)
                        .cloned()
                        .or_else(|| {
                            self.ctx
                                .field_index_map
                                .get(name)
                                .and_then(|idx| self.ctx.field_briev_types.get(*idx).cloned())
                        })
                        .unwrap_or(Type::int());
                    return TypedRegister {
                        name: reg.clone(),
                        ty: briev_ty,
                    };
                }
                // 2026-07-29: Path 2 — Vector phi group extractelement.
                // Checked after last_val_temps so that intra-iteration writes
                // to vector-grouped fields (if any) resolve correctly.
                if !self.fun.active_vector_groups.is_empty() {
                    let groups_clone = self.fun.active_vector_groups.clone();
                    let f2p_clone = self.fun.field_to_phi.clone();
                    let f2l_clone = self.fun.field_to_lane.clone();
                    if let Some(lane_reg) = crate::backend::llvm::vector_phi::emit_extractelement(
                        &mut self.fun, out, name, &groups_clone,
                        &f2p_clone, &f2l_clone, indent,
                    ) {
                        let briev_ty = self
                            .ctx
                            .field_index_map
                            .get(name)
                            .and_then(|idx| self.ctx.field_briev_types.get(*idx).cloned())
                            .unwrap_or(Type::int());
                        return TypedRegister { name: lane_reg, ty: briev_ty };
                    }
                }
                // 2026-07-31 (A5): obj member `self` slot read — a bare slot
                // name in a member body resolves to self+offset (GEP + load).
                // Array slots are skipped here — `data[i]` is handled by the
                // Index arm (self-slot array GEP), not loaded as a scalar.
                let self_binding = self.fun.self_binding.clone();
                if let Some((self_type, self_ptr)) = &self_binding {
                    let is_self_slot = self.ctx.struct_types.get(self_type)
                        .map_or(false, |f| f.iter().any(|(n, _)| n == name));
                    if is_self_slot {
                        let (slot_ty, _) = self.ctx.struct_types.get(self_type)
                            .and_then(|f| f.iter().find(|(n, _)| n == name))
                            .map(|(_, ty)| (ty.clone(), ()))
                            .unwrap_or((Type::int(), ()));
                        if !matches!(slot_ty, Type::Vector(_, _)) {
                            let offset = self.lookup_field_offset(self_type, name);
                            let gep = self.fun.gen_reg();
                            writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, gep, self_ptr, offset).ok();
                            // 2026-08-01 (D3): a STRUCT-typed self-slot
                            // (`inner: ListBuffer<T>`) — its value is its
                            // ADDRESS (the instance lives in the obj's
                            // storage), like state-slots/struct-literals. Field
                            // access on it (`inner.data`) GEPs the address;
                            // loading the aggregate would break
                            // emit_field_access's inttoptr base.
                            let is_struct_slot = match &slot_ty {
                                Type::Custom(n) => self.ctx.struct_types.contains_key(n),
                                Type::Applied(n, _) => self.ctx.struct_types.contains_key(n),
                                _ => false,
                            };
                            if is_struct_slot {
                                let addr = self.fun.gen_reg();
                                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, addr, gep).ok();
                                return TypedRegister { name: addr, ty: slot_ty };
                            }
                            // 2026-08-13 (Phase 5): an `atomic` self-slot reads
                            // with an atomic load.
                            if self.is_atomic_field(self_type, name) {
                                let alt = if matches!(slot_ty, Type::Ptr(_)) {
                                    format!("i{}", self.ctx.int_bits)
                                } else {
                                    self.llvm_type(&slot_ty)
                                };
                                let asz = types::type_size(&slot_ty, self.ctx.type_universe.as_ref()).max(1);
                                let aval = self.fun.gen_reg();
                                writeln!(out, "{}{} = load atomic {}, ptr {} seq_cst, align {}", indent, aval, alt, gep, asz).ok();
                                return TypedRegister { name: aval, ty: slot_ty };
                            }
                            // 2026-08-13 (pack): a packed self-slot reads its
                            // bit-slice out of the byte image (whole-byte =
                            // plain aligned load); typed Bits(bits) — the
                            // register's true width.
                            if let Some(pf) = self.packed_field(self_type, name) {
                                let pv = self.emit_packed_field_load(out, indent, &gep, &pf);
                                return TypedRegister { name: pv, ty: Type::Bits(pf.bits) };
                            }
                            // 2026-08-01 (D3): a Ptr-typed self-slot stores the
                            // HANDLE at i{int_bits} (ptrtoint at store) — load
                            // that width, not `ptr`, so inttoptr consumers
                            // (`buckets[h]`) work. 2026-08-11: width-aware —
                            // a wasm32 pointer slot is i32, not hardcoded i64.
                            let llvm_ty = if matches!(slot_ty, Type::Ptr(_)) {
                                format!("i{}", self.ctx.int_bits)
                            } else {
                                self.llvm_type(&slot_ty)
                            };
                            let val = self.fun.gen_reg();
                            writeln!(out, "{}{} = load {}, ptr {}", indent, val, llvm_ty, gep).ok();
                            return TypedRegister { name: val, ty: slot_ty };
                        }
                        // 2026-08-01 (A10): a self-slot ARRAY name (`data` in
                        // `data[i]`) is consumed by the Index arm's self-slot
                        // GEP path, so its identifier read is dead. Return the
                        // self pointer as a placeholder — falling through to
                        // the global lookup emitted an undefined `@data` global
                        // (the scalar self-slot read above deliberately skips
                        // Vector slots).
                        return TypedRegister { name: self_ptr.clone(), ty: slot_ty };
                    }
                }
                // 2026-07-17: Remaining paths: local binding,
                // phi register, state field, global constant.
                if let Some(reg) = self.get_local(name) {
                    // 2026-07-18: If the binding is an alloca (param slot,
                    // uninitialized let, or txn param slot), emit a load.
                    if self.fun.param_slots.values().any(|s| s == &reg)
                        || self.fun.let_binding_allocas.contains(&reg)
                    {
                        let loaded = self.fun.gen_reg();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, loaded, reg).ok();
                        let ty = self.get_local_type(name);
                        // 2026-08-11 (Phase 2a2 fix): a BOXED param slot holds
                        // the boxed i64 machine word (String/Data = address,
                        // Char = native i32, Bool = i8). Unbox at the load —
                        // the same conversion state-field reads apply — so the
                        // value's SSA type matches its register. Previously the
                        // param was typed `int()`, which on wasm32 is i32, so a
                        // String param was re-widened (`sext i32`) on store.
                        if self.is_string_operand(&ty) || self.is_blob_operand(&ty) {
                            let p = self.fun.gen_reg();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, loaded).ok();
                            TypedRegister { name: p, ty }
                        } else if self.is_protocol_member(&ty, "#Char") {
                            let t = self.fun.gen_reg();
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, t, loaded).ok();
                            TypedRegister { name: t, ty }
                        } else if self.is_protocol_member(&ty, "#Bool") {
                            let t = self.fun.gen_reg();
                            writeln!(out, "{}{} = trunc i64 {} to i8", indent, t, loaded).ok();
                            TypedRegister { name: t, ty }
                        } else {
                            // 2026-08-11 (Phase 2a2 fix): a narrow INTEGER
                            // param (`Int` on wasm32 is i32) is widened to the
                            // i64 slot at function entry; reading it back must
                            // truncate to the native width so the register type
                            // matches the value's Briev type (i32, not i64).
                            let lt = self.llvm_type(&ty);
                            if lt.starts_with('i') && lt != "i64" {
                                let t = self.fun.gen_reg();
                                writeln!(out, "{}{} = trunc i64 {} to {}", indent, t, loaded, lt).ok();
                                TypedRegister { name: t, ty }
                            } else {
                                TypedRegister { name: loaded, ty }
                            }
                        }
                    } else {
                        let ty = self.get_local_type(name);
                        // 2026-08-13: a boxed Bool/Char param is registered in
                        // let_binding_types as Int (emit_definition line 2066);
                        // let_original_types keeps the true Briev type. Recover
                        // it so the Char case can unbox below.
                        let orig_ty = self.fun.let_original_types.get(name).cloned().unwrap_or_else(|| ty.clone());
                        // 2026-08-13: a Float/Double param is boxed to an i64
                        // handle at function entry (emit_box_param), with the
                        // native float register cached in reg_float_cache. Any
                        // read of the local must yield the NATIVE float, else a
                        // `f as String` cast emits `call float_to_str(float %boxed_i64)`
                        // (a type mismatch). Unbox through the cache.
                        if (ty == Type::float() || ty == Type::float64())
                            && let Some(cached) = self.fun.reg_float_cache.get(&reg)
                        {
                            TypedRegister { name: cached.clone(), ty }
                        } else if self.is_protocol_member(&orig_ty, "#Char") {
                            // 2026-08-13: a Char param is boxed to i64 at
                            // entry (emit_box_param "zext.i32.to.i64#") but its
                            // native register is i32. A comparison against a
                            // Char literal (`c >= ' '`) emits the literal as
                            // i32, so the read must truncate the box to i32
                            // (an `icmp eq i64 %ac0, i32 %t7` is a mismatch).
                            // The register keeps the CHAR type (not the boxed
                            // Int) so downstream casts dispatch to char_to_str
                            // and Print# routes to __print_char.
                            let t = self.fun.gen_reg();
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, t, reg).ok();
                            TypedRegister { name: t, ty: orig_ty }
                        } else {
                            TypedRegister {
                                name: reg.clone(),
                                ty,
                            }
                        }
                    }
                } else if let Some(phi_reg_str) = self.fun.phi_field_regs.get(name).cloned() {
                    let briev_ty = self
                        .ctx
                        .field_index_map
                        .get(name)
                        .and_then(|idx| self.ctx.field_briev_types.get(*idx).cloned())
                        .unwrap_or(Type::int());
                    if briev_ty == Type::float64() {
                        // 2026-07-21: With native float types, phi is already double.
                        // Check field_types to determine if conversion is needed.
                        let is_native = self.ctx.field_index_map.get(name)
                            .and_then(|idx| self.ctx.field_types.get(*idx))
                            .map_or(false, |t| t == "double");
                        if is_native {
                            let dbl_reg = phi_reg_str.clone();
                            self.fun.reg_float_cache.insert(phi_reg_str, dbl_reg.clone());
                            TypedRegister { name: dbl_reg, ty: Type::float64() }
                        } else {
                            let dbl = self.fun.gen_reg();
                            writeln!(out, "{}{} = bitcast i64 {} to double", indent, dbl, phi_reg_str).ok();
                            self.fun.reg_float_cache.insert(phi_reg_str, dbl.clone());
                            TypedRegister { name: dbl, ty: Type::float64() }
                        }
                    } else if briev_ty == Type::float() {
                        // 2026-07-21: With native float types, phi is already float.
                        let is_native = self.ctx.field_index_map.get(name)
                            .and_then(|idx| self.ctx.field_types.get(*idx))
                            .map_or(false, |t| t == "float");
                        if is_native {
                            let fl_reg = phi_reg_str.clone();
                            self.fun.reg_float_cache.insert(phi_reg_str, fl_reg.clone());
                            TypedRegister { name: fl_reg, ty: Type::float() }
                        } else {
                            let tr = self.fun.gen_reg();
                            let fl = self.fun.gen_reg();
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, phi_reg_str).ok();
                            writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
                            self.fun.reg_float_cache.insert(phi_reg_str, fl.clone());
                            TypedRegister { name: fl, ty: Type::float() }
                        }
                    } else {
                        TypedRegister {
                            name: phi_reg_str,
                            ty: briev_ty,
                        }
                    }
                } else if let Some(&idx) = self.ctx.field_index_map.get(name) {
                    // 2026-07-19: Load with native type from field_types (e.g. "float",
                    // "double", "i64"). No unboxing needed — LLVM type matches %State
                    // struct layout. Phi registers remain i64 (handled above).
                    // 2026-07-20: State fields are always i64 in %State. For float-typed
                    // fields, trunc+bitcast i64 → float so downstream arithmetic gets
                    // correct types (matches the phi path at lines 100-104).
                    let (loaded, briev_ty) = self.emit_state_load_i64_by_idx(out, indent, idx);
                    // 2026-07-21: With native float types, the load already returns
                    // float/double. Check field_types[idx] to skip the conversion.
                    let field_llvm_ty = self.ctx.field_types.get(idx)
                        .cloned().unwrap_or_else(|| "i64".to_string());
                    if briev_ty == Type::float64() && field_llvm_ty == "double" {
                        TypedRegister { name: loaded, ty: Type::float64() }
                    } else if briev_ty == Type::float() && field_llvm_ty == "float" {
                        TypedRegister { name: loaded, ty: Type::float() }
                    } else if briev_ty == Type::float64() {
                        let dbl = self.fun.gen_reg();
                        writeln!(out, "{}{} = bitcast i64 {} to double", indent, dbl, loaded).ok();
                        TypedRegister {
                            name: dbl,
                            ty: Type::float64(),
                        }
                    } else if briev_ty == Type::float() {
                        let tr = self.fun.gen_reg();
                        let fl = self.fun.gen_reg();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, loaded).ok();
                        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
                        TypedRegister {
                            name: fl,
                            ty: Type::float(),
                        }
                    } else if self.is_string_operand(&briev_ty) || self.is_blob_operand(&briev_ty) {
                        // 2026-08-01 (B0): A Briev String value is a ptr to a
                        // length-prefixed [len][bytes] buffer. State slots hold
                        // the address as an i64 machine word (uniform %State
                        // layout, push_field_type), so a String field load must
                        // inttoptr the slot back to the ptr representation —
                        // mirroring the float unboxing branches above and the
                        // Ptr<T> state-adapter pattern.
                        // 2026-08-07 (Phase 7): Data shares the [len][bytes]
                        // representation (#Blob protocol) — its state slots
                        // must inttoptr the same way.
                        let str_p = self.fun.gen_reg();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, str_p, loaded).ok();
                        TypedRegister {
                            name: str_p,
                            ty: briev_ty,
                        }
                    } else {
                        // 2026-08-10: flexible Int/UInt %State slots are now
                        // i{int_bits} (push_field_type), so `loaded` is already
                        // the arithmetic width — no trunc needed. Bool/String/
                        // Data/Ptr slots stay i64 and are boxed/unboxed above.
                        TypedRegister {
                            name: loaded,
                            ty: briev_ty,
                        }
                    }
                } else if let Some((ty, _)) = self.ctx.constants.get(name) {
                    // 2026-07-17: Load global constants with the correct LLVM
                    // type. Float constants are declared as `constant float` in
                    // the IR (not i64), so loading them as i64 produces garbage
                    // bits and type mismatches in float operations.
                    if *ty == Type::float() {
                        writeln!(out, "{}{} = load float, ptr @{}", indent, v, name).ok();
                        TypedRegister {
                            name: v.to_string(),
                            ty: Type::float(),
                        }
                    } else if *ty == Type::float64() {
                        writeln!(out, "{}{} = load double, ptr @{}", indent, v, name).ok();
                        TypedRegister {
                            name: v.to_string(),
                            ty: Type::float64(),
                        }
                    } else if self.is_string_operand(ty) {
                        // 2026-08-01 (B3): a String constant's @s global holds
                        // the [len][bytes] pointer; load it as a ptr and type it
                        // with the DECLARED constant type (a #String member, not
                        // a hardcoded Type::string()) so reflection/ops see the
                        // right protocol. Was load i64 typed Int, which broke
                        // `s.^Length` on an unwritten literal (const-folded to a
                        // global).
                        writeln!(out, "{}{} = load ptr, ptr @{}", indent, v, name).ok();
                        TypedRegister {
                            name: v.to_string(),
                            ty: ty.clone(),
                        }
                    } else {
                        writeln!(out, "{}{} = load i64, ptr @{}", indent, v, name).ok();
                        TypedRegister {
                            name: v.to_string(),
                            ty: Type::int(),
                        }
                    }
                } else if let Some(init) = self.ctx.inits.get(name) {
                    // 2026-08-09 (init kind, Phase 2): a runtime-seeded
                    // invariant reads its seeded global. Load with the declared
                    // type (like the constants path) — the global is a mutable
                    // i64/float/double/ptr slot seeded once in the pre-reactor
                    // phase and never written again.
                    let ty = &init.ty;
                    if *ty == Type::float() {
                        writeln!(out, "{}{} = load float, ptr @{}", indent, v, name).ok();
                        TypedRegister { name: v.to_string(), ty: Type::float() }
                    } else if *ty == Type::float64() {
                        writeln!(out, "{}{} = load double, ptr @{}", indent, v, name).ok();
                        TypedRegister { name: v.to_string(), ty: Type::float64() }
                    } else if self.is_string_operand(ty) || matches!(ty, Type::Ptr(_)) {
                        writeln!(out, "{}{} = load ptr, ptr @{}", indent, v, name).ok();
                        TypedRegister { name: v.to_string(), ty: ty.clone() }
                    } else {
                        writeln!(out, "{}{} = load i64, ptr @{}", indent, v, name).ok();
                        TypedRegister { name: v.to_string(), ty: Type::int() }
                    }
                } else {
                    writeln!(out, "{}{} = load i64, ptr @{}", indent, v, name).ok();
                    TypedRegister {
                        name: v.to_string(),
                        ty: Type::int(),
                    }
                }
            }

            // ── Call ─────────────────────────────────────────────────
            // 2026-07-12: Intrinsic call if name ends with '#', else user call.
            Expr::Call(name, args, analysis_id) => {
                if name.ends_with('#') {
                    self.emit_intrinsic_call_dispatch(out, v, name, args, *analysis_id, indent)
                } else if self.ctx.struct_types.contains_key(name) {
                    // 2026-07-31 (A5): struct constructor call — `Stack()` /
                    // `Person("Alice", 30)` — emits a struct literal with the
                    // args as positional fields.
                    let fields: Vec<(String, Expr)> = self
                        .ctx
                        .struct_types
                        .get(name)
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                        .map(|(i, (fname, _))| {
                            let value = args
                                .get(i)
                                .cloned()
                                .unwrap_or_else(|| Expr::Decimal(0));
                            (fname.clone(), value)
                        })
                        .collect();
                    self.emit_struct_literal(out, v, name, &fields, indent)
                } else {
                    self.emit_user_call(out, v, name, args, indent)
                }
            }

            // ── BinaryOp ─────────────────────────────────────────────
            Expr::BinaryOp(kind, lhs, rhs) => {
                // 2026-07-21: Mod with phi-tracked counter dividend — trunc i64
                // to i32 so LLVM uses imul $magic (1 uop) instead of mul %reg
                // (3 uops, 128-bit) for modulo-by-constant optimization. The
                // counter is bounded by its loop precondition (< 2^31), so the
                // truncation is safe. Non-counter values skip this optimization.
                // Check BEFORE emit_expr so we can inspect the AST (field name).
                if matches!(kind, crate::ast::BinaryOpKind::Mod)
                    && matches!(lhs.as_ref(), Expr::Identifier(name)
                        if self.fun.phi_field_regs.contains_key(name))
                {
                    let l = self.emit_expr(out, lhs, indent);
                    let r = self.emit_expr(out, rhs, indent);
                    let tr_l = self.fun.gen_reg();
                    let tr_r = self.fun.gen_reg();
                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr_l, l.name).ok();
                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr_r, r.name).ok();
                    let ur = self.fun.gen_reg();
                    writeln!(out, "{}{} = urem i32 {}, {}", indent, ur, tr_l, tr_r).ok();
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ur).ok();
                    TypedRegister { name: v.to_string(), ty: Type::int() }
                } else {
                    let l = self.emit_expr(out, lhs, indent);
                    let r = self.emit_expr(out, rhs, indent);
                    // 2026-07-21: Convert operands to matching types if they differ
                    // (e.g., i64 constant in float operation). Native float types in
                    // %State expose this — previously all values were i64.
                    if l.ty != r.ty && (l.ty == Type::float() || r.ty == Type::float()) {
                        let (conv_l, conv_r) = if l.ty == Type::float() && r.ty == Type::int() {
                            let conv = self.fun.gen_reg();
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, conv, r.name).ok();
                            let fl = self.fun.gen_reg();
                            writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, conv).ok();
                            (l.clone(), TypedRegister { name: fl, ty: Type::float() })
                        } else if l.ty == Type::int() && r.ty == Type::float() {
                            let conv = self.fun.gen_reg();
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, conv, l.name).ok();
                            let fl = self.fun.gen_reg();
                            writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, conv).ok();
                            (TypedRegister { name: fl, ty: Type::float() }, r.clone())
                        } else {
                            (l.clone(), r.clone())
                        };
                        self.emit_binary_op(out, v, kind, &conv_l, &conv_r, indent)
                    } else {
                        self.emit_binary_op(out, v, kind, &l, &r, indent)
                    }
                }
            }

            // ── UnaryOp ──────────────────────────────────────────────
            Expr::UnaryOp(kind, e) => {
                let operand = self.emit_expr(out, e, indent);
                self.emit_unary_op(out, v, kind, &operand, indent)
            }

            // ── Block ────────────────────────────────────────────────
            Expr::Block(stmts) => {
                let mut last = TypedRegister {
                    name: v.to_string(),
                    ty: Type::void(),
                };
                for stmt in stmts {
                    last = self.emit_statement(out, stmt, indent);
                }
                last
            }

            // ── If ───────────────────────────────────────────────────
            Expr::If(cond, then, else_) => {
                let cond_reg = self.emit_expr(out, cond, indent);
                let then_l = self.fun.gen_reg();
                let else_l = self.fun.gen_reg();
                let end_l = self.fun.gen_reg();
                let then_lbl = format!("if.then{}", then_l);
                let else_lbl = format!("if.else{}", else_l);
                let end_lbl = format!("if.end{}", end_l);
                writeln!(
                    out,
                    "{}br i1 {}, label %{}, label %{}",
                    indent, cond_reg.name, then_lbl, else_lbl
                )
                .ok();
                writeln!(out, "{}{}:", indent, then_lbl).ok();
                let then_reg = self.emit_expr(out, then, indent);
                writeln!(out, "{}br label %{}", indent, end_lbl).ok();
                writeln!(out, "{}{}:", indent, else_lbl).ok();
                let else_reg = match else_ {
                    Some(e) => self.emit_expr(out, e, indent),
                    None => TypedRegister {
                        name: self.fun.gen_reg(),
                        ty: Type::void(),
                    },
                };
                writeln!(out, "{}br label %{}", indent, end_lbl).ok();
                writeln!(out, "{}{}:", indent, end_lbl).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: then_reg.ty,
                }
            }

            // ── Tuple ────────────────────────────────────────────────
            Expr::Tuple(exprs) => self.emit_heap_seq(out, v, exprs, indent),

            // ── List literal ─────────────────────────────────────────
            Expr::List(exprs) => {
                // 2026-07-24: Struct array optimization. When ALL elements
                // are struct literals of the same known struct type, emit
                // a contiguous stack array instead of a heap-allocated list.
                // This produces a C-compatible pointer for bridge code.
                if let Some(elem_ty) = self.detect_struct_list(exprs) {
                    return self.emit_struct_array(out, v, exprs, &elem_ty, indent);
                }
                // 2026-08-15 (coll plan §3.5): SVO removed (feature_svo was
                // never enabled in production — the coll scaffold constructs
                // lists through the collection ops). A bare list literal in
                // expression position falls to the heap-seq layout (tuples
                // and untyped products keep it).
                self.emit_heap_seq(out, v, exprs, indent)
            }

            // ── Struct literal ─────────────────────────────────────────
            Expr::StructLiteral { type_name, fields } => {
                self.emit_struct_literal(out, v, type_name, fields, indent)
            }

            // ── Field access ─────────────────────────────────────────
            Expr::Field(obj, field) => {
                // 2026-08-07 (object instance pools): an unpacked obj
                // instance's member (`b.total`, `b.data`) is a top-level field
                // slot `{recv}.{member}`. A scalar member loads the slot; an
                // array member returns a PTR to the slot's array (typed with
                // its Vector), so a following `[i]` indexes it via the
                // row-view path. Checked BEFORE emitting the receiver — `b`
                // itself has no slot (it unpacked), so emitting it would
                // produce an undefined `@b` global.
                if let Expr::Identifier(recv_name) = obj.as_ref() {
                    if let Some((base, row_reg)) = self.instance_prefix_for(recv_name) {
                        let slot = format!("{}.{}", base, field);
                        if let Some(&idx) = self.ctx.field_index_map.get(&slot) {
                            let (row, row_ty, load_ty) = self.emit_instance_column_row(out, indent, idx, &row_reg);
                            if matches!(&row_ty, Type::Vector(_, _)) {
                                return TypedRegister { name: row, ty: row_ty };
                            }
                            let loaded = self.fun.gen_reg();
                            writeln!(out, "{}{} = load {}, ptr {}", indent, loaded, load_ty, row).ok();
                            return TypedRegister { name: loaded, ty: row_ty };
                        }
                    }
                }
                let obj_reg = self.emit_expr(out, obj, indent);
                // 2026-07-14: Layout field access — #fieldname triggers bit-shift/mask
                if field.starts_with('#') {
                    return self.emit_layout_field_read(out, v, &obj_reg, field, indent);
                }
                // 2026-07-25: Array field access — GEP into struct for Int[N] fields.
                // Check if the field type is an array (Type::Vector with dimension).
                let field_idx = self.resolve_field_index(&obj_reg.ty, field);
                let field_ty = self.resolve_field_type(&obj_reg.ty, field);
                if let Some(Type::Vector(inner, dims)) = &field_ty {
                    if !dims.is_empty() {
                        // Emit GEP to get a pointer to the array field.
                        // 2026-07-31 (A7): the receiver register holds the
                        // struct's ADDRESS (state-slot or struct-literal), so
                        // GEP the address + field offset directly — no alloca
                        // spill (which treated the address as a struct value).
                        // 2026-08-07: MULTI-dim — the field read returns a PTR
                        // to the array (typed with the FULL Vector) so a
                        // following index (`m.data[i][j]`) GEPs through the
                        // row-view path.
                        let obj_type = match &obj_reg.ty {
                            Type::Custom(n) => n.clone(),
                            _ => {
                                if let Expr::Identifier(nm) = obj.as_ref() {
                                    self.ctx.field_index_map.get(nm)
                                        .and_then(|i| self.ctx.field_briev_types.get(*i))
                                        .and_then(|t| match t {
                                            Type::Custom(n) => Some(n.clone()),
                                            _ => None,
                                        })
                                } else { None }
                            }.unwrap_or_else(|| "".to_string()),
                        };
                        let offset = self.lookup_field_offset(&obj_type, field);
                        let base = self.fun.gen_reg();
                        // 2026-08-11 (wasm32): obj handles are i{int_bits}.
                        let hw = format!("i{}", self.ctx.int_bits);
                        writeln!(out, "{}{} = inttoptr {} {} to ptr", indent, base, hw, obj_reg.name).ok();
                        let gep = self.fun.gen_reg();
                        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, gep, base, offset).ok();
                        let _ = (field_idx, inner);
                        return TypedRegister {
                            name: gep,
                            ty: field_ty.clone().unwrap(),
                        };
                    }
                }
                // Struct field access via extractvalue (numeric index required)
                // 2026-07-31 (A7): replaced by emit_field_access — a struct
                // field's register holds the instance ADDRESS (state-slot or
                // struct-literal), so GEP-by-offset + load is the correct
                // access; extractvalue on an address is invalid IR.
                let _ = (field_idx, field_ty);
                return self.emit_field_access(out, v, obj, field, indent);
            }

            // ── Index ────────────────────────────────────────────────
            // 2026-07-14: List/seq indexing uses 2-slot heap protocol.
            // Ptr-typed values are heap-allocated buffers; others use extractelement.
            // 2026-07-17: Raw Ptr buffers (from Malloc#) use index directly.
            // List/tuple heap sequences use idx+1 (slot 0 = length header).
            // Check the original AST expression to decide.
            Expr::Index(obj, index) => {
                let obj_reg = self.emit_expr(out, obj, indent);
                let idx_reg = self.emit_expr(out, index, indent);
                // 2026-08-13 (c[i] dispatches op At): a Tier-2 collection
                // (`op Count` + `op At`) index must inline the At member body —
                // the legacy struct/heap-seq paths below read a List value's
                // fields directly (`b[0]` read the `inner` slot, then fed the
                // buffer pointer to briev_char_len → garbage/crash). Same
                // dispatch foreach uses. tier2_op_collection handles identifier
                // receivers; a call-result receiver (`bytes("abc")[0]`) is
                // dispatched by its register's type.
                let recv_is_tier2 = self.tier2_op_collection(obj).is_some()
                    || {
                        let base = match &obj_reg.ty {
                            Type::Custom(n) | Type::Applied(n, _) => Some(n.clone()),
                            _ => None,
                        };
                        base.map_or(false, |b| {
                            self.ctx.obj_members.get(&b).map_or(false, |members| {
                                members.iter().any(|m| matches!(m, TopLevel::TypeDefOperator(d) if d.name == "At"))
                            })
                        })
                    };
                if recv_is_tier2 {
                    // 2026-08-13: inline the At member body. The member's
                    // `term inner.data[i]` already unboxes String/Data elements
                    // (the Index arm's String-element path inttoprts), so no
                    // extra conversion here.
                    let out_tmp = self.fun.gen_reg();
                    return self.emit_method_call(out, &out_tmp, obj, "At", &[(**index).clone()], indent);
                }
                // 2026-08-10: clone so gep_index can take &mut self while the
                // struct_types borrow for self-array slots is live below.
                let idx_clone = idx_reg.clone();
                // 2026-08-07 (Phase 7): a Boolean-vector index is a MASK —
                // `data[mask]` selects the bytes at the true positions
                // (SPEC §16.5). Supported sources: a compile-time Boolean
                // list literal or a Bool[N] state field.
                let mask_source = {
                    let const_bits = Self::constant_bool_mask(index);
                    let field_idx = match index.as_ref() {
                        Expr::Identifier(name) => self.ctx.field_index_map.get(name).copied().filter(
                            |fidx| matches!(
                                self.ctx.field_briev_types.get(*fidx),
                                Some(t) if matches!(t, Type::Vector(inner, _)
                                    if self.is_protocol_member(inner, "#Bool"))
                            ),
                        ),
                        _ => None,
                    };
                    match const_bits {
                        Some(bits) => Some(MaskSource::Constant(bits)),
                        None => field_idx.map(MaskSource::StateField),
                    }
                };
                if let Some(source) = mask_source {
                    return self.emit_masked_index(
                        out,
                        MaskIndexOperands {
                            obj,
                            obj_reg: &obj_reg,
                            source,
                        },
                        indent,
                    );
                }
                // 2026-08-01 (D3): a Ptr-index read returns the POINTEE type
                // (`buckets[h]` on a Ptr<List<...>> → List<...>); a heap List
                // index returns its ELEMENT type (`List<String>[i]` → String).
                // Computed early so the load path can inttoptr string elements.
                let index_elem_ty = match &obj_reg.ty {
                    Type::Ptr(inner) => (**inner).clone(),
                    Type::Applied(_, args) if !args.is_empty() => args[0].clone(),
                    _ => Type::int(),
                };
                // 2026-07-18: SVO List indexing — removed (2026-08-15, coll
                // plan §3.5: feature_svo never enabled; coll indexes via
                // `op At`).
                // 2026-07-31 (A5): obj member `self` ARRAY slot indexing —
                // `data[i]` in a member body. GEP self + slot offset + elem.
                if let Some((self_type, self_ptr)) = self.fun.self_binding.clone() {
                     if let Expr::Identifier(sname) = obj.as_ref() {
                         // 2026-08-10: clone inner/dims so the struct_types
                         // borrow is released before gep_index (which takes
                         // &mut self) — the old code held s_ty across the call.
                         let slot_ty: Option<(crate::ast::Type, Vec<crate::ast::Dimension>)> =
                             self.ctx.struct_types.get(&self_type)
                                 .and_then(|f| f.iter().find(|(n, _)| n == sname))
                                 .and_then(|(_, ty)| match ty {
                                     Type::Vector(inner, dims) => Some(((**inner).clone(), dims.clone())),
                                     _ => None,
                                 });
                         if let Some((inner, dims)) = slot_ty {
                             if dims.len() >= 1 {
                                     let offset = self.lookup_field_offset(&self_type, sname);
                                     let elem_size = crate::backend::llvm::types::type_size(&inner, self.ctx.type_universe.as_ref());
                                     // 2026-08-07: multi-dim — the row stride
                                     // is the product of the REMAINING dims
                                     // (`data[row]` jumps a whole row of the
                                     // inner sub-array).
                                     let row_elems: usize = dims.iter().skip(1)
                                         .map(|d| match d {
                                             crate::ast::Dimension::Anonymous(n) => *n,
                                             crate::ast::Dimension::Named(n, c) if *c > 0 => *c,
                                             _ => 1,
                                         })
                                         .product::<usize>().max(1);
                                      let scaled = self.fun.gen_reg();
                                      let gep_idx = self.gep_index(out, indent, &idx_clone);
                                      writeln!(out, "{}{} = mul i64 {}, {}", indent, scaled, gep_idx, elem_size * row_elems as u64).ok();
                                     let total = self.fun.gen_reg();
                                     writeln!(out, "{}{} = add i64 {}, {}", indent, total, offset, scaled).ok();
                                     let gep = self.fun.gen_reg();
                                     writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, gep, self_ptr, total).ok();
                                     if dims.len() == 1 {
                                         let elem_llvm = self.llvm_type(&inner);
                                         let val = self.fun.gen_reg();
                                         writeln!(out, "{}{} = load {}, ptr {}", indent, val, elem_llvm, gep).ok();
                                         let _ = v;
                                         return TypedRegister { name: val, ty: inner.clone() };
                                     }
                                      // A row view — the enclosing index GEPs
                                      // into it (the row-view path below).
                                      let _ = v;
                                       return TypedRegister {
                                           name: gep,
                                           ty: Type::Vector(Box::new(inner.clone()), dims[1..].to_vec()),
                                       };
                                   }
                               }
                           }
                      }
                 // 2026-07-31 (A4): Array state-field indexing — GEP into
                 // %State + scalar load. A runtime index cannot extractvalue
                 // from a loaded aggregate; the whole-array load + extract
                 // path below is only valid for scalar fields.
                 if let Expr::Identifier(name) = obj.as_ref() {
                     if let Some(&fidx) = self.ctx.field_index_map.get(name) {
                         if let Type::Vector(inner, dims) = &obj_reg.ty {
                             let base = self.emit_state_gep(out, indent, "f", "%state", fidx);
                             // 2026-07-31: The GEP source type must be the
                             // %State field's real aggregate type
                             // (field_types[fidx] = "[16 x float]"), not
                             // llvm_type(Vector), which resolves to the
                             // scalar fallback.
                             let agg_ty = self
                                 .ctx
                                 .field_types
                                 .get(fidx)
                                 .cloned()
                                 .unwrap_or_else(|| "i64".into());
                              if dims.len() == 1 {
                                  // 1-dim field → the element.
                                  let elem = self.fun.gen_reg();
                                  let gep_idx = self.gep_index(out, indent, &idx_clone);
                                  writeln!(
                                      out,
                                      "{}{} = getelementptr {}, ptr {}, i64 0, i64 {}",
                                      indent, elem, agg_ty, base, gep_idx
                                  )
                                  .ok();
                                 let val = self.fun.gen_reg();
                                 writeln!(
                                     out,
                                     "{}{} = load {}, ptr {}",
                                     indent, val, self.llvm_type(inner), elem
                                 )
                                 .ok();
                                 let _ = v;
                                 return TypedRegister { name: val, ty: (**inner).clone() };
                             }
                             // 2026-08-07 (Phase 7): multi-dim field
                             // (`[M x [N x T]]`) — the index selects a ROW: a
                             // ptr into the aggregate typed with the remaining
                             // dims. The enclosing index (or a whole-row use)
                             // GEPs into it.
                              let row = self.fun.gen_reg();
                              let gep_idx = self.gep_index(out, indent, &idx_clone);
                              writeln!(
                                  out,
                                  "{}{} = getelementptr {}, ptr {}, i64 0, i64 {}",
                                  indent, row, agg_ty, base, gep_idx
                              )
                              .ok();
                              let _ = v;
                             return TypedRegister {
                                 name: row,
                                 ty: Type::Vector(Box::new((**inner).clone()), dims[1..].to_vec()),
                             };
                         }
                     }
                 }
                // 2026-08-07 (Phase 7): a ROW VIEW — the register is a ptr into
                // a multi-dim aggregate (produced by indexing a multi-dim
                // field, typed Vector with the REMAINING dims). GEP into it;
                // the final dim yields the element, further dims yield a
                // sub-row. The field-identifier paths above already handled
                // whole fields; this handles a Vector-typed ROW register.
                if let Type::Vector(inner, dims) = &obj_reg.ty {
                    if !dims.is_empty() {
                        let agg_ty = self.vector_array_llvm_type(&obj_reg.ty)
                            .unwrap_or_else(|| "i64".to_string());
                        let elem = self.fun.gen_reg();
                        let gep_idx = self.gep_index(out, indent, &idx_clone);
                        writeln!(
                            out,
                            "{}{} = getelementptr {}, ptr {}, i64 0, i64 {}",
                            indent, elem, agg_ty, obj_reg.name, gep_idx
                        )
                        .ok();
                        if dims.len() == 1 {
                            let val = self.fun.gen_reg();
                            writeln!(
                                out,
                                "{}{} = load {}, ptr {}",
                                indent, val, self.llvm_type(inner), elem
                            )
                            .ok();
                            let _ = v;
                            return TypedRegister { name: val, ty: (**inner).clone() };
                        }
                        let _ = v;
                        return TypedRegister {
                            name: elem,
                            ty: Type::Vector(Box::new((**inner).clone()), dims[1..].to_vec()),
                        };
                    }
                }
                // 2026-08-13: a bare `[99]`/`(1,2)` seq literal now carries the
                // boxed i64 handle type (Type::int) — recognize it by its AST
                // form so the 2-slot heap path still applies (it is a heap seq
                // with a length header, exactly like an Applied List value).
                let is_seq_literal = matches!(obj.as_ref(), Expr::List(_) | Expr::Tuple(_));
                if matches!(obj_reg.ty, Type::Ptr(_))
                    || matches!(&obj_reg.ty, Type::Applied(n, _) if n == "List")
                    || is_seq_literal
                {
                    let ptr = self.fun.gen_reg();
                    // 2026-08-11 (wasm32 obj-member fix): the Ptr/List receiver
                    // handle is i{int_bits} (i32 on wasm32) — inttoptr at that
                    // width, not hardcoded i64. 2026-08-12 (slice 4): a
                    // COLLECTION data pointer (`inner.data`, a struct Ptr field)
                    // loads/stores at i64 (the boxed-handle ABI), so inttoptr
                    // at i64 when the receiver value is i64; a local Ptr handle
                    // is i{int_bits}. Match the receiver register's LLVM type.
                    let recv_llvm = self.llvm_type(&obj_reg.ty);
                    let hw = if matches!(recv_llvm.as_str(), "i64" | "ptr") {
                        "i64".to_string()
                    } else {
                        format!("i{}", self.ctx.int_bits)
                    };
                    writeln!(
                        out,
                        "{}{} = inttoptr {} {} to ptr",
                        indent, ptr, hw, obj_reg.name
                    )
                    .ok();
                    let offset = self.fun.gen_reg();
                    // 2026-07-17: List/tuple types have a length header at
                    // slot 0 — elements start at index 1. Raw Ptr buffers
                    // (from Malloc#) have no header.
                    // 2026-07-18: Check by TYPE, not expression form — non-literal
                    // list identifiers also need the +1 offset.
                    // 2026-07-21: Only List types have a length header at slot 0.
                    // Raw Ptr<T> from Malloc# has no header — offset is the index.
                    let is_list_type = matches!(&obj_reg.ty, Type::Applied(n, _) if n == "List")
                        || is_seq_literal;
                    let gep_idx = self.gep_index(out, indent, &idx_clone);
                    if is_list_type {
                        writeln!(out, "{}{} = add i64 {}, 1", indent, offset, gep_idx).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 {}, 0", indent, offset, gep_idx).ok();
                    }
                    let gep = self.fun.gen_reg();
                    writeln!(
                        out,
                        "{}{} = getelementptr i64, ptr {}, i64 {}",
                        indent, gep, ptr, offset
                    )
                    .ok();
                    // 2026-08-01 (E): `vol let` — Ptr-Index reads emit
                    // `load volatile` (MMIO register arrays).
                    // 2026-08-04 (compiler-in-Briev): a String element is
                    // boxed in the i64 list slot — inttoptr it back to a real
                    // ptr so consumers (briev_str_eq) see a pointer, matching
                    // how string literals are represented.
                    if self.is_string_operand(&index_elem_ty) {
                        let raw = self.fun.gen_reg();
                        writeln!(out, "{}{} = load {}{}, ptr {}", indent, raw,
                            if self.fun.volatile_read { "volatile " } else { "" }, "i64", gep).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, v, raw).ok();
                    } else if self.is_protocol_member(&index_elem_ty, "#Float") {
                        // 2026-08-07 (Phase 7): a Float (f32) element is
                        // stored in the i64 list slot as
                        // `zext(bitcast float to i32)` — invert it.
                        let raw = self.fun.gen_reg();
                        writeln!(out, "{}{} = load {}{}, ptr {}", indent, raw,
                            if self.fun.volatile_read { "volatile " } else { "" }, "i64", gep).ok();
                        let tr = self.fun.gen_reg();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, raw).ok();
                        writeln!(out, "{}{} = bitcast i32 {} to float", indent, v, tr).ok();
                    } else if self.is_protocol_member(&index_elem_ty, "#Int") {
                        // 2026-08-12 (slice 4, wasm32 maze): an Int element is
                        // stored as a boxed i64 handle; on wasm32 the value is
                        // in the low i32 — load at the ELEMENT's native width
                        // (`llvm_type(Int)` = i32 on wasm32, i64 on x86_64) so
                        // the member's term return + consumers match. Loading
                        // the full i64 then returning it as i32 produced
                        // `%t117 defined with type 'i64' but expected 'i32'`.
                        writeln!(out, "{}{} = load {}{}, ptr {}", indent, v,
                            if self.fun.volatile_read { "volatile " } else { "" },
                            self.llvm_type(&index_elem_ty), gep).ok();
                    } else {
                        writeln!(out, "{}{} = load {}{}, ptr {}", indent, v,
                            if self.fun.volatile_read { "volatile " } else { "" }, "i64", gep).ok();
                    }
                } else {
                    writeln!(
                        out,
                        "{}{} = extractelement {} {}, {}",
                        indent,
                        v,
                        self.llvm_type(&obj_reg.ty),
                        obj_reg.name,
                        idx_reg.name
                    )
                    .ok();
                }
                // 2026-08-04 (compiler-in-Briev): the element type was computed
                // early (index_elem_ty) so the load path could inttoptr string
                // elements.
                let elem_ty = index_elem_ty.clone();
                TypedRegister {
                    name: v.to_string(),
                    ty: elem_ty,
                }
            }

            // ── Cast ─────────────────────────────────────────────────
            // 2026-07-30: Casting graph resolves protocol→protocol paths.
            // Falls through to LLVM coercion when no graph path exists.
            Expr::Cast(expr, target) => {
                let src = self.emit_expr(out, expr, indent);
                // Try casting graph path first
                if let Some(result) = self.emit_cast_path(out, v, &src, target, indent) {
                    return result;
                }
                let target_ll = self.llvm_type(target);
                let src_ll = self.llvm_type(&src.ty);
                // 2026-07-17: Priorities for cast dispatch (LLVM coercion fallback):
                // 1. Ptr<T> target → inttoptr (never String — String/Data have Custom type)
                // 2. String/Data target → runtime helper
                // 3. i64 target + Ptr<T> source → ptrtoint
                // 4. i64 target + string-producing expr → __str_to_int
                // 5. double/i64 float conversions
                // 6. Generic bitcast
                if matches!(target, Type::Ptr(_)) {
                    // 2026-07-26: Only inttoptr when the source is an integer.
                    // If source is already a pointer, ptr→i64→ptr to assign v.
                    if src_ll != "ptr" {
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, v, src.name).ok();
                    } else {
                        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, src.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, v, v).ok();
                    }
                } else if target_ll == "i64" && matches!(src.ty, Type::Ptr(_)) {
                    // 2026-07-30: Ptr values stored as i64 internally (ptrtoint
                    // at function entry). Register is already i64 — identity.
                    return TypedRegister { name: src.name.clone(), ty: target.clone() };
                } else if target_ll == "double" {
                    writeln!(out, "{}{} = sitofp i64 {} to double", indent, v, src.name).ok();
                } else if target_ll == "i64" && src_ll == "double" {
                    writeln!(out, "{}{} = fptosi double {} to i64", indent, v, src.name).ok();
                } else if target_ll == "i64" && src_ll == "ptr" {
                    // 2026-07-18: Custom types (String, etc.) are stored as i64 in
                    // state fields — the value is already ptrtoint. Skip conversion.
                    if matches!(src.ty, Type::Custom(_)) {
                        return TypedRegister {
                            name: src.name.clone(),
                            ty: target.clone(),
                        };
                    }
                    // 2026-07-30: Ptr values stored as i64 internally — identity.
                    if matches!(src.ty, Type::Ptr(_)) {
                        return TypedRegister { name: src.name.clone(), ty: target.clone() };
                    }
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, src.name).ok();
                } else if target_ll == "ptr" && src_ll == "i64" {
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, v, src.name).ok();
                } else if let Some(n) = crate::type_universe::bits_width(target) {
                    // 2026-08-13 (pack): no-graph fallback for `as Bits<N>` —
                    // same truncation contract as the casting-graph path.
                    let tgt = format!("i{}", n);
                    if src_ll == tgt {
                        return TypedRegister { name: src.name.clone(), ty: target.clone() };
                    }
                    writeln!(out, "{}{} = trunc {} {} to {}", indent, v, src_ll, src.name, tgt).ok();
                } else {
                    writeln!(
                        out,
                        "{}{} = bitcast {} {} to {}",
                        indent, v, src_ll, src.name, target_ll
                    )
                    .ok();
                }
                TypedRegister {
                    name: v.to_string(),
                    ty: target.clone(),
                }
            }

            // ── IsType ───────────────────────────────────────────────
            Expr::IsType(_, _) => {
                writeln!(out, "{}{} = add i8 0, 1", indent, v).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }

            // ── Within ───────────────────────────────────────────────
            Expr::Within(expr, _) => self.emit_expr(out, expr, indent),

            // ── Match ────────────────────────────────────────────────
            Expr::Match(scrutinee, arms) => {
                if arms.is_empty() {
                    return TypedRegister { name: v.to_string(), ty: Type::void() };
                }
                self.emit_match(out, v, scrutinee, arms, indent)
            }

            // ── Lambda ───────────────────────────────────────────────
            // 2026-08-14 (stdlib-cleanup): a lambda in expression position
            // (a call ARGUMENT — `iter_map(list, x -> x * 2)`) is a real
            // first-class closure value, NOT an inlined body evaluation. It
            // lowers to a heap env block `[fn_ptr, cap1..capN]` whose address
            // is the value, with the closure body emitted as a
            // `briev_closure_N` function — the same representation a `let`
            // binds (emit_stmt Statement::Let lambda arm). The address flows as
            // an i64 handle like other boxed values; the defn-call path
            // inttoptrs it when the callee's param is `Type::Function` (ptr).
            Expr::Lambda(params, body) => {
                let free_vars = crate::backend::llvm::context::collect_free_vars(body, params);
                let symbol = format!("briev_closure_{}", self.ctx.pending_closures.len());
                self.ctx.pending_closures.push(
                    crate::backend::llvm::context::PendingClosure {
                        symbol: symbol.clone(),
                        params: params.clone(),
                        body: (**body).clone(),
                        free_vars: free_vars.clone(),
                    },
                );
                let env_size = 8 * (1 + free_vars.len());
                let alloc = self.fun.gen_reg();
                writeln!(out, "{}{}_p = call ptr @malloc(i64 {})", indent, alloc, env_size).ok();
                writeln!(out, "{}{} = ptrtoint ptr {}_p to i64", indent, alloc, alloc).ok();
                let env_p = self.fun.gen_reg();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, env_p, alloc).ok();
                let fn_reg = self.fun.gen_reg();
                writeln!(out, "{}{} = ptrtoint ptr @{} to i64", indent, fn_reg, symbol).ok();
                writeln!(out, "{}store i64 {}, ptr {}", indent, fn_reg, env_p).ok();
                for (j, var) in free_vars.iter().enumerate() {
                    let cap = self.emit_expr(out, &Expr::Identifier(var.clone()), indent);
                    let slot = self.fun.gen_reg();
                    writeln!(
                        out,
                        "{}{} = getelementptr i64, ptr {}, i64 {}",
                        indent, slot, env_p, 1 + j
                    )
                    .ok();
                    writeln!(out, "{}store i64 {}, ptr {}", indent, cap.name, slot).ok();
                }
                TypedRegister { name: alloc, ty: Type::int() }
            }

            // ── Address-of ────────────────────────────────────────────
            Expr::AddrOf(inner) => {
                // &expr provides the address of a state field or value.
                // For state fields, emit GEP into %State and ptrtoint to i64.
                match inner.as_ref() {
                    Expr::Identifier(name) => {
                        if let Some(&idx) = self.ctx.field_index_map.get(name) {
                            // 2026-07-31 (A7): a struct-typed state field's
                            // slot holds the INSTANCE address — `&b` is that
                            // address, not the %State slot pointer.
                            let is_struct_field = self.ctx.field_briev_types.get(idx)
                                .map_or(false, |t| matches!(t, Type::Custom(n)
                                    if self.ctx.struct_types.contains_key(n)));
                            if is_struct_field {
                                let (loaded, _) = self.emit_state_load_i64_by_idx(out, indent, idx);
                                return TypedRegister { name: loaded, ty: Type::int() };
                            }
                            let gep = self.emit_state_gep(out, indent, "aof", "%state", idx);
                            let ptr = self.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, ptr, gep).ok();
                            TypedRegister {
                                name: ptr,
                                ty: Type::int(),
                            }
                        } else if let Some(alloca) = self.fun.struct_literal_allocas.get(name).cloned() {
                            // 2026-07-24: &struct_literal emits the alloca pointer.
                            // Used by PyModule_Create2(&moduledef, ...) in bridge code.
                            let ptr = self.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, ptr, alloca).ok();
                            TypedRegister {
                                name: ptr,
                                ty: Type::int(),
                            }
                        } else if let Some(reg) = self.get_local(name) {
                            // 2026-07-24: &let_var — take address of let-bound variable.
                            let slot = if self.fun.let_binding_allocas.contains(&reg)
                                || self.fun.param_slots.values().any(|s| s == &reg)
                            {
                                reg
                            } else {
                                // SSA register — spill to alloca first.
                                let slot = self.fun.gen_reg();
                                writeln!(out, "{}{} = alloca i64, align 8", indent, slot).ok();
                                writeln!(out, "{}store i64 {}, ptr {}", indent, reg, slot).ok();
                                self.fun.let_bindings.insert(name.clone(), slot.clone());
                                self.fun.let_binding_allocas.insert(slot.clone());
                                slot
                            };
                            let ptr = self.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, ptr, slot).ok();
                            TypedRegister {
                                name: ptr,
                                ty: Type::int(),
                            }
                        } else {
                            // 2026-07-24: &function_name emits function pointer.
                            // Used by struct literals for method table entries.
                            let ptr = self.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr @{} to i64", indent, ptr, name).ok();
                            TypedRegister {
                                name: ptr,
                                ty: Type::int(),
                            }
                        }
                    }
                    _ => {
                        let inner_reg = self.emit_expr(out, inner, indent);
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, inner_reg.name).ok();
                        TypedRegister {
                            name: v.to_string(),
                            ty: Type::int(),
                        }
                    }
                }
            }

            // ── Dereference ───────────────────────────────────────────
            Expr::Deref(inner) => {
                let ptr_reg = self.emit_expr(out, inner, indent);
                // 2026-07-30: Ptr values are stored as i64 internally (see
                // ptrtoint at emit_toplevel.rs:1188). Convert back to actual
                // LLVM ptr before loading.
                let load_ptr = if matches!(ptr_reg.ty, Type::Ptr(_)) {
                    let p = self.fun.gen_reg();
                    self.emit_inttoptr(out, indent, &p, &ptr_reg.name);
                    p.to_string()
                } else {
                    ptr_reg.name.clone()
                };
                let pointee_ty = match &ptr_reg.ty {
                    Type::Ptr(inner_ty) => inner_ty.as_ref().clone(),
                    _ => Type::int(), // fallback
                };
                let llvm_ty = self.llvm_type(&pointee_ty);
                writeln!(
                    out,
                    "{}{} = load {}{}, ptr {}, align 8",
                    indent,
                    v,
                    if self.fun.volatile_read { "volatile " } else { "" },
                    llvm_ty,
                    load_ptr
                )
                .ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: pointee_ty,
                }
            }

            // ── Field access / reflection / method call (2026-07-31) ─
            Expr::Field(recv, name) => {
                return self.emit_field_access(out, v, recv, name, indent);
            }
            Expr::Reflect(recv, target, kind) => {
                return self.emit_reflection(out, v, recv, target, *kind, indent);
            }
            Expr::MethodCall(recv, name, args, _) => {
                // 2026-07-31 (A5): self-bound member emission. Emit the
                // receiver's struct address, bind `self`, bind the params,
                // emit the member body inline, restore the previous binding.
                return self.emit_method_call(out, v, recv, name, args, indent);
            }
            Expr::DerivationBlock(_) | Expr::FormattingAnnotation(_) => {
                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::void(),
                }
            }
            // 2026-07-19: Plugin-intercept calls must be resolved by Front plugins
            // before codegen. If one reaches here, the plugin system failed.
            Expr::PluginIntercept { .. } => {
                panic!("unresolved plugin-intercept call reached codegen");
            }
            Expr::StructLiteral { type_name, fields } => {
                return self.emit_struct_literal(out, v, type_name, fields, indent);
            }
            Expr::Exists(name) => { panic!("compile-time existence check '{}' reached LLVM codegen", name) },
            Expr::Slice { array, start, end, stride } => {
                // 2026-07-26: Evaluate slice bounds for side effects.
                // The narrowing pass converts constant-bounds slices to Vector<T,N>
                // before codegen; this arm handles dynamic slices.
                let array_reg = self.emit_expr(out, array, indent);
                // 2026-08-13 (dynamic String slice): `s[a:b]` on a String was a
                // STUB that returned the whole array — every string.bv function
                // using a dynamic slice (replace, trim, substr, split, ...)
                // silently produced the full string (documented in BUGS.md).
                // The runtime half (briev_str_substr) has always existed; wire
                // it here. Vector slices stay on the narrowing-pass path.
                let arr_is_str = self.is_string_operand(&array_reg.ty)
                    || match array.as_ref() {
                        Expr::Identifier(nm) => self.is_string_operand(
                            &self.fun.let_original_types.get(nm).cloned().unwrap_or(Type::int())
                        ),
                        _ => false,
                    };
                if arr_is_str {
                    let sp = self.string_ptr(out, indent, &array_reg);
                    let lo = match start {
                        Some(s) => self.emit_expr(out, s, indent),
                        None => {
                            let z = self.fun.gen_reg();
                            writeln!(out, "{}{} = add i64 0, 0", indent, z).ok();
                            TypedRegister { name: z, ty: Type::int() }
                        }
                    };
                    let hi = match end {
                        Some(e) => self.emit_expr(out, e, indent),
                        None => {
                            let n = self.fun.gen_reg();
                            writeln!(out, "{}{} = add i64 0, 0", indent, n).ok();
                            TypedRegister { name: n, ty: Type::int() }
                        }
                    };
                    let _ = stride;
                    let sub = self.fun.gen_reg();
                    writeln!(out, "{}{} = call ptr @briev_str_substr(ptr {}, i64 {}, i64 {})",
                        indent, sub, sp, lo.name, hi.name).ok();
                    return TypedRegister { name: sub, ty: crate::ast::Type::Custom("String".to_string()) };
                }
                if let Some(s) = start { self.emit_expr(out, s, indent); }
                if let Some(e) = end { self.emit_expr(out, e, indent); }
                if let Some(s) = stride { self.emit_expr(out, s, indent); }
                array_reg
            }
            // 2026-08-07 (Phase 7): a range is an ITERABLE, consumed only by
            // `foreach` (SPEC §11.4) — the foreach arm lowers the counted
            // loop directly from the Expr::Range node, so this arm is only
            // reached for a range used as a scalar value (a hard error).
            Expr::Range { .. } => panic!(
                "a range expression is only valid as a `foreach` iterable, not as a value"
            ),
            // 2026-08-07 (object instance pools): `spawn Obj(args)` — allocate
            // the next pool row from the __spawn_next_<base> counter, run the
            // Init member at that row, increment the counter, and return the
            // row as the linear handle.
            Expr::Spawn { type_name, args, storage } => {
                // 2026-08-09 (Phase 10): `spawn defn(args)` is a TASK spawn —
                // the handle is the defn's result (SPEC §12.2). The reference
                // semantic scheduler is deterministic: a spawned task runs to
                // completion inline, and `await` reads the stored result. A
                // defn name is distinguishable from an obj base by the
                // defn_params/defn_return_types registration.
                if self.ctx.defn_params.contains_key(type_name.as_str()) {
                    return self.emit_task_spawn(out, indent, type_name, args);
                }
                if *storage != crate::ast::SpawnStorage::Pooled {
                    // 2026-08-09 (Phase 5): `box`/`spill` spawns are NOT pooled
                    // rows — box is a per-instance heap allocation, spill a
                    // growable buffer. The spawn pool analysis rejects them
                    // from the static/dependent column path; the emission for
                    // each storage class is handled by the storage emitter.
                    return self.emit_spawn_storage(out, indent, type_name, args);
                }
                let counter_name = format!("__spawn_next_{}", type_name);
                let Some(&counter_idx) = self.ctx.field_index_map.get(&counter_name) else {
                    panic!("spawn of '{}' with no registered instance pool", type_name);
                };
                let counter = self.emit_state_gep(out, indent, "sp", "%state", counter_idx);
                let cur = self.fun.gen_reg();
                writeln!(out, "{}{} = load i64, ptr {}", indent, cur, counter).ok();
                self.emit_spawn_init(out, indent, type_name.as_str(), args.as_slice(), &cur);
                let next = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 {}, 1", indent, next, cur).ok();
                writeln!(out, "{}store i64 {}, ptr {}", indent, next, counter).ok();
                TypedRegister {
                    name: cur,
                    ty: Type::Custom(type_name.clone()),
                }
            }
        }
    }

    // ── Sub-helpers ──────────────────────────────────────────────────

    /// 2026-08-07 (Phase 7): lower `array[mask]` — a masked select (SPEC
    /// §16.5). The mask is either a compile-time Boolean list (interned as a
    /// `@bmask` constant) or a Bool[N] state field (an i64-slot array in
    /// %State). Two object kinds:
    ///   - a byte buffer (Data/String/Bits) → a new [len][bytes] Data buffer
    ///     via `briev_mask_select` (ptr-typed, like a byte literal);
    ///   - an Int/Bool vector state field (`[N x i64]`) → a new heap List of
    ///     the selected elements via `briev_mask_select64`.
    /// Mask lengths longer than the data truncate (the mask governs), matching
    /// the interpreter. The typechecker has already rejected unsupported
    /// containers, so the object here is a byte buffer or an i64-slot vector.
    fn emit_masked_index(
        &mut self,
        out: &mut String,
        op: MaskIndexOperands<'_>,
        indent: &str,
    ) -> TypedRegister {
        // The mask pointer + mask length (shared by both gathers).
        let (mask_ptr, mask_len) = match op.source {
            MaskSource::Constant(bits) => {
                let bytes: Vec<u8> = bits.iter().map(|b| if *b { 1 } else { 0 }).collect();
                let mi = self
                    .ctx
                    .mask_constants
                    .iter()
                    .position(|m| m == &bytes)
                    .unwrap_or_else(|| {
                        self.ctx.mask_constants.push(bytes.clone());
                        self.ctx.mask_constants.len() - 1
                    });
                let cast = self.fun.gen_reg();
                writeln!(out, "{}{} = bitcast [{} x i64]* @bmask.{} to ptr",
                    indent, cast, bytes.len(), mi).ok();
                (cast, bytes.len() as i64)
            }
            MaskSource::StateField(fidx) => {
                let gep = self.emit_state_gep(out, indent, "m", "%state", fidx);
                let n = self.ctx.field_briev_types.get(fidx)
                    .map(|t| self.vector_element_count(t))
                    .unwrap_or(0);
                (gep, n as i64)
            }
        };
        // Byte-buffer object → the byte gather; result is a ptr-typed Data.
        if self.is_string_operand(&op.obj_reg.ty) || self.is_blob_operand(&op.obj_reg.ty) {
            let data_ptr = op.obj_reg.name.clone();
            let r = self.fun.gen_reg();
            writeln!(
                out,
                "{}{} = call ptr @briev_mask_select(ptr {}, ptr {}, i64 {})",
                indent, r, data_ptr, mask_ptr, mask_len
            )
            .ok();
            return TypedRegister {
                name: r,
                ty: Type::Custom("Blob".into()),
            };
        }
        if matches!(&op.obj_reg.ty, Type::Bits(_)) {
            // Bits is a raw byte sequence without a [len] header — the byte
            // gather needs the header; inttoptr the i64 handle's bits (the
            // slot holds the buffer pointer for a Bytes-typed field).
            let p = self.fun.gen_reg();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, op.obj_reg.name).ok();
            let r = self.fun.gen_reg();
            writeln!(
                out,
                "{}{} = call ptr @briev_mask_select(ptr {}, ptr {}, i64 {})",
                indent, r, p, mask_ptr, mask_len
            )
            .ok();
            return TypedRegister {
                name: r,
                ty: Type::Custom("Blob".into()),
            };
        }
        // Heap List value (`List<Int>` — a `[len, e0, e1, …]` i64 buffer
        // boxed to an i64 handle) → the typed gather over its elements.
        if matches!(&op.obj_reg.ty, Type::Applied(n, _) if n == "List") {
            let list_p = self.fun.gen_reg();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, list_p, op.obj_reg.name).ok();
            let len = self.fun.gen_reg();
            writeln!(out, "{}{} = load i64, ptr {}", indent, len, list_p).ok();
            let elems = self.fun.gen_reg();
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, elems, list_p).ok();
            let buf = self.fun.gen_reg();
            writeln!(
                out,
                "{}{} = call ptr @briev_mask_select64(ptr {}, i64 {}, ptr {}, i64 {})",
                indent, buf, elems, len, mask_ptr, mask_len
            )
            .ok();
            let handle = self.fun.gen_reg();
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, handle, buf).ok();
            return TypedRegister {
                name: handle,
                ty: op.obj_reg.ty.clone(),
            };
        }
        // Int/Bool vector state field → the typed gather; result is a heap
        // List handle (ptrtoint'd, like emit_heap_seq).
        let Expr::Identifier(name) = op.obj else {
            unreachable!("vector mask routing only admits state-field objects");
        };
        let fidx = *self.ctx.field_index_map.get(name).unwrap();
        let data_ptr = self.emit_state_gep(out, indent, "f", "%state", fidx);
        let n = self.vector_element_count(&op.obj_reg.ty) as i64;
        // 2026-08-07 (Phase 7): a Float (f32) vector field (`[N x float]`)
        // uses the f32 gather — the selected floats land in the List as i64
        // bit patterns, matching how heap List<Float> slots store floats.
        // Float64 (double) vectors are a hard error (no f64 gather yet).
        let is_f32 = matches!(&op.obj_reg.ty, Type::Vector(inner, _)
            if self.is_protocol_member(inner, "#Float")
                && self.ctx.type_universe.as_ref()
                    .and_then(|u| inner.universe_key().and_then(|k| u.get(k)))
                    .map(|rt| rt.max_bits <= 32)
                    .unwrap_or(true));
        if is_f32 {
            let buf = self.fun.gen_reg();
            writeln!(
                out,
                "{}{} = call ptr @briev_mask_select_f32(ptr {}, i64 {}, ptr {}, i64 {})",
                indent, buf, data_ptr, n, mask_ptr, mask_len
            )
            .ok();
            let handle = self.fun.gen_reg();
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, handle, buf).ok();
            let elem_ty = match &op.obj_reg.ty {
                Type::Vector(inner, _) => (**inner).clone(),
                _ => Type::int(),
            };
            return TypedRegister {
                name: handle,
                ty: Type::Applied("List".into(), vec![elem_ty]),
            };
        }
        // A Float64 (double) vector is NOT an i64-slot array — routing it to
        // briev_mask_select64 would read `[N x double]` as i64s (garbage).
        // No f64 gather exists yet: hard error, no silent wrongness.
        if matches!(&op.obj_reg.ty, Type::Vector(inner, _)
            if self.is_protocol_member(inner, "#Float"))
        {
            panic!("mask indexing on Float64 (double) vectors is not yet supported");
        }
        let helper = "@briev_mask_select64";
        let buf = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = call ptr {}(ptr {}, i64 {}, ptr {}, i64 {})",
            indent, buf, helper, data_ptr, n, mask_ptr, mask_len
        )
        .ok();
        let handle = self.fun.gen_reg();
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, handle, buf).ok();
        let elem_ty = match &op.obj_reg.ty {
            Type::Vector(inner, _) => (**inner).clone(),
            _ => Type::int(),
        };
        TypedRegister {
            name: handle,
            ty: Type::Applied("List".into(), vec![elem_ty]),
        }
    }

    /// 2026-08-07 (Phase 7): the bits of a compile-time Boolean mask literal
    /// (`[true, false, …]`), or None if `expr` is not one.
    fn constant_bool_mask(expr: &Expr) -> Option<Vec<bool>> {
        let Expr::List(elems) = expr else {
            return None;
        };
        let mut bits = Vec::with_capacity(elems.len());
        for e in elems {
            match e {
                Expr::Bool(b) => bits.push(*b),
                _ => return None,
            }
        }
        Some(bits)
    }

    /// 2026-07-14: Emit a heap-allocated sequence (list/tuple) with 2-slot header.
    /// Protocol: slot 0 = length (i64), slots 1..N = elements.
    /// Empty seq → a fresh 2-slot heap block (2026-08-15, coll plan §3.3 #4:
    /// @ll_empty_list DELETED — a shared sentinel aliases across users; a `[]`
    /// coll constructs via `op InitEmpty`, an empty tuple gets its own block).
    /// Non-empty → malloc((2+N)*8), bitcast, store N, store elements, ptrtoint.
    fn emit_heap_seq(
        &mut self,
        out: &mut String,
        v: &str,
        exprs: &[Expr],
        indent: &str,
    ) -> TypedRegister {
        let count = exprs.len();
        if count == 0 {
            let raw = self.fun.gen_reg();
            writeln!(out, "{}{} = call ptr @malloc(i64 16)", indent, raw).ok();
            let hdr = self.fun.gen_reg();
            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, hdr, raw).ok();
            writeln!(out, "{}store i64 0, ptr {}", indent, hdr).ok();
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, hdr).ok();
        } else {
            let total = (2 + count) * 8;
            let raw = self.fun.gen_reg();
            writeln!(out, "{}{} = call ptr @malloc(i64 {})", indent, raw, total).ok();
            let hdr = self.fun.gen_reg();
            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, hdr, raw).ok();
            writeln!(out, "{}store i64 {}, ptr {}", indent, count as i64, hdr).ok();
            for (i, elem) in exprs.iter().enumerate() {
                let e = self.emit_expr(out, elem, indent);
                let slot = self.fun.gen_reg();
                writeln!(
                    out,
                    "{}{} = getelementptr i64, ptr {}, i64 {}",
                    indent,
                    slot,
                    hdr,
                    i + 1
                )
                .ok();
                // 2026-08-04 (compiler-in-Briev): list slots are i64 — a
                // String element (a ptr to [len][bytes]) must be ptrtoint'd
                // before the store, or `store i64 <ptr>, ptr` is invalid IR.
                let e64 = self.adapt_to_i64(out, indent, &e);
                writeln!(out, "{}store i64 {}, ptr {}", indent, e64, slot).ok();
            }
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, hdr).ok();
        }
        // 2026-08-13 (obj value ABI): the seq value is a BOXED i64 handle
        // (ptrtoint'd above), so its register type is Int — NOT Ptr. A Ptr
        // type made the defn-call arg coercion (`param i64, reg ptr`) re-ptrtoint
        // the boxed handle, double-boxing the empty-list sentinel passed to a
        // List param.
        TypedRegister {
            name: v.to_string(),
            ty: Type::int(),
        }
    }

    /// 2026-07-25: Emit an integer constant. All intermediate values use i64 —
    /// narrowing is applied at the `ret` instruction.
    fn emit_int(&mut self, out: &mut String, v: &str, imm: i64, indent: &str) -> TypedRegister {
        // 2026-08-10: emit at the TARGET int width (i{int_bits}) — matching
        // llvm_type(Int) and binop_int_type(). The old hardcoded `add i64`
        // produced i64 literals that llvm_type(Int) (i32 on wasm32) and the
        // binary-op emitters treat as i32, so adapt_to_i64 emitted
        // `sext i32 <i64 reg>` — invalid IR. Char literals already used the
        // native width (emit_expr.rs Expr::Char); Int follows the same rule.
        let int_ty = format!("i{}", self.ctx.int_bits);
        writeln!(out, "{}{} = add {} 0, {}", indent, v, int_ty, imm).ok();
        TypedRegister { name: v.to_string(), ty: Type::int() }
    }

    /// 2026-08-10: Widen an array-index register to i64 for GEP. An index is an
    /// Int value (i{int_bits}, i32 on wasm32) but LLVM GEP indices are i64 —
    /// the old code passed the raw i{int_bits} register, producing
    /// `getelementptr [N x i32], ptr %x, i64 0, i64 <i32 reg>` (invalid). The
    /// extension is a no-op on x86_64 (index already i64). Takes &mut self to
    /// emit; uses ctx.int_bits for the width (indices are always Int scalars),
    /// so no llvm_type lookup that could conflict with a live borrow.
    pub(crate) fn gep_index(&mut self, out: &mut String, indent: &str, idx: &TypedRegister) -> String {
        let width = format!("i{}", self.ctx.int_bits);
        if width == "i64" {
            return idx.name.clone();
        }
        let r = self.fun.gen_reg();
        writeln!(out, "{}{} = sext {} {} to i64", indent, r, width, idx.name).ok();
        r
    }

    /// Emit a string literal as stack-allocated bytes + GEP.
    /// 2026-07-14: Use alloca instead of global constant to avoid placement
    /// issues (globals must be at module level, not inside functions).
    fn emit_string_literal(
        &mut self,
        out: &mut String,
        v: &str,
        bytes: &[u8],
        indent: &str,
    ) -> TypedRegister {
        // 2026-08-01 (B4): the SSO string-literal path was retired (the
        // feature flag is always off under the bits model — a String is a ptr
        // to [len][bytes]). All literals emit via the ptr representation.
        self.emit_legacy_string_literal(out, v, bytes, indent)
    }

    /// 2026-08-06 (Phase 7): emit a raw-bytes Data literal — a `[len][bytes]`
    /// constant with the EXACT bytes (`\xHH` escapes). Distinct from the
    /// string path, which lossily re-encodes bytes as UTF-8 text.
    fn emit_byte_literal(
        &mut self,
        out: &mut String,
        _v: &str,
        bytes: &[u8],
        indent: &str,
    ) -> TypedRegister {
        let si = self
            .ctx
            .byte_constants
            .iter()
            .position(|x| x == bytes)
            .unwrap_or_else(|| {
                self.ctx.byte_constants.push(bytes.to_vec());
                self.ctx.byte_constants.len() - 1
            });
        let g = format!("@bstr.{}", si);
        let str_p = self.fun.gen_reg();
        writeln!(out, "{}{} = bitcast <{{ i64, [{} x i8] }}>* {} to ptr",
            indent, str_p, bytes.len(), g).ok();
        TypedRegister { name: str_p, ty: Type::Custom("Blob".into()) }
    }

    // 2026-07-22: Legacy string literal emission (SSO OFF).
    // Uses global @str.N constants to avoid dangling stack pointers.
    // The old alloca-based approach caused use-after-free when string
    // return values were passed to subsequent function calls.
    fn emit_legacy_string_literal(
        &mut self,
        out: &mut String,
        v: &str,
        bytes: &[u8],
        indent: &str,
    ) -> TypedRegister {
        // 2026-07-22: Emit global constant if not already defined
        let s_str = String::from_utf8_lossy(bytes);
        let si = self.ctx.string_constants.iter()
            .position(|x| x.as_str() == s_str)
            .unwrap_or_else(|| {
                self.ctx.string_constants.push(s_str.to_string());
                self.ctx.string_constants.len() - 1
            });
        let g = format!("@str.{}", si);
        // 2026-07-22: The handle is a pointer to the start of the struct
        // {i64 length, [N x i8] chars}, so that the runtime's briev_str_to_c
        // reads *(int64_t*)handle as the length and data at handle+8 — the
        // [len][bytes] buffer layout. Do NOT add offset — emit_load_length
        // expects the struct pointer.
        // 2026-08-01 (B0): the value register IS the pointer (a Briev String
        // value is a ptr to [len][bytes] in every type-claiming site). The
        // old ptrtoint→i64 boxing here was one arm of the split-brain; the
        // ptr is passed straight to consumers (Print#, frgn calls, state
        // adapt_to_i64 which does the ptrtoint for the i64 slot).
        let str_p = self.fun.gen_reg();
        writeln!(out, "{}{} = bitcast <{{ i64, [{} x i8] }}>* {} to ptr",
            indent, str_p, bytes.len() + 1, g).ok();
        TypedRegister {
            name: str_p,
            ty: Type::string(),
        }
    }

    /// 2026-07-16: P5 — Emit a foreign function call with optional auto-meld.
    /// Derives convention extension from sig.from, checks meld compatibility,
    /// and applies identity conversion (same bit layout, meld-verified type tag).
    /// 2026-07-22: Extended to dispatch via pre-resolved ResolvedFrgn strategies.
    /// The dispatch decision (Inline vs Bridge vs Unsupported) is made during
    /// the main compilation pass, not inside the backend.
    /// Emit a struct literal: allocates stack space for the struct and
    /// stores each field at its offset.
    /// 2026-07-24: Reads StructDef from type universe for layout info.
    /// Falls back to struct_types (from registration pass) when universe unavailable.
    pub(crate) fn emit_struct_literal(
        &mut self,
        out: &mut String,
        v: &str,
        type_name: &str,
        fields: &[(String, Expr)],
        indent: &str,
    ) -> TypedRegister {
        let total_size = self.struct_type_size(type_name);
        let struct_ty = crate::ast::Type::Custom(type_name.to_string());

        // 2026-08-13 (struct value lifetime): a struct literal must live on the
        // HEAP, not the stack. The handle (`ptrtoint`) crosses function
        // boundaries (`term StringBuilder { ... }` from new_builder/append_*),
        // and a stack alloca's handle dangles once the constructing function
        // returns — the caller then reads a dead frame (append_str("") returned
        // a builder whose buffer read garbage). State-slot/field consumers read
        // the handle in the same function, so heap is a strict superset.
        let alloca_reg = self.fun.gen_reg();
        // 2026-08-13 (reactor fix, merged with main's heap lifetime): the
        // allocation (heap malloc per main, so the handle survives function
        // boundaries) is deferred to the loop PREHEADER when inside a reactor
        // loop body (flush_pending_struct_allocas) — an allocation in the loop
        // makes clang -O3 peel the loop and emit a bogus exit assumption (the
        // node fires once). Elsewhere it stays inline.
        if self.fun.defer_struct_allocas {
            self.fun.pending_struct_allocas.push(format!("{}  {} = call ptr @malloc(i64 {})", indent, alloca_reg, total_size));
        } else {
            writeln!(out, "{}  {} = call ptr @malloc(i64 {})", indent, alloca_reg, total_size).ok();
        }

        for (field_name, field_expr) in fields {
            let fr = self.fun.gen_reg();
            let val = self.emit_expr_inner(out, &fr, field_expr, indent);
            let offset = self.lookup_field_offset(type_name, field_name);
            let ptr_reg = self.fun.gen_reg();
            writeln!(out, "{}  {} = getelementptr i8, ptr {}, i64 {}", indent, ptr_reg, alloca_reg, offset).ok();
            // 2026-08-13 (pack): a packed struct-literal field stores its
            // bit-slice into the byte image (L-M-S for sub-byte fields).
            // 2026-08-13 (Phase 5): an `atomic` struct-literal field is
            // written with an atomic store (checked before the packed path).
            if self.is_atomic_field(type_name, field_name) {
                let alt = if matches!(val.ty, Type::Ptr(_)) {
                    format!("i{}", self.ctx.int_bits)
                } else {
                    self.llvm_type(&val.ty)
                };
                let asz = types::type_size(&val.ty, self.ctx.type_universe.as_ref()).max(1);
                let store_val = self.ensure_typed_value(
                    out, indent, &alt, &val.name, Some(val.ty.clone()),
                    self.ctx.type_universe.clone().as_ref(),
                );
                writeln!(out, "{}  store atomic {} {}, ptr {} seq_cst, align {}", indent, alt, store_val, ptr_reg, asz).ok();
                continue;
            }
            if let Some(pf) = self.packed_field(type_name, field_name) {
                out.push_str(&self.emit_packed_field_store(indent, &ptr_reg, &pf, &val));
                continue;
            }
            // Convert i64 to ptr if the field type expects a pointer
            if val.ty == Type::int() && self.is_ptr_field(type_name, field_name) {
                let conv = self.fun.gen_reg();
                writeln!(out, "{}  {} = inttoptr i64 {} to ptr", indent, conv, val.name).ok();
                writeln!(out, "{}  store ptr {}, ptr {}", indent, conv, ptr_reg).ok();
            } else {
                let fty = self.llvm_type(&val.ty);
                writeln!(out, "{}  store {} {}, ptr {}", indent, fty, val.name, ptr_reg).ok();
            }
        }

        let result = self.fun.gen_reg();
        // 2026-08-11 (wasm32 obj-member fix): obj/struct handles are
        // i{int_bits} (i32 on wasm32), not a hardcoded i64 — the state slot
        // they land in is the same width. x86_64 (int_bits=64) is unchanged.
        let hw = format!("i{}", self.ctx.int_bits);
        writeln!(out, "{}  {} = ptrtoint ptr {} to {}", indent, result, alloca_reg, hw).ok();
        // 2026-07-24: Record the alloca pointer so &let_var on a struct-typed
        // binding retrieves the stack address, not the ptrtoint value.
        self.fun.struct_literal_allocas.insert(result.clone(), alloca_reg.clone());
        TypedRegister { name: result, ty: struct_ty }
    }

    /// 2026-07-31: `p.name` — load a struct field. The receiver register
    /// holds the struct's address (struct literals emit ptrtoint; state slots
    /// store the same address form). GEP by field offset and load.
    fn emit_field_access(
        &mut self,
        out: &mut String,
        v: &str,
        recv: &Expr,
        name: &str,
        indent: &str,
    ) -> TypedRegister {
        // 2026-08-12 (Iterable protocol, slice 2 gap 2): a POOLED instance
        // receiver (`c.count` on a top-level `let c: Counter` whose members
        // unpack into `{base}.{member}` columns) must route to the column at
        // row 0 — emitting the receiver as a box handle produces an undefined
        // `@c` global. Mirrors the member-call receiver prefix path.
        if let Expr::Identifier(rname) = recv {
            if let Some((base, row_reg)) = self.instance_prefix_for(rname) {
                let slot = format!("{}.{}", base, name);
                if let Some(&idx) = self.ctx.field_index_map.get(&slot) {
                    let (row, row_ty, load_ty) =
                        self.emit_instance_column_row(out, indent, idx, &row_reg);
                    let loaded = self.fun.gen_reg();
                    writeln!(out, "{}{} = load {}, ptr {}", indent, loaded, load_ty, row).ok();
                    return TypedRegister { name: loaded, ty: row_ty };
                }
                // Boxed instance (per-heap block): inttoptr the handle + GEP
                // the member byte offset.
                if let Some(offsets) = self.ctx.boxed_offsets.get(base.as_str()) {
                    if let Some((off, mty)) = offsets.get(name) {
                        let ptr = self.fun.gen_reg();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, row_reg).ok();
                        let gep = self.fun.gen_reg();
                        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, gep, ptr, off).ok();
                        let llvm_ty = if matches!(mty, Type::Ptr(_)) {
                            "i64".to_string()
                        } else {
                            self.llvm_type(mty)
                        };
                        let loaded = self.fun.gen_reg();
                        writeln!(out, "{}{} = load {}, ptr {}", indent, loaded, llvm_ty, gep).ok();
                        return TypedRegister { name: loaded, ty: mty.clone() };
                    }
                }
            }
        }
        let _ = v;
        let recv_tmp = self.fun.gen_reg();
        let recv_reg = self.emit_expr_inner(out, &recv_tmp, recv, indent);
        let type_name = match self.resolve_obj_key(&recv_reg.ty) {
            Some(n) => n,
            None => panic!(
                "field access '.{}' on non-struct type '{}' reached codegen",
                name, recv_reg.ty
            ),
        };
        let Some(fields) = self.get_struct_fields(&type_name) else {
            panic!("field access '.{}': no struct layout for '{}'", name, type_name);
        };
        // 2026-08-13 (pack): packed structs take their byte offset from the
        // packed authority; otherwise the type_size walk stands.
        let packed = self.ctx.packed_structs.contains(&type_name);
        // 2026-08-13 (Phase 6): a union's fields all overlay at offset 0 — no
        // type_size accumulation.
        let is_union = self.ctx.unions.contains(&type_name);
        let mut offset = if packed || is_union {
            self.lookup_field_offset(&type_name, name)
        } else {
            0u64
        };
        let mut field_ty: Option<Type> = None;
        for (fname, fty) in fields {
            if fname == name {
                field_ty = Some(fty.clone());
                break;
            }
            if !packed && !is_union {
                offset += types::type_size(fty, self.ctx.type_universe.as_ref());
            }
        }
        let field_ty = field_ty.unwrap_or_else(|| {
            panic!("field access '.{}': no field '{}' on '{}'", name, name, type_name)
        });
        let ptr = self.fun.gen_reg();
        writeln!(out, "{}  {} = inttoptr i64 {} to ptr", indent, ptr, recv_reg.name).ok();
        let gep = self.fun.gen_reg();
        writeln!(out, "{}  {} = getelementptr i8, ptr {}, i64 {}", indent, gep, ptr, offset).ok();
        // 2026-08-13 (Phase 5): an `atomic` field reads with an atomic load
        // (SPEC §8.2). Tried before the packed path — a whole-byte packed
        // atomic field is an atomic load of the field's LLVM type.
        if self.is_atomic_field(&type_name, name) {
            let lt = if matches!(field_ty, Type::Ptr(_)) {
                format!("i{}", self.ctx.int_bits)
            } else {
                self.llvm_type(&field_ty)
            };
            let sz = types::type_size(&field_ty, self.ctx.type_universe.as_ref()).max(1);
            let val = self.fun.gen_reg();
            writeln!(out, "{}  {} = load atomic {}, ptr {} seq_cst, align {}", indent, val, lt, gep, sz).ok();
            return TypedRegister { name: val, ty: field_ty.clone() };
        }
        // 2026-08-13 (pack): a packed field reads its bit-slice out of the
        // byte image (whole-byte fields = the same plain aligned load). The
        // register is typed `Bits(bits)` — its true width — so `as Int`
        // truncates/extends correctly downstream.
        if let Some(pf) = self.packed_field(&type_name, name) {
            let val = self.emit_packed_field_load(out, indent, &gep, &pf);
            return TypedRegister { name: val, ty: Type::Bits(pf.bits) };
        }
        // 2026-08-01 (D3): a Ptr-typed struct field stores the i64 HANDLE
        // (ptrtoint at store) — load i64, not `ptr`, so the downstream
        // inttoptr consumers (inner.data[len]) work unchanged.
        let llvm_ty = if matches!(field_ty, Type::Ptr(_)) {
            "i64".to_string()
        } else {
            self.llvm_type(&field_ty)
        };
        let val = self.fun.gen_reg();
        writeln!(out, "{}  {} = load {}, ptr {}", indent, val, llvm_ty, gep).ok();
        TypedRegister { name: val, ty: field_ty }
    }

    /// 2026-07-31: `x.^Meta` (runtime) / `x.^^Meta` (compile-time).
    /// Compile-time targets emit constants (foldable); `Ptr` reuses the
    /// address-of path; `Len` emits a constant for fixed-size vectors and a
    /// clear error for dynamic receivers (Phase-1b boundary).
    /// 2026-07-31 (A5): `recv.name(args)` — inline the obj member body with
    /// `self` bound to the receiver instance's storage.
    /// 2026-08-07 (object instance pools): resolve an instance receiver's
    /// (base, pool row) — a static instance name (`b` → ("Box", "0")) or a
    /// spawned handle local (`h` → ("Counter", <row reg>)).
    pub(crate) fn instance_prefix_for(&self, name: &str) -> Option<(String, String)> {
        if let Some(p) = self.unpacked_instance_prefix(name) {
            return Some(p);
        }
        let reg = self.get_local(name)?;
        let base = self.fun.let_binding_types.get(name).and_then(|t| match t {
            Type::Custom(b) if self.ctx.obj_members.contains_key(b) => {
                // 2026-08-15 (coll plan): a GROWABLE `coll obj` (`Ptr<T>`
                // sequence) is a BOXED heap handle — never a pooled instance.
                // Its members are the scaffolded op surface, resolved through
                // the boxed self, not unpacked top-level columns. A FIXED
                // `T[N]` coll may pool (the Stack shape). This mirrors the
                // List-vs-Stack split (mod.rs build_field_index).
                if matches!(
                    self.ctx.coll_storage.get(b),
                    Some(crate::backend::llvm::coll_scaffold::CollStorage::HeapGrowable)
                ) {
                    return None;
                }
                Some(b.clone())
            }
            _ => None,
        })?;
        Some((base, reg))
    }

    pub(crate) fn emit_method_call(
        &mut self,
        out: &mut String,
        v: &str,
        recv: &Expr,
        name: &str,
        args: &[Expr],
        indent: &str,
    ) -> TypedRegister {
        let recv_tmp = self.fun.gen_reg();
        // 2026-08-07 (object instance pools): an unpacked instance receiver
        // (`b.set(...)`) — `b` has no slot (its members unpacked), so emitting
        // it would produce an undefined `@b`. The member body resolves bare
        // member names against the instance PREFIX; the base obj type comes
        // from the recorded instance Init. A SPAWNED handle local (`h.inc()`)
        // is a let-bound row id whose type names the obj — its register is
        // the pool row.
        let recv_prefix = match recv {
            Expr::Identifier(n) => {
                let p = self.instance_prefix_for(n);

                p
            }
            _ => None,
        };
        let (recv_reg, mut type_name) = if let Some((prefix, _row)) = &recv_prefix {
            let base = self.ctx.obj_instance_inits.get(prefix)
                .map(|(b, _)| b.clone())
                .unwrap_or_else(|| prefix.clone());
            let dummy = self.fun.gen_reg();
            writeln!(out, "{}{} = add i64 0, 0", indent, dummy).ok();
            (
                crate::backend::llvm::TypedRegister {
                    name: dummy,
                    ty: Type::Custom(base.clone()),
                },
                base,
            )
        } else {
            let recv_reg = self.emit_expr_inner(out, &recv_tmp, recv, indent);
            let type_name = match self.resolve_obj_key(&recv_reg.ty) {
                Some(n) => n,
                None => String::new(),
            };
            (recv_reg, type_name)
        };
        // 2026-07-31: a struct-typed state field loads as i64 (its address);
        // recover the struct type from field_briev_types for member lookup.
        if type_name.is_empty() {
            if let Expr::Identifier(rname) = recv {
                if let Some(&ridx) = self.ctx.field_index_map.get(rname) {
                    if let Some(Type::Custom(n)) = self.ctx.field_briev_types.get(ridx) {
                        type_name = n.clone();
                    }
                }
            }
        }
        let members = self.ctx.obj_members.get(&type_name).cloned().unwrap_or_default();
        // 2026-08-14 (UOL §6b.2): `a.OpName#(b)` — UFCS method form. Strip a
        // trailing `#` so the member lookup matches the op member (`op At`,
        // not a literal member named `At#`).
        let lookup_name = name.trim_end_matches('#');
        let member = members.iter().find(|m| member_briev_name(m) == lookup_name).cloned();
        let Some(member) = member else {
            // 2026-08-14 (UOL §6b): UFCS fallback — `a.f(b)` desugars to
            // `f(a, b)` when no member matches (learn-briev/00a §UFCS). For a
            // `#`-suffixed name, strip `#` and dispatch as a call (reaching the
            // generative op dispatch or a registered intrinsic); for a plain
            // name, call the top-level function with the receiver prepended.
            if name.ends_with('#') {
                // 2026-08-14 (UOL §6b): keep the `#` — `a.Add#(b)` dispatches
                // through the intrinsic signature (`Add#`) or generative op
                // identity, NOT a bare top-level function named `Add`.
                let mut all = vec![(*recv).clone()];
                all.extend(args.iter().cloned());
                return self.emit_expr_inner(out, v, &Expr::Call(name.to_string(), all, None), indent);
            }
            if self.ctx.defn_params.contains_key(name) {
                let mut all = vec![(*recv).clone()];
                all.extend(args.iter().cloned());
                return self.emit_user_call(out, v, name, &all, indent);
            }
            panic!("method call '.{}()': no member '{}' on '{}'", name, name, type_name);
        };
        let arg_regs: Vec<(String, Type)> = args.iter().map(|a| {
            let arg_tmp = self.fun.gen_reg();
            let r = self.emit_expr_inner(out, &arg_tmp, a, indent);
            (r.name, r.ty)
        }).collect();

        self.emit_member_body(out, v, MemberInvocation { recv_reg: &recv_reg, type_name: &type_name, member: &member, arg_regs: &arg_regs, prefix: recv_prefix }, indent)
    }

    /// 2026-07-31 (A5/A6): emit a member body with `self` bound to the
    /// receiver register and the given arg registers bound to the member's
    /// params. Shared by MethodCall codegen and the `<-` op dispatch
    /// (emit_strategy_member_call).
    /// 2026-08-07 (object instance pools): GEP an unpacked instance's member
    /// COLUMN at row 0 (the static instance) and return the row register, the
    /// member's Briev type (the column's dims[1..]; an empty tail means a
    /// scalar member), and the row's LLVM load type (the column's inner). A
    /// spawned instance would pass its id instead of 0.
    pub(crate) fn emit_instance_column_row(
        &mut self,
        out: &mut String,
        indent: &str,
        idx: usize,
        row_reg: &str,
    ) -> (String, Type, String) {
        let slot_ty = self.ctx.field_briev_types.get(idx).cloned().unwrap_or(Type::int());
        let col_ty = self.ctx.field_types.get(idx).cloned().unwrap_or_else(|| "i64".to_string());
        // 2026-08-09 (Phase 5): a BOXED/SPILLED instance's handle is its heap
        // block ADDRESS, not a pooled row id. When this slot's base is boxed,
        // the "column" is the per-instance block: inttoptr the handle + GEP the
        // member's byte offset, then load.
        let slot_name = self.ctx.field_index_map.iter()
            .find(|(_, v)| **v == idx)
            .map(|(k, _)| k.clone());
        if let Some(slot_name) = slot_name {
            if let Some((base, member)) = slot_name.split_once('.') {
                if let Some(offsets) = self.ctx.boxed_offsets.get(base) {
                    if let Some((off, mty)) = offsets.get(member) {
                        let ptr = self.fun.gen_reg();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, row_reg).ok();
                        let gep = self.fun.gen_reg();
                        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, gep, ptr, off).ok();
                        let llvm_ty = if matches!(mty, Type::Ptr(_)) {
                            "i64".to_string()
                        } else {
                            self.llvm_type(mty)
                        };
                        return (gep, mty.clone(), llvm_ty);
                    }
                }
            }
        }
        let base = self.emit_state_gep(out, indent, "i", "%state", idx);
        // 2026-08-07 (object instance pools): a DEPENDENT column is a heap
        // buffer — the slot holds the malloc'd buffer address. Load it, then
        // GEP the row inside the buffer (element = the member's llvm type).
        if let Some(elem_ty) = self.ctx.heap_columns.get(&idx) {
            let addr = self.fun.gen_reg();
            writeln!(out, "{}{} = load i64, ptr {}", indent, addr, base).ok();
            let buf = self.fun.gen_reg();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, buf, addr).ok();
            let row = self.fun.gen_reg();
            writeln!(out, "{}{} = getelementptr {}, ptr {}, i64 {}", indent, row, elem_ty, buf, row_reg).ok();
            let load_ty = elem_ty.clone();
            return (row, slot_ty, load_ty);
        }
        let row = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr {}, ptr {}, i64 0, i64 {}", indent, row, col_ty, base, row_reg).ok();
        let row_ty = match &slot_ty {
            Type::Vector(inner, dims) if dims.len() > 1 => {
                Type::Vector(inner.clone(), dims[1..].to_vec())
            }
            Type::Vector(inner, _) => (**inner).clone(),
            other => other.clone(),
        };
        // The load type = the column's inner (`[2 x i64]` → i64,
        // `[2 x { ptr, i64 }]` → `{ ptr, i64 }`, `[2 x float]` → float).
        let load_ty = col_ty
            .strip_prefix('[')
            .and_then(|rest| rest.split_once('x'))
            .map(|(_, t)| t.trim().trim_end_matches(']').trim().to_string())
            .unwrap_or_else(|| "i64".to_string());
        (row, row_ty, load_ty)
    }

    pub(crate) fn emit_member_body(
        &mut self,
        out: &mut String,
        v: &str,
        inv: MemberInvocation<'_>,
        indent: &str,
    ) -> TypedRegister {
        let recv_reg = inv.recv_reg;
        let type_name = inv.type_name;
        let member = inv.member;
        let arg_regs = inv.arg_regs;
        let self_prefix = inv.prefix;
        // 2026-08-07 (object instance pools): an UNPACKED instance's member
        // body resolves bare member names against the instance's top-level
        // slots (`st` → `st.data`/`st.len`) via `self_prefix`. The boxed
        // address self is the fallback (self_prefix None).
        // 2026-08-08 (pool lifecycle, Phase 1d): an obj INSTANCE must never
        // reach the boxed fallback — instance_prefix_for / unpacked_instance_prefix
        // resolve every instance (top-level + spawned handles) to a prefix.
        // The boxed path is STRUCT-only (struct state-fields, struct
        // construction, non-identifier receivers). If an obj receiver ever
        // lands here, that is a retired-path regression (the member body would
        // GEP a fake boxed self instead of the pool column) — fail loudly.
        let saved_prefix = self.fun.self_prefix.clone();
        let saved = self.fun.self_binding.clone();
        // 2026-08-13: clear the enclosing callable-txn convergence state while
        // the member body runs (see the restore below).
        let saved_ctr = self.fun.callable_txn_result.clone();
        let saved_ctpl = self.fun.callable_txn_post_label.clone();
        let saved_terminated = self.fun.terminated;
        self.fun.callable_txn_result = None;
        self.fun.callable_txn_post_label = None;
        self.fun.terminated = false;
        if let Some((prefix, row_reg)) = &self_prefix {
            self.fun.self_prefix = Some((prefix.clone(), row_reg.clone()));
            self.fun.self_binding = None;
        } else {
            // 2026-08-08 (pool lifecycle, Phase 1d): the boxed self path is
            // legitimate ONLY for non-pool objs — stdlib collection objs
            // (List, RingBuffer, ...) are boxed struct addresses, not instance
            // pools. A genuine POOL instance (one whose members are unpacked
            // into `{base}.{member}` top-level columns) must never land here:
            // its member body would GEP a fake boxed self instead of the pool
            // column. Detect a pool base by its `{base}.`-prefixed instance
            // slots (registered by build_field_index), and fail loudly if one
            // ever reaches the boxed fallback.
            let is_pool_instance = self.ctx.instance_slots.iter()
                .any(|slot| slot.starts_with(&format!("{}.", type_name)));
            if is_pool_instance {
                panic!(
                    "obj instance member call '.{}' on '{}' reached the retired boxed self path \
                     (instance must resolve to a pool prefix; this is a codegen regression)",
                    member_briev_name(member), type_name
                );
            }
            let self_ptr = self.fun.gen_reg();
            // 2026-08-11 (housekeeping 1b fix): the boxed self HANDLE is
            // `i{int_bits}` (a wasm32 List handle is i32) — inttoptr with the
            // target's integer width, not a hardcoded i64.
            let iw = format!("i{}", self.ctx.int_bits);
            writeln!(out, "{}{} = inttoptr {} {} to ptr", indent, self_ptr, iw, recv_reg.name).ok();
            self.fun.self_binding = Some((type_name.to_string(), self_ptr.clone()));
        }
        let saved_bindings = self.fun.let_bindings.clone();
        let saved_types = self.fun.let_binding_types.clone();
        let saved_orig = self.fun.let_original_types.clone();
        // 2026-07-31 (A5d): last_val_temps must NOT leak across emissions of
        // the same node body — the reactor emits a body more than once, and a
        // stale self-slot temp from the first pass would make the second
        // pass's reads resolve to the wrong register.
        let saved_lvt = self.fun.last_val_temps.clone();
        let saved_lvt_types = self.fun.last_val_types.clone();
        let (params, body): (Vec<(String, Type)>, Vec<crate::ast::Statement>) = match member {
            crate::ast::TopLevel::Transaction(t) => (
                t.parameters.iter().map(|(n, ty)| (n.clone(), ty.clone())).collect(),
                t.body.clone(),
            ),
            crate::ast::TopLevel::Definition(d) => (
                d.parameters.iter().map(|(n, ty)| (n.clone(), ty.clone())).collect(),
                d.body.clone(),
            ),
            // 2026-08-12 (Iterable protocol, op-as-member): an operator member
            // emits exactly like a defn member — the body is the implementation.
            crate::ast::TopLevel::TypeDefOperator(d) => (
                d.parameters.iter().map(|(n, ty)| (n.clone(), ty.clone())).collect(),
                d.body.clone(),
            ),
            _ => (Vec::new(), Vec::new()),
        };
        // 2026-08-12 (Iterable protocol, slice 4): substitute the member's
        // PARAM types with the collection's concrete args (`List<Int>` init's
        // `val: T` → `val: Int`) — without it a wasm32 member body bound `val`
        // as the raw generic `T`, so adapt_to_i64 couldn't widen the i32 value
        // to the i64 collection slot (`store i64 <i32>` was invalid IR). The
        // receiver type is usually a MONO key (`List<Int>`), so parse the base
        // + args from the type name.
        let (recv_base, recv_args) = match &recv_reg.ty {
            crate::ast::Type::Applied(n, a) => (n.clone(), a.clone()),
            crate::ast::Type::Custom(n) => {
                if let Some((b, rest)) = n.split_once('<') {
                    let inner = rest.trim_end_matches('>');
                    let args: Vec<crate::ast::Type> = inner
                        .split(',')
                        .filter_map(|a| {
                            let a = a.trim();
                            if a == "Int" {
                                Some(crate::ast::Type::int())
                            } else if a == "Bool" {
                                Some(crate::ast::Type::bool_())
                            } else if a == "String" {
                                Some(crate::ast::Type::string())
                            } else if a == "Float" {
                                Some(crate::ast::Type::float())
                            } else if a == "Float64" {
                                Some(crate::ast::Type::float64())
                            } else {
                                None
                            }
                        })
                        .collect();
                    (b.to_string(), args)
                } else {
                    (n.clone(), Vec::new())
                }
            }
            _ => (String::new(), Vec::new()),
        };
        let recv_type_params = self.ctx.obj_type_params.get(&recv_base).cloned().unwrap_or_default();
        let recv_subst: std::collections::HashMap<String, crate::ast::Type> =
            recv_type_params.into_iter().zip(recv_args.into_iter()).collect();
        let params: Vec<(String, crate::ast::Type)> = params
            .into_iter()
            .map(|(n, t)| (n.clone(), crate::typechecker::substitute_type(&t, &recv_subst)))
            .collect();
        for (i, (reg, rty)) in arg_regs.iter().enumerate() {
            if let Some((pname, pty)) = params.get(i) {
                // 2026-08-12 (slice 4): a String arg is bound as the PTR (the
                // "String in a register is a ptr" invariant) — the member body's
                // own stores (inner.data[len] = val) box it via adapt_to_i64.
                // The previous pre-boxing here (ptrtoint at the boundary) made
                // wasm32 DOUBLE-box (the store adapts again), producing
                // `ptrtoint ptr <i64 handle>`.
                self.fun.let_bindings.insert(pname.clone(), reg.clone());
                self.fun.let_binding_types.insert(pname.clone(), pty.clone());
                self.fun.let_original_types.insert(pname.clone(), pty.clone());
            }
            let _ = rty;
        }
        crate::backend::llvm::emit_stmt::emit_statement_sequence(self, out, &body, indent);
        self.fun.self_binding = saved;
        self.fun.self_prefix = saved_prefix;
        self.fun.let_bindings = saved_bindings;
        self.fun.let_binding_types = saved_types;
        self.fun.let_original_types = saved_orig;
        self.fun.last_val_temps = saved_lvt;
        self.fun.last_val_types = saved_lvt_types;
        // 2026-08-13 (member term inside a callable txn): the member body's
        // `term X` must record member_result, NOT terminate the ENCLOSING txn.
        // With callable_txn_result left set, an inlined `op At` body (`term
        // inner.data[i]`) inside join_loop stored to %result + br post +
        // terminated=true, silently dropping join_loop's own `term result`.
        self.fun.callable_txn_result = saved_ctr;
        self.fun.callable_txn_post_label = saved_ctpl;
        self.fun.terminated = saved_terminated;
        let _ = v;
        // 2026-08-01 (D3): a defn member's `term` value is the call's result —
        // return it (the caller binds it). Otherwise fall back to a fresh
        // void register (side-effect members like push/pop have no result).
        if let Some((rname, rty)) = self.fun.member_result.take() {
            TypedRegister { name: rname, ty: rty }
        } else {
            TypedRegister { name: self.fun.gen_reg(), ty: Type::void() }
        }
    }

    fn emit_reflection(
        &mut self,
        out: &mut String,
        v: &str,
        recv: &Expr,
        target: &str,
        kind: ReflectKind,
        indent: &str,
    ) -> TypedRegister {
        let _ = v;
        let recv_tmp = self.fun.gen_reg();
        let recv_reg = self.emit_expr_inner(out, &recv_tmp, recv, indent);
        match (target, kind) {
            // 2026-08-09 (Phase 12, SPEC §19.3): `feature.^^Available` — a
            // compile-time descriptor reflect that folds to a runtime
            // briev_symbol_available(symbol) check for an `optional frgn`. The
            // receiver is the frgn's local name; its FOREIGN symbol is what
            // gets checked (the codegen resolves it via the frgn declaration).
            ("Available", ReflectKind::CompileTime) => {
                let symbol = match recv {
                    Expr::Identifier(n) => {
                        let fb = self.ctx.frgn_map.get(n.as_str()).cloned();
                        let sym = match fb {
                            Some(sig) => sig.name.clone(),
                            None => n.clone(),
                        };
                        sym
                    }
                    _ => String::new(),
                };
                let symbol = match recv {
                    Expr::Identifier(n) => {
                        let fb = self.ctx.frgn_map.get(n.as_str()).cloned();
                        let sym = match fb {
                            Some(sig) => sig.name.clone(),
                            None => n.clone(),
                        };
                        sym
                    }
                    _ => String::new(),
                };
                // Emit the symbol as a stack NUL-terminated buffer (avoid the
                // pre-collected string-global pass, which runs before bodies).
                let buf = self.fun.gen_reg();
                let sym_bytes = symbol.as_bytes();
                writeln!(out, "{}{} = alloca i8, i64 {}", indent, buf, sym_bytes.len() + 1).ok();
                for (bi, b) in sym_bytes.iter().enumerate() {
                    let gep = self.fun.gen_reg();
                    writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, gep, buf, bi).ok();
                    writeln!(out, "{}  store i8 {}, ptr {}, align 1", indent, *b as i8, gep).ok();
                }
                let nul = self.fun.gen_reg();
                writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 {}", indent, nul, buf, sym_bytes.len()).ok();
                writeln!(out, "{}  store i8 0, ptr {}, align 1", indent, nul).ok();
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = call i64 @briev_symbol_available(ptr {})", indent, r, buf).ok();
                let b = self.fun.gen_reg();
                writeln!(out, "{}{} = trunc i64 {} to i1", indent, b, r).ok();
                let c = self.fun.gen_reg();
                writeln!(out, "{}{} = zext i1 {} to i8", indent, c, b).ok();
                TypedRegister { name: c, ty: Type::bool_() }
            }
            ("Size", ReflectKind::CompileTime) => {
                let count = self.vector_element_count(&recv_reg.ty);
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 0, {}", indent, r, count).ok();
                TypedRegister { name: r, ty: Type::int() }
            }
            // 2026-08-14 (Boxed Cat, iterable-protocol §10.4, tri-partite rule):
            // runtime `.^Size` is DELETED — the element count of a collection
            // is an operation, so its home is the `Count#` intrinsic, not a
            // reflection target (§6a of the 2026-08-14 plan). The typechecker
            // rejects runtime `.^Size` as a kind error, but a precondition
            // (elaborate_expr bypass) can still reach codegen — emit a clean
            // compile error directing to `Count#`, never the old `len`-slot
            // heuristic and never a silent fallback. `.^^Size` (compile-time,
            // above) keeps the vector shape.
            ("Size", ReflectKind::Runtime) => {
                panic!(
                    "runtime reflection 'Size' is deleted — the element count of a collection \
                     is the `Count#` intrinsic (e.g. `Count#(items)`), not a reflection target \
                     (2026-08-14 §6a)"
                )
            }
            ("Bytes", ReflectKind::CompileTime) => {
                // 2026-08-01 (B3): `x.^^Bytes` on a #String → the `Bytes` prop
                // default = O(1) header read (byte length is the [0] length
                // prefix of the [len][bytes] buffer). For non-strings, the
                // compile-time type size.
                if self.is_string_operand(&recv_reg.ty) {
                    let r = self.fun.gen_reg();
                    writeln!(out, "{}{} = load i64, ptr {}", indent, r, recv_reg.name).ok();
                    return TypedRegister { name: r, ty: Type::int() };
                }
                let sz = types::type_size(&recv_reg.ty, self.ctx.type_universe.as_ref());
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 0, {}", indent, r, sz).ok();
                TypedRegister { name: r, ty: Type::int() }
            }
            ("Alignment", ReflectKind::CompileTime) => {
                let align = self.type_alignment(&recv_reg.ty);
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 0, {}", indent, r, align).ok();
                TypedRegister { name: r, ty: Type::int() }
            }
            ("Ptr", ReflectKind::Runtime) => {
                // x.^Ptr ≡ &x — reuse the address-of path.
                let ptr_tmp = self.fun.gen_reg();
                self.emit_expr_inner(out, &ptr_tmp, &Expr::AddrOf(Box::new(recv.clone())), indent)
            }
            ("Length", ReflectKind::Runtime) => match &recv_reg.ty {
                Type::Vector(_, _) => {
                    let count = self.vector_element_count(&recv_reg.ty);
                    let r = self.fun.gen_reg();
                    writeln!(out, "{}{} = add i64 0, {}", indent, r, count).ok();
                    TypedRegister { name: r, ty: Type::int() }
                }
                 // 2026-08-12 (Iterable protocol): `x.^Length` on a #String is
                 // the STORED byte count — the [len] header of the
                 // [len][bytes] handle (O(1), no scan). The UTF8 CHARACTER
                 // count is the `CharCount#` intrinsic (a computed scan; SPEC
                 // §17.1/§17.3).
                 ty if self.is_string_operand(ty) => {
                     let r = self.fun.gen_reg();
                     writeln!(out, "{}{} = load i64, ptr {}", indent, r, recv_reg.name).ok();
                     TypedRegister { name: r, ty: Type::int() }
                 }
                 // 2026-08-06 (Phase 7): `x.^Length` on a #Blob — the byte length
                 // is the [len] header of the [len][bytes] handle (O(1), no
                 // codepoint scan). Data values are ptr handles like Strings.
                 ty if matches!(ty, Type::Custom(n) if n == "Blob") => {
                     let r = self.fun.gen_reg();
                     writeln!(out, "{}{} = load i64, ptr {}", indent, r, recv_reg.name).ok();
                     TypedRegister { name: r, ty: Type::int() }
                 }
                 // 2026-08-04 (compiler-in-Briev): a String value boxed to an
                 // i64 HANDLE at a call/binding boundary (String param, frgn
                 // result) is typed Custom("Int")/Int here — the physical value
                 // is still the [len][bytes] pointer. Recover the semantic type
                 // from the binding (the let's declared type) and inttoptr the
                 // handle before the byte-header read. 2026-08-12: `.^Length` is
                 // the STORED byte count; `CharCount#` is the char scan.
                 other if self.is_semantic_string(recv, &recv_reg) => {
                     let p = self.string_ptr(out, indent, &recv_reg);
                     let r = self.fun.gen_reg();
                     writeln!(out, "{}{} = load i64, ptr {}", indent, r, p).ok();
                     TypedRegister { name: r, ty: Type::int() }
                 }
                 // 2026-08-15 (coll plan §3.4.6): `x.^Length` on a `coll` type
                 // is the compiler-owned stored length. `coll obj`: the hidden
                 // `len` slot (offset 16 of [data, cap, len]). `coll struct`
                 // (fixed T[N]): the array element count N (a compile-time
                 // constant). O(1), SPEC §17.1.
                 other if self.is_coll_type(&recv_reg.ty) => {
                     let coll_base = match &recv_reg.ty {
                         Type::Custom(n) | Type::Applied(n, _) => n.as_str(),
                         _ => "",
                     };
                     match self.ctx.coll_storage.get(coll_base) {
                         Some(crate::backend::llvm::coll_scaffold::CollStorage::InlineFixed) => {
                             // N from the T[N] sequence member's dim.
                             let n = self.coll_fixed_length(&recv_reg.ty);
                             let r = self.fun.gen_reg();
                             writeln!(out, "{}{} = add i64 0, {}", indent, r, n).ok();
                             TypedRegister { name: r, ty: Type::int() }
                         }
                         _ => {
                             let p = self.fun.gen_reg();
                             let gep = self.fun.gen_reg();
                             let r = self.fun.gen_reg();
                             writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, recv_reg.name).ok();
                             writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 16", indent, gep, p).ok();
                             writeln!(out, "{}{} = load i64, ptr {}", indent, r, gep).ok();
                             TypedRegister { name: r, ty: Type::int() }
                         }
                     }
                 }
                other => panic!(
                    "runtime reflection target 'Len' on '{:?}' (reg ty {:?}) has no codegen yet (Phase-1b boundary)",
                    recv, recv_reg.ty
                ),
            },
            // 2026-08-14 (boundary plan, SPEC §17.3): the `.^Absolute` codegen
            // arm was REMOVED — the typechecker rejects it as an unknown
            // target (abs is the `Abs#` intrinsic). The catch-all below panics
            // if it ever reaches codegen, so no reflection emission remains.
            // 2026-08-06 (Phase 8): `x.^^Type` — a frozen descriptor = the
            // protocol category code (Int=0, Float=1, Bool=2, Char=3, Bits=4,
            // Product=5, Sum=6, Ref=7, Closure=8, Void=9), matching the
            // interpreter's reflect_type_code (rule #4 parity). A single
            // constant — no globals, no address acquisition.
            ("Type", ReflectKind::CompileTime) => {
                let code = self.type_category_code(&recv_reg.ty);
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 0, {}", indent, r, code).ok();
                TypedRegister { name: r, ty: Type::int() }
            }
            // 2026-08-14 (boundary plan): `x.^^Element` — a frozen descriptor =
            // the ELEMENT type's category code, folded exactly like `.^^Type`
            // (the typechecker already proved iterability + derived the element
            // single-source; this only materializes the code). A constant, no
            // globals, no runtime code.
            ("Element", ReflectKind::CompileTime) => {
                let elem_ty = self.reflect_element_type(&recv_reg.ty);
                let code = self.type_category_code(&elem_ty);
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 0, {}", indent, r, code).ok();
                TypedRegister { name: r, ty: Type::int() }
            }
            _ => panic!(
                "reflection '{}' with kind '{:?}' reached codegen without emission",
                target, kind
            ),
        }
    }

    /// 2026-07-31: Element count of a fixed-size vector type; 1 for scalars.
    pub(super) fn vector_element_count(&self, ty: &Type) -> u64 {
        match ty {
            Type::Vector(_, dims) => dims
                .iter()
                .map(|d| match d {
                    Dimension::Anonymous(n) => *n as u64,
                    _ => 1,
                })
                .product(),
            _ => 1,
        }
    }

    /// 2026-08-06 (Phase 8): the `x.^^Type` frozen-descriptor category code.
    /// MUST match interpreter::reflect_type_code (rule #4 parity).
    fn type_category_code(&self, ty: &Type) -> i64 {
        if self.is_string_operand(ty) || matches!(ty, Type::Bits(_)) {
            return 4;
        }
        match ty {
            Type::Custom(n) if n == "Float" || n == "Float64" || n == "Double" || n == "Float32" => 1,
            Type::Custom(n) if n == "Bool" || n == "UInt8" || n == "Int8" => 2,
            Type::Custom(n) if n == "Char" || n == "Byte" => 3,
            Type::Ptr(_) | Type::PtrConst(_) | Type::LayoutPtr(_) => 7,
            Type::Custom(n) if self.ctx.struct_types.contains_key(n) => 5,
            _ => 0,
        }
    }

    /// 2026-08-14 (boundary plan): the ELEMENT type of a `x.^^Element` receiver,
    /// folded from the receiver's static type. Mirrors the typechecker's
    /// `resolve_element_type` (single-source proof form, rule #4 parity): a
    /// `#String` operand → `Char` (frozen protocol fact), a Tier-2/1 type → the
    /// read op's return substituted with the concrete generic args, a vector →
    /// the inner type. The typechecker already validated iterability before
    /// codegen, so a None here is an internal inconsistency, not a user error.
    fn reflect_element_type(&self, ty: &Type) -> Type {
        use crate::ast::TopLevel;
        if self.is_string_operand(ty) {
            return Type::Custom("Char".to_string());
        }
        if let Type::Vector(inner, _) = ty {
            return (**inner).clone();
        }
        let base = match ty {
            Type::Custom(n) | Type::Applied(n, _) => n.clone(),
            _ => return Type::int(),
        };
        let members = self.ctx.obj_members.get(&base).cloned().unwrap_or_default();
        let at = members.iter().find_map(|m| match m {
            TopLevel::TypeDefOperator(d) if d.name == "At" => Some(d),
            _ => None,
        });
        let current = at.or_else(|| members.iter().find_map(|m| match m {
            TopLevel::TypeDefOperator(d) if d.name == "Current" => Some(d),
            _ => None,
        }));
        let Some(read) = current else { return Type::int(); };
        let Some(raw) = read.output_type.as_ref().and_then(|o| o.all_types().into_iter().next()) else {
            return Type::int();
        };
        let args = match ty {
            Type::Applied(_, a) => a.clone(),
            _ => Vec::new(),
        };
        let params = self.ctx.obj_type_params.get(&base).cloned().unwrap_or_default();
        let subst: std::collections::HashMap<String, Type> =
            params.into_iter().zip(args).collect();
        crate::typechecker::substitute_type(&raw, &subst)
    }

    /// 2026-07-31: Alignment of a type in bytes (compile-time reflection).
    fn type_alignment(&self, ty: &Type) -> u64 {
        match ty {
            Type::Ptr(_) => 8,
            Type::Vector(inner, _) => self.type_alignment(inner),
            Type::Custom(s) => match s.as_str() {
                "Int" | "Int64" | "Float64" | "String" | "Ptr" => 8,
                "Int32" | "Float" => 4,
                "Int16" => 2,
                "Int8" | "Bool" | "Char" | "Byte" => 1,
                _ => 8,
            },
            _ => 8,
        }
    }

    // 2026-07-24: Struct array list literal — detect when all elements
    // are struct literals of the same C-compatible struct type.
    fn detect_struct_list(&self, exprs: &[Expr]) -> Option<String> {
        if exprs.is_empty() {
            return None;
        }
        let mut common_type: Option<&str> = None;
        for expr in exprs {
            match expr {
                Expr::StructLiteral { type_name, .. } => {
                    match common_type {
                        None => {
                            if self.ctx.struct_types.contains_key(type_name) {
                                common_type = Some(type_name.as_str());
                            } else {
                                return None;
                            }
                        }
                        Some(t) => {
                            if type_name != t {
                                return None;
                            }
                        }
                    }
                }
                _ => return None,
            }
        }
        common_type.map(|s| s.to_string())
    }

    // 2026-07-24: Struct array list codegen — emit a contiguous stack array
    // for a list literal whose elements are all struct literals of the same
    // known struct type. Used by bridge code for C-compatible method tables.
    pub(crate) fn emit_struct_array(
        &mut self,
        out: &mut String,
        v: &str,
        exprs: &[Expr],
        elem_type_name: &str,
        indent: &str,
    ) -> TypedRegister {
        let elem_size = self.struct_type_size(elem_type_name);
        let count = exprs.len() as u64;
        let total_size = elem_size * count;

        let alloca_reg = self.fun.gen_reg();
        // 2026-08-13 (reactor fix): inside a reactor loop body the alloca is
        // deferred to the loop PREHEADER (flush_pending_struct_allocas) — an
        // alloca in the loop makes clang -O3 peel the loop and emit a bogus
        // exit assumption (the node fires once). Elsewhere it stays inline.
        if self.fun.defer_struct_allocas {
            self.fun.pending_struct_allocas.push(format!("{}  {} = alloca i8, i64 {}", indent, alloca_reg, total_size));
        } else {
            writeln!(out, "{}  {} = alloca i8, i64 {}", indent, alloca_reg, total_size).ok();
        }

        for (i, expr) in exprs.iter().enumerate() {
            let Expr::StructLiteral { type_name, fields } = expr else { continue; };
            let base_offset = (i as u64) * elem_size;
            for (field_name, field_expr) in fields {
                let fr = self.fun.gen_reg();
                let val = self.emit_expr_inner(out, &fr, field_expr, indent);
                let field_offset = self.lookup_field_offset(type_name, field_name);
                let offset = base_offset + field_offset;
                let ptr_reg = self.fun.gen_reg();
                writeln!(out, "{}  {} = getelementptr i8, ptr {}, i64 {}",
                    indent, ptr_reg, alloca_reg, offset).ok();
                // 2026-08-13 (Phase 5): an `atomic` array element field is
                // written with an atomic store (before the packed path).
                if self.is_atomic_field(type_name, field_name) {
                    let alt = if matches!(val.ty, Type::Ptr(_)) {
                        format!("i{}", self.ctx.int_bits)
                    } else {
                        self.llvm_type(&val.ty)
                    };
                    let asz = types::type_size(&val.ty, self.ctx.type_universe.as_ref()).max(1);
                    let store_val = self.ensure_typed_value(
                        out, indent, &alt, &val.name, Some(val.ty.clone()),
                        self.ctx.type_universe.clone().as_ref(),
                    );
                    writeln!(out, "{}  store atomic {} {}, ptr {} seq_cst, align {}", indent, alt, store_val, ptr_reg, asz).ok();
                    continue;
                }
                // 2026-08-13 (pack): packed elements store field bit-slices.
                if let Some(pf) = self.packed_field(type_name, field_name) {
                    out.push_str(&self.emit_packed_field_store(indent, &ptr_reg, &pf, &val));
                    continue;
                }
                if val.ty == Type::int() && self.is_ptr_field(type_name, field_name) {
                    let conv = self.fun.gen_reg();
                    writeln!(out, "{}  {} = inttoptr i64 {} to ptr",
                        indent, conv, val.name).ok();
                    writeln!(out, "{}  store ptr {}, ptr {}",
                        indent, conv, ptr_reg).ok();
                } else {
                    let fty = self.llvm_type(&val.ty);
                    writeln!(out, "{}  store {} {}, ptr {}",
                        indent, fty, val.name, ptr_reg).ok();
                }
            }
        }

        let result = self.fun.gen_reg();
        let hw = format!("i{}", self.ctx.int_bits);
        writeln!(out, "{}  {} = ptrtoint ptr {} to {}", indent, result, alloca_reg, hw).ok();
        self.fun.struct_literal_allocas.insert(result.clone(), alloca_reg.clone());
        TypedRegister { name: result, ty: Type::Custom(elem_type_name.to_string()) }
    }

    /// Look up the byte offset of a field in a struct definition.
    /// Get the fields of a struct type from the type universe or struct_types.
    /// 2026-07-24: Falls back to struct_types (registration pass) when the
    /// type universe is unavailable (common in test environments).
    fn get_struct_fields(&self, type_name: &str) -> Option<&[(String, Type)]> {
        // 2026-08-12 (slice 4, wasm32 maze): a MONO key (`ListBuffer<Int>`)
        // must prefer the SUBSTITUTED struct_types — the universe holds the
        // generic base fields (`Ptr<T>`), which leak the raw type param into
        // wasm32 width resolution (`inner.data` resolved to `Ptr<T>`, so the
        // element width couldn't be derived and the load stayed i64).
        if type_name.contains('<') {
            return self.ctx.struct_types.get(type_name).map(|v| v.as_slice());
        }
        // Try type universe first (production path, has precise types)
        if let Some(ref u) = self.ctx.type_universe {
            if let Some(info) = u.types.get(type_name) {
                if !info.fields.is_empty() {
                    return Some(&info.fields);
                }
            }
        }
        // Fall back to struct_types (set during registration, always available)
        self.ctx.struct_types.get(type_name).map(|v| v.as_slice())
    }

    /// Compute total byte size of a struct type from its fields.
    /// 2026-07-24: Falls back to struct_types when universe unavailable.
    /// 2026-08-13 (pack): a packed struct is bit-granular even in the fallback
    /// — Σ type_size over-counts sub-byte fields (Bits(12)+Bits(4) is 3 bytes
    /// as type_sizes, 2 bytes packed).
    pub(crate) fn struct_type_size(&self, type_name: &str) -> u64 {
        // Try type universe first
        if let Some(ref u) = self.ctx.type_universe {
            if let Some(info) = u.types.get(type_name) {
                if info.bytes > 0 {
                    return info.bytes;
                }
            }
        }
        // Fall back: compute from struct_types fields
        if self.ctx.packed_structs.contains(type_name) {
            if let Some(fields) = self.ctx.struct_types.get(type_name) {
                return crate::type_universe::packed_bytes(
                    fields,
                    self.ctx.type_universe.as_ref(),
                );
            }
        }
        // 2026-08-13 (Phase 6): a union's storage is its largest field.
        if self.ctx.unions.contains(type_name) {
            if let Some(fields) = self.ctx.struct_types.get(type_name) {
                return fields.iter().map(|(_, ty)| {
                    types::type_size(ty, self.ctx.type_universe.as_ref())
                }).max().unwrap_or(0);
            }
        }
        self.ctx.struct_types.get(type_name)
            .map(|fields| {
                fields.iter().map(|(_, ty)| types::type_size(ty, self.ctx.type_universe.as_ref())).sum()
            })
            .unwrap_or(8)
    }

    /// 2026-07-24: Computes offsets from field types using type_size (pack=1).
    /// Previously used simplified i*8 which was wrong for mixed-size fields.
    /// 2026-08-13 (pack): a packed struct's byte offset comes from the shared
    /// packed-layout authority (`src/type_universe/packed.rs`) — Σ type_size
    /// over-counts sub-byte fields, while whole-byte packed fields land on
    /// identical offsets as before. The returned byte is where the field's
    /// covering byte region starts (`PackedField.byte`); sub-byte reads/writes
    /// slice from there.
    pub(crate) fn lookup_field_offset(&self, type_name: &str, field_name: &str) -> u64 {
        // 2026-08-13 (Phase 6): a union's fields all overlay at offset 0.
        if self.ctx.unions.contains(type_name) {
            return 0;
        }
        if self.ctx.packed_structs.contains(type_name) {
            return self
                .packed_field(type_name, field_name)
                .map_or(0, |pf| pf.byte);
        }
        if let Some(fields) = self.get_struct_fields(type_name) {
            let mut offset = 0u64;
            for (fname, ftype) in fields {
                if fname == field_name {
                    return offset;
                }
                offset += types::type_size(ftype, self.ctx.type_universe.as_ref());
            }
        }
        0
    }

    /// 2026-08-13 (reactor fix): write the deferred struct-literal allocas
    /// into the output at the caller's chosen point (function entry or loop
    /// preheader). The allocas precede the buffered body textually (SSA
    /// dominance) but sit OUTSIDE any loop (clang -O3 peel hazard).
    pub(super) fn flush_pending_struct_allocas(&mut self, out: &mut String) {
        let allocas = std::mem::take(&mut self.fun.pending_struct_allocas);
        for line in allocas {
            writeln!(out, "{}", line).ok();
        }
    }

    /// 2026-08-13 (Phase 5): whether a struct field is declared `atomic`.
    /// Keyed `<type>.<field>` (registration populated the set from the
    /// parser's structured `atomic_fields` carrier).
    pub(crate) fn is_atomic_field(&self, type_name: &str, field_name: &str) -> bool {
        self.ctx.atomic_fields.contains(&format!("{}.{}", type_name, field_name))
    }

    /// 2026-08-13 (pack, layout-keywords plan Phase 2): the packed slice for
    /// one field, or None when the struct is not packed. The single authority
    /// for slice derivation (`packed_field_offsets`) consumed by every field
    /// load/store site in codegen — never recompute offsets inline.
    pub(crate) fn packed_field(&self, type_name: &str, field_name: &str) -> Option<crate::type_universe::PackedField> {
        if !self.ctx.packed_structs.contains(type_name) {
            return None;
        }
        let fields = self.get_struct_fields(type_name)?;
        let endian = self.struct_endian(type_name);
        let endian = endian.as_deref();
        crate::type_universe::packed_field_offsets(
            fields,
            endian,
            self.ctx.type_universe.as_ref(),
        )
        .into_iter()
        .find(|p| p.name == field_name)
    }

    /// 2026-08-13: the declared bit order (`spec Endian` metadata) surfaced
    /// from the registered universe. Absent or `Target` → native (the packed
    /// helper treats both as little-endian).
    pub(crate) fn struct_endian(&self, type_name: &str) -> Option<String> {
        self.ctx.type_universe.as_ref()?.get(type_name).and_then(|r| {
            match r.properties.get("endian") {
                Some(crate::ast::PropertyValue::Identifier(s)) => Some(s.clone()),
                _ => None,
            }
        })
    }

    /// 2026-08-13 (pack, rule-19 validated): emit a loaded field's bit-slice
    /// from the struct byte image — covered load (i{cov*8}, align 1) → endian
    /// byte-reversal (BE only) → right-shift → truncate to i{bits}. Whole-byte
    /// fields (shift 0, bits == cov*8) reduce to a plain aligned load, so
    /// packed whole-byte structs keep the byte-offset GEP + load native path.
    /// Returns the register holding an i{bits} value. A zero-width field is
    /// padding; its read yields the i8 constant 0 (no legal i0 value exists).
    pub(crate) fn emit_packed_field_load(
        &mut self,
        out: &mut String,
        indent: &str,
        base_ptr: &str,
        pf: &crate::type_universe::PackedField,
    ) -> String {
        if pf.bits == 0 {
            let r = self.fun.gen_reg();
            writeln!(out, "{}{} = or i8 0, 0", indent, r).ok();
            return r;
        }
        let w = pf.cov * 8;
        let raw = self.fun.gen_reg();
        writeln!(out, "{}{} = load i{}, ptr {}, align 1", indent, raw, w, base_ptr).ok();
        // Whole-byte field: the raw load IS the value.
        if pf.shift == 0 && pf.bits == w {
            return raw;
        }
        // Big-endian: bytes arrive little-endian from the address order; the
        // field sits at the TOP of the covered region, so mirror the bytes.
        let mut x = raw;
        if pf.endian == crate::type_universe::EndianKind::Big && pf.cov > 1 {
            x = self.emit_byte_reverse(out, indent, &x, pf.cov);
        }
        if pf.shift > 0 {
            let s = self.fun.gen_reg();
            writeln!(out, "{}{} = lshr i{} {}, {}", indent, s, w, x, pf.shift).ok();
            x = s;
        }
        let r = self.fun.gen_reg();
        writeln!(out, "{}{} = trunc i{} {} to i{}", indent, r, w, x, pf.bits).ok();
        r
    }

    /// 2026-08-13 (pack): mirror the byte order of a cov-byte integer that
    /// was loaded little-endian from address order — Big-endian fields sit at
    /// the TOP of their covered region. cov ≤ 8; a middle byte (odd cov)
    /// needs no move and is OR'd as-is.
    pub(crate) fn emit_byte_reverse(
        &mut self,
        out: &mut String,
        indent: &str,
        v: &str,
        cov: u64,
    ) -> String {
        let w = cov * 8;
        let mut acc: Option<String> = None;
        for k in 0..cov {
            let lane = self.fun.gen_reg();
            let lane_mask: u64 = 0xFFu64 << (8 * k as usize);
            writeln!(out, "{}{} = and i{} {}, {}", indent, lane, w, v, lane_mask).ok();
            let src = 8 * k as i64;
            let dst = 8 * (cov - 1 - k) as i64;
            let placed = if src == dst {
                lane
            } else {
                let s = self.fun.gen_reg();
                if src < dst {
                    writeln!(out, "{}{} = shl i{} {}, {}", indent, s, w, lane, dst - src).ok();
                } else {
                    writeln!(out, "{}{} = lshr i{} {}, {}", indent, s, w, lane, src - dst).ok();
                }
                s
            };
            acc = Some(match acc {
                None => placed,
                Some(a) => {
                    let r = self.fun.gen_reg();
                    writeln!(out, "{}{} = or i{} {}, {}", indent, r, w, a, placed).ok();
                    r
                }
            });
        }
        acc.expect("cov >= 1")
    }

    /// 2026-08-13 (pack): bit-insert store of a field value into the packed
    /// byte image at `base_ptr` (the field's covering region start). Covers
    /// whole-byte fields (plain aligned store of the adapted i{bits} value)
    /// and sub-byte fields (load-modify-store: clear the field's span in the
    /// covered integer, insert the shifted value, write back). Zero-width
    /// fields are padding — nothing to store. Returns the emitted IR block;
    /// the caller appends it to its output buffer.
    pub(crate) fn emit_packed_field_store(
        &mut self,
        indent: &str,
        base_ptr: &str,
        pf: &crate::type_universe::PackedField,
        val: &TypedRegister,
    ) -> String {
        let mut out = String::new();
        let w = pf.cov * 8;
        // Adapt the value register to the covering width i{w}: `Bits<N>` (both
        // AST forms) is i{N} (zext when N < w), an `Int` is i64 (trunc).
        let src = if let Some(n) = crate::type_universe::bits_width(&val.ty) {
            n
        } else {
            let lt = self.llvm_type(&val.ty);
            lt.trim_start_matches('i')
                .parse::<u64>()
                .unwrap_or(self.ctx.int_bits as u64)
        };
        let mut v = if src == w {
            val.name.clone()
        } else if src < w {
            let r = self.fun.gen_reg();
            writeln!(out, "{}{} = zext i{} {} to i{}", indent, r, src, val.name, w).ok();
            r
        } else {
            let r = self.fun.gen_reg();
            writeln!(out, "{}{} = trunc i{} {} to i{}", indent, r, src, val.name, w).ok();
            r
        };
        if pf.shift == 0 && pf.bits == w {
            writeln!(out, "{}store i{} {}, ptr {}, align 1", indent, w, v, base_ptr).ok();
            return out;
        }
        if pf.bits == 0 {
            return out; // padding
        }
        // 2026-08-13: mask the value to the field's width before insertion —
        // a packed store drops the upper bits by definition (the field's
        // declared width is its value domain). This also compensates the
        // casting graph's identity `Int → Bits<N>` lane, which leaves
        // out-of-range values untruncated for sub-byte widths (BUGS.md).
        if pf.bits < 64 {
            let mask: u64 = (1u64 << pf.bits) - 1;
            let m = self.fun.gen_reg();
            writeln!(out, "{}{} = and i{} {}, {}", indent, m, w, v, mask).ok();
            v = m;
        }
        let mshift: u64 = if pf.bits == 64 {
            0u64.wrapping_sub(0) // field spans the full word — no mask holes
        } else {
            ((1u64 << pf.bits) - 1) << pf.shift
        };
        let not_mask = if w == 64 {
            !mshift
        } else {
            !mshift & ((1u64 << w) - 1)
        };
        let raw = self.fun.gen_reg();
        writeln!(out, "{}{} = load i{}, ptr {}, align 1", indent, raw, w, base_ptr).ok();
        let mut region = raw;
        if pf.endian == crate::type_universe::EndianKind::Big && pf.cov > 1 {
            region = self.emit_byte_reverse(&mut out, indent, &region, pf.cov);
        }
        let cleared = self.fun.gen_reg();
        writeln!(out, "{}{} = and i{} {}, {}", indent, cleared, w, region, not_mask).ok();
        let shifted = self.fun.gen_reg();
        writeln!(out, "{}{} = shl i{} {}, {}", indent, shifted, w, v, pf.shift).ok();
        let merged = self.fun.gen_reg();
        writeln!(out, "{}{} = or i{} {}, {}", indent, merged, w, cleared, shifted).ok();
        let mut out_val = merged;
        if pf.endian == crate::type_universe::EndianKind::Big && pf.cov > 1 {
            out_val = self.emit_byte_reverse(&mut out, indent, &out_val, pf.cov);
        }
        writeln!(out, "{}store i{} {}, ptr {}, align 1", indent, w, out_val, base_ptr).ok();
        out
    }

    /// Check if a struct field has pointer type.
    fn is_ptr_field(&self, type_name: &str, field_name: &str) -> bool {
        if let Some(fields) = self.get_struct_fields(type_name) {
            for (fn_, ftype) in fields {
                if fn_ == field_name {
                    return matches!(ftype, Type::Ptr(_));
                }
            }
        }
        false
    }

    /// Emit a foreign function call with optional auto-meld.
    fn emit_frgn_call(
        &mut self,
        out: &mut String,
        v: &str,
        sig: &crate::ast::ForeignSignature,
        args: &[Expr],
        indent: &str,
    ) -> TypedRegister {
        // 2026-07-22: Look up the pre-resolved dispatch strategy.
        // Clone the dispatch enum to avoid the borrow checker issue:
        // we need to borrow self.resolved_frgns immutably and then call
        // self.emit_* which borrows self mutably.
        let dispatch = self.resolved_frgns.as_ref()
            .and_then(|m| m.get(&sig.name))
            .cloned();
        match dispatch {
            Some(crate::analysis::frgn_dispatch::ResolvedFrgn::Inline { symbol, .. }) => {
                let sym = symbol.clone();
                self.emit_direct_frgn_call(out, v, &sym, sig, args, indent)
            }
            Some(crate::analysis::frgn_dispatch::ResolvedFrgn::Bridge { language, param_paths, return_path, .. }) => {
                self.emit_bridge_frgn_call(out, v, sig, args, &language, &param_paths, &return_path, indent)
            }
            Some(crate::analysis::frgn_dispatch::ResolvedFrgn::Unsupported(msg)) => {
                // 2026-07-22: Return a zero-value for the return type.
                // The error message is logged as a backend warning.
                self.warnings.push(format!("frgn '{}' unsupported: {}", sig.name, msg));
                let ret_type = sig.result_type.return_type().unwrap_or(Type::int());
                if ret_type != Type::Void {
                    let ret_llvm = self.llvm_ret_abi_type(&ret_type);
                    if ret_llvm == "ptr" {
                        writeln!(out, "{}  {} = inttoptr i64 0 to ptr", indent, v).ok();
                    } else if ret_llvm == "float" {
                        writeln!(out, "{}  {} = fadd float 0.0, 0.0", indent, v).ok();
                    } else if ret_llvm == "double" {
                        writeln!(out, "{}  {} = fadd double 0.0, 0.0", indent, v).ok();
                    } else if ret_llvm.starts_with('i') && ret_llvm.len() > 1 {
                        // 2026-08-10: zero at the return type's actual width
                        // (i8 for Bool, i32/i64 for ints) — the hardcoded i64
                        // broke `ret i8 <i64 reg>` on any target where the
                        // return width is narrower.
                        writeln!(out, "{}  {} = add {} 0, 0", indent, v, ret_llvm).ok();
                    } else {
                        writeln!(out, "{}  {} = add i64 0, 0", indent, v).ok();
                    }
                    TypedRegister { name: v.to_string(), ty: ret_type }
                } else {
                    TypedRegister { name: v.to_string(), ty: Type::Void }
                }
            }
            None => {
                // 2026-07-22: Legacy path — no dispatch resolution available.
                self.emit_direct_frgn_call(out, v, &sig.name, sig, args, indent)
            }
        }
    }

    /// 2026-07-27: Coerce argument LLVM type to match declared parameter type.
    /// Emits the appropriate LLVM cast instruction (fptosi, sitofp, trunc, zext,
    /// bitcast) when the argument's SSA type differs from the parameter's LLVM type.
    /// This prevents ABI mismatches like `call i64 @__print_int(float %x)`.
    fn coerce_to_param_type(
        &mut self,
        out: &mut String,
        arg_reg: &TypedRegister,
        param_llvm_ty: &str,
        indent: &str,
    ) -> TypedRegister {
        // 2026-08-01 (C3): a boxed Float param (i64 handle boxed at defn entry)
        // has its native float cached (reg_float_cache maps boxed→native) —
        // llvm_type() reports "float" from the briev type, so src_llvm == param
        // would early-return the i64 handle. Unbox through the cache FIRST.
        if matches!(param_llvm_ty, "float" | "double") {
            if let Some(cached) = self.fun.reg_float_cache.get(&arg_reg.name) {
                return TypedRegister {
                    name: cached.clone(),
                    ty: if param_llvm_ty == "float" { Type::float() } else { Type::float64() },
                };
            }
        }
        let src_llvm = self.llvm_type(&arg_reg.ty);
        if src_llvm == param_llvm_ty {
            return arg_reg.clone();
        }
        let result = self.fun.gen_reg();
        match (src_llvm.as_str(), param_llvm_ty) {
            // float → i64: fptosi — semantic float-to-int, not bitcast
            ("float", "i64") => {
                writeln!(out, "{}  {} = fptosi float {} to i64", indent, result, arg_reg.name).ok();
                TypedRegister { name: result, ty: Type::int() }
            }
            // double → i64: fptosi — semantic float-to-int, not bitcast
            ("double", "i64") => {
                writeln!(out, "{}  {} = fptosi double {} to i64", indent, result, arg_reg.name).ok();
                TypedRegister { name: result, ty: Type::int() }
            }
            // i64 → float: sitofp — semantic int-to-float, not bitcast
            ("i64", "float") => {
                writeln!(out, "{}  {} = sitofp i64 {} to float", indent, result, arg_reg.name).ok();
                TypedRegister { name: result, ty: Type::float() }
            }
            // i64 → double: sitofp — semantic int-to-float, not bitcast
            ("i64", "double") => {
                writeln!(out, "{}  {} = sitofp i64 {} to double", indent, result, arg_reg.name).ok();
                TypedRegister { name: result, ty: Type::float64() }
            }
            // float ↔ double: fpext/fptrunc
            ("float", "double") => {
                writeln!(out, "{}  {} = fpext float {} to double", indent, result, arg_reg.name).ok();
                TypedRegister { name: result, ty: Type::float64() }
            }
            ("double", "float") => {
                writeln!(out, "{}  {} = fptrunc double {} to float", indent, result, arg_reg.name).ok();
                TypedRegister { name: result, ty: Type::float() }
            }
            // ptr ↔ i64: inttoptr/ptrtoint
            ("i64", "ptr") => {
                writeln!(out, "{}  {} = inttoptr i64 {} to ptr", indent, result, arg_reg.name).ok();
                TypedRegister { name: result, ty: arg_reg.ty.clone() }
            }
            ("ptr", "i64") => {
                // 2026-07-30: Ptr values are stored as i64 internally (ptrtoint
                // at function entry). The register is already i64 — no conversion
                // needed. The Briev type says Ptr but the LLVM value is i64.
                TypedRegister { name: arg_reg.name.clone(), ty: Type::int() }
            }
            // Integer widening: i8/i16/i32 → i64 (zext for unsigned, sext for signed)
            (src, "i64") if src.starts_with('i') && src.len() > 1 => {
                let bits: u32 = src[1..].parse().unwrap_or(64);
                if bits < 64 {
                    writeln!(out, "{}  {} = zext {} {} to i64", indent, result, src, arg_reg.name).ok();
                    TypedRegister { name: result, ty: Type::int() }
                } else {
                    arg_reg.clone()
                }
            }
            // Integer narrowing: i64 → iN
            ("i64", dst) if dst.starts_with('i') && dst.len() > 1 => {
                writeln!(out, "{}  {} = trunc i64 {} to {}", indent, result, arg_reg.name, dst).ok();
                TypedRegister { name: result, ty: arg_reg.ty.clone() }
            }
            // Fallback: use a bitcast (may still be wrong, but preserves compilation)
            _ => {
                writeln!(out, "{}  {} = bitcast {} {} to {}", indent, result, src_llvm, arg_reg.name, param_llvm_ty).ok();
                TypedRegister { name: result, ty: arg_reg.ty.clone() }
            }
        }
    }

    /// 2026-07-22: Emit a direct foreign function call (Inline path).
    /// Uses the `symbol` parameter (from `as_name` or briev_name) as the
    /// callee, applies meld extension conversion to arguments, and emits
    /// the LLVM `call` instruction.
    fn emit_direct_frgn_call(
        &mut self,
        out: &mut String,
        v: &str,
        symbol: &str,
        sig: &crate::ast::ForeignSignature,
        args: &[Expr],
        indent: &str,
    ) -> TypedRegister {
        let arg_regs: Vec<TypedRegister> = args
            .iter()
            .map(|a| self.emit_expr(out, a, indent))
            .collect();
        let ext = sig.from.extension();
        let ext_str = ext.as_deref().unwrap_or("");
        // 2026-07-16: Apply meld forward on each arg (identity conversion for now)
        let meld_args: Vec<TypedRegister> = if ext_str.is_empty() {
            arg_regs
        } else {
            arg_regs
                .iter()
                .zip(sig.inputs.iter())
                .map(|(arg, (_, param_ty))| {
                    let ty_name = match param_ty {
                        crate::ast::Type::Custom(name) => name.as_str(),
                        _ => return arg.clone(),
                    };
                    // 2026-08-01 (B4): the SSO String→C i8* shim was retired —
                    // a String IS a ptr to [len][bytes] under the bits model,
                    // so the arg is already pointer-typed and needs no handle
                    // extraction.
                    // 2026-08-09 (Phase 12, SPEC §19.6): the meld lookup was a
                    // no-op (both branches returned the arg unchanged) — removed.
                    let _ = ty_name;
                    arg.clone()
                })
                .collect()
        };
        // 2026-07-24: Convert i64 args to ptr when the frgn param expects Ptr.
        // This handles PyModule_Create2(&moduledef, ...) where &moduledef returns
        // an i64 address but the C function expects a pointer parameter.
        // 2026-08-10: also covers String/Data params — a boxed String/Data arg
        // is an i64 [len][bytes] address in SSA (boxed to Type::int() at defn
        // entry), so a ptr-typed frgn param needs `inttoptr i64`. On wasm32
        // `llvm_type(Int)` is i32, so coerce_to_param_type's ("i64","ptr") arm
        // never fires for boxed values — this explicit inttoptr is the fix.
        let final_args: Vec<TypedRegister> = meld_args
            .iter()
            .zip(sig.inputs.iter())
            .map(|(arg, (_, param_ty))| {
                let param_is_ptr = matches!(param_ty, Type::Ptr(_))
                    || self.is_string_operand(param_ty)
                    || self.is_blob_operand(param_ty);
                if param_is_ptr && arg.ty == Type::int() {
                    let ptr_reg = self.fun.gen_reg();
                    writeln!(out, "{}  {} = inttoptr i64 {} to ptr", indent, ptr_reg, arg.name).ok();
                    TypedRegister {
                        name: ptr_reg,
                        ty: Type::Ptr(Box::new(Type::int())),
                    }
                } else {
                    arg.clone()
                }
            })
            .collect();
        // 2026-07-27: Coerce each argument to match the declared parameter type.
        // Previously used arg's own LLVM type (via self.llvm_type(&reg.ty)), which
        // produced ABI mismatches for float↔int, ptr↔i64, and integer width differences.
        // The frgn declaration specifies the expected C type; we must cast to match.
        let arg_strs: Vec<String> = final_args
            .iter()
            .zip(sig.inputs.iter())
            .map(|(arg, (_, param_ty))| {
                let param_llvm = self.llvm_type(param_ty);
                let coerced = self.coerce_to_param_type(out, arg, &param_llvm, indent);
                format!("{} {}", param_llvm, coerced.name)
            })
            .collect();
        let ret_type = sig.result_type.return_type().unwrap_or(Type::int());
        // 2026-07-19: Void-returning functions must not have a name assignment.
        let ret_llvm = self.llvm_ret_abi_type(&ret_type);
        if ret_type == Type::Void {
            writeln!(
                out,
                "{}call {} @{}({})",
                indent,
                ret_llvm,
                symbol,
                arg_strs.join(", ")
            )
            .ok();
            TypedRegister {
                name: v.to_string(),
                ty: ret_type,
            }
        } else {
            writeln!(
                out,
                "{} {} = call {} @{}({})",
                indent,
                v,
                ret_llvm,
                symbol,
                arg_strs.join(", ")
            )
            .ok();
            TypedRegister {
                name: v.to_string(),
                ty: ret_type,
            }
        }
    }

    /// 2026-07-22: Emit a GLUE bridge foreign call (Bridge path).
    /// Applies protocol transforms to arguments, calls the foreign function
    /// through its bridge mechanism, transforms the return value, and
    /// wraps with fallback dispatch.
    fn emit_bridge_frgn_call(
        &mut self,
        out: &mut String,
        v: &str,
        sig: &crate::ast::ForeignSignature,
        args: &[Expr],
        _language: &str,
        param_paths: &[crate::analysis::frgn_dispatch::ProtocolStep],
        return_path: &Option<crate::analysis::frgn_dispatch::ProtocolStep>,
        indent: &str,
    ) -> TypedRegister {
        // 2026-07-22: Emit argument expressions and apply protocol transforms.
        let mut transformed_args: Vec<TypedRegister> = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            let reg = self.emit_expr(out, arg, indent);
            let path_for_arg = param_paths.get(i).map(|p| std::slice::from_ref(p)).unwrap_or(&[]);
            let result_reg = crate::glue::bridge::emit_protocol_chain(
                out, &reg.name, path_for_arg, &self.llvm_type(&reg.ty),
                &mut || self.fun.gen_reg(),
            ).unwrap_or_else(|_| reg.name.clone());
            transformed_args.push(TypedRegister {
                name: result_reg,
                ty: reg.ty.clone(),
            });
        }

        // 2026-07-22: Emit the foreign call through the bridge.
        // For now, emit a direct call with a bridge_ prefix to distinguish.
        // 2026-07-24: Convert i64 args to ptr when the frgn param expects Ptr.
        let bridge_args: Vec<TypedRegister> = transformed_args
            .iter()
            .zip(sig.inputs.iter())
            .map(|(arg, (_, param_ty))| {
                if matches!(param_ty, Type::Ptr(_)) && arg.ty == Type::int() {
                    let ptr_reg = self.fun.gen_reg();
                    writeln!(out, "{}  {} = inttoptr i64 {} to ptr", indent, ptr_reg, arg.name).ok();
                    TypedRegister {
                        name: ptr_reg,
                        ty: Type::Ptr(Box::new(Type::int())),
                    }
                } else {
                    arg.clone()
                }
            })
            .collect();
        let arg_strs: Vec<String> = bridge_args
            .iter()
            .map(|reg| format!("{} {}", self.llvm_type(&reg.ty), reg.name))
            .collect();
        let ret_type = sig.result_type.return_type().unwrap_or(Type::int());
        let ret_llvm = self.llvm_ret_abi_type(&ret_type);
        let bridge_name = format!("bridge_{}", sig.name);

        if ret_type == Type::Void {
            writeln!(
                out,
                "{}call {} @{}({})",
                indent, ret_llvm, bridge_name, arg_strs.join(", ")
            ).ok();
        } else {
            writeln!(
                out,
                "{} {} = call {} @{}({})",
                indent, v, ret_llvm, bridge_name, arg_strs.join(", ")
            ).ok();
        }

        // 2026-07-22: Transform return value back to Briev type.
        let final_reg = if let Some(ret_path) = return_path {
            crate::glue::bridge::emit_protocol_chain(
                out, v, std::slice::from_ref(ret_path), &ret_llvm,
                &mut || self.fun.gen_reg(),
            ).unwrap_or_else(|_| v.to_string())
        } else {
            v.to_string()
        };

        // 2026-08-09 (Phase 12, SPEC §19.3): the `fallback` dispatch phi is
        // removed — fallback behavior uses ordinary typed control flow. The
        // call result is used directly.
        TypedRegister {
            name: final_reg,
            ty: ret_type,
        }
    }

    /// 2026-08-09 (Phase 10): `spawn defn(args)` — a TASK spawn. The reference
    /// semantic scheduler is deterministic: the task runs to completion, and
    /// its result register IS the linear handle. `await`/`free`/`keep` then
    /// operate on that handle (the handle's liveness is checked by the
    /// typechecker/ownership analysis — a silently dropped live handle errors).
    pub(super) fn emit_task_spawn(
        &mut self,
        out: &mut String,
        indent: &str,
        name: &str,
        args: &[Expr],
    ) -> TypedRegister {
        let v = self.fun.gen_reg();
        self.emit_user_call(out, &v, name, args, indent)
    }

    /// Emit a user function call.
    /// 2026-07-17: defn functions expect (ptr %state, ...) as their first parameter.
    /// We must prepend the state pointer and adapt argument types from register
    /// types to the function's parameter types (via defn_params).
    pub(super) fn emit_user_call(
        &mut self,
        out: &mut String,
        v: &str,
        name: &str,
        args: &[Expr],
        indent: &str,
    ) -> TypedRegister {
        // 2026-08-06 (fix): a closure-let call goes INDIRECT through its env
        // block — load the value (env address), then the fn_ptr, then call it
        // with the env as the first (hidden) parameter. This makes the closure
        // a real first-class value (it can be passed around), replacing the
        // inline-at-call-site lowering.
        // 2026-08-14 (stdlib-cleanup): a FUNCTION-TYPED PARAM (`f: T -> U`) is
        // a closure value received from the callee — its slot holds the env
        // block address, so calls to it go indirect too. Previously a bare
        // `call @f(...)` referenced an undefined symbol (broken IR) and
        // flattened the result to Int.
        if self.fun.closure_lets.contains_key(name) {
            return self.emit_closure_indirect_call(out, name, args, indent);
        }
        let is_fn_param = self
            .fun
            .let_original_types
            .get(name)
            .or_else(|| self.fun.let_binding_types.get(name))
            .map(|t| matches!(t, Type::Function(_, _)))
            .unwrap_or(false);
        if is_fn_param {
            return self.emit_closure_indirect_call(out, name, args, indent);
        }
        // 2026-07-16: P5 — Check if this is a foreign function; if so, use emit_frgn_call
        // Clone the sig to avoid borrowing self.ctx while self.emit_expr needs &mut self.
        let frgn_sig = self.ctx.frgn_map.get(name).cloned();
        if let Some(sig) = frgn_sig {
            return self.emit_frgn_call(out, v, &sig, args, indent);
        }
        // 2026-07-14: collect typed registers so call includes argument types
        // 2026-08-14 (Iterable protocol, slice-6 literal args): a List-literal
        // argument (`iter_map([1,2,3], f)`) whose declared param type is a
        // Tier-2 collection MUST construct through the collection's own ops
        // (`op Init`/`op InsertAt`) — never the stale `[len][elems]` heap-seq
        // layout from emit_heap_seq. The adapter reads it back via `op Count`/
        // `op At`, which expect `[data@0, cap@1, len@2]`; a heap-seq literal
        // segfaulted in iter_map_loop (length read from slot 0 = data ptr).
        // Mirrors the typed-local-let path (emit_stmt.rs:363-377).
        let arg_regs: Vec<TypedRegister> = {
            let fn_params = self.ctx.defn_params.get(name).cloned();
            args.iter()
                .enumerate()
                .map(|(i, a)| {
                    if let Some(param_tys) = &fn_params {
                        if let Some(pt) = param_tys.get(i) {
                            if matches!(a, crate::ast::Expr::List(_))
                                && self.tier2_collection_type(pt)
                            {
                                if let Some(reg) = self.construct_local_collection(
                                    out, indent, pt, a,
                                ) {
                                    return reg;
                                }
                            }
                        }
                    }
                    self.emit_expr(out, a, indent)
                })
                .collect()
        };
        // 2026-07-17: Look up defn parameter types for type adaptation.
        let defn_param_tys = self.ctx.defn_params.get(name).cloned();
        let is_defn = defn_param_tys.is_some();
        let mut call_args: Vec<String> = Vec::new();
        if is_defn {
            call_args.push("ptr %state".to_string());
            let param_tys = defn_param_tys.as_ref().unwrap();
            for (i, reg) in arg_regs.iter().enumerate() {
                let reg_llvm_ty = self.llvm_type(&reg.ty);
                // 2026-07-17: Get the function's expected parameter type.
                // If available, use llvm_type() to determine the expected
                // LLVM type and insert conversions (i64 → ptr for String/Data).
                let param_llvm_ty = param_tys
                    .get(i)
                    .map(|pt| self.llvm_type(pt))
                    .unwrap_or_else(|| reg_llvm_ty.to_string());
                // 2026-07-30: Ptr values are stored as i64 internally (ptrtoint at
                // function entry). Convert back to LLVM ptr when the function expects
                // ptr but the register's Briev type is Ptr (meaning it's an i64 handle).
                // 2026-08-14 (stdlib-cleanup): Type::Function is the same storage
                // convention — a closure VALUE is an i64 env-block handle, and a
                // fn-typed param is ptrtoint'd to i64 at defn entry, so marshaling
                // it into a later fn-typed param must inttoptr again (the 2026-08-03
                // host-callback ABI kept llvm_type(Function)="ptr").
                if param_llvm_ty == "ptr"
                    && (reg_llvm_ty == "i64"
                        || matches!(reg.ty, Type::Ptr(_))
                        || matches!(reg.ty, Type::Function(_, _)))
                {
                    let conv = self.fun.gen_reg();
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, conv, reg.name).ok();
                    call_args.push(format!("ptr {}", conv));
                } else if param_llvm_ty == "i64" && reg_llvm_ty == "ptr" {
                    let conv = self.fun.gen_reg();
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, conv, reg.name).ok();
                    call_args.push(format!("i64 {}", conv));
                } else {
                    call_args.push(format!("{} {}", reg_llvm_ty, reg.name));
                }
            }
        } else {
            for reg in &arg_regs {
                // 2026-07-30: Non-defn calls (or calls where defn not found in
                // defn_params) also need inttoptr for Ptr-typed arguments, since
                // the register holds the i64 internal representation.
                if matches!(reg.ty, Type::Ptr(_)) {
                    let conv = self.fun.gen_reg();
                    self.emit_inttoptr(out, indent, &conv, &reg.name);
                    call_args.push(format!("ptr {}", conv));
                } else {
                    call_args.push(format!("{} {}", self.llvm_type(&reg.ty), reg.name));
                }
            }
        }
        // 2026-07-14: user call return type from defn_return_types — fall back to i64
        let ret_type = self
            .ctx
            .defn_return_types
            .get(name)
            .and_then(|types| types.first().cloned())
            .unwrap_or(Type::int());
        let ret_llvm = self.llvm_ret_abi_type(&ret_type);
        // 2026-08-05 (Phase 6): there is no `main` in Briev — no call-site
        // renaming to `briev_main`; the symbol is the declaration name.
        let symbol = name;
        writeln!(
            out,
            "{}{} = call {} @{}({})",
            indent,
            v,
            ret_llvm,
            symbol,
            call_args.join(", ")
        )
        .ok();
        TypedRegister {
            name: v.to_string(),
            ty: ret_type,
        }
    }

    /// 2026-08-06 (Phase 8): inline a let-bound closure call. Args evaluate in
    /// the caller scope; each param name is bound to its arg register in
    /// `last_val_temps` (checked first by Identifier resolution), the body
    /// emits, then the prior bindings restore. Captured free variables resolve
    /// from the enclosing function scope — by-value for immutable let-bound
    /// SSA registers, matching the interpreter's closure semantics.
    /// 2026-08-06 (fix): a closure call loads the value (the env-block
    /// address), then the fn_ptr from slot 0, and calls it indirectly with the
    /// env as the hidden first parameter. Uniform for every closure value —
    /// whether called by name or passed around and called through a parameter.
    /// 2026-08-14 (stdlib-cleanup): a FUNCTION-TYPED PARAM (`f: T -> U` in a
    /// defn/txn) is a closure value too — its binding is a param slot holding
    /// the env-block address, which must be LOADED from the slot (identifiers
    /// already do this; a bare binding register is only a let-bound closure's
    /// direct env address). The returned register carries the closure's
    /// DECLARED return type from the binding (`Type::Function(_, ret)`), so a
    /// downstream `.Count#()` on `f(x)` dispatches (e.g. `iter_flatmap_loop`'s
    /// `mapped.Count#()`); previously the result was flattened to Int and
    /// method dispatch panicked with "no member on ''".
    fn emit_closure_indirect_call(
        &mut self,
        out: &mut String,
        name: &str,
        args: &[Expr],
        indent: &str,
    ) -> TypedRegister {
        let env_val = if let Some(reg) = self.fun.let_bindings.get(name).cloned() {
            if self.fun.param_slots.values().any(|s| s == &reg)
                || self.fun.let_binding_allocas.contains(&reg)
            {
                let loaded = self.fun.gen_reg();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, loaded, reg).ok();
                loaded
            } else {
                reg
            }
        } else {
            self.resolve_name_register(name)
        };
        let env_p = self.fun.gen_reg();
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, env_p, env_val).ok();
        let fp = self.fun.gen_reg();
        writeln!(out, "{}{} = load i64, ptr {}", indent, fp, env_p).ok();
        let fp_p = self.fun.gen_reg();
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, fp_p, fp).ok();
        let arg_regs: Vec<String> = args
            .iter()
            .map(|a| {
                let r = self.emit_expr(out, a, indent);
                format!("i64 {}", r.name)
            })
            .collect();
        let result = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = call i64 {}(ptr {}, {})",
            indent,
            result,
            fp_p,
            env_p,
            arg_regs.join(", ")
        )
        .ok();
        let ret_ty = self
            .fun
            .let_original_types
            .get(name)
            .or_else(|| self.fun.let_binding_types.get(name))
            .and_then(|t| match t {
                Type::Function(_, ret) => Some((**ret).clone()),
                _ => None,
            })
            .unwrap_or(Type::int());
        // 2026-08-14 (stdlib-cleanup): the closure ABI boxes EVERY return to
        // i64. A Bool-returning closure (`T -> Bool`, e.g. iter_filter's pred)
        // therefore holds a 0/1 in an i64 register — narrow to i8 so guards
        // (`trunc i8 ... to i1`) and `when` conditions type-check instead of
        // truncating an i64 (clang error). Mirrors the i64→i8→i64 boxing of a
        // Bool PARAMETER at function entry.
        if ret_ty == Type::bool_() {
            let narrowed = self.fun.gen_reg();
            writeln!(out, "{}{} = trunc i64 {} to i8", indent, narrowed, result).ok();
            TypedRegister { name: narrowed, ty: ret_ty }
        } else {
            TypedRegister { name: result, ty: ret_ty }
        }
    }

        /// 2026-08-06 (Phase 7): lower a match expression. The scrutinee is
    /// evaluated once; each arm's pattern condition branches to the arm block
    /// or the next arm; the arm block binds pattern names, evaluates the
    /// `when` guard, and emits the body; the results merge at a phi. The
    /// no-match path (after the last arm) contributes a default 0.
    fn emit_match(
        &mut self,
        out: &mut String,
        v: &str,
        scrutinee: &Expr,
        arms: &[crate::ast::MatchArm],
        indent: &str,
    ) -> TypedRegister {
        let scrut = self.emit_expr(out, scrutinee, indent);
        let counter = self.fun.txn_counter;
        self.fun.txn_counter += 1;
        let end_label = format!(".match_end_{}", counter);
        let n = arms.len();
        let mut arm_labels: Vec<String> = Vec::with_capacity(n);
        let mut next_labels: Vec<String> = Vec::with_capacity(n);
        // Phase 1: the condition chain — each arm's condition branches to the
        // arm block or the next arm's condition.
        for (i, arm) in arms.iter().enumerate() {
            let arm_label = format!(".match_arm_{}_{}", counter, i);
            let next_label = format!(".match_next_{}_{}", counter, i);
            let cond = self.emit_pattern_condition(&arm.pattern, &scrut.name, out, indent);
            writeln!(out, "  br i1 {}, label %{}, label %{}", cond, arm_label, next_label).ok();
            writeln!(out, "{}:", next_label).ok();
            arm_labels.push(arm_label);
            next_labels.push(next_label);
        }
        // No arm matched — default to 0 and merge at the end.
        writeln!(out, "  br label %{}", end_label).ok();
        // Phase 2: the arm blocks.
        let mut phi_incoming: Vec<(String, String)> = Vec::with_capacity(n);
        for (i, arm) in arms.iter().enumerate() {
            writeln!(out, "{}:", arm_labels[i]).ok();
            self.bind_pattern(&arm.pattern, &scrut.name, out, indent);
            let body_reg;
            let body_block_label;
            if let Some(guard) = &arm.guard {
                let gv = self.emit_expr(out, guard, indent);
                // Briev bool comparisons emit i8 (0/1) — narrow to i1 for `br`.
                let g1 = self.fun.gen_reg();
                writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, g1, gv.name).ok();
                let body_label = format!(".match_guard_{}_{}", counter, i);
                // Guard false falls into the NEXT arm's condition block —
                // `next_labels[i]` is where arm i+1's condition begins (for
                // the last arm it is the no-match default to the end).
                writeln!(out, "  br i1 {}, label %{}, label %{}", g1, body_label, next_labels[i]).ok();
                writeln!(out, "{}:", body_label).ok();
                body_reg = self.emit_expr(out, &arm.body, indent);
                // The body lives in the guard block — that block branches to
                // the end, so the phi edge must come from it.
                body_block_label = body_label;
            } else {
                body_reg = self.emit_expr(out, &arm.body, indent);
                body_block_label = arm_labels[i].clone();
            }
            phi_incoming.push((body_reg.name.clone(), body_block_label));
            writeln!(out, "  br label %{}", end_label).ok();
        }
        // Phase 3: the end block merges the arm results (and the default).
        writeln!(out, "{}:", end_label).ok();
        let mut phi = format!("  {} = phi i64 [ 0, %{} ]", v, next_labels[n - 1]);
        for (reg, label) in phi_incoming {
            phi.push_str(&format!(", [ {}, %{} ]", reg, label));
        }
        writeln!(out, "{}", phi).ok();
        TypedRegister { name: v.to_string(), ty: Type::int() }
    }

    /// Emit the i1 condition a pattern matches the scrutinee register.
    /// Unimplemented patterns (Tuple/EnumVariant) emit `false` — the arm is
    /// never taken (documented boundary; the interpreter handles all forms).
    fn emit_pattern_condition(
        &mut self,
        pat: &crate::ast::Pattern,
        scrut: &str,
        out: &mut String,
        indent: &str,
    ) -> String {
        match pat {
            crate::ast::Pattern::Wildcard | crate::ast::Pattern::Binding(_) => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = icmp eq i64 0, 0", indent, r).ok();
                r
            }
            crate::ast::Pattern::Literal(lit) => {
                let lv = self.emit_pattern_literal(out, lit, indent);
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, r, scrut, lv).ok();
                r
            }
            crate::ast::Pattern::Range(start, end)
            | crate::ast::Pattern::RangeInclusive(start, end) => {
                let ls = self.emit_pattern_literal(out, start, indent);
                let le = self.emit_pattern_literal(out, end, indent);
                let inclusive = matches!(pat, crate::ast::Pattern::RangeInclusive(_, _));
                let ge = self.fun.gen_reg();
                writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, ge, scrut, ls).ok();
                let cmp = self.fun.gen_reg();
                writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, cmp,
                    if inclusive { "sle" } else { "slt" }, scrut, le).ok();
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = and i1 {}, {}", indent, r, ge, cmp).ok();
                r
            }
            _ => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = icmp eq i64 1, 0", indent, r).ok();
                r
            }
        }
    }

    /// Emit a literal pattern's integer value register.
    fn emit_pattern_literal(&mut self, out: &mut String, lit: &Expr, indent: &str) -> String {
        match lit {
            Expr::Decimal(n) => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 0, {}", indent, r, n).ok();
                r
            }
            Expr::Bool(b) => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 0, {}", indent, r, if *b { 1 } else { 0 }).ok();
                r
            }
            Expr::Float(f) => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = fptosi double {} to i64", indent, r, f).ok();
                r
            }
            _ => {
                let r = self.fun.gen_reg();
                writeln!(out, "{}{} = add i64 0, 0", indent, r).ok();
                r
            }
        }
    }

    /// Bind a pattern's Binding names to the scrutinee register (or sub-values)
    /// so the arm body resolves them. Tuple/EnumVariant bindings are not
    /// lowered (their conditions emit false, so the block is unreachable).
    fn bind_pattern(
        &mut self,
        pat: &crate::ast::Pattern,
        scrut: &str,
        out: &mut String,
        indent: &str,
    ) {
        if let crate::ast::Pattern::Binding(name) = pat {
            self.fun.last_val_temps.insert(name.clone(), scrut.to_string());
            self.fun.last_val_types.insert(name.clone(), Type::int());
            self.fun.let_bindings.insert(name.clone(), scrut.to_string());
            self.fun.let_binding_types.insert(name.clone(), Type::int());
            self.fun.let_original_types.insert(name.clone(), Type::int());
        }
        let _ = (out, indent);
    }

    /// Resolve the current register for a name bound by `let` (or a pending
    /// last-value temp). Used to load a closure's env-address value.
    fn resolve_name_register(&self, name: &str) -> String {        self.fun
            .last_val_temps
            .get(name)
            .cloned()
            .or_else(|| self.fun.let_bindings.get(name).cloned())
            .unwrap_or_else(|| {
                panic!("closure '{}' has no binding register", name)
            })
    }

    /// 2026-07-25: Return the integer type for binary operations based on
    /// the function's narrowed max width, or "i64" if no narrowing applies.
    /// 2026-07-25: Always return i64 for integer binary operations.
    /// Intermediate SSA values match the target's native integer width.
    /// 2026-07-29: Use self.ctx.int_bits instead of hardcoded i64 for
    /// cross-target correctness (wasm32 uses i32 intermediate values).
    fn binop_int_type(&self) -> String {
        format!("i{}", self.ctx.int_bits)
    }

    /// 2026-07-18: Try to emit a binary operation from config/llvm-ops.toml.
    /// Returns Some(reg) if the config had a matching template, None to fall
    /// through to the hardcoded match arms in emit_binary_op.
    fn emit_binop_from_config(
        &mut self,
        out: &mut String,
        v: &str,
        kind: &BinaryOpKind,
        l: &TypedRegister,
        r: &TypedRegister,
        indent: &str,
        ret_ty: &Type,
    ) -> Option<TypedRegister> {
        let op_name = match kind {
            BinaryOpKind::Add => "Add",
            BinaryOpKind::Sub => "Sub",
            BinaryOpKind::Mul => "Mul",
            BinaryOpKind::Div => "Div",
            BinaryOpKind::Mod => "Rem",
            BinaryOpKind::Eq => "Eq",
            BinaryOpKind::Neq => "Neq",
            BinaryOpKind::Lt => "Lt",
            BinaryOpKind::Gt => "Gt",
            BinaryOpKind::Ge => "Ge",
            BinaryOpKind::And => "And",
            BinaryOpKind::Or => "Or",
            BinaryOpKind::Shl => "Shl",
            BinaryOpKind::Shr => "Shr",
            BinaryOpKind::BitAnd => "BitAnd",
            BinaryOpKind::BitOr => "BitOr",
            BinaryOpKind::BitXor => "BitXor",
            // 2026-07-22: Concat is handled by emit_inline_concat directly
            // (not by config templates). Short-circuit here to avoid the
            // unnecessary llvm_type + template_for_op lookup.
            BinaryOpKind::Concat => return None,
            _ => return None,
        };
        // 2026-08-01 (B1): #String operands NEVER go through the config
        // template path. Their flexible primordial has bytes=0, so the
        // integer template derivation produces `i0` (invalid IR), and more
        // fundamentally Eq/Ne on Strings is a CONTENT comparison handled by
        // the dedicated arm in emit_binary_op (briev_str_eq). Returning None
        // here routes String ops to that arm.
        if self.is_string_operand(&l.ty)
            || self.is_string_operand(&r.ty)
        {
            return None;
        }
        // Get the llvm_type of the LHS operand to drive template dispatch
        let llvm_ty = self.llvm_type(&l.ty);
        // Get byte width from the universe if available
        let bytes = self
            .ctx
            .type_universe
            .as_ref()
            .and_then(|u| l.ty.universe_key().and_then(|k| u.get(k)))
            .map(|rt| rt.bytes)
            .unwrap_or(8u64);
        let tmpl = template_for_op(op_name, &llvm_ty, bytes)?;
        let is_cmp = matches!(
            kind,
            BinaryOpKind::Eq
                | BinaryOpKind::Neq
                | BinaryOpKind::Lt
                | BinaryOpKind::Le
                | BinaryOpKind::Gt
                | BinaryOpKind::Ge
        );
        // 2026-07-20: Templates include "%v = " prefix — strip it since the
        // caller writes "{v} = {line}" (avoiding double "=").
        let bare = tmpl.replace("%v = ", "");
        let line = bare.replace("%a", &l.name).replace("%b", &r.name);
        if is_cmp {
            let icmp_reg = self.fun.gen_reg();
            writeln!(out, "{}{} = {}", indent, icmp_reg, line).ok();
            writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp_reg).ok();
            Some(TypedRegister {
                name: v.to_string(),
                ty: Type::bool_(),
            })
        } else {
            // 2026-07-30: Preserve Ptr type for pointer arithmetic so downstream
            // Deref can detect it needs inttoptr before load/store. Ptr values are
            // stored as i64 internally (ptrtoint at function entry), but the type
            // must remain Ptr so consumption sites know to convert back.
            let effective_ret_ty = if matches!(kind, BinaryOpKind::Add | BinaryOpKind::Sub) {
                if matches!(l.ty, Type::Ptr(_)) {
                    l.ty.clone()
                } else if matches!(r.ty, Type::Ptr(_)) {
                    r.ty.clone()
                } else {
                    ret_ty.clone()
                }
            } else {
                ret_ty.clone()
            };
            writeln!(out, "{}{} = {}", indent, v, line).ok();
            Some(TypedRegister {
                name: v.to_string(),
                ty: effective_ret_ty,
            })
        }
    }
 
    /// Emit a binary operation.
    fn emit_binary_op(
        &mut self,
        out: &mut String,
        v: &str,
        kind: &crate::ast::BinaryOpKind,
        l: &TypedRegister,
        r: &TypedRegister,
        indent: &str,
    ) -> TypedRegister {
        // 2026-07-26: SIMD vector ops — when both operands are Vector<T,N>,
        // emit LLVM vector instructions (<N x T>).
        if let Type::Vector(l_inner, l_dims) = &l.ty {
            if let Type::Vector(r_inner, r_dims) = &r.ty {
                if l_inner == r_inner && l_dims == r_dims {
                    let op_name = match kind {
                        crate::ast::BinaryOpKind::Add => "add",
                        crate::ast::BinaryOpKind::Sub => "sub",
                        crate::ast::BinaryOpKind::Mul => "mul",
                        crate::ast::BinaryOpKind::Div => "sdiv",
                        _ => { return TypedRegister { name: v.to_string(), ty: l.ty.clone() }; }
                    };
                    let vec_ty = self.llvm_type(&l.ty);
                    writeln!(out, "{}{} = {} {} {}, {}",
                        indent, v, op_name, vec_ty, l.name, r.name).ok();
                    return TypedRegister { name: v.to_string(), ty: l.ty.clone() };
                    return TypedRegister { name: v.to_string(), ty: l.ty.clone() };
                }
            }
        }
        let is_float = l.ty == Type::float()
            || r.ty == Type::float()
            || l.ty == Type::float64()
            || r.ty == Type::float64();
        let is_double = l.ty == Type::float64() || r.ty == Type::float64();
        // 2026-07-17: Correct float type width — use "float" for Float (32-bit)
        // and "double" for Float64 (64-bit). The old code always used "double"
        // for all float operations, producing invalid IR when operands were
        // actually 32-bit float values loaded from constants or state fields.
        let int_ty = self.binop_int_type();
        let ty_str = if is_double {
            "double"
        } else if is_float {
            "float"
        } else {
            &int_ty
        };
        let fast = if is_float { " fast" } else { "" };
        let mut ret_ty = if is_double {
            Type::float64()
        } else if is_float {
            Type::float()
        } else {
            Type::int()
        };
        // 2026-07-18: Phase 0 — try config-driven dispatch first.
        // The OP_CONFIG maps (op_name, type_name, bytes) → LLVM IR template.
        // If the config has an entry, fill the template and skip hardcoded matches.
        if let Some(reg) = self.emit_binop_from_config(out, v, kind, l, r, indent, &ret_ty) {
            return reg;
        }
        match kind {
            crate::ast::BinaryOpKind::Add => {
                // 2026-08-03: `+` is string concat for #String/#Blob operands
                // (the `++`/Concat operation; + reads naturally and resolves
                // to the same concat binding in the typechecker).
                if self.is_string_operand(&l.ty) || self.is_string_operand(&r.ty) {
                    return self.emit_inline_concat(out, indent, &l, &r);
                }
                // 2026-07-17: Pointer-offset arithmetic: `buf + N` emits GEP.
                // When one operand is Ptr<T> and the other is Int, emit:
                //   %gep = getelementptr T, ptr %ptr, i64 %offset
                // preserving the pointer type for subsequent dereference.
                if matches!(l.ty, Type::Ptr(_)) && !is_float {
                    let ptr_ty = match &l.ty {
                        Type::Ptr(i) => *i.clone(),
                        _ => Type::int(),
                    };
                    writeln!(
                        out,
                        "{}{} = getelementptr {}, ptr {}, i64 {}",
                        indent,
                        v,
                        self.llvm_type(&ptr_ty),
                        l.name,
                        r.name
                    )
                    .ok();
                    ret_ty = l.ty.clone();
                } else if matches!(r.ty, Type::Ptr(_)) && !is_float {
                    let ptr_ty = match &r.ty {
                        Type::Ptr(i) => *i.clone(),
                        _ => Type::int(),
                    };
                    writeln!(
                        out,
                        "{}{} = getelementptr {}, ptr {}, i64 {}",
                        indent,
                        v,
                        self.llvm_type(&ptr_ty),
                        r.name,
                        l.name
                    )
                    .ok();
                    ret_ty = r.ty.clone();
                } else if is_float {
                    writeln!(
                        out,
                        "{}{} = fadd{} {} {}, {}",
                        indent, v, fast, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    let op_bits = self.binop_int_type();
                    writeln!(out, "{}{} = add nuw nsw {} {}, {}", indent, v, op_bits, l.name, r.name).ok();
                }
                TypedRegister {
                    name: v.to_string(),
                    ty: ret_ty,
                }
            }
            crate::ast::BinaryOpKind::Sub => {
                // 2026-07-14: Sub must branch on is_float — fsub i64 is invalid LLVM IR
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fsub{} {} {}, {}",
                        indent, v, fast, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    // 2026-07-25: Use binop_int_type() so narrowing pass controls width.
                    // Was hardcoded i64 — broke WASM i32 by emitting sub nsw i64 on i32 registers.
                    let op_bits = self.binop_int_type();
                    writeln!(out, "{}{} = sub nsw {} {}, {}", indent, v, op_bits, l.name, r.name).ok();
                }
                TypedRegister {
                    name: v.to_string(),
                    ty: ret_ty,
                }
            }
            crate::ast::BinaryOpKind::Mul => {
                // 2026-07-14: Mul must branch on is_float — fmul i64 is invalid LLVM IR
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fmul{} {} {}, {}",
                        indent, v, fast, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    // 2026-07-25: Use binop_int_type() so narrowing pass controls width.
                    let op_bits = self.binop_int_type();
                    writeln!(out, "{}{} = mul nsw {} {}, {}", indent, v, op_bits, l.name, r.name).ok();
                }
                TypedRegister {
                    name: v.to_string(),
                    ty: ret_ty,
                }
            }
            crate::ast::BinaryOpKind::Div => {
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fdiv{} {} {}, {}",
                        indent, v, fast, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    // 2026-07-25: Use binop_int_type() so narrowing pass controls width.
                    let op_bits = self.binop_int_type();
                    writeln!(out, "{}{} = sdiv {} {}, {}", indent, v, op_bits, l.name, r.name).ok();
                }
                TypedRegister {
                    name: v.to_string(),
                    ty: ret_ty,
                }
            }
            crate::ast::BinaryOpKind::Mod => {
                // 2026-07-25: Use binop_int_type() so narrowing pass controls width.
                let op_bits = self.binop_int_type();
                writeln!(out, "{}{} = srem {} {}, {}", indent, v, op_bits, l.name, r.name).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: ret_ty.clone(),
                }
            }
            crate::ast::BinaryOpKind::Eq => {
                // 2026-08-01 (B1): String operands compare CONTENT, not
                // addresses. 2026-08-04 (compiler-in-Briev): fire when EITHER
                // operand is #String — the typechecker guarantees both are
                // strings, but the other may have been boxed to i64
                // (adapt_to_i64 loses the String type), so inttoptr it before
                // the content compare. Matches the interpreter's content Eq.
                let is_str = self.is_string_operand(&l.ty)
                    || self.is_string_operand(&r.ty);
                if is_str {
                    let eq = self.fun.gen_reg();
                    let lp = self.string_ptr(out, indent, l);
                    let rp = self.string_ptr(out, indent, r);
                    writeln!(out, "{}{} = call i64 @briev_str_eq(ptr {}, ptr {})", indent, eq, lp, rp).ok();
                    let icmp = self.fun.gen_reg();
                    writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, icmp, eq).ok();
                    writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                    return TypedRegister { name: v.to_string(), ty: Type::bool_() };
                }
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fcmp oeq {} {}, {}",
                        indent, icmp, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    let cmp_ty = self.llvm_type(&l.ty);
                    writeln!(
                        out,
                        "{}{} = icmp eq {} {}, {}",
                        indent, icmp, cmp_ty, l.name, r.name
                    )
                    .ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            crate::ast::BinaryOpKind::Neq => {
                // 2026-08-01 (B1): content inequality — mirrors the Eq arm.
                // 2026-08-04: either-operand + inttoptr, as in Eq.
                let is_str = self.is_string_operand(&l.ty)
                    || self.is_string_operand(&r.ty);
                if is_str {
                    let eq = self.fun.gen_reg();
                    let lp = self.string_ptr(out, indent, l);
                    let rp = self.string_ptr(out, indent, r);
                    writeln!(out, "{}{} = call i64 @briev_str_eq(ptr {}, ptr {})", indent, eq, lp, rp).ok();
                    let icmp = self.fun.gen_reg();
                    writeln!(out, "{}{} = icmp eq i64 {}, 0", indent, icmp, eq).ok();
                    writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                    return TypedRegister { name: v.to_string(), ty: Type::bool_() };
                }
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fcmp one {} {}, {}",
                        indent, icmp, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    let cmp_ty = self.llvm_type(&l.ty);
                    writeln!(
                        out,
                        "{}{} = icmp ne {} {}, {}",
                        indent, icmp, cmp_ty, l.name, r.name
                    )
                    .ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            crate::ast::BinaryOpKind::Lt => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fcmp olt {} {}, {}",
                        indent, icmp, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    let cmp_ty = self.llvm_type(&l.ty);
                    writeln!(
                        out,
                        "{}{} = icmp slt {} {}, {}",
                        indent, icmp, cmp_ty, l.name, r.name
                    )
                    .ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            crate::ast::BinaryOpKind::Le => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fcmp ole {} {}, {}",
                        indent, icmp, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    let cmp_ty = self.llvm_type(&l.ty);
                    writeln!(
                        out,
                        "{}{} = icmp sle {} {}, {}",
                        indent, icmp, cmp_ty, l.name, r.name
                    )
                    .ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            crate::ast::BinaryOpKind::Gt => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fcmp ogt {} {}, {}",
                        indent, icmp, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    let cmp_ty = self.llvm_type(&l.ty);
                    writeln!(
                        out,
                        "{}{} = icmp sgt {} {}, {}",
                        indent, icmp, cmp_ty, l.name, r.name
                    )
                    .ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            crate::ast::BinaryOpKind::Ge => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(
                        out,
                        "{}{} = fcmp oge {} {}, {}",
                        indent, icmp, ty_str, l.name, r.name
                    )
                    .ok();
                } else {
                    let cmp_ty = self.llvm_type(&l.ty);
                    writeln!(
                        out,
                        "{}{} = icmp sge {} {}, {}",
                        indent, icmp, cmp_ty, l.name, r.name
                    )
                    .ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            crate::ast::BinaryOpKind::And => {
                let cmp_ty = self.llvm_type(&l.ty);
                writeln!(
                    out,
                    "{}{} = and {} {}, {}",
                    indent, v, cmp_ty, l.name, r.name
                )
                .ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            crate::ast::BinaryOpKind::Or => {
                let cmp_ty = self.llvm_type(&l.ty);
                writeln!(
                    out,
                    "{}{} = or {} {}, {}",
                    indent, v, cmp_ty, l.name, r.name
                )
                .ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::bool_(),
                }
            }
            crate::ast::BinaryOpKind::Concat => {
                self.emit_inline_concat(out, indent, l, r)
            }
            crate::ast::BinaryOpKind::BitAnd | crate::ast::BinaryOpKind::BitOr | crate::ast::BinaryOpKind::BitXor => {
                // 2026-08-01 (B1): #String bitwise defaults — operate on the
                // content bytes and return a NEW [len][bytes] buffer (same
                // length). When both operands are #String, emit the matching
                // runtime call; otherwise fall through to the numeric path
                // below (the config templates handle Int/etc.).
                if self.is_string_operand(&l.ty)
                    && self.is_string_operand(&r.ty)
                {
                    let rt = match kind {
                        crate::ast::BinaryOpKind::BitAnd => "@briev_str_band",
                        crate::ast::BinaryOpKind::BitOr => "@briev_str_bor",
                        _ => "@briev_str_bxor",
                    };
                    let res = self.fun.gen_reg();
                    writeln!(out, "{}{} = call ptr {}(ptr {}, ptr {})", indent, res, rt, l.name, r.name).ok();
                    return TypedRegister { name: res, ty: Type::string() };
                }
                let cmp_ty = self.llvm_type(&l.ty);
                let op = match kind {
                    crate::ast::BinaryOpKind::BitAnd => "and",
                    crate::ast::BinaryOpKind::BitOr => "or",
                    _ => "xor",
                };
                writeln!(out, "{}{} = {} {} {}, {}", indent, v, op, cmp_ty, l.name, r.name).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::int(),
                }
            }
            _ => {
                writeln!(out, "{}{} = add i64 {}, {}", indent, v, l.name, r.name).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::int(),
                }
            }
        }
    }

    /// Emit a unary operation.
    fn emit_unary_op(
        &mut self,
        out: &mut String,
        v: &str,
        kind: &crate::ast::UnaryOpKind,
        operand: &TypedRegister,
        indent: &str,
    ) -> TypedRegister {
        match kind {
            crate::ast::UnaryOpKind::Neg => {
                // 2026-07-14: Neg must use fsub for float operands — sub i64 is invalid for doubles
                let is_float = operand.ty == Type::float() || operand.ty == Type::float64();
                if is_float {
                    let fty = if operand.ty == Type::float64() {
                        "double"
                    } else {
                        "float"
                    };
                    writeln!(out, "{}{} = fsub {} -0.0, {}", indent, v, fty, operand.name).ok();
                } else {
                    writeln!(out, "{}{} = sub i64 0, {}", indent, v, operand.name).ok();
                }
                TypedRegister {
                    name: v.to_string(),
                    ty: operand.ty.clone(),
                }
            }
            crate::ast::UnaryOpKind::Not => {
                writeln!(out, "{}{} = xor i8 {}, 1", indent, v, operand.name).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: operand.ty.clone(),
                }
            }
            crate::ast::UnaryOpKind::BitNot => {
                // 2026-08-01 (B1): #String unary bitwise default — complement
                // each content byte and return a NEW [len][bytes] buffer (same
                // length). Numeric operands keep the i64 xor path below.
                if self.is_string_operand(&operand.ty) {
                    let res = self.fun.gen_reg();
                    writeln!(out, "{}{} = call ptr @briev_str_bnot(ptr {})", indent, res, operand.name).ok();
                    return TypedRegister { name: res, ty: Type::string() };
                }
                writeln!(out, "{}{} = xor i64 {}, -1", indent, v, operand.name).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: operand.ty.clone(),
                }
            }
        }
    }

    /// Emit a statement (delegates to emit_stmt module).
    pub(crate) fn emit_statement(
        &mut self,
        out: &mut String,
        stmt: &Statement,
        indent: &str,
    ) -> TypedRegister {
        crate::backend::llvm::emit_stmt::emit_statement(self, out, stmt, indent)
    }

    /// Get a local variable's register name from FunctionContext.
    /// 2026-07-18: Resolve a struct field name to its numeric index (for extractvalue).
    fn resolve_field_index(&self, ty: &Type, field: &str) -> usize {
        let universe = match self.ctx.type_universe.as_ref() {
            Some(u) => u,
            None => return self.ctx.field_index_map.get(field).copied().unwrap_or(0),
        };
        let key = match ty.universe_key() {
            Some(k) => k,
            None => return self.ctx.field_index_map.get(field).copied().unwrap_or(0),
        };
        let rt = match universe.get(key) {
            Some(r) => r,
            None => return self.ctx.field_index_map.get(field).copied().unwrap_or(0),
        };
        for (i, (f, _)) in rt.fields.iter().enumerate() {
            if f == field {
                return i;
            }
        }
        self.ctx.field_index_map.get(field).copied().unwrap_or(0)
    }

    /// 2026-07-25: Resolve struct field type.
    fn resolve_field_type(&self, ty: &Type, field: &str) -> Option<Type> {
        let universe = self.ctx.type_universe.as_ref()?;
        let key = ty.universe_key()?;
        let rt = universe.get(key)?;
        for (f, ft) in &rt.fields {
            if f == field {
                // 2026-08-07 (Phase 7): an Applied generic obj's member keeps
                // RAW const dims (`data: T[Rows][Cols]` → `Named("Rows", 0)`).
                // Substitute the concrete instance args (the same map
                // ensure_mono uses for struct_types) so the member's dims
                // resolve (`Rows` → Number(3) → Anonymous(3)).
                if let Type::Applied(base, args) = ty {
                    let params = self.ctx.obj_type_params.get(base).cloned().unwrap_or_default();
                    let subst: std::collections::HashMap<String, Type> =
                        params.into_iter().zip(args.iter().cloned()).collect();
                    return Some(crate::typechecker::substitute_type(ft, &subst));
                }
                return Some(ft.clone());
            }
        }
        None
    }

    fn get_local(&self, name: &str) -> Option<String> {
        self.fun.let_bindings.get(name).cloned()
    }

    /// Get a local variable's type from FunctionContext.
    fn get_local_type(&self, name: &str) -> Type {
        self.fun
            .let_binding_types
            .get(name)
            .cloned()
            .unwrap_or(Type::int())
    }

    fn emit_intrinsic_call_dispatch(
        &mut self,
        out: &mut String,
        v: &str,
        name: &str,
        args: &[Expr],
        analysis_id: Option<usize>,
        indent: &str,
    ) -> TypedRegister {
        emit_intrinsic_call(self, out, v, name, args, analysis_id, indent)
    }

    /// 2026-07-14: Emit bit-shift/mask for layout field access (value.#fieldname).
    /// Reads offset and width from ResolvedType properties attached by the normalizer.
    pub(crate) fn emit_layout_field_read(
        &mut self,
        out: &mut String,
        v: &str,
        obj_reg: &TypedRegister,
        field: &str,
        indent: &str,
    ) -> TypedRegister {
        let field_name = &field[1..];
        let offset_key = format!("field.{}.offset", field_name);
        let width_key = format!("field.{}.width", field_name);
        let (offset, width) = self
            .ctx
            .type_universe
            .as_ref()
            .and_then(|u| crate::type_universe::resolve_type(u, &obj_reg.ty))
            .map(|rt| {
                let off = rt
                    .properties
                    .get(&offset_key)
                    .and_then(|pv| {
                        if let PropertyValue::Int(n) = pv {
                            Some(*n as u64)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                let wid = rt
                    .properties
                    .get(&width_key)
                    .and_then(|pv| {
                        if let PropertyValue::Int(n) = pv {
                            Some(*n as u64)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(64);
                (off, wid)
            })
            .unwrap_or((0, 64));

        if offset == 0 && width == 64 {
            return TypedRegister {
                name: obj_reg.name.clone(),
                ty: Type::int(),
            };
        }
        let shifted = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = lshr {} {}, {}",
            indent,
            shifted,
            self.llvm_type(&obj_reg.ty),
            obj_reg.name,
            offset
        )
        .ok();
        if width < 64 {
            let mask = (1u128 << width).wrapping_sub(1);
            writeln!(
                out,
                "{}{} = and {} {}, {}",
                indent,
                v,
                self.llvm_type(&obj_reg.ty),
                shifted,
                mask
            )
            .ok();
            return TypedRegister {
                name: v.to_string(),
                ty: Type::int(),
            };
        }
        TypedRegister {
            name: shifted,
            ty: Type::int(),
        }
    }

    // ── Casting Graph Path Emission ──────────────────────────────────

    /// Try to resolve a cast through the casting graph protocol path.
    /// Returns None if no path exists (caller falls back to LLVM coercion).
    pub(super) fn emit_cast_path(
        &mut self, out: &mut String, v: &str,
        src: &TypedRegister, target: &Type, indent: &str,
    ) -> Option<TypedRegister> {
        // 2026-08-13 (pack): `x as Bits<N>` is a width assertion — the
        // casting graph treats Int ↔ Bits as a same-lane identity (no trunc),
        // leaving `16 as Bits<4>` holding 16 in an i64 register. Truncate the
        // integer source to exactly N bits here, before the graph. A source
        // that is already i{N} (a packed field read) is an identity.
        if let Some(n) = crate::type_universe::bits_width(target) {
            let src_ll = self.llvm_type(&src.ty);
            let tgt = format!("i{}", n);
            if src_ll != tgt && src_ll.starts_with('i') {
                let reg = self.fun.gen_reg();
                writeln!(out, "{}{} = trunc {} {} to {}", indent, reg, src_ll, src.name, tgt).ok();
                return Some(TypedRegister { name: reg, ty: target.clone() });
            }
            return Some(TypedRegister { name: src.name.clone(), ty: target.clone() });
        }
        let graph = self.ctx.casting_graph.as_ref()?;
        let universe = self.ctx.type_universe.as_ref()?;
        let (src_cat, src_var) = graph.type_to_protocol(universe, &src.ty);
        let (dst_cat, dst_var) = graph.type_to_protocol(universe, target);
        let path = graph.find_path(&src_cat, &src_var, &dst_cat, &dst_var);

        match &path {
            Some(p) => self.emit_cast_steps(out, v, src, target, indent, p),
            None => None,
        }
    }

    /// Emit LLVM IR for a sequence of cast steps returned by the graph.
    fn emit_cast_steps(
        &mut self, out: &mut String, v: &str,
        src: &TypedRegister, target: &Type, indent: &str,
        path: &[crate::casting::graph::CastStep],
    ) -> Option<TypedRegister> {
        let target_ll = self.llvm_type(target);

        // Identity (same protocol) — return source with target type
        if path.is_empty() {
            let src_ll = self.llvm_type(&src.ty);
            let target_ll = self.llvm_type(target);
            // 2026-08-03: a Float width change (float → double / double →
            // float) has no graph lane — it is not a representation change,
            // just a precision change, so emit fpext/fptrunc. This fixes
            // `2.0 as Float64` / `x as CDouble` emitting a bitcast+sitofp mess.
            if (src_ll == "float" || src_ll == "double")
                && (target_ll == "float" || target_ll == "double")
                && src_ll != target_ll
            {
                let reg = self.fun.gen_reg();
                let op = if target_ll == "double" { "fpext" } else { "fptrunc" };
                writeln!(out, "{}{} = {} {} {} to {}", indent, reg, op, src_ll, src.name, target_ll).ok();
                return Some(TypedRegister { name: reg, ty: target.clone() });
            }
            return Some(TypedRegister { name: src.name.clone(), ty: target.clone() });
        }

        // 2026-07-30: Ptr values stored as i64 internally — register is already
        // i64 even though llvm_type(Ptr) returns "ptr". Skip the bitcast/ptrtoint.
        if matches!(src.ty, Type::Ptr(_)) && target_ll == "i64" {
            if let Some(first) = path.first() {
                use crate::casting::graph::LaneKind;
                if matches!(first.lane, LaneKind::Bitcast | LaneKind::PtrToInt) {
                    return Some(TypedRegister { name: src.name.clone(), ty: target.clone() });
                }
            }
        }

        // 2026-07-30: i64 → ptr: skip bitcast; Deref/store handlers already
        // emit the inttoptr at consumption time. Just return the register with
        // the correct Ptr type — the value is i64 but the type system says Ptr.
        let src_ll = self.llvm_type(&src.ty);
        if matches!(target, Type::Ptr(_)) && src_ll == "i64" {
            return Some(TypedRegister { name: src.name.clone(), ty: target.clone() });
        }

        let mut cur = src.name.clone();
        let mut cur_ll = self.llvm_type(&src.ty);
        let total = path.len();

        for (i, step) in path.iter().enumerate() {
            let is_last = i == total - 1;
            let dst = if is_last { v.to_string() } else { self.fun.gen_reg() };
            let dst_ll = if is_last { target_ll.clone() } else { self.llvm_type(&src.ty) };

            match &step.lane {
                crate::casting::graph::LaneKind::Bitcast => {
                    // 2026-08-13 (pack): LLVM `bitcast` requires equal-width
                    // operands. A Bit lane between integer widths of different
                    // size (`Bits<12> as Int`) is a zext/trunc, not a bitcast —
                    // sub-byte packed field reads surface i12 registers.
                    if cur_ll != dst_ll
                        && cur_ll.starts_with('i')
                        && dst_ll.starts_with('i')
                    {
                        let (cw, dw) = (cur_ll[1..].parse::<u64>().unwrap_or(64),
                                        dst_ll[1..].parse::<u64>().unwrap_or(64));
                        if cw < dw {
                            writeln!(out, "{}{} = zext {} {} to {}", indent, dst, cur_ll, cur, dst_ll).ok();
                        } else {
                            writeln!(out, "{}{} = trunc {} {} to {}", indent, dst, cur_ll, cur, dst_ll).ok();
                        }
                    } else {
                        writeln!(out, "{}{} = bitcast {} {} to {}",
                            indent, dst, cur_ll, cur, dst_ll).ok();
                    }
                }
                crate::casting::graph::LaneKind::IntToFloat => {
                    // 2026-07-31: The destination type is dst_ll (float for a
                    // Float target, double for Float64/Double) — the old
                    // hardcoded `to double` broke `as Float` casts (telemetry
                    // stream): a double register fed a fadd float.
                    writeln!(out, "{}{} = sitofp {} {} to {}",
                        indent, dst, cur_ll, cur, dst_ll).ok();
                }
                crate::casting::graph::LaneKind::FloatToInt => {
                    writeln!(out, "{}{} = fptosi {} {} to i64",
                        indent, dst, cur_ll, cur).ok();
                }
                crate::casting::graph::LaneKind::ExtCall(fn_name) => {
                    // 2026-08-01: the ExtCall's return type must match the
                    // lane's destination LLVM type — `Int → #String` emits
                    // `call ptr @int_to_str(...)` (a String IS a ptr), while
                    // `#String → Int` emits `call i64 @str_to_int(...)`. The
                    // old hardcoded `i64` made int_to_str return an i64 that
                    // the String target then ptrtoint'd (a type mismatch).
                    // 2026-08-13: a native Char source (i32, from a boxed Char
                    // param unboxed at read) must be widened to the boxed i64
                    // the runtime helpers expect (`char_to_str(int64_t)`).
                    let (arg_ll, arg) = if cur_ll == "i32" && self.is_protocol_member(&src.ty, "#Char") {
                        let w = self.fun.gen_reg();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, w, cur).ok();
                        ("i64".to_string(), w)
                    } else {
                        (cur_ll.clone(), cur.clone())
                    };
                    writeln!(out, "{}{} = call {} @{}({} {})",
                        indent, dst, dst_ll, fn_name, arg_ll, arg).ok();
                }
                crate::casting::graph::LaneKind::ExtCallDyn(fn_name) => {
                    // 2026-08-03: proto-binding transform (owned function
                    // name), e.g. cstr_to_briev/str_to_c for #String<CString>.
                    writeln!(out, "{}{} = call {} @{}({} {})",
                        indent, dst, dst_ll, fn_name, cur_ll, cur).ok();
                }
                crate::casting::graph::LaneKind::ExtractData => {
                    writeln!(out, "{}{} = extractvalue {} {}, 0",
                        indent, dst, cur_ll, cur).ok();
                }
                crate::casting::graph::LaneKind::PtrToInt => {
                    writeln!(out, "{}{} = ptrtoint {} {} to i64",
                        indent, dst, cur_ll, cur).ok();
                }
                crate::casting::graph::LaneKind::IntToPtr => {
                    writeln!(out, "{}{} = inttoptr {} {} to ptr",
                        indent, dst, cur_ll, cur).ok();
                }
                crate::casting::graph::LaneKind::ZExt => {
                    writeln!(out, "{}{} = zext {} {} to {}",
                        indent, dst, cur_ll, cur, dst_ll).ok();
                }
                crate::casting::graph::LaneKind::Trunc => {
                    writeln!(out, "{}{} = trunc {} {} to {}",
                        indent, dst, cur_ll, cur, dst_ll).ok();
                }
                crate::casting::graph::LaneKind::FloatWidth => {
                    // 2026-08-03: the #Float protocol's width cast — fpext/
                    // fptrunc between float and double (same width → identity).
                    if cur_ll == target_ll {
                        return Some(TypedRegister { name: cur.clone(), ty: target.clone() });
                    }
                    let op = if target_ll == "double" { "fpext" } else { "fptrunc" };
                    writeln!(out, "{}{} = {} {} {} to {}", indent, dst, op, cur_ll, cur, target_ll).ok();
                }
                crate::casting::graph::LaneKind::Chain(a, b) => {
                    // 2026-07-30: Emit the composite as two consecutive lanes
                    // (recursively). The chain only appears in BFS-compressed
                    // paths (Bit → String often collapses a PtrToInt + IntToPtr).
                    // Handle by emitting each step separately via emit_cast_steps.
                    let _ = (a, b);
                    return None;
                }
                crate::casting::graph::LaneKind::CastFromBitCallback => {
                    // 2026-08-01 (B2): the ENCODING DOOR — `#Bit → <type>`.
                    // A registered CastFrom(#Bit) override for the target type
                    // calls the override function; otherwise the #String default
                    // is the UTF8 wrap: inttoptr the address, then
                    // briev_cstr_to_briev materializes the [len][bytes] header
                    // by construction (length derived from the bytes). The
                    // header is never inherited from the bits — it is created.
                    let override_fn = match target.universe_key() {
                        Some(key) => self
                            .ctx
                            .casting_graph
                            .as_ref()
                            .and_then(|g| g.get_cast_from_bit(key)),
                        None => None,
                    };
                    if let Some(fn_name) = override_fn {
                        writeln!(out, "{}{} = call {} @{}(i64 {})",
                            indent, dst, dst_ll, fn_name, cur).ok();
                    } else {
                        let p = self.fun.gen_reg();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, cur).ok();
                        writeln!(out, "{}{} = call ptr @briev_bits_to_str(ptr {})",
                            indent, dst, p).ok();
                    }
                }
            }
            cur = dst;
            cur_ll = dst_ll;
        }

        Some(TypedRegister { name: v.to_string(), ty: target.clone() })
    }

    /// Emit a single cast lane instruction (helper for Chain).
    fn emit_single_cast_lane(
        &self, out: &mut String, dst: &str,
        lane: &crate::casting::graph::LaneKind,
        src_name: &str, src_ll: &str, indent: &str,
    ) {
        match lane {
            crate::casting::graph::LaneKind::Bitcast => {
                writeln!(out, "{}{} = bitcast {} {} to i64",
                    indent, dst, src_ll, src_name).ok();
            }
            crate::casting::graph::LaneKind::IntToFloat => {
                writeln!(out, "{}{} = sitofp {} {} to double",
                    indent, dst, src_ll, src_name).ok();
            }
            crate::casting::graph::LaneKind::FloatToInt => {
                writeln!(out, "{}{} = fptosi {} {} to i64",
                    indent, dst, src_ll, src_name).ok();
            }
            crate::casting::graph::LaneKind::ZExt => {
                writeln!(out, "{}{} = zext {} {} to i64",
                    indent, dst, src_ll, src_name).ok();
            }
            crate::casting::graph::LaneKind::Trunc => {
                writeln!(out, "{}{} = trunc {} {} to i8",
                    indent, dst, src_ll, src_name).ok();
            }
            crate::casting::graph::LaneKind::ExtractData => {
                writeln!(out, "{}{} = extractvalue {} {}, 0",
                    indent, dst, src_ll, src_name).ok();
            }
            crate::casting::graph::LaneKind::PtrToInt => {
                writeln!(out, "{}{} = ptrtoint {} {} to i64",
                    indent, dst, src_ll, src_name).ok();
            }
            // For Chain-internal ExtCall — emit the call with conservative i64 return
            crate::casting::graph::LaneKind::ExtCall(fn_name) => {
                writeln!(out, "{}{} = call i64 @{}({} {})",
                    indent, dst, fn_name, src_ll, src_name).ok();
            }
            _ => {
                writeln!(out, "{}{} = bitcast {} {} to i64",
                    indent, dst, src_ll, src_name).ok();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}

/// 2026-07-31 (A5): the briev name of a member declaration (txn/defn/node).
pub(crate) fn member_briev_name(m: &crate::ast::TopLevel) -> &str {
    match m {
        crate::ast::TopLevel::Transaction(t) => &t.name,
        crate::ast::TopLevel::Definition(d) => &d.name,
        crate::ast::TopLevel::TypeDefOperator(d) => &d.name,
        _ => "",
    }
}
