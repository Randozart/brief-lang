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
                    crate::backend::llvm::TypedRegister { name: "%stub".into(), ty: crate::ast::Type::Int }
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
                    crate::backend::llvm::TypedRegister { name: "%stub".into(), ty: crate::ast::Type::Int }
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
                    crate::backend::llvm::TypedRegister { name: "%stub".into(), ty: crate::ast::Type::Int }
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
            Expr::Identifier(name) => {
                // SSA body mode: prefer pre-extracted old-value register
                // for int fields so all body ops are independent.
                if let Some(old_reg) = backend.fun.ssa_old_int_regs.get(name) {
                    // If the old register is a non-i64 type, cast to i64 first
                    if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                        let ft = &backend.ctx.field_types[idx];
                        if ft == "i8" {
                            let z = format!("%iz_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i8 {} to i1", indent, z, old_reg).ok();
                            return TypedRegister { name: z, ty: Type::Bool };
                        }
                        if ft == "i32" {
                            let z = format!("%iz_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, z, old_reg).ok();
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                            // i32 LLVM type means Char at the Brief level
                            // (the only Brief type mapped to i32).
                            return TypedRegister { name: v.to_string(), ty: Type::Char };
                        }
                        if ft == "i8*" || ft == "ptr" {
                            // old_reg is i8* from extractvalue on state (state stores
                            // native i8* for String fields, not boxed i64). ptrtoint to box.
                            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, old_reg).ok();
                            return TypedRegister { name: v.to_string(), ty: Type::Int };
                        }
                    }
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, old_reg).ok();
                    return TypedRegister { name: v.to_string(), ty: Type::Int };
                }
                // SSA body mode: prefer pre-extracted old-value register
                // for float fields so all body ops are independent.
                // 2026-06-29: Check field type to return Float (float) or Float64 (double).
                if let Some(old_reg) = backend.fun.ssa_old_float_regs.get(name) {
                    backend.fun.reg_float_cache.insert(old_reg.clone(), old_reg.clone());
                    let brief_ty = if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                        let ft = &backend.ctx.field_types[idx];
                        if ft == "double" { Type::Float64 } else { Type::Float }
                    } else {
                        Type::Float
                    };
                    return TypedRegister { name: old_reg.clone(), ty: brief_ty };
                }
                if let Some(ref ssa_reg) = backend.fun.ssa_state_reg.clone() {
                if let Some(&addr) = backend.ctx.mmio_fields.get(name) {
                    let p = format!("%gep_exit_{}", backend.fun.txn_counter);
                    backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                    writeln!(out, "{}{} = load volatile i64, i64* {}, align 1", indent, v, p).ok();
                } else if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                        let ll_ty = &backend.ctx.field_types[idx];
                        let brief_ty = backend.ctx.field_brief_types.get(idx).cloned().unwrap_or(Type::Int);
                        let ev = format!("%ev{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, idx).ok();
                        // 2026-06-29: Use field_brief_types to restore the correct Brief type.
                        // This handles Char→"i32", Int32→"i32" etc. correctly.
                        match brief_ty {
                            Type::Bool => {
                                let tr = format!("%tr_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, ev).ok();
                                return TypedRegister { name: tr, ty: Type::Bool };
                            }
                            Type::Float => {
                                let fc = backend.fun.txn_counter; backend.fun.txn_counter += 1;
                                let float_reg = format!("%flt_{}_{}", name, fc);
                                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, float_reg, ssa_reg, idx).ok();
                                backend.fun.reg_float_cache.insert(float_reg.clone(), float_reg.clone());
                                return TypedRegister { name: float_reg, ty: Type::Float };
                            }
                            Type::Float64 => {
                                let fc = backend.fun.txn_counter; backend.fun.txn_counter += 1;
                                let float_reg = format!("%flt_{}_{}", name, fc);
                                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, float_reg, ssa_reg, idx).ok();
                                return TypedRegister { name: float_reg, ty: Type::Float64 };
                            }
                            Type::Char => {
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ev).ok();
                                return TypedRegister { name: v.to_string(), ty: Type::Char };
                            }
                            Type::String | Type::Data => {
                                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ev).ok();
                                return TypedRegister { name: v.to_string(), ty: Type::Int };
                            }
                            // 2026-06-29: Fixed-width integer types — retain Brief type
                            Type::Int8 | Type::UInt8 => {
                                writeln!(out, "{}{} = add i8 0, {}", indent, v, ev).ok();
                                return TypedRegister { name: v.to_string(), ty: brief_ty };
                            }
                            Type::Int16 | Type::UInt16 => {
                                writeln!(out, "{}{} = add i16 0, {}", indent, v, ev).ok();
                                return TypedRegister { name: v.to_string(), ty: brief_ty };
                            }
                            Type::Int32 | Type::UInt32 => {
                                writeln!(out, "{}{} = add i32 0, {}", indent, v, ev).ok();
                                return TypedRegister { name: v.to_string(), ty: brief_ty };
                            }
                            _ => {
                                writeln!(out, "{}{} = add i64 0, {}", indent, v, ev).ok();
                                return TypedRegister { name: v.to_string(), ty: brief_ty };
                            }
                        };
                    }
                }
                if let Some(reg) = backend.fun.let_bindings.get(name) {
                    if let Some(ty) = backend.fun.let_binding_types.get(name) {
                        if *ty == Type::Float {
                            return TypedRegister { name: reg.clone(), ty: Type::Float };
                        }
                        // 2026-06-29: Float64 let-binding — return native double register
                        if *ty == Type::Float64 {
                            return TypedRegister { name: reg.clone(), ty: Type::Float64 };
                        }
                        if *ty == Type::Char {
                            // All Char registers from emit_expr are already i64.
                            // Copy the register as-is; no zext needed.
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
                            return TypedRegister { name: v.to_string(), ty: Type::Char };
                        }
                        if *ty == Type::Bool {
                            let z = format!("%iz_b{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = zext i1 {} to i64", indent, z, reg).ok();
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                            return TypedRegister { name: v.to_string(), ty: Type::Int };
                        }
                    }
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
                    if let Some(ty) = backend.fun.let_binding_types.get(name) {
                        return TypedRegister { name: v.to_string(), ty: ty.clone() };
                    }
                }
                if backend.ctx.trigger_names.contains(name) {
                    if let Some(sampled) = backend.sampled_triggers.get(name) {
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, sampled).ok();
                        return TypedRegister { name: v.to_string(), ty: Type::Int };
                    } else if let Some(t) = backend.ctx.triggers.get(name).cloned() {
                        // For built-in triggers (@stdin#, @timer#, @signal#), load from
                        // the state field (the event loop stored the value there).
                        if matches!(t.address, crate::ast::LinkRef::Stdin | crate::ast::LinkRef::Timer(_) | crate::ast::LinkRef::Signal(_)) {
                            if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                                let ll_ty = &backend.ctx.field_types[idx];
                                let sge = format!("%sge_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, sge, idx).ok();
                                let ev = format!("%ev_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                match ll_ty.as_str() {
                                    "i8" => { writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, ev, sge).ok(); }
                                    "i32" => { writeln!(out, "{}{} = load i32, i32* {}, align 4", indent, ev, sge).ok(); }
                                    _ => { writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ev, sge).ok(); }
                                }
                                backend.emit_trg_load_finish(out, indent, &v, ev, &t.ty);
                                return TypedRegister { name: v.to_string(), ty: t.ty.clone() };
                            } else {
                                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                                return TypedRegister { name: v.to_string(), ty: Type::Int };
                            }
                        } else if matches!(t.address, crate::ast::LinkRef::Explicit(0)) {
                            // Cell-binding triggers (trg name @ Console!) and other
                            // Explicit(0) triggers load from the %State field.
                            if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                                let ll_ty = &backend.ctx.field_types[idx];
                                let sge = format!("%sge_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, sge, idx).ok();
                                let ev = format!("%ev_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                match ll_ty.as_str() {
                                    "i8" => { writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, ev, sge).ok(); }
                                    "i32" => { writeln!(out, "{}{} = load i32, i32* {}, align 4", indent, ev, sge).ok(); }
                                    _ => { writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ev, sge).ok(); }
                                }
                                // 2026-06-28: String/Data types are boxed as i64 in %State.
                                // emit_trg_load_finish expects i8* for String; convert here.
                                if matches!(t.ty, Type::String | Type::Data) {
                                    let ip = format!("%tip_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, ip, ev).ok();
                                    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ip).ok();
                                } else {
                                    backend.emit_trg_load_finish(out, indent, &v, ev, &t.ty);
                                }
                                return TypedRegister { name: v.to_string(), ty: t.ty.clone() };
                            } else {
                                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                                return TypedRegister { name: v.to_string(), ty: Type::Int };
                            }
                        } else {
                            backend.emit_trg_load(out, indent, &v, &t.address, &t.ty);
                            return TypedRegister { name: v.to_string(), ty: t.ty.clone() };
                        }
                    } else {
                        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        return TypedRegister { name: v.to_string(), ty: Type::Int };
                    }
                } else if let Some((ty, expr)) = backend.ctx.constants.get(name) {
                    // Inline literal integer/bool constants as immediates
                    // instead of loading from global RAM.
                    match (ty, expr) {
                        (Type::Int | Type::UInt, Expr::Integer(n)) => {
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
                            return TypedRegister { name: v.to_string(), ty: Type::Int };
                        }
                        (Type::Bool, Expr::Bool(b)) => {
                            if *b {
                                writeln!(out, "{}{} = and i1 true, true", indent, v).ok();
                            } else {
                                writeln!(out, "{}{} = xor i1 true, true", indent, v).ok();
                            }
                            return TypedRegister { name: v.to_string(), ty: Type::Bool };
                        }
                        _ => {
                            // 2026-06-29: Handle Float64 constant loading (load as double, return native)
                            if *ty == Type::Float64 {
                                writeln!(out, "{}{} = load double, double* @{}, align 8", indent, v, name).ok();
                                backend.fun.reg_float_cache.insert(v.to_string(), v.to_string());
                                return TypedRegister { name: v.to_string(), ty: Type::Float64 };
                            }
                            let ll_ty = match ty {
                                Type::Float => "float",
                                Type::Int | Type::UInt => "i64",
                                Type::Bool => "i8",
                                _ => "i64",
                            };
                            let ld = format!("%il{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load {}, {}* @{}, align {}", indent, ld, ll_ty, ll_ty, name, backend.align_of(ll_ty)).ok();
                            let ret_ty = match ty {
                                Type::Float => {
                                    backend.fun.reg_float_cache.insert(ld.clone(), ld.clone());
                                    return TypedRegister { name: ld.clone(), ty: Type::Float };
                                }
                                Type::Bool => {
                                    let z = format!("%iz_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i8 {} to i1", indent, z, ld).ok();
                                    return TypedRegister { name: z, ty: Type::Bool };
                                }
                                _ => {
                                    writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
                                    ty.clone()
                                }
                            };
                            return TypedRegister { name: v.to_string(), ty: ret_ty };
                        }
                    }
                } else if let Some(&addr) = backend.ctx.mmio_fields.get(name) {
                    let p = format!("%mio{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                    writeln!(out, "{}{} = load volatile i64, i64* {}, align 1", indent, v, p).ok();
                } else if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let ty = &backend.ctx.field_types[idx];
                    let p = format!("%fdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, p, idx).ok();
                    let rng = backend.ctx.field_to_meta_idx.get(name).map(|m| format!(", !range !{}", m)).unwrap_or_default();
                    match ty {
                        s if s == "i8" => {
                            writeln!(out, "{}{} = load i8, i8* {}, align {}", indent, v, p, backend.align_of("i8")).ok();
                            let tr = format!("%tr_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, v).ok();
                            return TypedRegister { name: tr, ty: Type::Bool };
                        }
                        s if s == "float" => {
                            writeln!(out, "{}{} = load float, float* {}, align 4", indent, v, p).ok();
                            backend.fun.reg_float_cache.insert(v.to_string(), v.to_string());
                            return TypedRegister { name: v.to_string(), ty: Type::Float };
                        }
                        s if s == "double" => {
                            // 2026-06-29: Float64 field reads — load double, return Float64
                            writeln!(out, "{}{} = load double, double* {}, align 8", indent, v, p).ok();
                            backend.fun.reg_float_cache.insert(v.to_string(), v.to_string());
                            return TypedRegister { name: v.to_string(), ty: Type::Float64 };
                        }
                        s if s == "i8*" => {
                            let ld = format!("%ild{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i8*, i8** {}, align 8", indent, ld, p).ok();
                            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ld).ok();
                            return TypedRegister { name: v.to_string(), ty: Type::Int };
                        }
                        s if s == "i32" => {
                            let ld = format!("%il{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i32, i32* {}, align 4", indent, ld, p).ok();
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ld).ok();
                            return TypedRegister { name: v.to_string(), ty: Type::Char };
                        }
                        _ => {
                            writeln!(out, "{}{} = load {}, {}* {}, align {}{}", indent, v, ty, ty, p, backend.align_of(ty), rng).ok();
                            return TypedRegister { name: v.to_string(), ty: Type::Int };
                        }
                    }
                } else {
                    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                }
            }
            Expr::OwnedRef(name) => {
                // Redirect to Identifier — same semantics for LLVM
                return backend.emit_expr(out, &Expr::Identifier(name.clone()), indent);
            }
            Expr::PriorState(name) => {
                // Load the value from state BEFORE this tick's modifications.
                // The SSA state register holds the committed (pre-tick) value.
                if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let ll_ty = &backend.ctx.field_types[idx];
                    let ev = format!("%pev{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    if let Some(ref ssa_reg) = backend.fun.ssa_state_reg.clone() {
                        writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, idx).ok();
                        let field_ty = match ll_ty.as_str() {
                            "i8" => {
                                let tr = format!("%ptr_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, ev).ok();
                                return TypedRegister { name: tr, ty: Type::Bool };
                            }
                            "i32" => {
                                let z = format!("%piz_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, z, ev).ok();
                                writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                                return TypedRegister { name: v.to_string(), ty: Type::Char };
                            }
                            "float" => {
                                return TypedRegister { name: ev, ty: Type::Float };
                            }
                            _ => {
                                writeln!(out, "{}{} = add i64 0, {}", indent, v, ev).ok();
                                return TypedRegister { name: v.to_string(), ty: Type::Int };
                            }
                        };
                    }
                }
                panic!("emit_expr: PriorState field '{}' not found in field_index_map", name);
            }
            // 2026-06-29: Arithmetic/comparison/logical/bitwise dispatched to expr submodules
            Expr::Concat(l, r) => { let (a, b) = (backend.emit_expr(out, l, indent), backend.emit_expr(out, r, indent)); return backend.emit_inline_concat(out, indent, &a, &b); }
            // Call
            Expr::Call(name, args) => {
                // 2026-06-17: Inline negated (stdlib projection, not defined as a function)
                if name == "negated" && args.len() >= 1 {
                    let val = backend.emit_expr(out, &args[0], indent);
                    writeln!(out, "{}{} = sub i64 0, {}", indent, v, val.name).ok();
                    return TypedRegister { name: v.to_string(), ty: Type::Int };
                }
                // Clone foreign info upfront to avoid borrow conflict with emit_expr
                let frgn_sig: Option<(Vec<(String, Type)>, crate::ast::ResultType, bool, Option<crate::ast::Expr>, Vec<(String, Type)>)> =
                    backend.ctx.frgn_map.get(name).map(|s| (s.inputs.clone(), s.result_type.clone(), s.is_pipe, s.fallback.clone(), s.success_output.clone()));
                if let Some((inputs, ret_type, is_pipe, fallback, success_output)) = frgn_sig {
                    let mut marshaled: Vec<String> = Vec::new();
                    for (i, (_, arg_ty)) in inputs.iter().enumerate() {
                        if i < args.len() {
                            let raw = backend.emit_expr(out, &args[i], indent);
                            // Phase 3: Decay chimera arguments before FFI call
                            let raw = backend.emit_decay(out, &raw, Some(arg_ty), indent);
                            match arg_ty {
                                Type::Int | Type::UInt => marshaled.push(format!("i64 {}", raw)),
                                Type::Bool => {
                                    let boxed = backend.adapt_to_i64(out, indent, &raw);
                                    let z = format!("%fz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, boxed).ok();
                                    marshaled.push(format!("i32 {}", z));
                                }
                                Type::Char => {
                                    let boxed = backend.adapt_to_i64(out, indent, &raw);
                                    let z = format!("%fz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, boxed).ok();
                                    marshaled.push(format!("i32 {}", z));
                                }
                                Type::Float => {
                                    let fl = backend.ensure_float_reg(out, indent, &raw);
                                    marshaled.push(format!("float {}", fl));
                                }
                                Type::String | Type::Data => {
                                    let boxed = backend.adapt_to_i64(out, indent, &raw);
                                    let p = format!("%fp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, boxed).ok();
                                    marshaled.push(format!("i8* {}", p));
                                }
                                _ => marshaled.push(format!("i64 {}", raw)),
                            }
                        }
                    }
                    // Generic FFI call — no special-case magic
                    let is_float_ret = match &ret_type {
                        crate::ast::ResultType::Projection(ts) => ts.iter().any(|t| matches!(t, Type::Float)),
                        _ => false,
                    };
                    let call_ret = if is_float_ret { "float" } else { "i64" };
                    let args_str = marshaled.join(", ");
                    // 2026-06-28: Use txn_counter to prevent %t{N} collision
                    let call_result = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = call {} @{}({})", indent, call_result, call_ret, name, args_str).ok();

                    // Pipe-syntax frgn: emit sentinel checks using select (branchless).
                    // String/Data: null pointer → use fallback
                    // Float: NaN/Inf → use fallback
                    // Int/UInt/Bool/Char: always valid (no sentinel needed)
                    if is_pipe {
                        let success_ty = success_output.first()
                            .map(|(_, t)| t)
                            .cloned()
                            .unwrap_or(Type::Void);
                        let fallback_reg = fallback.as_ref().map(|e| backend.emit_expr(out, e, indent));

                        match (&success_ty, is_float_ret) {
                            (Type::String | Type::Data, _) => {
                                // Null pointer check for i8* returns
                                let is_null = format!("%pipe_null{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                // call_result is i64 (boxed ptr). Convert to i8* for null check.
                                let ptr = format!("%pipe_ptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, ptr, call_result).ok();
                                writeln!(out, "{}{} = icmp eq i8* {}, null", indent, is_null, ptr).ok();
                                // 2026-06-28: Use txn_counter to prevent %t{N} collision
                                let select_reg = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                let fbr = fallback_reg.as_ref().map(|r| r.name.as_str()).unwrap_or("null");
                                writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}",
                                    indent, select_reg, is_null, fbr, call_result).ok();
                                return TypedRegister { name: select_reg, ty: Type::Int };
                            }
                            (Type::Float, _) => {
                                // NaN check for float returns
                                let is_nan = format!("%pipe_nan{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = fcmp uno float {}, {}", indent, is_nan, call_result, call_result).ok();
                                // 2026-06-28: Use txn_counter to prevent %t{N} collision
                                let select_reg = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                let fbr = fallback_reg.as_ref().map(|r| r.name.as_str()).unwrap_or("0.0");
                                writeln!(out, "{}{} = select i1 {}, float {}, float {}",
                                    indent, select_reg, is_nan, fbr, call_result).ok();
                                let bi = format!("%fbi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                let ze = format!("%fze{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, select_reg).ok();
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                                backend.fun.reg_float_cache.insert(ze.clone(), select_reg.clone());
                                return TypedRegister { name: ze, ty: Type::Float };
                            }
                            _ => {
                                // Int/UInt/Bool/Char: always valid, just pass through
                                if is_float_ret {
                                    let bi = format!("%fbi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                    let ze = format!("%fze{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                    writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, call_result).ok();
                                    writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                                    backend.fun.reg_float_cache.insert(ze.clone(), call_result.clone());
                                    return TypedRegister { name: ze, ty: Type::Float };
                                }
                                return TypedRegister { name: call_result, ty: Type::Int };
                            }
                        }
                    }

                    if is_float_ret {
                        let bi = format!("%fbi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let ze = format!("%fze{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, call_result).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                        backend.fun.reg_float_cache.insert(ze.clone(), call_result.clone());
                        return TypedRegister { name: ze, ty: Type::Float };
                    }
                    return TypedRegister { name: call_result, ty: Type::Int };
                } else {
                    // Internal call — marshal i64 back to real types per definition
                    let def_tys: Option<Vec<Type>> = backend.ctx.defn_params.get(name).cloned();
                    let def_rets: Option<Vec<Type>> = backend.ctx.defn_return_types.get(name).cloned();
                    let mut a_strs = Vec::new();
                    for (ai, arg) in args.iter().enumerate() {
                        let raw = backend.emit_expr(out, arg, indent);
                        if let Some(ref tys) = def_tys {
                            if ai < tys.len() {
                                match &tys[ai] {
                                    Type::Bool => {
                                        let boxed = backend.adapt_to_i64(out, indent, &raw);
                                        let tr = format!("%ctr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, boxed).ok();
                                        a_strs.push(format!("i8 {}", tr));
                                    }
                                    Type::String | Type::Data => {
                                        let boxed = backend.adapt_to_i64(out, indent, &raw);
                                        let p = format!("%cip{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, boxed).ok();
                                        a_strs.push(format!("i8* {}", p));
                                    }
                                    Type::Float => {
                                        let fl = backend.ensure_float_reg(out, indent, &raw);
                                        a_strs.push(format!("float {}", fl));
                                    }
                                    _ => a_strs.push(format!("i64 {}", raw)),
                                }
                            } else {
                                a_strs.push(format!("i64 {}", raw));
                            }
                        } else {
                            // 2026-06-17: zext Bool/Char/Float to i64 for enum variant storage
                            let stored = if raw.ty == Type::Bool {
                                let z = format!("%cz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = zext i1 {} to i64", indent, z, raw.name).ok();
                                z
                            } else if raw.ty == Type::Char {
                                // Char registers are already i64 from emit_expr
                                raw.name.clone()
                            } else if raw.ty == Type::Float {
                                let bi = format!("%cfb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, raw.name).ok();
                                let ze = format!("%cfz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                                ze
                            } else {
                                raw.name.clone()
                            };
                            a_strs.push(format!("i64 {}", stored));
                        }
                    }
                    if name.starts_with(|c: char| c.is_uppercase()) && !backend.program_txns.contains(name) {
                        let disc_val = backend.ctx.variant_disc.get(name)
                            .map(|(_, d, _)| *d)
                            .unwrap_or(0u64);
                        let n_slots = a_strs.len() + 1;
                        let sz = format!("%csz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, 8", indent, sz, n_slots as i64).ok();
                        // Why malloc/arena for enum variants: tagged union requires heap
                        // allocation because different variants have different sizes.
                        // Arena handles this with bump alloc when in a loop context.
                        let pm = backend.emit_arena_alloc(out, indent, &sz);
                        let p = format!("%cop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, p, pm).ok();
                        let disc_gep = format!("%cdg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, disc_gep, p).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, disc_val, disc_gep).ok();
                        for (ai, arg_reg) in a_strs.iter().enumerate() {
                            let pay_gep = format!("%cpg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let parts: Vec<&str> = arg_reg.splitn(2, ' ').collect();
                            let rn = if parts.len() == 2 { parts[1] } else { arg_reg };
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, pay_gep, p, ai + 1).ok();
                            // 2026-06-17: Box float to i64 for enum storage
                            if parts.len() == 2 && (parts[0] == "float" || parts[0] == "float,") {
                                let bi = format!("%fbe{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, rn).ok();
                                let ze = format!("%fze{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, ze, pay_gep).ok();
                            } else {
                                eprintln!("DBG_store: arg_reg={:?}, parts={:?}, rn={:?}", arg_reg, parts, rn);
                                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, rn, pay_gep).ok();
                            }
                        }
                        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
                    } else {
                        // 2026-06-13: Pass %state to defns/callable txns — functions need
                        // the state pointer to access module-level fields (SSA is function-scoped).
                        let fn_name = if name == "main" && backend.ctx.defn_params.contains_key("main") {
                            "brief_main"
                        } else {
                            name
                        };
                        a_strs.insert(0, "ptr %state".to_string());
                        let is_float_ret = def_rets.as_ref().map_or(false, |rets| rets.iter().any(|t| matches!(t, Type::Float)));
                        let is_string_ret = def_rets.as_ref().map_or(false, |rets| rets.iter().any(|t| matches!(t, Type::String) || matches!(t, Type::Data)));
                        let call_ret = if is_float_ret { "float" } else { "i64" };
                        writeln!(out, "{}{} = call {} @{}({})", indent, v, call_ret, fn_name, a_strs.join(", ")).ok();
                        if is_float_ret {
                            return TypedRegister { name: v.to_string(), ty: Type::Float };
                        }
                        // Internal calls return i64 (boxed), so mark as Type::Int.
                        // Previously returned Type::String/Type::Bool for string/bool ret,
                        // but that confused downstream native-type handling.
                        return TypedRegister { name: v.to_string(), ty: Type::Int };
                }
            }
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
            // Without this, the result is Type::Int and the struct lookup fails.
            Expr::ListIndex(list, index) => {
                let list_val = backend.emit_expr(out, list, indent);
                let idx_val = backend.emit_expr(out, index, indent);
                let hp = format!("%xhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_val.name).ok();
                let dp = format!("%xdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                let de = format!("%xde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                let ep = format!("%xep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, de, idx_val.name).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, ep).ok();
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
                if let Some(et) = el_ty {
                    return TypedRegister { name: v.to_string(), ty: et };
                }
            }
            // ── Projection ──────────────────────────────────────
            Expr::Projection { source, target } => {
                // Function metadata projections — source is a function name, not a runtime value.
                if let Some(result) = backend.try_emit_fn_projection(out, source, target, indent) {
                    return result;
                }
                let src_val = backend.emit_expr(out, &*source, indent);
                // Phase 2: Check if this is a cached projection (Hot Dual path).
                let target_name = crate::analysis::transition_graph::projection_target_name(target);
                if let Some(tr) = backend.try_cached_projection(out, source.as_ref(), &src_val, &target_name, indent) {
                    return tr;
                }
                // Phase 2: Check if the source type has a meld route for this projection target.
                if let Some(tr) = backend.try_meld_projection(out, &src_val, &target_name, indent) {
                    return tr;
                }
                match target {
                    ProjectionTarget::Size => {
                        if matches!(source.as_ref(),
                            Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_))
                        {
                            writeln!(out, "{}{} = add i64 0, 1", indent, v).ok();
                        } else {
                            let hp = format!("%php{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                            let lp = format!("%plp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, lp).ok();
                        }
                    }
                    ProjectionTarget::Bytes => {
                        let bs = match &src_val.ty {
                            Type::Float => 4,
                            Type::Int | Type::UInt => 8,
                            Type::Bool => 1,
                            Type::Char => 4,
                            Type::String | Type::Data => 8,
                            Type::Custom(name) => {
                                match backend.ctx.struct_types.get(name) {
                                    Some(fields) => fields.len() as i64 * 8,
                                    None => {
                                        panic!("emit_expr: Bytes projection on unknown struct type '{:?}'", name);
                                        return TypedRegister { name: v.to_string(), ty: Type::Int };
                                    }
                                }
                            }
                            _ => {
                                panic!("emit_expr: Bytes projection on unknown type {:?}", src_val.ty);
                                return TypedRegister { name: v.to_string(), ty: Type::Int };
                            }
                        };
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, bs).ok();
                    }
                    ProjectionTarget::Ptr => {
                        writeln!(out, "{}{} = add i64 0, {} ; ptr", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::Alignment => {
                        writeln!(out, "{}{} = add i64 0, 8 ; alignment", indent, v).ok();
                    }
                    ProjectionTarget::Type => {
                        let tid = match src_val.ty {
                            Type::Int | Type::UInt => 1i64,
                            Type::Bool => 2,
                            Type::Char => 3,
                            Type::String | Type::Data => 4,
                            Type::Float => 5,
                            Type::Custom(_) => 6,
                            Type::Void => 0,
                            _ => 0,
                        };
                        writeln!(out, "{}{} = add i64 0, {} ; type", indent, v, tid).ok();
                    }
                    ProjectionTarget::Popcount => {
                        writeln!(out, "{}{} = call i64 @llvm.ctpop.i64(i64 {})", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::LeadingZeros => {
                        writeln!(out, "{}{} = call i64 @llvm.ctlz.i64(i64 {}, i1 false)", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::TrailingZeros => {
                        writeln!(out, "{}{} = call i64 @llvm.cttz.i64(i64 {}, i1 false)", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::Absolute => {
                        writeln!(out, "{}{} = call i64 @llvm.abs.i64(i64 {}, i1 false)", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::BitReverse => {
                        writeln!(out, "{}{} = call i64 @llvm.bitreverse.i64(i64 {})", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::Keys => {
                        writeln!(out, "{}{} = call i64 @__map_keys__(i64 {})", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::Values => {
                        writeln!(out, "{}{} = call i64 @__map_values__(i64 {})", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::AsStack | ProjectionTarget::AsQueue => {
                        writeln!(out, "{}{} = add i64 0, {} ; as_stack/as_queue (identity)", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::PtrBang => {
                        let hp = format!("%pbhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, hp).ok();
                    }
                    ProjectionTarget::Contains(expr) => {
                        // Linear search over list elements
                        let search_val = backend.emit_expr(out, expr, indent);
                        let search_boxed = backend.adapt_to_i64(out, indent, &search_val);
                        let hp = format!("%pchp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                        let lp = format!("%pclp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                        let len = format!("%pcln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                        let dp = format!("%pcdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, hp).ok();
                        // Emit a linear search loop
                        let e_l = format!("pc_entry{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let h_l = format!("pc_hdr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let b_l = format!("pc_body{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let f_l = format!("pc_found{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let d_l = format!("pc_done{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let i_r = format!("%pci{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let c_r = format!("%pcc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let el_r = format!("%pce{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let eq_r = format!("%pceq{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let n_r = format!("%pcn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}br label %{}", indent, e_l).ok();
                        writeln!(out, "{}{}:", indent, e_l).ok();
                        writeln!(out, "{}br label %{}", indent, h_l).ok();
                        writeln!(out, "{}{}:", indent, h_l).ok();
                        writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, i_r, e_l, n_r, b_l).ok();
                        writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, c_r, i_r, len).ok();
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, c_r, b_l, d_l).ok();
                        writeln!(out, "{}{}:", indent, b_l).ok();
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, el_r, dp, i_r).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, eq_r, el_r).ok();
                        writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, eq_r, eq_r, search_boxed).ok();
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, eq_r, f_l, h_l).ok();
                        writeln!(out, "{}{} = add i64 {}, 1", indent, n_r, i_r).ok();
                        writeln!(out, "{}br label %{}", indent, h_l).ok();
                        writeln!(out, "{}{}:", indent, f_l).ok();
                        writeln!(out, "{}br label %{}", indent, d_l).ok();
                        writeln!(out, "{}{}:", indent, d_l).ok();
                        writeln!(out, "{}{} = phi i1 [ false, %{} ], [ true, %{} ]", indent, v, e_l, f_l).ok();
                        return TypedRegister { name: v.to_string(), ty: Type::Bool };
                    }
                    ProjectionTarget::Range => {
                        // Return list length (same as Size) — Range = [0, len)
                        let hp = format!("%prhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                        let lp = format!("%prlp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, lp).ok();
                    }
                    ProjectionTarget::Top => {
                        writeln!(out, "{}{} = call i64 @__stack_top__(i64 {})", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::Front => {
                        writeln!(out, "{}{} = call i64 @__queue_front__(i64 {})", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::Get(expr) => {
                        let key_val = backend.emit_expr(out, expr, indent);
                        let key_boxed = backend.adapt_to_i64(out, indent, &key_val);
                        writeln!(out, "{}{} = call i64 @__hashmap_get__(i64 {}, i64 {})", indent, v, src_val.name, key_boxed).ok();
                    }
                    ProjectionTarget::Elements => {
                        writeln!(out, "{}{} = add i64 0, {} ; elements", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::IsEmpty => {
                        let hp = format!("%ieh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                        let lp = format!("%iel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                        let len = format!("%ien{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                        writeln!(out, "{}{} = icmp eq i64 {}, 0", indent, v, len).ok();
                        writeln!(out, "{}{} = zext i1 {} to i64", indent, v, v).ok();
                    }
                    ProjectionTarget::UserDefinedWithArg(name, arg_expr) => {
                        // Phase 3.5: Fast-path for well-known operator projections
                        if let Some(tr) = backend.try_projection_fast_path(out, &src_val, name.as_str(), arg_expr, indent, &v) {
                            return tr;
                        }
                        panic!("emit_expr: unhandled UserDefinedWithArg projection '{}'", name);
                        return TypedRegister { name: v.to_string(), ty: Type::Int };
                    }
                    ProjectionTarget::UserDefined(_) => {
                        panic!("emit_expr: unhandled UserDefined projection (no fast-path matched)");
                        return TypedRegister { name: v.to_string(), ty: Type::Int };
                    }
                    ProjectionTarget::BitRange(br) => {
                        // Extract bits via lshr + and
                        let (lo, hi) = match br {
                            crate::ast::BitRange::Single(i) => (*i, *i),
                            crate::ast::BitRange::Range(l, h) => (*l, *h),
                            crate::ast::BitRange::Any(w) => (0, *w - 1),
                        };
                        let width = hi - lo + 1;
                        let shifted = format!("%pbr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = lshr i64 {}, {}", indent, shifted, src_val.name, lo).ok();
                        if width >= 64 {
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, shifted).ok();
                        } else {
                            let mask_lit = (1u64 << width) - 1;
                            writeln!(out, "{}{} = and i64 {}, {}", indent, v, shifted, mask_lit).ok();
                        }
                    }
                    _ => {
                        writeln!(out, "{}{} = add i64 0, 0 ; projection catch-all", indent, v).ok();
                    }
                }
            }
            // ── StructInstance ──────────────────────────────────
             Expr::StructInstance(name, fields) => {
                let n = fields.len() as i64;
                let ai = format!("%sai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, n).ok();
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    let fv = backend.emit_expr(out, fval, indent);
                    let fp = format!("%sfp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, fp, ai, i as i64).ok();
                     let stored = if fv.ty == Type::Bool || fv.ty == Type::Char || fv.ty == Type::Float || fv.ty == Type::String {
                         backend.adapt_to_i64(out, indent, &fv)
                     } else { fv.name.clone() };
                     writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, stored, fp).ok();
                 }
                 writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
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
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, fp, ai, i as i64).ok();
                    let stored = if fv.ty == Type::Bool || fv.ty == Type::Char || fv.ty == Type::Float || fv.ty == Type::String {
                        backend.adapt_to_i64(out, indent, &fv)
                    } else { fv.name.clone() };
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, stored, fp).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
            }
            // ── FieldAccess ─────────────────────────────────────
            Expr::FieldAccess(obj, field) => {
                let obj_val = backend.emit_expr(out, obj, indent);
                let hp = format!("%fahp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, obj_val.name).ok();
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
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, fp, hp, offset).ok();
                    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, fp).ok();
                    // 2026-06-17: Return Float type for float fields so downstream
                    // code (emit_binop) correctly identifies them. String/Data fields
                    // remain Type::Int (stored boxed as i64 in struct).
                    let lookup_ty = || -> Option<Type> {
                        if let Expr::Identifier(name) = obj.as_ref() {
                            if let Some(Type::Custom(struct_name)) = backend.fun.let_binding_types.get(name) {
                                if let Some(fields) = backend.ctx.struct_types.get(struct_name) {
                                    let fi = offset as usize;
                                    if fi < fields.len() {
                                        let (_, field_ty) = &fields[fi];
                                        if matches!(field_ty, Type::Float) {
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
                                    if matches!(field_ty, Type::Float) {
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
                } else {
                    panic!("emit_expr: FieldAccess: field '{}' not found on object", field);
                    return TypedRegister { name: v.to_string(), ty: Type::Int };
                }
            }
            // ── PatternMatch ────────────────────────────────────
            Expr::PatternMatch { value, variant, fields } => {
                let src_val = backend.emit_expr(out, value, indent);
                let hp = format!("%php{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                let disc = format!("%pdisc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, disc, hp).ok();
                let expected = backend.ctx.variant_disc.get(variant)
                    .map(|(_, d, _)| *d as i64)
                    .unwrap_or(0);
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, v, disc, expected).ok();
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
                                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, rhp, result_reg.name).ok();
                                rhp
                            } else {
                                let ihp = format!("%mihp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, ihp, result_reg.name).ok();
                                ihp
                            };
                            let dp = format!("%mdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let cv = backend.emit_expr(out, idx_expr, indent);
                            let ep = format!("%mep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, de, cv.name).ok();
                            let lv = format!("%mlv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, lv, ep).ok();
                            result_reg = TypedRegister { name: lv, ty: Type::Int };
                            reboxed = false;
                        }
                        BracketOp::Coord(SliceCoordinate::Range { start, end }) => {
                            // Extract sub-range [start, end) into a new list
                            let hp = format!("%mrhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, result_reg.name).ok();
                            let dp = format!("%mrdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mrde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let slp = format!("%mrlp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, hp).ok();
                            let src_len = format!("%mrsl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, src_len, slp).ok();
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
                            writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, rai, rrm).ok();
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
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, r_gep, de, r_src).ok();
                            let r_el = format!("%mrel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, r_el, r_gep).ok();
                            let r_dst = format!("%mrdst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, r_dst, rai, ri).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, r_el, r_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, rn, ri).ok();
                            writeln!(out, "{}br label %{}", indent, r_hdr).ok();
                            writeln!(out, "{}{}:", indent, r_done).ok();
                            // Store header (data_ptr, length)
                            let r_dpp = format!("%mrdpp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, r_dpp, rai).ok();
                            let r_dpv = format!("%mrdpv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, r_dpv, r_dpp).ok();
                            let rs0 = format!("%mrs0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, rs0, rai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, r_dpv, rs0).ok();
                            let rs1 = format!("%mrs1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, rs1, rai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, rcnt, rs1).ok();
                            let rv = format!("%mrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, rv, rai).ok();
                            result_reg = TypedRegister { name: rv, ty: Type::Int };
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
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, result_reg.name).ok();
                            let dp = format!("%msdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%msde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let lp = format!("%mslp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                            let len = format!("%mslen{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, len, lp).ok();
                            // Allocate stride-filtered buffer
                            let sab = format!("%msab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = mul i64 {}, 8", indent, sab, len).ok();
                            let srm = backend.emit_arena_alloc(out, indent, &sab);
                            let sai = format!("%msai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, sai, srm).ok();
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
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, s_gep, de, sj).ok();
                            let s_el = format!("%msel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, s_el, s_gep).ok();
                            let s_dst = format!("%msdst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, s_dst, sai, sk).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, s_el, s_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, {}", indent, sn, sj, sv.name).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, snk, sk).ok();
                            writeln!(out, "{}br label %{}", indent, s_hdr).ok();
                            writeln!(out, "{}{}:", indent, s_done).ok();
                            // Store header
                            let s_dpp = format!("%msdpp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, s_dpp, sai).ok();
                            let s_dpv = format!("%msdpv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, s_dpv, s_dpp).ok();
                            let ss0 = format!("%mss0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, ss0, sai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, s_dpv, ss0).ok();
                            let ss1 = format!("%mss1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ss1, sai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, sk, ss1).ok();
                            let sv_reg = format!("%msv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, sv_reg, sai).ok();
                            result_reg = TypedRegister { name: sv_reg, ty: Type::Int };
                            reboxed = true;
                        }
                        BracketOp::Mask(mask_expr) => {
                            // Element-wise filter: evaluate mask for each element
                            let hp = format!("%mmhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, result_reg.name).ok();
                            let dp = format!("%mmdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mmde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let lp = format!("%mmlp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                            let len = format!("%mmlen{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, len, lp).ok();
                            // Allocate mask-filtered buffer (max size = len)
                            let mab = format!("%mmab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = mul i64 {}, 8", indent, mab, len).ok();
                            let mrm = backend.emit_arena_alloc(out, indent, &mab);
                            let mai = format!("%mmai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, mai, mrm).ok();
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
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, m_gep, de, mj).ok();
                            let m_el = format!("%mmel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, m_el, m_gep).ok();
                            // Bind _ to element, evaluate mask
                            backend.fun.let_bindings.insert("_".to_string(), m_el.clone());
                            let mask_r = backend.emit_expr(out, mask_expr, indent);
                            let mask_b = backend.as_bool_reg(out, indent, &mask_r);
                            let m_store_l = format!("mm_store{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            let m_skip_l = format!("mm_skip{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, mask_b, m_store_l, m_skip_l).ok();
                            writeln!(out, "{}{}:", indent, m_store_l).ok();
                            let m_dst = format!("%mmdst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, m_dst, mai, mk).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_el, m_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, mnk, mk).ok();
                            writeln!(out, "{}br label %{}", indent, m_skip_l).ok();
                            writeln!(out, "{}{}:", indent, m_skip_l).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, mn, mj).ok();
                            writeln!(out, "{}br label %{}", indent, m_hdr).ok();
                            writeln!(out, "{}{}:", indent, m_done).ok();
                            // Store header
                            let m_dpp = format!("%mmdpp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, m_dpp, mai).ok();
                            let m_dpv = format!("%mmdpv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, m_dpv, m_dpp).ok();
                            let ms0 = format!("%mms0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, ms0, mai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_dpv, ms0).ok();
                            let ms1 = format!("%mms1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ms1, mai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, mk, ms1).ok();
                            let mv_reg = format!("%mmv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, mv_reg, mai).ok();
                            result_reg = TypedRegister { name: mv_reg, ty: Type::Int };
                            reboxed = true;
                        }
                    }
                }
                writeln!(out, "{}{} = add i64 0, {}", indent, v, result_reg.name).ok();
                backend.fun.let_bindings = saved_bindings;
            }
            // ── Match ───────────────────────────────────────────
            Expr::Match { value, arms } => {
                let saved_bindings = backend.fun.let_bindings.clone();
                let saved_types = backend.fun.let_binding_types.clone();
                let val = backend.emit_expr(out, value, indent);
                let hp = format!("%mhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, val.name).ok();
                let disc_reg = format!("%mdisc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, disc_reg, hp).ok();

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
                                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, gep, hp, (j as i64) + 1).ok();
                                let fv = format!("%mfv{}_{}", i, j);
                                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, fv, gep).ok();
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
                    Type::String
                } else {
                    Type::Int
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
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                let dp = format!("%sdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                let de = format!("%sde{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                let src_len_reg = format!("%sln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let slp = format!("%slp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, hp).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, src_len_reg, slp).ok();

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
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, ai, rm).ok();

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
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, src_ep, de, src_idx).ok();
                let elem = format!("%selem{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, elem, src_ep).ok();
                // Store to dest[2 + i]
                let dst_idx = format!("%sdi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 2", indent, dst_idx, i_reg).ok();
                let dst_ep = format!("%sdep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, dst_ep, ai, dst_idx).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem, dst_ep).ok();
                writeln!(out, "{}{} = add i64 {}, 1", indent, next_reg, i_reg).ok();
                writeln!(out, "{}br label %{}", indent, header_label).ok();
                writeln!(out, "{}{}:", indent, done_label).ok();
                // Store data_ptr and length in the strided-result header
                let dp_ptr = format!("%sdp2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp_ptr, ai).ok();
                let dp_val = format!("%sdv2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, dp_val, dp_ptr).ok();
                let s0 = format!("%ss0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, s0, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp_val, s0).ok();
                let s1 = format!("%ss1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, s1, ai).ok();
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
                    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, m_ai, m_rm).ok();
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
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, m_gep, old_ai, m_j).ok();
                    let m_elem = format!("%smelem{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, m_elem, m_gep).ok();
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
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, m_dst, m_ai, m_k).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_elem, m_dst).ok();
                    writeln!(out, "{}{} = add i64 {}, 1", indent, m_next, m_k).ok();
                    writeln!(out, "{}br label %{}", indent, m_next_label).ok();
                    writeln!(out, "{}{}:", indent, m_next_label).ok();
                    writeln!(out, "{}{} = add i64 {}, 1", indent, m_next, m_j).ok();
                    writeln!(out, "{}br label %{}", indent, m_hdr).ok();

                    writeln!(out, "{}{}:", indent, m_done).ok();
                    // Store filtered header
                    let m_dp_ptr = format!("%smdp2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, m_dp_ptr, m_ai).ok();
                    let m_dp_val = format!("%smdv2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, m_dp_val, m_dp_ptr).ok();
                    let ms0 = format!("%sms0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, ms0, m_ai).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_dp_val, ms0).ok();
                    let ms1 = format!("%sms1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ms1, m_ai).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_k, ms1).ok();
                    writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, m_ai).ok();
                    backend.fun.let_bindings = saved_bindings;
                } else {
                    writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
                }
            }
            // — Subtype projection (e.g. list :> Size) —
            Expr::SubtypeProjection { source, .. } => {
                let src = backend.emit_expr(out, source, indent);
                let hp = format!("%shp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src.name).ok();
                let slp = format!("%slp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, hp).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, slp).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Int };
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
                return TypedRegister { name: v.to_string(), ty: Type::Bool };
            }
            Expr::FromCheck(expr, _ty) => {
                let _ = backend.emit_expr(out, expr, indent);
                writeln!(out, "{}{} = add i64 0, 1 ; from (compile-time)", indent, v).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Bool };
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
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, hp, ai).ok();
                let base = format!("%mba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, ai).ok();
                let dp = format!("%mdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, hp).ok();
                let ml1 = format!("%mml1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ml1, hp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, n, ml1).ok();
                // Store values (keys are compile-time in the source map literal)
                for (i, (_key, val)) in items.iter().enumerate() {
                    let kv = backend.emit_expr(out, val, indent);
                    let kvs = backend.adapt_to_i64(out, indent, &kv);
                    let ep = format!("%mep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, kvs, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ai).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Int };
            }
            Expr::SetLiteral(items) => {
                let n = items.len() as i64;
                let set_alloc_size = format!("%sas{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, set_alloc_size, (n + 2) * 8 + 8).ok();
                let ai = backend.emit_arena_alloc(out, indent, &set_alloc_size);
                let hp = format!("%shp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, hp, ai).ok();
                let base = format!("%sba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, ai).ok();
                let dp = format!("%sdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, hp).ok();
                let sl1 = format!("%ssl1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, sl1, hp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, n, sl1).ok();
                for (i, item) in items.iter().enumerate() {
                    let iv = backend.emit_expr(out, item, indent);
                    let ivs = backend.adapt_to_i64(out, indent, &iv);
                    let ep = format!("%sep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ivs, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ai).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Int };
            }
            // Why free+malloc+memcpy instead of realloc: Brief collections have
            // immutable-value semantics — `<-` produces a new list, the old one is
            // dead (no shared refs). realloc doesn't help (we still memcpy to make
            // room) and the old→new ptr mapping adds complexity. The free+malloc
            // pattern makes allocation visible to LLVM's malloc optimization passes.
            Expr::ArrowMut { dir: ArrowDir::Push, target, index: _, value: Some(val) } => {
                let list_val = backend.emit_expr(out, target, indent);
                let elem_val = backend.emit_expr(out, val, indent);
                let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
                let elem_boxed = backend.adapt_to_i64(out, indent, &elem_val);
                // Check InsertAt strategy: Custom functions get an early call emission,
                // built-in strategies determine prepend vs append behavior.
                let push_strategy = backend.check_insert_strategy(target);
                if let Some(crate::type_universe::InsertStrategy::Custom(fn_name)) = &push_strategy {
                    // Custom push: emit call @fn_name(i64, i64) -> i64
                    writeln!(out, "{}{} = call i64 @{}(i64 {}, i64 {})", indent, v, fn_name, list_boxed, elem_boxed).ok();
                    // Store new list handle back to state field if target is OwnedRef
                    if let Expr::OwnedRef(field_name) = target.as_ref() {
                        if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                            let ap = format!("%aap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                            let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, v, ap, tn).ok();
                        } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
                            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, v, slot).ok();
                        }
                    }
                    return TypedRegister { name: v.to_string(), ty: Type::Int };
                }
                let prepend = matches!(push_strategy, Some(crate::type_universe::InsertStrategy::Prepend));
                // Unbox list header: inttoptr i64 to i64*
                let hp = format!("%ahp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                // Read current length from header slot 1
                let lp = format!("%alp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let old_len = format!("%aol{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, old_len, lp).ok();
                // Phase 2 fast path: if preallocated capacity exists and
                // length < capacity, write directly without alloc/memcpy.
                // Only works for append — prepend requires element shifting
                // which the fast path doesn't support. The slow_l label is
                // emitted here (always) so the branch target exists even
                // when the fast path returns early.
                let slow_l = format!("push_slow_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                // Track whether we emitted a branch to slow_l. If not (e.g.
                // prepend mode or no prealloc info), we need a br label %slow_l
                // to terminate the preceding basic block before the label.
                let mut emitted_slow_branch = false;
                if !prepend {
                if let Expr::OwnedRef(field_name) = target.as_ref() {
                    if let Some((cap_reg, buf_i64)) = backend.fun.field_prealloc_info.get(field_name.as_str()).cloned() {
                        let cap_check = format!("%acap{}", backend.fun.txn_counter);
                        backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = icmp ult i64 {}, {}", indent, cap_check, old_len, cap_reg).ok();
                        let fast_l = format!("push_fast_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cap_check, fast_l, slow_l).ok();
                        emitted_slow_branch = true;
                        // Fast path: write element at header[2 + old_len], increment length.
                        // Uses buf_i64 (preallocated i64* buffer from prealloc_info)
                        // rather than hp (which alias the same memory but may be stale
                        // after the first iteration resets state via the normal store path).
                        writeln!(out, "{}{}:", indent, fast_l).ok();
                        let el_off = format!("%apfo{}", backend.fun.txn_counter);
                        backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = add i64 {}, 2", indent, el_off, old_len).ok();
                        let el_gep = format!("%apfg{}", backend.fun.txn_counter);
                        backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, el_gep, buf_i64, el_off).ok();
                        let new_len_fast = format!("%apfn{}", backend.fun.txn_counter);
                        backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = add i64 {}, 1", indent, new_len_fast, old_len).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem_boxed, el_gep).ok();
                        // Update length in header slot 1 of the preallocated buffer
                        let len_gep = format!("%apfl{}", backend.fun.txn_counter);
                        backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, len_gep, buf_i64).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len_fast, len_gep).ok();
                        // Store back to state (buffer pointer unchanged — we modified
                        // the preallocated buffer in-place, no new allocation).
                        let store_idx = backend.ctx.field_index_map[field_name];
                        let ap = format!("%aapf{}", backend.fun.txn_counter);
                        backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, store_idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[store_idx]);
                        let base_fast = format!("%apfb{}", backend.fun.txn_counter);
                        backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, base_fast, buf_i64).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base_fast, ap, tn).ok();
                        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, buf_i64).ok();
                        return TypedRegister { name: v.to_string(), ty: Type::Int };
                    }
                }
                }
                // 2026-06-26: If the fast-path branch was not emitted (e.g.
                // prepend mode or no prealloc info), terminate the preceding
                // block with br %slow_l so LLVM does not see an unterminated
                // basic block before the label. The push_slow label was always
                // emitted unconditionally, but its preceding br i1 only fired
                // when prealloc info existed — leaving the block unterminated
                // ("expected instruction opcode") for all other cases.
                if !emitted_slow_branch {
                    writeln!(out, "{}br label %{}", indent, slow_l).ok();
                }
                writeln!(out, "{}{}:", indent, slow_l).ok();
                // Allocate: when inside an arena scope (loop/tick), use bump
                // alloc (no free — arena resets at scope exit). Outside a
                // scope, fall back to per-operation malloc via emit_arena_alloc.
                let new_cnt = format!("%anc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 3", indent, new_cnt, old_len).ok();
                let alloc_bytes = format!("%aab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
                let new_buf = backend.emit_arena_alloc(out, indent, &alloc_bytes);
                // Free old buffer: when arena is active, the arena owns all
                // memory — no per-operation free needed. When arena is inactive
                // (standalone call), free the old buffer normally.
                if backend.fun.arena_slots.is_none() {
                    let old_ptr = format!("%aop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
                }
                // Set header: data_ptr at slot 0, new length at slot 1
                let new_hp = format!("%anh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
                let base = format!("%aba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
                let dp = format!("%adp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, new_hp).ok();
                let nlp = format!("%anp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
                let new_len = format!("%anl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, new_len, old_len).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
                // Copy old elements: for prepend, shift right by 1; for append, same position
                let old_dp = format!("%aod{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, old_dp, hp).ok();
                let copy_dst = if prepend {
                    // Prepend: copy to position 1 (one slot after base)
                    let cd = format!("%acd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 3", indent, cd, new_hp).ok();
                    cd
                } else {
                    // Append: copy to position old_len (same position as before)
                    let cd = format!("%acd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, cd, new_hp).ok();
                    cd
                };
                let copy_bytes = format!("%acb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, copy_bytes, old_len).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, copy_dst, old_dp, copy_bytes).ok();
                // Store new element at position 0 for prepend, or old_len+2 for append
                // (list header is 2 slots: capacity at 0, length at 1; elements start at 2).
                let ne_ptr = format!("%aep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let new_elem_pos = if prepend {
                    "2".to_string()
                } else {
                    // 2026-06-27: Compute element position = old_len + 2 account for
                    // the 2 header slots. Previously used old_len directly, which
                    // overwrote header slot 0 or the first element (queue_drain crash).
                    let ep = format!("%aep2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = add i64 {}, 2", indent, ep, old_len).ok();
                    ep
                };
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ne_ptr, new_hp, new_elem_pos).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem_boxed, ne_ptr).ok();
                // Store new list handle back to state field if target is OwnedRef
                if let Expr::OwnedRef(field_name) = target.as_ref() {
                    if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                        let ap = format!("%aap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
                        // 2026-06-27: Record field as modified for per-field phi
                        // back-edge reload. Without this, the phi back-edge for
                        // this field remains a pass-through (old value), causing
                        // the next tick to see the pre-push handle (queue_drain).
                        backend.fun.pending_phi_backedge.insert(field_name.clone(), base.clone());
                    } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
                    }
                }
                // Return new list handle (ptrtoint of new buffer)
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, new_buf).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Int };
            }
            // Why free+malloc+memcpy for pop: same semantics as push — the old
            // buffer is dead after the operation. Pop removes one element but
            // we still allocate a fresh buffer of len-1. An arena allocator
            // (planned) would replace the free+malloc with a bump pointer reset.
            Expr::ArrowMut { dir: ArrowDir::Pop, target, index, value: None } => {
                let pop_strategy = backend.check_extract_strategy(target);
                if let Some(crate::type_universe::ExtractStrategy::Custom(fn_name)) = &pop_strategy {
                    // Custom extract: call @fn_name(i64) -> { i64, i64 }
                    let list_val = backend.emit_expr(out, target, indent);
                    let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
                    let call_reg = format!("%pc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = call {{ i64, i64 }} @{}(i64 {})", indent, call_reg, fn_name, list_boxed).ok();
                    // Extract popped value (index 0) and new collection handle (index 1)
                    writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 0", indent, v, call_reg).ok();
                    let new_list = format!("%pnl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = extractvalue {{ i64, i64 }} {}, 1", indent, new_list, call_reg).ok();
                    // Store new list handle back to state
                    if let Expr::OwnedRef(field_name) = target.as_ref() {
                        if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                            let ap = format!("%pap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                            let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, new_list, ap, tn).ok();
                            // 2026-06-27: Record field as modified for per-field
                            // phi back-edge reload (same rationale as push).
                            backend.fun.pending_phi_backedge.insert(field_name.clone(), new_list.clone());
                        } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
                            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, new_list, slot).ok();
                        }
                    }
                    let popped = v.clone();
                    return TypedRegister { name: popped.to_string(), ty: Type::Int };
                }
                // Determine pop index: Shift strategy removes from front (0), otherwise end (len-1)
                let should_shift = matches!(pop_strategy, Some(crate::type_universe::ExtractStrategy::Shift));
                let list_val = backend.emit_expr(out, target, indent);
                let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
                // Unbox list header
                let hp = format!("%php{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                // Read length
                let lp = format!("%plp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let len = format!("%pln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                // Compute target index: 0 for shift, len - 1 for pop, or expression value
                let pop_idx = format!("%ppi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                match index.as_ref() {
                    Expr::Term if should_shift => {
                        writeln!(out, "{}{} = add i64 0, 0", indent, pop_idx).ok();
                    }
                    Expr::Term => {
                        writeln!(out, "{}{} = add i64 {}, -1", indent, pop_idx, len).ok();
                    }
                    other => {
                        let idx_val = backend.emit_expr(out, other, indent);
                        let idx_boxed = backend.adapt_to_i64(out, indent, &idx_val);
                        writeln!(out, "{}{} = add i64 {}, 0", indent, pop_idx, idx_boxed).ok();
                    }
                }
                // Load popped element from data_ptr[pop_idx]
                let dp = format!("%pdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, hp).ok();
                let ep = format!("%pep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, dp, pop_idx).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, ep).ok();
                let popped = v.clone();
                // Free old buffer: arena-active skips per-op free; standalone frees
                if backend.fun.arena_slots.is_none() {
                    let old_ptr = format!("%pop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
                }
                // Allocate new buffer: (len + 1) * 8 (2 header + len - 1 elements)
                let new_cnt = format!("%pnc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, new_cnt, len).ok();
                let alloc_bytes = format!("%pab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
                let new_buf = backend.emit_arena_alloc(out, indent, &alloc_bytes);
                // Set header
                let new_hp = format!("%pnh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
                let base = format!("%pba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
                let new_dp_val = format!("%pnd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, new_dp_val, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_dp_val, new_hp).ok();
                let nlp = format!("%pnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
                let new_len = format!("%pnl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, -1", indent, new_len, len).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
                // Copy elements before pop_idx
                let ndp = format!("%pndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
                let bef_bytes = format!("%pbb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, bef_bytes, pop_idx).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, ndp, dp, bef_bytes).ok();
                // Copy elements after pop_idx
                let after_off = format!("%pao{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, after_off, pop_idx).ok();
                let aft_src = format!("%pas{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_src, dp, after_off).ok();
                let aft_dst = format!("%pad{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_dst, ndp, pop_idx).ok();
                let aft_cnt = format!("%pac{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = sub i64 {}, {}", indent, aft_cnt, new_len, pop_idx).ok();
                let aft_bytes = format!("%pab2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, aft_bytes, aft_cnt).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, aft_dst, aft_src, aft_bytes).ok();
                // Store updated list back
                if let Expr::OwnedRef(field_name) = target.as_ref() {
                    if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                        let ap = format!("%pap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
                        // 2026-06-27: Record field as modified for per-field
                        // phi back-edge reload (same rationale as push).
                        backend.fun.pending_phi_backedge.insert(field_name.clone(), base.clone());
                    } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
                    }
                }
                return TypedRegister { name: popped.to_string(), ty: Type::Int };
            }
            Expr::ArrowDiscard { target, index } => {
                let list_val = backend.emit_expr(out, target, indent);
                let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
                // Unbox list header, read length
                let hp = format!("%dhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                let lp = format!("%dlp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let len = format!("%dln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                // Compute discard index
                let discard_idx = format!("%ddi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                if matches!(index.as_ref(), Expr::Term) {
                    writeln!(out, "{}{} = add i64 {}, -1", indent, discard_idx, len).ok();
                } else {
                    let iv = backend.emit_expr(out, index, indent);
                    let ib = backend.adapt_to_i64(out, indent, &iv);
                    writeln!(out, "{}{} = add i64 {}, 0", indent, discard_idx, ib).ok();
                }
                // Free old buffer: arena-active skips per-op free; standalone frees
                if backend.fun.arena_slots.is_none() {
                    let old_ptr = format!("%dop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
                }
                // Allocate new buffer: (len + 1) slots (2 header + len - 1 elements)
                let new_cnt = format!("%dnc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, new_cnt, len).ok();
                let alloc_bytes = format!("%dab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
                let new_buf = backend.emit_arena_alloc(out, indent, &alloc_bytes);
                // Set header
                let new_hp = format!("%dnh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
                let base = format!("%dba{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
                let ndv = format!("%dnd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, ndv, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ndv, new_hp).ok();
                let nlp = format!("%dnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
                let new_len = format!("%dnl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, -1", indent, new_len, len).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
                // Copy before discard_idx
                let dp = format!("%ddp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, hp).ok();
                let ndp = format!("%dndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
                let bef_bytes = format!("%dbb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, bef_bytes, discard_idx).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, ndp, dp, bef_bytes).ok();
                // Copy after discard_idx
                let after_off = format!("%dao{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, after_off, discard_idx).ok();
                let aft_src = format!("%das{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_src, dp, after_off).ok();
                let aft_dst = format!("%dad{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_dst, ndp, discard_idx).ok();
                let aft_cnt = format!("%dac{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = sub i64 {}, {}", indent, aft_cnt, new_len, discard_idx).ok();
                let aft_bytes = format!("%dab2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, aft_bytes, aft_cnt).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, aft_dst, aft_src, aft_bytes).ok();
                // Store updated list back
                if let Expr::OwnedRef(field_name) = target.as_ref() {
                    if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                        let ap = format!("%dap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
                    } else if let Some(slot) = backend.fun.param_slots.get(field_name).cloned() {
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
                    }
                }
                writeln!(out, "{}{} = add i64 0, {} ; discard", indent, v, base).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Int };
            }
            // ArrowTransfer moves ALL elements from source to destination.
            // Both old buffers are freed; a new combined buffer is allocated.
            // The source list becomes empty (2-slot header with data_ptr=null, len=0).
            // This is the most allocation-heavy arrow op — the arena plan (Phase 1)
            // benefits transfer the most.
            Expr::ArrowTransfer { dest, source, filter: _ } => {
                // Unfiltered: move all elements from source to dest
                let dest_val = backend.emit_expr(out, dest, indent);
                let src_val = backend.emit_expr(out, source, indent);
                let dest_boxed = backend.adapt_to_i64(out, indent, &dest_val);
                let src_boxed = backend.adapt_to_i64(out, indent, &src_val);
                let dhp = format!("%tdh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, dhp, dest_boxed).ok();
                let shp = format!("%tsh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, shp, src_boxed).ok();
                // Read lengths
                let dlp = format!("%tdl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, dlp, dhp).ok();
                let dlen = format!("%tdn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, dlen, dlp).ok();
                let slp = format!("%tsl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, shp).ok();
                let slen = format!("%tsn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, slen, slp).ok();
                // Total = dest_len + src_len
                let total = format!("%ttl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, {}", indent, total, dlen, slen).ok();
                // Free old buffers: arena skips per-op free; standalone frees
                if backend.fun.arena_slots.is_none() {
                    let dold = format!("%tdop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, dold, dest_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, dold).ok();
                    let sold = format!("%tsop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, sold, src_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, sold).ok();
                }
                // Allocate new dest buffer: (total + 2) * 8
                let alloc_slots = format!("%tas{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 2", indent, alloc_slots, total).ok();
                let alloc_bytes = format!("%tab{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, alloc_slots).ok();
                let new_buf = backend.emit_arena_alloc(out, indent, &alloc_bytes);
                // Set dest header
                let new_hp = format!("%tnh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
                let dbase = format!("%tdb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, dbase, new_buf).ok();
                let ndv = format!("%tnd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, ndv, dbase).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ndv, new_hp).ok();
                let tnlp = format!("%tnl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, tnlp, new_hp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, total, tnlp).ok();
                // Copy dest elements
                let ddp = format!("%tddp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ddp, dhp).ok();
                let ndp = format!("%tndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
                let dbytes = format!("%tdb2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, dbytes, dlen).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, ndp, ddp, dbytes).ok();
                // Copy source elements after dest
                let sdp = format!("%tsdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, sdp, shp).ok();
                let src_off = format!("%tso{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, src_off, ndp, dlen).ok();
                let sbytes = format!("%tsb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, sbytes, slen).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, src_off, sdp, sbytes).ok();
                // Store dest back
                if let Expr::OwnedRef(field_name) = dest.as_ref() {
                    if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                        let ap = format!("%tap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, dbase, ap, tn).ok();
                    }
                }
                // Store source (empty) back
                if let Expr::OwnedRef(field_name) = source.as_ref() {
                    if let Some(&idx) = backend.ctx.field_index_map.get(field_name) {
                        let ap = format!("%sap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&backend.ctx.field_types[idx]);
                        // Allocate new empty list for source
                        let ebuf = backend.emit_arena_alloc(out, indent, "16");
                        let ehp = format!("%seh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, ehp, ebuf).ok();
                        let ebase = format!("%seb2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, ebase, ebuf).ok();
                        let edv = format!("%sed{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = add i64 {}, 16", indent, edv, ebase).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, edv, ehp).ok();
                        let elp = format!("%sel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, elp, ehp).ok();
                        writeln!(out, "{}store i64 0, i64* {}, align 8, !tbaa !1", indent, elp).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, ebase, ap, tn).ok();
                    }
                }
                writeln!(out, "{}{} = add i64 0, {} ; transfer", indent, v, dbase).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Int };
            }
            Expr::Cast(inner, target_ty) => {
                let inner_val = backend.emit_expr(out, inner, indent);
                // 2026-06-28: Use txn_counter to prevent %t{N} collision
                let cv = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                backend.emit_cast_convert(out, indent, &cv, &inner_val.name, Some(inner_val.ty), target_ty);
                // Casts to boxed types (String/Data) produce i64, not native i8*.
                let ret_ty = if matches!(target_ty, Type::String | Type::Data) {
                    Type::Int
                } else {
                    target_ty.clone()
                };
                return TypedRegister { name: cv, ty: ret_ty };
            }
            // ── CellCall ──────────────────────────────────────────
            Expr::CellCall(callee, args) => {
                let callee_name = match callee.as_ref() {
                    Expr::Identifier(name) => name.clone(),
                    _ => { panic!("emit_expr: CellCall with non-identifier callee: {:?}", callee); return TypedRegister { name: v.to_string(), ty: Type::Int }; }
                };
                let cell = match backend.ctx.cell_defs.get(&callee_name) {
                    Some(c) => c.clone(),
                    None => { panic!("emit_expr: CellCall: cell '{}' not found in cell_defs", callee_name); return TypedRegister { name: v.to_string(), ty: Type::Int }; }
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
                                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, t, adapted).ok();
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
                            "i8" => Type::Bool,
                            "i32" => Type::Char,
                            "float" => Type::Float,
                            "i8*" => Type::String,
                            _ => Type::Int,
                        };
                        if ret_ty == Type::Int && ll_ty != "i64" {
                            // 2026-06-28: Use txn_counter to prevent %t{N} collision
                            let boxed = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = zext {} {} to i64", indent, boxed, ll_ty, v).ok();
                            return TypedRegister { name: boxed, ty: Type::Int };
                        }
                        if ret_ty == Type::String {
                            // 2026-06-28: Use txn_counter to prevent %t{N} collision
                            let boxed = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, boxed, v).ok();
                            return TypedRegister { name: boxed, ty: Type::Int };
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
                return TypedRegister { name: v.to_string(), ty: Type::Int };
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
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, gep).ok();
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
                writeln!(out, "{}    {} = load i64, i64* {}, align 8", indent, v_result, v_save).ok();
                return TypedRegister { name: v_result.clone(), ty: Type::Int };
            }
            _ => { unreachable!("emit_expr: unhandled Expr variant: {:?}", expr); }
        }
        // Default: treat as Int. Float operations are handled explicitly
        // by emit_binop/emit_fcmp which return Type::Float/Bool respectively.
        TypedRegister { name: v.to_string(), ty: Type::Int }
}
