// GLUE Integration Tests
//
// Tests the GLUE pipeline end-to-end using real registry files and bridge files.
// These run as `cargo test --test glue_test` (separate binary from lib tests).
// Only uses `pub` items from the library crate.

use std::path::Path;

// ============ DBVL Registry — Real Files ============

#[test]
fn test_glue_dbvl_parses() {
    let source = std::fs::read_to_string("lib/glue.dbvl")
        .expect("glue.dbvl should exist at project root");
    let file = brief_compiler::glue::dbvl_reader::parse_dbvl(&source);
    assert!(!file.entries.is_empty(), "glue.dbvl should have at least 1 entry");
    assert!(file.schema_path.is_some(), "glue.dbvl should have a schema directive");
}

#[test]
fn test_glue_dbvs_parses() {
    let source = std::fs::read_to_string("lib/glue.dbvs")
        .expect("glue.dbvs should exist at project root");
    let schema = brief_compiler::glue::dbvs_validator::parse_schema(&source)
        .expect("glue.dbvs should parse successfully");
    assert!(!schema.fields.is_empty(), "glue.dbvs should define at least 1 field");
    // Schema defines field types, not named entry types — just verify it parses
}

// ============ Adapter Lookup ============

#[test]
fn test_find_adapter_rust() {
    let result = brief_compiler::glue::export::find_adapter("rust", Path::new("lib/glue.dbvl"));
    assert!(result.is_ok(), "rust adapter should be found: {:?}", result.err());
    let adapter = result.unwrap();
    assert_eq!(adapter.language, "rust");
    assert!(!adapter.macro_path.is_empty());
    assert_eq!(adapter.file_extension, "rs");
    assert!(adapter.type_map.contains_key("Int"));
    assert_eq!(adapter.type_map.get("Int"), Some(&"i64".to_string()));
}

#[test]
fn test_find_adapter_python() {
    let result = brief_compiler::glue::export::find_adapter("python", Path::new("lib/glue.dbvl"));
    assert!(result.is_ok(), "python adapter should be found: {:?}", result.err());
    let adapter = result.unwrap();
    assert_eq!(adapter.language, "python");
    assert_eq!(adapter.file_extension, "py");
    assert_eq!(adapter.type_map.get("Int"), Some(&"int".to_string()));
}

#[test]
fn test_find_adapter_node() {
    let result = brief_compiler::glue::export::find_adapter("node", Path::new("lib/glue.dbvl"));
    assert!(result.is_ok(), "node adapter should be found: {:?}", result.err());
    let adapter = result.unwrap();
    assert_eq!(adapter.language, "node");
    assert_eq!(adapter.file_extension, "js");
    assert_eq!(adapter.type_map.get("Int"), Some(&"number".to_string()));
}

#[test]
fn test_find_adapter_nonexistent() {
    let result = brief_compiler::glue::export::find_adapter("cobol", Path::new("lib/glue.dbvl"));
    assert!(result.is_err(), "cobol adapter should not exist");
}

// ============ Bridge Info Extraction ============

#[test]
fn test_extract_bridge_info_from_test_bridge() {
    use brief_compiler::glue::export::extract_bridge_info;
    use brief_compiler::parser::Parser;

    let source = std::fs::read_to_string("examples/test-bridge.bv")
        .expect("test-bridge.bv should exist");
    let mut parser = Parser::new(&source);
    let program = parser.parse()
        .expect("test-bridge.bv should parse");

    let info = extract_bridge_info(&program, "test-bridge");
    assert_eq!(info.name, "test-bridge");
    assert_eq!(info.exports.len(), 2, "should find 2 #export functions");
    assert_eq!(info.frgns.len(), 0, "no frgn declarations in test bridge");

    let export_names: Vec<&str> = info.exports.iter().map(|e| e.name.as_str()).collect();
    assert!(export_names.contains(&"add"), "should have 'add' export");
    assert!(export_names.contains(&"multiply"), "should have 'multiply' export");

    // Check a specific export's fields
    let add_export = info.exports.iter().find(|e| e.name == "add").unwrap();
    assert_eq!(add_export.return_type, "Int");
    assert_eq!(add_export.params.len(), 2);
    assert_eq!(add_export.params[0].1, "Int");
    assert_eq!(add_export.params[1].1, "Int");
}

// ============ Link Pipeline ============

#[test]
fn test_link_generate_bridge_bv() {
    use brief_compiler::glue::link::{ForeignFunction, LinkResult, generate_bridge_bv};

    let result = LinkResult {
        library_path: "test.a".to_string(),
        functions: vec![
            ForeignFunction {
                name: "sqrt".to_string(),
                symbol: "sqrt".to_string(),
                is_intrinsic: true,
                intrinsic_name: Some("sqrt".to_string()),
            },
            ForeignFunction {
                name: "custom_op".to_string(),
                symbol: "custom_op".to_string(),
                is_intrinsic: false,
                intrinsic_name: None,
            },
        ],
    };
    let output = generate_bridge_bv(&result);
    assert!(output.contains("Auto-generated bridge for test.a"));
    assert!(output.contains("intrinsic: sqrt#()"));
    assert!(output.contains("frgn custom_op(...) -> Int ;"));
}
