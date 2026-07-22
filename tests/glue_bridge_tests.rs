// GLUE Bridge Unit Tests
//
// 2026-07-22: Tests protocol chain computation, bridge codegen helpers,
// and protocol path resolution using the TOML-based GLUE registry.

use std::collections::HashMap;
use std::path::Path;

use brief_compiler::analysis::frgn_dispatch::{
    compute_protocol_path, extension_to_language, resolve_single_frgn, ResolvedFrgn, TransformKind,
};
use brief_compiler::ast::top::{Fallback, ForeignBinding, ForeignTarget, FromSpec};
use brief_compiler::ast::Type;
use brief_compiler::glue::config::{find_language_by_extension, load_glue_config, GlueTarget};

fn sample_glue_targets() -> HashMap<String, GlueTarget> {
    let mut c_type_map = HashMap::new();
    c_type_map.insert("Int".to_string(), "int64_t".to_string());
    c_type_map.insert("Float".to_string(), "double".to_string());
    HashMap::from([
        (
            "python".to_string(),
            GlueTarget {
                language: "python".to_string(),
                types_module: std::path::PathBuf::from("glue/python/types.bv"),
                extension: "py".to_string(),
                bridge_kind: "native_module".to_string(),
                calling_convention: "c_abi".to_string(),
                c_type_map: c_type_map.clone(),
            },
        ),
        (
            "rust".to_string(),
            GlueTarget {
                language: "rust".to_string(),
                types_module: std::path::PathBuf::from("glue/rust/types.bv"),
                extension: "rs".to_string(),
                bridge_kind: "extern_c_crate".to_string(),
                calling_convention: "lto".to_string(),
                c_type_map: HashMap::new(),
            },
        ),
    ])
}

fn make_frgn(name: &str, ext: &str, as_name: Option<&str>, fallback: Fallback) -> ForeignBinding {
    ForeignBinding::new(
        name.to_string(),
        as_name.map(|s| s.to_string()),
        FromSpec::Literal(std::path::PathBuf::from(format!("lib.{}", ext))),
        ForeignTarget::Native,
        fallback,
    )
}

// ── Protocol Path Tests ────────────────────────────────────────────────

#[test]
fn test_compute_protocol_path_identity() {
    let int_type = Type::int();
    let path = compute_protocol_path(&int_type, &int_type).unwrap();
    assert_eq!(path.len(), 1, "identity path should have 1 step");
    assert!(matches!(path[0].kind, TransformKind::Identity));
}

#[test]
fn test_compute_protocol_path_bitcast() {
    let a = Type::Custom("A".to_string());
    let b = Type::Custom("B".to_string());
    let path = compute_protocol_path(&a, &b).unwrap();
    assert_eq!(path.len(), 1, "bitcast path should have 1 step");
    assert!(matches!(path[0].kind, TransformKind::Bitcast));
}

// ── Extension-to-Language Resolution ────────────────────────────────────

#[test]
fn test_find_language_by_extension_python() {
    let targets = sample_glue_targets();
    let found = find_language_by_extension(&targets, "py");
    assert!(found.is_some(), ".py should map to a language target");
    assert_eq!(found.unwrap().language, "python");
}

#[test]
fn test_find_language_by_extension_rust() {
    let targets = sample_glue_targets();
    let found = find_language_by_extension(&targets, "rs");
    assert!(found.is_some(), ".rs should map to a language target");
    assert_eq!(found.unwrap().language, "rust");
}

#[test]
fn test_find_language_by_extension_unknown() {
    let targets = sample_glue_targets();
    let found = find_language_by_extension(&targets, "kt");
    assert!(found.is_none(), ".kt should not be in any language target");
}

#[test]
fn test_extension_to_language_llvm_py() {
    assert_eq!(
        extension_to_language("py", brief_compiler::target::BackendKind::Llvm),
        Some("python")
    );
}

#[test]
fn test_extension_to_language_llvm_rs() {
    assert_eq!(
        extension_to_language("rs", brief_compiler::target::BackendKind::Llvm),
        Some("rust")
    );
}

#[test]
fn test_extension_to_language_circt() {
    assert_eq!(
        extension_to_language("py", brief_compiler::target::BackendKind::Circt),
        None
    );
}

#[test]
fn test_extension_to_language_with_dot() {
    assert_eq!(
        extension_to_language(".py", brief_compiler::target::BackendKind::Llvm),
        Some("python")
    );
}

// ── Resolve Single Frgn ─────────────────────────────────────────────────

#[test]
fn test_resolve_single_frgn_inline_c() {
    let fb = make_frgn("my_func", "c", None, Fallback::None);
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "c", &targets, brief_compiler::target::BackendKind::Llvm)
            .unwrap();
    match result {
        ResolvedFrgn::Inline { symbol, compile_source } => {
            assert_eq!(symbol, "my_func");
            assert!(compile_source);
        }
        other => panic!("Expected Inline, got {:?}", other),
    }
}

#[test]
fn test_resolve_single_frgn_bridge_python() {
    let fb = make_frgn("py_func", "py", None, Fallback::None);
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "py", &targets, brief_compiler::target::BackendKind::Llvm)
            .unwrap();
    match result {
        ResolvedFrgn::Bridge { language, .. } => {
            assert_eq!(language, "python");
        }
        other => panic!("Expected Bridge, got {:?}", other),
    }
}

#[test]
fn test_resolve_single_frgn_unsupported() {
    let fb = make_frgn("kotlin_func", "kt", None, Fallback::None);
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "kt", &targets, brief_compiler::target::BackendKind::Llvm)
            .unwrap();
    match result {
        ResolvedFrgn::Unsupported(msg) => {
            assert!(msg.contains("kt"), "msg should mention extension: {}", msg);
        }
        other => panic!("Expected Unsupported, got {:?}", other),
    }
}

#[test]
fn test_resolve_single_frgn_empty_extension() {
    let fb = make_frgn("no_ext", "", None, Fallback::None);
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "", &targets, brief_compiler::target::BackendKind::Llvm).unwrap();
    match result {
        ResolvedFrgn::Unsupported(msg) => {
            assert!(msg.contains("no file extension"), "msg: {}", msg);
        }
        other => panic!("Expected Unsupported, got {:?}", other),
    }
}

#[test]
fn test_resolve_single_frgn_with_as() {
    let fb = make_frgn("brief_name", "c", Some("foreign_sym"), Fallback::None);
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "c", &targets, brief_compiler::target::BackendKind::Llvm)
            .unwrap();
    match result {
        ResolvedFrgn::Inline { symbol, .. } => {
            assert_eq!(symbol, "foreign_sym");
        }
        other => panic!("Expected Inline, got {:?}", other),
    }
}

#[test]
fn test_resolve_single_frgn_native_so() {
    let fb = make_frgn("native_fn", "so", None, Fallback::None);
    let targets = sample_glue_targets();
    let result =
        resolve_single_frgn(&fb, "so", &targets, brief_compiler::target::BackendKind::Llvm)
            .unwrap();
    match result {
        ResolvedFrgn::Inline { symbol, compile_source } => {
            assert_eq!(symbol, "native_fn");
            assert!(!compile_source);
        }
        other => panic!("Expected Inline, got {:?}", other),
    }
}

// ── TOML Config Loading ─────────────────────────────────────────────────

#[test]
fn test_load_glue_config_shiped() {
    let config = load_glue_config(None).expect("should load built-in lib/glue.toml");
    assert!(config.contains_key("python"), "should have python entry");
    assert!(config.contains_key("rust"), "should have rust entry");
    let python = config.get("python").unwrap();
    assert_eq!(python.extension, "py");
    assert_eq!(python.calling_convention, "c_abi");
    assert_eq!(
        python.c_type_map.get("Int"),
        Some(&"int64_t".to_string())
    );
}

#[test]
fn test_load_glue_config_custom_path() {
    let custom = Path::new("lib/glue.toml");
    let config = load_glue_config(Some(custom)).expect("should load from custom path");
    assert!(config.contains_key("python"));
}
