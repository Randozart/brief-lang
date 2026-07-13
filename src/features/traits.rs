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

// 2026-07-13: Stripped to only the webstack traits needed by active backends.
// Removed ExprTypecheck, ExprEval, ExprParse, StmtTypecheck, StmtEval,
// ExprCodegenLLVM, StmtCodegenLLVM, and ExprCodegenVHDL — these types
// (TypeChecker, Interpreter, Parser, LlvmBackend) no longer exist as
// single concrete structs. Webstack is the only active backend consuming
// feature trait dispatch.

/// Router handle for sub-expression dispatch.
pub struct ExprDispatch;

/// Webstack codegen. Stateless string builder — takes `&WebstackGenerator`.
pub trait ExprCodegenWebstack {
    fn emit_js(
        &self,
        ctx: &crate::backend::webstack::WebstackGenerator,
        dispatch: &ExprDispatch,
    ) -> String;
}

/// Router handle for sub-statement dispatch.
pub struct StmtDispatch;

/// Webstack statement codegen.
pub trait StmtCodegenWebstack {
    fn emit_js(
        &self,
        ctx: &mut crate::backend::webstack::WebstackGenerator,
        out: &mut String,
        dispatch: &StmtDispatch,
    );
}

#[cfg(all(feature = "kani", feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_expr_dispatch_constructable() {
        let d = ExprDispatch;
        let _ = &d;
    }

    #[kani::proof]
    fn verify_expr_codegen_webstack_trait_satisfied() {
        fn assert_trait<T: ExprCodegenWebstack>() {}
        assert_trait::<crate::features::literal::LiteralExpr>();
    }
}
