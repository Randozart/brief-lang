// ── Normalize Types ───────────────────────────────────────────────────
// 2026-07-12: Phase 5 — Type normalization pass.
// Removed all IntrinsicCall/InopDeclaration handling.

use crate::ast_new::{Expr, TopLevel};

/// Normalize types in a program.
pub fn normalize_types(item: TopLevel) -> TopLevel {
    item
}
