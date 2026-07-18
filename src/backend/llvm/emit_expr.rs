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

use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use crate::backend::llvm::intrinsics::emit_intrinsic_call;
use crate::ast::*;
use crate::backend::llvm::emit_stmt;
use crate::backend::llvm::types::lower_type;
use std::fmt::Write;

impl LlvmBackend {
    /// Emit LLVM IR for an expression. Returns the result register and type.
    /// This is the entry point for all expression codegen.
    pub(crate) fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> TypedRegister {
        let expr = expr.clone();
        let v = self.fun.gen_reg();
        let indent = if indent.is_empty() { "  " } else { indent };
        self.emit_expr_inner(out, &v, &expr, indent)
    }

    /// Inner dispatch: one arm per Expr variant.
    fn emit_expr_inner(&mut self, out: &mut String, v: &str, expr: &Expr, indent: &str) -> TypedRegister {
        match expr {
            // ── Literals ─────────────────────────────────────────────
            Expr::Decimal(n) => {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
                TypedRegister { name: v.to_string(), ty: Type::int() }
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
                writeln!(out, "{}{} = bitcast i32 {} to float", indent, flt_reg, hex_reg).ok();
                writeln!(out, "{}{} = fadd float 0.0, {}", indent, v, flt_reg).ok();
                TypedRegister { name: v.to_string(), ty: Type::float() }
            }
            Expr::Bool(b) => {
                let val = if *b { 1 } else { 0 };
                writeln!(out, "{}{} = add i8 0, {}", indent, v, val).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            Expr::Quoted(bytes) => {
                self.emit_string_literal(out, v, bytes, indent)
            }

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
                    let brief_ty = self.fun.last_val_types.get(name)
                        .cloned()
                        .or_else(|| self.ctx.field_index_map.get(name)
                            .and_then(|idx| self.ctx.field_brief_types.get(*idx).cloned()))
                        .unwrap_or(Type::int());
                    TypedRegister { name: reg.clone(), ty: brief_ty }
                } else if let Some(reg) = self.get_local(name) {
                    TypedRegister { name: reg.clone(), ty: self.get_local_type(name) }
                } else if let Some(phi_reg_str) = self.fun.phi_field_regs.get(name).cloned() {
                    let brief_ty = self.ctx.field_index_map.get(name)
                        .and_then(|idx| self.ctx.field_brief_types.get(*idx).cloned())
                        .unwrap_or(Type::int());
                    if brief_ty == Type::float64() {
                        let dbl = self.fun.gen_reg();
                        writeln!(out, "{}{} = bitcast i64 {} to double", indent, dbl, phi_reg_str).ok();
                        self.fun.reg_float_cache.insert(phi_reg_str, dbl.clone());
                        TypedRegister { name: dbl, ty: Type::float64() }
                    } else if brief_ty == Type::float() {
                        let tr = self.fun.gen_reg();
                        let fl = self.fun.gen_reg();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, phi_reg_str).ok();
                        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
                        self.fun.reg_float_cache.insert(phi_reg_str, fl.clone());
                        TypedRegister { name: fl, ty: Type::float() }
                    } else {
                        TypedRegister { name: phi_reg_str, ty: brief_ty }
                    }
                } else if let Some(&idx) = self.ctx.field_index_map.get(name) {
                    let gep = self.fun.gen_reg();
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                        indent, gep, idx).ok();
                    let brief_ty = self.ctx.field_brief_types.get(idx)
                        .cloned().unwrap_or(Type::int());
                    writeln!(out, "{}{} = load i64, ptr {}", indent, v, gep).ok();
                    // 2026-07-17: Float fields packed as i64 in state — unbox.
                    if brief_ty == Type::float64() {
                        let dbl = self.fun.gen_reg();
                        writeln!(out, "{}{} = bitcast i64 {} to double", indent, dbl, v).ok();
                        self.fun.reg_float_cache.insert(v.to_string(), dbl.clone());
                        TypedRegister { name: dbl, ty: Type::float64() }
                    } else if brief_ty == Type::float() {
                        let tr = self.fun.gen_reg();
                        let fl = self.fun.gen_reg();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, v).ok();
                        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
                        self.fun.reg_float_cache.insert(v.to_string(), fl.clone());
                        TypedRegister { name: fl, ty: Type::float() }
                    } else {
                        TypedRegister { name: v.to_string(), ty: brief_ty }
                    }
                } else if let Some((ty, _)) = self.ctx.constants.get(name) {
                    // 2026-07-17: Load global constants with the correct LLVM
                    // type. Float constants are declared as `constant float` in
                    // the IR (not i64), so loading them as i64 produces garbage
                    // bits and type mismatches in float operations.
                    if *ty == Type::float() {
                        writeln!(out, "{}{} = load float, ptr @{}", indent, v, name).ok();
                        TypedRegister { name: v.to_string(), ty: Type::float() }
                    } else if *ty == Type::float64() {
                        writeln!(out, "{}{} = load double, ptr @{}", indent, v, name).ok();
                        TypedRegister { name: v.to_string(), ty: Type::float64() }
                    } else {
                        writeln!(out, "{}{} = load i64, ptr @{}", indent, v, name).ok();
                        TypedRegister { name: v.to_string(), ty: Type::int() }
                    }
                } else {
                    writeln!(out, "{}{} = load i64, ptr @{}", indent, v, name).ok();
                    TypedRegister { name: v.to_string(), ty: Type::int() }
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
                // 2026-07-17: Mod with small constant divisor — trunc to i32
                // so LLVM uses the cheaper imulq-based div magic (1 uop, 3c)
                // instead of the full 128-bit mulq (3 uops, 4c). Without this,
                // urem i64 forces LLVM to use mulq for the 64-bit magic constant
                // division, adding ~2 uops per iteration.
                // 2026-07-17: Mod with small constant divisor — trunc to i32
                // so LLVM uses the cheaper imulq-based div magic (1 uop, 3c)
                // instead of the full 128-bit mulq (3 uops, 4c).
                if matches!(kind, crate::ast::BinaryOpKind::Mod) {
                    if let Expr::Decimal(n) = rhs.as_ref() {
                        if *n > 0 && *n < (1i64 << 31) {
                            let l = self.emit_expr(out, lhs, indent);
                            let tr = self.fun.gen_reg();
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, l.name).ok();
                            // Emit the divisor constant directly as i32
                            let ur = self.fun.gen_reg();
                            writeln!(out, "{}{} = urem i32 {}, {}", indent, ur, tr, n).ok();
                            let rz = self.fun.gen_reg();
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, rz, ur).ok();
                            return TypedRegister { name: rz.to_string(), ty: Type::int() };
                        }
                    }
                }
                let l = self.emit_expr(out, lhs, indent);
                let r = self.emit_expr(out, rhs, indent);
                self.emit_binary_op(out, v, kind, &l, &r, indent)
            }

            // ── UnaryOp ──────────────────────────────────────────────
            Expr::UnaryOp(kind, e) => {
                let operand = self.emit_expr(out, e, indent);
                self.emit_unary_op(out, v, kind, &operand, indent)
            }

            // ── Block ────────────────────────────────────────────────
            Expr::Block(stmts) => {
                let mut last = TypedRegister { name: v.to_string(), ty: Type::void() };
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
                writeln!(out, "{}br i1 {}, label %{}, label %{}",
                    indent, cond_reg.name, then_lbl, else_lbl).ok();
                writeln!(out, "{}{}:", indent, then_lbl).ok();
                let then_reg = self.emit_expr(out, then, indent);
                writeln!(out, "{}br label %{}", indent, end_lbl).ok();
                writeln!(out, "{}{}:", indent, else_lbl).ok();
                let else_reg = match else_ {
                    Some(e) => self.emit_expr(out, e, indent),
                    None => TypedRegister { name: self.fun.gen_reg(), ty: Type::void() },
                };
                writeln!(out, "{}br label %{}", indent, end_lbl).ok();
                writeln!(out, "{}{}:", indent, end_lbl).ok();
                TypedRegister { name: v.to_string(), ty: then_reg.ty }
            }

            // ── Tuple ────────────────────────────────────────────────
            Expr::Tuple(exprs) => {
                self.emit_heap_seq(out, v, exprs, indent)
            }

            // ── List literal ─────────────────────────────────────────
            Expr::List(exprs) => {
                self.emit_heap_seq(out, v, exprs, indent)
            }

            // ── Field access ─────────────────────────────────────────
            Expr::Field(obj, field) => {
                let obj_reg = self.emit_expr(out, obj, indent);
                // 2026-07-14: Layout field access — #fieldname triggers bit-shift/mask
                if field.starts_with('#') {
                    return self.emit_layout_field_read(out, v, &obj_reg, field, indent);
                }
                // Struct field access via extractvalue or GEP
                writeln!(out, "{}{} = extractvalue {} {}, {}", indent, v,
                    lower_type(&obj_reg.ty), obj_reg.name, field).ok();
                TypedRegister { name: v.to_string(), ty: Type::int() }
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
                if matches!(obj_reg.ty, Type::Ptr(_)) {
                    let ptr = self.fun.gen_reg();
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, obj_reg.name).ok();
                    let offset = self.fun.gen_reg();
                    // 2026-07-17: List/tuple literals have a length header at
                    // slot 0 — elements start at index 1. Raw Ptr buffers
                    // (from Malloc#) have no header.
                    if matches!(obj.as_ref(), Expr::List(_) | Expr::Tuple(_)) {
                        writeln!(out, "{}{} = add i64 {}, 1", indent, offset, idx_reg.name).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 {}, 0", indent, offset, idx_reg.name).ok();
                    }
                    let gep = self.fun.gen_reg();
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, gep, ptr, offset).ok();
                    writeln!(out, "{}{} = load i64, ptr {}", indent, v, gep).ok();
                } else {
                    writeln!(out, "{}{} = extractelement {} {}, {}",
                        indent, v, lower_type(&obj_reg.ty), obj_reg.name, idx_reg.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: Type::int() }
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
                let target_ll = lower_type(target);
                let src_ll = lower_type(&src.ty);
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
                        writeln!(out, "{}{} = call i64 @__int_to_str__(i64 {})", indent, v, src.name).ok();
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
                } else {
                    writeln!(out, "{}{} = bitcast {} {} to {}", indent, v, src_ll, src.name, target_ll).ok();
                }
                TypedRegister { name: v.to_string(), ty: target.clone() }
            }

            // ── IsType ───────────────────────────────────────────────
            Expr::IsType(_, _) => {
                writeln!(out, "{}{} = add i8 0, 1", indent, v).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }

            // ── Within ───────────────────────────────────────────────
            Expr::Within(expr, _) => self.emit_expr(out, expr, indent),

            // ── Match ────────────────────────────────────────────────
            Expr::Match(_, arms) => {
                if let Some(first) = arms.first() {
                    self.emit_expr(out, &first.body, indent)
                } else {
                    TypedRegister { name: v.to_string(), ty: Type::void() }
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
                            let gep = self.fun.gen_reg();
                            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                                indent, gep, idx).ok();
                            let ptr = self.fun.gen_reg();
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, ptr, gep).ok();
                            TypedRegister { name: ptr, ty: Type::int() }
                        } else {
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                            TypedRegister { name: v.to_string(), ty: Type::int() }
                        }
                    }
                    _ => {
                        let inner_reg = self.emit_expr(out, inner, indent);
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, inner_reg.name).ok();
                        TypedRegister { name: v.to_string(), ty: Type::int() }
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
                let llvm_ty = lower_type(&pointee_ty);
                writeln!(out, "{}{} = load {}, ptr {}, align 8", indent, v, llvm_ty, ptr_reg.name).ok();
                TypedRegister { name: v.to_string(), ty: pointee_ty }
            }

            // ── DerivationBlock / PropertyGet / FormattingAnnotation ─
            Expr::DerivationBlock(_) | Expr::PropertyGet(_) | Expr::FormattingAnnotation(_) => {
                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                TypedRegister { name: v.to_string(), ty: Type::void() }
            }
        }
    }

    // ── Sub-helpers ──────────────────────────────────────────────────

    /// 2026-07-14: Emit a heap-allocated sequence (list/tuple) with 2-slot header.
    /// Protocol: slot 0 = length (i64), slots 1..N = elements.
    /// Empty seq → @ll_empty_list global sentinel.
    /// Non-empty → malloc((2+N)*8), bitcast, store N, store elements, ptrtoint.
    fn emit_heap_seq(&mut self, out: &mut String, v: &str, exprs: &[Expr], indent: &str) -> TypedRegister {
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
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, slot, hdr, i + 1).ok();
                writeln!(out, "{}store i64 {}, ptr {}", indent, e.name, slot).ok();
            }
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, hdr).ok();
        }
        TypedRegister { name: v.to_string(), ty: Type::ptr(Type::int()) }
    }

    /// Emit a string literal as stack-allocated bytes + GEP.
    /// 2026-07-14: Use alloca instead of global constant to avoid placement
    /// issues (globals must be at module level, not inside functions).
    fn emit_string_literal(&mut self, out: &mut String, v: &str, bytes: &[u8], indent: &str) -> TypedRegister {
        let len = bytes.len() + 1;
        let alloca = self.fun.gen_reg();
        writeln!(out, "{}{} = alloca [{} x i8], align 1", indent, alloca, len).ok();
        for (i, &b) in bytes.iter().enumerate() {
            let ptr = self.fun.gen_reg();
            writeln!(out, "{}{} = getelementptr inbounds [{} x i8], ptr {}, i32 0, i32 {}",
                indent, ptr, len, alloca, i).ok();
            writeln!(out, "{}store i8 {}, ptr {}", indent, b, ptr).ok();
        }
        // null terminator
        let last = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr inbounds [{} x i8], ptr {}, i32 0, i32 {}",
            indent, last, len, alloca, bytes.len()).ok();
        writeln!(out, "{}store i8 0, ptr {}", indent, last).ok();
        writeln!(out, "{}{} = getelementptr inbounds [{} x i8], ptr {}, i32 0, i32 0",
            indent, v, len, alloca).ok();
        // 2026-07-15: ptrtoint so callers see i64 (Brief universal type)
        let p2i = self.fun.gen_reg();
        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, p2i, v).ok();
        TypedRegister { name: p2i, ty: Type::int() }
    }

    /// 2026-07-16: P5 — Emit a foreign function call with optional auto-meld.
    /// Derives convention extension from sig.from, checks meld compatibility,
    /// and applies identity conversion (same bit layout, meld-verified type tag).
    fn emit_frgn_call(&mut self, out: &mut String, v: &str, sig: &crate::ast::ForeignSignature, args: &[Expr], indent: &str) -> TypedRegister {
        let arg_regs: Vec<TypedRegister> = args.iter()
            .map(|a| self.emit_expr(out, a, indent))
            .collect();
        let ext = sig.from.extension();
        let ext_str = ext.as_deref().unwrap_or("");
        // 2026-07-16: Apply meld forward on each arg (identity conversion for now)
        let meld_args: Vec<TypedRegister> = if ext_str.is_empty() {
            arg_regs
        } else {
            arg_regs.iter().zip(sig.inputs.iter()).map(|(arg, (_, param_ty))| {
                let ty_name = match param_ty {
                    crate::ast::Type::Custom(name) => name.as_str(),
                    _ => return arg.clone(),
                };
                if self.ctx.type_universe.as_ref().and_then(|u| u.find_meld_to_extension(ty_name, ext_str)).is_some() {
                    // meld exists — convention compatible, identity conversion
                    arg.clone()
                } else {
                    arg.clone()
                }
            }).collect()
        };
        let arg_strs: Vec<String> = meld_args.iter()
            .map(|reg| format!("{} {}", crate::backend::llvm::types::lower_type(&reg.ty), reg.name))
            .collect();
        let ret_type = sig.result_type.return_type().unwrap_or(Type::int());
        // 2026-07-16: Meld inverse on return value (identity for now)
        let ret_llvm = crate::backend::llvm::types::lower_type(&ret_type);
        writeln!(out, "{}{} = call {} @{}({})", indent, v, ret_llvm, sig.name, arg_strs.join(", ")).ok();
        TypedRegister { name: v.to_string(), ty: ret_type }
    }

    /// Emit a user function call.
    /// 2026-07-17: defn functions expect (ptr %state, ...) as their first parameter.
    /// We must prepend the state pointer and adapt argument types from register
    /// types to the function's parameter types (via defn_params).
    fn emit_user_call(&mut self, out: &mut String, v: &str, name: &str, args: &[Expr], indent: &str) -> TypedRegister {
        // 2026-07-16: P5 — Check if this is a foreign function; if so, use emit_frgn_call
        // Clone the sig to avoid borrowing self.ctx while self.emit_expr needs &mut self.
        let frgn_sig = self.ctx.frgn_map.get(name).cloned();
        if let Some(sig) = frgn_sig {
            return self.emit_frgn_call(out, v, &sig, args, indent);
        }
        // 2026-07-14: collect typed registers so call includes argument types
        let arg_regs: Vec<TypedRegister> = args.iter()
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
                let reg_llvm_ty = lower_type(&reg.ty);
                // 2026-07-17: Get the function's expected parameter type.
                // If available, use llvm_type() to determine the expected
                // LLVM type and insert conversions (i64 → ptr for String/Data).
                let param_llvm_ty = param_tys.get(i)
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
                call_args.push(format!("{} {}", lower_type(&reg.ty), reg.name));
            }
        }
        // 2026-07-14: user call return type from defn_return_types — fall back to i64
        let ret_type = self.ctx.defn_return_types.get(name)
            .and_then(|types| types.first().cloned())
            .unwrap_or(Type::int());
        let ret_llvm = lower_type(&ret_type);
        writeln!(out, "{}{} = call {} @{}({})", indent, v, ret_llvm, name, call_args.join(", ")).ok();
        TypedRegister { name: v.to_string(), ty: ret_type }
    }

    /// Emit a binary operation.
    fn emit_binary_op(&mut self, out: &mut String, v: &str,
        kind: &crate::ast::BinaryOpKind, l: &TypedRegister, r: &TypedRegister, indent: &str) -> TypedRegister {
        let is_float = l.ty == Type::float() || r.ty == Type::float() 
            || l.ty == Type::float64() || r.ty == Type::float64();
        let is_double = l.ty == Type::float64() || r.ty == Type::float64();
        // 2026-07-17: Correct float type width — use "float" for Float (32-bit)
        // and "double" for Float64 (64-bit). The old code always used "double"
        // for all float operations, producing invalid IR when operands were
        // actually 32-bit float values loaded from constants or state fields.
        let ty_str = if is_double { "double" } else if is_float { "float" } else { "i64" };
        let fast = if is_float { " fast" } else { "" };
        let mut ret_ty = if is_double { Type::float64() } else if is_float { Type::float() } else { Type::int() };
        match kind {
            crate::ast::BinaryOpKind::Add => {
                // 2026-07-17: Pointer-offset arithmetic: `buf + N` emits GEP.
                // When one operand is Ptr<T> and the other is Int, emit:
                //   %gep = getelementptr T, ptr %ptr, i64 %offset
                // preserving the pointer type for subsequent dereference.
                if matches!(l.ty, Type::Ptr(_)) && !is_float {
                    let ptr_ty = match &l.ty { Type::Ptr(i) => *i.clone(), _ => Type::int() };
                    writeln!(out, "{}{} = getelementptr {}, ptr {}, i64 {}", indent, v,
                        crate::backend::llvm::types::lower_type(&ptr_ty), l.name, r.name).ok();
                    ret_ty = l.ty.clone();
                } else if matches!(r.ty, Type::Ptr(_)) && !is_float {
                    let ptr_ty = match &r.ty { Type::Ptr(i) => *i.clone(), _ => Type::int() };
                    writeln!(out, "{}{} = getelementptr {}, ptr {}, i64 {}", indent, v,
                        crate::backend::llvm::types::lower_type(&ptr_ty), r.name, l.name).ok();
                    ret_ty = r.ty.clone();
                } else if is_float {
                    writeln!(out, "{}{} = fadd{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = add nsw i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: ret_ty }
            }
            crate::ast::BinaryOpKind::Sub => {
                // 2026-07-14: Sub must branch on is_float — fsub i64 is invalid LLVM IR
                if is_float {
                    writeln!(out, "{}{} = fsub{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = sub nsw i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: ret_ty }
            }
            crate::ast::BinaryOpKind::Mul => {
                // 2026-07-14: Mul must branch on is_float — fmul i64 is invalid LLVM IR
                if is_float {
                    writeln!(out, "{}{} = fmul{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = mul nsw i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: ret_ty }
            }
            crate::ast::BinaryOpKind::Div => {
                if is_float {
                    writeln!(out, "{}{} = fdiv{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = sdiv i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: ret_ty }
            }
            crate::ast::BinaryOpKind::Mod => {
                writeln!(out, "{}{} = srem i64 {}, {}", indent, v, l.name, r.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::int() }
            }
            crate::ast::BinaryOpKind::Eq => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(out, "{}{} = fcmp oeq {} {}, {}", indent, icmp, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, icmp, l.name, r.name).ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Neq => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(out, "{}{} = fcmp one {} {}, {}", indent, icmp, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp ne i64 {}, {}", indent, icmp, l.name, r.name).ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Lt => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(out, "{}{} = fcmp olt {} {}, {}", indent, icmp, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, icmp, l.name, r.name).ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Le => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(out, "{}{} = fcmp ole {} {}, {}", indent, icmp, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, icmp, l.name, r.name).ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Gt => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(out, "{}{} = fcmp ogt {} {}, {}", indent, icmp, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, icmp, l.name, r.name).ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Ge => {
                let icmp = self.fun.gen_reg();
                if is_float {
                    writeln!(out, "{}{} = fcmp oge {} {}, {}", indent, icmp, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, icmp, l.name, r.name).ok();
                }
                writeln!(out, "{}{} = zext i1 {} to i8", indent, v, icmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::And => {
                writeln!(out, "{}{} = and i8 {}, {}", indent, v, l.name, r.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Or => {
                writeln!(out, "{}{} = or i8 {}, {}", indent, v, l.name, r.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            _ => {
                writeln!(out, "{}{} = add i64 {}, {}", indent, v, l.name, r.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::int() }
            }
        }
    }

    /// Emit a unary operation.
    fn emit_unary_op(&mut self, out: &mut String, v: &str,
        kind: &crate::ast::UnaryOpKind, operand: &TypedRegister, indent: &str) -> TypedRegister {
        match kind {
            crate::ast::UnaryOpKind::Neg => {
                // 2026-07-14: Neg must use fsub for float operands — sub i64 is invalid for doubles
                let is_float = operand.ty == Type::float() || operand.ty == Type::float64();
                if is_float {
                    let fty = if operand.ty == Type::float64() { "double" } else { "float" };
                    writeln!(out, "{}{} = fsub {} -0.0, {}", indent, v, fty, operand.name).ok();
                } else {
                    writeln!(out, "{}{} = sub i64 0, {}", indent, v, operand.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: operand.ty.clone() }
            }
            crate::ast::UnaryOpKind::Not => {
                writeln!(out, "{}{} = xor i8 {}, 1", indent, v, operand.name).ok();
                TypedRegister { name: v.to_string(), ty: operand.ty.clone() }
            }
            crate::ast::UnaryOpKind::BitNot => {
                writeln!(out, "{}{} = xor i64 {}, -1", indent, v, operand.name).ok();
                TypedRegister { name: v.to_string(), ty: operand.ty.clone() }
            }
        }
    }

    /// Emit a statement (delegates to emit_stmt module).
    pub(crate) fn emit_statement(&mut self, out: &mut String, stmt: &Statement, indent: &str) -> TypedRegister {
        crate::backend::llvm::emit_stmt::emit_statement(self, out, stmt, indent)
    }

    /// Get a local variable's register name from FunctionContext.
    fn get_local(&self, name: &str) -> Option<String> {
        self.fun.let_bindings.get(name).cloned()
    }

    /// Get a local variable's type from FunctionContext.
    fn get_local_type(&self, name: &str) -> Type {
        self.fun.let_binding_types.get(name).cloned().unwrap_or(Type::int())
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
        &mut self, out: &mut String, v: &str,
        obj_reg: &TypedRegister, field: &str, indent: &str,
    ) -> TypedRegister {
        let field_name = &field[1..];
        let offset_key = format!("field.{}.offset", field_name);
        let width_key = format!("field.{}.width", field_name);
        let (offset, width) = self.ctx.type_universe.as_ref()
            .and_then(|u| crate::type_universe::resolve_type(u, &obj_reg.ty))
            .map(|rt| {
                let off = rt.properties.get(&offset_key)
                    .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(0);
                let wid = rt.properties.get(&width_key)
                    .and_then(|pv| if let PropertyValue::Int(n) = pv { Some(*n as u64) } else { None }).unwrap_or(64);
                (off, wid)
            })
            .unwrap_or((0, 64));

        if offset == 0 && width == 64 {
            return TypedRegister { name: obj_reg.name.clone(), ty: Type::int() };
        }
        let shifted = self.fun.gen_reg();
        writeln!(out, "{}{} = lshr {} {}, {}", indent, shifted,
            lower_type(&obj_reg.ty), obj_reg.name, offset).ok();
        if width < 64 {
            let mask = (1u128 << width).wrapping_sub(1);
            writeln!(out, "{}{} = and {} {}, {}", indent, v,
                lower_type(&obj_reg.ty), shifted, mask).ok();
            return TypedRegister { name: v.to_string(), ty: Type::int() };
        }
        TypedRegister { name: shifted, ty: Type::int() }
    }
}
