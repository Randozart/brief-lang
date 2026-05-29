use std::process::Command;

/// LLVM backend integration test.
///
/// Compiles a .bv file to .ll via the Rust backend, then runs
/// `opt -verify` on the output to ensure valid LLVM IR.
/// Requires `llc` and `opt` in PATH.
fn compile_and_verify_llvm(source: &str, name: &str) -> Result<String, String> {
    // Parse + generate via the full pipeline (same as `brief llvm`)
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_brief-compiler"))
        .args(["llvm", "--out", "/tmp"])
        .arg(format!("{}.bv", name))
        .output()
        .map_err(|e| format!("Failed to run brief-compiler: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("brief-compiler failed: {}", stderr));
    }

    let ll_path = format!("/tmp/{}.ll", name);
    let ll_content = std::fs::read_to_string(&ll_path)
        .map_err(|e| format!("Failed to read {}: {}", ll_path, e))?;

    // Run opt -verify
    let verify = Command::new("opt")
        .args(["-verify", &ll_path, "-o", "/dev/null"])
        .output()
        .map_err(|e| format!("opt -verify failed: {}", e))?;

    if !verify.status.success() {
        let stderr = String::from_utf8_lossy(&verify.stderr);
        return Err(format!("LLVM verification failed: {}", stderr));
    }

    // Run opt -O3
    let optimize = Command::new("opt")
        .args(["-O3", &ll_path, "-o", "/dev/null"])
        .output()
        .map_err(|e| format!("opt -O3 failed: {}", e))?;

    if !optimize.status.success() {
        let stderr = String::from_utf8_lossy(&optimize.stderr);
        return Err(format!("LLVM -O3 failed: {}", stderr));
    }

    Ok(ll_content)
}

#[test]
fn test_llvm_backend_basic_counter() {
    match compile_and_verify_llvm("tests/fixtures/counter.bv", "counter") {
        Ok(ir) => {
            assert!(ir.contains("%State"), "Output should contain %State type");
            assert!(ir.contains("noalias"), "Output should contain noalias");
            assert!(ir.contains("ret void"), "Output should contain ret void");
        }
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn test_llvm_backend_multifield() {
    match compile_and_verify_llvm("tests/fixtures/multifield.bv", "multifield") {
        Ok(ir) => {
            assert!(ir.contains("%State"), "Output should contain %State type");
            assert!(ir.contains("increment"), "Output should contain increment transaction");
            assert!(ir.contains("toggle"), "Output should contain toggle transaction");
        }
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn test_llvm_backend_minimal() {
    match compile_and_verify_llvm("tests/fixtures/minimal.bv", "minimal") {
        Ok(ir) => {
            assert!(ir.contains("%State"), "Output should contain %State type");
        }
        Err(e) => panic!("{}", e),
    }
}
