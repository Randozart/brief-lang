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
use crate::backend::llvm::intrinsics::{
    emit_intrinsic_call, emit_meld_shuffle, emit_simple_call, find_cast_impl,
    template_for_op, try_cast_protocol_path, type_name_str,
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
    fn emit_expr_inner(
        &mut self,
        out: &mut String,
        v: &str,
        expr: &Expr,
        indent: &str,
    ) -> TypedRegister {
        match expr {
            // ── Literals ─────────────────────────────────────────────
            Expr::Decimal(n) => {
                self.emit_int(out, v, *n, indent)
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
            Expr::Quoted(bytes) | Expr::TaggedQuotedLiteral(bytes, _) => self.emit_string_literal(out, v, bytes, indent),

            // ── Identifier ───────────────────────────────────────────
            Expr::Identifier(name) => {
                // 2026-07-29: Accumulation chaining — check last_val_temps FIRST.
                // When a field is written multiple times in one iteration, the second
                // read must return the just-computed value, not the loop-header phi,
                // so the first write forms a live dependency chain (not dead code).
                if let Some(reg) = self.fun.last_val_temps.get(name) {
                    let brief_ty = self
                        .fun
                        .last_val_types
                        .get(name)
                        .cloned()
                        .or_else(|| {
                            self.ctx
                                .field_index_map
                                .get(name)
                                .and_then(|idx| self.ctx.field_brief_types.get(*idx).cloned())
                        })
                        .unwrap_or(Type::int());
                    return TypedRegister {
                        name: reg.clone(),
                        ty: brief_ty,
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
                        let brief_ty = self
                            .ctx
                            .field_index_map
                            .get(name)
                            .and_then(|idx| self.ctx.field_brief_types.get(*idx).cloned())
                            .unwrap_or(Type::int());
                        return TypedRegister { name: lane_reg, ty: brief_ty };
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
                        TypedRegister {
                            name: loaded,
                            ty: self.get_local_type(name),
                        }
                    } else {
                        TypedRegister {
                            name: reg.clone(),
                            ty: self.get_local_type(name),
                        }
                    }
                } else if let Some(phi_reg_str) = self.fun.phi_field_regs.get(name).cloned() {
                    let brief_ty = self
                        .ctx
                        .field_index_map
                        .get(name)
                        .and_then(|idx| self.ctx.field_brief_types.get(*idx).cloned())
                        .unwrap_or(Type::int());
                    if brief_ty == Type::float64() {
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
                    } else if brief_ty == Type::float() {
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
                            ty: brief_ty,
                        }
                    }
                } else if let Some(&idx) = self.ctx.field_index_map.get(name) {
                    // 2026-07-19: Load with native type from field_types (e.g. "float",
                    // "double", "i64"). No unboxing needed — LLVM type matches %State
                    // struct layout. Phi registers remain i64 (handled above).
                    // 2026-07-20: State fields are always i64 in %State. For float-typed
                    // fields, trunc+bitcast i64 → float so downstream arithmetic gets
                    // correct types (matches the phi path at lines 100-104).
                    let (loaded, brief_ty) = self.emit_state_load_i64_by_idx(out, indent, idx);
                    // 2026-07-21: With native float types, the load already returns
                    // float/double. Check field_types[idx] to skip the conversion.
                    let field_llvm_ty = self.ctx.field_types.get(idx)
                        .cloned().unwrap_or_else(|| "i64".to_string());
                    if brief_ty == Type::float64() && field_llvm_ty == "double" {
                        TypedRegister { name: loaded, ty: Type::float64() }
                    } else if brief_ty == Type::float() && field_llvm_ty == "float" {
                        TypedRegister { name: loaded, ty: Type::float() }
                    } else if brief_ty == Type::float64() {
                        let dbl = self.fun.gen_reg();
                        writeln!(out, "{}{} = bitcast i64 {} to double", indent, dbl, loaded).ok();
                        TypedRegister {
                            name: dbl,
                            ty: Type::float64(),
                        }
                    } else if brief_ty == Type::float() {
                        let tr = self.fun.gen_reg();
                        let fl = self.fun.gen_reg();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, loaded).ok();
                        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
                        TypedRegister {
                            name: fl,
                            ty: Type::float(),
                        }
                    } else {
                        TypedRegister {
                            name: loaded,
                            ty: brief_ty,
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
                    } else {
                        writeln!(out, "{}{} = load i64, ptr @{}", indent, v, name).ok();
                        TypedRegister {
                            name: v.to_string(),
                            ty: Type::int(),
                        }
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
                // 2026-07-18: SVO — emit inline handle for small lists
                // when feature_svo is ON and the type is vector-like.
                if self.feature_svo && exprs.len() <= 3 {
                    // Check if the expression type is List<T> (vector-like)
                    // by inspecting the iteration variable's type context.
                    // For now, always emit inline for lists ≤3 elements.
                    return self.emit_svo_list(out, v, exprs, indent);
                }
                self.emit_heap_seq(out, v, exprs, indent)
            }

            // ── Struct literal ─────────────────────────────────────────
            Expr::StructLiteral { type_name, fields } => {
                self.emit_struct_literal(out, v, type_name, fields, indent)
            }

            // ── Field access ─────────────────────────────────────────
            Expr::Field(obj, field) => {
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
                    if dims.len() == 1 && matches!(dims[0], crate::ast::Dimension::Anonymous(_)) {
                        // Emit GEP to get a pointer to the array field.
                        // First get the struct's address from let_bindings if available.
                        let struct_ptr = if let Expr::Identifier(name) = obj.as_ref() {
                            self.get_local(name)
                        } else {
                            None
                        };
                        if let Some(slot) = struct_ptr {
                            let gep = self.fun.gen_reg();
                            writeln!(
                                out,
                                "{}{} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                                indent, gep, self.llvm_type(&obj_reg.ty), slot, field_idx
                            ).ok();
                            let result = self.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, result, gep).ok();
                            return TypedRegister {
                                name: result,
                                ty: Type::ptr(*inner.clone()),
                            };
                        }
                        // 2026-07-25: Struct not in let_bindings — spill to alloca + GEP.
                        // The struct value is in an SSA register; we need a pointer to GEP.
                        // Spill via alloca + store, then GEP into the alloca.
                        // LLVM mem2reg should eliminate the alloca in opt builds.
                        let alloca = self.fun.gen_reg();
                        writeln!(out, "{}{} = alloca {}, align 8", indent, alloca, self.llvm_type(&obj_reg.ty)).ok();
                        writeln!(out, "store {} {}, ptr {}, align 8", self.llvm_type(&obj_reg.ty), obj_reg.name, alloca).ok();
                        let gep = self.fun.gen_reg();
                        writeln!(
                            out,
                            "{}{} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                            indent, gep, self.llvm_type(&obj_reg.ty), alloca, field_idx
                        ).ok();
                        let result = self.fun.gen_reg();
                        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, result, gep).ok();
                        return TypedRegister {
                            name: result,
                            ty: Type::ptr(*inner.clone()),
                        };
                    }
                }
                // Struct field access via extractvalue (numeric index required)
                writeln!(
                    out,
                    "{}{} = extractvalue {} {}, {}",
                    indent,
                    v,
                    self.llvm_type(&obj_reg.ty),
                    obj_reg.name,
                    field_idx
                )
                .ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::int(),
                }
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
                // 2026-07-18: SVO List indexing — extract inline element.
                if self.feature_svo
                    && self
                        .ctx
                        .type_universe
                        .as_ref()
                        .map_or(false, |u| u.is_vector_like(&obj_reg.ty))
                {
                    return self.emit_svo_index(out, v, &obj_reg, &idx_reg, indent);
                }
                if matches!(obj_reg.ty, Type::Ptr(_)) {
                    let ptr = self.fun.gen_reg();
                    writeln!(
                        out,
                        "{}{} = inttoptr i64 {} to ptr",
                        indent, ptr, obj_reg.name
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
                    let is_list_type = matches!(&obj_reg.ty, Type::Applied(n, _) if n == "List");
                    if is_list_type {
                        writeln!(out, "{}{} = add i64 {}, 1", indent, offset, idx_reg.name).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 {}, 0", indent, offset, idx_reg.name).ok();
                    }
                    let gep = self.fun.gen_reg();
                    writeln!(
                        out,
                        "{}{} = getelementptr i64, ptr {}, i64 {}",
                        indent, gep, ptr, offset
                    )
                    .ok();
                    writeln!(out, "{}{} = load i64, ptr {}", indent, v, gep).ok();
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
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::int(),
                }
            }

            // ── Cast ─────────────────────────────────────────────────
            // 2026-07-14: Added string conversion paths. i64→ptr calls
            // __int_to_str__, ptr→i64 calls __str_to_int after inttoptr.
            // 2026-07-16: String → Int must check is_string_chain because
            // emit_string_literal returns Type::int() (ptrtoint representation)
            // but the value is semantically a String — the LLVM type match
            // alone would produce a no-op bitcast.
            // 2026-07-30: Protocol-based cast resolution (physical vs semantic)
            // checked before LLVM coercion. If the source or target participates
            // in a known protocol (Cast.#<Cat>), delegate to resolve_cast.
            Expr::Cast(expr, target) => {
                let src = self.emit_expr(out, expr, indent);
                // 2026-07-30: Check protocol-based cast path first.
                // Physical (target is #Bit) → literal memory bytes.
                // Semantic (target is any other protocol) → operator pipeline.
                if self.is_protocol_member(&src.ty, "#Bit")
                    || self.is_protocol_member(target, "#Bit")
                    || type_name_str(&src.ty).and_then(|n| find_cast_impl(self, &n, "Cast")).is_some()
                    || type_name_str(&src.ty).and_then(|n| find_cast_impl(self, &n, "CastTo")).is_some()
                    || type_name_str(target).and_then(|n| find_cast_impl(self, &n, "CastFrom")).is_some()
                {
                    if let Some(result) = self.resolve_cast(out, v, &src, target, indent) {
                        return result;
                    }
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
                } else if *target == Type::string() || *target == Type::data() {
                    if src_ll == "i64" {
                        writeln!(
                            out,
                            "{}{} = call i64 @__int_to_str__(i64 {})",
                            indent, v, src.name
                        )
                        .ok();
                    } else {
                        let ip = self.fun.gen_reg();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ip, src.name).ok();
                        writeln!(out, "{}{} = call i64 @__str_to_int(ptr {})", indent, v, ip).ok();
                    }
                } else if target_ll == "i64" && matches!(src.ty, Type::Ptr(_)) {
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, src.name).ok();
                } else if target_ll == "i64" && self.is_string_chain(expr) {
                    // String literal or string-producing expr → Int
                    let ip = self.fun.gen_reg();
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ip, src.name).ok();
                    writeln!(out, "{}{} = call i64 @__str_to_int(ptr {})", indent, v, ip).ok();
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
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, src.name).ok();
                } else if target_ll == "ptr" && src_ll == "i64" {
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, v, src.name).ok();
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
            Expr::Match(_, arms) => {
                if let Some(first) = arms.first() {
                    self.emit_expr(out, &first.body, indent)
                } else {
                    TypedRegister {
                        name: v.to_string(),
                        ty: Type::void(),
                    }
                }
            }

            // ── Lambda ───────────────────────────────────────────────
            Expr::Lambda(params, body) => {
                let _params = params;
                self.emit_expr(out, body, indent)
            }

            // ── Address-of ────────────────────────────────────────────
            Expr::AddrOf(inner) => {
                // &expr provides the address of a state field or value.
                // For state fields, emit GEP into %State and ptrtoint to i64.
                match inner.as_ref() {
                    Expr::Identifier(name) => {
                        if let Some(&idx) = self.ctx.field_index_map.get(name) {
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
                // Check if the pointer type carries a LLVM pointer representation.
                let pointee_ty = match &ptr_reg.ty {
                    Type::Ptr(inner_ty) => inner_ty.as_ref().clone(),
                    _ => Type::int(), // fallback
                };
                let llvm_ty = self.llvm_type(&pointee_ty);
                writeln!(
                    out,
                    "{}{} = load {}, ptr {}, align 8",
                    indent, v, llvm_ty, ptr_reg.name
                )
                .ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: pointee_ty,
                }
            }

            // ── DerivationBlock / PropertyGet / FormattingAnnotation ─
            Expr::DerivationBlock(_) | Expr::PropertyGet(_) | Expr::FormattingAnnotation(_) => {
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
            Expr::Exists(_) => { unreachable!("fn? only in stage eval") },
            Expr::Slice { array, start, end, stride } => {
                // 2026-07-26: Evaluate slice bounds for side effects.
                // The narrowing pass converts constant-bounds slices to Vector<T,N>
                // before codegen; this arm handles dynamic slices.
                let array_reg = self.emit_expr(out, array, indent);
                if let Some(s) = start { self.emit_expr(out, s, indent); }
                if let Some(e) = end { self.emit_expr(out, e, indent); }
                if let Some(s) = stride { self.emit_expr(out, s, indent); }
                array_reg
            }
        }
    }

    // ── Sub-helpers ──────────────────────────────────────────────────

    /// 2026-07-14: Emit a heap-allocated sequence (list/tuple) with 2-slot header.
    /// Protocol: slot 0 = length (i64), slots 1..N = elements.
    /// Empty seq → @ll_empty_list global sentinel.
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
            writeln!(out, "{}{} = ptrtoint ptr @ll_empty_list to i64", indent, v).ok();
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
                writeln!(out, "{}store i64 {}, ptr {}", indent, e.name, slot).ok();
            }
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, hdr).ok();
        }
        TypedRegister {
            name: v.to_string(),
            ty: Type::ptr(Type::int()),
        }
    }

    // 2026-07-18: SVO list literal — emit inline handle for ≤3 elements.
    // Handle format (N data slots + 1 len+cap+tag slot):
    //   slot[0..N-1] = element values as i64
    //   slot[N]      = (len << 32) | (cap << 32) | 1  (bit 0 = inline tag)
    fn emit_svo_list(
        &mut self,
        out: &mut String,
        v: &str,
        exprs: &[Expr],
        indent: &str,
    ) -> TypedRegister {
        let cap = 3usize;
        let len = exprs.len();
        // Build the struct via insertvalue
        let mut reg = format!("%svo_{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        let struct_ty = format!(
            "{{ {} }}",
            std::iter::repeat("i64")
                .take(cap + 1)
                .collect::<Vec<_>>()
                .join(", ")
        );
        writeln!(
            out,
            "{}{} = insertvalue {} undef, i64 0, 0",
            indent, reg, struct_ty
        )
        .ok();
        for (i, expr) in exprs.iter().enumerate() {
            let elem = self.emit_expr(out, expr, indent);
            let next = self.fun.gen_reg();
            writeln!(
                out,
                "{}{} = insertvalue {} %{}, i64 {}, {}",
                indent, next, struct_ty, reg, elem.name, i
            )
            .ok();
            reg = next;
        }
        // Pack len+cap+tag into the last slot
        let last_idx = cap;
        let packed = ((len as u64) << 32) | ((cap as u64) << 32) | 1u64;
        let tag_reg = self.fun.gen_reg();
        writeln!(out, "{}{} = add i64 0, {}", indent, tag_reg, packed).ok();
        let final_reg = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = insertvalue {} %{}, i64 {}, {}",
            indent, final_reg, struct_ty, reg, tag_reg, last_idx
        )
        .ok();
        TypedRegister {
            name: final_reg,
            ty: Type::int(),
        }
    }

    // 2026-07-18: SVO List indexing — extract element from inline storage
    // or heap. Uses stack-allocated array + GEP for dynamic inline indexing
    // (extractvalue requires constant indices, but the index is runtime).
    fn emit_svo_index(
        &mut self,
        out: &mut String,
        v: &str,
        obj: &TypedRegister,
        idx: &TypedRegister,
        indent: &str,
    ) -> TypedRegister {
        let cap = self
            .ctx
            .type_universe
            .as_ref()
            .map(|u| u.svo_capacity(&obj.ty))
            .unwrap_or(0);
        let n_slots = cap + 1;
        let struct_ty = format!(
            "{{ {} }}",
            std::iter::repeat("i64")
                .take(n_slots)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let counter = self.fun.txn_counter;
        self.fun.txn_counter += 1;
        let arr = format!("%svo_arr_{}", counter);
        writeln!(out, "{}{} = alloca [{} x i64], align 8", indent, arr, cap).ok();
        // Store each data slot into the stack array
        for i in 0..cap {
            let slot = format!("%svo_sl_{}_{}", counter, i);
            let gep = format!("%svo_gep_{}_{}", counter, i);
            writeln!(
                out,
                "{}{} = extractvalue {} {}, {}",
                indent, slot, struct_ty, obj.name, i
            )
            .ok();
            writeln!(
                out,
                "{}{} = getelementptr [{} x i64], ptr {}, i64 0, i64 {}",
                indent, gep, cap, arr, i
            )
            .ok();
            writeln!(out, "{}store i64 {}, ptr {}", indent, slot, gep).ok();
        }
        // Extract tag+len from last slot
        let tag_slot = format!("%svo_tag_{}", counter);
        writeln!(
            out,
            "{}{} = extractvalue {} {}, {}",
            indent, tag_slot, struct_ty, obj.name, cap
        )
        .ok();
        let is_inline = format!("%svo_inl_{}", counter);
        writeln!(out, "{}{} = and i64 {}, 1", indent, is_inline, tag_slot).ok();
        let inline_l = format!(".svo_idx_inl_{}", counter);
        let heap_l = format!(".svo_idx_heap_{}", counter);
        let done_l = format!(".svo_idx_done_{}", counter);
        writeln!(
            out,
            "{}br i1 %{}, label %{}, label %{}",
            indent, is_inline, inline_l, heap_l
        )
        .ok();
        // Inline path: GEP + load from stack array
        writeln!(out, "{}{}:", indent, inline_l).ok();
        let inl_gep = format!("%svo_ig_{}", counter);
        writeln!(
            out,
            "{}{} = getelementptr [{} x i64], ptr {}, i64 0, i64 {}",
            indent, inl_gep, cap, arr, idx.name
        )
        .ok();
        let inl_val = format!("%svo_iv_{}", counter);
        writeln!(out, "{}{} = load i64, ptr {}", indent, inl_val, inl_gep).ok();
        writeln!(out, "{}br label %{}", indent, done_l).ok();
        // Heap path: extract ptr from slot 0 (first data slot holds ptrtoint
        // when tag bit 0 is clear), then inttoptr + GEP + load.
        writeln!(out, "{}{}:", indent, heap_l).ok();
        let heap_slot0 = format!("%svo_hs{}", counter);
        writeln!(
            out,
            "{}{} = extractvalue {} {}, 0",
            indent, heap_slot0, struct_ty, obj.name
        )
        .ok();
        let heap_ptr = format!("%svo_hp_{}", counter);
        writeln!(
            out,
            "{}{} = inttoptr i64 {} to ptr",
            indent, heap_ptr, heap_slot0
        )
        .ok();
        let heap_off = format!("%svo_ho_{}", counter);
        writeln!(out, "{}{} = add i64 {}, 1", indent, heap_off, idx.name).ok();
        let heap_gep = format!("%svo_hg_{}", counter);
        writeln!(
            out,
            "{}{} = getelementptr i64, ptr {}, i64 {}",
            indent, heap_gep, heap_ptr, heap_off
        )
        .ok();
        let heap_val = format!("%svo_hv_{}", counter);
        writeln!(out, "{}{} = load i64, ptr {}", indent, heap_val, heap_gep).ok();
        writeln!(out, "{}br label %{}", indent, done_l).ok();
        // Merge
        writeln!(out, "{}{}:", indent, done_l).ok();
        let phi = format!("%svo_ph_{}", counter);
        writeln!(
            out,
            "{}{} = phi i64 [ %{}, %{} ], [ %{}, %{} ]",
            indent, phi, inl_val, inline_l, heap_val, heap_l
        )
        .ok();
        TypedRegister {
            name: phi,
            ty: Type::int(),
        }
    }

    /// 2026-07-25: Emit an integer constant. All intermediate values use i64 —
    /// narrowing is applied at the `ret` instruction.
    fn emit_int(&mut self, out: &mut String, v: &str, imm: i64, indent: &str) -> TypedRegister {
        writeln!(out, "{}{} = add i64 0, {}", indent, v, imm).ok();
        TypedRegister { name: v.to_string(), ty: Type::int() }
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
        // 2026-07-18: Phase B — SSO string path when feature is enabled.
        if self.feature_sso_strings {
            if bytes.len() <= 6 {
                return self.emit_sso_literal(out, v, bytes, indent);
            }
            return self.emit_sso_heap_literal(out, v, bytes, indent);
        }
        self.emit_legacy_string_literal(out, v, bytes, indent)
    }

    // 2026-07-18: SSO string literal — pack ≤6 bytes inline into handle[0] with
    // SSO tag (0b001), store length in handle[1]. Returns {i64, i64} struct.
    // No heap allocation, no 16-byte header, no null terminator needed for SSO.
    fn emit_sso_literal(
        &mut self,
        out: &mut String,
        v: &str,
        bytes: &[u8],
        indent: &str,
    ) -> TypedRegister {
        let len = bytes.len() as u64;
        // Pack bytes into u64 (little-endian), shift left 3 for tag bits, set bit 0 (SSO tag)
        let packed = bytes
            .iter()
            .enumerate()
            .fold(0u64, |acc, (i, &b)| acc | ((b as u64) << (i * 8)));
        let shifted = packed << 3;
        let t0 = self.fun.gen_reg();
        writeln!(out, "{}{} = or i64 {}, 1", indent, t0, shifted).ok();
        // Build {i64, i64} struct
        let t1 = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0",
            indent, t1, t0
        )
        .ok();
        let t2 = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = insertvalue {{ i64, i64 }} %{}, i64 {}, 1",
            indent, t2, t1, len
        )
        .ok();
        TypedRegister {
            name: t2,
            ty: Type::string(),
        }
    }

    // 2026-07-18: SSO heap string literal — allocate raw bytes + null terminator
    // on stack (no 16-byte header, no capacity slot). Handle[0] = ptrtoint with
    // tag 0b000 (heap), handle[1] = length. Returns {i64, i64} struct.
    fn emit_sso_heap_literal(
        &mut self,
        out: &mut String,
        v: &str,
        bytes: &[u8],
        indent: &str,
    ) -> TypedRegister {
        let len = bytes.len() as u64;
        let alloc_size = len + 1;
        let alloca_reg = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = alloca [{} x i8], align 1",
            indent, alloca_reg, alloc_size
        )
        .ok();
        for (i, &b) in bytes.iter().enumerate() {
            let ptr = self.fun.gen_reg();
            writeln!(
                out,
                "{}{} = getelementptr inbounds [{} x i8], ptr {}, i32 0, i32 {}",
                indent, ptr, alloc_size, alloca_reg, i
            )
            .ok();
            writeln!(out, "{}store i8 {}, ptr {}", indent, b, ptr).ok();
        }
        let last = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = getelementptr inbounds [{} x i8], ptr {}, i32 0, i32 {}",
            indent,
            last,
            alloc_size,
            alloca_reg,
            bytes.len()
        )
        .ok();
        writeln!(out, "{}store i8 0, ptr {}", indent, last).ok();
        let p2i = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = ptrtoint ptr {} to i64",
            indent, p2i, alloca_reg
        )
        .ok();
        let t1 = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0",
            indent, t1, p2i
        )
        .ok();
        let t2 = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = insertvalue {{ i64, i64 }} %{}, i64 {}, 1",
            indent, t2, t1, len
        )
        .ok();
        TypedRegister {
            name: t2,
            ty: Type::string(),
        }
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
        // {i64 data_ptr, i64 length, [N x i8] chars}, so that handle[1]
        // (getelementptr i64, ptr %handle, i64 1) reads the length field.
        // Do NOT add offset — emit_load_length expects the struct pointer.
        let str_p = self.fun.gen_reg();
        writeln!(out, "{}{} = bitcast <{{ i64, [{} x i8] }}>* {} to ptr",
            indent, str_p, bytes.len() + 1, g).ok();
        let p2i = self.fun.gen_reg();
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, p2i, str_p).ok();
        TypedRegister {
            name: p2i,
            ty: Type::int(),
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
    fn emit_struct_literal(
        &mut self,
        out: &mut String,
        v: &str,
        type_name: &str,
        fields: &[(String, Expr)],
        indent: &str,
    ) -> TypedRegister {
        let total_size = self.struct_type_size(type_name);
        let struct_ty = crate::ast::Type::Custom(type_name.to_string());

        let alloca_reg = self.fun.gen_reg();
        writeln!(out, "{}  {} = alloca i8, i64 {}", indent, alloca_reg, total_size).ok();

        for (field_name, field_expr) in fields {
            let fr = self.fun.gen_reg();
            let val = self.emit_expr_inner(out, &fr, field_expr, indent);
            let offset = self.lookup_field_offset(type_name, field_name);
            let ptr_reg = self.fun.gen_reg();
            writeln!(out, "{}  {} = getelementptr i8, ptr {}, i64 {}", indent, ptr_reg, alloca_reg, offset).ok();
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
        writeln!(out, "{}  {} = ptrtoint ptr {} to i64", indent, result, alloca_reg).ok();
        // 2026-07-24: Record the alloca pointer so &let_var on a struct-typed
        // binding retrieves the stack address, not the ptrtoint value.
        self.fun.struct_literal_allocas.insert(result.clone(), alloca_reg.clone());
        TypedRegister { name: result, ty: struct_ty }
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
    fn emit_struct_array(
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
        writeln!(out, "{}  {} = alloca i8, i64 {}", indent, alloca_reg, total_size).ok();

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
        writeln!(out, "{}  {} = ptrtoint ptr {} to i64", indent, result, alloca_reg).ok();
        self.fun.struct_literal_allocas.insert(result.clone(), alloca_reg.clone());
        TypedRegister { name: result, ty: Type::Custom(elem_type_name.to_string()) }
    }

    /// Look up the byte offset of a field in a struct definition.
    /// Get the fields of a struct type from the type universe or struct_types.
    /// 2026-07-24: Falls back to struct_types (registration pass) when the
    /// type universe is unavailable (common in test environments).
    fn get_struct_fields(&self, type_name: &str) -> Option<&[(String, Type)]> {
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
    fn struct_type_size(&self, type_name: &str) -> u64 {
        // Try type universe first
        if let Some(ref u) = self.ctx.type_universe {
            if let Some(info) = u.types.get(type_name) {
                if info.bytes > 0 {
                    return info.bytes;
                }
            }
        }
        // Fall back: compute from struct_types fields
        self.ctx.struct_types.get(type_name)
            .map(|fields| {
                fields.iter().map(|(_, ty)| types::type_size(ty, self.ctx.type_universe.as_ref())).sum()
            })
            .unwrap_or(8)
    }

    /// 2026-07-24: Computes offsets from field types using type_size (pack=1).
    /// Previously used simplified i*8 which was wrong for mixed-size fields.
    fn lookup_field_offset(&self, type_name: &str, field_name: &str) -> u64 {
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
            Some(crate::analysis::frgn_dispatch::ResolvedFrgn::Bridge { language, param_paths, return_path, fallback }) => {
                self.emit_bridge_frgn_call(out, v, sig, args, &language, &param_paths, &return_path, &fallback, indent)
            }
            Some(crate::analysis::frgn_dispatch::ResolvedFrgn::Unsupported(msg)) => {
                // 2026-07-22: Return a zero-value for the return type.
                // The error message is logged as a backend warning.
                self.warnings.push(format!("frgn '{}' unsupported: {}", sig.name, msg));
                let ret_type = sig.result_type.return_type().unwrap_or(Type::int());
                if ret_type != Type::Void {
                    let ret_llvm = self.llvm_type(&ret_type);
                    if ret_llvm == "ptr" {
                        writeln!(out, "{}  {} = inttoptr i64 0 to ptr", indent, v).ok();
                    } else if ret_llvm == "float" {
                        writeln!(out, "{}  {} = fadd float 0.0, 0.0", indent, v).ok();
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
                writeln!(out, "{}  {} = ptrtoint ptr {} to i64", indent, result, arg_reg.name).ok();
                TypedRegister { name: result, ty: Type::int() }
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
    /// Uses the `symbol` parameter (from `as_name` or brief_name) as the
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
                    // 2026-07-18: SSO String → C i8* shim. When SSO is ON, the String
                    // handle is {i64, i64} but C expects i8*. Extract handle[0] and
                    // inttoptr to i8*.
                    if self.feature_sso_strings
                        && (ty_name == "String" || ty_name == "Data")
                        && self
                            .ctx
                            .type_universe
                            .as_ref()
                            .map_or(false, |u| u.is_string_like(&arg.ty))
                    {
                        let extracted = self.fun.gen_reg();
                        writeln!(
                            out,
                            "{}  {} = extractvalue {{ i64, i64 }} {}, 0",
                            indent, extracted, arg.name
                        )
                        .ok();
                        let ptr_reg = self.fun.gen_reg();
                        writeln!(
                            out,
                            "{}  {} = inttoptr i64 {} to ptr",
                            indent, ptr_reg, extracted
                        )
                        .ok();
                        return TypedRegister {
                            name: ptr_reg,
                            ty: arg.ty.clone(),
                        };
                    }
                    if self
                        .ctx
                        .type_universe
                        .as_ref()
                        .and_then(|u| u.find_meld_to_extension(ty_name, ext_str))
                        .is_some()
                    {
                        // meld exists — convention compatible, identity conversion
                        arg.clone()
                    } else {
                        arg.clone()
                    }
                })
                .collect()
        };
        // 2026-07-24: Convert i64 args to ptr when the frgn param expects Ptr.
        // This handles PyModule_Create2(&moduledef, ...) where &moduledef returns
        // an i64 address but the C function expects a pointer parameter.
        let final_args: Vec<TypedRegister> = meld_args
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
        let ret_llvm = self.llvm_type(&ret_type);
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
        fallback: &crate::ast::top::Fallback,
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
        let ret_llvm = self.llvm_type(&ret_type);
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

        // 2026-07-22: Transform return value back to Brief type.
        let final_reg = if let Some(ret_path) = return_path {
            crate::glue::bridge::emit_protocol_chain(
                out, v, std::slice::from_ref(ret_path), &ret_llvm,
                &mut || self.fun.gen_reg(),
            ).unwrap_or_else(|_| v.to_string())
        } else {
            v.to_string()
        };

        // 2026-07-22: Apply fallback dispatch with full phi-node structure.
        // Uses self.fun.gen_reg() as the register generator for unique labels/registers.
        let result_reg = crate::glue::bridge::emit_fallback_llvm(
            out, &final_reg, &ret_type, &ret_llvm, fallback, indent,
            &mut || self.fun.gen_reg(),
        ).unwrap_or_else(|_| final_reg);

        TypedRegister {
            name: result_reg,
            ty: ret_type,
        }
    }

    /// Emit a user function call.
    /// 2026-07-17: defn functions expect (ptr %state, ...) as their first parameter.
    /// We must prepend the state pointer and adapt argument types from register
    /// types to the function's parameter types (via defn_params).
    fn emit_user_call(
        &mut self,
        out: &mut String,
        v: &str,
        name: &str,
        args: &[Expr],
        indent: &str,
    ) -> TypedRegister {
        // 2026-07-16: P5 — Check if this is a foreign function; if so, use emit_frgn_call
        // Clone the sig to avoid borrowing self.ctx while self.emit_expr needs &mut self.
        let frgn_sig = self.ctx.frgn_map.get(name).cloned();
        if let Some(sig) = frgn_sig {
            return self.emit_frgn_call(out, v, &sig, args, indent);
        }
        // 2026-07-14: collect typed registers so call includes argument types
        let arg_regs: Vec<TypedRegister> = args
            .iter()
            .map(|a| self.emit_expr(out, a, indent))
            .collect();
        // 2026-07-17: Look up defn parameter types for type adaptation.
        let defn_param_tys = self.ctx.defn_params.get(name);
        let is_defn = defn_param_tys.is_some();
        let mut call_args: Vec<String> = Vec::new();
        if is_defn {
            call_args.push("ptr %state".to_string());
            let param_tys = defn_param_tys.unwrap();
            for (i, reg) in arg_regs.iter().enumerate() {
                let reg_llvm_ty = self.llvm_type(&reg.ty);
                // 2026-07-17: Get the function's expected parameter type.
                // If available, use llvm_type() to determine the expected
                // LLVM type and insert conversions (i64 → ptr for String/Data).
                let param_llvm_ty = param_tys
                    .get(i)
                    .map(|pt| self.llvm_type(pt))
                    .unwrap_or_else(|| reg_llvm_ty.to_string());
                if param_llvm_ty == "ptr" && reg_llvm_ty == "i64" {
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
                call_args.push(format!("{} {}", self.llvm_type(&reg.ty), reg.name));
            }
        }
        // 2026-07-14: user call return type from defn_return_types — fall back to i64
        let ret_type = self
            .ctx
            .defn_return_types
            .get(name)
            .and_then(|types| types.first().cloned())
            .unwrap_or(Type::int());
        let ret_llvm = self.llvm_type(&ret_type);
        writeln!(
            out,
            "{}{} = call {} @{}({})",
            indent,
            v,
            ret_llvm,
            name,
            call_args.join(", ")
        )
        .ok();
        TypedRegister {
            name: v.to_string(),
            ty: ret_type,
        }
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
            writeln!(out, "{}{} = {}", indent, v, line).ok();
            Some(TypedRegister {
                name: v.to_string(),
                ty: ret_ty.clone(),
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

    // 2026-07-30: Unified cast resolution — physical vs semantic paths.
    // Physical: target is #Bit or Bit-derived → literal memory bytes.
    // Semantic: target is any other protocol → protocol pipeline.
    // Returns None when no protocol path exists (caller falls back to LLVM coercion).
    pub(super) fn resolve_cast(
        &mut self, out: &mut String, v: &str,
        src: &TypedRegister, target: &Type, indent: &str,
    ) -> Option<TypedRegister> {
        // Physical path: CastTo(#Bit) → literal memory bytes
        if self.is_protocol_member(target, "#Bit") {
            return self.resolve_physical_cast(out, v, src, indent);
        }
        // Semantic path: check operator_defs and protocol pipeline
        let src_name = type_name_str(&src.ty);
        let target_name = type_name_str(target);
        // Step 1: Direct op Cast(Target) on source type
        if let Some(ref name) = src_name {
            if let Some(impl_args) = find_cast_impl(self, name, "Cast") {
                let result = emit_simple_call(self, out, v, src, &impl_args, indent);
                return Some(TypedRegister { name: result, ty: target.clone() });
            }
        }
        // Step 2: CastTo(#Category) → CastFrom(#Category) protocol path
        if let (Some(ref s_name), Some(ref t_name)) = (src_name, target_name) {
            if let Some(impl_args) = try_cast_protocol_path(self, s_name, t_name) {
                let result = emit_simple_call(self, out, v, src, &impl_args, indent);
                return Some(TypedRegister { name: result, ty: target.clone() });
            }
        }
        // Step 3: Meld shuffle (structural bit remapping via @/ fields)
        let shuffle = self.resolve_shuffle_data(&src.ty);
        if let Some(ref data) = shuffle {
            if !data.is_empty() {
                return Some(emit_meld_shuffle(self, out, v, src, data, indent));
            }
        }
        // No protocol path found — caller falls back to LLVM coercion
        None
    }

    /// 2026-07-30: Physical cast — emit literal memory bytes of a value.
    /// For types with a custom CastTo(#Bit) operator, calls through the
    /// operator pipeline. Otherwise, bitcasts the register.
    fn resolve_physical_cast(
        &mut self, out: &mut String, v: &str,
        src: &TypedRegister, indent: &str,
    ) -> Option<TypedRegister> {
        // Check for custom CastTo(#Bit) operator on source type
        if let Some(name) = type_name_str(&src.ty) {
            // First try CastTo(#Bit) directly
            if let Some(impl_args) = find_cast_impl(self, &name, "CastTo") {
                let result = emit_simple_call(self, out, v, src, &impl_args, indent);
                return Some(TypedRegister { name: result, ty: src.ty.clone() });
            }
            // Fall back to op Cast(#Bit) (generic Cast)
            if let Some(impl_args) = find_cast_impl(self, &name, "Cast") {
                let result = emit_simple_call(self, out, v, src, &impl_args, indent);
                return Some(TypedRegister { name: result, ty: src.ty.clone() });
            }
        }
        // Fallback: bitcast the register to i64 (literal bytes)
        let src_ll = self.llvm_type(&src.ty);
        writeln!(out, "{}{} = bitcast {} {} to i64", indent, v, src_ll, src.name).ok();
        Some(TypedRegister { name: v.to_string(), ty: Type::int() })
    }

    /// 2026-07-30: Extract meld shuffle data from type universe properties.
    fn resolve_shuffle_data(&self, ty: &Type) -> Option<Vec<(u64, u64, u64)>> {
        let tu = self.ctx.type_universe.as_ref()?;
        let key = ty.universe_key()?;
        let rt = tu.get(key)?;
        let fields: Vec<String> = rt.properties.keys()
            .filter(|k| k.starts_with("shuffle.") && k.ends_with(".src_offset"))
            .map(|k| k.strip_prefix("shuffle.").unwrap()
                 .strip_suffix(".src_offset").unwrap().to_string())
            .collect();
        if fields.is_empty() { return None; }
        let data: Vec<(u64, u64, u64)> = fields.iter().map(|f| {
            let src_off = get_shuffle_int(&rt.properties, &format!("shuffle.{}.src_offset", f));
            let src_wid = get_shuffle_int(&rt.properties, &format!("shuffle.{}.src_width", f));
            let dst_off = get_shuffle_int(&rt.properties, &format!("shuffle.{}.dst_offset", f));
            (src_off, src_wid, dst_off)
        }).collect();
        Some(data)
    }
}

/// 2026-07-30: Extract integer from type properties for meld shuffle.
fn get_shuffle_int(properties: &std::collections::HashMap<String, crate::ast::PropertyValue>, key: &str) -> u64 {
    properties.get(key)
        .and_then(|pv| if let crate::ast::PropertyValue::Int(n) = pv { Some(*n as u64) } else { None })
        .unwrap_or(0)
}
