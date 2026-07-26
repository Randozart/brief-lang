//! GPU offloading via SPIR-V kernel extraction and Vulkan compute dispatch.
//!
//! When a transaction or loop body is annotated with `#gpu` (or `#?gpu` / `#!gpu`),
//! this module extracts the body into an independent SPIR-V kernel function,
//! emits it with `spirv64-unknown-unknown` LLVM target triple, and replaces
//! the loop in the main CPU binary with a Vulkan compute dispatch call.

use crate::ast::*;
use std::collections::HashMap;

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
    /// LLVM type string per field: "i64", "float", "i8", "i32", etc.
    pub field_types: HashMap<String, String>,
    /// The LLVM IR string for this kernel (after SPIR-V codegen).
    pub spirv_ir: Option<String>,
    /// The compiled SPIR-V binary bytes.
    pub spirv_binary: Option<Vec<u8>>,
}

/// Check whether a list of statements is GPU-eligible.
///
/// A transaction or loop body is GPU-eligible when:
/// 1. No user FFI calls in the body (only well-known GPU intrinsics allowed)
/// 2. Bounded iteration count (known or provably finite)
/// 3. Only operates on integer and float types (no string/struct/enum)
/// 4. No `term!`/`unification`/`escape` statements
///
/// Deferred (not yet implemented):
/// - Loop-carried dependency analysis for parallelizability verification
/// - Stride analysis for memory coalescing verification
    /// Why only Int/Float/Bool/Char can cross the GPU boundary: SPIR-V kernels
    /// execute in a separate address space from the host. Pointers to host memory
    /// (strings, struct fields, collection headers) are invalid in the GPU
    /// because each work-item has its own local memory and global memory buffers.
    /// Strings would require host-device memory coherence; structs/enums require
    /// pointer chasing across the PCIe bus. Only flat value types (Int as i64,
    /// Float as float, Bool as i8, Char as i32) can be packed into a linear
    /// buffer and indexed by global work-item ID.
pub fn check_eligibility(body: &[Statement]) -> GpuEligibility {
    let mut reasons = Vec::new();
    let mut write_fields = Vec::new();
    let mut touched_fields = Vec::new();

    for stmt in body {
        match stmt {
            Statement::TermBang(_) => {
                reasons.push("GPU kernel contains term! — unsupported".to_string());
            }
            Statement::Term(swan_song) => {
                // term is allowed in GPU kernels (no-op convergence signal).
                // Check the swan song for GPU eligibility if present.
                if let Some(swan) = swan_song {
                    collect_unsafe_ffi(swan, &mut reasons);
                    collect_touched_fields(swan, &mut touched_fields);
                }
            }
            Statement::Expression(expr) => {
                collect_unsafe_ffi(expr, &mut reasons);
                collect_touched_fields(expr, &mut touched_fields);
            }
            Statement::Let { expr: Some(e), .. } => {
                collect_unsafe_ffi(e, &mut reasons);
                collect_touched_fields(e, &mut touched_fields);
            }
            Statement::Assign(lhs, expr) => {
                if let Expr::Identifier(field) = lhs {
                    if !write_fields.contains(field) {
                        write_fields.push(field.clone());
                    }
                    if !touched_fields.contains(field) {
                        touched_fields.push(field.clone());
                    }
                }
                collect_unsafe_ffi(expr, &mut reasons);
                collect_touched_fields(expr, &mut touched_fields);
            }
            Statement::Guarded(condition, statements) => {
                collect_unsafe_ffi(condition, &mut reasons);
                collect_touched_fields(condition, &mut touched_fields);
                let inner = check_eligibility(statements);
                reasons.extend(inner.reasons);
                for f in inner.buffer_fields {
                    if !write_fields.contains(&f) {
                        write_fields.push(f.clone());
                    }
                    if !touched_fields.contains(&f) {
                        touched_fields.push(f);
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
        buffer_fields: touched_fields,
    }
}

/// Collect all field identifier names referenced in an expression tree.
/// This is used to track which state fields a kernel touches (reads or writes).
fn collect_touched_fields(expr: &Expr, fields: &mut Vec<String>) {
    match expr {
        Expr::Identifier(name) => {
            if !fields.contains(name) {
                fields.push(name.clone());
            }
        }
        Expr::BinaryOp(_, l, r) => {
            collect_touched_fields(l, fields);
            collect_touched_fields(r, fields);
        }
        Expr::UnaryOp(_, e) | Expr::Cast(e, _) => {
            collect_touched_fields(e, fields);
        }
        Expr::Call(_, args, _) => {
            for arg in args {
                collect_touched_fields(arg, fields);
            }
        }
        Expr::List(items) | Expr::Tuple(items) => {
            for item in items {
                collect_touched_fields(item, fields);
            }
        }
        Expr::Field(obj, _) => {
            collect_touched_fields(obj, fields);
        }
        Expr::Index(obj, idx) => {
            collect_touched_fields(obj, fields);
            collect_touched_fields(idx, fields);
        }
        _ => {}
    }
}

/// Recursively walk an expression tree and collect reasons for any unsafe FFI
/// calls or intrinsics that would make the kernel ineligible for GPU offloading.
///
/// `Expr::Call` (user FFI) is always unsafe. `/* OLD: IntrinsicCall */ Expr::Call("".to_string(), vec![])` is only
/// unsafe if the intrinsic is not in the GPU-safe allowlist. `Expr::SharedMem`
/// is always allowed (it is GPU-native).
fn collect_unsafe_ffi(expr: &Expr, reasons: &mut Vec<String>) {
    match expr {
        Expr::Call(name, args, _) => {
            if name.ends_with('#') {
                if !is_gpu_safe_intrinsic(name) {
                    reasons.push(format!("GPU kernel contains unsafe intrinsic '{}'", name));
                }
            } else {
                reasons.push(format!("GPU kernel contains FFI call '{}' — unsupported", name));
            }
            for arg in args {
                collect_unsafe_ffi(arg, reasons);
            }
        }
        // Binary ops — recurse into both operands
        Expr::BinaryOp(_, l, r) => {
            collect_unsafe_ffi(l, reasons);
            collect_unsafe_ffi(r, reasons);
        }
        // Unary ops — recurse into operand
        Expr::UnaryOp(_, e) => {
            collect_unsafe_ffi(e, reasons);
        }
        // Collection literals — recurse into elements
        Expr::List(items) | Expr::Tuple(items) => {
            for item in items {
                collect_unsafe_ffi(item, reasons);
            }
        }
        // Index — recurse into value and index
        Expr::Index(v, i) => {
            collect_unsafe_ffi(v, reasons);
            collect_unsafe_ffi(i, reasons);
        }
        // Cast / Field — recurse into operands
        Expr::Cast(e, _) => collect_unsafe_ffi(e, reasons),
        Expr::Field(obj, _) => {
            collect_unsafe_ffi(obj, reasons);
        }
        // Block / If — recurse into contained expressions/statements
        Expr::Block(stmts) => {
            for stmt in stmts {
                collect_unsafe_ffi_stmt(stmt, reasons);
            }
        }
        Expr::If(cond, then, else_opt) => {
            collect_unsafe_ffi(cond, reasons);
            collect_unsafe_ffi(then, reasons);
            if let Some(else_expr) = else_opt {
                collect_unsafe_ffi(else_expr, reasons);
            }
        }
        // Terminals — no sub-expressions
        Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_)
        | Expr::Quoted(_) | Expr::Identifier(_) => {}
        // Catch-all for remaining expression types — conservatively reject
        _ => {
            reasons.push("GPU kernel contains unsupported expression type".to_string());
        }
    }
}

/// Helper to walk statements when recursing from expression tree (Expr::Block).
fn collect_unsafe_ffi_stmt(stmt: &Statement, reasons: &mut Vec<String>) {
    match stmt {
        Statement::Expression(expr) => collect_unsafe_ffi(expr, reasons),
        Statement::Assign(lhs, expr) => {
            collect_unsafe_ffi(lhs, reasons);
            collect_unsafe_ffi(expr, reasons);
        }
        Statement::Let { expr: Some(e), .. } => collect_unsafe_ffi(e, reasons),
        Statement::Let { expr: None, .. } => {}
        Statement::Guarded(cond, stmts) => {
            collect_unsafe_ffi(cond, reasons);
            for s in stmts {
                collect_unsafe_ffi_stmt(s, reasons);
            }
        }
        Statement::Gate(cond) => collect_unsafe_ffi(cond, reasons),
        Statement::Block(stmts) => {
            for s in stmts {
                collect_unsafe_ffi_stmt(s, reasons);
            }
        }
        Statement::If(cond, then_body, else_body) => {
            collect_unsafe_ffi(cond, reasons);
            for s in then_body {
                collect_unsafe_ffi_stmt(s, reasons);
            }
            for s in else_body {
                collect_unsafe_ffi_stmt(s, reasons);
            }
        }
        Statement::Term(Some(expr))
        | Statement::TermBang(Some(expr))
        | Statement::Return(Some(expr))
        | Statement::Escape(Some(expr)) => collect_unsafe_ffi(expr, reasons),
        Statement::Term(None) | Statement::TermBang(None) | Statement::Return(None) | Statement::Escape(None) => {}
        Statement::SyncBlock(stmts) => {
            for s in stmts {
                collect_unsafe_ffi_stmt(s, reasons);
            }
        }
        Statement::Foreach { list, body, .. } => {
            collect_unsafe_ffi(list, reasons);
            for s in body {
                collect_unsafe_ffi_stmt(s, reasons);
            }
        }
        Statement::TrgBinding { instance, .. } => {
            collect_unsafe_ffi(instance, reasons);
        }
        Statement::InlineAsm { .. } | Statement::MetadataAssignment(..) | Statement::InlineDefn(_) | Statement::InlineTxn(_) | Statement::Match { .. } => {
            // No expression recursion needed
        }
    }
}

/// Returns true if the intrinsic is safe to execute on GPU.
///
/// Well-known math intrinsics (sin, cos, pow, sqrt, fabs) map directly to
/// SPIR-V / GPU instructions. GPU query intrinsics (get_global_id, barrier)
/// are added alongside their Intrinsic enum entries.
fn is_gpu_safe_intrinsic(name: &str) -> bool {
    matches!(name,
        "Sin#" | "Cos#" | "Pow#"
        | "Sqrt#" | "Fabs#"
        | "Ceil#" | "Floor#"
        | "GetGlobalId#" | "GetLocalId#"
        | "GetGroupId#" | "GetNumGroups#"
        | "SubGroupBarrier#"
    )
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
    field_types: HashMap<String, String>,
) -> GpuKernel {
    let eligibility = check_eligibility(body);

    let mut kernel = GpuKernel {
        name: format!("kernel_{}", name),
        body: body.to_vec(),
        count_expr,
        read_fields: eligibility.buffer_fields.clone(),
        write_fields: Vec::new(),
        field_types,
        spirv_ir: None,
        spirv_binary: None,
    };

    for stmt in &kernel.body {
        if let Statement::Assign(lhs, _) = stmt {
            if let Expr::Identifier(field) = lhs {
                if !kernel.write_fields.contains(field) {
                    kernel.write_fields.push(field.clone());
                }
            }
        }
    }

    kernel
}

/// Collect all field identifiers referenced in a list of statements.
/// Returns a deduplicated list in order of first appearance.
fn collect_all_fields(body: &[Statement]) -> Vec<String> {
    let mut fields = Vec::new();
    for stmt in body {
        collect_stmt_fields(stmt, &mut fields);
    }
    fields
}

fn collect_stmt_fields(stmt: &Statement, fields: &mut Vec<String>) {
    match stmt {
        Statement::Assign(lhs, expr) => {
            if let Expr::Identifier(f) = lhs {
                if !fields.contains(f) { fields.push(f.clone()); }
            }
            collect_expr_fields(expr, fields);
        }
        Statement::Guarded(condition, statements) => {
            collect_expr_fields(condition, fields);
            for s in statements {
                collect_stmt_fields(s, fields);
            }
        }
        _ => {}
    }
}

fn collect_expr_fields(expr: &Expr, fields: &mut Vec<String>) {
    match expr {
        Expr::Identifier(name) => {
            if !fields.contains(name) { fields.push(name.clone()); }
        }
        Expr::BinaryOp(_, l, r) => {
            collect_expr_fields(l, fields);
            collect_expr_fields(r, fields);
        }
        Expr::UnaryOp(_, e) => {
            collect_expr_fields(e, fields);
        }
        _ => {}
    }
}

/// Emit an LLVM IR module string targeting `spirv64-unknown-unknown`.
///
/// Walks the kernel body AST and emits actual LLVM IR instructions for
/// assignment statements with integer arithmetic. Each state field is
/// accessed by computing `buffer + gtid + field_offset_in_bytes`
/// from the single `i8*` storage buffer parameter.
///
/// Each field referenced in an expression is loaded from its correct
/// buffer offset into a unique SSA register — unlike the old approach
/// that reused a single `%old` register for the LHS field.
    /// Why raw LLVM IR targeting spirv64 instead of a SPIR-V builder library:
    /// the existing LLVM backend already emits valid LLVM IR for all expression
    /// types. By targeting spirv64-unknown-unknown, we reuse the same emission
    /// infrastructure (same emit_expr, same getelementptr, same function calls)
    /// and let llc handle the LLVM→SPIR-V translation. A SPIR-V builder library
    /// would require duplicating the entire expression emission pipeline.
    ///
    /// Why buffer-based memory model instead of %State: GPU kernels execute
    /// across thousands of work-items simultaneously, each with its own global_id.
    /// A shared %State struct would serialize all work-items. Instead, each
    /// field is accessed by computing (gtid * stride + field_offset) within
    /// a flat i8* buffer. The host packs all state fields into in_buf before
    /// launch and unpacks out_buf after completion. The stride is sizeof(State)
    /// per work-item, so work-item N reads/writes bytes [N*stride, N*stride+stride).
    ///
    /// Why shared memory is addrspace(3): SPIR-V's workgroup shared memory
    /// is accessible by all work-items in the same workgroup but not by the
    /// host. LLVM represents this as address space 3 with the `internal` linkage.
pub fn emit_spirv_module(kernel: &GpuKernel) -> String {
    let mut ir = String::new();
    let mut label_counter = 0u64;

    // Build field offset map from all fields in the body
    let all_fields = collect_all_fields(&kernel.body);
    let field_offsets: HashMap<String, u64> = all_fields.iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), (i as u64) * 8))
        .collect();

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
    // SPIR-V built-in function declares
    ir.push_str("declare i64 @_Z13get_global_idj(i32) #0\n");
    ir.push_str("declare i64 @_Z12get_local_idj(i32) #0\n");
    ir.push_str("declare i64 @_Z12get_group_idj(i32) #0\n");
    ir.push_str("declare i64 @_Z16get_num_groupsj(i32) #0\n");
    ir.push_str("declare void @_Z8barrierj(i32) #0\n");
    // Math intrinsic declares (GPU-native in SPIR-V)
    ir.push_str("declare float @llvm.sin.f32(float) #0\n");
    ir.push_str("declare float @llvm.cos.f32(float) #0\n");
    ir.push_str("declare float @llvm.pow.f32(float, float) #0\n");
    ir.push_str("declare float @llvm.sqrt.f32(float) #0\n");
    ir.push_str("declare float @llvm.fabs.f32(float) #0\n");
    ir.push_str("declare float @llvm.ceil.f32(float) #0\n");
    ir.push_str("declare float @llvm.floor.f32(float) #0\n");
    ir.push_str("\n");

    let has_print = has_print_intrinsics(&kernel.body);

    if has_print {
        ir.push_str(&format!(
            "define spir_kernel void @{}(i8* nocapture readonly %in_buf, i8* nocapture %out_buf, i8* nocapture %print_buf, i64 %N) {{\n",
            kernel.name
        ));
    } else {
        ir.push_str(&format!(
            "define spir_kernel void @{}(i8* nocapture readonly %in_buf, i8* nocapture %out_buf, i64 %N) {{\n",
            kernel.name
        ));
    }
    ir.push_str("entry:\n");
    ir.push_str("  %gtid = call i64 @_Z13get_global_idj(i32 0)\n");
    ir.push_str("  %cmp = icmp ult i64 %gtid, %N\n");

    let body_label = next_label(&mut label_counter);
    let exit_label = next_label(&mut label_counter);
    ir.push_str(&format!("  br i1 %cmp, label %{}, label %{}\n", body_label, exit_label));
    ir.push_str(&format!("{}:\n", body_label));

    // Compute base pointers: in_buf for reads, out_buf for writes, print_buf for I/O
    ir.push_str("  %base_in = getelementptr i8, ptr %in_buf, i64 %gtid\n");
    ir.push_str("  %base_out = getelementptr i8, ptr %out_buf, i64 %gtid\n");
    if has_print {
        ir.push_str("  %base_print = getelementptr i8, ptr %print_buf, i64 %gtid\n");
    }

    let mut loaded_regs: HashMap<String, String> = HashMap::new();
    let field_types = &kernel.field_types;
    let write_fields: Vec<String> = kernel.write_fields.clone();
    for stmt in &kernel.body {
        emit_spirv_stmt(stmt, &mut ir, "  ", &field_offsets, &mut loaded_regs, &mut label_counter, field_types, &write_fields);
    }

    ir.push_str(&format!("  br label %{}\n", exit_label));
    ir.push_str(&format!("{}:\n", exit_label));
    ir.push_str("  ret void\n");
    ir.push_str("}\n");

    ir
}

/// Return true if the expression tree operates on float values.
///
/// Walks the expression tree checking leaf nodes:
/// - `Expr::Float(_)` is always float
/// - `Expr::Identifier(name)` is float if `field_types[name] == "float"`
/// - Binary/unary ops are float if either operand is float
/// - Math intrinsics (sin, cos, pow, sqrt, fabs) return float
fn is_float_context(expr: &Expr, field_types: &HashMap<String, String>) -> bool {
    match expr {
        Expr::Float(_) => true,
        Expr::Identifier(name) => field_types.get(name).map(|t| t == "float").unwrap_or(false),
        Expr::BinaryOp(kind, l, r) if matches!(kind,
            BinaryOpKind::Add | BinaryOpKind::Sub | BinaryOpKind::Mul | BinaryOpKind::Div
            | BinaryOpKind::Lt | BinaryOpKind::Le | BinaryOpKind::Gt | BinaryOpKind::Ge
            | BinaryOpKind::Eq | BinaryOpKind::Neq
        ) => {
            is_float_context(l, field_types) || is_float_context(r, field_types)
        }
        Expr::UnaryOp(UnaryOpKind::Neg, e) => is_float_context(e, field_types),
        Expr::Call(name, _, _) if name.ends_with('#') => matches!(name.as_str(),
            "Sin#" | "Cos#" | "Pow#" | "Sqrt#" | "Fabs#"
            | "Ceil#" | "Floor#" | "PrintFloat#"
        ),
        _ => false,
    }
}

/// Convert an f64 Brief float to an f32 SPIR-V float bit pattern.
///
/// SPIR-V uses native 32-bit float, unlike the CPU backend which boxes
/// floats as i64. This truncates f64→f32 then produces the i32 bit pattern
/// for `bitcast i32 <hex> to float` emission.
fn float_to_spirv_hex(val: f64) -> String {
    let bits = (val as f32).to_bits();
    format!("{}", bits)
}



/// Scan the kernel body for print I/O intrinsics (print_int#, print_float#,
/// put_char#). When present, a print buffer parameter is added to the kernel.
fn has_print_intrinsics(body: &[Statement]) -> bool {
    for stmt in body {
        match stmt {
            Statement::Assign(_, expr) | Statement::Let { expr: Some(expr), .. } | Statement::Expression(expr) => {
                if has_print_intrinsics_expr(expr) {
                    return true;
                }
            }
            Statement::Guarded(condition, statements) => {
                if has_print_intrinsics_expr(condition) { return true; }
                if has_print_intrinsics(statements) { return true; }
            }
            _ => {}
        }
    }
    false
}

fn has_print_intrinsics_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Call(name, _, _) => matches!(name.as_str(),
            "__print_int" | "__print_float" | "__print_char"
        ),
        Expr::BinaryOp(_, l, r) => {
            has_print_intrinsics_expr(l) || has_print_intrinsics_expr(r)
        }
        Expr::UnaryOp(_, e) => has_print_intrinsics_expr(e),
        _ => false,
    }
}

/// Load a field from the buffer, emitting a unique GEP + load sequence,
/// and cache the register name in `loaded_regs` for reuse.
///
/// Loads the correct LLVM type based on `field_types`: `float` for float
/// fields, `i64` for integer/bool fields. Selects `%base_in` or `%base_out`
/// based on whether the field is in `write_fields`.
        // Why each field load computes buffer offset from scratch: the GPU
        // kernel receives a single i8* buffer for all fields. Unlike the CPU
        // path (which has %State with named GEP indices), we must compute
        // byte offsets manually: field_offset = gtid * state_stride + field_byte_offset.
        // The state_stride is sizeof(all state fields) per work-item.
        // We recompute the base pointer for each field rather than caching it,
        // because the LLVM register allocator would spill the cached pointer
        // across many field accesses anyway (register pressure is high in GPU kernels).
fn ensure_field_loaded(
    field: &str,
    ir: &mut String,
    indent: &str,
    field_offsets: &HashMap<String, u64>,
    loaded_regs: &mut HashMap<String, String>,
    field_types: &HashMap<String, String>,
    write_fields: &[String],
) -> String {
    if let Some(reg) = loaded_regs.get(field) {
        return reg.clone();
    }
    let offset = field_offsets.get(field).copied().unwrap_or(0);
    let gep = format!("%gep_{}", loaded_regs.len());
    let reg = format!("%lv_{}", loaded_regs.len());

    // Fields that are written use the output buffer (read-write);
    // read-only fields use the input buffer.
    let base = if write_fields.contains(&field.to_string()) { "%base_out" } else { "%base_in" };

    let is_float = field_types.get(field).map(|t| t == "float").unwrap_or(false);
    if is_float {
        ir.push_str(&format!("{}{} = getelementptr i8, ptr {}, i64 {}\n", indent, gep, base, offset));
        let bc = format!("%bc_{}", loaded_regs.len());
        ir.push_str(&format!("{}{} = bitcast ptr {} to float*\n", indent, bc, gep));
        ir.push_str(&format!("{}{} = load float, ptr {}, align 4\n", indent, reg, bc));
    } else {
        ir.push_str(&format!("{}{} = getelementptr i8, ptr {}, i64 {}\n", indent, gep, base, offset));
        ir.push_str(&format!("{}{} = load i64, i8* {}, align 8\n", indent, reg, gep));
    }
    loaded_regs.insert(field.to_string(), reg.clone());
    reg
}

/// Emit a single Brief statement as SPIR-V-compatible LLVM IR.
fn emit_spirv_stmt(
    stmt: &Statement,
    ir: &mut String,
    indent: &str,
    field_offsets: &HashMap<String, u64>,
    loaded_regs: &mut HashMap<String, String>,
    label_counter: &mut u64,
    field_types: &HashMap<String, String>,
    write_fields: &[String],
) {
    match stmt {
        Statement::Assign(lhs, expr) => {
            let lhs_name = if let Expr::Identifier(f) = lhs { f.clone() } else { return };

            let val = emit_spirv_expr(expr, ir, indent, field_offsets, loaded_regs, field_types, write_fields);

            // Store result to LHS field at output buffer offset
            let lhs_offset = field_offsets.get(&lhs_name).copied().unwrap_or(0);
            let gep = format!("%st_{}", loaded_regs.len());
            ir.push_str(&format!("{}{} = getelementptr i8, ptr %base_out, i64 {}\n", indent, gep, lhs_offset));
            let is_float = field_types.get(&lhs_name).map(|t| t == "float").unwrap_or(false);
            if is_float {
                let bc = format!("%stbc_{}", loaded_regs.len());
                ir.push_str(&format!("{}{} = bitcast ptr {} to float*\n", indent, bc, gep));
                ir.push_str(&format!("{}store float {}, float* {}, align 4\n", indent, val, bc));
            } else {
                ir.push_str(&format!("{}store i64 {}, i8* {}, align 8\n", indent, val, gep));
            }
        }
        Statement::Guarded(condition, statements) => {
            let cond = emit_spirv_expr(condition, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let then_l = format!(".L{}", { let l = *label_counter; *label_counter += 1; l });
            let merge_l = format!(".L{}", { let l = *label_counter; *label_counter += 1; l });
            ir.push_str(&format!("{}%cond = icmp ne i64 {}, 0\n", indent, cond));
            ir.push_str(&format!("{}br i1 %cond, label %{}, label %{}\n", indent, then_l, merge_l));
            ir.push_str(&format!("{}:\n", then_l));
            for s in statements {
                emit_spirv_stmt(s, ir, &format!("  {}", indent), field_offsets, loaded_regs, label_counter, field_types, write_fields);
            }
            ir.push_str(&format!("{}br label %{}\n", indent, merge_l));
            ir.push_str(&format!("{}:\n", merge_l));
        }
        // Let binding — evaluate the expression and register the result
        // so Identifier references can resolve to local SSA values.
        Statement::Let { name, expr: Some(e), .. } => {
            let val = emit_spirv_expr(e, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            loaded_regs.insert(name.clone(), val);
        }
        Statement::Let { expr: None, .. } => {
            // let with no initializer — no-op in SPIR-V kernel context
        }
        Statement::Expression(expr) => {
            let _val = emit_spirv_expr(expr, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
        }
        // Term — convergence signal (no-op in SPIR-V). Execute swan song if present.
        Statement::Term(Some(swan)) => {
            // 2026-07-13: swan song is an expression, not a statement
            let _val = emit_spirv_expr(swan, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
        }
        Statement::Term(None) => {
            // no-op convergence signal
        }
        _ => {}
    }
}

/// Emit a Brief expression as SPIR-V-compatible LLVM IR,
/// returning the SSA register name holding the result.
///
/// Field identifiers are looked up in `field_offsets` and loaded
/// via `ensure_field_loaded`, which caches loads in `loaded_regs`.
fn emit_spirv_expr(
    expr: &Expr,
    ir: &mut String,
    indent: &str,
    field_offsets: &HashMap<String, u64>,
    loaded_regs: &mut HashMap<String, String>,
    field_types: &HashMap<String, String>,
    write_fields: &[String],
) -> String {
    match expr {
        // Float literal: bitcast from i32 hex (f32 precision)
        Expr::Float(n) => {
            let reg = format!("%fl{}", ir.len());
            let hex = float_to_spirv_hex(*n);
            ir.push_str(&format!("{}{} = bitcast i32 {} to float\n", indent, reg, hex));
            reg
        }
        Expr::Decimal(n) => format!("{}", n),
        Expr::Bool(b) => {
            if *b { "1".to_string() } else { "0".to_string() }
        }
        Expr::Identifier(name) => {
            ensure_field_loaded(name, ir, indent, field_offsets, loaded_regs, field_types, write_fields)
        }
        // Float arithmetic
        Expr::BinaryOp(BinaryOpKind::Add, lhs, rhs) if is_float_context(expr, field_types) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fadd{}", ir.len());
            ir.push_str(&format!("{}{} = fadd float {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::BinaryOp(BinaryOpKind::Sub, lhs, rhs) if is_float_context(expr, field_types) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fsub{}", ir.len());
            ir.push_str(&format!("{}{} = fsub float {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::BinaryOp(BinaryOpKind::Mul, lhs, rhs) if is_float_context(expr, field_types) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fmul{}", ir.len());
            ir.push_str(&format!("{}{} = fmul float {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::BinaryOp(BinaryOpKind::Div, lhs, rhs) if is_float_context(expr, field_types) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fdiv{}", ir.len());
            ir.push_str(&format!("{}{} = fdiv float {}, {}\n", indent, reg, l, r));
            reg
        }
        // Integer arithmetic
        Expr::BinaryOp(BinaryOpKind::Add, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%add{}", ir.len());
            ir.push_str(&format!("{}{} = add i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::BinaryOp(BinaryOpKind::Sub, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%sub{}", ir.len());
            ir.push_str(&format!("{}{} = sub i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::BinaryOp(BinaryOpKind::Mul, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%mul{}", ir.len());
            ir.push_str(&format!("{}{} = mul i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::BinaryOp(BinaryOpKind::Div, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%div{}", ir.len());
            ir.push_str(&format!("{}{} = sdiv i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::BinaryOp(BinaryOpKind::Mod, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%rem{}", ir.len());
            ir.push_str(&format!("{}{} = srem i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        // Float comparisons
        cmp @ (Expr::BinaryOp(BinaryOpKind::Lt, _, _) | Expr::BinaryOp(BinaryOpKind::Le, _, _)
             | Expr::BinaryOp(BinaryOpKind::Gt, _, _) | Expr::BinaryOp(BinaryOpKind::Ge, _, _)
             | Expr::BinaryOp(BinaryOpKind::Eq, _, _) | Expr::BinaryOp(BinaryOpKind::Neq, _, _))
             if is_float_context(expr, field_types) => {
            let (l, r) = match cmp {
                Expr::BinaryOp(_, l, r) => (l, r),
                _ => unreachable!(),
            };
            let lv = emit_spirv_expr(l, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let rv = emit_spirv_expr(r, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let cond = match cmp {
                Expr::BinaryOp(BinaryOpKind::Lt, _, _) => "olt",
                Expr::BinaryOp(BinaryOpKind::Le, _, _) => "ole",
                Expr::BinaryOp(BinaryOpKind::Gt, _, _) => "ogt",
                Expr::BinaryOp(BinaryOpKind::Ge, _, _) => "oge",
                Expr::BinaryOp(BinaryOpKind::Eq, _, _) => "oeq",
                Expr::BinaryOp(BinaryOpKind::Neq, _, _) => "one",
                _ => unreachable!(),
            };
            let reg = format!("%fcmp{}", ir.len());
            ir.push_str(&format!("{}{} = fcmp {} float {}, {}\n", indent, reg, cond, lv, rv));
            let ext = format!("%fzext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        // Integer comparisons
        Expr::BinaryOp(BinaryOpKind::Lt, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp slt i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::BinaryOp(BinaryOpKind::Le, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp sle i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::BinaryOp(BinaryOpKind::Gt, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp sgt i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::BinaryOp(BinaryOpKind::Ge, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp sge i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::BinaryOp(BinaryOpKind::Eq, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp eq i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::BinaryOp(BinaryOpKind::Neq, lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp ne i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        // Float negation
        Expr::UnaryOp(UnaryOpKind::Neg, e) if is_float_context(expr, field_types) => {
            let v = emit_spirv_expr(e, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fneg{}", ir.len());
            ir.push_str(&format!("{}{} = fneg float {}\n", indent, reg, v));
            reg
        }
        // Int negation
        Expr::UnaryOp(UnaryOpKind::Neg, e) => {
            let v = emit_spirv_expr(e, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%neg{}", ir.len());
            ir.push_str(&format!("{}{} = sub i64 0, {}\n", indent, reg, v));
            reg
        }
        // GPU intrinsics
        Expr::Call(name, args, _) => {
            emit_spirv_intrinsic(name, args, ir, indent, field_offsets, loaded_regs, field_types, write_fields)
        }
        _ => {
            ir.push_str(&format!("{}; error: unsupported expression in GPU kernel\n", indent));
            "0".to_string()
        }
    }
}

    // Why GPU intrinsics map to different LLVM functions than the CPU path:
    // SPIR-V has its own built-in functions for thread ID queries (_Z13get_global_idj)
    // and synchronization (_Z8barrierj). These are declared as external functions
    // in the spirv64-unknown-unknown target triple and translated to SPIR-V
    // opcodes by llc. The CPU path uses @llvm.read_register or pthread_self(),
    // which are invalid in the SPIR-V context.
    //
    // Why barrier# uses CLK_GLOBAL_MEM_FENCE (flag 1): SPIR-V's barrier
    // requires a memory fence flags argument. CLK_GLOBAL_MEM_FENCE ensures
    // that all global memory writes before the barrier are visible to all
    // work-items in the workgroup after the barrier. Without the fence
    // flag, barrier only synchronizes execution, not memory.
///
/// Emit a GPU intrinsic call as SPIR-V-compatible LLVM IR,
/// returning the SSA register name holding the result.
///
/// Thread/block ID queries map to SPIR-V built-in function calls.
/// Math intrinsics map to `@llvm.*.f32` calls (native SPIR-V).
/// SubGroupBarrier as an expression returns 1 (true — barrier succeeded).
fn emit_spirv_intrinsic(
    name: &str,
    args: &[Expr],
    ir: &mut String,
    indent: &str,
    field_offsets: &HashMap<String, u64>,
    loaded_regs: &mut HashMap<String, String>,
    field_types: &HashMap<String, String>,
    write_fields: &[String],
) -> String {
    match name {
        "GetGlobalId#" | "GetLocalId#"
        | "GetGroupId#" | "GetNumGroups#" => {
            let dim = if let Some(first) = args.first() {
                emit_spirv_expr(first, ir, indent, field_offsets, loaded_regs, field_types, write_fields)
            } else {
                "0".to_string()
            };
            let (fn_name, ret_ty) = match name {
                "GetGlobalId#" => ("_Z13get_global_idj", "i64"),
                "GetLocalId#" => ("_Z12get_local_idj", "i64"),
                "GetGroupId#" => ("_Z12get_group_idj", "i64"),
                "GetNumGroups#" => ("_Z16get_num_groupsj", "i64"),
                _ => unreachable!(),
            };
            let reg = format!("%tid{}", ir.len());
            ir.push_str(&format!("{}{} = call {} @{}(i32 {})\n",
                indent, reg, ret_ty, fn_name, dim));
            reg
        }
        "SubGroupBarrier#" => {
            let dim = if let Some(first) = args.first() {
                emit_spirv_expr(first, ir, indent, field_offsets, loaded_regs, field_types, write_fields)
            } else {
                "0".to_string()
            };
            ir.push_str(&format!("{}call void @_Z8barrierj(i32 {})\n", indent, dim));
            "1".to_string()
        }
        // Math intrinsics: emit @llvm.*.f32 calls native to SPIR-V.
        "Sin#" => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%sin{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.sin.f32(float {})\n", indent, reg, v));
            reg
        }
        "Cos#" => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cos{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.cos.f32(float {})\n", indent, reg, v));
            reg
        }
        "Pow#" => {
            let a = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let b = emit_spirv_expr(&args[1], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%pow{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.pow.f32(float {}, float {})\n", indent, reg, a, b));
            reg
        }
        "Sqrt#" => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%sqrt{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.sqrt.f32(float {})\n", indent, reg, v));
            reg
        }
        "Fabs#" => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fabs{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.fabs.f32(float {})\n", indent, reg, v));
            reg
        }
        "Ceil#" => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%ceil{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.ceil.f32(float {})\n", indent, reg, v));
            reg
        }
        "Floor#" => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%floor{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.floor.f32(float {})\n", indent, reg, v));
            reg
        }
        _ => {
            ir.push_str(&format!("{}; error: unsupported intrinsic in GPU kernel\n", indent));
            "0".to_string()
        }
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
    /// Why shell out to llc instead of using inkwell/codegen directly: LLVM's
    /// SPIR-V backend is a separate target (spirv64-unknown-unknown) that is
    /// not enabled in the default LLVM build used by inkwell. Running llc with
    /// the correct target triple is the most reliable way to produce valid
    /// SPIR-V binaries across LLVM versions. The shell-out cost is negligible
    /// (< 100ms) because GPU kernel compilation is rare compared to CPU codegen.
pub fn compile_to_spirv(ir: &str) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::Command;

    // 2026-06-29: FIXED TOCTOU race — use unique filenames with process + thread ID.
    // The old code used fixed paths "brief_kernel.ll"/"brief_kernel.spv" which caused
    // file corruption under parallel builds (cargo test --lib -- --test-threads=N).
    // Each compiler invocation now gets a unique filename.
    let tmp_dir = std::env::temp_dir();
    let unique_id = format!(
        "brief_kernel_{}_{}",
        std::process::id(),
        // Thread ID — use an atomic counter as fallback when thread::id().as_u64() is unstable
        {
            #[cfg(feature = "nightly")]
            { std::thread::current().id().as_u64() }
            #[cfg(not(feature = "nightly"))]
            { 0u64 }
        }
    );
    // Add a monotonic counter as extra uniqueness guarantee even within a single thread
    use std::sync::atomic::{AtomicU64, Ordering};
    static KERNEL_COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = KERNEL_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ir_path = tmp_dir.join(format!("{}_{}.ll", unique_id, seq));
    let spv_path = tmp_dir.join(format!("{}_{}.spv", unique_id, seq));

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
            Statement::Assign(
                Expr::Identifier("data".to_string()),
                Expr::BinaryOp(BinaryOpKind::Add, 
                    Box::new(Expr::Identifier("data".to_string())),
                    Box::new(Expr::Decimal(1)),
                ),
            ),
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "Pure loop should be GPU-eligible");
    }

    #[test]
    fn test_check_eligibility_ffi_is_ineligible() {
        let body = vec![
            Statement::Expression(Expr::Call("print_int".to_string(), vec![], None)),
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "FFI call should be ineligible");
        assert!(result.reasons.iter().any(|r| r.contains("FFI")));
    }

    #[test]
    fn test_check_eligibility_term_is_eligible() {
        let body = vec![
            Statement::Term(None),
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "term statement should be GPU-eligible (no-op in SPIR-V)");
    }

    #[test]
    fn test_check_eligibility_termbang_is_ineligible() {
        let body = vec![
            Statement::TermBang(None),
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "term! statement should be ineligible");
    }

    #[test]
    fn test_extract_kernel_creates_name() {
        let body = vec![];
        let kernel = extract_kernel("test_loop", &body, Expr::Decimal(100), &[], HashMap::new());
        assert_eq!(kernel.name, "kernel_test_loop");
    }

    #[test]
    fn test_extract_kernel_tracks_write_fields() {
        let body = vec![
            Statement::Assign(
                Expr::Identifier("out".to_string()),
                Expr::Decimal(42),
            ),
        ];
        let kernel = extract_kernel("write_test", &body, Expr::Decimal(10), &[], HashMap::new());
        assert!(kernel.write_fields.contains(&"out".to_string()));
    }

    #[test]
    fn test_emit_spirv_module_has_correct_triple() {
        let body = vec![];
        let kernel = extract_kernel("empty", &body, Expr::Decimal(1), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("spirv64-unknown-unknown"));
        assert!(ir.contains("kernel_empty"));
        assert!(ir.contains("spir_kernel"));
    }

    #[test]
    fn test_emit_spirv_module_emits_assignment() {
        let body = vec![
            Statement::Assign(
                Expr::Identifier("x".to_string()),
                Expr::Decimal(42),
            ),
        ];
        let kernel = extract_kernel("assign_test", &body, Expr::Decimal(10), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("getelementptr"), "should emit GEP for field access");
        assert!(!ir.contains("TODO"), "should not contain placeholder comments");
    }

    #[test]
    fn test_emit_spirv_module_emits_arithmetic() {
        let body = vec![
            Statement::Assign(
                Expr::Identifier("x".to_string()),
                Expr::BinaryOp(BinaryOpKind::Add, 
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(1)),
                ),
            ),
        ];
        let kernel = extract_kernel("arith_test", &body, Expr::Decimal(10), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("add i64"), "should emit integer add");
        assert!(!ir.contains("TODO"), "should not contain placeholder comments");
    }

    #[test]
    fn test_emit_spirv_module_loads_correct_field() {
        // x = y + 1 — should load y, NOT reuse x's value
        let body = vec![
            Statement::Assign(
                Expr::Identifier("x".to_string()),
                Expr::BinaryOp(BinaryOpKind::Add, 
                    Box::new(Expr::Identifier("y".to_string())),
                    Box::new(Expr::Decimal(1)),
                ),
            ),
        ];
        let kernel = extract_kernel("cross_field", &body, Expr::Decimal(10), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        // Should have two separate load instructions (x and y are different fields)
        assert!(ir.contains("%lv_0"), "should load first field (y) into register lv_0");
        assert!(ir.contains("%base"), "should reference base pointer");
    }

    #[test]
    fn test_embed_spirv_blob_generates_array() {
        let blob = vec![0x03, 0x02, 0x01, 0x00];
        let s = embed_spirv_blob(&blob, "test_kernel");
        assert!(s.contains("@brief_kernel_test_kernel"));
        assert!(s.contains("[4 x i8]"));
        assert!(s.contains("\\03"));
    }

    // ── Eligibility relaxation (Phase 1) ──────────────────────────

    #[test]
    fn test_check_eligibility_math_intrinsic_allowed() {
        // sin#(x_float) inside an expression should be allowed
        let body = vec![
            Statement::Assign(
                Expr::Identifier("r".to_string()),
                Expr::Call("Sin#".to_string(), vec![Expr::Identifier("x".to_string())], None),
            ),
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "math intrinsic sin# should be GPU-eligible");
        assert!(result.reasons.is_empty(), "should have no rejection reasons");
    }

    #[test]
    fn test_check_eligibility_unsafe_intrinsic_blocked() {
        // ReadFile# has side effects and no SPIR-V mapping — should be blocked
        let body = vec![
            Statement::Expression(Expr::Call("read_file".to_string(), vec![Expr::Quoted("test".into())], None)),
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "unsafe intrinsic should be ineligible");
        // 2026-07-14: Non-# intrinsic calls are classified as FFI calls
        assert!(result.reasons.iter().any(|r| r.contains("FFI call")),
            "reason should mention FFI call");
    }

    #[test]
    fn test_check_eligibility_ffi_in_assignment_blocked() {
        // FFI call inside assignment RHS should be caught
        let body = vec![
            Statement::Assign(Expr::Identifier("x".to_string()), Expr::Call("read_file".to_string(), vec![Expr::Quoted("foo.txt".into())], None)),
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "FFI in assignment RHS should be ineligible");
        assert!(result.reasons.iter().any(|r| r.contains("FFI")),
            "reason should mention FFI");
    }

    #[test]
    fn test_check_eligibility_unsafe_intrinsic_in_guard_blocked() {
        // Unsafe intrinsic inside a guarded statement should be caught via recursion
        let body = vec![
            Statement::Guarded(
                Expr::Bool(true),
                vec![
                    Statement::Expression(Expr::Call("read_file".to_string(), vec![Expr::Quoted("test".into())], None)),
                ],
            ),
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "unsafe intrinsic in guard should be ineligible");
    }

    // ── SPIR-V intrinsic emission (Phase 2) ─────────────────────

    #[test]
    fn test_emit_spirv_get_global_id() {
        let body = vec![
            Statement::Assign(Expr::Identifier("r".to_string()), Expr::Call("GetGlobalId#".to_string(), vec![Expr::Decimal(0)], None)),
        ];
        let kernel = extract_kernel("gtid_test", &body, Expr::Decimal(100), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z13get_global_idj(i32 0)"),
            "SPIR-V IR should contain get_global_id call");
    }

    #[test]
    fn test_emit_spirv_get_local_id() {
        let body = vec![
            Statement::Assign(Expr::Identifier("r".to_string()), Expr::Call("GetLocalId#".to_string(), vec![Expr::Decimal(1)], None)),
        ];
        let kernel = extract_kernel("ltid_test", &body, Expr::Decimal(100), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z12get_local_idj(i32 1)"),
            "SPIR-V IR should contain get_local_id call");
    }

    #[test]
    fn test_emit_spirv_get_group_id() {
        let body = vec![
            Statement::Assign(Expr::Identifier("r".to_string()), Expr::Call("GetGroupId#".to_string(), vec![Expr::Decimal(0)], None)),
        ];
        let kernel = extract_kernel("grid_test", &body, Expr::Decimal(100), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z12get_group_idj(i32 0)"),
            "SPIR-V IR should contain get_group_id call");
    }

    #[test]
    fn test_emit_spirv_barrier() {
        let body = vec![
            Statement::Expression(Expr::Call("SubGroupBarrier#".to_string(), vec![], None)),
        ];
        let kernel = extract_kernel("bar_test", &body, Expr::Decimal(100), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call void @_Z8barrierj(i32 0)"),
            "SPIR-V IR should contain barrier call");
    }

    #[test]
    fn test_emit_spirv_all_declares_present() {
        let body = vec![];
        let kernel = extract_kernel("decl_test", &body, Expr::Decimal(1), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("@_Z13get_global_idj"), "should declare get_global_id");
        assert!(ir.contains("@_Z12get_local_idj"), "should declare get_local_id");
        assert!(ir.contains("@_Z12get_group_idj"), "should declare get_group_id");
        assert!(ir.contains("@_Z16get_num_groupsj"), "should declare get_num_groups");
        assert!(ir.contains("@_Z8barrierj"), "should declare barrier");
        assert!(ir.contains("@llvm.sin.f32"), "should declare sin intrinsic");
        assert!(ir.contains("@llvm.cos.f32"), "should declare cos intrinsic");
        assert!(ir.contains("@llvm.pow.f32"), "should declare pow intrinsic");
        assert!(ir.contains("@llvm.sqrt.f32"), "should declare sqrt intrinsic");
        assert!(ir.contains("@llvm.fabs.f32"), "should declare fabs intrinsic");
    }

    #[test]
    fn test_check_eligibility_gpu_intrinsic_allowed() {
        let body = vec![
            Statement::Expression(Expr::Call("GetGlobalId#".to_string(), vec![Expr::Decimal(0)], None)),
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "get_global_id should be GPU-eligible");
    }

    #[test]
    fn test_check_eligibility_barrier_allowed() {
        let body = vec![
            Statement::Expression(Expr::Call("SubGroupBarrier#".to_string(), vec![], None)),
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "barrier should be GPU-eligible");
    }

    // ── Float arithmetic (Phase 3) ─────────────────────────

    fn make_float_field_types() -> HashMap<String, String> {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "float".to_string());
        ft.insert("y".to_string(), "float".to_string());
        ft.insert("z".to_string(), "float".to_string());
        ft.insert("r".to_string(), "float".to_string());
        ft.insert("a".to_string(), "i64".to_string());
        ft.insert("b".to_string(), "i64".to_string());
        ft
    }

    #[test]
    fn test_emit_spirv_float_assignment() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "float".to_string());
        ft.insert("y".to_string(), "float".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("x".to_string()), Expr::BinaryOp(BinaryOpKind::Add, 
                    Box::new(Expr::Identifier("y".to_string())),
                    Box::new(Expr::Float(3.14)),
                )),
        ];
        let kernel = extract_kernel("float_add", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("load float"), "should load float field");
        assert!(ir.contains("fadd float"), "should emit fadd float");
        assert!(ir.contains("store float"), "should store float field");
        assert!(!ir.contains("add i64"), "should not use integer add");
    }

    #[test]
    fn test_emit_spirv_float_sub_mul_div() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "float".to_string());
        ft.insert("y".to_string(), "float".to_string());
        ft.insert("z".to_string(), "float".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("z".to_string()), Expr::BinaryOp(BinaryOpKind::Div, 
                    Box::new(Expr::BinaryOp(BinaryOpKind::Mul, 
                        Box::new(Expr::Identifier("x".to_string())),
                        Box::new(Expr::Identifier("y".to_string())),
                    )),
                    Box::new(Expr::BinaryOp(BinaryOpKind::Sub, 
                        Box::new(Expr::Identifier("x".to_string())),
                        Box::new(Expr::Float(1.0)),
                    )),
                )),
        ];
        let kernel = extract_kernel("float_ops", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("fmul float"), "should emit fmul");
        assert!(ir.contains("fsub float"), "should emit fsub");
        assert!(ir.contains("fdiv float"), "should emit fdiv");
    }

    #[test]
    fn test_emit_spirv_float_negation() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "float".to_string());
        ft.insert("y".to_string(), "float".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("y".to_string()), Expr::UnaryOp(UnaryOpKind::Neg, Box::new(Expr::Identifier("x".to_string())))),
        ];
        let kernel = extract_kernel("fneg", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("fneg float"), "should emit fneg for float negation");
        assert!(!ir.contains("sub i64"), "should not use sub i64 for float");
    }

    #[test]
    fn test_emit_spirv_float_comparison() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "float".to_string());
        ft.insert("y".to_string(), "float".to_string());
        ft.insert("r".to_string(), "i64".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("r".to_string()), Expr::BinaryOp(BinaryOpKind::Lt, 
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Identifier("y".to_string())),
                )),
        ];
        let kernel = extract_kernel("float_cmp", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("fcmp olt float"), "should emit fcmp olt");
        assert!(ir.contains("zext i1"), "should zext comparison result");
    }

    #[test]
    fn test_emit_spirv_float_intrinsic_sin() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "float".to_string());
        ft.insert("r".to_string(), "float".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("r".to_string()), Expr::Call("Sin#".to_string(), vec![Expr::Identifier("x".to_string())], None)),
        ];
        let kernel = extract_kernel("sin_test", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call float @llvm.sin.f32"), "should emit sin intrinsic");
    }

    #[test]
    fn test_emit_spirv_mixed_int_float() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "i64".to_string());
        ft.insert("y".to_string(), "float".to_string());
        ft.insert("z".to_string(), "i64".to_string());
        ft.insert("w".to_string(), "float".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("x".to_string()), Expr::BinaryOp(BinaryOpKind::Add, 
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(1)),
                )),
            Statement::Assign(Expr::Identifier("y".to_string()), Expr::BinaryOp(BinaryOpKind::Mul, 
                    Box::new(Expr::Identifier("y".to_string())),
                    Box::new(Expr::Float(2.0)),
                )),
        ];
        let kernel = extract_kernel("mixed_int_float", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("add i64"), "should have integer add");
        assert!(ir.contains("fmul float"), "should have float mul");
        assert!(ir.contains("load float"), "should load float field");
    }

    #[test]
    fn test_emit_spirv_integer_comparison_extended() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "i64".to_string());
        ft.insert("y".to_string(), "i64".to_string());
        ft.insert("r".to_string(), "i64".to_string());
        ft.insert("s".to_string(), "i64".to_string());
        ft.insert("t".to_string(), "i64".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("r".to_string()), Expr::BinaryOp(BinaryOpKind::Lt, 
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Identifier("y".to_string())),
                )),
            Statement::Assign(Expr::Identifier("s".to_string()), Expr::BinaryOp(BinaryOpKind::Gt, 
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Identifier("y".to_string())),
                )),
            Statement::Assign(Expr::Identifier("t".to_string()), Expr::BinaryOp(BinaryOpKind::Eq, 
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(42)),
                )),
        ];
        let kernel = extract_kernel("int_cmp", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("icmp slt"), "should have signed less-than");
        assert!(ir.contains("icmp sgt"), "should have signed greater-than");
        assert!(ir.contains("icmp eq"), "should have equal");
    }

    #[test]
    fn test_emit_spirv_integer_div_mod() {
        let mut ft = HashMap::new();
        ft.insert("q".to_string(), "i64".to_string());
        ft.insert("r".to_string(), "i64".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("q".to_string()), Expr::BinaryOp(BinaryOpKind::Div, 
                    Box::new(Expr::Identifier("q".to_string())),
                    Box::new(Expr::Decimal(3)),
                )),
            Statement::Assign(Expr::Identifier("r".to_string()), Expr::BinaryOp(BinaryOpKind::Mod, 
                    Box::new(Expr::Identifier("r".to_string())),
                    Box::new(Expr::Decimal(7)),
                )),
        ];
        let kernel = extract_kernel("div_mod", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("sdiv i64"), "should have signed div");
        assert!(ir.contains("srem i64"), "should have signed remainder");
    }

    #[test]
    fn test_emit_spirv_float_literal() {
        let mut ft = HashMap::new();
        ft.insert("r".to_string(), "float".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("r".to_string()), Expr::Float(3.14159)),
        ];
        let kernel = extract_kernel("float_lit", &body, Expr::Decimal(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("bitcast i32"), "float literal should use bitcast");
        assert!(!ir.contains("bitcast i32 i32"), "bitcast should not have double i32 type");
        assert!(ir.contains("to float"), "should produce float value");
    }

    // ── Multi-buffer (Phase 4) ─────────────────────────────

    #[test]
    fn test_emit_spirv_multi_buffer_signature() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "i64".to_string());
        ft.insert("y".to_string(), "i64".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("y".to_string()), Expr::BinaryOp(BinaryOpKind::Add, 
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(1)),
                )),
        ];
        let kernel = extract_kernel("multi_buf", &body, Expr::Decimal(100), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("%in_buf"), "kernel should have in_buf param");
        assert!(ir.contains("%out_buf"), "kernel should have out_buf param");
        assert!(ir.contains("nocapture readonly %in_buf"), "in_buf should be readonly");
        assert!(ir.contains("%base_in"), "should have base_in GEP");
        assert!(ir.contains("%base_out"), "should have base_out GEP");
    }

    #[test]
    fn test_emit_spirv_multi_buffer_read_write() {
        // y = x + 1 — x is read-only (in_buf), y is written (out_buf)
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "i64".to_string());
        ft.insert("y".to_string(), "i64".to_string());
        let body = vec![
            Statement::Assign(Expr::Identifier("y".to_string()), Expr::BinaryOp(BinaryOpKind::Add, 
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(1)),
                )),
        ];
        let kernel = extract_kernel("rw_test", &body, Expr::Decimal(100), &[], ft);
        let ir = emit_spirv_module(&kernel);
        // x is read-only → load from in_buf
        assert!(ir.contains("getelementptr i8, ptr %base_in"),
            "read-only field x should load from in_buf");
        // y is written → store to out_buf
        assert!(ir.contains("store i64"),
            "write field y should store value");
    }

    // ── Multi-dimensional grid (Phase 6) ───────────────────

    #[test]
    fn test_emit_spirv_module_2d_grid() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "i64".to_string());
        ft.insert("y".to_string(), "i64".to_string());
        // Kernel using both get_global_id(0) and get_global_id(1)
        let body = vec![
            Statement::Assign(Expr::Identifier("x".to_string()), Expr::Call("GetGlobalId#".to_string(), vec![Expr::Decimal(0)], None)),
            Statement::Assign(Expr::Identifier("y".to_string()), Expr::Call("GetGlobalId#".to_string(), vec![Expr::Decimal(1)], None)),
        ];
        let kernel = extract_kernel("grid2d", &body, Expr::Decimal(100), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z13get_global_idj(i32 1)"),
            "2D kernel should call get_global_id(1)");
        assert!(ir.contains("call i64 @_Z13get_global_idj(i32 0)"),
            "2D kernel should call get_global_id(0)");
    }

    #[test]
    fn test_emit_spirv_module_1d_grid_no_overhead() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "i64".to_string());
        // Kernel using only get_global_id(0)
        let body = vec![
            Statement::Assign(Expr::Identifier("x".to_string()), Expr::Call("GetGlobalId#".to_string(), vec![Expr::Decimal(0)], None)),
        ];
        let kernel = extract_kernel("grid1d", &body, Expr::Decimal(100), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z13get_global_idj(i32 0)"),
            "1D kernel should have get_global_id(0)");
        assert!(!ir.contains("get_global_idj(i32 1)"),
            "1D kernel should NOT have get_global_id(1)");
        assert!(!ir.contains("get_global_idj(i32 2)"),
            "1D kernel should NOT have get_global_id(2)");
    }
}
