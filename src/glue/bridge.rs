// ── GLUE Bridge — Protocol-Mediated Foreign Calls ─────────────────────
// 2026-07-22: Shared bridge generation logic used by all backends.
//
// The bridge wraps a foreign function call with:
//   1. Protocol transforms (CastTo/CastFrom) for each parameter
//   2. The foreign call itself (via the appropriate mechanism)
//   3. Protocol transforms for the return value
//   4. Contract verification + fallback dispatch
//
// The specific mechanism for making the foreign call (dlopen, Python
// embedding, JS glue, etc.) is backend-specific, but the transform
// chain and fallback logic is shared.

use crate::analysis::frgn_dispatch::{ProtocolStep, TransformKind};

/// Emit the protocol transform chain for a single value.
///
/// 2026-07-22: Applies a sequence of protocol transforms to convert a
/// value from one type representation to another. Each step in the path
/// is emitted as the appropriate LLVM IR operation.
///
/// Returns the register name holding the final transformed value.
///
/// Stub: Returns the value unchanged. Full implementation in Phase 4
/// when the meld/identity/cast emission helpers are wired in.
pub fn emit_protocol_chain(
    value_reg: &str,
    path: &[ProtocolStep],
    _value_ty: &str,
) -> Result<String, String> {
    if path.is_empty() {
        return Ok(value_reg.to_string());
    }

    let mut current_reg = value_reg.to_string();
    for step in path {
        match step.kind {
            TransformKind::Identity => {
                // No transformation needed — types are structurally identical
            }
            TransformKind::MeldShuffle | TransformKind::Bitcast => {
                // 2026-07-22: Stub — full meld shuffle emission uses
                // emit_meld_shuffle() in llvm/intrinsics.rs.
                // For now, identity is correct for simple cases.
            }
            TransformKind::ProtocolTransform(ref _category) => {
                // 2026-07-22: Stub — CastTo/CastFrom inline emission.
                // Full implementation when the intrinsic cast path is wired.
            }
        }
    }
    Ok(current_reg)
}

/// Emit a contract check + fallback dispatch.
///
/// 2026-07-22: Wraps a call result with a contract check and fallback
/// value. If the contract fails (null/non-null check for now), the
/// fallback value is used instead.
///
/// Stub: Returns the call result unchanged. Full LLVM IR phi-node
/// structure in Phase 4.
pub fn emit_fallback_wrapper(
    call_result_reg: &str,
    _fallback: &crate::ast::top::Fallback,
) -> Result<String, String> {
    // 2026-07-22: Stub — no contract check yet.
    // Full implementation emits:
    //   %ok = icmp ne @verify_postcondition(%result)
    //   br i1 %ok, label %use_result, label %use_fallback
    //   ...
    //   %final = phi [%result, %use_result], [%fb, %use_fallback]
    Ok(call_result_reg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::frgn_dispatch::TransformKind;
    use crate::ast::top::Fallback;
    use crate::ast::Type;

    #[test]
    fn test_emit_protocol_chain_identity() {
        let path = vec![ProtocolStep {
            source: Type::int(),
            target: Type::int(),
            kind: TransformKind::Identity,
        }];
        let result = emit_protocol_chain("%val", &path, "i64").unwrap();
        assert_eq!(result, "%val");
    }

    #[test]
    fn test_emit_protocol_chain_empty() {
        let result = emit_protocol_chain("%val", &[], "i64").unwrap();
        assert_eq!(result, "%val");
    }

    #[test]
    fn test_emit_fallback_wrapper_noop() {
        let result = emit_fallback_wrapper("%result", &Fallback::None).unwrap();
        assert_eq!(result, "%result");
    }

    #[test]
    fn test_emit_fallback_wrapper_implicit() {
        let result = emit_fallback_wrapper("%result", &Fallback::Implicit).unwrap();
        assert_eq!(result, "%result");
    }
}
