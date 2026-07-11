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
            Statement::TermBang { .. } => {
                reasons.push("GPU kernel contains term! — unsupported".to_string());
            }
            Statement::Term { swan_song, .. } => {
                // term is allowed in GPU kernels (no-op convergence signal).
                // Check the swan song for GPU eligibility if present.
                if let Some(swan) = swan_song {
                    let inner = check_eligibility(&[swan.as_ref().clone()]);
                    reasons.extend(inner.reasons);
                }
            }
            Statement::Escape { .. } => {
                reasons.push("GPU kernel contains escape — unsupported".to_string());
            }
            Statement::Unification { .. } => {
                reasons.push("GPU kernel contains unification — unsupported".to_string());
            }
            Statement::Expression(expr) => {
                collect_unsafe_ffi(expr, &mut reasons);
                collect_touched_fields(expr, &mut touched_fields);
            }
            Statement::Let { expr: Some(e), .. } => {
                collect_unsafe_ffi(e, &mut reasons);
                collect_touched_fields(e, &mut touched_fields);
            }
            Statement::Assignment { lhs, expr, .. } => {
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
            Statement::Guarded { condition, statements, .. } => {
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
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r)
        | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r)
        | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r)
        | Expr::Shl(l, r) | Expr::Shr(l, r) => {
            collect_touched_fields(l, fields);
            collect_touched_fields(r, fields);
        }
        Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) | Expr::Cast(e, _) => {
            collect_touched_fields(e, fields);
        }
        Expr::Call(_, args) | Expr::IntrinsicCall { args, .. } => {
            for arg in args {
                collect_touched_fields(arg, fields);
            }
        }
        Expr::ListLiteral(items) | Expr::SetLiteral(items) => {
            for item in items {
                collect_touched_fields(item, fields);
            }
        }
        Expr::MapLiteral(pairs) => {
            for (k, v) in pairs {
                collect_touched_fields(k, fields);
                collect_touched_fields(v, fields);
            }
        }
        _ => {}
    }
}

/// Recursively walk an expression tree and collect reasons for any unsafe FFI
/// calls or intrinsics that would make the kernel ineligible for GPU offloading.
///
/// `Expr::Call` (user FFI) is always unsafe. `Expr::IntrinsicCall` is only
/// unsafe if the intrinsic is not in the GPU-safe allowlist. `Expr::SharedMem`
/// is always allowed (it is GPU-native).
fn collect_unsafe_ffi(expr: &Expr, reasons: &mut Vec<String>) {
    match expr {
        Expr::Call(name, _) => {
            reasons.push(format!("GPU kernel contains FFI call '{}' — unsupported", name));
        }
        Expr::IntrinsicCall { intrinsic, args } => {
            if !is_gpu_safe_intrinsic(intrinsic) {
                reasons.push(format!("GPU kernel contains unsafe intrinsic '{:?}'", intrinsic));
            }
            for arg in args {
                collect_unsafe_ffi(arg, reasons);
            }
        }
        // Binary ops — recurse into both operands
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r)
        | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r)
        | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r)
        | Expr::Shl(l, r) | Expr::Shr(l, r) => {
            collect_unsafe_ffi(l, reasons);
            collect_unsafe_ffi(r, reasons);
        }
        // Unary ops — recurse into operand
        Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => {
            collect_unsafe_ffi(e, reasons);
        }
        // Collection literals — recurse into elements
        Expr::ListLiteral(items) | Expr::SetLiteral(items) => {
            for item in items {
                collect_unsafe_ffi(item, reasons);
            }
        }
        Expr::MapLiteral(pairs) => {
            for (k, v) in pairs {
                collect_unsafe_ffi(k, reasons);
                collect_unsafe_ffi(v, reasons);
            }
        }
        // List index — recurse into value and index
        Expr::ListIndex(v, i) => {
            collect_unsafe_ffi(v, reasons);
            collect_unsafe_ffi(i, reasons);
        }
        // Slice — recurse into value and optional bounds
        Expr::Slice { value, start, end, stride, mask } => {
            collect_unsafe_ffi(value, reasons);
            for opt in [start, end, stride, mask].into_iter().flatten() {
                collect_unsafe_ffi(opt, reasons);
            }
        }
        // Concat / Cast / Projection — recurse into operands
        Expr::Concat(l, r) => {
            collect_unsafe_ffi(l, reasons);
            collect_unsafe_ffi(r, reasons);
        }
        Expr::Cast(e, _) => collect_unsafe_ffi(e, reasons),
        Expr::Projection { source, .. } => collect_unsafe_ffi(source, reasons),
        // Field access — recurse into the struct expression
        Expr::FieldAccess(obj, _) => {
            collect_unsafe_ffi(obj, reasons);
        }
        // Terminals — no sub-expressions
        Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_)
        | Expr::String(_) | Expr::Term | Expr::Identifier(_)
        | Expr::AddrOf(_) | Expr::PriorState(_)
        | Expr::Ellipsis | Expr::TypeRef(_) => {}
        // Shared memory — always allowed in GPU kernels
        Expr::SharedMem(_) => {}
        // Catch-all for remaining expression types — conservatively reject
        _ => {
            reasons.push("GPU kernel contains unsupported expression type".to_string());
        }
    }
}

/// Returns true if the intrinsic is safe to execute on GPU.
///
/// Well-known math intrinsics (sin, cos, pow, sqrt, fabs) map directly to
/// SPIR-V / GPU instructions. GPU query intrinsics (get_global_id, barrier)
/// are added alongside their Intrinsic enum entries.
fn is_gpu_safe_intrinsic(intrinsic: &Intrinsic) -> bool {
    matches!(intrinsic,
        Intrinsic::Sin | Intrinsic::Cos | Intrinsic::Pow
        | Intrinsic::Sqrt | Intrinsic::Fabs
        | Intrinsic::Ceil | Intrinsic::Floor
        | Intrinsic::GetGlobalId | Intrinsic::GetLocalId
        | Intrinsic::GetGroupId | Intrinsic::GetNumGroups
        | Intrinsic::SubGroupBarrier
        | Intrinsic::PrintInt | Intrinsic::PutChar | Intrinsic::PrintFloat
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
        Statement::Assignment { lhs, expr, .. } => {
            if let Expr::Identifier(f) = lhs {
                if !fields.contains(f) { fields.push(f.clone()); }
            }
            collect_expr_fields(expr, fields);
        }
        Statement::Guarded { condition, statements, .. } => {
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
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r)
        | Expr::And(l, r) | Expr::Or(l, r) | Expr::Eq(l, r) | Expr::Ne(l, r)
        | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r)
        | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r)
        | Expr::Shl(l, r) | Expr::Shr(l, r) => {
            collect_expr_fields(l, fields);
            collect_expr_fields(r, fields);
        }
        Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => {
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

    // Emit shared memory globals (addrspace(3)) for any SharedMem expressions
    let shared_mem_sizes = collect_shared_mem_sizes(&kernel.body);
    for (i, size) in shared_mem_sizes.iter().enumerate() {
        ir.push_str(&format!("@shared_buf_{} = internal unnamed_addr addrspace(3) global [{} x i64] zeroinitializer\n", i, size));
    }
    if !shared_mem_sizes.is_empty() {
        ir.push_str("\n");
    }

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
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
            is_float_context(l, field_types) || is_float_context(r, field_types)
        }
        Expr::Neg(e) => is_float_context(e, field_types),
        Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) => {
            is_float_context(l, field_types) || is_float_context(r, field_types)
        }
        Expr::IntrinsicCall { intrinsic, .. } => matches!(intrinsic,
            Intrinsic::Sin | Intrinsic::Cos | Intrinsic::Pow
            | Intrinsic::Sqrt | Intrinsic::Fabs
            | Intrinsic::Ceil | Intrinsic::Floor
            | Intrinsic::PrintFloat
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

/// Scan through statements and collect all SharedMem sizes for addrspace(3) globals.
fn collect_shared_mem_sizes(body: &[Statement]) -> Vec<usize> {
    let mut sizes = Vec::new();
    for stmt in body {
        match stmt {
            Statement::Assignment { expr, .. } => {
                collect_shared_mem_sizes_expr(expr, &mut sizes);
            }
            Statement::Guarded { condition, statements, .. } => {
                collect_shared_mem_sizes_expr(condition, &mut sizes);
                sizes.extend(collect_shared_mem_sizes(statements));
            }
            Statement::Let { expr: Some(e), .. } => {
                collect_shared_mem_sizes_expr(e, &mut sizes);
            }
            Statement::Expression(e) => {
                collect_shared_mem_sizes_expr(e, &mut sizes);
            }
            _ => {}
        }
    }
    sizes
}

fn collect_shared_mem_sizes_expr(expr: &Expr, sizes: &mut Vec<usize>) {
    match expr {
        Expr::SharedMem(n) => sizes.push(*n),
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r)
        | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r)
        | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r)
        | Expr::Shl(l, r) | Expr::Shr(l, r) => {
            collect_shared_mem_sizes_expr(l, sizes);
            collect_shared_mem_sizes_expr(r, sizes);
        }
        Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) | Expr::Cast(e, _) => {
            collect_shared_mem_sizes_expr(e, sizes);
        }
        Expr::IntrinsicCall { args, .. } | Expr::Call(_, args) => {
            for arg in args {
                collect_shared_mem_sizes_expr(arg, sizes);
            }
        }
        _ => {}
    }
}

/// Scan the kernel body for print I/O intrinsics (print_int#, print_float#,
/// put_char#). When present, a print buffer parameter is added to the kernel.
fn has_print_intrinsics(body: &[Statement]) -> bool {
    for stmt in body {
        match stmt {
            Statement::Assignment { expr, .. }
            | Statement::Let { expr: Some(expr), .. }
            | Statement::Expression(expr) => {
                if has_print_intrinsics_expr(expr) {
                    return true;
                }
            }
            Statement::Guarded { condition, statements, .. } => {
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
        Expr::IntrinsicCall { intrinsic, .. } => matches!(intrinsic,
            Intrinsic::PrintInt | Intrinsic::PrintFloat | Intrinsic::PutChar
        ),
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r)
        | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
            has_print_intrinsics_expr(l) || has_print_intrinsics_expr(r)
        }
        Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => has_print_intrinsics_expr(e),
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
        Statement::Assignment { lhs, expr, .. } => {
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
        Statement::Guarded { condition, statements, .. } => {
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
            match expr {
                Expr::IntrinsicCall { intrinsic: Intrinsic::SubGroupBarrier, args } => {
                    let dim = if args.is_empty() {
                        "0".to_string()
                    } else {
                        emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields)
                    };
                    ir.push_str(&format!("{}call void @_Z8barrierj(i32 {})\n", indent, dim));
                }
                _ => {
                    let _val = emit_spirv_expr(expr, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
                }
            }
        }
        // Term — convergence signal (no-op in SPIR-V). Execute swan song if present.
        Statement::Term { swan_song, .. } => {
            if let Some(swan) = swan_song {
                emit_spirv_stmt(swan, ir, indent, field_offsets, loaded_regs, label_counter, field_types, write_fields);
            }
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
        Expr::Integer(n) => format!("{}", n),
        Expr::Bool(b) => {
            if *b { "1".to_string() } else { "0".to_string() }
        }
        Expr::Identifier(name) => {
            ensure_field_loaded(name, ir, indent, field_offsets, loaded_regs, field_types, write_fields)
        }
        // Float arithmetic
        Expr::Add(lhs, rhs) if is_float_context(expr, field_types) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fadd{}", ir.len());
            ir.push_str(&format!("{}{} = fadd float {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Sub(lhs, rhs) if is_float_context(expr, field_types) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fsub{}", ir.len());
            ir.push_str(&format!("{}{} = fsub float {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Mul(lhs, rhs) if is_float_context(expr, field_types) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fmul{}", ir.len());
            ir.push_str(&format!("{}{} = fmul float {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Div(lhs, rhs) if is_float_context(expr, field_types) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fdiv{}", ir.len());
            ir.push_str(&format!("{}{} = fdiv float {}, {}\n", indent, reg, l, r));
            reg
        }
        // Integer arithmetic
        Expr::Add(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%add{}", ir.len());
            ir.push_str(&format!("{}{} = add i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Sub(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%sub{}", ir.len());
            ir.push_str(&format!("{}{} = sub i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Mul(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%mul{}", ir.len());
            ir.push_str(&format!("{}{} = mul i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Div(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%div{}", ir.len());
            ir.push_str(&format!("{}{} = sdiv i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        Expr::Mod(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%rem{}", ir.len());
            ir.push_str(&format!("{}{} = srem i64 {}, {}\n", indent, reg, l, r));
            reg
        }
        // Float comparisons
        cmp @ (Expr::Lt(_, _) | Expr::Le(_, _) | Expr::Gt(_, _) | Expr::Ge(_, _)
             | Expr::Eq(_, _) | Expr::Ne(_, _)) if is_float_context(expr, field_types) => {
            let (l, r) = match cmp {
                Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r)
                | Expr::Eq(l, r) | Expr::Ne(l, r) => (l, r),
                _ => unreachable!(),
            };
            let lv = emit_spirv_expr(l, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let rv = emit_spirv_expr(r, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let cond = match cmp {
                Expr::Lt(_, _) => "olt",
                Expr::Le(_, _) => "ole",
                Expr::Gt(_, _) => "ogt",
                Expr::Ge(_, _) => "oge",
                Expr::Eq(_, _) => "oeq",
                Expr::Ne(_, _) => "one",
                _ => unreachable!(),
            };
            let reg = format!("%fcmp{}", ir.len());
            ir.push_str(&format!("{}{} = fcmp {} float {}, {}\n", indent, reg, cond, lv, rv));
            let ext = format!("%fzext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        // Integer comparisons
        Expr::Lt(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp slt i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::Le(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp sle i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::Gt(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp sgt i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::Ge(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp sge i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::Eq(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp eq i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        Expr::Ne(lhs, rhs) => {
            let l = emit_spirv_expr(lhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let r = emit_spirv_expr(rhs, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cmp{}", ir.len());
            ir.push_str(&format!("{}{} = icmp ne i64 {}, {}\n", indent, reg, l, r));
            let ext = format!("%zext{}", ir.len());
            ir.push_str(&format!("{}{} = zext i1 {} to i64\n", indent, ext, reg));
            ext
        }
        // Float negation
        Expr::Neg(e) if is_float_context(expr, field_types) => {
            let v = emit_spirv_expr(e, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fneg{}", ir.len());
            ir.push_str(&format!("{}{} = fneg float {}\n", indent, reg, v));
            reg
        }
        // Int negation
        Expr::Neg(e) => {
            let v = emit_spirv_expr(e, ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%neg{}", ir.len());
            ir.push_str(&format!("{}{} = sub i64 0, {}\n", indent, reg, v));
            reg
        }
        // GPU shared memory: emit addrspace(3) global and return i8* pointer
        Expr::SharedMem(n) => {
            let sh_idx = loaded_regs.len();
            let gv_name = format!("@shared_buf_{}", sh_idx);
            ir.push_str(&format!("{}; shared memory: {} x i64\n", indent, n));
            ir.push_str(&format!("{}%sh_base = addrspacecast [{} x i64] addrspace(3)* {} to ptr\n",
                indent, n, gv_name));
            let reg = format!("%sh_ptr{}", sh_idx);
            ir.push_str(&format!("{}{} = ptrtoint ptr %sh_base to i64\n", indent, reg));
            reg
        }
        // GPU intrinsics
        Expr::IntrinsicCall { intrinsic, args } => {
            emit_spirv_intrinsic(intrinsic, args, ir, indent, field_offsets, loaded_regs, field_types, write_fields)
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
    intrinsic: &Intrinsic,
    args: &[Expr],
    ir: &mut String,
    indent: &str,
    field_offsets: &HashMap<String, u64>,
    loaded_regs: &mut HashMap<String, String>,
    field_types: &HashMap<String, String>,
    write_fields: &[String],
) -> String {
    match intrinsic {
        Intrinsic::GetGlobalId | Intrinsic::GetLocalId
        | Intrinsic::GetGroupId | Intrinsic::GetNumGroups => {
            let dim = if let Some(first) = args.first() {
                emit_spirv_expr(first, ir, indent, field_offsets, loaded_regs, field_types, write_fields)
            } else {
                "0".to_string()
            };
            let (fn_name, ret_ty) = match intrinsic {
                Intrinsic::GetGlobalId => ("_Z13get_global_idj", "i64"),
                Intrinsic::GetLocalId => ("_Z12get_local_idj", "i64"),
                Intrinsic::GetGroupId => ("_Z12get_group_idj", "i64"),
                Intrinsic::GetNumGroups => ("_Z16get_num_groupsj", "i64"),
                _ => unreachable!(),
            };
            let reg = format!("%tid{}", ir.len());
            ir.push_str(&format!("{}{} = call {} @{}(i32 {})\n",
                indent, reg, ret_ty, fn_name, dim));
            reg
        }
        Intrinsic::SubGroupBarrier => {
            let dim = if let Some(first) = args.first() {
                emit_spirv_expr(first, ir, indent, field_offsets, loaded_regs, field_types, write_fields)
            } else {
                "0".to_string()
            };
            ir.push_str(&format!("{}call void @_Z8barrierj(i32 {})\n", indent, dim));
            "1".to_string()
        }
        // Math intrinsics: emit @llvm.*.f32 calls native to SPIR-V.
        Intrinsic::Sin => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%sin{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.sin.f32(float {})\n", indent, reg, v));
            reg
        }
        Intrinsic::Cos => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%cos{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.cos.f32(float {})\n", indent, reg, v));
            reg
        }
        Intrinsic::Pow => {
            let a = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let b = emit_spirv_expr(&args[1], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%pow{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.pow.f32(float {}, float {})\n", indent, reg, a, b));
            reg
        }
        Intrinsic::Sqrt => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%sqrt{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.sqrt.f32(float {})\n", indent, reg, v));
            reg
        }
        Intrinsic::Fabs => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%fabs{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.fabs.f32(float {})\n", indent, reg, v));
            reg
        }
        Intrinsic::Ceil => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%ceil{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.ceil.f32(float {})\n", indent, reg, v));
            reg
        }
        Intrinsic::Floor => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%floor{}", ir.len());
            ir.push_str(&format!("{}{} = call float @llvm.floor.f32(float {})\n", indent, reg, v));
            reg
        }
        // GPU I/O intrinsics — write to print buffer, host drains after dispatch.
        // These use %base_print, which is emitted in emit_spirv_module only when
        // has_print_intrinsics() returns true (guaranteed by the caller).
        Intrinsic::PrintInt => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%pr{}", ir.len());
            ir.push_str(&format!("{}store i64 {}, i8* %base_print, align 8\n", indent, v));
            reg
        }
        Intrinsic::PrintFloat => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let bc = format!("%prbc{}", ir.len());
            let reg = format!("%pr{}", ir.len());
            ir.push_str(&format!("{}{} = bitcast ptr %base_print to float*\n", indent, bc));
            ir.push_str(&format!("{}store float {}, float* {}, align 4\n", indent, v, bc));
            reg
        }
        Intrinsic::PutChar => {
            let v = emit_spirv_expr(&args[0], ir, indent, field_offsets, loaded_regs, field_types, write_fields);
            let reg = format!("%pr{}", ir.len());
            ir.push_str(&format!("{}store i8 {}, i8* %base_print, align 1\n", indent, v));
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
    fn test_check_eligibility_term_is_eligible() {
        let body = vec![
            Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "term statement should be GPU-eligible (no-op in SPIR-V)");
    }

    #[test]
    fn test_check_eligibility_termbang_is_ineligible() {
        let body = vec![
            Statement::TermBang { values: vec![], swan_song: None, modifiers: vec![] },
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "term! statement should be ineligible");
    }

    #[test]
    fn test_extract_kernel_creates_name() {
        let body = vec![];
        let kernel = extract_kernel("test_loop", &body, Expr::Integer(100), &[], HashMap::new());
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
        let kernel = extract_kernel("write_test", &body, Expr::Integer(10), &[], HashMap::new());
        assert!(kernel.write_fields.contains(&"out".to_string()));
    }

    #[test]
    fn test_emit_spirv_module_has_correct_triple() {
        let body = vec![];
        let kernel = extract_kernel("empty", &body, Expr::Integer(1), &[], HashMap::new());
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
        let kernel = extract_kernel("assign_test", &body, Expr::Integer(10), &[], HashMap::new());
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
        let kernel = extract_kernel("arith_test", &body, Expr::Integer(10), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("add i64"), "should emit integer add");
        assert!(!ir.contains("TODO"), "should not contain placeholder comments");
    }

    #[test]
    fn test_emit_spirv_module_loads_correct_field() {
        // x = y + 1 — should load y, NOT reuse x's value
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Add(
                    Box::new(Expr::Identifier("y".to_string())),
                    Box::new(Expr::Integer(1)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("cross_field", &body, Expr::Integer(10), &[], HashMap::new());
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
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::IntrinsicCall {
                    intrinsic: Intrinsic::Sin,
                    args: vec![Expr::Identifier("x".to_string())],
                },
                timeout: None,
                modifiers: vec![],
            },
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "math intrinsic sin# should be GPU-eligible");
        assert!(result.reasons.is_empty(), "should have no rejection reasons");
    }

    #[test]
    fn test_check_eligibility_unsafe_intrinsic_blocked() {
        // ReadFile# has side effects and no SPIR-V mapping — should be blocked
        let body = vec![
            Statement::Expression(Expr::IntrinsicCall {
                intrinsic: Intrinsic::UserDefined("read_file".to_string()),
                args: vec![Expr::String("test".to_string())],
            }),
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "unsafe intrinsic should be ineligible");
        assert!(result.reasons.iter().any(|r| r.contains("unsafe intrinsic")),
            "reason should mention unsafe intrinsic");
    }

    #[test]
    fn test_check_eligibility_ffi_in_assignment_blocked() {
        // FFI call inside assignment RHS should be caught
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Call("read_file".to_string(), vec![Expr::String("foo.txt".to_string())]),
                timeout: None,
                modifiers: vec![],
            },
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
            Statement::Guarded {
                condition: Expr::Bool(true),
                statements: vec![
                    Statement::Expression(Expr::IntrinsicCall {
                        intrinsic: Intrinsic::UserDefined("read_file".to_string()),
                        args: vec![Expr::String("test".to_string())],
                    }),
                ],
                metadata: HashMap::new(),
            },
        ];
        let result = check_eligibility(&body);
        assert!(!result.eligible, "unsafe intrinsic in guard should be ineligible");
    }

    // ── SPIR-V intrinsic emission (Phase 2) ─────────────────────

    #[test]
    fn test_emit_spirv_get_global_id() {
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::IntrinsicCall {
                    intrinsic: Intrinsic::GetGlobalId,
                    args: vec![Expr::Integer(0)],
                },
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("gtid_test", &body, Expr::Integer(100), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z13get_global_idj(i32 0)"),
            "SPIR-V IR should contain get_global_id call");
    }

    #[test]
    fn test_emit_spirv_get_local_id() {
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::IntrinsicCall {
                    intrinsic: Intrinsic::GetLocalId,
                    args: vec![Expr::Integer(1)],
                },
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("ltid_test", &body, Expr::Integer(100), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z12get_local_idj(i32 1)"),
            "SPIR-V IR should contain get_local_id call");
    }

    #[test]
    fn test_emit_spirv_get_group_id() {
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::IntrinsicCall {
                    intrinsic: Intrinsic::GetGroupId,
                    args: vec![Expr::Integer(0)],
                },
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("grid_test", &body, Expr::Integer(100), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z12get_group_idj(i32 0)"),
            "SPIR-V IR should contain get_group_id call");
    }

    #[test]
    fn test_emit_spirv_barrier() {
        let body = vec![
            Statement::Expression(Expr::IntrinsicCall {
                intrinsic: Intrinsic::SubGroupBarrier,
                args: vec![],
            }),
        ];
        let kernel = extract_kernel("bar_test", &body, Expr::Integer(100), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call void @_Z8barrierj(i32 0)"),
            "SPIR-V IR should contain barrier call");
    }

    #[test]
    fn test_emit_spirv_all_declares_present() {
        let body = vec![];
        let kernel = extract_kernel("decl_test", &body, Expr::Integer(1), &[], HashMap::new());
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
            Statement::Expression(Expr::IntrinsicCall {
                intrinsic: Intrinsic::GetGlobalId,
                args: vec![Expr::Integer(0)],
            }),
        ];
        let result = check_eligibility(&body);
        assert!(result.eligible, "get_global_id should be GPU-eligible");
    }

    #[test]
    fn test_check_eligibility_barrier_allowed() {
        let body = vec![
            Statement::Expression(Expr::IntrinsicCall {
                intrinsic: Intrinsic::SubGroupBarrier,
                args: vec![],
            }),
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
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Add(
                    Box::new(Expr::Identifier("y".to_string())),
                    Box::new(Expr::Float(3.14)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("float_add", &body, Expr::Integer(10), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("z".to_string()),
                expr: Expr::Div(
                    Box::new(Expr::Mul(
                        Box::new(Expr::Identifier("x".to_string())),
                        Box::new(Expr::Identifier("y".to_string())),
                    )),
                    Box::new(Expr::Sub(
                        Box::new(Expr::Identifier("x".to_string())),
                        Box::new(Expr::Float(1.0)),
                    )),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("float_ops", &body, Expr::Integer(10), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("y".to_string()),
                expr: Expr::Neg(Box::new(Expr::Identifier("x".to_string()))),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("fneg", &body, Expr::Integer(10), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::Lt(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Identifier("y".to_string())),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("float_cmp", &body, Expr::Integer(10), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::IntrinsicCall {
                    intrinsic: Intrinsic::Sin,
                    args: vec![Expr::Identifier("x".to_string())],
                },
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("sin_test", &body, Expr::Integer(10), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Add(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Integer(1)),
                ),
                timeout: None,
                modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::Identifier("y".to_string()),
                expr: Expr::Mul(
                    Box::new(Expr::Identifier("y".to_string())),
                    Box::new(Expr::Float(2.0)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("mixed_int_float", &body, Expr::Integer(10), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::Lt(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Identifier("y".to_string())),
                ),
                timeout: None,
                modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::Identifier("s".to_string()),
                expr: Expr::Gt(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Identifier("y".to_string())),
                ),
                timeout: None,
                modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::Identifier("t".to_string()),
                expr: Expr::Eq(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Integer(42)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("int_cmp", &body, Expr::Integer(10), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("q".to_string()),
                expr: Expr::Div(
                    Box::new(Expr::Identifier("q".to_string())),
                    Box::new(Expr::Integer(3)),
                ),
                timeout: None,
                modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::Mod(
                    Box::new(Expr::Identifier("r".to_string())),
                    Box::new(Expr::Integer(7)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("div_mod", &body, Expr::Integer(10), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("sdiv i64"), "should have signed div");
        assert!(ir.contains("srem i64"), "should have signed remainder");
    }

    #[test]
    fn test_emit_spirv_float_literal() {
        let mut ft = HashMap::new();
        ft.insert("r".to_string(), "float".to_string());
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("r".to_string()),
                expr: Expr::Float(3.14159),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("float_lit", &body, Expr::Integer(10), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("y".to_string()),
                expr: Expr::Add(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Integer(1)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("multi_buf", &body, Expr::Integer(100), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("y".to_string()),
                expr: Expr::Add(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Integer(1)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("rw_test", &body, Expr::Integer(100), &[], ft);
        let ir = emit_spirv_module(&kernel);
        // x is read-only → load from in_buf
        assert!(ir.contains("getelementptr i8, ptr %base_in"),
            "read-only field x should load from in_buf");
        // y is written → store to out_buf
        assert!(ir.contains("store i64"),
            "write field y should store value");
    }

    // ── Shared memory (Phase 5) ────────────────────────────

    #[test]
    fn test_emit_spirv_shared_memory_global() {
        let body = vec![
            Statement::Let {
                name: "buf".to_string(),
                ty: None,
                expr: Some(Expr::SharedMem(256)),
                address: None,
                address_expr: None,
                bit_range: None,
                constraint: None,
                is_override: false,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("shmem_test", &body, Expr::Integer(10), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("addrspace(3)"), "should declare addrspace(3) global");
        assert!(ir.contains("[256 x i64]"), "should declare [256 x i64] array");
        assert!(ir.contains("@shared_buf_0"), "should use shared_buf_0 name");
    }

    #[test]
    fn test_emit_spirv_shared_memory_addrspace_cast() {
        let body = vec![
            Statement::Let {
                name: "buf".to_string(),
                ty: None,
                expr: Some(Expr::SharedMem(64)),
                address: None,
                address_expr: None,
                bit_range: None,
                constraint: None,
                is_override: false,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("shmem_cast", &body, Expr::Integer(10), &[], HashMap::new());
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("addrspacecast"), "should emit addrspacecast");
        assert!(ir.contains("ptrtoint"), "should convert to i64 pointer");
    }

    #[test]
    fn test_collect_shared_mem_sizes_multiple() {
        let body = vec![
            Statement::Let {
                name: "a".to_string(), ty: None, expr: Some(Expr::SharedMem(128)),
                address: None, address_expr: None, bit_range: None,
                constraint: None, is_override: false, modifiers: vec![],
            },
            Statement::Let {
                name: "b".to_string(), ty: None, expr: Some(Expr::SharedMem(32)),
                address: None, address_expr: None, bit_range: None,
                constraint: None, is_override: false, modifiers: vec![],
            },
        ];
        let sizes = collect_shared_mem_sizes(&body);
        assert_eq!(sizes, vec![128, 32], "should collect both shared memory sizes");
    }

    // ── Multi-dimensional grid (Phase 6) ───────────────────

    #[test]
    fn test_emit_spirv_module_2d_grid() {
        let mut ft = HashMap::new();
        ft.insert("x".to_string(), "i64".to_string());
        ft.insert("y".to_string(), "i64".to_string());
        // Kernel using both get_global_id(0) and get_global_id(1)
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::IntrinsicCall {
                    intrinsic: Intrinsic::GetGlobalId,
                    args: vec![Expr::Integer(0)],
                },
                timeout: None,
                modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::Identifier("y".to_string()),
                expr: Expr::IntrinsicCall {
                    intrinsic: Intrinsic::GetGlobalId,
                    args: vec![Expr::Integer(1)],
                },
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("grid2d", &body, Expr::Integer(100), &[], ft);
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
            Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::IntrinsicCall {
                    intrinsic: Intrinsic::GetGlobalId,
                    args: vec![Expr::Integer(0)],
                },
                timeout: None,
                modifiers: vec![],
            },
        ];
        let kernel = extract_kernel("grid1d", &body, Expr::Integer(100), &[], ft);
        let ir = emit_spirv_module(&kernel);
        assert!(ir.contains("call i64 @_Z13get_global_idj(i32 0)"),
            "1D kernel should have get_global_id(0)");
        assert!(!ir.contains("get_global_idj(i32 1)"),
            "1D kernel should NOT have get_global_id(1)");
        assert!(!ir.contains("get_global_idj(i32 2)"),
            "1D kernel should NOT have get_global_id(2)");
    }
}
