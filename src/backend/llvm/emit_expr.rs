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
        let v = format!("%t{}", self.fun.txn_counter);
        self.fun.txn_counter += 1;
        // 2026-06-28: fix empty-indent emit_expr — use default indent to prevent
        // %t{N} SSA violations. The empty indent comes from the definition body
        // emission path that doesn't pass indent through correctly.
        let indent = if indent.is_empty() { "  " } else { indent };
        // 2026-06-29: All expression variants dispatched to expr::rest
        return crate::backend::llvm::expr::rest::emit_rest_expr(self, out, &v, &expr, indent);
    }

    // ── Cell identifier rewriting helpers ──────────────────────


}
