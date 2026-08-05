# Plan D: End-to-end smoke test

> Created: 2026-05-30T14:15Z
> Status: Draft — ready for implementation
> Depends on: Plan A (willreturn), Plan B (auto-link) — but can be implemented independently

## Problem

There is no end-to-end test that compiles a `.bv` file with `#io` wake triggers through the full pipeline (parse → desugar → typecheck → generate → verify LLVM IR) and asserts the output contains the expected wake-trigger patterns (`@llvm.wake_triggers`, `__rt_init()`, `__rt_wait()`).

The existing `tests/llvm_backend_test.rs` compiles `.bv` fixtures and runs `opt -verify` + `opt -O3`, but does not assert on specific IR patterns for wake triggers.

## Goal

Extend the existing integration test infrastructure to cover the wake-trigger codegen path end-to-end.

## Implementation

### Step 1: Create fixture file

**File**: `tests/fixtures/wake_triggers.bv`

```briv
#!dispatch(parallel)
#io sigint;
#io sigterm;
node handle_sigint [sigint] { term; };
node handle_sigterm [sigterm] { term; };
```

This creates two wake triggers (`__sigint_flag`, `__sigterm_flag`) and two reactive transactions.

### Step 2: Extend `tests/llvm_backend_test.rs`

Read the existing file:
```
/home/randozart/Desktop/Projects/briv-compiler/tests/llvm_backend_test.rs
```

Add a new test function:

```rust
#[test]
fn test_wake_trigger_llvm_output() -> Result<(), Box<dyn std::error::Error>> {
    let bv_path = PathBuf::from("tests/fixtures/wake_triggers.bv");
    let source = std::fs::read_to_string(&bv_path)?;

    // Parse
    let mut parser = Parser::new(&source);
    let mut program = parser.parse().map_err(|e| format!("Parse error: {}", e))?;

    // Desugar
    let mut desug = Desugarer::new();
    let program = desug.desugar(&program);

    // Typecheck
    let mut tc = TypeChecker::new()
        .with_target(CompilationTarget::Interpreter);
    let type_errors = tc.check_program(&mut program.clone());
    if !type_errors.is_empty() {
        return Err(format!("Type errors: {:?}", type_errors).into());
    }

    // Generate LLVM IR
    let mut backend = LlvmBackend::new();
    let output = backend.generate(&program);

    // Verify wake trigger metadata
    assert!(output.contains("@llvm.wake_triggers = appending global [2 x i8*]"),
        "Should have appending global with 2 wake trigger symbols");
    assert!(output.contains("__sigint_flag"),
        "Should reference __sigint_flag");
    assert!(output.contains("__sigterm_flag"),
        "Should reference __sigterm_flag");

    // Verify main() calls __rt_init and __rt_wait
    assert!(output.contains("call void @__rt_init()"),
        "main() should call __rt_init()");
    assert!(output.contains("call void @__rt_wait()"),
        "main() should call __rt_wait()");

    // Verify __rt_init and __rt_wait are declared
    assert!(output.contains("declare void @__rt_init()"),
        "__rt_init should be declared");
    assert!(output.contains("declare void @__rt_wait()"),
        "__rt_wait should be declared");

    // Verify main uses non-willreturn attribute
    assert!(output.contains("define i32 @main() local_unnamed_addr #2"),
        "main() should use non-willreturn attribute #2");

    // Run opt -verify (existing pattern)
    verify_llvm_ir(&output)?;

    Ok(())
}
```

Also add a test for the **no-wake** case:

```rust
#[test]
fn test_no_wake_busy_loop() -> Result<(), Box<dyn std::error::Error>> {
    let bv_path = PathBuf::from("tests/fixtures/minimal.bv");
    let source = std::fs::read_to_string(&bv_path)?;

    let mut parser = Parser::new(&source);
    let mut program = parser.parse().map_err(|e| format!("Parse error: {}", e))?;
    let mut desug = Desugarer::new();
    let program = desug.desugar(&program);

    let mut backend = LlvmBackend::new();
    let output = backend.generate(&program);

    // No wake triggers → no __rt_init or __rt_wait calls
    assert!(!output.contains("call void @__rt_init()"),
        "No wake triggers should not call __rt_init()");
    assert!(!output.contains("call void @__rt_wait()"),
        "No wake triggers should not call __rt_wait()");
    assert!(!output.contains("@llvm.wake_triggers"),
        "No wake triggers should not emit @llvm.wake_triggers");

    Ok(())
}
```

### Step 3: Add `verify_llm_ir` helper if it doesn't exist

If `tests/llvm_backend_test.rs` already has a helper that runs `opt -verify`, reuse it. If not, add:

```rust
fn verify_llvm_ir(ir: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::new()?;
    write!(tmp, "{}", ir)?;
    let status = std::process::Command::new("opt")
        .args(["-verify", tmp.path().to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| "opt not found — install LLVM tools")?;
    if !status.success() {
        return Err("opt -verify failed".into());
    }
    Ok(())
}
```

### Dependencies

- Plan A (willreturn fix) — the assertion `define i32 @main() ... #2` depends on this
- The fixture file depends on nothing
- The test imports need `use` paths for `Parser`, `Desugarer`, `TypeChecker`, `LlvmBackend`, `CompilationTarget`

### Test verification

- `cargo test --test llvm_backend_test` — runs the new integration test
- Requires `opt` (LLVM tools) on PATH — skip with `#[ignore]` or guard if unavailable

### Edge cases

- `opt` not installed: skip the verification, but still run IR pattern assertions
- `.bv` fixture parse errors: test will fail with clear error message
- Multiple transactions: verify all transactions appear in reactor_tick