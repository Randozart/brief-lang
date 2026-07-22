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

use std::fmt::Write;

use crate::analysis::frgn_dispatch::{ProtocolStep, TransformKind};
use crate::ast::top::Fallback;
use crate::ast::Type;

/// Emit the protocol transform chain for a single value.
///
/// 2026-07-22: Applies a sequence of protocol transforms to convert a
/// value from one type representation to another. Each step in the path
/// is emitted as the appropriate LLVM IR operation.
///
/// Returns the register name holding the final transformed value.
///
/// Stub: Returns the value unchanged. Full implementation when the
/// meld/identity/cast emission helpers are wired in.
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
            }
            TransformKind::ProtocolTransform(ref _category) => {
                // 2026-07-22: Stub — CastTo/CastFrom inline emission.
            }
        }
    }
    Ok(current_reg)
}

/// Emit LLVM IR for the fallback dispatch phi-node structure.
///
/// 2026-07-22: Gives the call result a null-check and fallback value
/// using a phi node. The structure emitted is:
///
/// ```llvm
///   %ok = icmp ne <ret_ty> %call_result, zeroinitializer
///   br i1 %ok, label %use_result_N, label %use_fallback_N
///
/// use_result_N:
///   br label %merge_N
///
/// use_fallback_N:
///   %fb_N = ... (fallback value)
///   br label %merge_N
///
/// merge_N:
///   %final = phi <ret_ty> [%call_result, %use_result_N], [%fb_N, %use_fallback_N]
/// ```
///
/// For void returns, no contract check is emitted — the call is always used.
///
/// # Parameters
/// * `out` — The LLVM IR string buffer to write to
/// * `call_reg` — The register holding the call result
/// * `ret_type` — The Brief return type (used to determine void vs non-void)
/// * `ret_llvm_ty` — The LLVM type string (e.g., "i64", "double", "void")
/// * `fallback` — The fallback strategy to use
/// * `indent` — Whitespace indentation for IR lines
/// * `gen_reg` — Closure that generates unique register names (e.g., `%t42`)
pub fn emit_fallback_llvm(
    out: &mut String,
    call_reg: &str,
    ret_type: &Type,
    ret_llvm_ty: &str,
    fallback: &Fallback,
    indent: &str,
    gen_reg: &mut dyn FnMut() -> String,
) -> Result<String, String> {
    if *ret_type == Type::Void {
        // 2026-07-22: Void return — no contract check needed.
        // The call result is used unconditionally.
        return Ok(call_reg.to_string());
    }

    // 2026-07-22: Generate unique label suffixes using gen_reg counter.
    let label_suffix = gen_reg();
    let use_result_label = format!("use_result{}", label_suffix);
    let use_fallback_label = format!("use_fallback{}", label_suffix);
    let merge_label = format!("merge{}", label_suffix);

    // 2026-07-22: Emit contract check: non-null comparison.
    let ok_reg = gen_reg();
    writeln!(
        out,
        "{}  {} = icmp ne {} {}, {} zeroinitializer",
        indent, ok_reg, ret_llvm_ty, call_reg, ret_llvm_ty
    ).ok();
    writeln!(
        out,
        "{}  br i1 {}, label %{}, label %{}",
        indent, ok_reg, use_result_label, use_fallback_label
    ).ok();

    // 2026-07-22: use_result block — just branch to merge.
    writeln!(out, "\n{}:", use_result_label).ok();
    writeln!(out, "{}  br label %{}", indent, merge_label).ok();

    // 2026-07-22: use_fallback block — compute and use the fallback value.
    writeln!(out, "\n{}:", use_fallback_label).ok();
    let fb_reg = emit_fallback_value(out, ret_llvm_ty, fallback, indent, gen_reg)?;
    writeln!(out, "{}  br label %{}", indent, merge_label).ok();

    // 2026-07-22: merge block — phi of the two incoming values.
    let final_reg = gen_reg();
    writeln!(out, "\n{}:", merge_label).ok();
    writeln!(
        out,
        "{}  {} = phi {} [ {}, %{} ], [ {}, %{} ]",
        indent, final_reg, ret_llvm_ty, call_reg, use_result_label,
        fb_reg, use_fallback_label
    ).ok();

    Ok(final_reg)
}

/// Emit the fallback value as LLVM IR based on the fallback strategy.
///
/// 2026-07-22: Returns the register name holding the fallback value.
/// Each branch generates a different form:
/// * `Static(expr)` — A constant expression
/// * `FnCall(name, args)` — A call to a Brief function
/// * `Implicit` — Zero-value (void-return equivalent, for completeness)
/// * `None` — Zero-initializer of the return type
fn emit_fallback_value(
    out: &mut String,
    ret_llvm_ty: &str,
    fallback: &Fallback,
    indent: &str,
    gen_reg: &mut dyn FnMut() -> String,
) -> Result<String, String> {
    match fallback {
        Fallback::Static(_expr) => {
            // 2026-07-22: Emit a zero-initializer for the return type.
            // Full constant expression evaluation will be added in Phase 7.
            let fb_reg = gen_reg();
            writeln!(
                out,
                "{}  {} = {} zeroinitializer",
                indent, fb_reg, ret_llvm_ty
            ).ok();
            Ok(fb_reg)
        }
        Fallback::FnCall(name, _args) => {
            // 2026-07-22: Emit a call to a fallback function with zero-initialized args.
            // Full argument emission will be added in Phase 7.
            let fb_reg = gen_reg();
            writeln!(
                out,
                "{}  {} = call {} @{}({} zeroinitializer)",
                indent, fb_reg, ret_llvm_ty, name, ret_llvm_ty
            ).ok();
            Ok(fb_reg)
        }
        Fallback::Implicit | Fallback::None => {
            // 2026-07-22: Zero-value of the return type.
            let fb_reg = gen_reg();
            writeln!(
                out,
                "{}  {} = {} zeroinitializer",
                indent, fb_reg, ret_llvm_ty
            ).ok();
            Ok(fb_reg)
        }
    }
}

/// Emit a contract check + fallback dispatch.
///
/// 2026-07-22: Wraps a call result with a contract check and fallback
/// value. Delegates to `emit_fallback_llvm` with no-op generators for
/// backward compatibility.
///
/// Deprecated: Prefer `emit_fallback_llvm` directly.
pub fn emit_fallback_wrapper(
    call_result_reg: &str,
    fallback: &crate::ast::top::Fallback,
) -> Result<String, String> {
    // 2026-07-22: For the no-context stub, just return the call result.
    if *fallback == Fallback::None || *fallback == Fallback::Implicit {
        Ok(call_result_reg.to_string())
    } else {
        Ok(call_result_reg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::frgn_dispatch::TransformKind;
    use crate::ast::top::Fallback;
    use crate::ast::Type;

    /// Helper: creates a mutable closure for register generation.
    fn test_gen_reg() -> Box<dyn FnMut() -> String> {
        let mut counter = 0u64;
        Box::new(move || {
            let n = counter;
            counter += 1;
            format!("%t{}", n)
        })
    }

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
    fn test_fallback_static_literal_llvm() {
        let mut out = String::new();
        let mut gen_reg = test_gen_reg();
        let result = emit_fallback_llvm(
            &mut out, "%call", &Type::int(), "i64",
            &Fallback::Static(crate::ast::Expr::Decimal(42)),
            "  ", &mut gen_reg,
        ).unwrap();
        assert!(out.contains("icmp ne"), "should emit contract check: {}", out);
        assert!(out.contains("phi"), "should emit phi: {}", out);
        assert!(out.contains("zeroinitializer"), "should emit fallback value: {}", out);
        assert!(result.starts_with("%t"), "result should be a register");
    }

    #[test]
    fn test_fallback_fn_call_llvm() {
        let mut out = String::new();
        let mut gen_reg = test_gen_reg();
        let result = emit_fallback_llvm(
            &mut out, "%call", &Type::int(), "i64",
            &Fallback::FnCall("default_val".to_string(), vec![]),
            "  ", &mut gen_reg,
        ).unwrap();
        assert!(out.contains("call"), "should emit call: {}", out);
        assert!(out.contains("phi"), "should emit phi: {}", out);
        assert!(result.starts_with("%t"), "result should be a register");
    }

    #[test]
    fn test_fallback_implicit_llvm() {
        let mut out = String::new();
        let mut gen_reg = test_gen_reg();
        let result = emit_fallback_llvm(
            &mut out, "%call", &Type::int(), "i64",
            &Fallback::Implicit,
            "  ", &mut gen_reg,
        ).unwrap();
        assert!(out.contains("icmp ne"), "should emit contract check: {}", out);
        assert!(out.contains("phi"), "should emit phi: {}", out);
        assert!(result.starts_with("%t"), "result should be a register");
    }

    #[test]
    fn test_fallback_none_llvm() {
        let mut out = String::new();
        let mut gen_reg = test_gen_reg();
        let result = emit_fallback_llvm(
            &mut out, "%call", &Type::int(), "i64",
            &Fallback::None,
            "  ", &mut gen_reg,
        ).unwrap();
        assert!(out.contains("icmp ne"), "should emit contract check: {}", out);
        assert!(out.contains("phi"), "should emit phi: {}", out);
        assert!(result.starts_with("%t"), "result should be a register");
    }

    #[test]
    fn test_fallback_void_ret_no_check() {
        let mut out = String::new();
        let mut gen_reg = test_gen_reg();
        let result = emit_fallback_llvm(
            &mut out, "%call", &Type::Void, "void",
            &Fallback::None,
            "  ", &mut gen_reg,
        ).unwrap();
        assert!(!out.contains("icmp"), "void should skip contract check");
        assert_eq!(result, "%call", "void returns the call reg directly");
    }

    #[test]
    fn test_fallback_wrapper_noop() {
        let result = emit_fallback_wrapper("%result", &Fallback::None).unwrap();
        assert_eq!(result, "%result");
    }

    #[test]
    fn test_fallback_wrapper_implicit() {
        let result = emit_fallback_wrapper("%result", &Fallback::Implicit).unwrap();
        assert_eq!(result, "%result");
    }
}
