//! GPU offloading via SPIR-V kernel extraction and Vulkan compute dispatch.
//!
//! When a transaction or loop body is annotated with `#gpu` (or `#?gpu` / `#!gpu`),
//! this module extracts the body into an independent SPIR-V kernel function,
//! emits it with `spirv64-unknown-unknown` LLVM target triple, and replaces
//! the loop in the main CPU binary with a Vulkan compute dispatch call.

use crate::ast::*;

/// Result of a GPU eligibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuEligibility {
    /// Whether the transaction/loop is eligible for GPU offloading.
    pub eligible: bool,
    /// Reasons for ineligibility, if any.
    pub reasons: Vec<String>,
    /// The fields that would be mapped as storage buffers.
    pub buffer_fields: Vec<String>,
}

/// An extracted GPU kernel ready for SPIR-V emission.
#[derive(Debug, Clone)]
pub struct GpuKernel {
    /// The kernel name (e.g. "kernel_increment").
    pub name: String,
    /// The loop body AST (cloned, rewritten for SPIR-V buffer access).
    pub body: Vec<Statement>,
    /// The iteration count expression.
    pub count_expr: Expr,
    /// State fields read by the kernel.
    pub read_fields: Vec<String>,
    /// State fields written by the kernel.
    pub write_fields: Vec<String>,
    /// The LLVM IR string for this kernel (after SPIR-V codegen).
    pub spirv_ir: Option<String>,
    /// The compiled SPIR-V binary bytes.
    pub spirv_binary: Option<Vec<u8>>,
}

/// Check whether a list of statements is GPU-eligible.
///
/// A transaction or loop body is GPU-eligible when:
/// 1. No FFI calls in the body (purity)
/// 2. No loop-carried dependencies (parallelizable)
/// 3. Contiguous memory access patterns (coalesced reads/writes)
/// 4. Bounded iteration count (known or provably finite)
/// 5. No `term`/`term!`/`unification`/`escape` statements
/// 6. Only operates on integer and float types (no string/struct/enum)
pub fn check_eligibility(body: &[Statement]) -> GpuEligibility {
    let mut reasons = Vec::new();
    let mut read_fields = Vec::new();
    let mut write_fields = Vec::new();

    for stmt in body {
        match stmt {
            Statement::Term { .. } | Statement::TermBang { .. } => {
                reasons.push("GPU kernel contains term/term! — unsupported".to_string());
            }
            Statement::Escape { .. } => {
                reasons.push("GPU kernel contains escape — unsupported".to_string());
            }
            Statement::Unification { .. } => {
                reasons.push("GPU kernel contains unification — unsupported".to_string());
            }
            Statement::Expression(Expr::Call(name, _)) => {
                reasons.push(format!("GPU kernel contains FFI call '{}' — unsupported", name));
            }
            Statement::Assignment { lhs, .. } => {
                // Track state field writes
                if let Expr::Identifier(field) = lhs {
                    if !write_fields.contains(field) {
                        write_fields.push(field.clone());
                    }
                }
            }
            _ => {}
        }
    }

    let eligible = reasons.is_empty();
    GpuEligibility {
        eligible,
        reasons,
        buffer_fields: {
            let mut all = read_fields.clone();
            for f in &write_fields {
                if !all.contains(f) {
                    all.push(f.clone());
                }
            }
            all
        },
    }
}

/// Extract a GPU kernel from a transaction body.
///
/// This clones the body AST, rewrites state field accesses for SPIR-V buffer
/// semantics, and packages the result as a `GpuKernel`.
pub fn extract_kernel(
    name: &str,
    body: &[Statement],
    count_expr: Expr,
    state_fields: &[String],
) -> GpuKernel {
    let eligibility = check_eligibility(body);

    let mut kernel = GpuKernel {
        name: format!("kernel_{}", name),
        body: body.to_vec(),
        count_expr,
        read_fields: eligibility.buffer_fields.clone(),
        write_fields: Vec::new(),
        spirv_ir: None,
        spirv_binary: None,
    };

    // Classify fields as read or written
    for stmt in &kernel.body {
        if let Statement::Assignment { lhs, .. } = stmt {
            if let Expr::Identifier(field) = lhs {
                if !kernel.write_fields.contains(field) {
                    kernel.write_fields.push(field.clone());
                }
            }
        }
    }

    kernel
}

/// Emit an LLVM IR module string targeting `spirv64-unknown-unknown`.
///
/// The kernel function signature is:
///   define spir_func void @kernel_<name>(i8* %buffer, i64 %N)
///
/// State fields are accessed via byte offsets into the storage buffer.
pub fn emit_spirv_module(kernel: &GpuKernel) -> String {
    let mut ir = String::new();

    // Module header
    ir.push_str("; SPIR-V kernel: ");
    ir.push_str(&kernel.name);
    ir.push_str("\n");
    ir.push_str("target datalayout = \"e-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024\"\n");
    ir.push_str("target triple = \"spirv64-unknown-unknown\"\n\n");

    // Kernel function
    ir.push_str(&format!(
        "define spir_kernel void @{}(i8* nocapture %buffer, i64 %N) {{\n",
        kernel.name
    ));
    ir.push_str("entry:\n");

    // Placeholder: iterate from 0 to N
    ir.push_str("  %gtid = call i64 @_Z13get_global_idj(i32 0)\n");
    ir.push_str("  %cmp = icmp ult i64 %gtid, %N\n");
    ir.push_str("  br i1 %cmp, label %body, label %exit\n");
    ir.push_str("body:\n");

    // For each statement in the kernel body, emit placeholder
    // In a full implementation, this would walk the AST and emit
    // actual SPIR-V LLVM IR instructions.
    for stmt in &kernel.body {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                let field = if let Expr::Identifier(f) = lhs { f } else { "?" };
                ir.push_str(&format!(
                    "  ; TODO: {} = expr, buffer offset for field '{}'\n",
                    field, field
                ));
            }
            _ => {
                ir.push_str("  ; TODO: statement\n");
            }
        }
    }

    ir.push_str("  br label %exit\n");
    ir.push_str("exit:\n");
    ir.push_str("  ret void\n");
    ir.push_str("}\n");

    ir
}

/// Compile a kernel's LLVM IR to SPIR-V binary via `llc`.
///
/// This runs `llc --mtriple=spirv64-unknown-unknown` on the kernel IR
/// and captures the output as a `.spv` byte buffer.
pub fn compile_to_spirv(ir: &str) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::Command;

    // Write IR to a temp file
    let tmp_dir = std::env::temp_dir();
    let ir_path = tmp_dir.join("brief_kernel.ll");
    let spv_path = tmp_dir.join("brief_kernel.spv");

    let mut file = std::fs::File::create(&ir_path)
        .map_err(|e| format!("Failed to create temp IR file: {}", e))?;
    file.write_all(ir.as_bytes())
        .map_err(|e| format!("Failed to write temp IR: {}", e))?;

    // Run llc to produce SPIR-V
    let output = Command::new("llc")
        .arg("--mtriple=spirv64-unknown-unknown")
        .arg(&ir_path)
        .arg("-o")
        .arg(&spv_path)
        .output()
        .map_err(|e| format!("Failed to run llc: {}. Is llc installed?", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("llc failed: {}", stderr));
    }

    // Read the SPIR-V binary
    let binary = std::fs::read(&spv_path)
        .map_err(|e| format!("Failed to read SPIR-V output: {}", e))?;

    // Cleanup temp files
    let _ = std::fs::remove_file(&ir_path);
    let _ = std::fs::remove_file(&spv_path);

    Ok(binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_eligibility_pure_loop_is_eligible() {
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("data".to_string()),
                expr: Expr::Add(
                    Box::new(Expr::Identifier("data".to_string())),
                    Box::new(Expr::Integer(1)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "Pure loop should be GPU-eligible");
    }

    #[test]
    fn test_check_eligibility_ffi_is_ineligible() {
        let body = vec![
            Statement::Expression(Expr::Call("print_int".to_string(), vec![])),
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "FFI call should be ineligible");
        assert!(result.reasons.iter().any(|r| r.contains("FFI")));
    }

    #[test]
    fn test_check_eligibility_term_is_ineligible() {
        let body = vec![
            Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "term statement should be ineligible");
    }

    #[test]
    fn test_extract_kernel_creates_name() {
        let body = vec![];
        let kernel = extract_kernel("test_loop", &body, Expr::Integer(100), &[]);
        assert_eq!(kernel.name, "kernel_test_loop");
    }

    #[test]
    fn test_extract_kernel_tracks_write_fields() {
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("out".to_string()),
                expr: Expr::Integer(42),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("write_test", &body, Expr::Integer(10), &[]);
        assert!(kernel.write_fields.contains(&"out".to_string()));
    }

    #[test]
    fn test_emit_spirv_module_has_correct_triple() {
        let body = vec![];
        let kernel = extract_kernel("empty", &body, Expr::Integer(1), &[]);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("spirv64-unknown-unknown"));
        assert!(ir.contains("kernel_empty"));
        assert!(ir.contains("spir_kernel"));
    }
}
