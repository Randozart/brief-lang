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

// ── Pattern B: Feature Traits ──────────────────────────────────────────
//
// One trait per concern, per backend. Separate traits = separate compilation
// units (changing VHDL emission never recompiles LLVM codegen).
//
// Each feature struct implements only the traits relevant to it. Missing
// backend impls fall through to the router's default stub.
//
// All sub-expression recursion goes through the dispatch reference, not
// directly into the pass files.

use crate::ast::{Expr, Type};
use crate::errors::TypeError;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::parser::Parser;
use crate::typechecker::TypeChecker;

/// Router handle for sub-expression dispatch.
/// Feature structs call `dispatch.infer_expr()` / `dispatch.eval_expr()` etc.
pub struct ExprDispatch;

/// Parse: Parser routes tokens to feature struct constructors.
pub trait ExprParse {
    type Output;
    fn parse(parser: &mut Parser) -> Self::Output;
}

/// Typecheck: Typechecker routes each Expr variant to its feature.
pub trait ExprTypecheck {
    fn typecheck(
        &self,
        ctx: &mut TypeChecker,
        dispatch: &ExprDispatch,
    ) -> Result<Type, TypeError>;
}

/// Eval: Interpreter routes each Expr variant to its feature.
pub trait ExprEval {
    fn evaluate(
        &self,
        ctx: &mut Interpreter,
        dispatch: &ExprDispatch,
    ) -> Result<Value, RuntimeError>;
}

/// LLVM Codegen — feature structs reference &mut LlvmBackend directly.
pub trait ExprCodegenLLVM {
    fn emit_llvm(
        &self,
        ctx: &mut crate::backend::llvm::LlvmBackend,
        out: &mut String,
        dispatch: &ExprDispatch,
    ) -> crate::backend::llvm::TypedRegister;
}

/// Webstack Codegen. Stateless string builder — takes `&WebstackGenerator`.
pub trait ExprCodegenWebstack {
    fn emit_js(
        &self,
        ctx: &crate::backend::webstack::WebstackGenerator,
        dispatch: &ExprDispatch,
    ) -> String;
}

// ── Statement Feature Traits ───────────────────────────────────────

/// Router handle for sub-statement and sub-expression dispatch.
pub struct StmtDispatch;

/// Typecheck: Typechecker routes each Statement variant to its feature.
pub trait StmtTypecheck {
    fn typecheck(
        &self,
        ctx: &mut crate::typechecker::TypeChecker,
        dispatch: &StmtDispatch,
    ) -> Result<(), crate::errors::TypeError>;
}

/// Eval: Interpreter routes each Statement variant to its feature.
pub trait StmtEval {
    fn evaluate(
        &self,
        ctx: &mut crate::interpreter::Interpreter,
        dispatch: &StmtDispatch,
    ) -> Result<(), crate::interpreter::RuntimeError>;
}

/// LLVM Codegen — feature structs reference &mut LlvmBackend directly.
pub trait StmtCodegenLLVM {
    fn emit_llvm(
        &self,
        ctx: &mut crate::backend::llvm::LlvmBackend,
        out: &mut String,
        dispatch: &StmtDispatch,
        indent: &str,
    );
}

/// Webstack Codegen.
pub trait StmtCodegenWebstack {
    fn emit_js(
        &self,
        ctx: &mut crate::backend::webstack::WebstackGenerator,
        out: &mut String,
        dispatch: &StmtDispatch,
    );
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_expr_dispatch_constructable() {
        let d = ExprDispatch;
        let _ = &d;
    }

    #[kani::proof]
    fn verify_expr_codegen_llvm_trait_satisfied() {
        fn assert_trait<T: ExprCodegenLLVM>() {}
        assert_trait::<crate::features::literal::LiteralExpr>();
    }

    #[kani::proof]
    fn verify_expr_codegen_webstack_trait_satisfied() {
        fn assert_trait<T: ExprCodegenWebstack>() {}
        assert_trait::<crate::features::literal::LiteralExpr>();
    }

    #[kani::proof]
    fn verify_expr_eval_trait_satisfied() {
        fn assert_trait<T: ExprEval>() {}
        assert_trait::<crate::features::literal::LiteralExpr>();
    }

    #[kani::proof]
    fn verify_expr_typecheck_trait_satisfied() {
        fn assert_trait<T: ExprTypecheck>() {}
        assert_trait::<crate::features::literal::LiteralExpr>();
    }
}
