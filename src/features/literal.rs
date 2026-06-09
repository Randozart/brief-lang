// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

use crate::ast::Type;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralExpr {
    Integer(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Term,
}

impl ExprTypecheck for LiteralExpr {
    fn typecheck(&self, ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        match self {
            LiteralExpr::Integer(_) => Ok(Type::Int),
            LiteralExpr::Float(_) => {
                if ctx.target == crate::typechecker::CompilationTarget::Verilog {
                    // Verilog doesn't support native floats
                }
                Ok(Type::Float)
            }
            LiteralExpr::String(_) => Ok(Type::String),
            LiteralExpr::Char(_) => Ok(Type::Char),
            LiteralExpr::Bool(_) => Ok(Type::Bool),
            LiteralExpr::Term => Ok(Type::Void),
        }
    }
}

impl ExprEval for LiteralExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        match self {
            LiteralExpr::Integer(v) => Ok(Value::Int(*v)),
            LiteralExpr::Float(v) => Ok(Value::Float(*v)),
            LiteralExpr::String(v) => Ok(Value::String(v.clone())),
            LiteralExpr::Char(v) => Ok(Value::Char(*v)),
            LiteralExpr::Bool(v) => Ok(Value::Bool(*v)),
            LiteralExpr::Term => ctx
                .state
                .get("term")
                .cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable("term".to_string())),
        }
    }
}

impl ExprCodegenLLVM for LiteralExpr {
    fn emit_llvm(
        &self,
        ctx: &mut crate::backend::llvm::LlvmBackend,
        out: &mut String,
        _dispatch: &ExprDispatch,
    ) -> crate::backend::llvm::TypedRegister {
        let v = format!("%t{}", ctx.txn_counter);
        ctx.txn_counter += 1;
        match self {
            LiteralExpr::Integer(n) => {
                writeln!(out, "{indent}{v} = add i64 0, {n}", indent = "", v = v, n = n).ok();
                crate::backend::llvm::TypedRegister { name: v, ty: Type::Int }
            }
            LiteralExpr::Bool(b) => {
                let val = if *b { 1 } else { 0 };
                writeln!(out, "{indent}{v} = add i64 0, {val}", indent = "", v = v, val = val).ok();
                crate::backend::llvm::TypedRegister { name: v, ty: Type::Bool }
            }
            LiteralExpr::Float(f) => {
                let bits = crate::backend::llvm::float_to_llvm_hex(*f);
                let fl = format!("%ff{}", ctx.txn_counter); ctx.txn_counter += 1;
                writeln!(out, "{indent}{fl} = bitcast i32 {bits} to float", indent = "", fl = fl, bits = bits).ok();
                let i32r = format!("%fi{}", ctx.txn_counter); ctx.txn_counter += 1;
                writeln!(out, "{indent}{i32r} = bitcast float {fl} to i32", indent = "", i32r = i32r, fl = fl).ok();
                writeln!(out, "{indent}{v} = zext i32 {i32r} to i64", indent = "", v = v, i32r = i32r).ok();
                ctx.reg_float_cache.insert(v.clone(), fl.clone());
                crate::backend::llvm::TypedRegister { name: v, ty: Type::Float }
            }
            LiteralExpr::String(s) => {
                let si = ctx.string_constants.iter().position(|x| x == s).unwrap_or(0);
                let g = format!("@str.{}", si);
                let p = format!("%sp{}", ctx.txn_counter); ctx.txn_counter += 1;
                writeln!(out, "{indent}{p} = getelementptr inbounds [{len} x i8], [{len} x i8]* {g}, i64 0, i64 0",
                    indent = "", p = p, len = s.len() + 1, g = g).ok();
                writeln!(out, "{indent}{v} = ptrtoint i8* {p} to i64", indent = "", v = v, p = p).ok();
                crate::backend::llvm::TypedRegister { name: v, ty: Type::String }
            }
            LiteralExpr::Char(c) => {
                let ci = format!("%cc{}", ctx.txn_counter); ctx.txn_counter += 1;
                writeln!(out, "{indent}{ci} = add i32 0, {cval}", indent = "", ci = ci, cval = *c as i32).ok();
                writeln!(out, "{indent}{v} = zext i32 {ci} to i64", indent = "", v = v, ci = ci).ok();
                crate::backend::llvm::TypedRegister { name: v, ty: Type::Char }
            }
            LiteralExpr::Term => {
                writeln!(out, "{indent}{v} = add i64 0, 0", indent = "", v = v).ok();
                crate::backend::llvm::TypedRegister { name: v, ty: Type::Int }
            }
        }
    }
}

impl LiteralExpr {
    pub fn format(&self) -> String {
        match self {
            LiteralExpr::Integer(n) => n.to_string(),
            LiteralExpr::Float(f) => f.to_string(),
            LiteralExpr::String(s) => format!("\"{}\"", s),
            LiteralExpr::Char(c) => format!("'{}'", c.escape_default()),
            LiteralExpr::Bool(b) => b.to_string(),
            LiteralExpr::Term => "term".to_string(),
        }
    }
}

#[cfg(kani)]
mod kani_tests {
    use super::*;
    use crate::ast::Expr;

    #[kani::proof]
    fn verify_expr_as_integer_dual_path() {
        let old = Expr::Integer(42);
        let new = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        assert_eq!(old.as_integer(), new.as_integer());
    }

    #[kani::proof]
    fn verify_expr_as_bool_dual_path() {
        let old = Expr::Bool(true);
        let new = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        assert_eq!(old.as_bool(), new.as_bool());
    }

    #[kani::proof]
    fn verify_expr_is_term_dual_path() {
        let old = Expr::Term;
        let new = Expr::Literal(Box::new(LiteralExpr::Term));
        assert_eq!(old.is_term(), new.is_term());
    }

    #[kani::proof]
    fn verify_expr_as_integer_none_for_non_int() {
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        assert_eq!(expr.as_integer(), None);
    }

    #[kani::proof]
    fn verify_expr_is_term_false_for_non_term() {
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(0)));
        assert!(!expr.is_term());
    }
}

// ── Full: format/float/string tests (may involve Display/formatting loops) ──
#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    use crate::ast::Expr;

    #[kani::proof]
    fn verify_literal_format_no_panic() {
        let lit = LiteralExpr::Integer(42);
        let s = lit.format();
        assert!(!s.is_empty());
    }

    #[kani::proof]
    fn verify_expr_as_float_dual_path() {
        let old = Expr::Float(3.14);
        let new = Expr::Literal(Box::new(LiteralExpr::Float(3.14)));
        assert_eq!(old.as_float(), new.as_float());
        let f = new.as_float();
        assert!(f.is_some() && (f.unwrap() - 3.14).abs() < 1e-10);
    }

    #[kani::proof]
    fn verify_expr_as_string_dual_path() {
        let old = Expr::String("hello".to_string());
        let new = Expr::Literal(Box::new(LiteralExpr::String("hello".to_string())));
        assert_eq!(old.as_string(), new.as_string());
        assert_eq!(new.as_string(), Some(&"hello".to_string()));
    }
}

impl ExprCodegenVHDL for LiteralExpr {
    fn emit_vhdl(&self, _ctx: &crate::backend::vhdl::VhdlGenerator, _dispatch: &ExprDispatch) -> String {
        match self {
            LiteralExpr::Bool(b) => (if *b { "'1'" } else { "'0'" }).to_string(),
            LiteralExpr::Integer(i) => i.to_string(),
            LiteralExpr::Float(f) => f.to_string(),
            LiteralExpr::String(s) => format!("\"{}\"", s),
            LiteralExpr::Char(c) => format!("character'val({})", *c as u32),
            LiteralExpr::Term => "true".to_string(),
        }
    }
}

impl ExprCodegenWebstack for LiteralExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        match self {
            LiteralExpr::Integer(n) => format!("JsValue::from({})", n),
            LiteralExpr::Bool(true) => "JsValue::TRUE".to_string(),
            LiteralExpr::Bool(false) => "JsValue::FALSE".to_string(),
            LiteralExpr::String(s) => format!("JsValue::from(\"{}\")", s),
            LiteralExpr::Float(f) => format!("JsValue::from({})", f),
            LiteralExpr::Char(c) => format!("JsValue::from(\"{}\")", c.escape_default()),
            LiteralExpr::Term => "JsValue::undefined".to_string(),
        }
    }
}
