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
use crate::backend::llvm::intrinsics::{emit_intrinsic_call, template_for_op};
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
                writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::int(),
                }
            }
            Expr::TaggedLiteral(n, _) => {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::int(),
                }
            }
            Expr::Float(f) => {
                // 2026-07-17: Emit as float (32-bit) matching the typechecker.
                // Use a bitcast from the hex i32 pattern to avoid LLVM's
                // verifier rejecting high-precision literals — the string
                // "0.001660076642744037" has more significant digits than
                // f32 can represent, causing "floating point constant
                // invalid for type" even though the rounded value is valid.
                let h = crate::backend::llvm::float_to_llvm_hex(*f);
                let hex_reg = self.fun.gen_reg();
                let flt_reg = self.fun.gen_reg();
                writeln!(out, "{}{} = add i32 0, {}", indent, hex_reg, h).ok();
                writeln!(
                    out,
                    "{}{} = bitcast i32 {} to float",
                    indent, flt_reg, hex_reg
                )
                .ok();
                writeln!(out, "{}{} = fadd float 0.0, {}", indent, v, flt_reg).ok();
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
            Expr::Quoted(bytes) => self.emit_string_literal(out, v, bytes, indent),

            // ── Identifier ───────────────────────────────────────────
            Expr::Identifier(name) => {
                // 2026-07-17: Five paths: last_val_temps, local binding,
                // phi register, state field, global constant.
                //
                // Check last_val_temps FIRST — this catches values written
                // earlier in the same body iteration (e.g. count = count + 1
                // followed by guard [count % 5000000 == 0]). Without this,
                // the guard reads the phi register (start-of-iteration value)
                // instead of the updated value, producing wrong guard results.
                // The type is looked up from last_val_types (parallel map) or
                // falls back to field_index_map or Int.
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
                    TypedRegister {
                        name: reg.clone(),
                        ty: brief_ty,
                    }
                } else if let Some(reg) = self.get_local(name) {
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

            // ── Field access ─────────────────────────────────────────
            Expr::Field(obj, field) => {
                let obj_reg = self.emit_expr(out, obj, indent);
                // 2026-07-14: Layout field access — #fieldname triggers bit-shift/mask
                if field.starts_with('#') {
                    return self.emit_layout_field_read(out, v, &obj_reg, field, indent);
                }
                // Struct field access via extractvalue (numeric index required)
                let field_idx = self.resolve_field_index(&obj_reg.ty, field);
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
            Expr::Cast(expr, target) => {
                let src = self.emit_expr(out, expr, indent);
                let target_ll = self.llvm_type(target);
                let src_ll = self.llvm_type(&src.ty);
                // 2026-07-17: Priorities for cast dispatch:
                // 1. Ptr<T> target → inttoptr (never String — String/Data have Custom type)
                // 2. String/Data target → runtime helper
                // 3. i64 target + Ptr<T> source → ptrtoint
                // 4. i64 target + string-producing expr → __str_to_int
                // 5. double/i64 float conversions
                // 6. Generic bitcast
                if matches!(target, Type::Ptr(_)) {
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, v, src.name).ok();
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
                        } else {
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                            TypedRegister {
                                name: v.to_string(),
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

    // 2026-07-18: Legacy string literal emission (SSO OFF).
    // Stack-allocated [len+1 x i8] buffer, ptrtoint to i64, returned as Int type.
    fn emit_legacy_string_literal(
        &mut self,
        out: &mut String,
        v: &str,
        bytes: &[u8],
        indent: &str,
    ) -> TypedRegister {
        let len = bytes.len() + 1;
        let alloca = self.fun.gen_reg();
        writeln!(out, "{}{} = alloca [{} x i8], align 1", indent, alloca, len).ok();
        for (i, &b) in bytes.iter().enumerate() {
            let ptr = self.fun.gen_reg();
            writeln!(
                out,
                "{}{} = getelementptr inbounds [{} x i8], ptr {}, i32 0, i32 {}",
                indent, ptr, len, alloca, i
            )
            .ok();
            writeln!(out, "{}store i8 {}, ptr {}", indent, b, ptr).ok();
        }
        // null terminator
        let last = self.fun.gen_reg();
        writeln!(
            out,
            "{}{} = getelementptr inbounds [{} x i8], ptr {}, i32 0, i32 {}",
            indent,
            last,
            len,
            alloca,
            bytes.len()
        )
        .ok();
        writeln!(out, "{}store i8 0, ptr {}", indent, last).ok();
        writeln!(
            out,
            "{}{} = getelementptr inbounds [{} x i8], ptr {}, i32 0, i32 0",
            indent, v, len, alloca
        )
        .ok();
        // 2026-07-15: ptrtoint so callers see i64 (Brief universal type)
        let p2i = self.fun.gen_reg();
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, p2i, v).ok();
        TypedRegister {
            name: p2i,
            ty: Type::int(),
        }
    }

    /// 2026-07-16: P5 — Emit a foreign function call with optional auto-meld.
    /// Derives convention extension from sig.from, checks meld compatibility,
    /// and applies identity conversion (same bit layout, meld-verified type tag).
    fn emit_frgn_call(
        &mut self,
        out: &mut String,
        v: &str,
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
        let arg_strs: Vec<String> = meld_args
            .iter()
            .map(|reg| format!("{} {}", self.llvm_type(&reg.ty), reg.name))
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
                sig.name,
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
                "{}{} = call {} @{}({})",
                indent,
                v,
                ret_llvm,
                sig.name,
                arg_strs.join(", ")
            )
            .ok();
            TypedRegister {
                name: v.to_string(),
                ty: ret_type,
            }
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
            BinaryOpKind::Le => "Le",
            BinaryOpKind::Gt => "Gt",
            BinaryOpKind::Ge => "Ge",
            BinaryOpKind::And => "And",
            BinaryOpKind::Or => "Or",
            BinaryOpKind::Shl => "Shl",
            BinaryOpKind::Shr => "Shr",
            BinaryOpKind::BitAnd => "BitAnd",
            BinaryOpKind::BitOr => "BitOr",
            BinaryOpKind::BitXor => "BitXor",
            BinaryOpKind::Concat => "Concat",
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
        let is_float = l.ty == Type::float()
            || r.ty == Type::float()
            || l.ty == Type::float64()
            || r.ty == Type::float64();
        let is_double = l.ty == Type::float64() || r.ty == Type::float64();
        // 2026-07-17: Correct float type width — use "float" for Float (32-bit)
        // and "double" for Float64 (64-bit). The old code always used "double"
        // for all float operations, producing invalid IR when operands were
        // actually 32-bit float values loaded from constants or state fields.
        let ty_str = if is_double {
            "double"
        } else if is_float {
            "float"
        } else {
            "i64"
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
                    writeln!(out, "{}{} = add nuw nsw i64 {}, {}", indent, v, l.name, r.name).ok();
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
                    writeln!(out, "{}{} = sub nsw i64 {}, {}", indent, v, l.name, r.name).ok();
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
                    writeln!(out, "{}{} = mul nsw i64 {}, {}", indent, v, l.name, r.name).ok();
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
                    writeln!(out, "{}{} = sdiv i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister {
                    name: v.to_string(),
                    ty: ret_ty,
                }
            }
            crate::ast::BinaryOpKind::Mod => {
                writeln!(out, "{}{} = srem i64 {}, {}", indent, v, l.name, r.name).ok();
                TypedRegister {
                    name: v.to_string(),
                    ty: Type::int(),
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
                // 2026-07-19: Wire SSO concat — handle SSO inline (≤6 bytes)
                // and heap paths via emit_inline_concat. Previously fell through
                // to add i64, producing garbage for string ++.
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
}
