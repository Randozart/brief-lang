// Fallback Unit Tests
//
// 2026-07-22: Tests fallback parsing, dispatch logic, and integration
// with the frgn pipeline. Covers static literals, fn calls, and implicit
// fallbacks.

use brief_compiler::analysis::frgn_dispatch::{resolve_single_frgn, ResolvedFrgn};
use brief_compiler::ast::top::{Fallback, ForeignBinding, ForeignTarget, FromSpec};
use brief_compiler::ast::Expr;
use brief_compiler::glue::config::GlueTarget;
use std::collections::HashMap;

fn sample_glue_targets() -> HashMap<String, GlueTarget> {
    HashMap::from([(
        "python".to_string(),
        GlueTarget {
            language: "python".to_string(),
            types_module: std::path::PathBuf::from("glue/python/types.bv"),
            extension: "py".to_string(),
            bridge_kind: "native_module".to_string(),
            calling_convention: "c_abi".to_string(),
            module_init: false,
            protocols: HashMap::new(),
            templates: HashMap::new(),
            conversions: brief_compiler::glue::config::Conversions::default(),
            state: brief_compiler::glue::config::StateAbi::default(),
            param_decl: "{name}: {type}".to_string(),
        },
    )])
}

fn make_frgn(name: &str, ext: &str, fallback: Fallback) -> ForeignBinding {
    ForeignBinding::new(
        name.to_string(),
        None,
        FromSpec::Literal(std::path::PathBuf::from(format!("lib.{}", ext))),
        ForeignTarget::Native,
        fallback,
    )
}

// ── Fallback Enum Tests ─────────────────────────────────────────────────

#[test]
fn test_fallback_none_default() {
    let fb = make_frgn("get_val", "so", Fallback::None);
    assert!(matches!(fb.fallback, Fallback::None));
}

#[test]
fn test_fallback_static_literal() {
    let fb = make_frgn("get_val", "py", Fallback::Static(Expr::Decimal(0)));
    assert!(matches!(fb.fallback, Fallback::Static(Expr::Decimal(0))));
}

#[test]
fn test_fallback_fn_call() {
    let fb = make_frgn(
        "get_val",
        "py",
        Fallback::FnCall("default".to_string(), vec![Expr::Decimal(42)]),
    );
    match &fb.fallback {
        Fallback::FnCall(name, args) => {
            assert_eq!(name, "default");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("Expected FnCall fallback"),
    }
}

#[test]
fn test_fallback_implicit() {
    let fb = make_frgn("void_func", "py", Fallback::Implicit);
    assert!(matches!(fb.fallback, Fallback::Implicit));
}

// ── Fallback + Dispatch ─────────────────────────────────────────────────

#[test]
fn test_fallback_none_in_bridge_dispatch() {
    let fb = make_frgn("get_val", "py", Fallback::None);
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "py", &targets, brief_compiler::target::BackendKind::Llvm, None)
            .unwrap();
    match result {
        ResolvedFrgn::Bridge { ref fallback, .. } => {
            assert!(matches!(fallback, Fallback::None));
        }
        other => panic!("Expected Bridge, got {:?}", other),
    }
}

#[test]
fn test_fallback_static_in_bridge_dispatch() {
    let fb = make_frgn("get_val", "py", Fallback::Static(Expr::Decimal(0)));
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "py", &targets, brief_compiler::target::BackendKind::Llvm, None)
            .unwrap();
    match result {
        ResolvedFrgn::Bridge { ref fallback, .. } => {
            assert!(matches!(fallback, Fallback::Static(Expr::Decimal(0))));
        }
        other => panic!("Expected Bridge, got {:?}", other),
    }
}

#[test]
fn test_fallback_fn_call_in_bridge_dispatch() {
    let fb = make_frgn(
        "get_val",
        "py",
        Fallback::FnCall("default".to_string(), vec![]),
    );
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "py", &targets, brief_compiler::target::BackendKind::Llvm, None)
            .unwrap();
    match result {
        ResolvedFrgn::Bridge { ref fallback, .. } => {
            assert!(matches!(fallback, Fallback::FnCall(..)));
        }
        other => panic!("Expected Bridge, got {:?}", other),
    }
}

// ── Fallback + Inline Frgn ──────────────────────────────────────────────

#[test]
fn test_fallback_with_inline_frgn() {
    let fb = make_frgn("get_val", "c", Fallback::Static(Expr::Decimal(42)));
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "c", &targets, brief_compiler::target::BackendKind::Llvm, None)
            .unwrap();
    match result {
        ResolvedFrgn::Inline { symbol, .. } => {
            assert_eq!(symbol, "get_val");
            // Inline dispatch preserves fallback on the ForeignBinding
        }
        other => panic!("Expected Inline, got {:?}", other),
    }
}

// ── Extension Mapping ───────────────────────────────────────────────────

#[test]
fn test_extension_mapping_for_fallback_brige() {
    let fb = make_frgn("node_func", "mjs", Fallback::None);
    let targets = sample_glue_targets(); // no node in this sample
    let result =
        resolve_single_frgn(&fb, "mjs", &targets, brief_compiler::target::BackendKind::Llvm, None)
            .unwrap();
    assert!(
        matches!(result, ResolvedFrgn::Unsupported(_)),
        "mjs should be unsupported without node in glue targets"
    );
}

// ── Doc comment: these tests verify the fallback round-trip from creation
// through dispatch, exercising the integration between ForeignBinding and
// ResolvedFrgn::Bridge. The full IR emission tests for fallback phi-node
// structure are in src/glue/bridge.rs unit tests (lib crate).
