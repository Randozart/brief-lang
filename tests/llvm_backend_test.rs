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
        .arg(source)
        .output()
        .map_err(|e| format!("Failed to run brief-compiler: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("brief-compiler failed: {}", stderr));
    }

    let ll_path = format!("/tmp/{}.ll", name);
    let ll_content = std::fs::read_to_string(&ll_path)
        .map_err(|e| format!("Failed to read {}: {}", ll_path, e))?;

    // Run opt -verify (new PM syntax: -passes=verify)
    let verify = Command::new("opt")
        .args(["-passes=verify", &ll_path, "-o", "/dev/null"])
        .output()
        .map_err(|e| format!("opt -verify failed: {}", e))?;

    if !verify.status.success() {
        let stderr = String::from_utf8_lossy(&verify.stderr);
        return Err(format!("LLVM verification failed: {}", stderr));
    }

    // Run opt -O3 (new PM syntax: -passes='default<O3>')
    let optimize = Command::new("opt")
        .args(["-passes=default<O3>", &ll_path, "-o", "/dev/null"])
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

#[test]
fn test_llvm_backend_wake_triggers() {
    match compile_and_verify_llvm("tests/fixtures/wake_triggers.bv", "wake_triggers") {
        Ok(ir) => {
            assert!(ir.contains("@llvm.wake_triggers = constant [2 x i8*]"),
                "Should have wake triggers metadata with 2 symbols");
            assert!(ir.contains("__sigint_flag"),
                "Should reference __sigint_flag");
            assert!(ir.contains("__sigterm_flag"),
                "Should reference __sigterm_flag");
            assert!(ir.contains("call void @__rt_wait()"),
                "main() should call __rt_wait()");
            assert!(ir.contains("declare void @__rt_wait()"),
                "__rt_wait should always be declared");
        }
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn test_llvm_backend_sync_block() {
    match compile_and_verify_llvm("tests/test_sync_block.bv", "test_sync_block") {
        Ok(ir) => {
            assert!(ir.contains("%State"), "Output should contain %State type");
            assert!(ir.contains("test_sync"), "Should contain sync test transaction");
            assert!(ir.contains("ret void"), "Main should have ret void");
        }
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn test_llvm_backend_no_wake_busy_loop() {
    match compile_and_verify_llvm("tests/fixtures/minimal.bv", "minimal") {
        Ok(ir) => {
            assert!(!ir.contains("call void @__rt_init()"),
                "No wake triggers should not call __rt_init()");
            assert!(!ir.contains("call void @__rt_wait()"),
                "No wake triggers should not call __rt_wait()");
            assert!(!ir.contains("@llvm.wake_triggers"),
                "No wake triggers should not emit @llvm.wake_triggers");
        }
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn test_llvm_backend_inop_sadd() {
    match compile_and_verify_llvm("tests/fixtures/inop_sadd.bv", "inop_sadd") {
        Ok(ir) => {
            assert!(ir.contains("define i64 @sadd"),
                "LLVM IR should contain definition of @sadd");
            assert!(ir.contains("add i64 %a, %b"),
                "LLVM IR should contain the inop add instruction");
            assert!(ir.contains("ret i64 %res"),
                "LLVM IR should contain term→ret lowering");
        }
        Err(e) => panic!("{}", e),
    }
}

#[test]
fn test_llvm_backend_inop_divmod() {
    match compile_and_verify_llvm("tests/fixtures/inop_divmod.bv", "inop_divmod") {
        Ok(ir) => {
            assert!(ir.contains("define { i64, i64 } @divmod"),
                "LLVM IR should contain definition of @divmod with struct return");
            assert!(ir.contains("sdiv i64 %a, %b"),
                "LLVM IR should contain the sdiv instruction");
            assert!(ir.contains("srem i64 %a, %b"),
                "LLVM IR should contain the srem instruction");
            assert!(ir.contains("insertvalue"),
                "LLVM IR should use insertvalue for struct construction");
        }
        Err(e) => panic!("{}", e),
    }
}
