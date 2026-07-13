// ── Desugarer ──────────────────────────────────────────────────────────
// 2026-07-12: Phase 5 — Desugaring pass (removed IntrinsicCall handling).
// No IntrinsicCall or InopDeclaration — all # intrinsics are Call variants.

use crate::ast_new::{Expr, Statement, TopLevel};

/// Desugar a program's AST: normalize patterns, etc.
pub fn desugar(items: Vec<TopLevel>) -> Vec<TopLevel> {
    items
}
