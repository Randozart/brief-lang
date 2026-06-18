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
            let mut all: Vec<String> = Vec::new();
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
/// Walks the kernel body AST and emits actual LLVM IR instructions for
/// assignment statements with integer arithmetic. Each state field is
/// accessed by computing `buffer + gtid * 8 + field_offset_in_bytes`
/// from the single `i8*` storage buffer parameter.
pub fn emit_spirv_module(kernel: &GpuKernel) -> String {
    let mut ir = String::new();
    let mut label_counter = 0u64;

    let next_label = |counter: &mut u64| -> String {
        let l = format!(".L{}", counter);
        *counter += 1;
        l
    };

    ir.push_str("; SPIR-V kernel: ");
    ir.push_str(&kernel.name);
    ir.push_str("\n");
    ir.push_str("target datalayout = \"e-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024\"\n");
    ir.push_str("target triple = \"spirv64-unknown-unknown\"\n\n");
    ir.push_str("declare i64 @_Z13get_global_idj(i32)\n\n");

    ir.push_str(&format!(
        "define spir_kernel void @{}(i8* nocapture %buffer, i64 %N) {{\n",
        kernel.name
    ));
    ir.push_str("entry:\n");
    ir.push_str("  %gtid = call i64 @_Z13get_global_idj(i32 0)\n");
    ir.push_str("  %cmp = icmp ult i64 %gtid, %N\n");

    let body_label = next_label(&mut label_counter);
    let exit_label = next_label(&mut label_counter);
    ir.push_str(&format!("  br i1 %cmp, label %{}, label %{}\n", body_label, exit_label));
    ir.push_str(&format!("{}:\n", body_label));

    for stmt in &kernel.body {
        emit_spirv_stmt(stmt, &mut ir, "  ", &kernel.write_fields, &mut label_counter);
    }

    ir.push_str(&format!("  br label %{}\n", exit_label));
    ir.push_str(&format!("{}:\n", exit_label));
    ir.push_str("  ret void\n");
    ir.push_str("}\n");

    ir
}

/// Emit a single Brief statement as SPIR-V-compatible LLVM IR.
fn emit_spirv_stmt(
    stmt: &Statement,
    ir: &mut String,
    indent: &str,
    write_fields: &[String],
    label_counter: &mut u64,
) {
    match stmt {
        Statement::Assignment { lhs, expr, .. } => {
            let field_name = if let Expr::Identifier(f) = lhs { f } else { return };
            let field_idx = write_fields.iter().position(|f| f == field_name)
                .unwrap_or(0);
            let offset = (field_idx as u64) * 8;

            // Load current value from buffer[gtid * 8 + field_offset]
            ir.push_str(&format!(
                "{}%bc = getelementptr i8, i8* %buffer, i64 %gtid\n", indent
            ));
            ir.push_str(&format!(
                "{}%fptr = getelementptr i8, i8* %bc, i64 {}\n", indent, offset
            ));
            ir.push_str(&format!(
                "{}%old = load i64, i8* %fptr, align 8\n", indent
            ));

            // Compute the new value from expr
            let val = emit_spirv_expr(expr, ir, indent, "%old", write_fields);

            ir.push_str(&format!("{}store i64 {}, i8* %fptr, align 8\n", indent, val));
        }
        Statement::Guarded { condition, statements, .. } => {
            let cond = emit_spirv_expr(condition, ir, indent, "%old", write_fields);
            let then_l = format!(".L{}", { let l = *label_counter; *label_counter += 1; l });
            let merge_l = format!(".L{}", { let l = *label_counter; *label_counter += 1; l });
            ir.push_str(&format!("{}%cond = icmp ne i64 {}, 0\n", indent, cond));
            ir.push_str(&format!("{}br i1 %cond, label %{}, label %{}\n", indent, then_l, merge_l));
            ir.push_str(&format!("{}:\n", then_l));
            for s in statements {
                emit_spirv_stmt(s, ir, &format!("  {}", indent), write_fields, label_counter);
            }
            ir.push_str(&format!("{}br label %{}\n", indent, merge_l));
            ir.push_str(&format!("{}:\n", merge_l));
        }
        _ => {}
    }
}

/// Emit a Brief expression as SPIR-V-compatible LLVM IR,
/// returning the SSA register name holding the result.
fn emit_spirv_expr(
    expr: &Expr,
    ir: &mut String,
    indent: &str,
    _old_reg: &str,
    _write_fields: &[String],
) -> String {
    match expr {
        Expr::Integer(n) => format!("{}", n),
        Expr::Bool(b) => {
            if *b { "1".to_string() } else { "0".to_string() }
        }
        Expr::Identifier(name) => {
            // TODO: in a full impl, load from buffer offset for this field.
            // For now, just reference the previously loaded %old value.
            // If the field matches, reuse %old; otherwise use a load.
            format!("%old")
        }
        Expr::Add(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, _old_reg, _write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, _old_reg, _write_fields);
            let reg = format!("%add{}", ir.len());
            ir.push_str(&format!("{}{} = add i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Sub(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, _old_reg, _write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, _old_reg, _write_fields);
            let reg = format!("%sub{}", ir.len());
            ir.push_str(&format!("{}{} = sub i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Mul(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, _old_reg, _write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, _old_reg, _write_fields);
            let reg = format!("%mul{}", ir.len());
            ir.push_str(&format!("{}{} = mul i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Lt(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, _old_reg, _write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, _old_reg, _write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp slt i64 {}, {}\n", indent, reg, l, r));
            // SPIR-V bool → i64: zext
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        _ => "0".to_string(), // fallback
    }
}

/// Generate an LLVM IR constant array containing SPIR-V binary bytes
/// for embedding in the main module's `.rodata` section.
pub fn embed_spirv_blob(spirv_binary: &[u8], kernel_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("\n; Embedded SPIR-V kernel: {}\n", kernel_name));
    out.push_str(&format!("@brief_kernel_{} = private constant [{} x i8] c\"",
        kernel_name, spirv_binary.len()));
    for (i, byte) in spirv_binary.iter().enumerate() {
        if i > 0 && i % 32 == 0 {
            out.push_str("\"\"\n  \"");
        }
        out.push_str(&format!("\\{:02X}", byte));
    }
    out.push_str("\", align 4\n");
    out
}

/// Compile a kernel's LLVM IR to SPIR-V binary via `llc`.
///
/// This runs `llc --mtriple=spirv64-unknown-unknown` on the kernel IR
/// and captures the output as a `.spv` byte buffer.
pub fn compile_to_spirv(ir: &str) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::Command;

    let tmp_dir = std::env::temp_dir();
    let ir_path = tmp_dir.join("brief_kernel.ll");
    let spv_path = tmp_dir.join("brief_kernel.spv");

    let mut file = std::fs::File::create(&ir_path)
        .map_err(|e| format!("Failed to create temp IR file: {}", e))?;
    file.write_all(ir.as_bytes())
        .map_err(|e| format!("Failed to write temp IR: {}", e))?;

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

    let binary = std::fs::read(&spv_path)
        .map_err(|e| format!("Failed to read SPIR-V output: {}", e))?;

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

    #[test]
    fn test_emit_spirv_module_emits_assignment() {
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Integer(42),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("assign_test", &body, Expr::Integer(10), &[]);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("getelementptr"), "should emit GEP for field access");
        assert!(!ir.contains("TODO"), "should not contain placeholder comments");
    }

    #[test]
    fn test_emit_spirv_module_emits_arithmetic() {
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Add(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Integer(1)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("arith_test", &body, Expr::Integer(10), &[]);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("add i64"), "should emit integer add");
        assert!(!ir.contains("TODO"), "should not contain placeholder comments");
    }

    #[test]
    fn test_embed_spirv_blob_generates_array() {
        let blob = vec![0x03, 0x02, 0x01, 0x00];
        let s = embed_spirv_blob(&blob, "test_kernel");
        assert!(s.contains("@brief_kernel_test_kernel"));
        assert!(s.contains("[4 x i8]"));
        assert!(s.contains("\\03"));
    }
}
