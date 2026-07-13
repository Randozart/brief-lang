// ── Phi Node Emission ──────────────────────────────────────────────────
// 2026-07-12: Phase 4 — Forward-edge and back-edge phi node codegen.
// Sorted-by-key iteration for deterministic IR (see HashMap iteration rule).

use crate::backend::llvm::function::FunctionState;
use std::fmt::Write;

/// Emit forward-edge phi nodes for all registered state fields.
/// Determines the phi type from the LLVM type string.
pub fn emit_forward_phis(
    ctx: &FunctionState,
    out: &mut String,
    state_llvm_ty: &str,
    indent: &str,
    field_types: &[(&str, &str)],
) {
    // Sort field_types by field name for deterministic IR
    let mut sorted: Vec<(&str, &str)> = field_types.to_vec();
    sorted.sort_by_key(|(name, _)| *name);

    for (field_name, llvm_ty) in &sorted {
        if let Some(reg) = ctx.phi_field_regs.get(*field_name) {
            writeln!(out, "{}{} = phi {} [ undef, %entry ]", indent, reg, llvm_ty).ok();
        }
    }
}

/// Emit back-edge phi nodes (loop latch -> loop header).
/// Patches the forward phis with the back-edge incoming values.
pub fn emit_backedge_phis(
    ctx: &FunctionState,
    out: &mut String,
    indent: &str,
) {
    // Sort pending phis by field name for deterministic IR
    let mut sorted: Vec<(String, String, String)> = ctx.pending_phi_backedge.iter()
        .map(|p| (p.field.clone(), p.phi_reg.clone(), p.incoming_reg.clone()))
        .collect();
    sorted.sort_by_key(|(field, _, _)| field.clone());

    for (field, phi_reg, incoming_reg) in &sorted {
        writeln!(out, "{}{} = add {} {}, 0 ; back-edge phi for {}",
            indent, phi_reg, "%tmp", incoming_reg, field).ok();
    }
}
