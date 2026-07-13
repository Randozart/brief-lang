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

use crate::features::traits::{ExprCodegenWebstack, ExprDispatch};

// 2026-07-13: Stripped to only webstack-related code. Removed ExprTypecheck,
// ExprEval, and ExprCodegenLLVM impls — those types are now handled directly
// by the interpreter/typechecker passes or the LLVM backend.
// The LiteralExpr enum remains as a compat type referenced by many backends.

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralExpr {
    Integer(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Term,
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
