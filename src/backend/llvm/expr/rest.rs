// ── Remaining Expression Dispatcher ──────────────────────────
//
// 2026-06-29: Extracted from emit_expr.rs. Handles ALL expression
// types not handled by dedicated submodules (literal, math, compare,
// collections, intrinsics).

use crate::ast::{ArrowDir, BracketOp, Expr, Intrinsic, MatchArm, MatchPattern, OutputType, Pattern, PipeChain, PipeStep, ProjectionTarget, SliceCoordinate, Statement, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use crate::features::arrow::{ArrowMutExpr, ArrowDiscardExpr, ArrowTransferExpr};
use crate::features::binary_op::BinaryOpExpr;
use crate::features::block::BlockExpr;
use crate::features::call::CallExpr;
use crate::features::collection::{ListLiteralExpr, MapLiteralExpr, MultiSliceExpr, SetLiteralExpr, SliceExpr};
use crate::features::ellipsis::EllipsisExpr;
use crate::features::field::{FieldAccessExpr, ObjectLiteralExpr, StructInstanceExpr};
use crate::features::pattern::{MatchExpr, PatternMatchExpr};
use crate::features::projection::ProjectionExpr;
use crate::features::sigcall::SigCallExpr;
use crate::features::subtype::SubtypeProjectionExpr;
use crate::features::traits::{ExprCodegenLLVM, ExprDispatch};
use crate::features::tuple::{TupleDestructureExpr, TupleExpr};
use crate::features::unary_op::UnaryOpExpr;
use std::collections::HashMap;
use std::fmt::Write;

/// Emit any expression variant not handled by dedicated submodules.
pub fn emit_rest_expr(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    expr: &Expr,
    indent: &str,
) -> TypedRegister {
        match &expr {
            // 2026-06-29: Literal expressions dispatched to expr::literal submodule.
            // Keeps emit_expr focused on dispatching rather than implementation.
            Expr::Integer(_) => return crate::backend::llvm::expr::literal::emit_integer(backend, out, &v, &expr, indent),
            Expr::IntegerSuffixed(_, _) => return crate::backend::llvm::expr::literal::emit_integer_suffixed(backend, out, &v, &expr, indent),
            Expr::Bool(_) => return crate::backend::llvm::expr::literal::emit_bool(backend, out, &v, &expr, indent),
            Expr::Float64(_) => return crate::backend::llvm::expr::literal::emit_float64(backend, out, &v, &expr, indent),
            Expr::Float(_) => return crate::backend::llvm::expr::literal::emit_float(backend, out, &v, &expr, indent),
            Expr::String(_) | Expr::RegexLiteral(_) => return crate::backend::llvm::expr::literal::emit_string(backend, out, &v, &expr, indent),
            Expr::Char(_) => return crate::backend::llvm::expr::literal::emit_char(backend, out, &v, &expr, indent),
            Expr::Term => return crate::backend::llvm::expr::literal::emit_term(backend, out, &v, indent),
            // 2026-06-29: Arithmetic and bitwise expressions dispatched to expr::math
            Expr::BinaryOp(bop) => {
                let mut builder = crate::backend::llvm::LLVMBuilder::new();
                let mut emit_expr = |_backend: &mut crate::backend::llvm::LlvmBackend,
                                     _out: &mut String,
                                     _builder: &mut crate::backend::llvm::LLVMBuilder,
                                     _expr: &crate::ast::Expr,
                                     _indent: &str| {
                    crate::backend::llvm::TypedRegister { name: "%stub".into(), ty: crate::ast::Type::Custom("Int".to_string()) }
                };
                let result = bop.emit_llvm(backend, out, &mut builder, &ExprDispatch, &mut emit_expr);
                builder.finish_into(out, indent.len() as usize);
                return result;
            }
            Expr::UnaryOp(uop) => {
                let mut builder = crate::backend::llvm::LLVMBuilder::new();
                let mut emit_expr = |_backend: &mut crate::backend::llvm::LlvmBackend,
                                     _out: &mut String,
                                     _builder: &mut crate::backend::llvm::LLVMBuilder,
                                     _expr: &crate::ast::Expr,
                                     _indent: &str| {
                    crate::backend::llvm::TypedRegister { name: "%stub".into(), ty: crate::ast::Type::Custom("Int".to_string()) }
                };
                let result = uop.emit_llvm(backend, out, &mut builder, &ExprDispatch, &mut emit_expr);
                builder.finish_into(out, indent.len() as usize);
                return result;
            }
            Expr::Literal(lit) => {
                let mut builder = crate::backend::llvm::LLVMBuilder::new();
                let mut emit_expr = |_backend: &mut crate::backend::llvm::LlvmBackend,
                                     _out: &mut String,
                                     _builder: &mut crate::backend::llvm::LLVMBuilder,
                                     _expr: &crate::ast::Expr,
                                     _indent: &str| {
                    crate::backend::llvm::TypedRegister { name: "%stub".into(), ty: crate::ast::Type::Custom("Int".to_string()) }
                };
                let result = lit.emit_llvm(backend, out, &mut builder, &ExprDispatch, &mut emit_expr);
                builder.finish_into(out, indent.len() as usize);
                return result;
            }
            Expr::Add(_, _) => return crate::backend::llvm::expr::math::emit_add(backend, out, &v, &expr, indent),
            Expr::Sub(_, _) => return crate::backend::llvm::expr::math::emit_sub(backend, out, &v, &expr, indent),
            Expr::Mul(_, _) => return crate::backend::llvm::expr::math::emit_mul(backend, out, &v, &expr, indent),
            Expr::Div(_, _) => return crate::backend::llvm::expr::math::emit_div(backend, out, &v, &expr, indent),
            Expr::Mod(_, _) => return crate::backend::llvm::expr::math::emit_mod(backend, out, &v, &expr, indent),
            Expr::Neg(_) => return crate::backend::llvm::expr::math::emit_neg(backend, out, &v, &expr, indent),
            Expr::BitAnd(_, _) => return crate::backend::llvm::expr::math::emit_bitand(backend, out, &v, &expr, indent),
            Expr::BitOr(_, _) => return crate::backend::llvm::expr::math::emit_bitor(backend, out, &v, &expr, indent),
            Expr::BitXor(_, _) => return crate::backend::llvm::expr::math::emit_bitxor(backend, out, &v, &expr, indent),
            Expr::BitNot(_) => return crate::backend::llvm::expr::math::emit_bitnot(backend, out, &v, &expr, indent),
            Expr::Shl(_, _) => return crate::backend::llvm::expr::math::emit_shl(backend, out, &v, &expr, indent),
            Expr::Shr(_, _) => return crate::backend::llvm::expr::math::emit_shr(backend, out, &v, &expr, indent),
            // Comparisons & logical ops dispatched to expr::compare
            Expr::Eq(l, r) => { return backend.emit_fcmp(out, indent, l, r, "oeq"); }
            Expr::Ne(l, r) => { return backend.emit_fcmp(out, indent, l, r, "one"); }
            Expr::Lt(l, r) => { return backend.emit_fcmp(out, indent, l, r, "olt"); }
            Expr::Le(l, r) => { return backend.emit_fcmp(out, indent, l, r, "ole"); }
            Expr::Gt(l, r) => { return backend.emit_fcmp(out, indent, l, r, "ogt"); }
            Expr::Ge(l, r) => { return backend.emit_fcmp(out, indent, l, r, "oge"); }
            Expr::And(_, _) => return crate::backend::llvm::expr::compare::emit_and(backend, out, &v, &expr, indent),
            Expr::Or(_, _) => return crate::backend::llvm::expr::compare::emit_or(backend, out, &v, &expr, indent),
            Expr::Not(_) => return crate::backend::llvm::expr::compare::emit_not(backend, out, &v, &expr, indent),
            // ── Identifier / AddrOf / PriorState ────────────────
            Expr::Identifier(_) | Expr::PriorState(_) => {
                return crate::backend::llvm::expr::identifier::emit_identifier(
                    backend, out, v, expr, indent);
            }
            // 2026-06-29: Arithmetic/comparison/logical/bitwise dispatched to expr submodules
            Expr::Concat(l, r) => { let (a, b) = (backend.emit_expr(out, l, indent), backend.emit_expr(out, r, indent)); return backend.emit_inline_concat(out, indent, &a, &b); }
            // Call
            Expr::Call(name, args) => {
                return crate::backend::llvm::expr::call::emit_call(backend, out, v, name, args, indent);
            }
            // ── IntrinsicCall ────────────────────────────────────
            // ── IntrinsicCall ────────────────────────────────────
            Expr::IntrinsicCall { intrinsic, args } => {
                return crate::backend::llvm::expr::intrinsics::emit_intrinsic_call(backend, out, &v, intrinsic, args, indent);
            }
            // ── ListLiteral ──────────────────────────────────────
            Expr::ListLiteral(_) => return crate::backend::llvm::expr::collections::emit_list_literal(backend, out, &v, &expr, indent),
            Expr::Tuple(_) => return crate::backend::llvm::expr::collections::emit_tuple(backend, out, &v, &expr, indent),
            // ── ListIndex ───────────────────────────────────────
            // 2026-06-27: propagate element type from the list's type so that
            // downstream FieldAccess can resolve struct fields (e.g. `rules[i].slot_count`).
            // Without this, the result is Type::Custom("Int".to_string()) and the struct lookup fails.
            Expr::ListIndex(list, index) => {
                let list_val = backend.emit_expr(out, list, indent);
                let idx_val = backend.emit_expr(out, index, indent);
                let hp = format!("%xhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, list_val.name).ok();
                let dp = format!("%xdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, hp).ok();
                let de = format!("%xde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, de, dp).ok();
                let ep = format!("%xep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, de, idx_val.name).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, v, ep).ok();
                // 2026-06-27: propagate element type from List<T> so downstream
                // FieldAccess can resolve struct fields (e.g., rules[i].slot_count).
                // The list_val.ty may be transformed to Ptr by the backend, so we
                // also check the original variable's type from let_binding_types.
                let el_ty = match &list_val.ty {
                    Type::Applied(name, args) if name == "List" => args.first().cloned(),
                    _ => {
                        if let Expr::Identifier(var_name) = list.as_ref() {
                            backend.fun.let_binding_types.get(var_name).and_then(|ty| {
                                if let Type::Applied(name, args) = ty {
                                    if name == "List" { args.first().cloned() } else { None }
                                } else { None }
                            })
                        } else { None }
                    }
                };
                // 2026-06-30: If element type is unknown, fall back to Int.
                if let Some(et) = el_ty {
                    return TypedRegister { name: v.to_string(), ty: et };
                }
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            // ── Projection ──────────────────────────────────────
            Expr::Projection { source, target } => {
                return crate::backend::llvm::expr::projection::emit_projection(
                    backend, out, v, source, target, indent);
            }
            // ── StructInstance ──────────────────────────────────
             Expr::StructInstance(name, fields) => {
                let n = fields.len() as i64;
                let ai = format!("%sai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, n).ok();
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    let fv = backend.emit_expr(out, fval, indent);
                    let fp = format!("%sfp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, fp, ai, i as i64).ok();
                     let stored = if fv.ty == Type::Custom("Bool".to_string()) || fv.ty == Type::Custom("Char".to_string()) || fv.ty == Type::Custom("Float".to_string()) || fv.ty == Type::Custom("String".to_string()) {
                         backend.adapt_to_i64(out, indent, &fv)
                     } else { fv.name.clone() };
                     writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, stored, fp).ok();
                 }
                 writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ai).ok();
                 return TypedRegister { name: v.to_string(), ty: Type::Custom(name.clone()) };
             }
             // ── ObjectLiteral ───────────────────────────────────
            Expr::ObjectLiteral(fields) => {
                let n = fields.len() as i64;
                let ai = format!("%oai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, n).ok();
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    let fv = backend.emit_expr(out, fval, indent);
                    let fp = format!("%ofp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, fp, ai, i as i64).ok();
                    let stored = if fv.ty == Type::Custom("Bool".to_string()) || fv.ty == Type::Custom("Char".to_string()) || fv.ty == Type::Custom("Float".to_string()) || fv.ty == Type::Custom("String".to_string()) {
                        backend.adapt_to_i64(out, indent, &fv)
                    } else { fv.name.clone() };
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, stored, fp).ok();
                }
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ai).ok();
                // 2026-06-30: BUG FIX — missing return statement since initial Phase 6
                // extraction. ObjectLiteral produces a boxed i64, same as StructInstance,
                // but has no named type — return Type::Custom("Int".to_string()).
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            // ── FieldAccess ─────────────────────────────────────
            Expr::FieldAccess(obj, field) => {
                let obj_val = backend.emit_expr(out, obj, indent);
                let hp = format!("%fahp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, obj_val.name).ok();
                let mut found_offset = false;
                let mut offset = 0i64;
                if let Expr::Identifier(name) = obj.as_ref() {
                    if let Some(Type::Custom(struct_name)) = backend.fun.let_binding_types.get(name) {
                        if let Some(fields) = backend.ctx.struct_types.get(struct_name) {
                            for (fi, (fn_, _)) in fields.iter().enumerate() {
                                if fn_ == field {
                                    offset = fi as i64;
                                    found_offset = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if !found_offset {
                    if let Type::Custom(struct_name) = &obj_val.ty {
                        if let Some(fields) = backend.ctx.struct_types.get(struct_name) {
                            for (fi, (fn_, _)) in fields.iter().enumerate() {
                                if fn_ == field {
                                    offset = fi as i64;
                                    found_offset = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if found_offset {
                    let fp = format!("%fafp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, fp, hp, offset).ok();
                    writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, v, fp).ok();
                    // 2026-06-17: Return Float type for float fields so downstream
                    // code (emit_binop) correctly identifies them. String/Data fields
                    // remain Type::Custom("Int".to_string()) (stored boxed as i64 in struct).
                    let lookup_ty = || -> Option<Type> {
                        if let Expr::Identifier(name) = obj.as_ref() {
                            if let Some(Type::Custom(struct_name)) = backend.fun.let_binding_types.get(name) {
                                if let Some(fields) = backend.ctx.struct_types.get(struct_name) {
                                    let fi = offset as usize;
                                    if fi < fields.len() {
                                        let (_, field_ty) = &fields[fi];
                                        if matches!(field_ty, Type::Custom(__t) if __t == "Float") {
                                            return Some(field_ty.clone());
                                        }
                                    }
                                }
                            }
                        }
                        if let Type::Custom(struct_name) = &obj_val.ty {
                            if let Some(fields) = backend.ctx.struct_types.get(struct_name) {
                                let fi = offset as usize;
                                if fi < fields.len() {
                                    let (_, field_ty) = &fields[fi];
                                    if matches!(field_ty, Type::Custom(__t) if __t == "Float") {
                                        return Some(field_ty.clone());
                                    }
                                }
                            }
                        }
                        None
                    };
                    if let Some(ft) = lookup_ty() {
                        return TypedRegister { name: v.to_string(), ty: ft };
                    }
                    // 2026-06-30: Non-Float fields are stored as boxed i64 — return Int.
                    return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
                } else {
                    panic!("emit_expr: FieldAccess: field '{}' not found on object", field);
                    return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
                }
            }
            // ── PatternMatch ────────────────────────────────────
            Expr::PatternMatch { value, variant, fields } => {
                let src_val = backend.emit_expr(out, value, indent);
                let hp = format!("%php{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, src_val.name).ok();
                let disc = format!("%pdisc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, disc, hp).ok();
                let expected = backend.ctx.variant_disc.get(variant)
                    .map(|(_, d, _)| *d as i64)
                    .unwrap_or(0);
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, v, disc, expected).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) };
            }
            // ── MultiSlice ──────────────────────────────────────
            Expr::MultiSlice { value, ops } => {
                let src_val = backend.emit_expr(out, value, indent);
                // Atomic value literals: coord returns backend, stride/mask return 0
                let is_atomic_literal = matches!(value.as_ref(), Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_));
                if is_atomic_literal {
                    let has_coord = ops.iter().any(|op| matches!(op, BracketOp::Coord(_)));
                    let has_other = ops.iter().any(|op| matches!(op, BracketOp::Stride(_) | BracketOp::Mask(_)));
                    if has_coord && !has_other {
                        writeln!(out, "{}{} = add i64 0, {} ; atomic coord passthrough", indent, v, src_val.name).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 0, {} ; atomic multislice", indent, v, src_val.name).ok();
                    }
                    return TypedRegister { name: v.to_string(), ty: src_val.ty };
                }
                // Non-atomic: process ops as a sequential pipeline.
                // Phase 1: apply all Coord ops (index/slice) to extract sublist/element.
                // Phase 2: apply Stride ops (step-by filter).
                // Phase 3: apply Mask ops (element-wise boolean filter).
                let mut result_reg = src_val.clone();
                let saved_bindings = backend.fun.let_bindings.clone();
                let mut reboxed = false; // true if result is a freshly-boxed list

                for op in ops {
                    match op {
                        BracketOp::Coord(SliceCoordinate::Index(idx_expr)) => {
                            // If the current result is a freshly-boxed list (not the
                            // original source), unbox it to get the data pointer.
                            let hp = if reboxed {
                                let rhp = format!("%mrhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, rhp, result_reg.name).ok();
                                rhp
                            } else {
                                let ihp = format!("%mihp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ihp, result_reg.name).ok();
                                ihp
                            };
                            let dp = format!("%mdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, de, dp).ok();
                            let cv = backend.emit_expr(out, idx_expr, indent);
                            let ep = format!("%mep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, de, cv.name).ok();
                            let lv = format!("%mlv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, lv, ep).ok();
                            result_reg = TypedRegister { name: lv, ty: Type::Custom("Int".to_string()) };
                            reboxed = false;
                        }
                        BracketOp::Coord(SliceCoordinate::Range { start, end }) => {
                            // Extract sub-range [start, end) into a new list
                            let hp = format!("%mrhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, result_reg.name).ok();
                            let dp = format!("%mrdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mrde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, de, dp).ok();
                            let slp = format!("%mrlp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, slp, hp).ok();
                            let src_len = format!("%mrsl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, src_len, slp).ok();
                            // Start bound
                            let start_reg = start.as_ref().map(|s| backend.emit_expr(out, s, indent));
                            let end_reg = end.as_ref().map(|e| backend.emit_expr(out, e, indent));
                            let lo = format!("%mrlo{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            if let Some(s) = &start_reg {
                                writeln!(out, "{}{} = add i64 0, {}", indent, lo, s.name).ok();
                            } else {
                                writeln!(out, "{}{} = add i64 0, 0", indent, lo).ok();
                            }
                            let hi = format!("%mrhi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            if let Some(e) = &end_reg {
                                writeln!(out, "{}{} = add i64 0, {}", indent, hi, e.name).ok();
                            } else {
                                writeln!(out, "{}{} = add i64 0, {}", indent, hi, src_len).ok();
                            }
                            let rcnt = format!("%mrcnt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = sub i64 {}, {}", indent, rcnt, hi, lo).ok();
                            // Allocate new list
                            let rab = format!("%mrab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = mul i64 {}, 8", indent, rab, rcnt).ok();
                            let rrm = backend.emit_arena_alloc(out, indent, &rab);
                            let rai = format!("%mrai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, rai, rrm).ok();
                            // Copy loop
                            let r_entry = format!("mr_entry{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let r_hdr = format!("mr_hdr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let r_body = format!("mr_body{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let r_done = format!("mr_done{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let ri = format!("%mri{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let rc = format!("%mrc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let rn = format!("%mrn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}br label %{}", indent, r_entry).ok();
                            writeln!(out, "{}{}:", indent, r_entry).ok();
                            writeln!(out, "{}br label %{}", indent, r_hdr).ok();
                            writeln!(out, "{}{}:", indent, r_hdr).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, ri, r_entry, rn, r_body).ok();
                            writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, rc, ri, rcnt).ok();
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, rc, r_body, r_done).ok();
                            writeln!(out, "{}{}:", indent, r_body).ok();
                            let r_src = format!("%mrsrc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = add i64 {}, {}", indent, r_src, lo, ri).ok();
                            let r_gep = format!("%mrgep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, r_gep, de, r_src).ok();
                            let r_el = format!("%mrel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, r_el, r_gep).ok();
                            let r_dst = format!("%mrdst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, r_dst, rai, ri).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, r_el, r_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, rn, ri).ok();
                            writeln!(out, "{}br label %{}", indent, r_hdr).ok();
                            writeln!(out, "{}{}:", indent, r_done).ok();
                            // Store header (data_ptr, length)
                            let r_dpp = format!("%mrdpp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 2", indent, r_dpp, rai).ok();
                            let r_dpv = format!("%mrdpv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, r_dpv, r_dpp).ok();
                            let rs0 = format!("%mrs0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, rs0, rai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, r_dpv, rs0).ok();
                            let rs1 = format!("%mrs1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, rs1, rai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, rcnt, rs1).ok();
                            let rv = format!("%mrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, rv, rai).ok();
                            result_reg = TypedRegister { name: rv, ty: Type::Custom("Int".to_string()) };
                            reboxed = true;
                        }
                        BracketOp::Coord(_) => {
                            // Named/AtDimension/Ellipsis coords are desugared before
                            // codegen; treat as passthrough.
                        }
                        BracketOp::Stride(stride_expr) => {
                            // Step-by filter: keep every Nth element
                            let sv = backend.emit_expr(out, stride_expr, indent);
                            // Unbox current list
                            let hp = format!("%mshp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, result_reg.name).ok();
                            let dp = format!("%msdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, hp).ok();
                            let de = format!("%msde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, de, dp).ok();
                            let lp = format!("%mslp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, hp).ok();
                            let len = format!("%mslen{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, len, lp).ok();
                            // Allocate stride-filtered buffer
                            let sab = format!("%msab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = mul i64 {}, 8", indent, sab, len).ok();
                            let srm = backend.emit_arena_alloc(out, indent, &sab);
                            let sai = format!("%msai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, sai, srm).ok();
                            // Loop: j = 0; k = 0; while j < len { copy[j]; j += stride; k++ }
                            let s_entry = format!("ms_entry{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let s_hdr = format!("ms_hdr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let s_body = format!("ms_body{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let s_done = format!("ms_done{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let sj = format!("%msj{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let sc = format!("%msc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let sn = format!("%msn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let sk = format!("%msk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let snk = format!("%msnk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}br label %{}", indent, s_entry).ok();
                            writeln!(out, "{}{}:", indent, s_entry).ok();
                            writeln!(out, "{}br label %{}", indent, s_hdr).ok();
                            writeln!(out, "{}{}:", indent, s_hdr).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, sj, s_entry, sn, s_body).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, sk, s_entry, snk, s_body).ok();
                            writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, sc, sj, len).ok();
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, sc, s_body, s_done).ok();
                            writeln!(out, "{}{}:", indent, s_body).ok();
                            let s_gep = format!("%msgep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, s_gep, de, sj).ok();
                            let s_el = format!("%msel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, s_el, s_gep).ok();
                            let s_dst = format!("%msdst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, s_dst, sai, sk).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, s_el, s_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, {}", indent, sn, sj, sv.name).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, snk, sk).ok();
                            writeln!(out, "{}br label %{}", indent, s_hdr).ok();
                            writeln!(out, "{}{}:", indent, s_done).ok();
                            // Store header
                            let s_dpp = format!("%msdpp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 2", indent, s_dpp, sai).ok();
                            let s_dpv = format!("%msdpv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, s_dpv, s_dpp).ok();
                            let ss0 = format!("%mss0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, ss0, sai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, s_dpv, ss0).ok();
                            let ss1 = format!("%mss1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, ss1, sai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, sk, ss1).ok();
                            let sv_reg = format!("%msv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, sv_reg, sai).ok();
                            result_reg = TypedRegister { name: sv_reg, ty: Type::Custom("Int".to_string()) };
                            reboxed = true;
                        }
                        BracketOp::Mask(mask_expr) => {
                            // Element-wise filter: evaluate mask for each element
                            let hp = format!("%mmhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, result_reg.name).ok();
                            let dp = format!("%mmdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mmde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, de, dp).ok();
                            let lp = format!("%mmlp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, hp).ok();
                            let len = format!("%mmlen{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, len, lp).ok();
                            // Allocate mask-filtered buffer (max size = len)
                            let mab = format!("%mmab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = mul i64 {}, 8", indent, mab, len).ok();
                            let mrm = backend.emit_arena_alloc(out, indent, &mab);
                            let mai = format!("%mmai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, mai, mrm).ok();
                            // Loop: j = 0; k = 0; while j < len
                            let m_entry = format!("mm_entry{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let m_hdr = format!("mm_hdr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let m_body = format!("mm_body{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let m_done = format!("mm_done{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let mj = format!("%mmj{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let mc = format!("%mmc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let mn = format!("%mmn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let mk = format!("%mmk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let mnk = format!("%mmnk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}br label %{}", indent, m_entry).ok();
                            writeln!(out, "{}{}:", indent, m_entry).ok();
                            writeln!(out, "{}br label %{}", indent, m_hdr).ok();
                            writeln!(out, "{}{}:", indent, m_hdr).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, mj, m_entry, mn, m_body).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, mk, m_entry, mnk, m_body).ok();
                            writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, mc, mj, len).ok();
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, mc, m_body, m_done).ok();
                            writeln!(out, "{}{}:", indent, m_body).ok();
                            let m_gep = format!("%mmgep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, m_gep, de, mj).ok();
                            let m_el = format!("%mmel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, m_el, m_gep).ok();
                            // Bind _ to element, evaluate mask
                            backend.fun.let_bindings.insert("_".to_string(), m_el.clone());
                            let mask_r = backend.emit_expr(out, mask_expr, indent);
                            let mask_b = backend.as_bool_reg(out, indent, &mask_r);
                            let m_store_l = format!("mm_store{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let m_skip_l = format!("mm_skip{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, mask_b, m_store_l, m_skip_l).ok();
                            writeln!(out, "{}{}:", indent, m_store_l).ok();
                            let m_dst = format!("%mmdst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, m_dst, mai, mk).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_el, m_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, mnk, mk).ok();
                            writeln!(out, "{}br label %{}", indent, m_skip_l).ok();
                            writeln!(out, "{}{}:", indent, m_skip_l).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, mn, mj).ok();
                            writeln!(out, "{}br label %{}", indent, m_hdr).ok();
                            writeln!(out, "{}{}:", indent, m_done).ok();
                            // Store header
                            let m_dpp = format!("%mmdpp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 2", indent, m_dpp, mai).ok();
                            let m_dpv = format!("%mmdpv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, m_dpv, m_dpp).ok();
                            let ms0 = format!("%mms0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, ms0, mai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_dpv, ms0).ok();
                            let ms1 = format!("%mms1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, ms1, mai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, mk, ms1).ok();
                            let mv_reg = format!("%mmv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, mv_reg, mai).ok();
                            result_reg = TypedRegister { name: mv_reg, ty: Type::Custom("Int".to_string()) };
                            reboxed = true;
                        }
                    }
                }
                writeln!(out, "{}{} = add i64 0, {}", indent, v, result_reg.name).ok();
                backend.fun.let_bindings = saved_bindings;
                // 2026-06-30: Explicit return for normal path (was fallthrough).
                return TypedRegister { name: v.to_string(), ty: result_reg.ty };
            }
            // ── Match ───────────────────────────────────────────
            Expr::Match { value, arms } => {
                let saved_bindings = backend.fun.let_bindings.clone();
                let saved_types = backend.fun.let_binding_types.clone();
                let val = backend.emit_expr(out, value, indent);
                let hp = format!("%mhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, val.name).ok();
                let disc_reg = format!("%mdisc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, disc_reg, hp).ok();

                let mut variant_arms: Vec<(u64, &MatchArm)> = Vec::new();
                let mut wildcard_arm: Option<&MatchArm> = None;
                for arm in arms {
                    match &arm.pattern {
                        MatchPattern::Variant { name, .. } => {
                            if let Some(&(_, disc_val, _)) = backend.ctx.variant_disc.get(name) {
                                variant_arms.push((disc_val, arm));
                            }
                        }
                        MatchPattern::Wildcard => { wildcard_arm = Some(arm); }
                        _ => {}
                    }
                }

                let default_label = format!("mdef{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let merge_label = format!("mmerge{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cases: Vec<String> = variant_arms.iter().enumerate()
                    .map(|(i, (disc, _))| format!("i64 {}, label %marm{}", disc, i))
                    .collect();
                writeln!(out, "{}switch i64 {}, label %{} [ {} ]", indent, disc_reg, default_label, cases.join(" ")).ok();

                for (i, (disc, arm)) in variant_arms.iter().enumerate() {
                    writeln!(out, "{}marm{}:", indent, i).ok();
                    if let MatchPattern::Variant { fields, .. } = &arm.pattern {
                        for (j, field) in fields.iter().enumerate() {
                            if let Pattern::Var(var_name) = field {
                                let gep = format!("%mgep{}_{}", i, j);
                                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, gep, hp, (j as i64) + 1).ok();
                                let fv = format!("%mfv{}_{}", i, j);
                                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, fv, gep).ok();
                                backend.fun.let_bindings.insert(var_name.clone(), fv);
                            }
                        }
                    }
                    let body_val = backend.emit_expr(out, &arm.body, indent);
                    writeln!(out, "{}{} = add i64 0, {} ; match arm", indent, v, body_val.name).ok();
                    writeln!(out, "{}br label %{}", indent, merge_label).ok();
                }

                writeln!(out, "{}{}:", indent, default_label).ok();
                if let Some(wildcard) = wildcard_arm {
                    let body_val = backend.emit_expr(out, &wildcard.body, indent);
                    writeln!(out, "{}{} = add i64 0, {} ; match wildcard", indent, v, body_val.name).ok();
                    writeln!(out, "{}br label %{}", indent, merge_label).ok();
                } else {
                    writeln!(out, "{}unreachable", indent).ok();
                }
                writeln!(out, "{}{}:", indent, merge_label).ok();
                backend.fun.let_bindings = saved_bindings;
                backend.fun.let_binding_types = saved_types;
                let match_ty = if arms.iter().all(|a| matches!(a.body.as_ref(), Expr::String(_))) {
                    Type::Custom("String".to_string())
                } else {
                    Type::Custom("Int".to_string())
                };
                return TypedRegister { name: v.to_string(), ty: match_ty };
            }
            // ── Slice ───────────────────────────────────────────
            Expr::Slice { value, start, end, stride, mask } => {
                let src_val = backend.emit_expr(out, value, indent);
                // Atomic value literals: pass through (single element is itself)
                let is_atomic_literal = matches!(value.as_ref(), Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_));
                if is_atomic_literal {
                    writeln!(out, "{}{} = add i64 0, {} ; atomic slice passthrough", indent, v, src_val.name).ok();
                    return crate::backend::llvm::TypedRegister { name: v.to_string(), ty: src_val.ty };
                }
                // List: pointer-based list access
                let hp = format!("%shp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, src_val.name).ok();
                let dp = format!("%sdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, hp).ok();
                let de = format!("%sde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, de, dp).ok();
                let src_len_reg = format!("%sln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let slp = format!("%slp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, slp, hp).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, src_len_reg, slp).ok();

                let start_reg = start.as_ref().map(|s| backend.emit_expr(out, s, indent));
                let end_reg = end.as_ref().map(|e| backend.emit_expr(out, e, indent));
                let stride_reg = stride.as_ref().map(|s| backend.emit_expr(out, s, indent));
                // Compute raw range = end - start
                let raw_count = format!("%sraw{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                if let (Some(s), Some(e)) = (&start_reg, &end_reg) {
                    writeln!(out, "{}{} = sub i64 {}, {}", indent, raw_count, e.name, s.name).ok();
                } else {
                    writeln!(out, "{}{} = add i64 0, {}", indent, raw_count, src_len_reg).ok();
                }
                // Compute effective count with stride: ceil(raw_count / stride)
                let count_reg = format!("%scnt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                if let Some(str) = &stride_reg {
                    let adj = format!("%sadj{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = add i64 {}, -1", indent, adj, raw_count).ok();
                    let div = format!("%sdiv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = udiv i64 {}, {}", indent, div, adj, str.name).ok();
                    writeln!(out, "{}{} = add i64 {}, 1", indent, count_reg, div).ok();
                } else {
                    writeln!(out, "{}{} = add i64 0, {}", indent, count_reg, raw_count).ok();
                }

                // Why malloc for slice results: slice produces a new list whose size
                // is only known at runtime (depends on start, end, stride). Stack
                // allocation is impossible because the size varies per execution.
                // Allocate new list header (avoids invalid dynamic alloca in non-entry block)
                let ab = format!("%sab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, ab, count_reg).ok();
                let rm = backend.emit_arena_alloc(out, indent, &ab);
                let ai = format!("%sai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, ai, rm).ok();

                let entry_label = format!("s_entry{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let header_label = format!("s_hdr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let body_label = format!("s_body{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let done_label = format!("s_done{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let i_reg = format!("%si{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cond_reg = format!("%scond{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let next_reg = format!("%snext{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;

                writeln!(out, "{}br label %{}", indent, entry_label).ok();
                writeln!(out, "{}{}:", indent, entry_label).ok();
                writeln!(out, "{}br label %{}", indent, header_label).ok();
                writeln!(out, "{}{}:", indent, header_label).ok();
                writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, i_reg, entry_label, next_reg, body_label).ok();
                writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cond_reg, i_reg, count_reg).ok();
                writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cond_reg, body_label, done_label).ok();
                writeln!(out, "{}{}:", indent, body_label).ok();
                // Copy element: src[start + i*stride]
                let src_idx = format!("%ssi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                if let Some(s) = &start_reg {
                    if let Some(str) = &stride_reg {
                        let si_stride = format!("%sist{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, {}", indent, si_stride, i_reg, str.name).ok();
                        writeln!(out, "{}{} = add i64 {}, {}", indent, src_idx, s.name, si_stride).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 {}, {}", indent, src_idx, s.name, i_reg).ok();
                    }
                } else {
                    if let Some(str) = &stride_reg {
                        let si_stride = format!("%sist{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, {}", indent, si_stride, i_reg, str.name).ok();
                        writeln!(out, "{}{} = add i64 0, {}", indent, src_idx, si_stride).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 0, {}", indent, src_idx, i_reg).ok();
                    }
                }
                let src_ep = format!("%ssep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, src_ep, de, src_idx).ok();
                let elem = format!("%selem{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, elem, src_ep).ok();
                // Store to dest[2 + i]
                let dst_idx = format!("%sdi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 2", indent, dst_idx, i_reg).ok();
                let dst_ep = format!("%sdep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, dst_ep, ai, dst_idx).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem, dst_ep).ok();
                writeln!(out, "{}{} = add i64 {}, 1", indent, next_reg, i_reg).ok();
                writeln!(out, "{}br label %{}", indent, header_label).ok();
                writeln!(out, "{}{}:", indent, done_label).ok();
                // Store data_ptr and length in the strided-result header
                let dp_ptr = format!("%sdp2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 2", indent, dp_ptr, ai).ok();
                let dp_val = format!("%sdv2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, dp_val, dp_ptr).ok();
                let s0 = format!("%ss0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, s0, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp_val, s0).ok();
                let s1 = format!("%ss1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, s1, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, count_reg, s1).ok();

                // ── Mask filter (second pass) ──
                // If a mask expression is present, walk the strided result and
                // keep only elements where mask(_, elem) evaluates to true.
                if let Some(mask_expr) = mask {
                    let saved_bindings = backend.fun.let_bindings.clone();
                    let old_count = count_reg;
                    let old_ai = ai;
                    let m_entry = format!("sm_entry{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let m_hdr = format!("sm_hdr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let m_body = format!("sm_body{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let m_done = format!("sm_done{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let m_j = format!("%smj{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let m_cond = format!("%smcond{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let m_next = format!("%smnext{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let m_k = format!("%smk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    // Allocate max-size filtered buffer
                    let m_ab = format!("%smab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = mul i64 {}, 8", indent, m_ab, old_count).ok();
                    let m_rm = backend.emit_arena_alloc(out, indent, &m_ab);
                    let m_ai = format!("%smai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, m_ai, m_rm).ok();
                    let zero_reg = format!("%smz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = add i64 0, 0", indent, zero_reg).ok();

                    writeln!(out, "{}br label %{}", indent, m_entry).ok();
                    writeln!(out, "{}{}:", indent, m_entry).ok();
                    writeln!(out, "{}br label %{}", indent, m_hdr).ok();
                    writeln!(out, "{}{}:", indent, m_hdr).ok();
                    writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, m_j, m_entry, m_next, m_body).ok();
                    writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, m_k, m_entry, m_k, m_body).ok();
                    writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, m_cond, m_j, old_count).ok();
                    writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, m_cond, m_body, m_done).ok();
                    writeln!(out, "{}{}:", indent, m_body).ok();
                    // Load element from strided result
                    let m_gep = format!("%smgep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, m_gep, old_ai, m_j).ok();
                    let m_elem = format!("%smelem{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, m_elem, m_gep).ok();
                    // Bind _ to element, evaluate mask
                    backend.fun.let_bindings.insert("_".to_string(), m_elem.clone());
                    let mask_reg = backend.emit_expr(out, mask_expr, indent);
                    let mask_bool = backend.as_bool_reg(out, indent, &mask_reg);
                    writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, mask_bool, m_done, m_hdr);
                    // If mask true, append to filtered buffer
                    // (true branch already jumps to m_done — use a separate skip label)
                    let m_store = format!("sm_store{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let m_next_label = format!("sm_next{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{}:", indent, m_store).ok();
                    let m_dst = format!("%smdst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, m_dst, m_ai, m_k).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_elem, m_dst).ok();
                    writeln!(out, "{}{} = add i64 {}, 1", indent, m_next, m_k).ok();
                    writeln!(out, "{}br label %{}", indent, m_next_label).ok();
                    writeln!(out, "{}{}:", indent, m_next_label).ok();
                    writeln!(out, "{}{} = add i64 {}, 1", indent, m_next, m_j).ok();
                    writeln!(out, "{}br label %{}", indent, m_hdr).ok();

                    writeln!(out, "{}{}:", indent, m_done).ok();
                    // Store filtered header
                    let m_dp_ptr = format!("%smdp2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 2", indent, m_dp_ptr, m_ai).ok();
                    let m_dp_val = format!("%smdv2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, m_dp_val, m_dp_ptr).ok();
                    let ms0 = format!("%sms0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, ms0, m_ai).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_dp_val, ms0).ok();
                    let ms1 = format!("%sms1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, ms1, m_ai).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_k, ms1).ok();
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, m_ai).ok();
                    backend.fun.let_bindings = saved_bindings;
                } else {
                    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ai).ok();
                }
                // 2026-06-30: Explicit return for normal path (was fallthrough).
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            // — Subtype projection (e.g. list :> Size) —
            Expr::SubtypeProjection { source, .. } => {
                let src = backend.emit_expr(out, source, indent);
                let hp = format!("%shp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, hp, src.name).ok();
                let slp = format!("%slp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, slp, hp).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, v, slp).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            Expr::SubtypeProjectionExpr(e) => {
                return backend.emit_expr(out, &Expr::SubtypeProjection {
                    source: e.source.clone(),
                    ops: e.ops.clone(),
                }, indent);
            }
            Expr::IsType(expr, target) => {
                let _ = backend.emit_expr(out, expr, indent);
                let comment = match target {
                    crate::ast::IsTarget::Type(_) => "is type",
                    crate::ast::IsTarget::Variant(v) => v,
                };
                writeln!(out, "{}{} = add i64 0, 1 ; {} (compile-time)", indent, v, comment).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) };
            }
            Expr::FromCheck(expr, _ty) => {
                let _ = backend.emit_expr(out, expr, indent);
                writeln!(out, "{}{} = add i64 0, 1 ; from (compile-time)", indent, v).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) };
            }
            Expr::Like(l, r) => {
                return backend.emit_fcmp(out, indent, l, r, "oeq");
            }
            Expr::Block(stmts, last) => {
                for s in stmts {
                    backend.emit_stmt(out, s, indent);
                    if backend.fun.terminated {
                        return TypedRegister { name: "_".to_string(), ty: Type::Void };
                    }
                }
                return backend.emit_expr(out, last, indent);
            }
            Expr::MapLiteral(items) => {
                let n = items.len() as i64;
                let alloc_slots = n + 2;
                // Why malloc/arena for map/set literals: the literal may have a large
                // number of entries (hundreds). Stack via alloca would risk overflow.
                // Arena handles this with bump alloc when in a loop context.
                let map_alloc_size = format!("%mas{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, map_alloc_size, (alloc_slots * 8 + 8)).ok();
                let ai = backend.emit_arena_alloc(out, indent, &map_alloc_size);
                let hp = format!("%mhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, hp, ai).ok();
                let base = format!("%mba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, base, ai).ok();
                let dp = format!("%mdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, hp).ok();
                let ml1 = format!("%mml1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, ml1, hp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, n, ml1).ok();
                // Store values (keys are compile-time in the source map literal)
                for (i, (_key, val)) in items.iter().enumerate() {
                    let kv = backend.emit_expr(out, val, indent);
                    let kvs = backend.adapt_to_i64(out, indent, &kv);
                    let ep = format!("%mep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, kvs, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ai).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            Expr::SetLiteral(items) => {
                let n = items.len() as i64;
                let set_alloc_size = format!("%sas{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, set_alloc_size, (n + 2) * 8 + 8).ok();
                let ai = backend.emit_arena_alloc(out, indent, &set_alloc_size);
                let hp = format!("%shp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, hp, ai).ok();
                let base = format!("%sba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, base, ai).ok();
                let dp = format!("%sdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, hp).ok();
                let sl1 = format!("%ssl1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, sl1, hp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, n, sl1).ok();
                for (i, item) in items.iter().enumerate() {
                    let iv = backend.emit_expr(out, item, indent);
                    let ivs = backend.adapt_to_i64(out, indent, &iv);
                    let ep = format!("%sep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ivs, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ai).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            // Why free+malloc+memcpy instead of realloc: Brief collections have
            // Arrow handlers dispatched to expr::arrow submodule.
            // 2026-06-30: Extracted from rest.rs to src/backend/llvm/expr/arrow.rs.
            Expr::ArrowMut { dir: ArrowDir::Push, target, index, value: Some(val) } => {
                return crate::backend::llvm::expr::arrow::emit_arrow_push(backend, out, v, target, index, val, indent);
            }
            // Why free+malloc+memcpy for pop: same semantics as push — the old
            // buffer is dead after the operation. Pop removes one element but
            // we still allocate a fresh buffer of len-1. An arena allocator
            // (planned) would replace the free+malloc with a bump pointer reset.
            // 2026-06-30: Extracted to src/backend/llvm/expr/arrow.rs.
            Expr::ArrowMut { dir: ArrowDir::Pop, target, index, value: None } => {
                return crate::backend::llvm::expr::arrow::emit_arrow_pop(backend, out, v, target, index, indent);
            }
            // 2026-06-30: Extracted to src/backend/llvm/expr/arrow.rs.
            Expr::ArrowDiscard { target, index } => {
                return crate::backend::llvm::expr::arrow::emit_arrow_discard(backend, out, v, target, index, indent);
            }
            // ArrowTransfer moves ALL elements from source to destination.
            // Both old buffers are freed; a new combined buffer is allocated.
            // The source list becomes empty (2-slot header with data_ptr=null, len=0).
            // This is the most allocation-heavy arrow op — the arena plan (Phase 1)
            // benefits transfer the most.
            // 2026-06-30: Extracted to src/backend/llvm/expr/arrow.rs.
            Expr::ArrowTransfer { dest, source, filter: _ } => {
                return crate::backend::llvm::expr::arrow::emit_arrow_transfer(backend, out, v, dest, source, indent);
            }
            Expr::Cast(inner, target_ty) => {
                // 2026-07-03: EOR optimization — detect Cast(BinaryOp(Cast(a, T), Cast(b, T)), U)
                // where U <:> T. Skips redundant casts, emits native arithmetic.
                if let Some(tr) = backend.try_emit_eor(out, v, inner, target_ty, indent) {
                    return tr;
                }
                let inner_val = backend.emit_expr(out, inner, indent);
                // 2026-06-28: Use txn_counter to prevent %t{N} collision
                let cv = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                backend.emit_cast_convert(out, indent, &cv, &inner_val.name, Some(inner_val.ty), target_ty);
                // Casts to boxed types (String/Data) produce i64, not native i8*.
                let ret_ty = if matches!(target_ty, Type::Custom(__t) if __t == "String" || __t == "Data") {
                    Type::Custom("Int".to_string())
                } else {
                    target_ty.clone()
                };
                return TypedRegister { name: cv, ty: ret_ty };
            }
            // ── CellCall ──────────────────────────────────────────
            Expr::CellCall(callee, args) => {
                let callee_name = match callee.as_ref() {
                    Expr::Identifier(name) => name.clone(),
                    _ => { panic!("emit_expr: CellCall with non-identifier callee: {:?}", callee); return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }; }
                };
                let cell = match backend.ctx.cell_defs.get(&callee_name) {
                    Some(c) => c.clone(),
                    None => { panic!("emit_expr: CellCall: cell '{}' not found in cell_defs", callee_name); return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }; }
                };

                // 1. Store input args to prefixed parameter fields
                for (i, (param_name, _param_ty)) in cell.parameters.iter().enumerate() {
                    if i < args.len() {
                        let arg_reg = backend.emit_expr(out, &args[i], indent);
                        let prefixed = format!("cell${}${}", callee_name, param_name);
                        if let Some(&idx) = backend.ctx.field_index_map.get(&prefixed) {
                            let ll_ty = backend.ctx.field_types[idx].clone();
                            let gep = format!("%csp_{}_{}", &callee_name, &param_name);
                            writeln!(out, "{}{} = getelementptr %State, ptr {}, i32 0, i32 {}",
                                indent, gep, backend.fun.state_reg_name, idx).ok();
                            let adapted = backend.adapt_to_i64(out, indent, &arg_reg);
                            let store_val = match ll_ty.as_str() {
                                "i8" => {
                                    let t = format!("%cstr_{}_{}", &callee_name, &param_name);
                                    writeln!(out, "{}{} = trunc i64 {} to i8", indent, t, adapted).ok();
                                    t
                                }
                                "i32" => {
                                    let t = format!("%cst_{}_{}", &callee_name, &param_name);
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, t, adapted).ok();
                                    t
                                }
                                "float" => {
                                    let t = format!("%cstf_{}_{}", &callee_name, &param_name);
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, t, adapted).ok();
                                    let fl = format!("%cstfl_{}_{}", &callee_name, &param_name);
                                    writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, t).ok();
                                    fl
                                }
                                "i8*" => {
                                    let t = format!("%cstp_{}_{}", &callee_name, &param_name);
                                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, t, adapted).ok();
                                    t
                                }
                                _ => adapted,
                            };
                            writeln!(out, "{}store {} {}, ptr {}, align 8",
                                indent, ll_ty, store_val, gep).ok();
                        }
                    }
                }

                // 2. Convergence loop: repeat txns until stasis
                let loop_h = format!(".celloop_{}", backend.fun.txn_counter);
                let done_l = format!(".celldone_{}", backend.fun.txn_counter);
                let any_fired = format!("%cany_{}", backend.fun.txn_counter);
                backend.fun.txn_counter += 1;

                // Alloca for any_fired flag (initialized to false)
                writeln!(out, "{}{} = alloca i8, align 1", indent, any_fired).ok();
                writeln!(out, "{}store i8 0, ptr {}, align 1", indent, any_fired).ok();

                writeln!(out, "{}br label %{}", indent, loop_h).ok();
                writeln!(out, "{}:", loop_h).ok();
                // Clear SSA old-value cache so precondition evaluation emits
                // fresh loads instead of stale cached values. Without this, the
                // CellCall convergence loop sees stale field values and loops
                // forever when the body stores new values to the same fields.
                backend.fun.ssa_old_int_regs.clear();
                backend.fun.ssa_old_float_regs.clear();

                for (ti, txn) in cell.transactions.iter().enumerate() {
                    let fire_l = format!(".cl_{}_{}", backend.fun.txn_counter, ti);
                    let post_ok_l = format!(".cl_{}_{}_pok", backend.fun.txn_counter, ti);
                    let reset_l = format!(".cl_{}_{}_pres", backend.fun.txn_counter, ti);
                    let skip_l = format!(".cl_{}_s_{}", backend.fun.txn_counter, ti);

                    // Evaluate precondition with rewritten identifiers
                    let pre_expr = crate::backend::llvm::LlvmBackend::rewrite_cell_identifiers(&txn.contract.pre_condition, &callee_name);
                    let pre_val = backend.emit_expr(out, &pre_expr, indent);

                    writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, pre_val.name, fire_l, skip_l).ok();
                    writeln!(out, "{}:", fire_l).ok();

                    // Execute body
                    for stmt in &txn.body {
                        let rewritten = crate::backend::llvm::LlvmBackend::rewrite_cell_stmt_identifiers(stmt, &callee_name);
                        backend.emit_stmt(out, &rewritten, indent);
                    }

                    // Check postcondition — set any_fired only if postcondition is true
                    let post_expr = crate::backend::llvm::LlvmBackend::rewrite_cell_identifiers(&txn.contract.post_condition, &callee_name);
                    let post_val = backend.emit_expr(out, &post_expr, indent);
                    writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, post_val.name, post_ok_l, reset_l).ok();
                    writeln!(out, "{}:", post_ok_l).ok();
                    writeln!(out, "{}store i8 1, ptr {}, align 1", indent, any_fired).ok();
                    writeln!(out, "{}br label %{}", indent, skip_l).ok();
                    writeln!(out, "{}:", reset_l).ok();
                    writeln!(out, "{}store i8 0, ptr {}, align 1", indent, any_fired).ok();
                    writeln!(out, "{}br label %{}", indent, skip_l).ok();
                    writeln!(out, "{}:", skip_l).ok();
                }

                // After all txns: check any_fired → loop or done
                let af_load = format!("%cal_{}", backend.fun.txn_counter);
                writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, af_load, any_fired).ok();
                let af_bool = format!("%cab_{}", backend.fun.txn_counter);
                writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, af_bool, af_load).ok();
                writeln!(out, "{}store i8 0, ptr {}, align 1", indent, any_fired).ok();
                writeln!(out, "{}br i1 {}, label %{}, label %{}",
                    indent, af_bool, loop_h, done_l).ok();
                writeln!(out, "{}:", done_l).ok();

                // 3. Read designated output from prefixed output field
                let output_names = crate::backend::llvm::LlvmBackend::extract_output_names_llvm(&cell.output_type);
                if let Some(first_name) = output_names.first() {
                    let prefixed = format!("cell${}${}", callee_name, first_name);
                    if let Some(&idx) = backend.ctx.field_index_map.get(&prefixed) {
                        let ll_ty = &backend.ctx.field_types[idx];
                        let gep = format!("%cgo_{}_{}", &callee_name, first_name);
                        writeln!(out, "{}{} = getelementptr %State, ptr {}, i32 0, i32 {}",
                            indent, gep, backend.fun.state_reg_name, idx).ok();
                        writeln!(out, "{}{} = load {}, ptr {}, align 8", indent, v, ll_ty, gep).ok();
                        let ret_ty = match ll_ty.as_str() {
                            "i8" => Type::Custom("Bool".to_string()),
                            "i32" => Type::Custom("Char".to_string()),
                            "float" => Type::Custom("Float".to_string()),
                            "i8*" => Type::Custom("String".to_string()),
                            _ => Type::Custom("Int".to_string()),
                        };
                        if ret_ty == Type::Custom("Int".to_string()) && ll_ty != "i64" {
                            // 2026-06-28: Use txn_counter to prevent %t{N} collision
                            let boxed = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = zext {} {} to i64", indent, boxed, ll_ty, v).ok();
                            return TypedRegister { name: boxed, ty: Type::Custom("Int".to_string()) };
                        }
                        if ret_ty == Type::Custom("String".to_string()) {
                            // 2026-06-28: Use txn_counter to prevent %t{N} collision
                            let boxed = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, boxed, v).ok();
                            return TypedRegister { name: boxed, ty: Type::Custom("Int".to_string()) };
                        }
                        return TypedRegister { name: v.to_string(), ty: ret_ty };
                    }
                    // NOTE: Multi-output cells return via extract_output_names_llvm which
                    // returns all named port names, but we only read the first one here.
                    // The interpreter supports full multi-output via Value::Tuple, but
                    // LLVM codegen returns a single i64 register. For cells with multiple
                    // output ports, the second+ ports are unreachable from LLVM codegen
                    // until TypedRegister supports tuple types. Interpreter is the
                    // reference — LLVM multi-output is deferred.
                }

                // Fallback: return 0
                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            Expr::Within { body, bound: _, unit: _, retries: _, fallback } => {
                // Evaluate body in the CURRENT block using direct GEP + load from %State
                // for identifiers. This avoids SSA register dominance issues (the within
                // blocks branch from here, so %State is always valid in all successors).
                let wid = backend.fun.within_counter;
                backend.fun.within_counter += 1;
                let l_entry = format!("we_{}", wid);
                let l_fallback = format!("wf_{}", wid);
                let l_done = format!("wd_{}", wid);
                let v_save = format!("%ws_{}", wid);
                let v_body = format!("%wb_{}", wid);
                let v_result = format!("%wr_{}", wid);

                // Load body value: try GEP+load, fallback to emit_expr, then to 0
                let body_val = match body.as_ref() {
                    Expr::Identifier(name) => {
                        if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                            let gep = format!("%wgp_{}_{}", wid, idx);
                            let ld = format!("%wld_{}_{}", wid, idx);
                            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, gep, idx).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ld, gep).ok();
                            ld
                        } else {
                            // Identifier not in state — try emit_expr, fallback to 0
                            let reg = backend.emit_expr(out, body, indent);
                            if !reg.name.is_empty() {
                                let cp = format!("%wcp_{}", backend.fun.within_counter);
                                backend.fun.within_counter += 1;
                                writeln!(out, "{}{} = add i64 0, {}", indent, cp, reg.name).ok();
                                cp
                            } else {
                                format!("%wz_{}", wid)
                            }
                        }
                    }
                    _ => {
                        let reg = backend.emit_expr(out, body, indent);
                        let cp = format!("%wcp_{}", backend.fun.within_counter);
                        backend.fun.within_counter += 1;
                        writeln!(out, "{}{} = add i64 0, {}", indent, cp, reg.name).ok();
                        cp
                    }
                };
                writeln!(out, "{}{} = add i64 0, {}", indent, v_body, body_val).ok();

                // Branch to entry, save body result
                writeln!(out, "{}  br label %{}", indent, l_entry).ok();
                writeln!(out, "{}  {}:", indent, l_entry).ok();
                writeln!(out, "{}    {} = alloca i64, align 8", indent, v_save).ok();
                writeln!(out, "{}    store i64 {}, i64* {}, align 8", indent, v_body, v_save).ok();
                writeln!(out, "{}    br label %{}", indent, l_done).ok();

                // Fallback block: evaluate fallback in a clean block
                writeln!(out, "{}  {}:", indent, l_fallback).ok();
                let fb_reg = backend.emit_expr(out, fallback, &format!("{}    ", indent));
                writeln!(out, "{}    store i64 {}, i64* {}, align 8", indent, fb_reg.name, v_save).ok();
                writeln!(out, "{}    br label %{}", indent, l_done).ok();

                // Done: load result
                writeln!(out, "{}  {}:", indent, l_done).ok();
                writeln!(out, "{}    {} = load i64, ptr {}, align 8", indent, v_result, v_save).ok();
                return TypedRegister { name: v_result.clone(), ty: Type::Custom("Int".to_string()) };
            }
            // ── AddrOf (address-of) ──────────────────────────────
            Expr::AddrOf(inner) => {
                // emit_addr_of returns a pointer register (ptr in LLVM IR).
                // The result type is Ptr<T> which maps to LLVM's opaque ptr.
                match crate::backend::llvm::expr::identifier::emit_addr_of(backend, out, inner, indent) {
                    Ok(ptr_reg) => {
                        return TypedRegister { name: ptr_reg, ty: Type::Applied("Ptr".to_string(), vec![Type::Custom("Int".to_string())]) };
                    }
                    Err(msg) => {
                        unreachable!("emit_expr: cannot take address: {}", msg);
                    }
                }
            }
            // ── Deref (dereference) ───────────────────────────────
            Expr::Deref(inner) => {
                // Evaluate the pointer expression → gets a ptr register
                let ptr = backend.emit_expr(out, inner, indent);
                // Load from the pointer → gets the value
                let v_reg = format!("%t{}", backend.fun.txn_counter);
                backend.fun.txn_counter += 1;
                // Determine the pointee type for the load
                let (llvm_ty, pointee_ty) = match crate::type_universe::pointee_type(&ptr.ty) {
                    Some(inner_ty) => {
                        // Map Brief type to LLVM type
                        match inner_ty {
                            Type::Custom(ref s) if s == "Bool" => ("i1".to_string(), inner_ty),
                            Type::Custom(ref s) if s == "Char" => ("i32".to_string(), inner_ty),
                            Type::Custom(ref s) if s == "Int" => ("i64".to_string(), inner_ty),
                            Type::Custom(ref s) if s == "Float" => ("float".to_string(), inner_ty),
                            Type::Custom(ref s) if s == "Float64" => ("double".to_string(), inner_ty),
                            _ => ("i64".to_string(), Type::Custom("Int".to_string())),
                        }
                    }
                    None => ("i64".to_string(), Type::Custom("Int".to_string())),
                };
                let align = if llvm_ty == "i1" { 1 } else if llvm_ty == "i32" { 4 } else if llvm_ty == "float" || llvm_ty == "double" { 4 } else { 8 };
                writeln!(out, "{}{} = load {}, ptr %{}, align {}", indent, v_reg, llvm_ty, ptr.name, align).ok();
                return TypedRegister { name: v_reg, ty: pointee_ty };
            }
            _ => { unreachable!("emit_expr: unhandled Expr variant: {:?}", expr); }
        }
        // Default: treat as Int. Float operations are handled explicitly
        // by emit_binop/emit_fcmp which return Type::Custom("Float".to_string())/Bool respectively.
        TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
}
