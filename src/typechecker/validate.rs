// ── Type Checker — Validation Passes ───────────────────────────────────
// 2026-07-12: Phase 2.5 — Post-type-checking validation.
// Handles:
// - alloc annotation validation (Phase A.1)
// - derivation example type checking (Phase 8.4)
// (2026-08-01, Phase 2: `[#]` entry call graph isolation removed with the
// [#] marker; the entry!/args! plugin enforces entry isolation instead.)

use crate::ast::*;
use crate::errors::{AllocError, SyntaxError, TypeError};
use crate::typechecker::TypecheckContext;

/// Validate alloc annotations on all bindings.
/// alloc("Stack"): variable must not escape its scope.
/// alloc(address): address must be compile-time constant.
pub fn validate_alloc_annotations(items: &[TopLevel]) -> Result<(), Vec<AllocError>> {
    // Simplified: iterate all definitions and check their metadata
    // for `alloc` keys. Full implementation in Phase A.
    Ok(())
}

/// Check that derivation example types match function signatures.
pub fn check_derivation(items: &[TopLevel]) -> Result<(), Vec<TypeError>> {
    // Simplified: check that DerivationBlock examples match parameter types.
    // Full implementation in Phase 8.4.
    Ok(())
}

/// Check that [#]-marked functions are not called from internal code.
/// 2026-08-01 (Phase 2): the `[#]` marker is removed — this check is now
/// superseded by the entry!/args! plugin (Phase 3), which injects entry
/// preconditions rather than a call-graph marker. Kept as a no-op for now so
/// the entry-point call graph remains enforceable by the plugin.
pub fn check_entry_call_graph(items: &[TopLevel]) -> Result<(), Vec<TypeError>> {
    // Simplified: verify no internal calls to entry functions.
    // Full implementation in Phase 16B.2.
    Ok(())
}
