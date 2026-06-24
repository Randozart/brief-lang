use crate::ast::{ArrowDir, BracketOp, Expr, Intrinsic, MatchArm, MatchPattern, OutputType, Pattern, PipeChain, PipeStep, ProjectionTarget, SliceCoordinate, Statement, Type};
use crate::backend::llvm::{float_to_llvm_hex, LlvmBackend, TypedRegister};
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

impl LlvmBackend {
    pub(crate) fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> TypedRegister {
        // 2026-06-13: equality_saturation::simplify() REMOVED from here.
        // It caused exponential blowup on deeply nested || chains (32+ terms = 13M+ calls).
        // Root cause: simplify() fixpoint loop × simplify_pass() recursive simplify()
        // on children produces O(6^n) calls. LLVM -O3 handles the same folds.
        // To restore: replace the clone below with the conditional simplify call,
        // but first fix the exponential blowup (add depth cap or move to separate pass).
        // See patches/2026-06-13-remove-simplify-from-emit-expr.patch for exact removed code.
        let expr = expr.clone();
        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        match &expr {
            Expr::Integer(n) => { writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok(); return TypedRegister { name: v, ty: Type::Int }; }
            Expr::Bool(b) => { if *b { writeln!(out, "{}{} = and i1 true, true", indent, v).ok(); } else { writeln!(out, "{}{} = xor i1 true, true", indent, v).ok(); } return TypedRegister { name: v, ty: Type::Bool }; }
            Expr::Float(f) => {
                let bits = float_to_llvm_hex(*f);
                writeln!(out, "{}{} = bitcast i32 {} to float", indent, v, bits).ok();
                self.reg_float_cache.insert(v.clone(), v.clone());
                return TypedRegister { name: v, ty: Type::Float };
            }
            Expr::String(s) | Expr::RegexLiteral(s) => {
                let si = self.string_constants.iter().position(|x| x == s).unwrap_or(0);
                let g = format!("@str.{}", si);
                let bp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast <{{ i64, i64, [{} x i8] }}>* {} to i8*", indent, bp, s.len() + 1, g).ok();
                // Tag static string pointers with bit 0 (=1) so concat can distinguish
                // them from heap-allocated strings and avoid freeing static data.
                let pi = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, pi, bp).ok();
                let ori = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = or i64 {}, 1", indent, ori, pi).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, v, ori).ok();
                return TypedRegister { name: v, ty: Type::String };
            }
            Expr::Char(c) => {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, *c as i32).ok();
                return TypedRegister { name: v, ty: Type::Char };
            }
            Expr::Term => { writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); return TypedRegister { name: v, ty: Type::Int }; }
            Expr::BinaryOp(bop) => return bop.emit_llvm(self, out, &ExprDispatch),
            Expr::UnaryOp(uop) => return uop.emit_llvm(self, out, &ExprDispatch),
            Expr::Literal(lit) => return lit.emit_llvm(self, out, &ExprDispatch),
            Expr::Identifier(name) => {
                // SSA body mode: prefer pre-extracted old-value register
                // for int fields so all body ops are independent.
                if let Some(old_reg) = self.ssa_old_int_regs.get(name) {
                    // If the old register is a non-i64 type, cast to i64 first
                    if let Some(&idx) = self.field_index_map.get(name) {
                        let ft = &self.field_types[idx];
                        if ft == "i8" {
                            let z = format!("%iz_{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i8 {} to i1", indent, z, old_reg).ok();
                            return TypedRegister { name: z, ty: Type::Bool };
                        }
                        if ft == "i32" {
                            let z = format!("%iz_{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, z, old_reg).ok();
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                            // i32 LLVM type means Char at the Brief level
                            // (the only Brief type mapped to i32).
                            return TypedRegister { name: v, ty: Type::Char };
                        }
                        if ft == "i8*" || ft == "ptr" {
                            // old_reg is i8* from extractvalue on state (state stores
                            // native i8* for String fields, not boxed i64). ptrtoint to box.
                            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, old_reg).ok();
                            return TypedRegister { name: v, ty: Type::Int };
                        }
                    }
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, old_reg).ok();
                    return TypedRegister { name: v, ty: Type::Int };
                }
                // SSA body mode: prefer pre-extracted old-value register
                // for float fields so all body ops are independent.
                if let Some(old_reg) = self.ssa_old_float_regs.get(name) {
                    self.reg_float_cache.insert(old_reg.clone(), old_reg.clone());
                    return TypedRegister { name: old_reg.clone(), ty: Type::Float };
                }
                if let Some(ref ssa_reg) = self.ssa_state_reg.clone() {
                if let Some(&addr) = self.mmio_fields.get(name) {
                    let p = format!("%gep_exit_{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                    writeln!(out, "{}{} = load volatile i64, i64* {}, align 1", indent, v, p).ok();
                } else if let Some(&idx) = self.field_index_map.get(name) {
                        let ll_ty = &self.field_types[idx];
                        let ev = format!("%ev{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, idx).ok();
                        let field_ty = match ll_ty.as_str() {
                            "i8" => {
                                let tr = format!("%tr_{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, ev).ok();
                                return TypedRegister { name: tr, ty: Type::Bool };
                            }
                            "float" => {
                                let fc = self.txn_counter; self.txn_counter += 1;
                                let float_reg = format!("%flt_{}_{}", name, fc);
                                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, float_reg, ssa_reg, idx).ok();
                                self.reg_float_cache.insert(float_reg.clone(), float_reg.clone());
                                return TypedRegister { name: float_reg, ty: Type::Float };
                            }
                            "i8*" => {
                                // ev is i8* from extractvalue — ptrtoint to box.
                                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ev).ok();
                                return TypedRegister { name: v, ty: Type::Int };
                            }
                            "i32" => {
                                // i32 LLVM type means Char at the Brief level.
                                // zext to i64 and preserve the Char type so that
                                // downstream casts use Char→String conversion.
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ev).ok();
                                return TypedRegister { name: v.clone(), ty: Type::Char };
                            }
                            _ => {
                                writeln!(out, "{}{} = add i64 0, {}", indent, v, ev).ok();
                                return TypedRegister { name: v, ty: Type::Int };
                            }
                        };
                    }
                }
                if let Some(reg) = self.let_bindings.get(name) {
                    if let Some(ty) = self.let_binding_types.get(name) {
                        if *ty == Type::Float {
                            return TypedRegister { name: reg.clone(), ty: Type::Float };
                        }
                        if *ty == Type::Char {
                            // All Char registers from emit_expr are already i64.
                            // Copy the register as-is; no zext needed.
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
                            return TypedRegister { name: v, ty: Type::Char };
                        }
                        if *ty == Type::Bool {
                            let z = format!("%iz_b{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = zext i1 {} to i64", indent, z, reg).ok();
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                            return TypedRegister { name: v, ty: Type::Int };
                        }
                    }
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
                    if let Some(ty) = self.let_binding_types.get(name) {
                        return TypedRegister { name: v, ty: ty.clone() };
                    }
                }
                if self.trigger_names.contains(name) {
                    if let Some(sampled) = self.sampled_triggers.get(name) {
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, sampled).ok();
                        return TypedRegister { name: v.clone(), ty: Type::Int };
                    } else if let Some(t) = self.triggers.get(name).cloned() {
                        // For built-in triggers (@stdin#, @timer#, @signal#), load from
                        // the state field (the event loop stored the value there).
                        if matches!(t.address, crate::ast::LinkRef::Stdin | crate::ast::LinkRef::Timer(_) | crate::ast::LinkRef::Signal(_)) {
                            if let Some(&idx) = self.field_index_map.get(name) {
                                let ll_ty = &self.field_types[idx];
                                let sge = format!("%sge_{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, sge, idx).ok();
                                let ev = format!("%ev_{}", self.txn_counter); self.txn_counter += 1;
                                match ll_ty.as_str() {
                                    "i8" => { writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, ev, sge).ok(); }
                                    "i32" => { writeln!(out, "{}{} = load i32, i32* {}, align 4", indent, ev, sge).ok(); }
                                    _ => { writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ev, sge).ok(); }
                                }
                                self.emit_trg_load_finish(out, indent, &v, ev, &t.ty);
                                return TypedRegister { name: v.clone(), ty: t.ty.clone() };
                            } else {
                                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                                return TypedRegister { name: v.clone(), ty: Type::Int };
                            }
                        } else {
                            self.emit_trg_load(out, indent, &v, &t.address, &t.ty);
                            return TypedRegister { name: v.clone(), ty: t.ty.clone() };
                        }
                    } else {
                        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        return TypedRegister { name: v.clone(), ty: Type::Int };
                    }
                } else if let Some((ty, expr)) = self.constants.get(name) {
                    // Inline literal integer/bool constants as immediates
                    // instead of loading from global RAM.
                    match (ty, expr) {
                        (Type::Int | Type::UInt, Expr::Integer(n)) => {
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
                            return TypedRegister { name: v, ty: Type::Int };
                        }
                        (Type::Bool, Expr::Bool(b)) => {
                            if *b {
                                writeln!(out, "{}{} = and i1 true, true", indent, v).ok();
                            } else {
                                writeln!(out, "{}{} = xor i1 true, true", indent, v).ok();
                            }
                            return TypedRegister { name: v, ty: Type::Bool };
                        }
                        _ => {
                            let ll_ty = match ty {
                                Type::Float => "float",
                                Type::Int | Type::UInt => "i64",
                                Type::Bool => "i8",
                                _ => "i64",
                            };
                            let ld = format!("%il{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load {}, {}* @{}, align {}", indent, ld, ll_ty, ll_ty, name, self.align_of(ll_ty)).ok();
                            let ret_ty = match ty {
                                Type::Float => {
                                    self.reg_float_cache.insert(ld.clone(), ld.clone());
                                    return TypedRegister { name: ld.clone(), ty: Type::Float };
                                }
                                Type::Bool => {
                                    let z = format!("%iz_{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i8 {} to i1", indent, z, ld).ok();
                                    return TypedRegister { name: z, ty: Type::Bool };
                                }
                                _ => {
                                    writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
                                    ty.clone()
                                }
                            };
                            return TypedRegister { name: v, ty: ret_ty };
                        }
                    }
                } else if let Some(&addr) = self.mmio_fields.get(name) {
                    let p = format!("%mio{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                    writeln!(out, "{}{} = load volatile i64, i64* {}, align 1", indent, v, p).ok();
                } else if let Some(&idx) = self.field_index_map.get(name) {
                    let ty = &self.field_types[idx];
                    let p = format!("%fdp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, p, idx).ok();
                    let rng = self.field_to_meta_idx.get(name).map(|m| format!(", !range !{}", m)).unwrap_or_default();
                    match ty {
                        s if s == "i8" => {
                            writeln!(out, "{}{} = load i8, i8* {}, align {}", indent, v, p, self.align_of("i8")).ok();
                            let tr = format!("%tr_{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, v).ok();
                            return TypedRegister { name: tr, ty: Type::Bool };
                        }
                        s if s == "float" => {
                            writeln!(out, "{}{} = load float, float* {}, align 4", indent, v, p).ok();
                            self.reg_float_cache.insert(v.clone(), v.clone());
                            return TypedRegister { name: v.clone(), ty: Type::Float };
                        }
                        s if s == "i8*" => {
                            let ld = format!("%ild{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i8*, i8** {}, align 8", indent, ld, p).ok();
                            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ld).ok();
                            return TypedRegister { name: v.clone(), ty: Type::Int };
                        }
                        s if s == "i32" => {
                            // i32 state fields are Char at the Brief level.
                            // zext to i64 and preserve the Char type so that
                            // downstream casts use Char→String conversion.
                            let ld = format!("%il{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i32, i32* {}, align 4", indent, ld, p).ok();
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ld).ok();
                            return TypedRegister { name: v.clone(), ty: Type::Char };
                        }
                        _ => {
                            writeln!(out, "{}{} = load {}, {}* {}, align {}{}", indent, v, ty, ty, p, self.align_of(ty), rng).ok();
                        }
                    }
                }
            }
            Expr::OwnedRef(name) => {
                // Redirect to Identifier — same semantics for LLVM
                return self.emit_expr(out, &Expr::Identifier(name.clone()), indent);
            }
            Expr::PriorState(name) => {
                // Load the value from state BEFORE this tick's modifications.
                // The SSA state register holds the committed (pre-tick) value.
                if let Some(&idx) = self.field_index_map.get(name) {
                    let ll_ty = &self.field_types[idx];
                    let ev = format!("%pev{}", self.txn_counter); self.txn_counter += 1;
                    if let Some(ref ssa_reg) = self.ssa_state_reg.clone() {
                        writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, idx).ok();
                        let field_ty = match ll_ty.as_str() {
                            "i8" => {
                                let tr = format!("%ptr_{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, ev).ok();
                                return TypedRegister { name: tr, ty: Type::Bool };
                            }
                            "i32" => {
                                let z = format!("%piz_{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, z, ev).ok();
                                writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                                return TypedRegister { name: v, ty: Type::Char };
                            }
                            "float" => {
                                return TypedRegister { name: ev, ty: Type::Float };
                            }
                            _ => {
                                writeln!(out, "{}{} = add i64 0, {}", indent, v, ev).ok();
                                return TypedRegister { name: v, ty: Type::Int };
                            }
                        };
                    }
                }
                writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                writeln!(out, "{}{} = add i64 0, 0 ; @{} (not found)", indent, v, name).ok();
            }
            // Binary ops
            Expr::Add(l, r) => {
                // 2026-06-17: String + String → inline concat. Both typed as
                // Type::Int (boxed), so check the AST recursively.
                if self.is_string_chain(l) || self.is_string_chain(r) {
                    let a = self.emit_expr(out, l, indent);
                    let b = self.emit_expr(out, r, indent);
                    return self.emit_inline_concat(out, indent, &a, &b);
                }
                return self.emit_binop(out, indent, l, r, "add", "fadd");
            }
            Expr::Sub(l, r) => { return self.emit_binop(out, indent, l, r, "sub", "fsub"); }
            Expr::Mul(l, r) => { return self.emit_binop(out, indent, l, r, "mul", "fmul"); }
            Expr::Div(l, r) => { return self.emit_binop(out, indent, l, r, "sdiv", "fdiv"); }
            Expr::Mod(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = srem i64 {}, {}", indent, v, a, b).ok(); }
            // Comparisons
            Expr::Eq(l, r) => { return self.emit_fcmp(out, indent, l, r, "oeq"); }
            Expr::Ne(l, r) => { return self.emit_fcmp(out, indent, l, r, "one"); }
            Expr::Lt(l, r) => { return self.emit_fcmp(out, indent, l, r, "olt"); }
            Expr::Le(l, r) => { return self.emit_fcmp(out, indent, l, r, "ole"); }
            Expr::Gt(l, r) => { return self.emit_fcmp(out, indent, l, r, "ogt"); }
            Expr::Ge(l, r) => { return self.emit_fcmp(out, indent, l, r, "oge"); }
            // Logical
            Expr::And(l, r) => {
                let a = self.emit_expr(out, l, indent);
                let b = self.emit_expr(out, r, indent);
                let an = self.as_bool_reg(out, indent, &a);
                let bn = self.as_bool_reg(out, indent, &b);
                writeln!(out, "{}{} = and i1 {}, {}", indent, v, an, bn).ok();
                return TypedRegister { name: v, ty: Type::Bool };
            }
            Expr::Or(l, r) => {
                let a = self.emit_expr(out, l, indent);
                let b = self.emit_expr(out, r, indent);
                let an = self.as_bool_reg(out, indent, &a);
                let bn = self.as_bool_reg(out, indent, &b);
                writeln!(out, "{}{} = or i1 {}, {}", indent, v, an, bn).ok();
                return TypedRegister { name: v, ty: Type::Bool };
            }
            Expr::Not(e) => {
                let inner = self.emit_expr(out, e, indent);
                let name = self.as_bool_reg(out, indent, &inner);
                writeln!(out, "{}{} = xor i1 {}, true", indent, v, name).ok();
                return TypedRegister { name: v, ty: Type::Bool };
            }
            Expr::Neg(e) => {
                let inner = self.emit_expr(out, e, indent);
                if inner.ty == Type::Float {
                    let fl = self.ensure_float_reg(out, indent, &inner);
                    writeln!(out, "{}{} = fsub fast float -0.0, {}", indent, v, fl).ok();
                    self.reg_float_cache.insert(v.clone(), v.clone());
                    return TypedRegister { name: v, ty: Type::Float };
                } else {
                    writeln!(out, "{}{} = sub i64 0, {}", indent, v, inner.name).ok();
                    return TypedRegister { name: v, ty: Type::Int };
                }
            }
            // Bitwise
            Expr::BitAnd(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = and i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitOr(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = or i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitXor(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = xor i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitNot(e) => { let inner = self.emit_expr(out, e, indent); writeln!(out, "{}{} = xor i64 {}, -1", indent, v, inner).ok(); }
            Expr::Shl(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = shl i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Shr(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = lshr i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Concat(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); return self.emit_inline_concat(out, indent, &a, &b); }
            // Call
            Expr::Call(name, args) => {
                // 2026-06-17: Inline negated (stdlib projection, not defined as a function)
                if name == "negated" && args.len() >= 1 {
                    let val = self.emit_expr(out, &args[0], indent);
                    writeln!(out, "{}{} = sub i64 0, {}", indent, v, val.name).ok();
                    return TypedRegister { name: v, ty: Type::Int };
                }
                // Clone foreign info upfront to avoid borrow conflict with emit_expr
                let frgn_sig: Option<(Vec<(String, Type)>, crate::ast::ResultType, bool, Option<crate::ast::Expr>, Vec<(String, Type)>)> =
                    self.frgn_map.get(name).map(|s| (s.inputs.clone(), s.result_type.clone(), s.is_pipe, s.fallback.clone(), s.success_output.clone()));
                if let Some((inputs, ret_type, is_pipe, fallback, success_output)) = frgn_sig {
                    let mut marshaled: Vec<String> = Vec::new();
                    for (i, (_, arg_ty)) in inputs.iter().enumerate() {
                        if i < args.len() {
                            let raw = self.emit_expr(out, &args[i], indent);
                            // Phase 3: Decay chimera arguments before FFI call
                            let raw = self.emit_decay(out, &raw, Some(arg_ty), indent);
                            match arg_ty {
                                Type::Int | Type::UInt => marshaled.push(format!("i64 {}", raw)),
                                Type::Bool => {
                                    let boxed = self.adapt_to_i64(out, indent, &raw);
                                    let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, boxed).ok();
                                    marshaled.push(format!("i32 {}", z));
                                }
                                Type::Char => {
                                    let boxed = self.adapt_to_i64(out, indent, &raw);
                                    let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, boxed).ok();
                                    marshaled.push(format!("i32 {}", z));
                                }
                                Type::Float => {
                                    let fl = self.ensure_float_reg(out, indent, &raw);
                                    marshaled.push(format!("float {}", fl));
                                }
                                Type::String | Type::Data => {
                                    let boxed = self.adapt_to_i64(out, indent, &raw);
                                    let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1;
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
                    let call_result = format!("%t{}", self.txn_counter); self.txn_counter += 1;
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
                        let fallback_reg = fallback.as_ref().map(|e| self.emit_expr(out, e, indent));

                        match (&success_ty, is_float_ret) {
                            (Type::String | Type::Data, _) => {
                                // Null pointer check for i8* returns
                                let is_null = format!("%pipe_null{}", self.txn_counter); self.txn_counter += 1;
                                // call_result is i64 (boxed ptr). Convert to i8* for null check.
                                let ptr = format!("%pipe_ptr{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, ptr, call_result).ok();
                                writeln!(out, "{}{} = icmp eq i8* {}, null", indent, is_null, ptr).ok();
                                let select_reg = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                                let fbr = fallback_reg.as_ref().map(|r| r.name.as_str()).unwrap_or("null");
                                writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}",
                                    indent, select_reg, is_null, fbr, call_result).ok();
                                return TypedRegister { name: select_reg, ty: Type::Int };
                            }
                            (Type::Float, _) => {
                                // NaN check for float returns
                                let is_nan = format!("%pipe_nan{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = fcmp uno float {}, {}", indent, is_nan, call_result, call_result).ok();
                                let select_reg = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                                let fbr = fallback_reg.as_ref().map(|r| r.name.as_str()).unwrap_or("0.0");
                                writeln!(out, "{}{} = select i1 {}, float {}, float {}",
                                    indent, select_reg, is_nan, fbr, call_result).ok();
                                let bi = format!("%fbi{}", self.txn_counter); self.txn_counter += 1;
                                let ze = format!("%fze{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, select_reg).ok();
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                                self.reg_float_cache.insert(ze.clone(), select_reg.clone());
                                return TypedRegister { name: ze, ty: Type::Float };
                            }
                            _ => {
                                // Int/UInt/Bool/Char: always valid, just pass through
                                if is_float_ret {
                                    let bi = format!("%fbi{}", self.txn_counter); self.txn_counter += 1;
                                    let ze = format!("%fze{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, call_result).ok();
                                    writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                                    self.reg_float_cache.insert(ze.clone(), call_result.clone());
                                    return TypedRegister { name: ze, ty: Type::Float };
                                }
                                return TypedRegister { name: call_result, ty: Type::Int };
                            }
                        }
                    }

                    if is_float_ret {
                        let bi = format!("%fbi{}", self.txn_counter); self.txn_counter += 1;
                        let ze = format!("%fze{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, call_result).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                        self.reg_float_cache.insert(ze.clone(), call_result.clone());
                        return TypedRegister { name: ze, ty: Type::Float };
                    }
                    return TypedRegister { name: call_result, ty: Type::Int };
                } else {
                    // Internal call — marshal i64 back to real types per definition
                    let def_tys: Option<Vec<Type>> = self.defn_params.get(name).cloned();
                    let def_rets: Option<Vec<Type>> = self.defn_return_types.get(name).cloned();
                    let mut a_strs = Vec::new();
                    for (ai, arg) in args.iter().enumerate() {
                        let raw = self.emit_expr(out, arg, indent);
                        if let Some(ref tys) = def_tys {
                            if ai < tys.len() {
                                match &tys[ai] {
                                    Type::Bool => {
                                        let boxed = self.adapt_to_i64(out, indent, &raw);
                                        let tr = format!("%ctr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, boxed).ok();
                                        a_strs.push(format!("i8 {}", tr));
                                    }
                                    Type::String | Type::Data => {
                                        let boxed = self.adapt_to_i64(out, indent, &raw);
                                        let p = format!("%cip{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, boxed).ok();
                                        a_strs.push(format!("i8* {}", p));
                                    }
                                    Type::Float => {
                                        let fl = self.ensure_float_reg(out, indent, &raw);
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
                                let z = format!("%cz{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = zext i1 {} to i64", indent, z, raw.name).ok();
                                z
                            } else if raw.ty == Type::Char {
                                // Char registers are already i64 from emit_expr
                                raw.name.clone()
                            } else if raw.ty == Type::Float {
                                let bi = format!("%cfb{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, raw.name).ok();
                                let ze = format!("%cfz{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                                ze
                            } else {
                                raw.name.clone()
                            };
                            a_strs.push(format!("i64 {}", stored));
                        }
                    }
                    if name.starts_with(|c: char| c.is_uppercase()) && !self.program_txns.contains(name) {
                        let disc_val = self.variant_disc.get(name)
                            .map(|(_, d, _)| *d)
                            .unwrap_or(0u64);
                        let n_slots = a_strs.len() + 1;
                        let sz = format!("%csz{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, 8", indent, sz, n_slots as i64).ok();
                        // Why malloc/arena for enum variants: tagged union requires heap
                        // allocation because different variants have different sizes.
                        // Arena handles this with bump alloc when in a loop context.
                        let pm = self.emit_arena_alloc(out, indent, &sz);
                        let p = format!("%cop{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, p, pm).ok();
                        let disc_gep = format!("%cdg{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, disc_gep, p).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, disc_val, disc_gep).ok();
                        for (ai, arg_reg) in a_strs.iter().enumerate() {
                            let pay_gep = format!("%cpg{}", self.txn_counter); self.txn_counter += 1;
                            let parts: Vec<&str> = arg_reg.splitn(2, ' ').collect();
                            let rn = if parts.len() == 2 { parts[1] } else { arg_reg };
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, pay_gep, p, ai + 1).ok();
                            // 2026-06-17: Box float to i64 for enum storage
                            if parts.len() == 2 && (parts[0] == "float" || parts[0] == "float,") {
                                let bi = format!("%fbe{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, rn).ok();
                                let ze = format!("%fze{}", self.txn_counter); self.txn_counter += 1;
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
                        let fn_name = if name == "main" && self.defn_params.contains_key("main") {
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
                            return TypedRegister { name: v, ty: Type::Float };
                        }
                        // Internal calls return i64 (boxed), so mark as Type::Int.
                        // Previously returned Type::String/Type::Bool for string/bool ret,
                        // but that confused downstream native-type handling.
                        return TypedRegister { name: v, ty: Type::Int };
                }
            }
        }
            // ── IntrinsicCall ────────────────────────────────────
            Expr::IntrinsicCall { intrinsic, args } => {
                let emit_intrinsic_float_unary = |backend: &mut LlvmBackend, out: &mut String, indent: &str, v: &str, llvm_name: &str, arg: &Expr| -> TypedRegister {
                    let raw = backend.emit_expr(out, arg, indent);
                    let fl = backend.ensure_float_reg(out, indent, &raw);
                    writeln!(out, "{}{} = call float @llvm.{}.f32(float {})", indent, v, llvm_name, fl).ok();
                    TypedRegister { name: v.to_string(), ty: Type::Float }
                };
                match intrinsic {
                    Intrinsic::Sqrt => { return emit_intrinsic_float_unary(self, out, indent, &v, "sqrt", &args[0]); }
                    Intrinsic::Sin => { return emit_intrinsic_float_unary(self, out, indent, &v, "sin", &args[0]); }
                    Intrinsic::Cos => { return emit_intrinsic_float_unary(self, out, indent, &v, "cos", &args[0]); }
                    Intrinsic::Pow => {
                        let a = self.emit_expr(out, &args[0], indent);
                        let b = self.emit_expr(out, &args[1], indent);
                        writeln!(out, "{}{} = call double @pow(double {}, double {})", indent, v, a.name, b.name).ok();
                        return TypedRegister { name: v.to_string(), ty: Type::Float };
                    }
                    Intrinsic::Fabs => { return emit_intrinsic_float_unary(self, out, indent, &v, "fabs", &args[0]); }
                    Intrinsic::Ceil => { return emit_intrinsic_float_unary(self, out, indent, &v, "ceil", &args[0]); }
                    Intrinsic::Floor => { return emit_intrinsic_float_unary(self, out, indent, &v, "floor", &args[0]); }
                    Intrinsic::FloatToStr => {
                        if !args.is_empty() {
                            let a_raw = self.emit_expr(out, &args[0], indent);
                            let a_f = self.ensure_float_reg(out, indent, &a_raw);
                            writeln!(out, "{}{} = call i64 @__float_to_str(float {})", indent, v, a_f).ok();
                        }
                        return TypedRegister { name: v.to_string(), ty: Type::String };
                    }
                    Intrinsic::ToStr => {
                        if !args.is_empty() {
                            let a_raw = self.emit_expr(out, &args[0], indent);
                            writeln!(out, "{}{} = call i64 @__to_str(i64 {})", indent, v, a_raw.name).ok();
                        }
                        return TypedRegister { name: v.to_string(), ty: Type::String };
                    }
                    Intrinsic::Ctpop => {
                        let raw = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}{} = call i64 @llvm.ctpop.i64(i64 {})", indent, v, raw).ok();
                    }
                    Intrinsic::Ctlz => {
                        let raw = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}{} = call i64 @llvm.ctlz.i64(i64 {}, i1 false)", indent, v, raw).ok();
                    }
                    Intrinsic::Cttz => {
                        let raw = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}{} = call i64 @llvm.cttz.i64(i64 {}, i1 false)", indent, v, raw).ok();
                    }
                    Intrinsic::Abs => {
                        let raw = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}{} = call i64 @llvm.abs.i64(i64 {}, i1 false)", indent, v, raw).ok();
                    }
                    Intrinsic::Bitreverse => {
                        let raw = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}{} = call i64 @llvm.bitreverse.i64(i64 {})", indent, v, raw).ok();
                    }
                    Intrinsic::ByteCount => {
                        writeln!(out, "{}{} = add i64 0, 8 ; bytes", indent, v).ok();
                    }
                    Intrinsic::StrBytes => {
                        if let Some(first) = args.first() {
                            let n = self.emit_expr(out, first, indent);
                            let boxed = self.adapt_to_i64(out, indent, &n);
                            writeln!(out, "{}{} = call i64 @__str_bytes__(i64 {})", indent, v, boxed).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::Size => {
                        if let Some(first) = args.first() {
                            let list_val = self.emit_expr(out, first, indent);
                            let list_boxed = self.adapt_to_i64(out, indent, &list_val);
                            let hp = format!("%ishp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                            let lp = format!("%islp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, lp).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::Pop => {
                        if let Some(first) = args.first() {
                            let list_val = self.emit_expr(out, first, indent);
                            let list_boxed = self.adapt_to_i64(out, indent, &list_val);
                            let hp = format!("%ipphp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                            let lp = format!("%ipplp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                            let len = format!("%ippln{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                            let dpp = format!("%ippdp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dpp, hp).ok();
                            let pi = format!("%ippi{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = add i64 {}, -1", indent, pi, len).ok();
                            let ep = format!("%ippep{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, dpp, pi).ok();
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, ep).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::Contains => {
                        if args.len() >= 2 {
                            let list_val = self.emit_expr(out, &args[0], indent);
                            let elem_val = self.emit_expr(out, &args[1], indent);
                            let list_boxed = self.adapt_to_i64(out, indent, &list_val);
                            let elem_boxed = self.adapt_to_i64(out, indent, &elem_val);
                            let cmp = format!("%isc{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, list_boxed, elem_boxed).ok();
                            writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::Keys | Intrinsic::Values => {
                        if let Some(first) = args.first() {
                            let list_val = self.emit_expr(out, first, indent);
                            let list_boxed = self.adapt_to_i64(out, indent, &list_val);
                            // Return the list as-is (Keys/Values of a List is the list itself)
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, list_boxed).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    // System I/O intrinsics (stubs — passthrough to frgn calls)
                    Intrinsic::Println => {
                        // Print a Brief String followed by newline.
                        // Brief String value is i64 (ptrtoint of struct ptr).
                        // Load the first field (ptr_to_data) to get the data pointer.
                        if !args.is_empty() {
                            let msg = self.emit_expr(out, &args[0], indent);
                            let sptr = format!("%ppls{}", self.txn_counter); self.txn_counter += 1;
                            let sp = format!("%pplp{}", self.txn_counter); self.txn_counter += 1;
                            let data_ptr = format!("%ppld{}", self.txn_counter); self.txn_counter += 1;
                            let str_ptr = format!("%pplp{}", self.txn_counter); self.txn_counter += 1;
                            let so = format!("%pplo{}", self.txn_counter); self.txn_counter += 1;
                            let so2 = format!("%pplo{}", self.txn_counter); self.txn_counter += 1;
                            let fmt = format!("%pplf{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sptr, msg).ok();
                            writeln!(out, "{}{} = bitcast ptr {} to i64*", indent, sp, sptr).ok();
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, data_ptr, sp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, str_ptr, data_ptr).ok();
                            writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                            writeln!(out, "{}{} = getelementptr [4 x i8], [4 x i8]* @FMT_STR, i64 0, i64 0", indent, fmt).ok();
                            let fr = format!("%ppfr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, ptr {})",
                                indent, fr, so, fmt, str_ptr).ok();
                            writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so2).ok();
                            writeln!(out, "{}{} = call i32 @fflush(ptr {})", indent, v, so2).ok();
                        } else {
                            writeln!(out, "{}{} = add i64 0, 1 ; println no arg", indent, v).ok();
                        }
                    }
                    Intrinsic::Print => {
                        // Print a Brief String WITHOUT newline.
                        // Load hdr[0] (data pointer) and call fprintf.
                        if !args.is_empty() {
                            let msg = self.emit_expr(out, &args[0], indent);
                            let sptr = format!("%ppls{}", self.txn_counter); self.txn_counter += 1;
                            let sp = format!("%pplp{}", self.txn_counter); self.txn_counter += 1;
                            let data_ptr = format!("%ppld{}", self.txn_counter); self.txn_counter += 1;
                            let str_ptr = format!("%pplp{}", self.txn_counter); self.txn_counter += 1;
                            let so = format!("%pplo{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sptr, msg).ok();
                            writeln!(out, "{}{} = bitcast ptr {} to i64*", indent, sp, sptr).ok();
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, data_ptr, sp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, str_ptr, data_ptr).ok();
                            writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                            let fr = format!("%ppfr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = call i32 @fputs(ptr {}, ptr {})",
                                indent, v, str_ptr, so).ok();
                        } else {
                            writeln!(out, "{}{} = add i64 0, 1 ; print no arg", indent, v).ok();
                        }
                    }
                    Intrinsic::Readln => {
                        writeln!(out, "{}{} = call i64 @__readln__()", indent, v).ok();
                    }
                    Intrinsic::Exit => {
                        // Call libc exit to terminate the program
                        if args.len() >= 1 {
                            let code = self.emit_expr(out, &args[0], indent);
                            let ct = format!("%pext{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ct, code).ok();
                            writeln!(out, "{}call void @exit(i32 {})", indent, ct).ok();
                        } else {
                            writeln!(out, "{}call void @exit(i32 0)", indent).ok();
                        }
                        writeln!(out, "{}{} = add i64 0, 1", indent, v).ok();
                    }
                    Intrinsic::Time => {
                        writeln!(out, "{}{} = call i64 @time(i64* null)", indent, v).ok();
                    }
                    Intrinsic::ReadFile => {
                        // 2026-06-18: brief_read_file now returns i64 (Brief string ptr)
                        // or 0 on failure. The C function handles the fopen/read/boxing.
                        if args.len() >= 1 {
                            let path_val = self.emit_expr(out, &args[0], indent);
                            let boxed = self.adapt_to_i64(out, indent, &path_val);
                            let raw = format!("%frraw{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = call i64 @__read_file__(i64 {})", indent, raw, boxed).ok();
                            let is_zero = format!("%frisz{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = icmp eq i64 {}, 0", indent, is_zero, raw).ok();
                            let el = format!("rf_err{}", self.txn_counter); self.txn_counter += 1;
                            let ol = format!("rf_ok{}", self.txn_counter); self.txn_counter += 1;
                            let dl = format!("rf_done{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_zero, el, ol).ok();

                            // Err("file not found") — packed Result: disc=1 low 8 bits,
                            // payload=ptrtoint(@STR_READFILE_ERR) << 8
                            writeln!(out, "{}{}:", indent, el).ok();
                            let e_gp = format!("%rgep{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr [15 x i8], [15 x i8]* @STR_READFILE_ERR, i64 0, i64 0", indent, e_gp).ok();
                            let e_pa = format!("%rfpa{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, e_pa, e_gp).ok();
                            let e_sh = format!("%rfsh{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = shl i64 {}, 8", indent, e_sh, e_pa).ok();
                            let e_re = format!("%rfer{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = or i64 {}, 1", indent, e_re, e_sh).ok();
                            writeln!(out, "{}br label %{}", indent, dl).ok();

                            // Ok(contents) — packed Result: disc=0 low 8 bits,
                            // payload = raw (already a Brief string pointer) << 8
                            writeln!(out, "{}{}:", indent, ol).ok();
                            let o_re = format!("%rfor{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = shl i64 {}, 8", indent, o_re, raw).ok();
                            writeln!(out, "{}br label %{}", indent, dl).ok();

                            writeln!(out, "{}{}:", indent, dl).ok();
                            writeln!(out, "{}{} = phi i64 [ {}, %{} ], [ {}, %{} ]", indent, v, e_re, el, o_re, ol).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::WriteFile => {
                        // WriteFile(path: String, data: String) -> Bool
                        // Brief strings are passed as boxed i64 (ptrtoint of header).
                        let path_val = self.emit_expr(out, &args[0], indent);
                        let data_val = self.emit_expr(out, &args[1], indent);
                        let path_boxed = self.adapt_to_i64(out, indent, &path_val);
                        let data_boxed = self.adapt_to_i64(out, indent, &data_val);
                        let wf_ret = format!("%wfr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i64 @__write_file__(i64 {}, i64 {})", indent, wf_ret, path_boxed, data_boxed).ok();
                        writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, v, wf_ret).ok();
                        return TypedRegister { name: v, ty: Type::Bool };
                    }
                    Intrinsic::Sleep => {
                        // Sleep takes milliseconds, converts to seconds + nanoseconds for nanosleep
                        let ms = self.emit_expr(out, &args[0], indent);
                        let micro = format!("%slmc{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, 1000", indent, micro, ms.name).ok();
                        let sec = format!("%slsc{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = udiv i64 {}, 1000000", indent, sec, micro).ok();
                        let usec = format!("%sluc{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = urem i64 {}, 1000000", indent, usec, micro).ok();
                        let nsec = format!("%slnec{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, 1000", indent, nsec, usec).ok();
                        // Allocate and fill timespec
                        let ts = format!("%slts{}", self.txn_counter); self.txn_counter += 1;
                        let tsp = format!("%sltsp{}", self.txn_counter); self.txn_counter += 1;
                        let tsnp = format!("%sltsn{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = alloca {{ i64, i64 }}, align 8", indent, ts).ok();
                        writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 0", indent, tsp, ts).ok();
                        writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 1", indent, tsnp, ts).ok();
                        writeln!(out, "{}store i64 {}, ptr {}", indent, sec, tsp).ok();
                        writeln!(out, "{}store i64 {}, ptr {}", indent, nsec, tsnp).ok();
                        // Call nanosleep (ignore remainder)
                        let rv = format!("%slrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @nanosleep(ptr {}, ptr null)", indent, rv, ts).ok();
                        // Return true (Bool)
                        writeln!(out, "{}{} = add i64 0, 1 ; sleep done", indent, v).ok();
                    }
                    // ===== Phase A: Terminal (intrinsics.md D4) =====
                    Intrinsic::TtyRawMode => {
                        let arg = self.emit_expr(out, &args[0], indent);
                        let arg64 = self.adapt_to_i64(out, indent, &arg);
                        let raw = format!("%trm{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i64 @__tty_raw_mode__(i64 {})", indent, raw, arg64).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i1", indent, v, raw).ok();
                        return TypedRegister { name: v, ty: Type::Bool };
                    }
                    Intrinsic::TtySize => {
                        let ws = format!("%ttywsp{}", self.txn_counter); self.txn_counter += 1;
                        let ws_bc = format!("%ttywsbc{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%ttyrv{}", self.txn_counter); self.txn_counter += 1;
                        let is_err = format!("%ttyie{}", self.txn_counter); self.txn_counter += 1;
                        let z_l = format!("tty_sz_z{}", self.txn_counter); self.txn_counter += 1;
                        let o_l = format!("tty_sz_o{}", self.txn_counter); self.txn_counter += 1;
                        let e_l = format!("tty_sz_e{}", self.txn_counter); self.txn_counter += 1;
                        let row_p = format!("%ttyrp{}", self.txn_counter); self.txn_counter += 1;
                        let col_p = format!("%ttycp{}", self.txn_counter); self.txn_counter += 1;
                        let row = format!("%ttyrw{}", self.txn_counter); self.txn_counter += 1;
                        let col = format!("%ttycw{}", self.txn_counter); self.txn_counter += 1;
                        let row64 = format!("%ttyr64{}", self.txn_counter); self.txn_counter += 1;
                        let col64 = format!("%ttyc64{}", self.txn_counter); self.txn_counter += 1;
                        let shifted = format!("%ttysh{}", self.txn_counter); self.txn_counter += 1;
                        let packed = format!("%ttypk{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = alloca {{ i16, i16, i16, i16 }}, align 2", indent, ws).ok();
                        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, ws_bc, ws).ok();
                        writeln!(out, "{}{} = call i32 @ioctl(i32 1, i64 21523, ptr {})", indent, rv, ws_bc).ok();
                        writeln!(out, "{}{} = icmp eq i32 {}, -1", indent, is_err, rv).ok();
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_err, z_l, o_l).ok();
                        writeln!(out, "{}{}:", indent, z_l).ok();
                        writeln!(out, "{}  br label %{}", indent, e_l).ok();
                        writeln!(out, "{}{}:", indent, o_l).ok();
                        writeln!(out, "{}{} = getelementptr {{ i16, i16, i16, i16 }}, ptr {}, i32 0, i32 0", indent, row_p, ws).ok();
                        writeln!(out, "{}{} = getelementptr {{ i16, i16, i16, i16 }}, ptr {}, i32 0, i32 1", indent, col_p, ws).ok();
                        writeln!(out, "{}{} = load i16, ptr {}", indent, row, row_p).ok();
                        writeln!(out, "{}{} = load i16, ptr {}", indent, col, col_p).ok();
                        writeln!(out, "{}{} = zext i16 {} to i64", indent, row64, row).ok();
                        writeln!(out, "{}{} = zext i16 {} to i64", indent, col64, col).ok();
                        // 2026-06-17: Pack as col * 10000 + row (C convention used by
                        // officina: term_width = encoded/10000, term_height = encoded%10000)
                        let mult = format!("%ttym{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, 10000", indent, mult, col64).ok();
                        let packed = format!("%ttypk{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = add i64 {}, {}", indent, packed, mult, row64).ok();
                        writeln!(out, "{}  br label %{}", indent, e_l).ok();
                        writeln!(out, "{}{}:", indent, e_l).ok();
                        writeln!(out, "{}{} = phi i64 [ 800024, %{} ], [ {}, %{} ]", indent, v, z_l, packed, o_l).ok();
                    }
                    Intrinsic::TtyReadKey => {
                        let cbuf = format!("%trkcb{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%trkrv{}", self.txn_counter); self.txn_counter += 1;
                        let ok = format!("%trkok{}", self.txn_counter); self.txn_counter += 1;
                        let err_l = format!("trk_err{}", self.txn_counter); self.txn_counter += 1;
                        let ok_l = format!("trk_ok{}", self.txn_counter); self.txn_counter += 1;
                        let end_l = format!("trk_end{}", self.txn_counter); self.txn_counter += 1;
                        let c = format!("%trkc{}", self.txn_counter); self.txn_counter += 1;
                        let tmp = format!("%trkt{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = alloca i8, align 1", indent, cbuf).ok();
                        writeln!(out, "{}{} = call i64 @read(i32 0, ptr {}, i64 1)", indent, rv, cbuf).ok();
                        writeln!(out, "{}{} = icmp ne i64 {}, 1", indent, ok, rv).ok();
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, ok, err_l, ok_l).ok();
                        writeln!(out, "{}{}:", indent, err_l).ok();
                        writeln!(out, "{}  br label %{}", indent, end_l).ok();
                        writeln!(out, "{}{}:", indent, ok_l).ok();
                        writeln!(out, "{}{} = load i8, ptr {}", indent, c, cbuf).ok();
                        writeln!(out, "{}{} = zext i8 {} to i32", indent, tmp, c).ok();
                        writeln!(out, "{}  br label %{}", indent, end_l).ok();
                        writeln!(out, "{}{}:", indent, end_l).ok();
                        let phi_r = format!("%trkp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = phi i32 [ -1, %{} ], [ {}, %{} ]", indent, phi_r, err_l, tmp, ok_l).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, phi_r).ok();
                        return TypedRegister { name: v, ty: Type::Char };
                    }
                    Intrinsic::IoCtl => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let req = self.emit_expr(out, &args[1], indent);
                        let arg = self.emit_expr(out, &args[2], indent);
                        let fdt = format!("%iofdt{}", self.txn_counter); self.txn_counter += 1;
                        let ap = format!("%ioap{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%iorv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, arg.name).ok();
                        writeln!(out, "{}{} = call i32 @ioctl(i32 {}, i64 {}, ptr {})", indent, rv, fdt, req.name, ap).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::IsTty => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let fdt = format!("%istfdt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%istrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = call i32 @isatty(i32 {})", indent, rv, fdt).ok();
                        writeln!(out, "{}{} = trunc i32 {} to i1", indent, v, rv).ok();
                        return TypedRegister { name: v, ty: Type::Bool };
                    }
                    // ===== Phase A: Process (intrinsics.md D5) =====
                    Intrinsic::SpawnWithOutput => {
                        let cmd = self.emit_expr(out, &args[0], indent);
                        let boxed = self.adapt_to_i64(out, indent, &cmd);
                        let raw = format!("%sp{}", self.txn_counter); self.txn_counter += 1;
                        // brief_spawn_with_output takes i64 (Brief string ptr), returns i64
                        writeln!(out, "{}{} = call i64 @__spawn_with_output__(i64 {})", indent, raw, boxed).ok();
                        return TypedRegister { name: raw, ty: Type::Int };
                    }
                    Intrinsic::Spawn => {
                        let cmd = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%spwnsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%spwndp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%spwncp{}", self.txn_counter); self.txn_counter += 1;
                        let st = format!("%spwnst{}", self.txn_counter); self.txn_counter += 1;
                        let neg = format!("%spwnng{}", self.txn_counter); self.txn_counter += 1;
                        let wst = format!("%spwnws{}", self.txn_counter); self.txn_counter += 1;
                        let andv = format!("%spwnan{}", self.txn_counter); self.txn_counter += 1;
                        let val = format!("%spwnvl{}", self.txn_counter); self.txn_counter += 1;
                        let el = format!("spwn_er{}", self.txn_counter); self.txn_counter += 1;
                        let ol = format!("spwn_ok{}", self.txn_counter); self.txn_counter += 1;
                        let _el = format!("spwn_en{}", self.txn_counter); self.txn_counter += 1;
                        // 2026-06-17: Direct libc — system() + WEXITSTATUS
                        // system() returns int(i32): -1 on error, else waitpid status.
                        // WEXITSTATUS = ((status & 0xff00) >> 8)
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, cmd.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = call i32 @system(ptr {})", indent, st, cp).ok();
                        writeln!(out, "{}{} = icmp slt i32 {}, 0", indent, neg, st).ok();
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, neg, el, ol).ok();
                        writeln!(out, "{}{}:", indent, el).ok();
                        writeln!(out, "{}  br label %{}", indent, _el).ok();
                        writeln!(out, "{}{}:", indent, ol).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, wst, st).ok();
                        writeln!(out, "{}{} = and i64 {}, 0xff00", indent, andv, wst).ok();
                        writeln!(out, "{}{} = lshr i64 {}, 8", indent, val, andv).ok();
                        writeln!(out, "{}  br label %{}", indent, _el).ok();
                        writeln!(out, "{}{}:", indent, _el).ok();
                        writeln!(out, "{}{} = phi i64 [ -1, %{} ], [ {}, %{} ]", indent, v, el, val, ol).ok();
                    }
                    Intrinsic::Argv => {
                        panic!("argv#() called at runtime — not supported in LLVM backend yet");
                    }
                    // ===== Phase B: Raw File I/O (intrinsics.md D2) =====
                    Intrinsic::Open => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let flags = self.emit_expr(out, &args[1], indent);
                        let mode = self.emit_expr(out, &args[2], indent);
                        let sp = format!("%opsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%opdp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%opcp{}", self.txn_counter); self.txn_counter += 1;
                        let ft = format!("%opft{}", self.txn_counter); self.txn_counter += 1;
                        let mt = format!("%opmt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%oprv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
                        writeln!(out, "{}{} = call i32 @open(ptr {}, i32 {}, i32 {})", indent, rv, cp, ft, mt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Close => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let fdt = format!("%cfdt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%crv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = call i32 @close(i32 {})", indent, rv, fdt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Read => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let buf = self.emit_expr(out, &args[1], indent);
                        let count = self.emit_expr(out, &args[2], indent);
                        let fdt = format!("%rfdt{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%rbp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
                        writeln!(out, "{}{} = call i64 @read(i32 {}, ptr {}, i64 {})", indent, v, fdt, bp, count.name).ok();
                    }
                    Intrinsic::Write => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let buf = self.emit_expr(out, &args[1], indent);
                        let count = self.emit_expr(out, &args[2], indent);
                        let fdt = format!("%wfdt{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%wbp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
                        writeln!(out, "{}{} = call i64 @write(i32 {}, ptr {}, i64 {})", indent, v, fdt, bp, count.name).ok();
                    }
                    Intrinsic::LSeek => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let offset = self.emit_expr(out, &args[1], indent);
                        let whence = self.emit_expr(out, &args[2], indent);
                        let fdt = format!("%lfdt{}", self.txn_counter); self.txn_counter += 1;
                        let wt = format!("%lwt{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, wt, whence.name).ok();
                        writeln!(out, "{}{} = call i64 @lseek(i32 {}, i64 {}, i32 {})", indent, v, fdt, offset.name, wt).ok();
                    }
                    Intrinsic::PRead => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let buf = self.emit_expr(out, &args[1], indent);
                        let count = self.emit_expr(out, &args[2], indent);
                        let offset = self.emit_expr(out, &args[3], indent);
                        let fdt = format!("%prfdt{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%prbp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
                        writeln!(out, "{}{} = call i64 @pread(i32 {}, ptr {}, i64 {}, i64 {})", indent, v, fdt, bp, count.name, offset.name).ok();
                    }
                    Intrinsic::PWrite => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let buf = self.emit_expr(out, &args[1], indent);
                        let count = self.emit_expr(out, &args[2], indent);
                        let offset = self.emit_expr(out, &args[3], indent);
                        let fdt = format!("%pwfdt{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%pwbp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
                        writeln!(out, "{}{} = call i64 @pwrite(i32 {}, ptr {}, i64 {}, i64 {})", indent, v, fdt, bp, count.name, offset.name).ok();
                    }
                    Intrinsic::Stat => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%stsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%stdp{}", self.txn_counter); self.txn_counter += 1;
                        let buf = format!("%stbuf{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%strv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, buf, dp).ok();
                        let st = format!("%stst{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = alloca i8, i64 200, align 8", indent, st).ok();
                        writeln!(out, "{}{} = call i32 @stat(ptr {}, ptr {})", indent, rv, buf, st).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::FStat => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let fdt = format!("%fsfdt{}", self.txn_counter); self.txn_counter += 1;
                        let st = format!("%fsst{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%fsrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = alloca i8, i64 200, align 8", indent, st).ok();
                        writeln!(out, "{}{} = call i32 @fstat(i32 {}, ptr {})", indent, rv, fdt, st).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::FTruncate => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let len = self.emit_expr(out, &args[1], indent);
                        let sp = format!("%ttsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%ttdp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%ttcp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%ttrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = call i32 @truncate(ptr {}, i64 {})", indent, rv, cp, len.name).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::FTruncate => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let len = self.emit_expr(out, &args[1], indent);
                        let fdt = format!("%ftfdt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%ftrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = call i32 @ftruncate(i32 {}, i64 {})", indent, rv, fdt, len.name).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::FSync => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let fdt = format!("%yfdt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%yrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = call i32 @fsync(i32 {})", indent, rv, fdt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::FDup => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let fdt = format!("%dfdt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%drv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = call i32 @dup(i32 {})", indent, rv, fdt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::FDup2 => {
                        let old = self.emit_expr(out, &args[0], indent);
                        let newfd = self.emit_expr(out, &args[1], indent);
                        let ot = format!("%d2ot{}", self.txn_counter); self.txn_counter += 1;
                        let nt = format!("%d2nt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%d2rv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ot, old.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, nt, newfd.name).ok();
                        writeln!(out, "{}{} = call i32 @dup2(i32 {}, i32 {})", indent, rv, ot, nt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::FCntl => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let cmd = self.emit_expr(out, &args[1], indent);
                        let arg = self.emit_expr(out, &args[2], indent);
                        let fdt = format!("%cnfdt{}", self.txn_counter); self.txn_counter += 1;
                        let ct = format!("%cnct{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%cnrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ct, cmd.name).ok();
                        writeln!(out, "{}{} = call i32 @fcntl(i32 {}, i32 {}, i64 {})", indent, rv, fdt, ct, arg.name).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    // ===== Phase C: Filesystem (intrinsics.md D3) =====
                    Intrinsic::MkDir => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let mode = self.emit_expr(out, &args[1], indent);
                        let sp = format!("%mksp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%mkdp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%mkcp{}", self.txn_counter); self.txn_counter += 1;
                        let mt = format!("%mkmt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%mkrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
                        writeln!(out, "{}{} = call i32 @mkdir(ptr {}, i32 {})", indent, rv, cp, mt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::RmDir => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%rdsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%rddp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%rdcp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%rdrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = call i32 @rmdir(ptr {})", indent, rv, cp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Unlink => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%ulsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%uldp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%ulcp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%ulrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = call i32 @unlink(ptr {})", indent, rv, cp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Rename => {
                        let old = self.emit_expr(out, &args[0], indent);
                        let new = self.emit_expr(out, &args[1], indent);
                        let osp = format!("%rosp{}", self.txn_counter); self.txn_counter += 1;
                        let odp = format!("%rodp{}", self.txn_counter); self.txn_counter += 1;
                        let nsp = format!("%rnsp{}", self.txn_counter); self.txn_counter += 1;
                        let ndp = format!("%rndp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%rrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, osp, old.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, odp, osp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, new.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
                        let ocp = format!("%rocp{}", self.txn_counter); self.txn_counter += 1;
                        let ncp = format!("%rncp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ocp, odp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
                        writeln!(out, "{}{} = call i32 @rename(ptr {}, ptr {})", indent, rv, ocp, ncp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::SymLink => {
                        let target = self.emit_expr(out, &args[0], indent);
                        let link = self.emit_expr(out, &args[1], indent);
                        let tsp = format!("%sytsp{}", self.txn_counter); self.txn_counter += 1;
                        let tdp = format!("%sytdp{}", self.txn_counter); self.txn_counter += 1;
                        let lsp = format!("%sylsp{}", self.txn_counter); self.txn_counter += 1;
                        let ldp = format!("%syldp{}", self.txn_counter); self.txn_counter += 1;
                        let tcp = format!("%sytcp{}", self.txn_counter); self.txn_counter += 1;
                        let lcp = format!("%sylcp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%syrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, tsp, target.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, tdp, tsp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, lsp, link.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ldp, lsp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, tcp, tdp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, lcp, ldp).ok();
                        writeln!(out, "{}{} = call i32 @symlink(ptr {}, ptr {})", indent, rv, tcp, lcp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::ReadLink => {
                        let path = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}{} = call i64 @__readlink__(i64 {})", indent, v, path.name).ok();
                    }
                    Intrinsic::Link => {
                        let old = self.emit_expr(out, &args[0], indent);
                        let new = self.emit_expr(out, &args[1], indent);
                        let osp = format!("%lkosp{}", self.txn_counter); self.txn_counter += 1;
                        let odp = format!("%lkodp{}", self.txn_counter); self.txn_counter += 1;
                        let nsp = format!("%lknsp{}", self.txn_counter); self.txn_counter += 1;
                        let ndp = format!("%lkndp{}", self.txn_counter); self.txn_counter += 1;
                        let ocp = format!("%lkocp{}", self.txn_counter); self.txn_counter += 1;
                        let ncp = format!("%lkncp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%lkrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, osp, old.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, odp, osp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, new.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ocp, odp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
                        writeln!(out, "{}{} = call i32 @link(ptr {}, ptr {})", indent, rv, ocp, ncp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetCwd => {
                        writeln!(out, "{}{} = call i64 @__getcwd__()", indent, v).ok();
                    }
                    Intrinsic::ChDir => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%chdsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%chddp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%chdcp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%chdrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = call i32 @chdir(ptr {})", indent, rv, cp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::ReadDir => {
                        let path = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}{} = call i64 @__readdir__(i64 {})", indent, v, path.name).ok();
                    }
                    Intrinsic::ChMod => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let mode = self.emit_expr(out, &args[1], indent);
                        let sp = format!("%chmsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%chmdp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%chmcp{}", self.txn_counter); self.txn_counter += 1;
                        let mt = format!("%chmmt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%chmrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
                        writeln!(out, "{}{} = call i32 @chmod(ptr {}, i32 {})", indent, rv, cp, mt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::ChOwn => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let uid = self.emit_expr(out, &args[1], indent);
                        let gid = self.emit_expr(out, &args[2], indent);
                        let sp = format!("%chosp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%chodp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%chocp{}", self.txn_counter); self.txn_counter += 1;
                        let ut = format!("%chout{}", self.txn_counter); self.txn_counter += 1;
                        let gt = format!("%chogt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%chorv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ut, uid.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, gt, gid.name).ok();
                        writeln!(out, "{}{} = call i32 @chown(ptr {}, i32 {}, i32 {})", indent, rv, cp, ut, gt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::UMask => {
                        let mask = self.emit_expr(out, &args[0], indent);
                        let mt = format!("%ummt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%umrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mask.name).ok();
                        writeln!(out, "{}{} = call i32 @umask(i32 {})", indent, rv, mt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Access => {
                        let path = self.emit_expr(out, &args[0], indent);
                        let mode = self.emit_expr(out, &args[1], indent);
                        let sp = format!("%acsp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%acdp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%accp{}", self.txn_counter); self.txn_counter += 1;
                        let mt = format!("%acmt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%acrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
                        writeln!(out, "{}{} = call i32 @access(ptr {}, i32 {})", indent, rv, cp, mt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    // ===== Phase D: Memory (intrinsics.md D1) — Shim category =====
                    Intrinsic::Mmap => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let length = self.emit_expr(out, &args[1], indent);
                        let prot = self.emit_expr(out, &args[2], indent);
                        let flags = self.emit_expr(out, &args[3], indent);
                        let fd = self.emit_expr(out, &args[4], indent);
                        let offset = self.emit_expr(out, &args[5], indent);
                        let ap = format!("%mmap{}", self.txn_counter); self.txn_counter += 1;
                        let pt = format!("%mmpt{}", self.txn_counter); self.txn_counter += 1;
                        let ft = format!("%mmft{}", self.txn_counter); self.txn_counter += 1;
                        let fdt = format!("%mmfdt{}", self.txn_counter); self.txn_counter += 1;
                        let ret_ptr = format!("%mmret{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, pt, prot.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = call ptr @mmap(ptr {}, i64 {}, i32 {}, i32 {}, i32 {}, i64 {})", indent, ret_ptr, ap, length.name, pt, ft, fdt, offset.name).ok();
                        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ret_ptr).ok();
                    }
                    Intrinsic::MUnmap => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let length = self.emit_expr(out, &args[1], indent);
                        let ap = format!("%mua{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%murv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
                        writeln!(out, "{}{} = call i32 @munmap(ptr {}, i64 {})", indent, rv, ap, length.name).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::MProtect => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let length = self.emit_expr(out, &args[1], indent);
                        let prot = self.emit_expr(out, &args[2], indent);
                        let ap = format!("%mpa{}", self.txn_counter); self.txn_counter += 1;
                        let pt = format!("%mppt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%mprv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, pt, prot.name).ok();
                        writeln!(out, "{}{} = call i32 @mprotect(ptr {}, i64 {}, i32 {})", indent, rv, ap, length.name, pt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Brk => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let ap = format!("%brap{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%brrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
                        writeln!(out, "{}{} = call i32 @brk(ptr {})", indent, rv, ap).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::MLock => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let length = self.emit_expr(out, &args[1], indent);
                        let ap = format!("%mla{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%mlrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
                        writeln!(out, "{}{} = call i32 @mlock(ptr {}, i64 {})", indent, rv, ap, length.name).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    // ===== Phase D: Synchronization (intrinsics.md D9) — Native category =====
                    Intrinsic::AtomicLoad => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let _order = self.emit_expr(out, &args[1], indent); // order arg consumed for eval
                        let ptr = format!("%aptr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
                        writeln!(out, "{}{} = load atomic i64, ptr {} acquire, align 8", indent, v, ptr).ok();
                    }
                    Intrinsic::AtomicStore => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let val = self.emit_expr(out, &args[1], indent);
                        let _order = self.emit_expr(out, &args[2], indent);
                        let ptr = format!("%aptr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
                        writeln!(out, "{}store atomic i64 {}, ptr {} release, align 8", indent, val.name, ptr).ok();
                        writeln!(out, "{}{} = add i64 undef, 0 ; atomic_store is void", indent, v).ok();
                    }
                    Intrinsic::AtomicCas => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let expected = self.emit_expr(out, &args[1], indent);
                        let new = self.emit_expr(out, &args[2], indent);
                        let _order = self.emit_expr(out, &args[3], indent);
                        let ptr = format!("%aptr{}", self.txn_counter); self.txn_counter += 1;
                        let pair = format!("%apair{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
                        writeln!(out, "{}{} = cmpxchg ptr {}, i64 {}, i64 {} acquire", indent, pair, ptr, expected.name, new.name).ok();
                        writeln!(out, "{}{} = extractvalue {{ i64, i1 }} {}, 0", indent, v, pair).ok();
                    }
                    Intrinsic::AtomicXchg => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let val = self.emit_expr(out, &args[1], indent);
                        let _order = self.emit_expr(out, &args[2], indent);
                        let ptr = format!("%aptr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
                        writeln!(out, "{}{} = atomicrmw xchg ptr {}, i64 {} acquire", indent, v, ptr, val.name).ok();
                    }
                    Intrinsic::AtomicAdd => {
                        let addr = self.emit_expr(out, &args[0], indent);
                        let val = self.emit_expr(out, &args[1], indent);
                        let _order = self.emit_expr(out, &args[2], indent);
                        let ptr = format!("%aptr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
                        writeln!(out, "{}{} = atomicrmw add ptr {}, i64 {} acquire", indent, v, ptr, val.name).ok();
                    }
                    Intrinsic::Fence => {
                        let _order = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}fence acquire", indent).ok();
                        writeln!(out, "{}{} = add i64 undef, 0 ; fence is void", indent, v).ok();
                    }
                    Intrinsic::Futex => {
                        // Evaluate all arguments for side effects (futex is a real
                        // syscall with observable behavior when implemented).
                        let _uaddr = self.emit_expr(out, &args[0], indent);
                        let _op = self.emit_expr(out, &args[1], indent);
                        let _val = self.emit_expr(out, &args[2], indent);
                        let _timeout = self.emit_expr(out, &args[3], indent);
                        let _uaddr2 = self.emit_expr(out, &args[4], indent);
                        let _val3 = self.emit_expr(out, &args[5], indent);
                        // 2026-06-17: Inline stub — C brief_futex was already a
                        // stub returning -1 (futex is Linux-specific, architecture-
                        // dependent). A real implementation would use @syscall.
                        writeln!(out, "{}{} = add i64 0, -1", indent, v).ok();
                    }
                    // ===== Phase E: IPC (intrinsics.md D11) — Shim =====
                    Intrinsic::Pipe => {
                        let fds = self.emit_expr(out, &args[0], indent);
                        let parr = format!("%pipearr{}", self.txn_counter); self.txn_counter += 1;
                        let prv = format!("%piperv{}", self.txn_counter); self.txn_counter += 1;
                        let p0 = format!("%pipep0{}", self.txn_counter); self.txn_counter += 1;
                        let p1 = format!("%pipep1{}", self.txn_counter); self.txn_counter += 1;
                        let pf0 = format!("%pipef0{}", self.txn_counter); self.txn_counter += 1;
                        let pf1 = format!("%pipef1{}", self.txn_counter); self.txn_counter += 1;
                        let zf0 = format!("%pipef0z{}", self.txn_counter); self.txn_counter += 1;
                        let zf1 = format!("%pipef1z{}", self.txn_counter); self.txn_counter += 1;
                        let dst1 = format!("%piped1{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%pipebp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = alloca [2 x i32], align 4", indent, parr).ok();
                        writeln!(out, "{}{} = call i32 @pipe(ptr {})", indent, prv, parr).ok();
                        writeln!(out, "{}{} = getelementptr [2 x i32], ptr {}, i64 0, i64 0", indent, p0, parr).ok();
                        writeln!(out, "{}{} = getelementptr [2 x i32], ptr {}, i64 0, i64 1", indent, p1, parr).ok();
                        writeln!(out, "{}{} = load i32, ptr {}", indent, pf0, p0).ok();
                        writeln!(out, "{}{} = load i32, ptr {}", indent, pf1, p1).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, zf0, pf0).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, zf1, pf1).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, fds.name).ok();
                        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, zf0, bp).ok();
                        writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, dst1, bp).ok();
                        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, zf1, dst1).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, prv).ok();
                    }
                    Intrinsic::ShmOpen => {
                        let name = self.emit_expr(out, &args[0], indent);
                        let flags = self.emit_expr(out, &args[1], indent);
                        let mode = self.emit_expr(out, &args[2], indent);
                        let nsp = format!("%shnsp{}", self.txn_counter); self.txn_counter += 1;
                        let ndp = format!("%shndp{}", self.txn_counter); self.txn_counter += 1;
                        let ncp = format!("%shncp{}", self.txn_counter); self.txn_counter += 1;
                        let ft = format!("%shft{}", self.txn_counter); self.txn_counter += 1;
                        let mt = format!("%shmt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%shrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, name.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
                        writeln!(out, "{}{} = call i32 @shm_open(ptr {}, i32 {}, i32 {})", indent, rv, ncp, ft, mt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::ShmUnlink => {
                        let name = self.emit_expr(out, &args[0], indent);
                        let nsp = format!("%slnsp{}", self.txn_counter); self.txn_counter += 1;
                        let ndp = format!("%slndp{}", self.txn_counter); self.txn_counter += 1;
                        let ncp = format!("%slncp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%slrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, name.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
                        writeln!(out, "{}{} = call i32 @shm_unlink(ptr {})", indent, rv, ncp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::SemOpen => {
                        let name = self.emit_expr(out, &args[0], indent);
                        let flags = self.emit_expr(out, &args[1], indent);
                        let mode = self.emit_expr(out, &args[2], indent);
                        let value = self.emit_expr(out, &args[3], indent);
                        let nsp = format!("%sonsp{}", self.txn_counter); self.txn_counter += 1;
                        let ndp = format!("%sondp{}", self.txn_counter); self.txn_counter += 1;
                        let ncp = format!("%soncp{}", self.txn_counter); self.txn_counter += 1;
                        let ft = format!("%sonft{}", self.txn_counter); self.txn_counter += 1;
                        let mt = format!("%sonmt{}", self.txn_counter); self.txn_counter += 1;
                        let vt = format!("%sonvt{}", self.txn_counter); self.txn_counter += 1;
                        let rp = format!("%sonrp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, name.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, vt, value.name).ok();
                        writeln!(out, "{}{} = call ptr @sem_open(ptr {}, i32 {}, i32 {}, i32 {})", indent, rp, ncp, ft, mt, vt).ok();
                        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, rp).ok();
                    }
                    Intrinsic::SemWait => {
                        let sem = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%swsp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%swrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, sem.name).ok();
                        writeln!(out, "{}{} = call i32 @sem_wait(ptr {})", indent, rv, sp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::SemPost => {
                        let sem = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%spsp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%sprv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, sem.name).ok();
                        writeln!(out, "{}{} = call i32 @sem_post(ptr {})", indent, rv, sp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    // ===== Phase F: Signals (intrinsics.md D8) — Shim =====
                    Intrinsic::SigAction => {
                        let signum = self.emit_expr(out, &args[0], indent);
                        let handler = self.emit_expr(out, &args[1], indent);
                        writeln!(out, "{}{} = call i64 @__sigaction__(i64 {}, i64 {})", indent, v, signum.name, handler.name).ok();
                    }
                    Intrinsic::SigProcMask => {
                        let how = self.emit_expr(out, &args[0], indent);
                        let mask = self.emit_expr(out, &args[1], indent);
                        writeln!(out, "{}{} = call i64 @__sigprocmask__(i64 {}, i64 {})", indent, v, how.name, mask.name).ok();
                    }
                    Intrinsic::Kill => {
                        let pid = self.emit_expr(out, &args[0], indent);
                        let sig = self.emit_expr(out, &args[1], indent);
                        let pt = format!("%kpt{}", self.txn_counter); self.txn_counter += 1;
                        let st = format!("%kst{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%krv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, pt, pid.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, st, sig.name).ok();
                        writeln!(out, "{}{} = call i32 @kill(i32 {}, i32 {})", indent, rv, pt, st).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::SignalFd => {
                        // 2026-06-17: Direct libc — alloca sigset_t + memset + signalfd.
                        // The C shim ignored the mask arg and created an empty set.
                        let _mask = self.emit_expr(out, &args[0], indent);
                        let set = format!("%sigfds{}", self.txn_counter); self.txn_counter += 1;
                        let bc = format!("%sigfdbc{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%sigfdrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = alloca [16 x i64], align 8", indent, set).ok();
                        writeln!(out, "{}{} = bitcast [16 x i64]* {} to ptr", indent, bc, set).ok();
                        writeln!(out, "{}call void @llvm.memset.p0i8.i64(ptr {}, i8 0, i64 128, i1 false)", indent, bc).ok();
                        writeln!(out, "{}{} = call i32 @signalfd(i32 -1, ptr {}, i32 2048)", indent, rv, bc).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::TimerFdCreate => {
                        // 2026-06-17: Direct libc — timerfd_create + alloca itimerspec + timerfd_settime.
                        // itimerspec layout on x86_64: {i64,i64,i64,i64} = {interval_sec, interval_nsec,
                        // value_sec, value_nsec}
                        let hz = self.emit_expr(out, &args[0], indent);
                        let nsec = format!("%tfnsec{}", self.txn_counter); self.txn_counter += 1;
                        let fd = format!("%tffd{}", self.txn_counter); self.txn_counter += 1;
                        let spec = format!("%tfspec{}", self.txn_counter); self.txn_counter += 1;
                        let sp = format!("%tfspp{}", self.txn_counter); self.txn_counter += 1;
                        let is0 = format!("%tfis0{}", self.txn_counter); self.txn_counter += 1;
                        let z_l = format!("tf_z{}", self.txn_counter); self.txn_counter += 1;
                        let s_l = format!("tf_s{}", self.txn_counter); self.txn_counter += 1;
                        let e_l = format!("tf_e{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%tfrv{}", self.txn_counter); self.txn_counter += 1;
                        let phiv = format!("%tfphi{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = udiv i64 {}, 1000000000", indent, nsec, hz.name).ok();
                        writeln!(out, "{}{} = call i32 @timerfd_create(i32 1, i32 2048)", indent, fd).ok();
                        writeln!(out, "{}{} = icmp sgt i64 {}, 0", indent, is0, hz.name).ok();
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is0, s_l, z_l).ok();
                        writeln!(out, "{}{}:", indent, z_l).ok();
                        writeln!(out, "{}  br label %{}", indent, e_l).ok();
                        writeln!(out, "{}{}:", indent, s_l).ok();
                        writeln!(out, "{}{} = alloca {{ i64, i64, i64, i64 }}, align 8", indent, spec).ok();
                        writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, sp, spec).ok();
                        writeln!(out, "{}call void @llvm.memset.p0i8.i64(ptr {}, i8 0, i64 32, i1 false)", indent, sp).ok();
                        let iv_ns = format!("%tfivns{}", self.txn_counter); self.txn_counter += 1;
                        let vl_ns = format!("%tfvlns{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = urem i64 {}, 1000000000", indent, iv_ns, hz.name).ok();
                        writeln!(out, "{}{} = urem i64 {}, 1000000000", indent, vl_ns, hz.name).ok();
                        let iv_off = format!("%tfiiv{}", self.txn_counter); self.txn_counter += 1;
                        let vl_off = format!("%tfivl{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 8", indent, iv_off, sp).ok();
                        writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 24", indent, vl_off, sp).ok();
                        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, iv_ns, iv_off).ok();
                        writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, vl_ns, vl_off).ok();
                        writeln!(out, "{}{} = call i32 @timerfd_settime(i32 {}, i32 0, ptr {}, ptr null)", indent, rv, fd, sp).ok();
                        writeln!(out, "{}  br label %{}", indent, e_l).ok();
                        writeln!(out, "{}{}:", indent, e_l).ok();
                        writeln!(out, "{}{} = phi i32 [ {}, %{} ], [ {}, %{} ]", indent, phiv, fd, z_l, rv, s_l).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, phiv).ok();
                    }
                    // ===== Phase G: Networking (intrinsics.md D10) — Shim =====
                    Intrinsic::Socket => {
                        let domain = self.emit_expr(out, &args[0], indent);
                        let sock_type = self.emit_expr(out, &args[1], indent);
                        let protocol = self.emit_expr(out, &args[2], indent);
                        let dt = format!("%sodt{}", self.txn_counter); self.txn_counter += 1;
                        let st = format!("%sost{}", self.txn_counter); self.txn_counter += 1;
                        let pt = format!("%sopt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%sorv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, dt, domain.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, st, sock_type.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, pt, protocol.name).ok();
                        writeln!(out, "{}{} = call i32 @socket(i32 {}, i32 {}, i32 {})", indent, rv, dt, st, pt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Bind => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let addr = self.emit_expr(out, &args[1], indent);
                        let addrlen = self.emit_expr(out, &args[2], indent);
                        let fdt = format!("%bifdt{}", self.txn_counter); self.txn_counter += 1;
                        let ap = format!("%bia{}", self.txn_counter); self.txn_counter += 1;
                        let alt = format!("%bialt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%birv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
                        writeln!(out, "{}{} = call i32 @bind(i32 {}, ptr {}, i32 {})", indent, rv, fdt, ap, alt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Listen => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let backlog = self.emit_expr(out, &args[1], indent);
                        let fdt = format!("%lifdt{}", self.txn_counter); self.txn_counter += 1;
                        let bt = format!("%libt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%lirv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, bt, backlog.name).ok();
                        writeln!(out, "{}{} = call i32 @listen(i32 {}, i32 {})", indent, rv, fdt, bt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Accept => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let addr = self.emit_expr(out, &args[1], indent);
                        let addrlen = self.emit_expr(out, &args[2], indent);
                        let fdt = format!("%acfdt{}", self.txn_counter); self.txn_counter += 1;
                        let ap = format!("%acap{}", self.txn_counter); self.txn_counter += 1;
                        let als = format!("%acals{}", self.txn_counter); self.txn_counter += 1;
                        let alt = format!("%acalt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%acrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
                        writeln!(out, "{}{} = alloca i32, align 4", indent, als).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
                        writeln!(out, "{}store i32 {}, ptr {}, align 4", indent, alt, als).ok();
                        writeln!(out, "{}{} = call i32 @accept(i32 {}, ptr {}, ptr {})", indent, rv, fdt, ap, als).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Connect => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let addr = self.emit_expr(out, &args[1], indent);
                        let addrlen = self.emit_expr(out, &args[2], indent);
                        let fdt = format!("%cofdt{}", self.txn_counter); self.txn_counter += 1;
                        let ap = format!("%coap{}", self.txn_counter); self.txn_counter += 1;
                        let alt = format!("%coalt{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%corv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
                        writeln!(out, "{}{} = call i32 @connect(i32 {}, ptr {}, i32 {})", indent, rv, fdt, ap, alt).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Send => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let buf = self.emit_expr(out, &args[1], indent);
                        let len = self.emit_expr(out, &args[2], indent);
                        let flags = self.emit_expr(out, &args[3], indent);
                        let fdt = format!("%sdfdt{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%sdbp{}", self.txn_counter); self.txn_counter += 1;
                        let ft = format!("%sdft{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
                        writeln!(out, "{}{} = call i64 @send(i32 {}, ptr {}, i64 {}, i32 {})", indent, v, fdt, bp, len.name, ft).ok();
                    }
                    Intrinsic::Recv => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let buf = self.emit_expr(out, &args[1], indent);
                        let len = self.emit_expr(out, &args[2], indent);
                        let flags = self.emit_expr(out, &args[3], indent);
                        let fdt = format!("%rcfdt{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%rcbp{}", self.txn_counter); self.txn_counter += 1;
                        let ft = format!("%rcft{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
                        writeln!(out, "{}{} = call i64 @recv(i32 {}, ptr {}, i64 {}, i32 {})", indent, v, fdt, bp, len.name, ft).ok();
                    }
                    Intrinsic::SendTo => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let buf = self.emit_expr(out, &args[1], indent);
                        let len = self.emit_expr(out, &args[2], indent);
                        let flags = self.emit_expr(out, &args[3], indent);
                        let dest_addr = self.emit_expr(out, &args[4], indent);
                        let addrlen = self.emit_expr(out, &args[5], indent);
                        let fdt = format!("%stofdt{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%stobp{}", self.txn_counter); self.txn_counter += 1;
                        let ft = format!("%stoft{}", self.txn_counter); self.txn_counter += 1;
                        let da = format!("%stoda{}", self.txn_counter); self.txn_counter += 1;
                        let alt = format!("%stoalt{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, da, dest_addr.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
                        writeln!(out, "{}{} = call i64 @sendto(i32 {}, ptr {}, i64 {}, i32 {}, ptr {}, i32 {})", indent, v, fdt, bp, len.name, ft, da, alt).ok();
                    }
                    Intrinsic::RecvFrom => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let buf = self.emit_expr(out, &args[1], indent);
                        let len = self.emit_expr(out, &args[2], indent);
                        let flags = self.emit_expr(out, &args[3], indent);
                        let src_addr = self.emit_expr(out, &args[4], indent);
                        let addrlen = self.emit_expr(out, &args[5], indent);
                        let fdt = format!("%rfdt{}", self.txn_counter); self.txn_counter += 1;
                        let bp = format!("%rbp{}", self.txn_counter); self.txn_counter += 1;
                        let ft = format!("%rft{}", self.txn_counter); self.txn_counter += 1;
                        let sa = format!("%rsa{}", self.txn_counter); self.txn_counter += 1;
                        let als = format!("%rals{}", self.txn_counter); self.txn_counter += 1;
                        let alt = format!("%ralt{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sa, src_addr.name).ok();
                        writeln!(out, "{}{} = alloca i32, align 4", indent, als).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
                        writeln!(out, "{}store i32 {}, ptr {}, align 4", indent, alt, als).ok();
                        writeln!(out, "{}{} = call i64 @recvfrom(i32 {}, ptr {}, i64 {}, i32 {}, ptr {}, ptr {})", indent, v, fdt, bp, len.name, ft, sa, als).ok();
                    }
                    Intrinsic::SetSockOpt => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let level = self.emit_expr(out, &args[1], indent);
                        let opt = self.emit_expr(out, &args[2], indent);
                        let val = self.emit_expr(out, &args[3], indent);
                        let len = self.emit_expr(out, &args[4], indent);
                        let fdt = format!("%ssofdt{}", self.txn_counter); self.txn_counter += 1;
                        let lt = format!("%ssolt{}", self.txn_counter); self.txn_counter += 1;
                        let ot = format!("%ssoot{}", self.txn_counter); self.txn_counter += 1;
                        let vp = format!("%ssovp{}", self.txn_counter); self.txn_counter += 1;
                        let lt2 = format!("%ssolt2{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%ssorv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, lt, level.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ot, opt.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, vp, val.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, lt2, len.name).ok();
                        writeln!(out, "{}{} = call i32 @setsockopt(i32 {}, i32 {}, i32 {}, ptr {}, i32 {})", indent, rv, fdt, lt, ot, vp, lt2).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetSockOpt => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let level = self.emit_expr(out, &args[1], indent);
                        let opt = self.emit_expr(out, &args[2], indent);
                        let val = self.emit_expr(out, &args[3], indent);
                        let len = self.emit_expr(out, &args[4], indent);
                        let fdt = format!("%gsofdt{}", self.txn_counter); self.txn_counter += 1;
                        let lt = format!("%gsolt{}", self.txn_counter); self.txn_counter += 1;
                        let ot = format!("%gsoot{}", self.txn_counter); self.txn_counter += 1;
                        let vp = format!("%gsovp{}", self.txn_counter); self.txn_counter += 1;
                        let ls = format!("%gsols{}", self.txn_counter); self.txn_counter += 1;
                        let lt2 = format!("%gsolt2{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%gsorv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, lt, level.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ot, opt.name).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, vp, val.name).ok();
                        writeln!(out, "{}{} = alloca i32, align 4", indent, ls).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, lt2, len.name).ok();
                        writeln!(out, "{}store i32 {}, ptr {}, align 4", indent, lt2, ls).ok();
                        writeln!(out, "{}{} = call i32 @getsockopt(i32 {}, i32 {}, i32 {}, ptr {}, ptr {})", indent, rv, fdt, lt, ot, vp, ls).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::Shutdown => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        let how = self.emit_expr(out, &args[1], indent);
                        let fdt = format!("%shfdt{}", self.txn_counter); self.txn_counter += 1;
                        let ht = format!("%shht{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%shrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ht, how.name).ok();
                        writeln!(out, "{}{} = call i32 @shutdown(i32 {}, i32 {})", indent, rv, fdt, ht).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetAddrInfo => {
                        let node = self.emit_expr(out, &args[0], indent);
                        let service = self.emit_expr(out, &args[1], indent);
                        writeln!(out, "{}{} = call i64 @__getaddrinfo__(i64 {}, i64 {})", indent, v, node.name, service.name).ok();
                    }
                    // ===== Phase H: Everything Else (intrinsics.md D6, D7) — Shim =====
                    Intrinsic::GetEnv => {
                        let name = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%gesp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%gedp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%gecp{}", self.txn_counter); self.txn_counter += 1;
                        let rp = format!("%gerp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, name.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = call ptr @getenv(ptr {})", indent, rp, cp).ok();
                        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, rp).ok();
                    }
                    Intrinsic::SetEnv => {
                        let name = self.emit_expr(out, &args[0], indent);
                        let val = self.emit_expr(out, &args[1], indent);
                        let nsp = format!("%senp{}", self.txn_counter); self.txn_counter += 1;
                        let ndp = format!("%sendp{}", self.txn_counter); self.txn_counter += 1;
                        let vsp = format!("%sevp{}", self.txn_counter); self.txn_counter += 1;
                        let vdp = format!("%sevdp{}", self.txn_counter); self.txn_counter += 1;
                        let ncp = format!("%secnp{}", self.txn_counter); self.txn_counter += 1;
                        let vcp = format!("%secvp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%serv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, name.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, vsp, val.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, vdp, vsp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, vcp, vdp).ok();
                        writeln!(out, "{}{} = call i32 @setenv(ptr {}, ptr {}, i32 1)", indent, rv, ncp, vcp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::UnsetEnv => {
                        let name = self.emit_expr(out, &args[0], indent);
                        let sp = format!("%uensp{}", self.txn_counter); self.txn_counter += 1;
                        let dp = format!("%uendp{}", self.txn_counter); self.txn_counter += 1;
                        let cp = format!("%uencp{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%uenrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, name.name).ok();
                        writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                        writeln!(out, "{}{} = call i32 @unsetenv(ptr {})", indent, rv, cp).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetPid => {
                        let rv = format!("%gpidrv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @getpid()", indent, rv).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetPPid => {
                        let rv = format!("%gpprv{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @getppid()", indent, rv).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::ClockGetTime => {
                        let clock_id = self.emit_expr(out, &args[0], indent);
                        let ci = format!("%cgtci{}", self.txn_counter); self.txn_counter += 1;
                        let ts = format!("%cgtts{}", self.txn_counter); self.txn_counter += 1;
                        let _rv = format!("%cgtrv{}", self.txn_counter); self.txn_counter += 1;
                        let sp = format!("%cgtsp{}", self.txn_counter); self.txn_counter += 1;
                        let np = format!("%cgtnp{}", self.txn_counter); self.txn_counter += 1;
                        let sec = format!("%cgtsec{}", self.txn_counter); self.txn_counter += 1;
                        let nsec = format!("%cgtnsec{}", self.txn_counter); self.txn_counter += 1;
                        let mulv = format!("%cgtmul{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ci, clock_id.name).ok();
                        writeln!(out, "{}{} = alloca {{ i64, i64 }}, align 8", indent, ts).ok();
                        writeln!(out, "{}{} = call i32 @clock_gettime(i32 {}, ptr {})", indent, _rv, ci, ts).ok();
                        writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 0", indent, sp, ts).ok();
                        writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 1", indent, np, ts).ok();
                        writeln!(out, "{}{} = load i64, ptr {}", indent, sec, sp).ok();
                        writeln!(out, "{}{} = load i64, ptr {}", indent, nsec, np).ok();
                        writeln!(out, "{}{} = mul i64 {}, 1000000000", indent, mulv, sec).ok();
                        writeln!(out, "{}{} = add i64 {}, {}", indent, v, mulv, nsec).ok();
                    }
                    Intrinsic::NanoSleep => {
                        let ns = self.emit_expr(out, &args[0], indent);
                        let sec = format!("%nnsec{}", self.txn_counter); self.txn_counter += 1;
                        let nsec = format!("%nnnsec{}", self.txn_counter); self.txn_counter += 1;
                        let ts = format!("%nnts{}", self.txn_counter); self.txn_counter += 1;
                        let tsp = format!("%nntsp{}", self.txn_counter); self.txn_counter += 1;
                        let tsnp = format!("%nntsnp{}", self.txn_counter); self.txn_counter += 1;
                        let rem = format!("%nnrem{}", self.txn_counter); self.txn_counter += 1;
                        let rv = format!("%nnrv{}", self.txn_counter); self.txn_counter += 1;
                        let zero_c = format!("%nnzc{}", self.txn_counter); self.txn_counter += 1;
                        let z_l = format!("nn_z{}", self.txn_counter); self.txn_counter += 1;
                        let r_l = format!("nn_r{}", self.txn_counter); self.txn_counter += 1;
                        let e_l = format!("nn_e{}", self.txn_counter); self.txn_counter += 1;
                        let rsp = format!("%nnrsp{}", self.txn_counter); self.txn_counter += 1;
                        let rnp = format!("%nnrnp{}", self.txn_counter); self.txn_counter += 1;
                        let rsec = format!("%nnrsec{}", self.txn_counter); self.txn_counter += 1;
                        let rnsec = format!("%nnrnsec{}", self.txn_counter); self.txn_counter += 1;
                        let rmul = format!("%nnrmul{}", self.txn_counter); self.txn_counter += 1;
                        let rns = format!("%nnrns{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = udiv i64 {}, 1000000000", indent, sec, ns.name).ok();
                        writeln!(out, "{}{} = urem i64 {}, 1000000000", indent, nsec, ns.name).ok();
                        writeln!(out, "{}{} = alloca {{ i64, i64 }}, align 8", indent, ts).ok();
                        writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 0", indent, tsp, ts).ok();
                        writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 1", indent, tsnp, ts).ok();
                        writeln!(out, "{}store i64 {}, ptr {}", indent, sec, tsp).ok();
                        writeln!(out, "{}store i64 {}, ptr {}", indent, nsec, tsnp).ok();
                        writeln!(out, "{}{} = alloca {{ i64, i64 }}, align 8", indent, rem).ok();
                        writeln!(out, "{}{} = call i32 @nanosleep(ptr {}, ptr {})", indent, rv, ts, rem).ok();
                        writeln!(out, "{}{} = icmp eq i32 {}, 0", indent, zero_c, rv).ok();
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, zero_c, z_l, r_l).ok();
                        writeln!(out, "{}{}:", indent, z_l).ok();
                        writeln!(out, "{}  br label %{}", indent, e_l).ok();
                        writeln!(out, "{}{}:", indent, r_l).ok();
                        writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 0", indent, rsp, rem).ok();
                        writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 1", indent, rnp, rem).ok();
                        writeln!(out, "{}{} = load i64, ptr {}", indent, rsec, rsp).ok();
                        writeln!(out, "{}{} = load i64, ptr {}", indent, rnsec, rnp).ok();
                        writeln!(out, "{}{} = mul i64 {}, 1000000000", indent, rmul, rsec).ok();
                        writeln!(out, "{}{} = add i64 {}, {}", indent, rns, rmul, rnsec).ok();
                        writeln!(out, "{}  br label %{}", indent, e_l).ok();
                        writeln!(out, "{}{}:", indent, e_l).ok();
                        writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, v, z_l, rns, r_l).ok();
                    }
                    Intrinsic::Sort => {
                        if let Some(first) = args.first() {
                            let list_val = self.emit_expr(out, first, indent);
                            let boxed = self.adapt_to_i64(out, indent, &list_val);
                            writeln!(out, "{}{} = call i64 @__sort_list__(i64 {})", indent, v, boxed).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::Reverse => {
                        if let Some(first) = args.first() {
                            let list_val = self.emit_expr(out, first, indent);
                            let boxed = self.adapt_to_i64(out, indent, &list_val);
                            writeln!(out, "{}{} = call i64 @__reverse_list__(i64 {})", indent, v, boxed).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::Range => {
                        if let Some(first) = args.first() {
                            let end_val = self.emit_expr(out, first, indent);
                            let boxed = self.adapt_to_i64(out, indent, &end_val);
                            writeln!(out, "{}{} = call i64 @__range__(i64 {})", indent, v, boxed).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::TrimLeft => {
                        if let Some(first) = args.first() {
                            let s = self.emit_expr(out, first, indent);
                            let sp = format!("%tls{}", self.txn_counter); self.txn_counter += 1;
                            let dp = format!("%tld{}", self.txn_counter); self.txn_counter += 1;
                            let cp = format!("%tlc{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                            writeln!(out, "{}{} = call i64 @__trim_left__(ptr {})", indent, v, cp).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::TrimRight => {
                        if let Some(first) = args.first() {
                            let s = self.emit_expr(out, first, indent);
                            let sp = format!("%trs{}", self.txn_counter); self.txn_counter += 1;
                            let dp = format!("%trd{}", self.txn_counter); self.txn_counter += 1;
                            let cp = format!("%trc{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                            writeln!(out, "{}{} = call i64 @__trim_right__(ptr {})", indent, v, cp).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::ToLower => {
                        if let Some(first) = args.first() {
                            let s = self.emit_expr(out, first, indent);
                            let sp = format!("%tlrs{}", self.txn_counter); self.txn_counter += 1;
                            let dp = format!("%tlrd{}", self.txn_counter); self.txn_counter += 1;
                            let cp = format!("%tlrc{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                            writeln!(out, "{}{} = call i64 @__to_lower__(ptr {})", indent, v, cp).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::ContainsAt => {
                        if args.len() >= 3 {
                            let haystack = self.emit_expr(out, &args[0], indent);
                            let needle = self.emit_expr(out, &args[1], indent);
                            let start = self.emit_expr(out, &args[2], indent);
                            let sp = format!("%cas{}", self.txn_counter); self.txn_counter += 1;
                            let dp = format!("%cad{}", self.txn_counter); self.txn_counter += 1;
                            let cp = format!("%cac{}", self.txn_counter); self.txn_counter += 1;
                            let sp2 = format!("%cbs{}", self.txn_counter); self.txn_counter += 1;
                            let dp2 = format!("%cbd{}", self.txn_counter); self.txn_counter += 1;
                            let cp2 = format!("%cbc{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, haystack).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp2, needle).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp2, sp2).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp2, dp2).ok();
                            writeln!(out, "{}{} = call i64 @__contains_at__(ptr {}, ptr {}, i64 {})", indent, v, cp, cp2, start).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::FindFrom => {
                        if args.len() >= 3 {
                            let s = self.emit_expr(out, &args[0], indent);
                            let needle = self.emit_expr(out, &args[1], indent);
                            let start = self.emit_expr(out, &args[2], indent);
                            let sp = format!("%ffs{}", self.txn_counter); self.txn_counter += 1;
                            let dp = format!("%ffd{}", self.txn_counter); self.txn_counter += 1;
                            let cp = format!("%ffc{}", self.txn_counter); self.txn_counter += 1;
                            let sp2 = format!("%fns{}", self.txn_counter); self.txn_counter += 1;
                            let dp2 = format!("%fnd{}", self.txn_counter); self.txn_counter += 1;
                            let cp2 = format!("%fnc{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp2, needle).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp2, sp2).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp2, dp2).ok();
                            writeln!(out, "{}{} = call i64 @__find_from__(ptr {}, ptr {}, i64 {})", indent, v, cp, cp2, start).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 -1, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::SplitN => {
                        if args.len() >= 3 {
                            let s = self.emit_expr(out, &args[0], indent);
                            let delim = self.emit_expr(out, &args[1], indent);
                            let n_val = self.emit_expr(out, &args[2], indent);
                            let sp = format!("%sps{}", self.txn_counter); self.txn_counter += 1;
                            let dp = format!("%spd{}", self.txn_counter); self.txn_counter += 1;
                            let cp = format!("%spc{}", self.txn_counter); self.txn_counter += 1;
                            let sp2 = format!("%sds{}", self.txn_counter); self.txn_counter += 1;
                            let dp2 = format!("%sdd{}", self.txn_counter); self.txn_counter += 1;
                            let cp2 = format!("%sdc{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp2, delim).ok();
                            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp2, sp2).ok();
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp2, dp2).ok();
                            writeln!(out, "{}{} = call i64 @__splitn__(ptr {}, ptr {}, i64 {})", indent, v, cp, cp2, n_val).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::IntToStr => {
                        if let Some(first) = args.first() {
                            let n = self.emit_expr(out, first, indent);
                            let boxed = self.adapt_to_i64(out, indent, &n);
                            writeln!(out, "{}{} = call i64 @__int_to_str__(i64 {})", indent, v, boxed).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }
                    Intrinsic::Strlen => {
                        if let Some(first) = args.first() {
                            let ptr = self.emit_expr(out, first, indent);
                            let ptr_name = self.adapt_to_i64(out, indent, &ptr);
                            let unbox = format!("%pstr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, unbox, ptr_name).ok();
                            writeln!(out, "{}{} = call i64 @strlen(ptr {})", indent, v, unbox).ok();
                        } else {
                            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        }
                    }

                    // Benchmark intrinsics (2026-06-16) — direct libc, no brief_rt.c shims
                    Intrinsic::PrintInt => {
                        let n = self.emit_expr(out, &args[0], indent);
                        let so = format!("%pso{}", self.txn_counter); self.txn_counter += 1;
                        let fmt = format!("%pfi{}", self.txn_counter); self.txn_counter += 1;
                        let pi = format!("%ppi{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                        writeln!(out, "{}{} = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0", indent, fmt).ok();
                        writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, i64 {})",
                            indent, pi, so, fmt, n).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, pi).ok();
                    }
                    Intrinsic::PutChar => {
                        let c = self.emit_expr(out, &args[0], indent);
                        let ct = format!("%pct{}", self.txn_counter); self.txn_counter += 1;
                        let pc = format!("%ppc{}", self.txn_counter); self.txn_counter += 1;
                        let so = format!("%pso{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, ct, c).ok();
                        writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                        writeln!(out, "{}{} = call i32 @fputc(i32 {}, ptr {})",
                            indent, v, ct, so).ok();
                    }
                    Intrinsic::PrintFloat => {
                        let d = self.emit_expr(out, &args[0], indent);
                        let fl = self.ensure_float_reg(out, indent, &d);
                        let fd = format!("%pfd{}", self.txn_counter); self.txn_counter += 1;
                        let so = format!("%pso{}", self.txn_counter); self.txn_counter += 1;
                        let fmt = format!("%pff{}", self.txn_counter); self.txn_counter += 1;
                        let pf = format!("%ppf{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = fpext float {} to double", indent, fd, fl).ok();
                        writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                        writeln!(out, "{}{} = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0", indent, fmt).ok();
                        writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, double {})",
                            indent, pf, so, fmt, fd).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, pf).ok();
                    }
                    Intrinsic::GetEnvInt => {
                        let name = self.emit_expr(out, &args[0], indent);
                        // Brief String value is i64 (ptrtoint of struct ptr).
                        // The struct has layout { ptr_to_data: i64, length: i64, data: [N x i8] }.
                        // Load the first field (ptr_to_data) to get the actual data pointer.
                        let sptr = format!("%gsr{}", self.txn_counter); self.txn_counter += 1;
                        let sp = format!("%gsp{}", self.txn_counter); self.txn_counter += 1;
                        let data_ptr = format!("%gdp{}", self.txn_counter); self.txn_counter += 1;
                        let str_ptr = format!("%gnp{}", self.txn_counter); self.txn_counter += 1;
                        let gv = format!("%gnv{}", self.txn_counter); self.txn_counter += 1;
                        let isnull = format!("%gnvl{}", self.txn_counter); self.txn_counter += 1;
                        let nul_l = format!("genv_nul{}", self.txn_counter); self.txn_counter += 1;
                        let ok_l = format!("genv_ok{}", self.txn_counter); self.txn_counter += 1;
                        let after_l = format!("genv_af{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sptr, name).ok();
                        writeln!(out, "{}{} = bitcast ptr {} to i64*", indent, sp, sptr).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, data_ptr, sp).ok();
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, str_ptr, data_ptr).ok();
                        writeln!(out, "{}{} = call ptr @getenv(ptr {})", indent, gv, str_ptr).ok();
                        writeln!(out, "{}{} = icmp eq ptr {}, null", indent, isnull, gv).ok();
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, isnull, nul_l, ok_l).ok();
                        writeln!(out, "{}{}:", indent, nul_l).ok();
                        writeln!(out, "{}  br label %{}", indent, after_l).ok();
                        writeln!(out, "{}{}:", indent, ok_l).ok();
                        let av = format!("%gav{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i64 @atol(ptr {})", indent, av, gv).ok();
                        writeln!(out, "{}  br label %{}", indent, after_l).ok();
                        writeln!(out, "{}{}:", indent, after_l).ok();
                        writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]",
                            indent, v, nul_l, av, ok_l).ok();
                    }
                    Intrinsic::Compile => {
                        // compile#() is compile-time only — should never reach LLVM backend
                        panic!("compile#() called at runtime — this is a compiler bug");
                    }
                    Intrinsic::MacroError => {
                        panic!("error#() called at runtime — this is a compiler bug");
                    }
                    Intrinsic::MacroWarn => {
                        panic!("warn#() called at runtime — this is a compiler bug");
                    }
                    Intrinsic::MacroGenSym => {
                        panic!("gensym#() called at runtime — this is a compiler bug");
                    }
                    Intrinsic::EmitFile => {
                        panic!("emit_file#() called at runtime — this is a compiler bug");
                    }
                    // GPU compute intrinsics (2026-06-18)
                    // CPU fallback: emit C runtime calls that return sensible defaults.
                    Intrinsic::GetGlobalId => {
                        let dim = if !args.is_empty() {
                            self.emit_expr(out, &args[0], indent).name.clone()
                        } else { "0".to_string() };
                        writeln!(out, "{}  {} = call i64 @__get_global_id(i32 {})", indent, v, dim).ok();
                    }
                    Intrinsic::GetLocalId => {
                        let dim = if !args.is_empty() {
                            self.emit_expr(out, &args[0], indent).name.clone()
                        } else { "0".to_string() };
                        writeln!(out, "{}  {} = call i64 @__get_local_id(i32 {})", indent, v, dim).ok();
                    }
                    Intrinsic::GetGroupId => {
                        let dim = if !args.is_empty() {
                            self.emit_expr(out, &args[0], indent).name.clone()
                        } else { "0".to_string() };
                        writeln!(out, "{}  {} = call i64 @__get_group_id(i32 {})", indent, v, dim).ok();
                    }
                    Intrinsic::GetNumGroups => {
                        let dim = if !args.is_empty() {
                            self.emit_expr(out, &args[0], indent).name.clone()
                        } else { "0".to_string() };
                        writeln!(out, "{}  {} = call i64 @__get_num_groups(i32 {})", indent, v, dim).ok();
                    }
                    Intrinsic::SubGroupBarrier => {
                        writeln!(out, "{}  call void @__barrier__()", indent).ok();
                        writeln!(out, "{}  {} = add i64 0, 1 ; barrier returns true", indent, v).ok();
                    }
                    // ===== D12: Random / Entropy (2026-06-19) =====
                    Intrinsic::Errno => {
                        writeln!(out, "{}  {} = call i64 @__errno__()", indent, v).ok();
                    }
                    Intrinsic::GetRandom => {
                        let buf = self.emit_expr(out, &args[0], indent);
                        let len = self.emit_expr(out, &args[1], indent);
                        let flags = self.emit_expr(out, &args[2], indent);
                        writeln!(out, "{}  {} = call i64 @__getrandom__(i64 {}, i64 {}, i64 {})",
                            indent, v, buf.name, len.name, flags.name).ok();
                    }
                    // ===== D13: System Info (2026-06-19) =====
                    Intrinsic::Uname => {
                        writeln!(out, "{}  {} = call i64 @__uname__()", indent, v).ok();
                    }
                    Intrinsic::PageSize => {
                        writeln!(out, "{}  {} = call i64 @sysconf(i32 29) ; _SC_PAGESIZE", indent, v).ok();
                    }
                    Intrinsic::CpuCount => {
                        writeln!(out, "{}  {} = call i64 @sysconf(i32 84) ; _SC_NPROCESSORS_ONLN", indent, v).ok();
                    }
                    Intrinsic::Hostname => {
                        writeln!(out, "{}  {} = call i64 @__hostname__()", indent, v).ok();
                    }
                    Intrinsic::StrError => {
                        let errnum = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__strerror__(i64 {})", indent, v, errnum.name).ok();
                    }
                    Intrinsic::StrSignal => {
                        let signum = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__strsignal__(i64 {})", indent, v, signum.name).ok();
                    }
                    Intrinsic::RealPath => {
                        let path = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__realpath__(i64 {})", indent, v, path.name).ok();
                    }
                    // ===== D14: Debugging (2026-06-19) =====
                    Intrinsic::Abort => {
                        writeln!(out, "{}  call void @abort()", indent).ok();
                        writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                        return TypedRegister { name: "_".to_string(), ty: Type::Void };
                    }
                    Intrinsic::Backtrace => {
                        writeln!(out, "{}  {} = call i64 @__backtrace__()", indent, v).ok();
                    }
                    // ===== D15: Scheduling (2026-06-19) =====
                    Intrinsic::SchedYield => {
                        let rv = format!("%sy{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @sched_yield()", indent, rv).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetPriority => {
                        let which = self.emit_expr(out, &args[0], indent);
                        let who = self.emit_expr(out, &args[1], indent);
                        let wi = format!("%gwi{}", self.txn_counter); self.txn_counter += 1;
                        let wo = format!("%gwo{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, wi, which.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, wo, who.name).ok();
                        let rv = format!("%gpr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @getpriority(i32 {}, i32 {})", indent, rv, wi, wo).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::SetPriority => {
                        let which = self.emit_expr(out, &args[0], indent);
                        let who = self.emit_expr(out, &args[1], indent);
                        let prio = self.emit_expr(out, &args[2], indent);
                        let wi = format!("%swi{}", self.txn_counter); self.txn_counter += 1;
                        let wo = format!("%swo{}", self.txn_counter); self.txn_counter += 1;
                        let wp = format!("%swp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, wi, which.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, wo, who.name).ok();
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, wp, prio.name).ok();
                        let rv = format!("%spr{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @setpriority(i32 {}, i32 {}, i32 {})", indent, rv, wi, wo, wp).ok();
                        writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
                    }
                    // ===== D16: User / Group (2026-06-19) =====
                    Intrinsic::GetUid => {
                        let rv = format!("%guid{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @getuid()", indent, rv).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetEUid => {
                        let rv = format!("%geuid{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @geteuid()", indent, rv).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetGid => {
                        let rv = format!("%ggid{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @getgid()", indent, rv).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetEGid => {
                        let rv = format!("%gegid{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = call i32 @getegid()", indent, rv).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
                    }
                    Intrinsic::GetPwUid => {
                        let uid = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__getpwuid__(i64 {})", indent, v, uid.name).ok();
                    }
                    Intrinsic::GetGrGid => {
                        let gid = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__getgrgid__(i64 {})", indent, v, gid.name).ok();
                    }
                    // ===== D17: Threading (2026-06-19) =====
                    Intrinsic::ThreadCreate => {
                        let fn_ptr = self.emit_expr(out, &args[0], indent);
                        let arg = self.emit_expr(out, &args[1], indent);
                        writeln!(out, "{}  {} = call i64 @__thread_create__(i64 {}, i64 {})",
                            indent, v, fn_ptr.name, arg.name).ok();
                    }
                    Intrinsic::ThreadJoin => {
                        let thread = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__thread_join__(i64 {})", indent, v, thread.name).ok();
                    }
                    Intrinsic::ThreadExit => {
                        let code = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  call void @__thread_exit__(i64 {})", indent, code.name).ok();
                        writeln!(out, "{}  {} = add i64 undef, 0 ; thread_exit is noreturn", indent, v).ok();
                    }
                    Intrinsic::MutexLock => {
                        let mptr = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__mutex_lock__(i64 {})", indent, v, mptr.name).ok();
                    }
                    Intrinsic::MutexUnlock => {
                        let mptr = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__mutex_unlock__(i64 {})", indent, v, mptr.name).ok();
                    }
                    Intrinsic::CondvarWait => {
                        let cptr = self.emit_expr(out, &args[0], indent);
                        let mptr = self.emit_expr(out, &args[1], indent);
                        writeln!(out, "{}  {} = call i64 @__condvar_wait__(i64 {}, i64 {})",
                            indent, v, cptr.name, mptr.name).ok();
                    }
                    Intrinsic::CondvarSignal => {
                        let cptr = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__condvar_signal__(i64 {})", indent, v, cptr.name).ok();
                    }
                    Intrinsic::CondvarBroadcast => {
                        let cptr = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__condvar_broadcast__(i64 {})", indent, v, cptr.name).ok();
                    }
                    // ===== D18: Resource Limits (2026-06-19) =====
                    Intrinsic::GetRlimit => {
                        let resource = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__getrlimit__(i64 {})", indent, v, resource.name).ok();
                    }
                    Intrinsic::SetRlimit => {
                        let resource = self.emit_expr(out, &args[0], indent);
                        let packed = self.emit_expr(out, &args[1], indent);
                        writeln!(out, "{}  {} = call i64 @__setrlimit__(i64 {}, i64 {})",
                            indent, v, resource.name, packed.name).ok();
                    }
                    // ===== Extra intrinsics (2026-06-19) =====
                    Intrinsic::MkStemp => {
                        let template = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__mkstemp__(i64 {})", indent, v, template.name).ok();
                    }
                    Intrinsic::MkDtemp => {
                        let template = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__mkdtemp__(i64 {})", indent, v, template.name).ok();
                    }
                    Intrinsic::DlOpen => {
                        let filename = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__dlopen__(i64 {})", indent, v, filename.name).ok();
                    }
                    Intrinsic::DlSym => {
                        let handle = self.emit_expr(out, &args[0], indent);
                        let symbol = self.emit_expr(out, &args[1], indent);
                        writeln!(out, "{}  {} = call i64 @__dlsym__(i64 {}, i64 {})",
                            indent, v, handle.name, symbol.name).ok();
                    }
                    Intrinsic::DlClose => {
                        let handle = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__dlclose__(i64 {})", indent, v, handle.name).ok();
                    }
                    Intrinsic::TtyName => {
                        let fd = self.emit_expr(out, &args[0], indent);
                        writeln!(out, "{}  {} = call i64 @__ttyname__(i64 {})", indent, v, fd.name).ok();
                    }
                    Intrinsic::Halt => {
                        // CPU halt: WFI on ARM, HLT on x86, WFI on RISC-V
                        // The target triple determines the instruction.
                        writeln!(out, "{}call void asm sideeffect \"wfi\", \"\"()", indent).ok();
                        writeln!(out, "{}{} = add i64 undef, 0 ; halt is void", indent, v).ok();
                    }
                    Intrinsic::UserDefined(name) => {
                        // Extract return and param type info before any mutable borrows.
                        // Clone the inop declaration to avoid borrow conflicts with emit_expr.
                        let inop_clone = self.inop_decls.get(name).cloned();
                        let ret_ty = inop_clone.as_ref().map_or("i64", |d| {
                            if d.outputs.iter().any(|t| {
                                let resolved = self.resolve_bild_type(t);
                                matches!(resolved, Type::Float)
                            }) {
                                "float"
                            } else {
                                "i64"
                            }
                        });
                        let param_tys: Vec<String> = inop_clone.as_ref().map_or_else(Vec::new, |d| {
                            d.params.iter().map(|(_, t)| {
                                let resolved = self.resolve_bild_type(t);
                                self.llvm_type(&resolved).to_string()
                            }).collect()
                        });
                        // Pre-evaluate all arguments before emitting the call,
                        // so argument computation code appears before the call instruction.
                        let mut arg_regs = Vec::new();
                        for arg in args {
                            let r = self.emit_expr(out, arg, indent);
                            arg_regs.push(r.name.clone());
                        }
                        // Detect multi-output: outputs.len() > 1
                        let has_state_access = inop_clone.as_ref()
                            .map(|d| d.has_state_access).unwrap_or(false);
                        let is_multi = inop_clone.as_ref()
                            .map(|d| d.outputs.len() > 1).unwrap_or(false);

                        if is_multi {
                            // Multi-output call: emit struct-return call, then
                            // extract values into a boxed tuple matching Brief's format.
                            let count = inop_clone.as_ref().map(|d| d.outputs.len()).unwrap_or(2);
                            let struct_ty = (0..count).map(|_| "i64").collect::<Vec<_>>().join(", ");
                            let struct_ty = format!("{{ {} }}", struct_ty);

                            // Emit the call
                            let call_reg = format!("%mc{}", self.txn_counter); self.txn_counter += 1;
                            write!(out, "{}{} = call {} @{}(", indent, call_reg, struct_ty, name).ok();
                            if has_state_access {
                                write!(out, " ptr %state").ok();
                            }
                            for (i, rn) in arg_regs.iter().enumerate() {
                                let native_ty = param_tys.get(i).map(|s| s.as_str()).unwrap_or("i64");
                                if has_state_access || i > 0 {
                                    write!(out, ", {} {}", native_ty, rn).ok();
                                } else {
                                    write!(out, "{} {}", native_ty, rn).ok();
                                }
                            }
                            writeln!(out, ")").ok();

                            // Allocate boxed tuple: [header_ptr, len, elem0, elem1, ...]
                            let total = count as i64 + 2;
                            let ai = format!("%mai{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, total).ok();
                            let dp_ptr = format!("%mdp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp_ptr, ai).ok();
                            let dp_val = format!("%mdv{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, dp_val, dp_ptr).ok();
                            let s0 = format!("%ms0{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, s0, ai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp_val, s0).ok();
                            let s1 = format!("%ms1{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, s1, ai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, count as i64, s1).ok();

                            for i in 0..count {
                                let ev = format!("%mev{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = extractvalue {} {}, {}", indent, ev, struct_ty, call_reg, i).ok();
                                let ep = format!("%mep{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, ai, (i as i64) + 2).ok();
                                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, ev, ep).ok();
                            }

                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
                        } else {
                            // Emit the call with native types for each parameter
                            write!(out, "{}{} = call {} @{}(", indent, v, ret_ty, name).ok();
                            if has_state_access {
                                write!(out, "ptr %state").ok();
                            }
                            for (i, rn) in arg_regs.iter().enumerate() {
                                let native_ty = param_tys.get(i).map(|s| s.as_str()).unwrap_or("i64");
                                if has_state_access || i > 0 {
                                    write!(out, ", {} {}", native_ty, rn).ok();
                                } else {
                                    write!(out, "{} {}", native_ty, rn).ok();
                                }
                            }
                            writeln!(out, ")").ok();
                        }
                    }
                }
            }
            // ── ListLiteral ──────────────────────────────────────
            Expr::ListLiteral(items) => {
                let n = items.len() as i64;
                let total = n + 2;
                let ai = format!("%lai{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, total).ok();
                let dp_ptr = format!("%ldp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp_ptr, ai).ok();
                let dp_val = format!("%ldv{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, dp_val, dp_ptr).ok();
                let s0 = format!("%ls0{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, s0, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp_val, s0).ok();
                let s1 = format!("%ls1{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, s1, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, n, s1).ok();
                for (i, item) in items.iter().enumerate() {
                    let iv = self.emit_expr(out, item, indent);
                    let adapted = self.adapt_to_i64(out, indent, &iv);
                    let ep = format!("%lep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, ai, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, adapted, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
            }
            // ── Tuple ───────────────────────────────────────────
            Expr::Tuple(items) => {
                let n = items.len() as i64;
                let total = n + 2;
                let ai = format!("%tai{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, total).ok();
                let dp_ptr = format!("%tdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp_ptr, ai).ok();
                let dp_val = format!("%tdv{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, dp_val, dp_ptr).ok();
                let s0 = format!("%ts0{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, s0, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp_val, s0).ok();
                let s1 = format!("%ts1{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, s1, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, n, s1).ok();
                for (i, item) in items.iter().enumerate() {
                    let iv = self.emit_expr(out, item, indent);
                    let adapted = self.adapt_to_i64(out, indent, &iv);
                    let ep = format!("%tep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, ai, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, adapted, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
            }
            // ── ListIndex ───────────────────────────────────────
            Expr::ListIndex(list, index) => {
                let list_val = self.emit_expr(out, list, indent);
                let idx_val = self.emit_expr(out, index, indent);
                let hp = format!("%xhp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_val.name).ok();
                let dp = format!("%xdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                let de = format!("%xde{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                let ep = format!("%xep{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, de, idx_val.name).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, ep).ok();
            }
            // ── Projection ──────────────────────────────────────
            Expr::Projection { source, target } => {
                let src_val = self.emit_expr(out, &*source, indent);
                // Phase 2: Check if this is a cached projection (Hot Dual path).
                let target_name = crate::analysis::transition_graph::projection_target_name(target);
                if let Some(tr) = self.try_cached_projection(out, source.as_ref(), &src_val, &target_name, indent) {
                    return tr;
                }
                // Phase 2: Check if the source type has a meld route for this projection target.
                if let Some(tr) = self.try_meld_projection(out, &src_val, &target_name, indent) {
                    return tr;
                }
                match target {
                    ProjectionTarget::Size => {
                        if matches!(source.as_ref(),
                            Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_))
                        {
                            writeln!(out, "{}{} = add i64 0, 1", indent, v).ok();
                        } else {
                            let hp = format!("%php{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                            let lp = format!("%plp{}", self.txn_counter); self.txn_counter += 1;
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
                                match self.struct_types.get(name) {
                                    Some(fields) => fields.len() as i64 * 8,
                                    None => {
                                        writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                                        writeln!(out, "{}{} = add i64 0, 0 ; bytes: unknown struct", indent, v).ok();
                                        return TypedRegister { name: v, ty: Type::Int };
                                    }
                                }
                            }
                            _ => {
                                writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                                writeln!(out, "{}{} = add i64 0, 0 ; bytes: unknown type", indent, v).ok();
                                return TypedRegister { name: v, ty: Type::Int };
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
                        let hp = format!("%pbhp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, hp).ok();
                    }
                    ProjectionTarget::Contains(expr) => {
                        // Linear search over list elements
                        let search_val = self.emit_expr(out, expr, indent);
                        let search_boxed = self.adapt_to_i64(out, indent, &search_val);
                        let hp = format!("%pchp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                        let lp = format!("%pclp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                        let len = format!("%pcln{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                        let dp = format!("%pcdp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, hp).ok();
                        // Emit a linear search loop
                        let e_l = format!("pc_entry{}", self.txn_counter); self.txn_counter += 1;
                        let h_l = format!("pc_hdr{}", self.txn_counter); self.txn_counter += 1;
                        let b_l = format!("pc_body{}", self.txn_counter); self.txn_counter += 1;
                        let f_l = format!("pc_found{}", self.txn_counter); self.txn_counter += 1;
                        let d_l = format!("pc_done{}", self.txn_counter); self.txn_counter += 1;
                        let i_r = format!("%pci{}", self.txn_counter); self.txn_counter += 1;
                        let c_r = format!("%pcc{}", self.txn_counter); self.txn_counter += 1;
                        let el_r = format!("%pce{}", self.txn_counter); self.txn_counter += 1;
                        let eq_r = format!("%pceq{}", self.txn_counter); self.txn_counter += 1;
                        let n_r = format!("%pcn{}", self.txn_counter); self.txn_counter += 1;
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
                        return TypedRegister { name: v, ty: Type::Bool };
                    }
                    ProjectionTarget::Range => {
                        // Return list length (same as Size) — Range = [0, len)
                        let hp = format!("%prhp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                        let lp = format!("%prlp{}", self.txn_counter); self.txn_counter += 1;
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
                        let key_val = self.emit_expr(out, expr, indent);
                        let key_boxed = self.adapt_to_i64(out, indent, &key_val);
                        writeln!(out, "{}{} = call i64 @__hashmap_get__(i64 {}, i64 {})", indent, v, src_val.name, key_boxed).ok();
                    }
                    ProjectionTarget::Elements => {
                        writeln!(out, "{}{} = add i64 0, {} ; elements", indent, v, src_val.name).ok();
                    }
                    ProjectionTarget::IsEmpty => {
                        let hp = format!("%ieh{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                        let lp = format!("%iel{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                        let len = format!("%ien{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                        writeln!(out, "{}{} = icmp eq i64 {}, 0", indent, v, len).ok();
                        writeln!(out, "{}{} = zext i1 {} to i64", indent, v, v).ok();
                    }
                    ProjectionTarget::UserDefinedWithArg(name, arg_expr) => {
                        // Phase 3.5: Fast-path for well-known operator projections
                        if let Some(tr) = self.try_projection_fast_path(out, &src_val, name.as_str(), arg_expr, indent, &v) {
                            return tr;
                        }
                        writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    }
                    ProjectionTarget::UserDefined(_) => {
                        writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    }
                    ProjectionTarget::BitRange(br) => {
                        // Extract bits via lshr + and
                        let (lo, hi) = match br {
                            crate::ast::BitRange::Single(i) => (*i, *i),
                            crate::ast::BitRange::Range(l, h) => (*l, *h),
                            crate::ast::BitRange::Any(w) => (0, *w - 1),
                        };
                        let width = hi - lo + 1;
                        let shifted = format!("%pbr{}", self.txn_counter); self.txn_counter += 1;
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
                let ai = format!("%sai{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, n).ok();
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    let fv = self.emit_expr(out, fval, indent);
                    let fp = format!("%sfp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, fp, ai, i as i64).ok();
                     let stored = if fv.ty == Type::Bool || fv.ty == Type::Char || fv.ty == Type::Float || fv.ty == Type::String {
                         self.adapt_to_i64(out, indent, &fv)
                     } else { fv.name.clone() };
                     writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, stored, fp).ok();
                 }
                 writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
                 return TypedRegister { name: v, ty: Type::Custom(name.clone()) };
             }
             // ── ObjectLiteral ───────────────────────────────────
            Expr::ObjectLiteral(fields) => {
                let n = fields.len() as i64;
                let ai = format!("%oai{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, n).ok();
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    let fv = self.emit_expr(out, fval, indent);
                    let fp = format!("%ofp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, fp, ai, i as i64).ok();
                    let stored = if fv.ty == Type::Bool || fv.ty == Type::Char || fv.ty == Type::Float || fv.ty == Type::String {
                        self.adapt_to_i64(out, indent, &fv)
                    } else { fv.name.clone() };
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, stored, fp).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
            }
            // ── FieldAccess ─────────────────────────────────────
            Expr::FieldAccess(obj, field) => {
                let obj_val = self.emit_expr(out, obj, indent);
                let hp = format!("%fahp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, obj_val.name).ok();
                let mut found_offset = false;
                let mut offset = 0i64;
                if let Expr::Identifier(name) = obj.as_ref() {
                    if let Some(Type::Custom(struct_name)) = self.let_binding_types.get(name) {
                        if let Some(fields) = self.struct_types.get(struct_name) {
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
                        if let Some(fields) = self.struct_types.get(struct_name) {
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
                    let fp = format!("%fafp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, fp, hp, offset).ok();
                    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, fp).ok();
                    // 2026-06-17: Return Float type for float fields so downstream
                    // code (emit_binop) correctly identifies them. String/Data fields
                    // remain Type::Int (stored boxed as i64 in struct).
                    let lookup_ty = || -> Option<Type> {
                        if let Expr::Identifier(name) = obj.as_ref() {
                            if let Some(Type::Custom(struct_name)) = self.let_binding_types.get(name) {
                                if let Some(fields) = self.struct_types.get(struct_name) {
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
                            if let Some(fields) = self.struct_types.get(struct_name) {
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
                        return TypedRegister { name: v, ty: ft };
                    }
                } else {
                    writeln!(out, "{}  call void @llvm.trap()", indent).ok();
                    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                }
            }
            // ── PatternMatch ────────────────────────────────────
            Expr::PatternMatch { value, variant, fields } => {
                let src_val = self.emit_expr(out, value, indent);
                let hp = format!("%php{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                let disc = format!("%pdisc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, disc, hp).ok();
                let expected = self.variant_disc.get(variant)
                    .map(|(_, d, _)| *d as i64)
                    .unwrap_or(0);
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, v, disc, expected).ok();
            }
            // ── MultiSlice ──────────────────────────────────────
            Expr::MultiSlice { value, ops } => {
                let src_val = self.emit_expr(out, value, indent);
                // Atomic value literals: coord returns self, stride/mask return 0
                let is_atomic_literal = matches!(value.as_ref(), Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_));
                if is_atomic_literal {
                    let has_coord = ops.iter().any(|op| matches!(op, BracketOp::Coord(_)));
                    let has_other = ops.iter().any(|op| matches!(op, BracketOp::Stride(_) | BracketOp::Mask(_)));
                    if has_coord && !has_other {
                        writeln!(out, "{}{} = add i64 0, {} ; atomic coord passthrough", indent, v, src_val.name).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 0, {} ; atomic multislice", indent, v, src_val.name).ok();
                    }
                    return TypedRegister { name: v, ty: src_val.ty };
                }
                // Non-atomic: process ops as a sequential pipeline.
                // Phase 1: apply all Coord ops (index/slice) to extract sublist/element.
                // Phase 2: apply Stride ops (step-by filter).
                // Phase 3: apply Mask ops (element-wise boolean filter).
                let mut result_reg = src_val.clone();
                let saved_bindings = self.let_bindings.clone();
                let mut reboxed = false; // true if result is a freshly-boxed list

                for op in ops {
                    match op {
                        BracketOp::Coord(SliceCoordinate::Index(idx_expr)) => {
                            // If the current result is a freshly-boxed list (not the
                            // original source), unbox it to get the data pointer.
                            let hp = if reboxed {
                                let rhp = format!("%mrhp{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, rhp, result_reg.name).ok();
                                rhp
                            } else {
                                let ihp = format!("%mihp{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, ihp, result_reg.name).ok();
                                ihp
                            };
                            let dp = format!("%mdp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mde{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let cv = self.emit_expr(out, idx_expr, indent);
                            let ep = format!("%mep{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, de, cv.name).ok();
                            let lv = format!("%mlv{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, lv, ep).ok();
                            result_reg = TypedRegister { name: lv, ty: Type::Int };
                            reboxed = false;
                        }
                        BracketOp::Coord(SliceCoordinate::Range { start, end }) => {
                            // Extract sub-range [start, end) into a new list
                            let hp = format!("%mrhp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, result_reg.name).ok();
                            let dp = format!("%mrdp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mrde{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let slp = format!("%mrlp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, hp).ok();
                            let src_len = format!("%mrsl{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, src_len, slp).ok();
                            // Start bound
                            let start_reg = start.as_ref().map(|s| self.emit_expr(out, s, indent));
                            let end_reg = end.as_ref().map(|e| self.emit_expr(out, e, indent));
                            let lo = format!("%mrlo{}", self.txn_counter); self.txn_counter += 1;
                            if let Some(s) = &start_reg {
                                writeln!(out, "{}{} = add i64 0, {}", indent, lo, s.name).ok();
                            } else {
                                writeln!(out, "{}{} = add i64 0, 0", indent, lo).ok();
                            }
                            let hi = format!("%mrhi{}", self.txn_counter); self.txn_counter += 1;
                            if let Some(e) = &end_reg {
                                writeln!(out, "{}{} = add i64 0, {}", indent, hi, e.name).ok();
                            } else {
                                writeln!(out, "{}{} = add i64 0, {}", indent, hi, src_len).ok();
                            }
                            let rcnt = format!("%mrcnt{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = sub i64 {}, {}", indent, rcnt, hi, lo).ok();
                            // Allocate new list
                            let rab = format!("%mrab{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = mul i64 {}, 8", indent, rab, rcnt).ok();
                            let rrm = self.emit_arena_alloc(out, indent, &rab);
                            let rai = format!("%mrai{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, rai, rrm).ok();
                            // Copy loop
                            let r_entry = format!("mr_entry{}", self.txn_counter); self.txn_counter += 1;
                            let r_hdr = format!("mr_hdr{}", self.txn_counter); self.txn_counter += 1;
                            let r_body = format!("mr_body{}", self.txn_counter); self.txn_counter += 1;
                            let r_done = format!("mr_done{}", self.txn_counter); self.txn_counter += 1;
                            let ri = format!("%mri{}", self.txn_counter); self.txn_counter += 1;
                            let rc = format!("%mrc{}", self.txn_counter); self.txn_counter += 1;
                            let rn = format!("%mrn{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}br label %{}", indent, r_entry).ok();
                            writeln!(out, "{}{}:", indent, r_entry).ok();
                            writeln!(out, "{}br label %{}", indent, r_hdr).ok();
                            writeln!(out, "{}{}:", indent, r_hdr).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, ri, r_entry, rn, r_body).ok();
                            writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, rc, ri, rcnt).ok();
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, rc, r_body, r_done).ok();
                            writeln!(out, "{}{}:", indent, r_body).ok();
                            let r_src = format!("%mrsrc{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = add i64 {}, {}", indent, r_src, lo, ri).ok();
                            let r_gep = format!("%mrgep{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, r_gep, de, r_src).ok();
                            let r_el = format!("%mrel{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, r_el, r_gep).ok();
                            let r_dst = format!("%mrdst{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, r_dst, rai, ri).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, r_el, r_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, rn, ri).ok();
                            writeln!(out, "{}br label %{}", indent, r_hdr).ok();
                            writeln!(out, "{}{}:", indent, r_done).ok();
                            // Store header (data_ptr, length)
                            let r_dpp = format!("%mrdpp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, r_dpp, rai).ok();
                            let r_dpv = format!("%mrdpv{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, r_dpv, r_dpp).ok();
                            let rs0 = format!("%mrs0{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, rs0, rai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, r_dpv, rs0).ok();
                            let rs1 = format!("%mrs1{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, rs1, rai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, rcnt, rs1).ok();
                            let rv = format!("%mrv{}", self.txn_counter); self.txn_counter += 1;
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
                            let sv = self.emit_expr(out, stride_expr, indent);
                            // Unbox current list
                            let hp = format!("%mshp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, result_reg.name).ok();
                            let dp = format!("%msdp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%msde{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let lp = format!("%mslp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                            let len = format!("%mslen{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, len, lp).ok();
                            // Allocate stride-filtered buffer
                            let sab = format!("%msab{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = mul i64 {}, 8", indent, sab, len).ok();
                            let srm = self.emit_arena_alloc(out, indent, &sab);
                            let sai = format!("%msai{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, sai, srm).ok();
                            // Loop: j = 0; k = 0; while j < len { copy[j]; j += stride; k++ }
                            let s_entry = format!("ms_entry{}", self.txn_counter); self.txn_counter += 1;
                            let s_hdr = format!("ms_hdr{}", self.txn_counter); self.txn_counter += 1;
                            let s_body = format!("ms_body{}", self.txn_counter); self.txn_counter += 1;
                            let s_done = format!("ms_done{}", self.txn_counter); self.txn_counter += 1;
                            let sj = format!("%msj{}", self.txn_counter); self.txn_counter += 1;
                            let sc = format!("%msc{}", self.txn_counter); self.txn_counter += 1;
                            let sn = format!("%msn{}", self.txn_counter); self.txn_counter += 1;
                            let sk = format!("%msk{}", self.txn_counter); self.txn_counter += 1;
                            let snk = format!("%msnk{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}br label %{}", indent, s_entry).ok();
                            writeln!(out, "{}{}:", indent, s_entry).ok();
                            writeln!(out, "{}br label %{}", indent, s_hdr).ok();
                            writeln!(out, "{}{}:", indent, s_hdr).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, sj, s_entry, sn, s_body).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, sk, s_entry, snk, s_body).ok();
                            writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, sc, sj, len).ok();
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, sc, s_body, s_done).ok();
                            writeln!(out, "{}{}:", indent, s_body).ok();
                            let s_gep = format!("%msgep{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, s_gep, de, sj).ok();
                            let s_el = format!("%msel{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, s_el, s_gep).ok();
                            let s_dst = format!("%msdst{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, s_dst, sai, sk).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, s_el, s_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, {}", indent, sn, sj, sv.name).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, snk, sk).ok();
                            writeln!(out, "{}br label %{}", indent, s_hdr).ok();
                            writeln!(out, "{}{}:", indent, s_done).ok();
                            // Store header
                            let s_dpp = format!("%msdpp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, s_dpp, sai).ok();
                            let s_dpv = format!("%msdpv{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, s_dpv, s_dpp).ok();
                            let ss0 = format!("%mss0{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, ss0, sai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, s_dpv, ss0).ok();
                            let ss1 = format!("%mss1{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ss1, sai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, sk, ss1).ok();
                            let sv_reg = format!("%msv{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, sv_reg, sai).ok();
                            result_reg = TypedRegister { name: sv_reg, ty: Type::Int };
                            reboxed = true;
                        }
                        BracketOp::Mask(mask_expr) => {
                            // Element-wise filter: evaluate mask for each element
                            let hp = format!("%mmhp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, result_reg.name).ok();
                            let dp = format!("%mmdp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mmde{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let lp = format!("%mmlp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                            let len = format!("%mmlen{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, len, lp).ok();
                            // Allocate mask-filtered buffer (max size = len)
                            let mab = format!("%mmab{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = mul i64 {}, 8", indent, mab, len).ok();
                            let mrm = self.emit_arena_alloc(out, indent, &mab);
                            let mai = format!("%mmai{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, mai, mrm).ok();
                            // Loop: j = 0; k = 0; while j < len
                            let m_entry = format!("mm_entry{}", self.txn_counter); self.txn_counter += 1;
                            let m_hdr = format!("mm_hdr{}", self.txn_counter); self.txn_counter += 1;
                            let m_body = format!("mm_body{}", self.txn_counter); self.txn_counter += 1;
                            let m_done = format!("mm_done{}", self.txn_counter); self.txn_counter += 1;
                            let mj = format!("%mmj{}", self.txn_counter); self.txn_counter += 1;
                            let mc = format!("%mmc{}", self.txn_counter); self.txn_counter += 1;
                            let mn = format!("%mmn{}", self.txn_counter); self.txn_counter += 1;
                            let mk = format!("%mmk{}", self.txn_counter); self.txn_counter += 1;
                            let mnk = format!("%mmnk{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}br label %{}", indent, m_entry).ok();
                            writeln!(out, "{}{}:", indent, m_entry).ok();
                            writeln!(out, "{}br label %{}", indent, m_hdr).ok();
                            writeln!(out, "{}{}:", indent, m_hdr).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, mj, m_entry, mn, m_body).ok();
                            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, mk, m_entry, mnk, m_body).ok();
                            writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, mc, mj, len).ok();
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, mc, m_body, m_done).ok();
                            writeln!(out, "{}{}:", indent, m_body).ok();
                            let m_gep = format!("%mmgep{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, m_gep, de, mj).ok();
                            let m_el = format!("%mmel{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, m_el, m_gep).ok();
                            // Bind _ to element, evaluate mask
                            self.let_bindings.insert("_".to_string(), m_el.clone());
                            let mask_r = self.emit_expr(out, mask_expr, indent);
                            let mask_b = self.as_bool_reg(out, indent, &mask_r);
                            let m_store_l = format!("mm_store{}", self.txn_counter); self.txn_counter += 1;
                            let m_skip_l = format!("mm_skip{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, mask_b, m_store_l, m_skip_l).ok();
                            writeln!(out, "{}{}:", indent, m_store_l).ok();
                            let m_dst = format!("%mmdst{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, m_dst, mai, mk).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_el, m_dst).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, mnk, mk).ok();
                            writeln!(out, "{}br label %{}", indent, m_skip_l).ok();
                            writeln!(out, "{}{}:", indent, m_skip_l).ok();
                            writeln!(out, "{}{} = add i64 {}, 1", indent, mn, mj).ok();
                            writeln!(out, "{}br label %{}", indent, m_hdr).ok();
                            writeln!(out, "{}{}:", indent, m_done).ok();
                            // Store header
                            let m_dpp = format!("%mmdpp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, m_dpp, mai).ok();
                            let m_dpv = format!("%mmdpv{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, m_dpv, m_dpp).ok();
                            let ms0 = format!("%mms0{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, ms0, mai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_dpv, ms0).ok();
                            let ms1 = format!("%mms1{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ms1, mai).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, mk, ms1).ok();
                            let mv_reg = format!("%mmv{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, mv_reg, mai).ok();
                            result_reg = TypedRegister { name: mv_reg, ty: Type::Int };
                            reboxed = true;
                        }
                    }
                }
                writeln!(out, "{}{} = add i64 0, {}", indent, v, result_reg.name).ok();
                self.let_bindings = saved_bindings;
            }
            // ── Match ───────────────────────────────────────────
            Expr::Match { value, arms } => {
                let saved_bindings = self.let_bindings.clone();
                let saved_types = self.let_binding_types.clone();
                let val = self.emit_expr(out, value, indent);
                let hp = format!("%mhp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, val.name).ok();
                let disc_reg = format!("%mdisc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, disc_reg, hp).ok();

                let mut variant_arms: Vec<(u64, &MatchArm)> = Vec::new();
                let mut wildcard_arm: Option<&MatchArm> = None;
                for arm in arms {
                    match &arm.pattern {
                        MatchPattern::Variant { name, .. } => {
                            if let Some(&(_, disc_val, _)) = self.variant_disc.get(name) {
                                variant_arms.push((disc_val, arm));
                            }
                        }
                        MatchPattern::Wildcard => { wildcard_arm = Some(arm); }
                        _ => {}
                    }
                }

                let default_label = format!("mdef{}", self.txn_counter); self.txn_counter += 1;
                let merge_label = format!("mmerge{}", self.txn_counter); self.txn_counter += 1;
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
                                self.let_bindings.insert(var_name.clone(), fv);
                            }
                        }
                    }
                    let body_val = self.emit_expr(out, &arm.body, indent);
                    writeln!(out, "{}{} = add i64 0, {} ; match arm", indent, v, body_val.name).ok();
                    writeln!(out, "{}br label %{}", indent, merge_label).ok();
                }

                writeln!(out, "{}{}:", indent, default_label).ok();
                if let Some(wildcard) = wildcard_arm {
                    let body_val = self.emit_expr(out, &wildcard.body, indent);
                    writeln!(out, "{}{} = add i64 0, {} ; match wildcard", indent, v, body_val.name).ok();
                    writeln!(out, "{}br label %{}", indent, merge_label).ok();
                } else {
                    writeln!(out, "{}unreachable", indent).ok();
                }
                writeln!(out, "{}{}:", indent, merge_label).ok();
                self.let_bindings = saved_bindings;
                self.let_binding_types = saved_types;
                let match_ty = if arms.iter().all(|a| matches!(a.body.as_ref(), Expr::String(_))) {
                    Type::String
                } else {
                    Type::Int
                };
                return TypedRegister { name: v, ty: match_ty };
            }
            // ── Slice ───────────────────────────────────────────
            Expr::Slice { value, start, end, stride, mask } => {
                let src_val = self.emit_expr(out, value, indent);
                // Atomic value literals: pass through (single element is itself)
                let is_atomic_literal = matches!(value.as_ref(), Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_));
                if is_atomic_literal {
                    writeln!(out, "{}{} = add i64 0, {} ; atomic slice passthrough", indent, v, src_val.name).ok();
                    return crate::backend::llvm::TypedRegister { name: v, ty: src_val.ty };
                }
                // List: pointer-based list access
                let hp = format!("%shp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                let dp = format!("%sdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                let de = format!("%sde{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                let src_len_reg = format!("%sln{}", self.txn_counter); self.txn_counter += 1;
                let slp = format!("%slp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, hp).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, src_len_reg, slp).ok();

                let start_reg = start.as_ref().map(|s| self.emit_expr(out, s, indent));
                let end_reg = end.as_ref().map(|e| self.emit_expr(out, e, indent));
                let stride_reg = stride.as_ref().map(|s| self.emit_expr(out, s, indent));
                // Compute raw range = end - start
                let raw_count = format!("%sraw{}", self.txn_counter); self.txn_counter += 1;
                if let (Some(s), Some(e)) = (&start_reg, &end_reg) {
                    writeln!(out, "{}{} = sub i64 {}, {}", indent, raw_count, e.name, s.name).ok();
                } else {
                    writeln!(out, "{}{} = add i64 0, {}", indent, raw_count, src_len_reg).ok();
                }
                // Compute effective count with stride: ceil(raw_count / stride)
                let count_reg = format!("%scnt{}", self.txn_counter); self.txn_counter += 1;
                if let Some(str) = &stride_reg {
                    let adj = format!("%sadj{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = add i64 {}, -1", indent, adj, raw_count).ok();
                    let div = format!("%sdiv{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = udiv i64 {}, {}", indent, div, adj, str.name).ok();
                    writeln!(out, "{}{} = add i64 {}, 1", indent, count_reg, div).ok();
                } else {
                    writeln!(out, "{}{} = add i64 0, {}", indent, count_reg, raw_count).ok();
                }

                // Why malloc for slice results: slice produces a new list whose size
                // is only known at runtime (depends on start, end, stride). Stack
                // allocation is impossible because the size varies per execution.
                // Allocate new list header (avoids invalid dynamic alloca in non-entry block)
                let ab = format!("%sab{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, ab, count_reg).ok();
                let rm = self.emit_arena_alloc(out, indent, &ab);
                let ai = format!("%sai{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, ai, rm).ok();

                let entry_label = format!("s_entry{}", self.txn_counter); self.txn_counter += 1;
                let header_label = format!("s_hdr{}", self.txn_counter); self.txn_counter += 1;
                let body_label = format!("s_body{}", self.txn_counter); self.txn_counter += 1;
                let done_label = format!("s_done{}", self.txn_counter); self.txn_counter += 1;
                let i_reg = format!("%si{}", self.txn_counter); self.txn_counter += 1;
                let cond_reg = format!("%scond{}", self.txn_counter); self.txn_counter += 1;
                let next_reg = format!("%snext{}", self.txn_counter); self.txn_counter += 1;

                writeln!(out, "{}br label %{}", indent, entry_label).ok();
                writeln!(out, "{}{}:", indent, entry_label).ok();
                writeln!(out, "{}br label %{}", indent, header_label).ok();
                writeln!(out, "{}{}:", indent, header_label).ok();
                writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, i_reg, entry_label, next_reg, body_label).ok();
                writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cond_reg, i_reg, count_reg).ok();
                writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cond_reg, body_label, done_label).ok();
                writeln!(out, "{}{}:", indent, body_label).ok();
                // Copy element: src[start + i*stride]
                let src_idx = format!("%ssi{}", self.txn_counter); self.txn_counter += 1;
                if let Some(s) = &start_reg {
                    if let Some(str) = &stride_reg {
                        let si_stride = format!("%sist{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, {}", indent, si_stride, i_reg, str.name).ok();
                        writeln!(out, "{}{} = add i64 {}, {}", indent, src_idx, s.name, si_stride).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 {}, {}", indent, src_idx, s.name, i_reg).ok();
                    }
                } else {
                    if let Some(str) = &stride_reg {
                        let si_stride = format!("%sist{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = mul i64 {}, {}", indent, si_stride, i_reg, str.name).ok();
                        writeln!(out, "{}{} = add i64 0, {}", indent, src_idx, si_stride).ok();
                    } else {
                        writeln!(out, "{}{} = add i64 0, {}", indent, src_idx, i_reg).ok();
                    }
                }
                let src_ep = format!("%ssep{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, src_ep, de, src_idx).ok();
                let elem = format!("%selem{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, elem, src_ep).ok();
                // Store to dest[2 + i]
                let dst_idx = format!("%sdi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 2", indent, dst_idx, i_reg).ok();
                let dst_ep = format!("%sdep{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, dst_ep, ai, dst_idx).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem, dst_ep).ok();
                writeln!(out, "{}{} = add i64 {}, 1", indent, next_reg, i_reg).ok();
                writeln!(out, "{}br label %{}", indent, header_label).ok();
                writeln!(out, "{}{}:", indent, done_label).ok();
                // Store data_ptr and length in the strided-result header
                let dp_ptr = format!("%sdp2{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp_ptr, ai).ok();
                let dp_val = format!("%sdv2{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, dp_val, dp_ptr).ok();
                let s0 = format!("%ss0{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, s0, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp_val, s0).ok();
                let s1 = format!("%ss1{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, s1, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, count_reg, s1).ok();

                // ── Mask filter (second pass) ──
                // If a mask expression is present, walk the strided result and
                // keep only elements where mask(_, elem) evaluates to true.
                if let Some(mask_expr) = mask {
                    let saved_bindings = self.let_bindings.clone();
                    let old_count = count_reg;
                    let old_ai = ai;
                    let m_entry = format!("sm_entry{}", self.txn_counter); self.txn_counter += 1;
                    let m_hdr = format!("sm_hdr{}", self.txn_counter); self.txn_counter += 1;
                    let m_body = format!("sm_body{}", self.txn_counter); self.txn_counter += 1;
                    let m_done = format!("sm_done{}", self.txn_counter); self.txn_counter += 1;
                    let m_j = format!("%smj{}", self.txn_counter); self.txn_counter += 1;
                    let m_cond = format!("%smcond{}", self.txn_counter); self.txn_counter += 1;
                    let m_next = format!("%smnext{}", self.txn_counter); self.txn_counter += 1;
                    let m_k = format!("%smk{}", self.txn_counter); self.txn_counter += 1;
                    // Allocate max-size filtered buffer
                    let m_ab = format!("%smab{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = mul i64 {}, 8", indent, m_ab, old_count).ok();
                    let m_rm = self.emit_arena_alloc(out, indent, &m_ab);
                    let m_ai = format!("%smai{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, m_ai, m_rm).ok();
                    let zero_reg = format!("%smz{}", self.txn_counter); self.txn_counter += 1;
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
                    let m_gep = format!("%smgep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, m_gep, old_ai, m_j).ok();
                    let m_elem = format!("%smelem{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, m_elem, m_gep).ok();
                    // Bind _ to element, evaluate mask
                    self.let_bindings.insert("_".to_string(), m_elem.clone());
                    let mask_reg = self.emit_expr(out, mask_expr, indent);
                    let mask_bool = self.as_bool_reg(out, indent, &mask_reg);
                    writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, mask_bool, m_done, m_hdr);
                    // If mask true, append to filtered buffer
                    // (true branch already jumps to m_done — use a separate skip label)
                    let m_store = format!("sm_store{}", self.txn_counter); self.txn_counter += 1;
                    let m_next_label = format!("sm_next{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{}:", indent, m_store).ok();
                    let m_dst = format!("%smdst{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, m_dst, m_ai, m_k).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_elem, m_dst).ok();
                    writeln!(out, "{}{} = add i64 {}, 1", indent, m_next, m_k).ok();
                    writeln!(out, "{}br label %{}", indent, m_next_label).ok();
                    writeln!(out, "{}{}:", indent, m_next_label).ok();
                    writeln!(out, "{}{} = add i64 {}, 1", indent, m_next, m_j).ok();
                    writeln!(out, "{}br label %{}", indent, m_hdr).ok();

                    writeln!(out, "{}{}:", indent, m_done).ok();
                    // Store filtered header
                    let m_dp_ptr = format!("%smdp2{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, m_dp_ptr, m_ai).ok();
                    let m_dp_val = format!("%smdv2{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, m_dp_val, m_dp_ptr).ok();
                    let ms0 = format!("%sms0{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, ms0, m_ai).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_dp_val, ms0).ok();
                    let ms1 = format!("%sms1{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ms1, m_ai).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, m_k, ms1).ok();
                    writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, m_ai).ok();
                    self.let_bindings = saved_bindings;
                } else {
                    writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
                }
            }
            // — Subtype projection (e.g. list :> Size) —
            Expr::SubtypeProjection { source, .. } => {
                let src = self.emit_expr(out, source, indent);
                let hp = format!("%shp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src.name).ok();
                let slp = format!("%slp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, hp).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, slp).ok();
                return TypedRegister { name: v, ty: Type::Int };
            }
            Expr::SubtypeProjectionExpr(e) => {
                return self.emit_expr(out, &Expr::SubtypeProjection {
                    source: e.source.clone(),
                    ops: e.ops.clone(),
                }, indent);
            }
            Expr::IsType(expr, target) => {
                let _ = self.emit_expr(out, expr, indent);
                let comment = match target {
                    crate::ast::IsTarget::Type(_) => "is type",
                    crate::ast::IsTarget::Variant(v) => v,
                };
                writeln!(out, "{}{} = add i64 0, 1 ; {} (compile-time)", indent, v, comment).ok();
                return TypedRegister { name: v, ty: Type::Bool };
            }
            Expr::FromCheck(expr, _ty) => {
                let _ = self.emit_expr(out, expr, indent);
                writeln!(out, "{}{} = add i64 0, 1 ; from (compile-time)", indent, v).ok();
                return TypedRegister { name: v, ty: Type::Bool };
            }
            Expr::Like(l, r) => {
                return self.emit_fcmp(out, indent, l, r, "oeq");
            }
            Expr::Block(stmts, last) => {
                for s in stmts {
                    self.emit_stmt(out, s, indent);
                    if self.terminated {
                        return TypedRegister { name: "_".to_string(), ty: Type::Void };
                    }
                }
                return self.emit_expr(out, last, indent);
            }
            Expr::MapLiteral(items) => {
                let n = items.len() as i64;
                let alloc_slots = n + 2;
                // Why malloc/arena for map/set literals: the literal may have a large
                // number of entries (hundreds). Stack via alloca would risk overflow.
                // Arena handles this with bump alloc when in a loop context.
                let map_alloc_size = format!("%mas{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, map_alloc_size, (alloc_slots * 8 + 8)).ok();
                let ai = self.emit_arena_alloc(out, indent, &map_alloc_size);
                let hp = format!("%mhp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, hp, ai).ok();
                let base = format!("%mba{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, ai).ok();
                let dp = format!("%mdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, hp).ok();
                let ml1 = format!("%mml1{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ml1, hp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, n, ml1).ok();
                // Store values (keys are compile-time in the source map literal)
                for (i, (_key, val)) in items.iter().enumerate() {
                    let kv = self.emit_expr(out, val, indent);
                    let kvs = self.adapt_to_i64(out, indent, &kv);
                    let ep = format!("%mep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, kvs, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ai).ok();
                return TypedRegister { name: v, ty: Type::Int };
            }
            Expr::SetLiteral(items) => {
                let n = items.len() as i64;
                let set_alloc_size = format!("%sas{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, set_alloc_size, (n + 2) * 8 + 8).ok();
                let ai = self.emit_arena_alloc(out, indent, &set_alloc_size);
                let hp = format!("%shp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, hp, ai).ok();
                let base = format!("%sba{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, ai).ok();
                let dp = format!("%sdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, hp).ok();
                let sl1 = format!("%ssl1{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, sl1, hp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, n, sl1).ok();
                for (i, item) in items.iter().enumerate() {
                    let iv = self.emit_expr(out, item, indent);
                    let ivs = self.adapt_to_i64(out, indent, &iv);
                    let ep = format!("%sep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, hp, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ivs, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ai).ok();
                return TypedRegister { name: v, ty: Type::Int };
            }
            // Why free+malloc+memcpy instead of realloc: Brief collections have
            // immutable-value semantics — `<-` produces a new list, the old one is
            // dead (no shared refs). realloc doesn't help (we still memcpy to make
            // room) and the old→new ptr mapping adds complexity. The free+malloc
            // pattern makes allocation visible to LLVM's malloc optimization passes.
            Expr::ArrowMut { dir: ArrowDir::Push, target, index: _, value: Some(val) } => {
                let list_val = self.emit_expr(out, target, indent);
                let elem_val = self.emit_expr(out, val, indent);
                let list_boxed = self.adapt_to_i64(out, indent, &list_val);
                let elem_boxed = self.adapt_to_i64(out, indent, &elem_val);
                // Check InsertAt strategy for this target
                let prepend = self.check_insert_strategy(target).map_or(false,
                    |s| s == crate::type_universe::InsertStrategy::Prepend);
                // Unbox list header: inttoptr i64 to i64*
                let hp = format!("%ahp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                // Read current length from header slot 1
                let lp = format!("%alp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let old_len = format!("%aol{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, old_len, lp).ok();
                // Phase 2 fast path: if preallocated capacity exists and
                // length < capacity, write directly without alloc/memcpy.
                // Only works for append — prepend requires element shifting
                // which the fast path doesn't support. The slow_l label is
                // emitted here (always) so the branch target exists even
                // when the fast path returns early.
                let slow_l = format!("push_slow_{}", self.txn_counter); self.txn_counter += 1;
                if !prepend {
                if let Expr::OwnedRef(field_name) = target.as_ref() {
                    if let Some((cap_reg, buf_i64)) = self.field_prealloc_info.get(field_name.as_str()).cloned() {
                        let cap_check = format!("%acap{}", self.txn_counter);
                        self.txn_counter += 1;
                        writeln!(out, "{}{} = icmp ult i64 {}, {}", indent, cap_check, old_len, cap_reg).ok();
                        let fast_l = format!("push_fast_{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cap_check, fast_l, slow_l).ok();
                        // Fast path: write element at header[2 + old_len], increment length.
                        // Uses buf_i64 (preallocated i64* buffer from prealloc_info)
                        // rather than hp (which alias the same memory but may be stale
                        // after the first iteration resets state via the normal store path).
                        writeln!(out, "{}{}:", indent, fast_l).ok();
                        let el_off = format!("%apfo{}", self.txn_counter);
                        self.txn_counter += 1;
                        writeln!(out, "{}{} = add i64 {}, 2", indent, el_off, old_len).ok();
                        let el_gep = format!("%apfg{}", self.txn_counter);
                        self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, el_gep, buf_i64, el_off).ok();
                        let new_len_fast = format!("%apfn{}", self.txn_counter);
                        self.txn_counter += 1;
                        writeln!(out, "{}{} = add i64 {}, 1", indent, new_len_fast, old_len).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem_boxed, el_gep).ok();
                        // Update length in header slot 1 of the preallocated buffer
                        let len_gep = format!("%apfl{}", self.txn_counter);
                        self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, len_gep, buf_i64).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len_fast, len_gep).ok();
                        // Store back to state (buffer pointer unchanged — we modified
                        // the preallocated buffer in-place, no new allocation).
                        let store_idx = self.field_index_map[field_name];
                        let ap = format!("%aapf{}", self.txn_counter);
                        self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, store_idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&self.field_types[store_idx]);
                        let base_fast = format!("%apfb{}", self.txn_counter);
                        self.txn_counter += 1;
                        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, base_fast, buf_i64).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base_fast, ap, tn).ok();
                        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, buf_i64).ok();
                        return TypedRegister { name: v, ty: Type::Int };
                    }
                }
                }
                writeln!(out, "{}{}:", indent, slow_l).ok();
                // Allocate: when inside an arena scope (loop/tick), use bump
                // alloc (no free — arena resets at scope exit). Outside a
                // scope, fall back to per-operation malloc via emit_arena_alloc.
                let new_cnt = format!("%anc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 3", indent, new_cnt, old_len).ok();
                let alloc_bytes = format!("%aab{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
                let new_buf = self.emit_arena_alloc(out, indent, &alloc_bytes);
                // Free old buffer: when arena is active, the arena owns all
                // memory — no per-operation free needed. When arena is inactive
                // (standalone call), free the old buffer normally.
                if self.arena_slots.is_none() {
                    let old_ptr = format!("%aop{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
                }
                // Set header: data_ptr at slot 0, new length at slot 1
                let new_hp = format!("%anh{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
                let base = format!("%aba{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
                let dp = format!("%adp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, dp, new_hp).ok();
                let nlp = format!("%anp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
                let new_len = format!("%anl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, new_len, old_len).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
                // Copy old elements: for prepend, shift right by 1; for append, same position
                let old_dp = format!("%aod{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, old_dp, hp).ok();
                let copy_dst = if prepend {
                    // Prepend: copy to position 1 (one slot after base)
                    let cd = format!("%acd{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 3", indent, cd, new_hp).ok();
                    cd
                } else {
                    // Append: copy to position old_len (same position as before)
                    let cd = format!("%acd{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, cd, new_hp).ok();
                    cd
                };
                let copy_bytes = format!("%acb{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, copy_bytes, old_len).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, copy_dst, old_dp, copy_bytes).ok();
                // Store new element at position 0 for prepend, or old_len for append
                let ne_ptr = format!("%aep{}", self.txn_counter); self.txn_counter += 1;
                let new_elem_pos = if prepend { "2".to_string() } else { old_len.clone() };
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ne_ptr, new_hp, new_elem_pos).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, elem_boxed, ne_ptr).ok();
                // Store new list handle back to state field if target is OwnedRef
                if let Expr::OwnedRef(field_name) = target.as_ref() {
                    if let Some(&idx) = self.field_index_map.get(field_name) {
                        let ap = format!("%aap{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&self.field_types[idx]);
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
                    } else if let Some(slot) = self.param_slots.get(field_name).cloned() {
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
                    }
                }
                // Return new list handle (ptrtoint of new buffer)
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, new_buf).ok();
                return TypedRegister { name: v, ty: Type::Int };
            }
            // Why free+malloc+memcpy for pop: same semantics as push — the old
            // buffer is dead after the operation. Pop removes one element but
            // we still allocate a fresh buffer of len-1. An arena allocator
            // (planned) would replace the free+malloc with a bump pointer reset.
            Expr::ArrowMut { dir: ArrowDir::Pop, target, index, value: None } => {
                let list_val = self.emit_expr(out, target, indent);
                let list_boxed = self.adapt_to_i64(out, indent, &list_val);
                // Unbox list header
                let hp = format!("%php{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                // Read length
                let lp = format!("%plp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let len = format!("%pln{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                // Compute target index: len - 1 (pop from end) or expression value
                let pop_idx = format!("%ppi{}", self.txn_counter); self.txn_counter += 1;
                match index.as_ref() {
                    Expr::Term => {
                        writeln!(out, "{}{} = add i64 {}, -1", indent, pop_idx, len).ok();
                    }
                    other => {
                        let idx_val = self.emit_expr(out, other, indent);
                        let idx_boxed = self.adapt_to_i64(out, indent, &idx_val);
                        writeln!(out, "{}{} = add i64 {}, 0", indent, pop_idx, idx_boxed).ok();
                    }
                }
                // Load popped element from data_ptr[pop_idx]
                let dp = format!("%pdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, hp).ok();
                let ep = format!("%pep{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, dp, pop_idx).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, ep).ok();
                let popped = v.clone();
                // Free old buffer: arena-active skips per-op free; standalone frees
                if self.arena_slots.is_none() {
                    let old_ptr = format!("%pop{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
                }
                // Allocate new buffer: (len + 1) * 8 (2 header + len - 1 elements)
                let new_cnt = format!("%pnc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, new_cnt, len).ok();
                let alloc_bytes = format!("%pab{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
                let new_buf = self.emit_arena_alloc(out, indent, &alloc_bytes);
                // Set header
                let new_hp = format!("%pnh{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
                let base = format!("%pba{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
                let new_dp_val = format!("%pnd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, new_dp_val, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_dp_val, new_hp).ok();
                let nlp = format!("%pnp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
                let new_len = format!("%pnl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, -1", indent, new_len, len).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
                // Copy elements before pop_idx
                let ndp = format!("%pndp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
                let bef_bytes = format!("%pbb{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, bef_bytes, pop_idx).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, ndp, dp, bef_bytes).ok();
                // Copy elements after pop_idx
                let after_off = format!("%pao{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, after_off, pop_idx).ok();
                let aft_src = format!("%pas{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_src, dp, after_off).ok();
                let aft_dst = format!("%pad{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_dst, ndp, pop_idx).ok();
                let aft_cnt = format!("%pac{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = sub i64 {}, {}", indent, aft_cnt, new_len, pop_idx).ok();
                let aft_bytes = format!("%pab2{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, aft_bytes, aft_cnt).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, aft_dst, aft_src, aft_bytes).ok();
                // Store updated list back
                if let Expr::OwnedRef(field_name) = target.as_ref() {
                    if let Some(&idx) = self.field_index_map.get(field_name) {
                        let ap = format!("%pap{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&self.field_types[idx]);
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
                    } else if let Some(slot) = self.param_slots.get(field_name).cloned() {
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
                    }
                }
                return TypedRegister { name: popped, ty: Type::Int };
            }
            Expr::ArrowDiscard { target, index } => {
                let list_val = self.emit_expr(out, target, indent);
                let list_boxed = self.adapt_to_i64(out, indent, &list_val);
                // Unbox list header, read length
                let hp = format!("%dhp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                let lp = format!("%dlp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let len = format!("%dln{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                // Compute discard index
                let discard_idx = format!("%ddi{}", self.txn_counter); self.txn_counter += 1;
                if matches!(index.as_ref(), Expr::Term) {
                    writeln!(out, "{}{} = add i64 {}, -1", indent, discard_idx, len).ok();
                } else {
                    let iv = self.emit_expr(out, index, indent);
                    let ib = self.adapt_to_i64(out, indent, &iv);
                    writeln!(out, "{}{} = add i64 {}, 0", indent, discard_idx, ib).ok();
                }
                // Free old buffer: arena-active skips per-op free; standalone frees
                if self.arena_slots.is_none() {
                    let old_ptr = format!("%dop{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, old_ptr, list_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, old_ptr).ok();
                }
                // Allocate new buffer: (len + 1) slots (2 header + len - 1 elements)
                let new_cnt = format!("%dnc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, new_cnt, len).ok();
                let alloc_bytes = format!("%dab{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, new_cnt).ok();
                let new_buf = self.emit_arena_alloc(out, indent, &alloc_bytes);
                // Set header
                let new_hp = format!("%dnh{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
                let base = format!("%dba{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, new_buf).ok();
                let ndv = format!("%dnd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, ndv, base).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ndv, new_hp).ok();
                let nlp = format!("%dnp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, nlp, new_hp).ok();
                let new_len = format!("%dnl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, -1", indent, new_len, len).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, new_len, nlp).ok();
                // Copy before discard_idx
                let dp = format!("%ddp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, hp).ok();
                let ndp = format!("%dndp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
                let bef_bytes = format!("%dbb{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, bef_bytes, discard_idx).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, ndp, dp, bef_bytes).ok();
                // Copy after discard_idx
                let after_off = format!("%dao{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 1", indent, after_off, discard_idx).ok();
                let aft_src = format!("%das{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_src, dp, after_off).ok();
                let aft_dst = format!("%dad{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, aft_dst, ndp, discard_idx).ok();
                let aft_cnt = format!("%dac{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = sub i64 {}, {}", indent, aft_cnt, new_len, discard_idx).ok();
                let aft_bytes = format!("%dab2{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, aft_bytes, aft_cnt).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, aft_dst, aft_src, aft_bytes).ok();
                // Store updated list back
                if let Expr::OwnedRef(field_name) = target.as_ref() {
                    if let Some(&idx) = self.field_index_map.get(field_name) {
                        let ap = format!("%dap{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&self.field_types[idx]);
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, base, ap, tn).ok();
                    } else if let Some(slot) = self.param_slots.get(field_name).cloned() {
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, base, slot).ok();
                    }
                }
                writeln!(out, "{}{} = add i64 0, {} ; discard", indent, v, base).ok();
                return TypedRegister { name: v, ty: Type::Int };
            }
            // ArrowTransfer moves ALL elements from source to destination.
            // Both old buffers are freed; a new combined buffer is allocated.
            // The source list becomes empty (2-slot header with data_ptr=null, len=0).
            // This is the most allocation-heavy arrow op — the arena plan (Phase 1)
            // benefits transfer the most.
            Expr::ArrowTransfer { dest, source, filter: _ } => {
                // Unfiltered: move all elements from source to dest
                let dest_val = self.emit_expr(out, dest, indent);
                let src_val = self.emit_expr(out, source, indent);
                let dest_boxed = self.adapt_to_i64(out, indent, &dest_val);
                let src_boxed = self.adapt_to_i64(out, indent, &src_val);
                let dhp = format!("%tdh{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, dhp, dest_boxed).ok();
                let shp = format!("%tsh{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, shp, src_boxed).ok();
                // Read lengths
                let dlp = format!("%tdl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, dlp, dhp).ok();
                let dlen = format!("%tdn{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, dlen, dlp).ok();
                let slp = format!("%tsl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, slp, shp).ok();
                let slen = format!("%tsn{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, slen, slp).ok();
                // Total = dest_len + src_len
                let total = format!("%ttl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, {}", indent, total, dlen, slen).ok();
                // Free old buffers: arena skips per-op free; standalone frees
                if self.arena_slots.is_none() {
                    let dold = format!("%tdop{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, dold, dest_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, dold).ok();
                    let sold = format!("%tsop{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, sold, src_boxed).ok();
                    writeln!(out, "{}call void @free(i8* {})", indent, sold).ok();
                }
                // Allocate new dest buffer: (total + 2) * 8
                let alloc_slots = format!("%tas{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 2", indent, alloc_slots, total).ok();
                let alloc_bytes = format!("%tab{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, alloc_bytes, alloc_slots).ok();
                let new_buf = self.emit_arena_alloc(out, indent, &alloc_bytes);
                // Set dest header
                let new_hp = format!("%tnh{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, new_hp, new_buf).ok();
                let dbase = format!("%tdb{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, dbase, new_buf).ok();
                let ndv = format!("%tnd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 16", indent, ndv, dbase).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, ndv, new_hp).ok();
                let tnlp = format!("%tnl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, tnlp, new_hp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, total, tnlp).ok();
                // Copy dest elements
                let ddp = format!("%tddp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ddp, dhp).ok();
                let ndp = format!("%tndp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, ndp, new_hp).ok();
                let dbytes = format!("%tdb2{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, dbytes, dlen).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, ndp, ddp, dbytes).ok();
                // Copy source elements after dest
                let sdp = format!("%tsdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, sdp, shp).ok();
                let src_off = format!("%tso{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, src_off, ndp, dlen).ok();
                let sbytes = format!("%tsb{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = mul i64 {}, 8", indent, sbytes, slen).ok();
                writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)",
                    indent, src_off, sdp, sbytes).ok();
                // Store dest back
                if let Expr::OwnedRef(field_name) = dest.as_ref() {
                    if let Some(&idx) = self.field_index_map.get(field_name) {
                        let ap = format!("%tap{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&self.field_types[idx]);
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, dbase, ap, tn).ok();
                    }
                }
                // Store source (empty) back
                if let Expr::OwnedRef(field_name) = source.as_ref() {
                    if let Some(&idx) = self.field_index_map.get(field_name) {
                        let ap = format!("%sap{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, ap, idx).ok();
                        let tn = crate::backend::llvm::tbaa_node(&self.field_types[idx]);
                        // Allocate new empty list for source
                        let ebuf = self.emit_arena_alloc(out, indent, "16");
                        let ehp = format!("%seh{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, ehp, ebuf).ok();
                        let ebase = format!("%seb2{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, ebase, ebuf).ok();
                        let edv = format!("%sed{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = add i64 {}, 16", indent, edv, ebase).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, edv, ehp).ok();
                        let elp = format!("%sel{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, elp, ehp).ok();
                        writeln!(out, "{}store i64 0, i64* {}, align 8, !tbaa !1", indent, elp).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !{}", indent, ebase, ap, tn).ok();
                    }
                }
                writeln!(out, "{}{} = add i64 0, {} ; transfer", indent, v, dbase).ok();
                return TypedRegister { name: v, ty: Type::Int };
            }
            Expr::Cast(inner, target_ty) => {
                let inner_val = self.emit_expr(out, inner, indent);
                let cv = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                self.emit_cast_convert(out, indent, &cv, &inner_val.name, Some(inner_val.ty), target_ty);
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
                    _ => { writeln!(out, "{}call void @llvm.trap()", indent).ok(); writeln!(out, "{}unreachable", indent).ok(); return TypedRegister { name: v, ty: Type::Int }; }
                };
                let cell = match self.cell_defs.get(&callee_name) {
                    Some(c) => c.clone(),
                    None => { writeln!(out, "{}call void @llvm.trap()", indent).ok(); writeln!(out, "{}unreachable", indent).ok(); return TypedRegister { name: v, ty: Type::Int }; }
                };

                // 1. Store input args to prefixed parameter fields
                for (i, (param_name, _param_ty)) in cell.parameters.iter().enumerate() {
                    if i < args.len() {
                        let arg_reg = self.emit_expr(out, &args[i], indent);
                        let prefixed = format!("cell${}${}", callee_name, param_name);
                        if let Some(&idx) = self.field_index_map.get(&prefixed) {
                            let ll_ty = self.field_types[idx].clone();
                            let gep = format!("%csp_{}_{}", &callee_name, &param_name);
                            writeln!(out, "{}{} = getelementptr %State, ptr {}, i32 0, i32 {}",
                                indent, gep, self.state_reg_name, idx).ok();
                            let adapted = self.adapt_to_i64(out, indent, &arg_reg);
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
                let loop_h = format!(".celloop_{}", self.txn_counter);
                let done_l = format!(".celldone_{}", self.txn_counter);
                let any_fired = format!("%cany_{}", self.txn_counter);
                self.txn_counter += 1;

                // Alloca for any_fired flag (initialized to false)
                writeln!(out, "{}{} = alloca i8, align 1", indent, any_fired).ok();
                writeln!(out, "{}store i8 0, ptr {}, align 1", indent, any_fired).ok();

                writeln!(out, "{}br label %{}", indent, loop_h).ok();
                writeln!(out, "{}:", loop_h).ok();
                // Clear SSA old-value cache so precondition evaluation emits
                // fresh loads instead of stale cached values. Without this, the
                // CellCall convergence loop sees stale field values and loops
                // forever when the body stores new values to the same fields.
                self.ssa_old_int_regs.clear();
                self.ssa_old_float_regs.clear();

                for (ti, txn) in cell.transactions.iter().enumerate() {
                    let fire_l = format!(".cl_{}_{}", self.txn_counter, ti);
                    let post_ok_l = format!(".cl_{}_{}_pok", self.txn_counter, ti);
                    let reset_l = format!(".cl_{}_{}_pres", self.txn_counter, ti);
                    let skip_l = format!(".cl_{}_s_{}", self.txn_counter, ti);

                    // Evaluate precondition with rewritten identifiers
                    let pre_expr = Self::rewrite_cell_identifiers(&txn.contract.pre_condition, &callee_name);
                    let pre_val = self.emit_expr(out, &pre_expr, indent);

                    writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, pre_val.name, fire_l, skip_l).ok();
                    writeln!(out, "{}:", fire_l).ok();

                    // Execute body
                    for stmt in &txn.body {
                        let rewritten = Self::rewrite_cell_stmt_identifiers(stmt, &callee_name);
                        self.emit_stmt(out, &rewritten, indent);
                    }

                    // Check postcondition — set any_fired only if postcondition is true
                    let post_expr = Self::rewrite_cell_identifiers(&txn.contract.post_condition, &callee_name);
                    let post_val = self.emit_expr(out, &post_expr, indent);
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
                let af_load = format!("%cal_{}", self.txn_counter);
                writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, af_load, any_fired).ok();
                let af_bool = format!("%cab_{}", self.txn_counter);
                writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, af_bool, af_load).ok();
                writeln!(out, "{}store i8 0, ptr {}, align 1", indent, any_fired).ok();
                writeln!(out, "{}br i1 {}, label %{}, label %{}",
                    indent, af_bool, loop_h, done_l).ok();
                writeln!(out, "{}:", done_l).ok();

                // 3. Read designated output from prefixed output field
                let output_names = Self::extract_output_names_llvm(&cell.output_type);
                if let Some(first_name) = output_names.first() {
                    let prefixed = format!("cell${}${}", callee_name, first_name);
                    if let Some(&idx) = self.field_index_map.get(&prefixed) {
                        let ll_ty = &self.field_types[idx];
                        let gep = format!("%cgo_{}_{}", &callee_name, first_name);
                        writeln!(out, "{}{} = getelementptr %State, ptr {}, i32 0, i32 {}",
                            indent, gep, self.state_reg_name, idx).ok();
                        writeln!(out, "{}{} = load {}, ptr {}, align 8", indent, v, ll_ty, gep).ok();
                        let ret_ty = match ll_ty.as_str() {
                            "i8" => Type::Bool,
                            "i32" => Type::Char,
                            "float" => Type::Float,
                            "i8*" => Type::String,
                            _ => Type::Int,
                        };
                        if ret_ty == Type::Int && ll_ty != "i64" {
                            let boxed = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = zext {} {} to i64", indent, boxed, ll_ty, v).ok();
                            return TypedRegister { name: boxed, ty: Type::Int };
                        }
                        if ret_ty == Type::String {
                            let boxed = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, boxed, v).ok();
                            return TypedRegister { name: boxed, ty: Type::Int };
                        }
                        return TypedRegister { name: v.clone(), ty: ret_ty };
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
                return TypedRegister { name: v, ty: Type::Int };
            }
            _ => { unreachable!("emit_expr: unhandled Expr variant: {:?}", expr); }
        }
        // Default: treat as Int. Float operations are handled explicitly
        // by emit_binop/emit_fcmp which return Type::Float/Bool respectively.
        TypedRegister { name: v, ty: Type::Int }
    }

    // ── Cell identifier rewriting helpers ──────────────────────

    pub(super) fn rewrite_cell_identifiers(expr: &Expr, cell_name: &str) -> Expr {
        let p = |name: &str| -> String { format!("cell${}${}", cell_name, name) };
        match expr {
            // Leaf nodes — no identifiers
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::RegexLiteral(_)
                | Expr::Char(_) | Expr::Bool(_) | Expr::Term | Expr::Ellipsis
                | Expr::SharedMem(_) => expr.clone(),
            Expr::Literal(lit) => Expr::Literal(lit.clone()),
            // Identifier variants — rewrite to prefixed form
            Expr::Identifier(name) => Expr::Identifier(p(name)),
            Expr::OwnedRef(name) => Expr::OwnedRef(p(name)),
            Expr::PriorState(name) => Expr::PriorState(p(name)),
            Expr::EllipsisExpr(e) => Expr::EllipsisExpr(e.clone()),
            Expr::TypeRef(name) => Expr::TypeRef(name.clone()),
            // Arrow variants
            Expr::ArrowMut { dir, target, index, value } => Expr::ArrowMut {
                dir: dir.clone(),
                target: Box::new(Self::rewrite_cell_identifiers(target, cell_name)),
                index: Box::new(Self::rewrite_cell_identifiers(index, cell_name)),
                value: value.as_ref().map(|v| Box::new(Self::rewrite_cell_identifiers(v, cell_name))),
            },
            Expr::ArrowDiscard { target, index } => Expr::ArrowDiscard {
                target: Box::new(Self::rewrite_cell_identifiers(target, cell_name)),
                index: Box::new(Self::rewrite_cell_identifiers(index, cell_name)),
            },
            Expr::ArrowTransfer { dest, source, filter } => Expr::ArrowTransfer {
                dest: Box::new(Self::rewrite_cell_identifiers(dest, cell_name)),
                source: Box::new(Self::rewrite_cell_identifiers(source, cell_name)),
                filter: filter.as_ref().map(|f| Box::new(Self::rewrite_cell_identifiers(f, cell_name))),
            },
            Expr::ArrowMutExpr(e) => Expr::ArrowMutExpr(ArrowMutExpr {
                dir: e.dir.clone(),
                target: Box::new(Self::rewrite_cell_identifiers(&e.target, cell_name)),
                index: Box::new(Self::rewrite_cell_identifiers(&e.index, cell_name)),
                value: e.value.as_ref().map(|v| Box::new(Self::rewrite_cell_identifiers(v, cell_name))),
            }),
            Expr::ArrowDiscardExpr(e) => Expr::ArrowDiscardExpr(ArrowDiscardExpr {
                target: Box::new(Self::rewrite_cell_identifiers(&e.target, cell_name)),
                index: Box::new(Self::rewrite_cell_identifiers(&e.index, cell_name)),
            }),
            Expr::ArrowTransferExpr(e) => Expr::ArrowTransferExpr(ArrowTransferExpr {
                dest: Box::new(Self::rewrite_cell_identifiers(&e.dest, cell_name)),
                source: Box::new(Self::rewrite_cell_identifiers(&e.source, cell_name)),
                filter: e.filter.as_ref().map(|f| Box::new(Self::rewrite_cell_identifiers(f, cell_name))),
            }),
            // Binary ops — two children
            Expr::Add(l, r) => Expr::Add(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Sub(l, r) => Expr::Sub(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Mul(l, r) => Expr::Mul(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Div(l, r) => Expr::Div(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Mod(l, r) => Expr::Mod(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Eq(l, r) => Expr::Eq(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Ne(l, r) => Expr::Ne(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Lt(l, r) => Expr::Lt(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Le(l, r) => Expr::Le(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Gt(l, r) => Expr::Gt(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Ge(l, r) => Expr::Ge(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::And(l, r) => Expr::And(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Or(l, r) => Expr::Or(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::BitAnd(l, r) => Expr::BitAnd(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::BitOr(l, r) => Expr::BitOr(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::BitXor(l, r) => Expr::BitXor(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Shl(l, r) => Expr::Shl(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Shr(l, r) => Expr::Shr(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            Expr::Concat(l, r) => Expr::Concat(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            // Unary ops
            Expr::Not(e) => Expr::Not(Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            Expr::Neg(e) => Expr::Neg(Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            Expr::BitNot(e) => Expr::BitNot(Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            // IsType / FromCheck / Like
            Expr::IsType(e, target) => Expr::IsType(Box::new(Self::rewrite_cell_identifiers(e, cell_name)), target.clone()),
            Expr::FromCheck(e, ty) => Expr::FromCheck(Box::new(Self::rewrite_cell_identifiers(e, cell_name)), ty.clone()),
            Expr::Like(l, r) => Expr::Like(Box::new(Self::rewrite_cell_identifiers(l, cell_name)), Box::new(Self::rewrite_cell_identifiers(r, cell_name))),
            // Pattern B: BinaryOp / UnaryOp
            Expr::BinaryOp(e) => Expr::BinaryOp(Box::new(BinaryOpExpr {
                kind: e.kind,
                left: Box::new(Self::rewrite_cell_identifiers(&e.left, cell_name)),
                right: Box::new(Self::rewrite_cell_identifiers(&e.right, cell_name)),
            })),
            Expr::UnaryOp(e) => Expr::UnaryOp(Box::new(UnaryOpExpr {
                kind: e.kind,
                operand: Box::new(Self::rewrite_cell_identifiers(&e.operand, cell_name)),
            })),
            // Cast and Projection
            Expr::Cast(e, ty) => Expr::Cast(Box::new(Self::rewrite_cell_identifiers(e, cell_name)), ty.clone()),
            Expr::Projection { source, target } => Expr::Projection {
                source: Box::new(Self::rewrite_cell_identifiers(source, cell_name)),
                target: target.clone(),
            },
            Expr::ProjectionExpr(e) => Expr::ProjectionExpr(ProjectionExpr {
                source: Box::new(Self::rewrite_cell_identifiers(&e.source, cell_name)),
                target: e.target.clone(),
            }),
            // Calls
            Expr::Call(name, args) => Expr::Call(
                name.clone(),
                args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            Expr::CallExpr(e) => Expr::CallExpr(CallExpr {
                name: e.name.clone(),
                args: e.args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            }),
            Expr::CellCall(callee, args) => Expr::CellCall(
                Box::new(Self::rewrite_cell_identifiers(callee, cell_name)),
                args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            // Template/Macro calls
            Expr::TemplateCall { name, args, block, span } => Expr::TemplateCall {
                name: name.clone(),
                args: args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
                block: block.clone(),
                span: *span,
            },
            Expr::MacroCall { name, args, block, span } => Expr::MacroCall {
                name: name.clone(),
                args: args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
                block: block.clone(),
                span: *span,
            },
            Expr::IntrinsicCall { intrinsic, args } => Expr::IntrinsicCall {
                intrinsic: intrinsic.clone(),
                args: args.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            },
            // Collections
            Expr::ListLiteral(items) => Expr::ListLiteral(
                items.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            Expr::ListLiteralExpr(e) => Expr::ListLiteralExpr(ListLiteralExpr {
                elements: e.elements.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            }),
            Expr::MapLiteral(pairs) => Expr::MapLiteral(
                pairs.iter().map(|(k, v)| (Self::rewrite_cell_identifiers(k, cell_name), Self::rewrite_cell_identifiers(v, cell_name))).collect(),
            ),
            Expr::MapLiteralExpr(e) => Expr::MapLiteralExpr(MapLiteralExpr {
                entries: e.entries.iter().map(|(k, v)| (Self::rewrite_cell_identifiers(k, cell_name), Self::rewrite_cell_identifiers(v, cell_name))).collect(),
            }),
            Expr::SetLiteral(items) => Expr::SetLiteral(
                items.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            Expr::SetLiteralExpr(e) => Expr::SetLiteralExpr(SetLiteralExpr {
                entries: e.entries.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            }),
            Expr::ListIndex(list, idx) => Expr::ListIndex(
                Box::new(Self::rewrite_cell_identifiers(list, cell_name)),
                Box::new(Self::rewrite_cell_identifiers(idx, cell_name)),
            ),
            // Slice / MultiSlice
            Expr::Slice { value, start, end, stride, mask } => Expr::Slice {
                value: Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                start: start.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                end: end.as_ref().map(|e| Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
                stride: stride.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                mask: mask.as_ref().map(|m| Box::new(Self::rewrite_cell_identifiers(m, cell_name))),
            },
            Expr::SliceExpr(e) => Expr::SliceExpr(SliceExpr {
                value: Box::new(Self::rewrite_cell_identifiers(&e.value, cell_name)),
                start: e.start.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                end: e.end.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                stride: e.stride.as_ref().map(|s| Box::new(Self::rewrite_cell_identifiers(s, cell_name))),
                mask: e.mask.as_ref().map(|m| Box::new(Self::rewrite_cell_identifiers(m, cell_name))),
            }),
            Expr::MultiSlice { value, ops } => Expr::MultiSlice {
                value: Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                ops: ops.clone(),
            },
            Expr::MultiSliceExpr(e) => Expr::MultiSliceExpr(MultiSliceExpr {
                value: Box::new(Self::rewrite_cell_identifiers(&e.value, cell_name)),
                ops: e.ops.clone(),
            }),
            // Field access
            Expr::FieldAccess(obj, field) => Expr::FieldAccess(
                Box::new(Self::rewrite_cell_identifiers(obj, cell_name)),
                field.clone(),
            ),
            Expr::FieldAccessExpr(e) => Expr::FieldAccessExpr(FieldAccessExpr {
                obj: Box::new(Self::rewrite_cell_identifiers(&e.obj, cell_name)),
                field: e.field.clone(),
            }),
            // Struct / Object
            Expr::StructInstance(name, fields) => Expr::StructInstance(
                name.clone(),
                fields.iter().map(|(n, e)| (n.clone(), Self::rewrite_cell_identifiers(e, cell_name))).collect(),
            ),
            Expr::StructInstanceExpr(e) => Expr::StructInstanceExpr(StructInstanceExpr {
                typename: e.typename.clone(),
                fields: e.fields.iter().map(|(n, e)| (n.clone(), Self::rewrite_cell_identifiers(e, cell_name))).collect(),
            }),
            Expr::ObjectLiteral(fields) => Expr::ObjectLiteral(
                fields.iter().map(|(n, e)| (n.clone(), Self::rewrite_cell_identifiers(e, cell_name))).collect(),
            ),
            Expr::ObjectLiteralExpr(e) => Expr::ObjectLiteralExpr(ObjectLiteralExpr {
                fields: e.fields.iter().map(|(n, e)| (n.clone(), Self::rewrite_cell_identifiers(e, cell_name))).collect(),
            }),
            // Pattern / Match
            Expr::PatternMatch { value, variant, fields } => Expr::PatternMatch {
                value: Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                variant: variant.clone(),
                fields: fields.clone(),
            },
            Expr::PatternMatchExpr(e) => Expr::PatternMatchExpr(PatternMatchExpr {
                value: Box::new(Self::rewrite_cell_identifiers(&e.value, cell_name)),
                variant: e.variant.clone(),
                fields: e.fields.clone(),
            }),
            Expr::Match { value, arms } => Expr::Match {
                value: Box::new(Self::rewrite_cell_identifiers(value, cell_name)),
                arms: arms.clone(),
            },
            Expr::MatchExpr(e) => Expr::MatchExpr(MatchExpr {
                value: Box::new(Self::rewrite_cell_identifiers(&e.value, cell_name)),
                arms: e.arms.clone(),
            }),
            // Block
            Expr::Block(stmts, last) => Expr::Block(
                stmts.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                Box::new(Self::rewrite_cell_identifiers(last, cell_name)),
            ),
            Expr::BlockExpr(e) => Expr::BlockExpr(BlockExpr {
                stmts: e.stmts.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                last: Box::new(Self::rewrite_cell_identifiers(&e.last, cell_name)),
            }),
            // Quote / Interpolation
            Expr::Interpolate(name) => Expr::Interpolate(name.clone()),
            Expr::InterpolateExpr(e) => Expr::InterpolateExpr(Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            Expr::QuoteBlock { statements, trailing_expr } => Expr::QuoteBlock {
                statements: statements.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                trailing_expr: trailing_expr.as_ref().map(|e| Box::new(Self::rewrite_cell_identifiers(e, cell_name))),
            },
            // Tuple
            Expr::TupleDestructure(names, expr) => Expr::TupleDestructure(
                names.clone(),
                Box::new(Self::rewrite_cell_identifiers(expr, cell_name)),
            ),
            Expr::TupleDestructureExpr(e) => Expr::TupleDestructureExpr(TupleDestructureExpr {
                names: e.names.clone(),
                expr: Box::new(Self::rewrite_cell_identifiers(&e.expr, cell_name)),
            }),
            Expr::Tuple(items) => Expr::Tuple(
                items.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            ),
            Expr::TupleExpr(e) => Expr::TupleExpr(TupleExpr {
                exprs: e.exprs.iter().map(|a| Self::rewrite_cell_identifiers(a, cell_name)).collect(),
            }),
            // SigCall
            Expr::SigCall { modifier, expr } => Expr::SigCall {
                modifier: modifier.clone(),
                expr: Box::new(Self::rewrite_cell_identifiers(expr, cell_name)),
            },
            Expr::SigCallExpr(e) => Expr::SigCallExpr(SigCallExpr {
                modifier: e.modifier.clone(),
                expr: Box::new(Self::rewrite_cell_identifiers(&e.expr, cell_name)),
            }),
            // Subtype projection
            Expr::SubtypeProjection { source, ops } => Expr::SubtypeProjection {
                source: Box::new(Self::rewrite_cell_identifiers(source, cell_name)),
                ops: ops.clone(),
            },
            Expr::SubtypeProjectionExpr(e) => Expr::SubtypeProjectionExpr(SubtypeProjectionExpr {
                source: Box::new(Self::rewrite_cell_identifiers(&e.source, cell_name)),
                ops: e.ops.clone(),
            }),
            // DBVL
            Expr::DbvlTable { path, field_names, key_offsets, schema_name } => Expr::DbvlTable {
                path: path.clone(),
                field_names: field_names.clone(),
                key_offsets: key_offsets.clone(),
                schema_name: schema_name.clone(),
            },
            Expr::DbvlTableExpr(e) => Expr::DbvlTableExpr(e.clone()),
            // Pipe chain
            Expr::PipeChain(chain) => Expr::PipeChain(PipeChain {
                initial: Box::new(Self::rewrite_cell_identifiers(&chain.initial, cell_name)),
                steps: chain.steps.iter().map(|s| PipeStep {
                    target: Box::new(Self::rewrite_cell_identifiers(&s.target, cell_name)),
                    skip: s.skip,
                }).collect(),
            }),
        }
    }

    pub(super) fn rewrite_cell_stmt_identifiers(stmt: &Statement, cell_name: &str) -> Statement {
        match stmt {
            Statement::Assignment { lhs, expr, timeout, modifiers } => Statement::Assignment {
                lhs: Self::rewrite_cell_identifiers(lhs, cell_name),
                expr: Self::rewrite_cell_identifiers(expr, cell_name),
                timeout: timeout.clone(),
                modifiers: modifiers.clone(),
            },
            Statement::Unification { name, variant, fields, expr } => Statement::Unification {
                name: name.clone(),
                variant: variant.clone(),
                fields: fields.clone(),
                expr: Self::rewrite_cell_identifiers(expr, cell_name),
            },
            Statement::Guarded { condition, statements } => Statement::Guarded {
                condition: Self::rewrite_cell_identifiers(condition, cell_name),
                statements: statements.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            },
            Statement::Term { values, swan_song, modifiers } => Statement::Term {
                values: values.iter().map(|v| v.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name))).collect(),
                swan_song: swan_song.as_ref().map(|s| Box::new(Self::rewrite_cell_stmt_identifiers(s, cell_name))),
                modifiers: modifiers.clone(),
            },
            Statement::TermBang { values, swan_song, modifiers } => Statement::TermBang {
                values: values.iter().map(|v| v.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name))).collect(),
                swan_song: swan_song.as_ref().map(|s| Box::new(Self::rewrite_cell_stmt_identifiers(s, cell_name))),
                modifiers: modifiers.clone(),
            },
            Statement::Escape(expr) => Statement::Escape(
                expr.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name)),
            ),
            Statement::Expression(expr) => Statement::Expression(
                Self::rewrite_cell_identifiers(expr, cell_name),
            ),
            Statement::Let { name, ty, expr, address, address_expr, bit_range, constraint, is_override, modifiers } => Statement::Let {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name)),
                address: *address,
                address_expr: address_expr.as_ref().map(|a| Box::new(Self::rewrite_cell_identifiers(a, cell_name))),
                bit_range: bit_range.clone(),
                constraint: constraint.as_ref().map(|c| Box::new(Self::rewrite_cell_identifiers(c, cell_name))),
                is_override: *is_override,
                modifiers: modifiers.clone(),
            },
            Statement::InlineAsm { asm_string, clobbers, span } => Statement::InlineAsm {
                asm_string: asm_string.clone(),
                clobbers: clobbers.clone(),
                span: *span,
            },
            Statement::LocalTrigger { name, ty, expr, span } => Statement::LocalTrigger {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr.as_ref().map(|e| Self::rewrite_cell_identifiers(e, cell_name)),
                span: *span,
            },
            Statement::TrgBinding { name, ty, instance, port, modifiers } => Statement::TrgBinding {
                name: name.clone(),
                ty: ty.clone(),
                instance: Self::rewrite_cell_identifiers(instance, cell_name),
                port: port.clone(),
                modifiers: modifiers.clone(),
            },
            Statement::Alka(alka) => Statement::Alka(alka.clone()),
            Statement::OnExit { body, span } => Statement::OnExit {
                body: body.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                span: *span,
            },
            Statement::SyncBlock { body } => Statement::SyncBlock {
                body: body.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
            },
            Statement::Foreach { item, list, body, modifiers } => Statement::Foreach {
                item: item.clone(),
                list: Box::new(Self::rewrite_cell_identifiers(list, cell_name)),
                body: body.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                modifiers: modifiers.clone(),
            },
            Statement::Oracle { handler, body, span } => Statement::Oracle {
                handler: handler.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                body: body.iter().map(|s| Self::rewrite_cell_stmt_identifiers(s, cell_name)).collect(),
                span: *span,
            },
            Statement::Await { expr, modifiers } => Statement::Await {
                expr: Self::rewrite_cell_identifiers(expr, cell_name),
                modifiers: modifiers.clone(),
            },
            Statement::Async { body, modifiers } => Statement::Async {
                body: Box::new(Self::rewrite_cell_stmt_identifiers(body, cell_name)),
                modifiers: modifiers.clone(),
            },
            Statement::AsyncAwait { body, lhs, modifiers } => Statement::AsyncAwait {
                body: Box::new(Self::rewrite_cell_stmt_identifiers(body, cell_name)),
                lhs: lhs.clone(),
                modifiers: modifiers.clone(),
            },
        }
    }

    pub(super) fn extract_output_names_llvm(ot: &Option<OutputType>) -> Vec<String> {
        match ot {
            Some(OutputType::Named(name, inner)) => {
                let mut names = vec![name.clone()];
                names.extend(Self::extract_output_names_llvm(&Some(inner.as_ref().clone())));
                names
            }
            Some(OutputType::Tuple(types)) => {
                types.iter().flat_map(|t| Self::extract_output_names_llvm(&Some(t.clone()))).collect()
            }
            Some(OutputType::Union(types)) => {
                types.iter().flat_map(|t| Self::extract_output_names_llvm(&Some(t.clone()))).collect()
            }
            Some(OutputType::Single(_)) | Some(OutputType::Array(_)) | None => Vec::new(),
        }
    }

    /// Emit a main() that stores final precomputed values and returns.
    /// A000: no runtime loop, no iteration. The region analyzer simulated
    /// all transactions within --optimize-budget and produced final values.
    /// This is the most extreme optimization: zero runtime memory traffic.
    pub(crate) fn emit_precomputed_main(
        &mut self,
        out: &mut String,
        final_values: &[(Vec<String>, std::collections::HashMap<String, i64>)],
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        self.emit_inline_init_stores(out, "%state");
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (_, bindings) in final_values {
            for (var, val) in bindings {
                if !seen.insert(var) { continue; }
                if let Some(&idx) = self.field_index_map.get(var) {
                    let ty = &self.field_types[idx];
                    writeln!(out, "  %gp_{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", var, idx).ok();
                    match ty.as_str() {
                        "float" => {
                            let bits = *val as i32 as u32;
                            writeln!(out, "  store float bitcast (i32 {} to float), float* %gp_{}, align 4", bits, var).ok();
                        }
                        "i8" => {
                            writeln!(out, "  store i8 {}, i8* %gp_{}, align 1", val, var).ok();
                        }
                        _ => {
                            writeln!(out, "  store i64 {}, i64* %gp_{}, align 8", val, var).ok();
                        }
                    }
                } else if let Some(&addr) = self.mmio_fields.get(var) {
                    writeln!(out, "  %gp_{} = inttoptr i64 {} to i64*", var, addr).ok();
                    writeln!(out, "  store volatile i64 {}, i64* %gp_{}, align 1", val, var).ok();
                }
            }
        }
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ── WAKE TRIGGER METADATA ─────────────────────────────────
    pub(crate) fn emit_wake_metadata(&self, out: &mut String) {
        let wake_symbols: Vec<&str> = self.triggers.values()
            .filter(|t| t.is_wake)
            .filter_map(|t| match &t.address {
                crate::ast::LinkRef::Linked(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        if wake_symbols.is_empty() { return; }
        let count = wake_symbols.len();
        let sym_list = wake_symbols.iter().map(|s| format!("i8* @{}", s)).collect::<Vec<_>>().join(", ");
        writeln!(out, "@llvm.wake_triggers = constant [{} x i8*] [{}]", count, sym_list).ok();
        writeln!(out, "!llvm.wake_triggers = !{{!6}}").ok();
        write!(out, "!6 = !{{").ok();
        for (i, sym) in wake_symbols.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "!\"{}\"", sym).ok();
        }
        writeln!(out, "}}").ok();
    }

    // ── THREAD POOL METADATA ────────────────────────────────
    pub(crate) fn emit_thread_pool_metadata(&self, out: &mut String) {
        if !self.has_async_txns || self.is_lightweight_async { return; }
        let count = self.async_txn_names.len();
        let fn_list: Vec<String> = self.async_txn_names.iter()
            .map(|n| format!("i8* bitcast (void (ptr)* @async_body_{} to i8*)", n))
            .collect();
        writeln!(out, "@llvm.thread_pool = constant [{} x i8*] [{}]",
            count, fn_list.join(", ")).ok();
        // Emit a packed array of function pointers for brief_thread_pool_init
        writeln!(out, "@thread_pool_fns = private constant [{} x void (ptr)*] [{}]",
            count,
            self.async_txn_names.iter()
                .map(|n| format!("void (ptr)* @async_body_{}", n))
                .collect::<Vec<_>>().join(", "),
        ).ok();
    }

    /// Emit the async phase calls in main: release workers, run sequential
    /// reactor, wait for workers. Used by emit_main and emit_enum_main.
    pub(crate) fn emit_async_phase(&self, out: &mut String) {
        if !self.has_async_txns || self.is_lightweight_async { return; }
        writeln!(out, "  call void @__barrier_release__()").ok();
        // Sequential reactor runs in main thread concurrently with workers
        writeln!(out, "  call void @reactor_tick(ptr noalias nocapture %state)").ok();
        writeln!(out, "  call void @__barrier_wait__()").ok();
    }

    // ── FUSABLE PAIRS ────────────────────────────────────────
    pub(crate) fn resolve_fusable_pairs(&self, txns: &[(String, &crate::ast::Transaction)]) -> Vec<(String, String)> {
        let prg = crate::ast::Program {
            items: txns.iter().map(|(_, t)| crate::ast::TopLevel::Transaction((*t).clone())).collect(),
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: crate::ast::StrictMode::Off, dispatch_mode: crate::ast::DispatchMode::Sequential, exit_condition: None, out_pragmas: vec![], default_sig_modifier: None, watchdog_defaults: (None, None),
        };
        let mut pairs = crate::backend::detect_fusable_pairs(&prg);
        pairs.retain(|(a, b)| {
            if let (Some((_, ta)), Some((_, tb))) = (txns.iter().find(|(n, _)| n == a), txns.iter().find(|(n, _)| n == b)) {
                if ta.is_async || tb.is_async { return false; }
                // Skip callable txns — they don't use %State*, can't be fused with reactive txns
                if !ta.is_reactive || !tb.is_reactive { return false; }
                let aw = crate::backend::collect_assigned_identifiers(&ta.body);
                let bw = crate::backend::collect_assigned_identifiers(&tb.body);
                if aw.iter().any(|w| bw.contains(w)) { return false; }
                if self.trg_in_pre(&tb.contract.pre_condition) { return false; }
                true
            } else { false }
        });
        pairs
    }

    pub(crate) fn trg_in_pre(&self, pre: &Expr) -> bool {
        let mut ids = std::collections::HashSet::new();
        crate::backend::collect_expr_identifiers(pre, &mut ids);
        ids.iter().any(|id| self.trigger_names.contains(id))
    }

    pub(crate) fn emit_cast_convert(&mut self, out: &mut String, indent: &str, dst: &str, src: &str, src_ty: Option<Type>, target: &Type) {
        let src_ty = match src_ty {
            Some(t) => t,
            None => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
                return;
            }
        };
        if &src_ty == target {
            let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            return;
        }
        match (&src_ty, target) {
            (Type::Int | Type::UInt, Type::Float) => {
                // 2026-06-17: Native float, not boxed i64. Downstream
                // code (emit_binop, enum constructors) converts to/from
                // i64 as needed via ensure_float_reg / native_float_or_box.
                let _ = writeln!(out, "{}{} = sitofp i64 {} to float", indent, dst, src);
            }
            (Type::Float, Type::Int | Type::UInt) => {
                let tr = format!("%ctr{}", self.txn_counter); self.txn_counter += 1;
                let fl = format!("%cfl{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
                let _ = writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr);
                let _ = writeln!(out, "{}{} = fptosi float {} to i64", indent, dst, fl);
            }
            (Type::Int | Type::UInt, Type::Bool) => {
                let ci = format!("%ccb{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Bool, Type::Int | Type::UInt) => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            (Type::Float, Type::Bool) => {
                let tr = format!("%cfbtr{}", self.txn_counter); self.txn_counter += 1;
                let fl = format!("%cfbfl{}", self.txn_counter); self.txn_counter += 1;
                let ci = format!("%cfbci{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
                let _ = writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr);
                let _ = writeln!(out, "{}{} = fcmp fast une float {}, 0.0", indent, ci, fl);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Bool, Type::Float) => {
                let ci = format!("%cbfci{}", self.txn_counter); self.txn_counter += 1;
                let fl = format!("%cbffl{}", self.txn_counter); self.txn_counter += 1;
                let fi = format!("%cbffi{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = select i1 {}, float 1.000000e+00, float 0.000000e+00", indent, fl, ci);
                let _ = writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fl);
                let _ = writeln!(out, "{}{} = zext i32 {} to i64", indent, dst, fi);
            }
            // Char ↔ Bool
            (Type::Char, Type::Bool) => {
                let ci = format!("%cci{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Bool, Type::Char) => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            // Char ↔ Int
            (Type::Char, Type::Int | Type::UInt) => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            (Type::Int | Type::UInt, Type::Char) => {
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, dst, src);
            }
            // Char ↔ String (construct Brief string struct {cap, len, data})
            (Type::Char, Type::String) => {
                let tr = format!("%cctr{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
                let alloc = format!("%ccac{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = call i8* @malloc(i64 24)", indent, alloc);
                let hp = format!("%cchp{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, hp, alloc);
                let base = format!("%ccba{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, alloc);
                let dp = format!("%ccdp{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base);
                let _ = writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp, hp);
                let ls = format!("%ccls{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, ls, hp);
                let _ = writeln!(out, "{}store i64 1, i64* {}, align 8", indent, ls);
                let cs = format!("%cccs{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = getelementptr i8, i8* {}, i64 16", indent, cs, alloc);
                let tb = format!("%cctb{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i32 {} to i8", indent, tb, tr);
                let _ = writeln!(out, "{}store i8 {}, i8* {}, align 1", indent, tb, cs);
                let nt = format!("%ccnt{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = getelementptr i8, i8* {}, i64 17", indent, nt, alloc);
                let _ = writeln!(out, "{}store i8 0, i8* {}, align 1", indent, nt);
                let _ = writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, dst, alloc);
            }
            (Type::String, Type::Char) => {
                let ip = format!("%csip{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, ip, src);
                let lb = format!("%cslb{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, lb, ip);
                let _ = writeln!(out, "{}{} = zext i8 {} to i64", indent, dst, lb);
            }
            // Int ↔ String (via existing __int_to_str)
            (Type::Int | Type::UInt, Type::String) => {
                let _ = writeln!(out, "{}{} = call i64 @__int_to_str__(i64 {})", indent, dst, src);
            }
            (Type::String, Type::Int | Type::UInt) => {
                let ip = format!("%csii{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, ip, src);
                let _ = writeln!(out, "{}{} = call i64 @__str_to_int(i8* {})", indent, dst, ip);
            }
            // String ↔ Bool (non-empty is true)
            (Type::String, Type::Bool) => {
                let ip = format!("%csbi{}", self.txn_counter); self.txn_counter += 1;
                let lb = format!("%csbl{}", self.txn_counter); self.txn_counter += 1;
                let ci = format!("%csbc{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, ip, src);
                let _ = writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, lb, ip);
                let _ = writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, ci, lb);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Bool, Type::String) => {
                let ci = format!("%cbsc{}", self.txn_counter); self.txn_counter += 1;
                let ip = format!("%cbsi{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = call i8* @__chr_to_str(i32 {})", indent, ip, ci);
                let _ = writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, dst, ip);
            }
            _ => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
        }
    }

    pub(crate) fn i64_to_float_reg(&mut self, out: &mut String, reg: &str, indent: &str) -> String {
        // Check cache first: these are actual float registers from SSA extraction
        // or float literal caching. Do NOT check register_types here — that map
        // tracks Brief-level float semantics, not LLVM type (boxed as i64).
        if let Some(cached) = self.reg_float_cache.get(reg) {
            return cached.clone();
        }
        let tr = format!("%ftr{}", self.txn_counter); self.txn_counter += 1;
        let fl = format!("%ffl{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, reg).ok();
        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
        fl
    }

    /// If `reg` is an i64 (Int), truncate to i1. Otherwise return its name as-is.
    pub(super) fn as_bool_reg(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        if reg.ty == Type::Int {
            let t = format!("%tb{}", self.txn_counter);
            self.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i1", indent, t, reg.name).ok();
            t
        } else {
            reg.name.clone()
        }
    }

    /// Convert a String/Data typed register to i64 for C ABI calls.
    /// Int/Bool/Char/Float registers are passed through as-is.
    fn ptrtoint_if_string(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        if reg.ty == Type::String || reg.ty == Type::Data {
            let p = format!("%ptri{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, p, reg.name).ok();
            p
        } else {
            reg.name.clone()
        }
    }

    /// Emit inline string concatenation: malloc + header setup + memcpy + free.
    /// Both operands are i8* (Brief header pointers). Returns i8*.
    ///
    /// Tag convention (2026-06-19):
    ///   bit 0 = static string constant (don't free, don't read header at -16)
    ///   bit 1 = temporary concat result (safe to free when consumed)
    /// State-loaded strings have both bits clear (heap, state-owned).
    /// Only concat results get bit 1 set.
    //
    // Why inline string concat instead of calling sprintf/strcat: the compiler
    // knows each operand's length at emit time (from header slot 1), so it can
    // compute the total allocation size and emit memcpy calls that LLVM can
    // lower to rep movsb or inline. sprintf would need to scan for null
    // terminators at runtime, losing the length information.
    //
    // Tag bits: bit 0 = static string constant (from .rodata, don't free),
    // bit 1 = temporary concat result (safe to free when consumed).
    // State-loaded strings have both bits clear. The tag convention avoids
    // separate tracking data structures.
    fn emit_inline_concat(&mut self, out: &mut String, indent: &str, a: &TypedRegister, b: &TypedRegister) -> TypedRegister {
        let a_boxed = self.adapt_to_i64(out, indent, a);
        let b_boxed = self.adapt_to_i64(out, indent, b);
        // Mask off tag bits (bit 0 = static, bit 1 = temp) before reading headers
        let a_clean = format!("%cam{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = and i64 {}, -4", indent, a_clean, a_boxed).ok();
        let b_clean = format!("%cbm{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = and i64 {}, -4", indent, b_clean, b_boxed).ok();
        let ha = format!("%cha{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, ha, a_clean).ok();
        let la_ptr = format!("%clp{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, la_ptr, ha).ok();
        let la = format!("%cla{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, la, la_ptr).ok();
        let hb = format!("%chb{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hb, b_clean).ok();
        let lb_ptr = format!("%clq{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lb_ptr, hb).ok();
        let lb = format!("%clb{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, lb, lb_ptr).ok();
        let total = format!("%ctl{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = add i64 {}, {}", indent, total, la, lb).ok();
        // Tight packing: 16 byte header + total chars + 1 null byte
        let header_size = format!("%chs{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = add i64 16, {}", indent, header_size, total).ok();
        let alloc_size = format!("%cas{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = add i64 {}, 1", indent, alloc_size, header_size).ok();
        let result = self.emit_arena_alloc(out, indent, &alloc_size);
        let hp = format!("%chp{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, hp, result).ok();
        let base = format!("%cba{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, base, result).ok();
        let dp = format!("%cdp{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = add i64 {}, 16", indent, dp, base).ok();
        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp, hp).ok();
        let len_slot = format!("%cls{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, len_slot, hp).ok();
        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, total, len_slot).ok();
        let a_dp = format!("%cad{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, a_dp, ha).ok();
        let a_chars = format!("%cac{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, a_chars, a_dp).ok();
        let dest_start = format!("%cds{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i8, i8* {}, i64 16", indent, dest_start, result).ok();
        writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)", indent, dest_start, a_chars, la).ok();
        let dest_off = format!("%cdo{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i8, i8* {}, i64 {}", indent, dest_off, dest_start, la).ok();
        let b_dp = format!("%cbd{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, b_dp, hb).ok();
        let b_chars = format!("%cbc{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, b_chars, b_dp).ok();
        writeln!(out, "{}call void @llvm.memcpy.p0i8.p0i8.i64(i8* {}, i8* {}, i64 {}, i1 false)", indent, dest_off, b_chars, lb).ok();
        // Null terminate
        let nt = format!("%cnt{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr i8, i8* {}, i64 {}", indent, nt, dest_start, total).ok();
        writeln!(out, "{}store i8 0, i8* {}, align 1", indent, nt).ok();
        // Free heap-allocated operands that are temporaries (bit 1 set).
        // When arena is active, the arena owns all allocations — skip.
        // Static constants (bit 0=1) and state fields (bit 0=0,bit 1=0) are
        // always preserved regardless of arena mode.
        if self.arena_slots.is_none() {
            let tag_a = format!("%cta{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, 2", indent, tag_a, a_boxed).ok();
            let is_temp_a = format!("%cia{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, is_temp_a, tag_a).ok();
            let free_a_label = format!("free_a_{}", self.txn_counter);
            let after_free_a_label = format!("af_a_{}", self.txn_counter);
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_temp_a, free_a_label, after_free_a_label).ok();
            writeln!(out, "{}{}:", indent, free_a_label).ok();
            let a_clean_all = format!("%cca{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, -4", indent, a_clean_all, a_boxed).ok();
            let a_free_ptr = format!("%cfp{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, a_free_ptr, a_clean_all).ok();
            writeln!(out, "{}call void @free(i8* {})", indent, a_free_ptr).ok();
            writeln!(out, "{}br label %{}", indent, after_free_a_label).ok();
            writeln!(out, "{}{}:", indent, after_free_a_label).ok();
            // Same for operand B
            let tag_b = format!("%ctb{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, 2", indent, tag_b, b_boxed).ok();
            let is_temp_b = format!("%cib{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, is_temp_b, tag_b).ok();
            let free_b_label = format!("free_b_{}", self.txn_counter);
            let after_free_b_label = format!("af_b_{}", self.txn_counter);
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_temp_b, free_b_label, after_free_b_label).ok();
            writeln!(out, "{}{}:", indent, free_b_label).ok();
            let b_clean_all = format!("%ccb{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, -4", indent, b_clean_all, b_boxed).ok();
            let b_free_ptr = format!("%cfq{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, b_free_ptr, b_clean_all).ok();
            writeln!(out, "{}call void @free(i8* {})", indent, b_free_ptr).ok();
            writeln!(out, "{}br label %{}", indent, after_free_b_label).ok();
            writeln!(out, "{}{}:", indent, after_free_b_label).ok();
        }

        let v = format!("%t{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = bitcast i8* {} to i8*", indent, v, result).ok();
        // Box to i64 — downstream code expects i64 (ptrtoint).
        let vi = format!("%t{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, vi, v).ok();
        // Tag as temporary (bit 1 = 1) so future concat calls can free it
        let vi_tagged = format!("%t{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = or i64 {}, 2", indent, vi_tagged, vi).ok();
        TypedRegister { name: vi_tagged, ty: Type::Int }
    }

    pub(crate) fn emit_binop(&mut self, out: &mut String, indent: &str, l: &Expr, r: &Expr, int_op: &str, float_op: &str) -> TypedRegister {
        // Peephole: constant-fold integer binops at compile time
        if let (Expr::Integer(li), Expr::Integer(ri)) = (l, r) {
            let result = match int_op {
                "add" => Some(li.wrapping_add(*ri)),
                "sub" => Some(li.wrapping_sub(*ri)),
                "mul" => Some(li.wrapping_mul(*ri)),
                "sdiv" if *ri != 0 => Some(li / ri),
                "and" => Some(li & ri),
                "or"  => Some(li | ri),
                "xor" => Some(li ^ ri),
                "shl" => Some(li.wrapping_shl(*ri as u32)),
                "lshr" => Some((*li as u64).wrapping_shr(*ri as u32) as i64),
                _ => None,
            };
            if let Some(folded) = result {
                let v = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, {}", indent, v, folded).ok();
                return TypedRegister { name: v, ty: Type::Int };
            }
        }
        let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent));
        if a.ty == Type::Float || b.ty == Type::Float {
            // 2026-06-17: Skip float path if either operand is String/Data
            // (prevents pointer→float corruption, e.g. String + Float).
            if a.ty == Type::String || a.ty == Type::Data || b.ty == Type::String || b.ty == Type::Data {
                let v = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                let a_i64 = self.adapt_to_i64(out, indent, &a);
                let b_i64 = self.adapt_to_i64(out, indent, &b);
                writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, a_i64, b_i64).ok();
                TypedRegister { name: v, ty: Type::Int }
            } else {
                let fa = self.ensure_float_reg(out, indent, &a);
                let fb = self.ensure_float_reg(out, indent, &b);
                let fr = format!("%bfr{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = {} fast float {}, {}", indent, fr, float_op, fa, fb).ok();
                self.reg_float_cache.insert(fr.clone(), fr.clone());
                TypedRegister { name: fr, ty: Type::Float }
            }
        } else {
            let v = format!("%t{}", self.txn_counter); self.txn_counter += 1;
            let a_i64 = self.adapt_to_i64(out, indent, &a);
            let b_i64 = self.adapt_to_i64(out, indent, &b);
            writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, a_i64, b_i64).ok();
            TypedRegister { name: v, ty: Type::Int }
        }
    }

    /// Check if an expression is a reference to a linked String trigger.
    fn is_linked_string_trigger(&self, expr: &Expr) -> bool {
        if let Expr::Identifier(name) = expr {
            if let Some(trg) = self.triggers.get(name) {
                return matches!(trg.ty, Type::String | Type::Data);
            }
        }
        false
    }

    pub(crate) fn emit_fcmp(&mut self, out: &mut String, indent: &str, l: &Expr, r: &Expr, cond: &str) -> TypedRegister {
        // Peephole: constant-fold integer comparisons at compile time
        if let (Expr::Integer(li), Expr::Integer(ri)) = (l, r) {
            let result = match cond {
                "oeq" => li == ri,
                "one" => li != ri,
                "olt" => li < ri,
                "ole" => li <= ri,
                "ogt" => li > ri,
                "oge" => li >= ri,
                _ => false,
            };
            let v = format!("%t{}", self.txn_counter); self.txn_counter += 1;
            if result {
                writeln!(out, "{}{} = and i1 true, true", indent, v).ok();
            } else {
                writeln!(out, "{}{} = xor i1 true, true", indent, v).ok();
            }
            return TypedRegister { name: v, ty: Type::Bool };
        }
        // String trigger vs string literal: dereference pointer and compare first byte
        if let Expr::String(s) = r {
            if self.is_linked_string_trigger(l) {
                let a = self.emit_expr(out, l, indent);
                let icmp_cond = match cond {
                    "oeq" => "eq", "one" => "ne", "olt" => "slt",
                    "ole" => "sle", "ogt" => "sgt", "oge" => "sge",
                    _ => cond,
                };
                let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, a.name).ok();
                let b = format!("%fb{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, b, p).ok();
                let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, b).ok();
                let byte_val = s.as_bytes().first().copied().unwrap_or(0u8) as i64;
                let c = format!("%fc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, z, byte_val).ok();
                return TypedRegister { name: c, ty: Type::Bool };
            }
        }
        if let Expr::String(s) = l {
            if self.is_linked_string_trigger(r) {
                let b = self.emit_expr(out, r, indent);
                let icmp_cond = match cond {
                    "oeq" => "eq", "one" => "ne", "olt" => "slt",
                    "ole" => "sle", "ogt" => "sgt", "oge" => "sge",
                    _ => cond,
                };
                let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, b.name).ok();
                let bv = format!("%fb{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, bv, p).ok();
                let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, bv).ok();
                let byte_val = s.as_bytes().first().copied().unwrap_or(0u8) as i64;
                let c = format!("%fc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, z, byte_val).ok();
                return TypedRegister { name: c, ty: Type::Bool };
            }
        }
        let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent));
        let c = format!("%c{}", self.txn_counter); self.txn_counter += 1;
        if a.ty == Type::Float || b.ty == Type::Float {
            let fa = self.ensure_float_reg(out, indent, &a);
            let fb = self.ensure_float_reg(out, indent, &b);
            writeln!(out, "{}{} = fcmp fast {} float {}, {}", indent, c, cond, fa, fb).ok();
        } else {
            let icmp_cond = match cond {
                "oeq" => "eq",
                "one" => "ne",
                "olt" => "slt",
                "ole" => "sle",
                "ogt" => "sgt",
                "oge" => "sge",
                _ => cond,
            };
            // Ensure both operands are i64 for icmp — Bool (i1) and Char (i32)
            // need zext to i64. Uses let mut to handle inline zext.
            let a_i64 = self.adapt_to_i64(out, indent, &a);
            let b_i64 = self.adapt_to_i64(out, indent, &b);
            writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, a_i64, b_i64).ok();
        }
        TypedRegister { name: c, ty: Type::Bool }
    }

    /// Recursively detect if an expression chain produces a String/Data value.
    /// Used by emit_inline_concat to determine whether to use the inline
    /// concat path or emit generic Add IR.
    ///
    /// Why this exists: a + b on Ints should emit `add i64`, but a + b on
    /// Strings should emit malloc+memcpy. The type tracker checks type
    /// bindings, defn return types, and cast targets.
    fn is_string_chain(&self, e: &Expr) -> bool {
        match e {
            Expr::String(_) => true,
            Expr::Literal(lit) => matches!(lit.as_ref(), crate::features::literal::LiteralExpr::String(_)),
            Expr::Identifier(name) => {
                matches!(self.let_binding_types.get(name), Some(t) if *t == Type::String || *t == Type::Data)
                || matches!(self.let_original_types.get(name), Some(t) if *t == Type::String || *t == Type::Data)
                || {
                    // Check state fields whose LLVM type is i8* (String/Data)
                    self.field_index_map.get(name)
                        .and_then(|&idx| self.field_types.get(idx))
                        .map(|ft| ft == "i8*" || ft == "ptr")
                        .unwrap_or(false)
                }
            }
            Expr::Add(l, r) | Expr::Concat(l, r) => {
                self.is_string_chain(l) || self.is_string_chain(r)
            }
            Expr::Cast(inner, target_ty) => {
                matches!(*target_ty, Type::String | Type::Data)
                    || self.is_string_chain(inner)
            }
            Expr::BinaryOp(bo) if bo.kind == crate::features::binary_op::BinaryOpKind::Add => {
                self.is_string_chain(&bo.left) || self.is_string_chain(&bo.right)
            }
            Expr::Call(name, _) => {
                self.defn_return_types.get(name.as_str())
                    .map(|types| types.iter().any(|t| *t == Type::String || *t == Type::Data))
                    .unwrap_or(false)
            }
            _ => false,
        }
    }

    /// Emit native LLVM IR for well-known UserDefinedWithArg projections.
    /// 45+ operator/type pairs (Add/Sub/Mul/Div/Eq/Ne on Int/Float/Bool).
    /// Avoids boxing through i64 — native add/fadd/icmp instructions.
    ///
    /// Why this exists: Brief's projection system is generic (any operator
    /// on any type dispatches through UserDefinedWithArg). But for primitive
    /// types, the generic dispatch would: load i64, convert to native, exec
    /// op, convert back. The fast path emits native IR directly, skipping
    /// both conversions.
    fn try_projection_fast_path(
        &mut self,
        out: &mut String,
        src_val: &TypedRegister,
        name: &str,
        arg_expr: &Expr,
        indent: &str,
        v: &str,
    ) -> Option<TypedRegister> {
        let rhs = self.emit_expr(out, arg_expr, indent);
        let tr = match (src_val.ty.clone(), name) {
            // ── Int arithmetic ──
            (Type::Int, "Add") => {
                writeln!(out, "{}{} = add i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Sub") => {
                writeln!(out, "{}{} = sub i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Mul") => {
                writeln!(out, "{}{} = mul i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Div") => {
                writeln!(out, "{}{} = sdiv i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Mod") => {
                writeln!(out, "{}{} = srem i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            // ── Int comparison ──
            (Type::Int, "Eq") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Ne") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Lt") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Le") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Gt") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Ge") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            // ── Int bitwise ──
            (Type::Int, "BitAnd") => {
                writeln!(out, "{}{} = and i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "BitOr") => {
                writeln!(out, "{}{} = or i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "BitXor") => {
                writeln!(out, "{}{} = xor i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Shl") => {
                writeln!(out, "{}{} = shl i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Shr") => {
                writeln!(out, "{}{} = lshr i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            // ── Int/Char logical (treated as boolean in Brief) ──
            (Type::Int, "And") => {
                writeln!(out, "{}{} = and i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            (Type::Int, "Or") => {
                writeln!(out, "{}{} = or i64 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Int }
            }
            // ── Float arithmetic ──
            (Type::Float, "Add") => {
                writeln!(out, "{}{} = fadd float {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Sub") => {
                writeln!(out, "{}{} = fsub float {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Mul") => {
                writeln!(out, "{}{} = fmul float {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Div") => {
                writeln!(out, "{}{} = fdiv float {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Eq") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = fcmp oeq float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Ne") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = fcmp one float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Lt") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = fcmp olt float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Le") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = fcmp ole float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Gt") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = fcmp ogt float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            (Type::Float, "Ge") => {
                let cmp = format!("%pcmp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = fcmp oge float {}, {}", indent, cmp, src_val.name, rhs.name).ok();
                let ext = format!("%pce{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, ext, cmp).ok();
                writeln!(out, "{}{} = sitofp i64 {} to float", indent, v, ext).ok();
                TypedRegister { name: v.to_string(), ty: Type::Float }
            }
            // ── Bool logical ──
            (Type::Bool, "And") => {
                writeln!(out, "{}{} = and i1 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Bool }
            }
            (Type::Bool, "Or") => {
                writeln!(out, "{}{} = or i1 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Bool }
            }
            (Type::Bool, "Eq") => {
                writeln!(out, "{}{} = icmp eq i1 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Bool }
            }
            (Type::Bool, "Ne") => {
                writeln!(out, "{}{} = icmp ne i1 {}, {}", indent, v, src_val.name, rhs.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::Bool }
            }
            // ── Unknown combination — not a fast-path ──
            _ => return None,
        };
        Some(tr)
    }

    /// Emit a cached projection: load valid flag, branch on hit/miss.
    /// Hit: load cached value. Miss: compute, store in cache, set flag.
    /// Phi merges hit/miss paths. Cache slots are appended to %State by
    /// dead-field elimination (apply_field_modes).
    pub(crate) fn try_cached_projection(&mut self, out: &mut String, source_expr: &Expr,
        src_val: &TypedRegister, target_name: &str, indent: &str) -> Option<TypedRegister>
    {
        // Extract the field name from the source expression (must be a state field identifier)
        let field_name = match source_expr {
            Expr::Identifier(n) => n.clone(),
            _ => return None,
        };
        // Check if this field has a cache slot for this projection target
        let &(cache_idx, valid_idx) = self.cache_slots.get(&field_name)
            .and_then(|targets| targets.get(target_name))?;

        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        let valid_gep = format!("%cvp{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, valid_gep, valid_idx).ok();
        let valid_load = format!("%cvv{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, valid_load, valid_gep).ok();
        let valid_cond = format!("%cvc{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = icmp ne i8 {}, 0", indent, valid_cond, valid_load).ok();

        let hit_label = format!(".chit{}", self.txn_counter);
        let miss_label = format!(".cmiss{}", self.txn_counter);
        let merge_label = format!(".cmerge{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, valid_cond, hit_label, miss_label).ok();
        writeln!(out, "{}:", hit_label).ok();
        let cache_gep = format!("%cve{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, cache_gep, cache_idx).ok();
        let cache_val = format!("%cvv{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, cache_val, cache_gep).ok();
        writeln!(out, "{}br label %{}", indent, merge_label).ok();
        writeln!(out, "{}:", miss_label).ok();
        // Compute the projection value — reuses the source value as-is
        writeln!(out, "{}{} = add i64 0, {}", indent, v, src_val.name).ok();
        // Store the computed value in the cache and set valid flag
        let store_gep = format!("%cse{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, store_gep, cache_idx).ok();
        writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, v, store_gep).ok();
        let valid_store_gep = format!("%csve{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
            indent, valid_store_gep, valid_idx).ok();
        writeln!(out, "{}store i8 1, i8* {}, align 1", indent, valid_store_gep).ok();
        writeln!(out, "{}br label %{}", indent, merge_label).ok();
        writeln!(out, "{}:", merge_label).ok();
        let phi_reg = format!("%cp{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = phi i64 [ {}, %{} ], [ {}, %{} ]",
            indent, phi_reg, cache_val, hit_label, v, miss_label).ok();
        Some(TypedRegister { name: phi_reg, ty: Type::Int })
    }

    /// Phase 2: Check if the source type has a meld route for the given projection target.
    /// When a meld route exists, evaluates the route's destination expression to derive
    /// the projection result from the backing value. Handles:
    /// - `Expr::Identifier(name)` where name is a projection target → emit direct projection
    /// - `Expr::IntrinsicCall { intrinsic, args }` → emit intrinsic with args as projections
    /// - `Expr::Projection { source, target }` → emit projection with substituted source
    pub(crate) fn try_meld_projection(&mut self, out: &mut String, src_val: &TypedRegister,
        target_name: &str, indent: &str) -> Option<TypedRegister>
    {
        let custom_name = match &src_val.ty {
            crate::ast::Type::Custom(n) => n.clone(),
            _ => return None,
        };
        let universe = self.type_universe.as_ref()?;
        // Find meld — clone data to avoid borrow conflict with mutable self
        let meld_entry = universe.melds.iter().find(|((a, b), _decl)| {
            a == &custom_name || b == &custom_name
        });
        let ((name_a, name_b), meld_decl) = meld_entry?;
        let partner = if *name_a == custom_name { name_b.clone() } else { name_a.clone() };
        let route = meld_decl.routes.iter().find(|r| r.accessor == target_name)?;
        let route_dest = route.dest_expr.clone();

        let result = self.emit_route_expression(out, &route_dest, src_val, &partner, indent);
        if let Some(ref reg) = result {
            // The backing type is the meld partner — the source value is viewed through
            // the custom_name lens but the actual bits are the partner type's bits.
            self.mark_chimera(&reg.name, &partner);
        }
        result
    }

    /// Evaluate a meld route's destination expression, substituting the meld partner's
    /// type name with the actual source value and treating known projection target names
    /// as projections on the backing value.
    fn emit_route_expression(&mut self, out: &mut String, expr: &Expr,
        src_val: &TypedRegister, partner: &str, indent: &str) -> Option<TypedRegister>
    {
        match expr {
            // Pattern 1: identity projection — "Ptr" or "Size" on the backing value
            Expr::Identifier(name) if name == "Ptr" || name == "Size"
                || name == "Bytes" || name == "Alignment" || name == "Type" => {
                self.emit_direct_projection(out, src_val, name, indent)
            }
            // Pattern 2: intrinsic call — "strlen#(Ptr)" etc.
            Expr::IntrinsicCall { intrinsic, args } => {
                let v = format!("%t{}", self.txn_counter);
                self.txn_counter += 1;
                // Handle strlen#(arg) — the common meld route for CString.Size
                if let crate::ast::Intrinsic::Strlen = intrinsic {
                    if args.len() == 1 {
                        let arg_name = match &args[0] {
                            Expr::Identifier(n) => Some(n.clone()),
                            _ => None,
                        };
                        if let Some(ref name) = arg_name {
                            if name == "Ptr" || name == "Size" || name == "Bytes" {
                                let proj_reg = self.emit_direct_projection(out, src_val, name, indent)?;
                                writeln!(out, "{}{} = call i64 @__strlen__(i64 {})", indent, v, proj_reg.name).ok();
                                return Some(TypedRegister { name: v, ty: Type::Int });
                            }
                        }
                    }
                }
                None
            }
            // Pattern 3: field access on the partner type — "CString.ptr"
            Expr::FieldAccess(obj, field) => {
                if let Expr::Identifier(n) = obj.as_ref() {
                    if n == partner {
                        // Substitute with the actual source value and emit the field
                        // as the corresponding projection (Ptr, Size, etc.)
                        self.emit_direct_projection(out, src_val, field, indent)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            // Pattern 4: projection on the partner type — "CString :> Size"
            Expr::Projection { source: sub_source, target: sub_target } => {
                let sub_name = match sub_source.as_ref() {
                    Expr::Identifier(n) => Some(n.clone()),
                    _ => None,
                };
                if let Some(ref name) = sub_name {
                    if name == partner {
                        // Substitute with the actual source value and emit the projection
                        // without going through the meld check again (avoid recursion)
                        let target_name = crate::analysis::transition_graph::projection_target_name(sub_target);
                        self.emit_direct_projection(out, src_val, &target_name, indent)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Phase 3: Decay a chimera value to its canonical type at a boundary.
    /// When `target_ty` is `None`, assumes decay to the backing type (identity).
    /// Real field-level materialization will be added per type pair.
    pub(crate) fn emit_decay(&mut self, out: &mut String, val: &TypedRegister,
        target_ty: Option<&Type>, indent: &str) -> TypedRegister
    {
        if !self.is_chimera(&val.name) {
            return val.clone();
        }
        let backing = match self.chimera_backing(&val.name) {
            Some(b) => b.to_string(),
            None => return val.clone(),
        };
        let target_name = match target_ty {
            Some(Type::Custom(n)) => n.clone(),
            _ => return val.clone(), // primitive target → identity (bits are valid)
        };
        if backing == target_name {
            // Decay to own backing type — identity
            return val.clone();
        }
        // Generic materialization: look up the meld between backing and target,
        // derive each field of the target type from the backing value via routes.
        // Clone all data first to avoid borrow conflicts with mutable self.
        let meld_routes: Vec<crate::ast::MeldRouteDef> = {
            let universe = match self.type_universe.as_ref() {
                Some(u) => u,
                None => return val.clone(),
            };
            match universe.find_meld(&backing, &target_name) {
                Some(m) => m.routes.clone(),
                None => return val.clone(),
            }
        };
        let target_fields = match self.struct_types.get(&target_name) {
            Some(f) => f.clone(),
            None => return val.clone(),
        };

        // Derive each field value from the backing via meld routes
        let mut field_results: Vec<(String, Type, String)> = Vec::new(); // name, ty, reg
        for (field_name, field_ty) in &target_fields {
            if let Some(route) = meld_routes.iter().find(|r| r.accessor == *field_name) {
                // Use backing as partner — the route evaluates from backing's perspective
                if let Some(reg) = self.emit_route_expression(out, &route.dest_expr, val, &backing, indent) {
                    field_results.push((field_name.clone(), field_ty.clone(), reg.name));
                } else {
                    // Route evaluation failed — emit 0 as placeholder
                    let v = format!("%t{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    field_results.push((field_name.clone(), field_ty.clone(), v));
                }
            } else {
                // No route for this field — emit 0 as placeholder
                let v = format!("%t{}", self.txn_counter);
                self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                field_results.push((field_name.clone(), field_ty.clone(), v));
            }
        }

        if field_results.is_empty() {
            return val.clone();
        }

        // For single-field types: return the field value directly
        if field_results.len() == 1 {
            let (_, ref ty, ref reg) = field_results[0];
            return TypedRegister { name: reg.clone(), ty: ty.clone() };
        }

        // For multi-field types: allocate a struct on the heap
        let total_size = field_results.len() * 8; // each field is i64
        let alloc = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = call i8* @malloc(i64 {})", indent, alloc, total_size).ok();
        let struct_ptr = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        writeln!(out, "{}{} = bitcast i8* {} to i64*", indent, struct_ptr, alloc).ok();

        for (i, (_name, _ty, reg)) in field_results.iter().enumerate() {
            let gep = format!("%t{}", self.txn_counter);
            self.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, gep, struct_ptr, i).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8, !tbaa !1", indent, reg, gep).ok();
        }

        let struct_ptr_name = format!("%t{}", self.txn_counter - 1);
        TypedRegister { name: struct_ptr_name, ty: Type::Custom(target_name.clone()) }
    }

    /// Emit a direct projection on a value without going through the meld route check.
    /// This avoids infinite recursion when a meld route maps to the same projection target.
    fn emit_direct_projection(&mut self, out: &mut String, src_val: &TypedRegister,
        target_name: &str, indent: &str) -> Option<TypedRegister>
    {
        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        match target_name {
            "Ptr" => {
                writeln!(out, "{}{} = add i64 0, {} ; ptr", indent, v, src_val.name).ok();
                Some(TypedRegister { name: v, ty: Type::Int })
            }
            "Size" => {
                let hp = format!("%drphp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, src_val.name).ok();
                let lp = format!("%drplp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, lp).ok();
                Some(TypedRegister { name: v, ty: Type::Int })
            }
            "Bytes" => {
                writeln!(out, "{}{} = add i64 0, 8 ; bytes", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::Int })
            }
            "Alignment" => {
                writeln!(out, "{}{} = add i64 0, 8 ; alignment", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::Int })
            }
            "Type" => {
                writeln!(out, "{}{} = add i64 0, 6 ; type=custom", indent, v).ok();
                Some(TypedRegister { name: v, ty: Type::Int })
            }
            _ => None,
        }
    }
}
