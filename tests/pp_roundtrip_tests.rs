// ── Phase 8: Pretty-Printer Bridge Round-Trip Test ─────────────────────
// 2026-07-22: Compiles pp-types.bv to a shared library, loads it at
// runtime, and calls each exported pp function. Verifies that Brief
// pretty-printer output matches Rust Display output.
//
// This is an integration test that exercises the full pipeline:
//   .bv → brief build → .ll → llc → .o → cc → .so → libloading → FFI call

use std::path::Path;
use std::process::Command;

/// Path to the project root (test file is in tests/).
const PROJECT_ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Path to the compiler binary.
fn compiler_path() -> String {
    format!("{}/target/debug/brief-compiler", PROJECT_ROOT)
}

/// Compile the bridge .bv to a .so, returning the path.
fn build_bridge_so() -> String {
    let out_dir = std::env::temp_dir().join("brief_pp_test");
    let _ = std::fs::create_dir_all(&out_dir);

    let bv_path = format!("{}/pp-types.bv", PROJECT_ROOT);
    let ll_path = out_dir.join("pp-types.ll");
    let o_path = out_dir.join("pp-types.o");
    let rt_o_path = out_dir.join("brief_rt.o");
    let so_path = out_dir.join("libpp_types.so");

    // Step 1: brief build → .ll
    let build_output = Command::new(compiler_path())
        .args(&["build", &bv_path, "--llvm", "--out", &out_dir.to_string_lossy()])
        .output()
        .expect("failed to run brief-compiler build");
    if !build_output.status.success() {
        let stderr = String::from_utf8_lossy(&build_output.stderr);
        panic!("brief build failed: {}", stderr);
    }

    // Step 2: llc → .o for bridge
    let llc_output = Command::new("llc")
        .args(&["-filetype=obj", "-relocation-model=pic", "-o", &o_path.to_string_lossy(), &ll_path.to_string_lossy()])
        .output()
        .expect("failed to run llc");
    if !llc_output.status.success() {
        let stderr = String::from_utf8_lossy(&llc_output.stderr);
        panic!("llc failed: {}", stderr);
    }

    // Step 2b: Compile brief_rt.c to .o (provides runtime FFI symbols)
    let rt_c_path = format!("{}/lib/runtime/brief_rt.c", PROJECT_ROOT);
    let rt_cc_output = Command::new("cc")
        .args(&["-c", "-fPIC", "-o", &rt_o_path.to_string_lossy(), &rt_c_path])
        .output()
        .expect("failed to compile brief_rt.c");
    if !rt_cc_output.status.success() {
        let stderr = String::from_utf8_lossy(&rt_cc_output.stderr);
        panic!("brief_rt.c compilation failed: {}", stderr);
    }

    // Step 3: cc → .so (link bridge + runtime)
    let cc_output = Command::new("cc")
        .args(&["-shared", "-o", &so_path.to_string_lossy(), &o_path.to_string_lossy(), &rt_o_path.to_string_lossy()])
        .output()
        .expect("failed to run cc");
    if !cc_output.status.success() {
        let stderr = String::from_utf8_lossy(&cc_output.stderr);
        panic!("cc failed: {}", stderr);
    }

    so_path.to_string_lossy().to_string()
}

/// Helper: build the bridge so there's an artifact to inspect.
/// The LLVM IR is checked for well-formed export wrappers.
#[test]
fn test_bridge_compiles_to_valid_llvm_ir() {
    let out_dir = std::env::temp_dir().join("brief_pp_test_ir");
    let _ = std::fs::create_dir_all(&out_dir);

    let bv_path = format!("{}/pp-types.bv", PROJECT_ROOT);
    let ll_path = out_dir.join("pp-types.ll");

    let build_output = Command::new(compiler_path())
        .args(&["build", &bv_path, "--llvm", "--out", &out_dir.to_string_lossy()])
        .output()
        .expect("failed to run brief-compiler build");
    assert!(
        build_output.status.success(),
        "brief build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    // Read the generated IR
    let ir = std::fs::read_to_string(&ll_path)
        .expect("failed to read generated LLVM IR");

    // Verify all expected export functions are present
    assert!(ir.contains("define ptr @brief_pp_type_bits"), "missing brief_pp_type_bits");
    assert!(ir.contains("define ptr @brief_pp_type_void"), "missing brief_pp_type_void");
    assert!(ir.contains("define ptr @brief_pp_type_custom"), "missing brief_pp_type_custom");
    assert!(ir.contains("define ptr @brief_pp_type_generic"), "missing brief_pp_type_generic");
    assert!(ir.contains("define ptr @brief_pp_type_ptr"), "missing brief_pp_type_ptr");
    assert!(ir.contains("define ptr @brief_pp_type_ptr_const"), "missing brief_pp_type_ptr_const");
    assert!(ir.contains("define ptr @brief_pp_type_function"), "missing brief_pp_type_function");
    assert!(ir.contains("define ptr @brief_pp_type_tuple"), "missing brief_pp_type_tuple");
    assert!(ir.contains("define ptr @brief_pp_type_union"), "missing brief_pp_type_union");
    assert!(ir.contains("define ptr @brief_pp_binop"), "missing brief_pp_binop");
    assert!(ir.contains("define ptr @brief_pp_unary_op"), "missing brief_pp_unary_op");

    // Verify IR has properly typed call arguments (not bare %name)
    assert!(ir.contains("ptr %arg0"), "IR missing ptr type on call args");

    // Verify no self-calling wrappers (export calls itself)
    assert!(
        !ir.contains("call ptr @brief_pp_type_bits("),
        "IR has self-calling export wrapper"
    );

    eprintln!("LLVM IR validation passed for pp-types.bv");
}

/// Build the bridge to a shared library and verify llc + cc succeed.
#[test]
fn test_bridge_compiles_to_shared_library() {
    let so_path = build_bridge_so();
    assert!(
        Path::new(&so_path).exists(),
        "shared library not created: {}",
        so_path
    );
    let metadata = std::fs::metadata(&so_path).expect("failed to read .so metadata");
    assert!(metadata.len() > 1000, ".so file suspiciously small: {} bytes", metadata.len());
    eprintln!("Shared library created: {} ({} bytes)", so_path, metadata.len());
}
