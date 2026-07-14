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
                writeln!(out, "{}{} = fadd double 0.0, {}", indent, v, f).ok();
                TypedRegister { name: v.to_string(), ty: Type::float64() }
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
                // Local binding lookup — if not found, emit as global ref
                if let Some(reg) = self.get_local(name) {
                    TypedRegister { name: reg.clone(), ty: self.get_local_type(name) }
                } else {
                    writeln!(out, "{}{} = load i64, ptr @{}", indent, v, name).ok();
                    TypedRegister { name: v.to_string(), ty: Type::int() }
                }
            }

            // ── Call ─────────────────────────────────────────────────
            // 2026-07-12: Intrinsic call if name ends with '#', else user call.
            Expr::Call(name, args) => {
                if name.ends_with('#') {
                    self.emit_intrinsic_call_dispatch(out, v, name, args, indent)
                } else {
                    self.emit_user_call(out, v, name, args, indent)
                }
            }

            // ── BinaryOp ─────────────────────────────────────────────
            Expr::BinaryOp(kind, lhs, rhs) => {
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
                if let Some(first) = exprs.first() {
                    self.emit_expr(out, first, indent)
                } else {
                    TypedRegister { name: v.to_string(), ty: Type::void() }
                }
            }

            // ── List literal ─────────────────────────────────────────
            Expr::List(exprs) => {
                // Emit a stack allocation for the list
                let count = exprs.len();
                let alloca = self.fun.gen_reg();
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, alloca, count).ok();
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, alloca).ok();
                TypedRegister { name: v.to_string(), ty: Type::ptr(Type::int()) }
            }

            // ── Field access ─────────────────────────────────────────
            Expr::Field(obj, field) => {
                let obj_reg = self.emit_expr(out, obj, indent);
                // Struct field access via extractvalue or GEP
                writeln!(out, "{}{} = extractvalue {} {}, {}", indent, v,
                    lower_type(&obj_reg.ty), obj_reg.name, field).ok();
                TypedRegister { name: v.to_string(), ty: Type::int() }
            }

            // ── Index ────────────────────────────────────────────────
            Expr::Index(obj, index) => {
                let obj_reg = self.emit_expr(out, obj, indent);
                let idx_reg = self.emit_expr(out, index, indent);
                writeln!(out, "{}{} = extractelement {} {}, {}",
                    indent, v, lower_type(&obj_reg.ty), obj_reg.name, idx_reg.name).ok();
                TypedRegister { name: v.to_string(), ty: Type::int() }
            }

            // ── Cast ─────────────────────────────────────────────────
            Expr::Cast(expr, target) => {
                let src = self.emit_expr(out, expr, indent);
                let target_ll = lower_type(target);
                if target_ll == "double" {
                    writeln!(out, "{}{} = sitofp i64 {} to double", indent, v, src.name).ok();
                } else if target_ll == "i64" && src.ty == Type::float64() {
                    writeln!(out, "{}{} = fptosi double {} to i64", indent, v, src.name).ok();
                } else {
                    writeln!(out, "{}{} = bitcast {} {} to {}", indent, v, lower_type(&src.ty), src.name, target_ll).ok();
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

            // ── DerivationBlock / PropertyGet / FormattingAnnotation ─
            Expr::DerivationBlock(_) | Expr::PropertyGet(_) | Expr::FormattingAnnotation(_) => {
                writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                TypedRegister { name: v.to_string(), ty: Type::void() }
            }
        }
    }

    // ── Sub-helpers ──────────────────────────────────────────────────

    /// Emit a string literal as a global constant + GEP.
    fn emit_string_literal(&mut self, out: &mut String, v: &str, bytes: &[u8], indent: &str) -> TypedRegister {
        let str_name = format!(".str{}", self.fun.gen_reg());
        let escaped: String = bytes.iter().flat_map(|&b| std::ascii::escape_default(b)).map(|c| c as char).collect();
        writeln!(out, "{}@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1",
            indent, str_name, bytes.len() + 1, escaped).ok();
        let gep = self.fun.gen_reg();
        writeln!(out, "{}{} = getelementptr inbounds ([{} x i8], ptr @{}, i32 0, i32 0)",
            indent, gep, bytes.len() + 1, str_name).ok();
        TypedRegister { name: gep, ty: Type::string() }
    }

    /// Emit a user function call.
    fn emit_user_call(&mut self, out: &mut String, v: &str, name: &str, args: &[Expr], indent: &str) -> TypedRegister {
        let arg_regs: Vec<String> = args.iter()
            .map(|a| self.emit_expr(out, a, indent).name)
            .collect();
        writeln!(out, "{}{} = call i64 @{}({})", indent, v, name, arg_regs.join(", ")).ok();
        TypedRegister { name: v.to_string(), ty: Type::int() }
    }

    /// Emit a binary operation.
    fn emit_binary_op(&mut self, out: &mut String, v: &str,
        kind: &crate::ast::BinaryOpKind, l: &TypedRegister, r: &TypedRegister, indent: &str) -> TypedRegister {
        let is_float = l.ty == Type::float() || r.ty == Type::float() 
            || l.ty == Type::float64() || r.ty == Type::float64();
        let ty_str = if is_float { "double" } else { "i64" };
        let fast = if is_float { " fast" } else { "" };
        match kind {
            crate::ast::BinaryOpKind::Add => {
                writeln!(out, "{}{} = fadd{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
                TypedRegister { name: v.to_string(), ty: if is_float { Type::float() } else { Type::int() } }
            }
            crate::ast::BinaryOpKind::Sub => {
                writeln!(out, "{}{} = fsub{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
                TypedRegister { name: v.to_string(), ty: if is_float { Type::float() } else { Type::int() } }
            }
            crate::ast::BinaryOpKind::Mul => {
                writeln!(out, "{}{} = fmul{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
                TypedRegister { name: v.to_string(), ty: if is_float { Type::float() } else { Type::int() } }
            }
            crate::ast::BinaryOpKind::Div => {
                if is_float {
                    writeln!(out, "{}{} = fdiv{} {} {}, {}", indent, v, fast, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = sdiv i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: if is_float { Type::float() } else { Type::int() } }
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
                if is_float {
                    writeln!(out, "{}{} = fcmp olt {} {}, {}", indent, v, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Le => {
                if is_float {
                    writeln!(out, "{}{} = fcmp ole {} {}, {}", indent, v, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Gt => {
                if is_float {
                    writeln!(out, "{}{} = fcmp ogt {} {}, {}", indent, v, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, v, l.name, r.name).ok();
                }
                TypedRegister { name: v.to_string(), ty: Type::bool_() }
            }
            crate::ast::BinaryOpKind::Ge => {
                if is_float {
                    writeln!(out, "{}{} = fcmp oge {} {}, {}", indent, v, ty_str, l.name, r.name).ok();
                } else {
                    writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, v, l.name, r.name).ok();
                }
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
                writeln!(out, "{}{} = sub i64 0, {}", indent, v, operand.name).ok();
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
        indent: &str,
    ) -> TypedRegister {
        emit_intrinsic_call(self, out, v, name, args, indent)
    }
}
