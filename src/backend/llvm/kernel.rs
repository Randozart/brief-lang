//! Accel kernel emission — SPIR-V kernel modules built by REUSING the LLVM
//! expression/statement emitter (emit_stmt/emit_expr + the casting graph)
//! against a kernel-scoped `%State` struct.
//!
//! The kernel state is the MINIMAL projection of the host state restricted to
//! the frontend-proven buffer/scalar contract (`AnalysisResults.accel`
//! `KernelShape`): array buffers (read/write) plus read-only scalars, in a
//! deterministic order. The work-item index is bound to the SPIR-V
//! global-id intrinsic; array reads/writes become GEP + load/store on the
//! buffer fields via the exact same emitter paths the host codegen uses
//! (`emit_array_state_store`, the Index read arm, state-scalar loads). This
//! keeps ONE expression pipeline — no hand-rolled kernel emitter that can
//! drift from CPU codegen (see docs/plans/2026-08-06-accel-gpu-offload.md §6.1).
//!
//! Each kernel is emitted as a self-contained module (`target triple =
//! "spirv64-unknown-unknown"` with its own `%State`), compiled via
//! `llc --mtriple=spirv64-unknown-unknown`, and the resulting blob is embedded
//! in the host module. The host runtime (briv_accel_rt) marshals buffers into
//! the kernel `%State` layout, launches, and unpacks.

use crate::analysis::accel::{AccelDecision, AccelEntry, KernelShape};
use crate::ast::{Expr, Type};
use std::collections::HashMap;

/// A compiled, embeddable SPIR-V kernel blob.
pub(crate) struct AccelKernelBlob {
    pub txn_name: String,
    pub bytes: Vec<u8>,
}

impl super::LlvmBackend {
    /// Emit + compile a SPIR-V kernel for every `Gpu`/`Probe` accel body.
    /// Emission order is deterministic (sorted txn names — HashMap iteration
    /// order varies per process and would shuffle blob indices).
    pub(super) fn collect_accel_kernels(
        &mut self,
        entries: &HashMap<String, AccelEntry>,
    ) -> Vec<AccelKernelBlob> {
        let mut blobs = Vec::new();
        let mut names: Vec<&String> = entries.keys().collect();
        names.sort();
        for name in names {
            let entry = &entries[name];
            if !entry.shape.eligible
                || !matches!(entry.decision, AccelDecision::Gpu | AccelDecision::Probe)
            {
                continue;
            }
            match self.emit_kernel_module(name, &entry.shape) {
                Ok(ir) => match compile_to_spirv(&ir) {
                    Ok(bytes) => blobs.push(AccelKernelBlob { txn_name: name.clone(), bytes }),
                    Err(e) => self.warnings.push(format!(
                        "warning: accel '{}': SPIR-V compilation failed — {} (staying CPU)",
                        name, e
                    )),
                },
                Err(e) => self.warnings.push(format!(
                    "warning: accel '{}': kernel emission failed — {} (staying CPU)",
                    name, e
                )),
            }
        }
        blobs
    }

    /// Emit one self-contained SPIR-V kernel module. Reuses the host field
    /// maps (compact kernel projection) so every array/scalar access goes
    /// through the standard emitter paths against the kernel `%State`.
    /// Requires the backend's program-wide field maps to be populated
    /// (i.e. after `generate()` built the host field index). Exposed for
    /// integration testing — the emitted IR is the deterministic contract;
    /// SPIR-V blob compilation (`llc`) is machine-dependent.
    pub(crate) fn emit_kernel_module(
        &mut self,
        txn_name: &str,
        shape: &KernelShape,
    ) -> Result<String, String> {
        // Kernel state fields: array buffers (read ∪ write) then scalars,
        // each sorted for determinism.
        let mut arrays: Vec<String> = shape
            .read_buffers
            .iter()
            .chain(shape.write_buffers.iter())
            .cloned()
            .collect();
        arrays.sort();
        arrays.dedup();
        let mut scalars: Vec<String> = shape.scalar_ins.clone();
        scalars.sort();
        scalars.dedup();

        // Build the compact kernel field maps from the program-wide ones.
        let mut kernel_index: HashMap<String, usize> = HashMap::new();
        let mut kernel_types: Vec<String> = Vec::new();
        let mut kernel_briv: Vec<Type> = Vec::new();
        let mut struct_fields: Vec<String> = Vec::new();
        let mut const_globals: Vec<(String, Type, Expr)> = Vec::new();
        for name in arrays.iter().chain(scalars.iter()) {
            if let Some(&fidx) = self.ctx.field_index_map.get(name) {
                let agg = self.ctx.field_types[fidx].clone();
                let briv = self.ctx.field_briv_types[fidx].clone();
                kernel_index.insert(name.clone(), kernel_types.len());
                kernel_types.push(agg.clone());
                kernel_briv.push(briv);
                struct_fields.push(agg);
            } else if let Some((ty, val)) = self.ctx.constants.get(name).cloned() {
                // Read-only global constant referenced by the kernel — it gets
                // a module-local global (the host module's @name is absent here).
                if !matches!(val, Expr::Decimal(_) | Expr::Float(_)) {
                    return Err(format!(
                        "constant '{}' has a non-literal value — kernels need literal consts",
                        name
                    ));
                }
                const_globals.push((name.clone(), ty, val));
            } else {
                return Err(format!("kernel references unknown field '{}'", name));
            }
        }
        if struct_fields.is_empty() {
            return Err("kernel has no state buffers".to_string());
        }

        // Save/restore the program-wide field maps around kernel emission.
        let saved_index = std::mem::replace(&mut self.ctx.field_index_map, kernel_index);
        let saved_types = std::mem::replace(&mut self.ctx.field_types, kernel_types);
        let saved_briv = std::mem::replace(&mut self.ctx.field_briv_types, kernel_briv);

        // Isolated function state — kernels emit after all host emission, so
        // resetting the accumulator/label caches cannot disturb the host module.
        self.fun.reset();

        let mut out = String::new();
        out.push_str(&format!(
            "; Accel kernel: {}\n",
            txn_name
        ));
        out.push_str("target datalayout = \"e-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024\"\n");
        out.push_str("target triple = \"spirv64-unknown-unknown\"\n\n");
        out.push_str("declare i64 @_Z13get_global_idj(i32) #0\n\n");
        for (cname, cty, cval) in &const_globals {
            let cty_llvm = self.llvm_type(cty);
            let lit = match cval {
                Expr::Decimal(n) => n.to_string(),
                Expr::Float(f) => {
                    if f.is_finite() {
                        format!("{:e}", f)
                    } else {
                        "0.000000e+00".to_string()
                    }
                }
                _ => unreachable!(),
            };
            out.push_str(&format!(
                "@{} = private constant {} {}\n",
                cname, cty_llvm, lit
            ));
        }
        out.push_str(&format!("%State = type {{ {} }}\n", struct_fields.join(", ")));
        // Entry point is `main`: each kernel module is self-contained (one
        // function), and both device drivers (Vulkan pipeline entry + OpenCL
        // clCreateKernel) expect "main".
        out.push_str(&format!(
            "\ndefine spir_kernel void @main(ptr %state, i64 %n) {{\n",
        ));
        out.push_str("entry:\n");
        out.push_str("  %gtid = call i64 @_Z13get_global_idj(i32 0)\n");
        out.push_str("  %cmp = icmp ult i64 %gtid, %n\n");
        out.push_str("  br i1 %cmp, label %body, label %exit\n");
        out.push_str("body:\n");

        // Bind the work-item index (the virtual `[i < N]` variable) to the
        // global-id register so the standard emitter resolves `i` everywhere.
        self.fun
            .let_bindings
            .insert(shape.index_var.clone(), "%gtid".to_string());
        self.fun
            .let_binding_types
            .insert(shape.index_var.clone(), Type::int());
        self.fun
            .let_original_types
            .insert(shape.index_var.clone(), Type::int());

        for stmt in &shape.kernel_stmts {
            super::emit_stmt::emit_statement(self, &mut out, stmt, "  ");
        }

        self.fun.let_bindings.remove(&shape.index_var);
        self.fun.let_binding_types.remove(&shape.index_var);
        self.fun.let_original_types.remove(&shape.index_var);

        out.push_str("  br label %exit\n");
        out.push_str("exit:\n  ret void\n}\n");

        self.ctx.field_index_map = saved_index;
        self.ctx.field_types = saved_types;
        self.ctx.field_briv_types = saved_briv;
        Ok(out)
    }
}

/// Size in bytes of an LLVM aggregate-type string from the host `%State`
/// layout (`field_types`), e.g. `[16 x float]` → 64, `i64` → 8, `float` → 4.
/// Unknown shapes fall back to 8 (the %State scalar default).
fn llvm_agg_size(s: &str) -> u64 {
    if let Some(rest) = s.strip_prefix('[') {
        if let Some(x) = rest.find(" x ") {
            let n: u64 = rest[..x].trim().parse().unwrap_or(0);
            let elem = rest[x + 3..].trim_end_matches(']').trim();
            return n.saturating_mul(llvm_agg_size(elem));
        }
        return 8;
    }
    match s {
        "i8" => 1,
        "i16" => 2,
        "i32" => 4,
        "i64" => 8,
        "float" => 4,
        "double" => 8,
        "ptr" => 8,
        _ => 8,
    }
}

/// Byte offset of host `%State` field `fidx` (sum of preceding field sizes).
fn host_field_offset(field_types: &[String], fidx: usize) -> u64 {
    field_types[..fidx].iter().map(|t| llvm_agg_size(t)).sum()
}

/// One kernel's descriptor fields in kernel `%State` order (arrays sorted,
/// then scalars sorted), as IR text + the field names.
struct KernelFieldTable {
    fields: Vec<String>,
    names: Vec<String>,
}

/// Field IR entry for one kernel field.
fn field_entry_ir(
    host_off: u64,
    ty: &str,
    name: &str,
    txn: &str,
    write_set: &std::collections::HashSet<&str>,
) -> String {
    let (kind, elem_bytes, count, w) = if ty.starts_with('[') {
        let n: u64 = ty
            .strip_prefix('[')
            .and_then(|r| r.split(" x ").next())
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or(0);
        let write = if write_set.contains(name) { 1u32 } else { 0u32 };
        (1u32, llvm_agg_size(ty) / n.max(1), n, write)
    } else {
        (2u32, llvm_agg_size(ty), 1u64, 0u32)
    };
    format!(
        "%briv.field {{ ptr @str.briv.{}.{}, i32 {}, i64 {}, i64 {}, i64 {}, i32 {} }}",
        txn, name, kind, host_off, elem_bytes, count, w
    )
}

/// Build one kernel's field table (kernel field order, host offsets).
fn kernel_field_table(backend: &super::LlvmBackend, txn: &str) -> KernelFieldTable {
    let entry = &backend.accel_entries[txn];
    let shape = &entry.shape;
    let mut arrays: Vec<&String> = shape.read_buffers.iter().collect();
    let read_set: std::collections::HashSet<&String> = arrays.iter().copied().collect();
    for w in &shape.write_buffers {
        if !read_set.contains(w) {
            arrays.push(w);
        }
    }
    arrays.sort();
    let mut scalars: Vec<&String> = shape.scalar_ins.iter().collect();
    scalars.sort();
    let write_set: std::collections::HashSet<&str> =
        shape.write_buffers.iter().map(|s| s.as_str()).collect();

    let field_types = &backend.ctx.field_types;
    let mut fields = Vec::new();
    let mut names = Vec::new();
    for name in arrays.iter().chain(scalars.iter()) {
        let fidx = backend.ctx.field_index_map[*name];
        fields.push(field_entry_ir(
            host_field_offset(field_types, fidx),
            &field_types[fidx],
            name,
            txn,
            &write_set,
        ));
        names.push((*name).clone());
    }
    KernelFieldTable { fields, names }
}

/// Emit one kernel's descriptor: name/field string constants, the fields
/// table, and the descriptor entry. Returns the `%briv.kernel` entry text.
fn emit_one_kernel_desc(
    backend: &super::LlvmBackend,
    out: &mut String,
    k: &AccelKernelBlob,
) -> String {
    let txn = &k.txn_name;
    let table = kernel_field_table(backend, txn);
    for name in &table.names {
        let bytes = format!("{}\0", name);
        out.push_str(&format!(
            "@str.briv.{}.{} = private constant [{} x i8] c\"{}\\00\"\n",
            txn,
            name,
            bytes.len(),
            name
        ));
    }
    out.push_str(&format!(
        "@briv_kernel_{}_fields = private constant [{} x %briv.field] [{}]\n",
        txn,
        table.fields.len(),
        table.fields.join(", ")
    ));
    out.push_str(&format!(
        "@str.briv.{} = private constant [{} x i8] c\"{}\\00\"\n",
        txn,
        txn.len() + 1,
        txn
    ));
    // Per-txn auto-tuning verdict (Probe decisions): 0 = CPU, 1 = GPU.
    // The emitted run_probe sets it once at startup; the dispatch wrapper gates
    // on it. Per-txn so independent accel bodies can commit independently.
    out.push_str(&format!(
        "@briv_accel_verdict_{} = private global i32 0\n",
        txn
    ));
    format!(
        "%briv.kernel {{ ptr @str.briv.{}, i32 ptrtoint (ptr @briv_kernel_{} to i32), i32 {}, i32 {}, ptr @briv_kernel_{}_fields }}",
        txn,
        txn,
        k.bytes.len(),
        table.fields.len(),
        txn
    )
}

/// Emit the accel descriptor tables + ABI declares the host program links
/// against (`briv_accel_rt.c`). Each kernel's fields are listed in KERNEL
/// `%State` order (arrays sorted, then scalars sorted) with their HOST
/// offsets, so the runtime's generic pack/unpack matches the kernel's GEPs.
/// Returns the IR text and the txn-name → descriptor-index map the host
/// dispatch wrappers use.
pub(crate) fn emit_accel_descriptors(
    backend: &super::LlvmBackend,
    kernels: &[AccelKernelBlob],
) -> (String, HashMap<String, u32>) {
    let mut out = String::new();
    let mut idx_of: HashMap<String, u32> = HashMap::new();
    if kernels.is_empty() {
        return (out, idx_of);
    }
    out.push_str("\n; === Accel kernel descriptors ===\n");
    out.push_str("%briv.field = type { ptr, i32, i64, i64, i64, i32 }\n");
    out.push_str("%briv.kernel = type { ptr, i32, i32, i32, ptr }\n");
    out.push_str("@briv_accel_ready = private global i32 0\n");

    let mut desc_entries: Vec<String> = Vec::new();
    for (i, k) in kernels.iter().enumerate() {
        idx_of.insert(k.txn_name.clone(), i as u32);
        desc_entries.push(emit_one_kernel_desc(backend, &mut out, k));
    }
    out.push_str(&format!(
        "@briv_accel_descs = private constant [{} x %briv.kernel] [{}]\n",
        desc_entries.len(),
        desc_entries.join(", ")
    ));
    out.push_str("declare i32 @briv_accel_init(ptr, i32)\n");
    out.push_str("declare i32 @briv_accel_launch(i32, ptr, i64)\n");
    out.push_str("declare i32 @briv_accel_available()\n");
    out.push_str("declare i32 @briv_accel_probe(ptr, ptr, ptr, i64, i64, double, double, ptr)\n");
    (out, idx_of)
}
///
/// Runs `llc --mtriple=spirv64-unknown-unknown` on the kernel IR. Shelling out
/// Compile kernel LLVM IR to SPIR-V binary via `llc`.
///
/// Runs `llc --mtriple=spirv64-unknown-unknown` on the kernel IR. Shelling out
/// is required because LLVM's SPIR-V backend (spirv64-unknown-unknown) is a
/// separate target not enabled in the default LLVM build used by inkwell.
/// Uses unique temp filenames (process + atomic counter) — a fixed path
/// corrupted parallel test runs (TOCTOU, 2026-06-29).
pub(crate) fn compile_to_spirv(ir: &str) -> Result<Vec<u8>, String> {    use std::io::Write;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    let tmp_dir = std::env::temp_dir();
    static KERNEL_COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = KERNEL_COUNTER.fetch_add(1, Ordering::Relaxed);
    let unique = format!("briv_kernel_{}_{}", std::process::id(), seq);
    let ir_path = tmp_dir.join(format!("{}.ll", unique));
    let spv_path = tmp_dir.join(format!("{}.spv", unique));

    let mut file = std::fs::File::create(&ir_path)
        .map_err(|e| format!("failed to create temp IR file: {}", e))?;
    file.write_all(ir.as_bytes())
        .map_err(|e| format!("failed to write temp IR: {}", e))?;

    let output = Command::new("llc")
        .arg("--mtriple=spirv64-unknown-unknown")
        .arg(&ir_path)
        .arg("-o")
        .arg(&spv_path)
        .output()
        .map_err(|e| format!("failed to run llc: {}. Is llc installed?", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&ir_path);
        return Err(format!("llc failed: {}", stderr));
    }

    let binary = std::fs::read(&spv_path)
        .map_err(|e| format!("failed to read SPIR-V output: {}", e))?;
    let _ = std::fs::remove_file(&ir_path);
    let _ = std::fs::remove_file(&spv_path);
    Ok(binary)
}

/// Emit an embedded SPIR-V blob as an LLVM global constant in the host module.
pub(crate) fn embed_spirv_blob(spirv_binary: &[u8], kernel_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("\n; Embedded SPIR-V kernel: {}\n", kernel_name));
    out.push_str(&format!(
        "@briv_kernel_{} = private constant [{} x i8] c\"",
        kernel_name,
        spirv_binary.len()
    ));
    for (i, byte) in spirv_binary.iter().enumerate() {
        if i > 0 && i % 32 == 0 {
            out.push_str("\"\"\n  \"");
        }
        out.push_str(&format!("\\{:02X}", byte));
    }
    out.push_str("\", align 4\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_blob_wraps_bytes() {
        let blob = vec![0x03u8, 0x02, 0x23, 0x07];
        let out = embed_spirv_blob(&blob, "force");
        assert!(out.contains("@briv_kernel_force = private constant [4 x i8] c\""));
        assert!(out.contains("\\03\\02\\23\\07"));
    }

    #[test]
    fn compile_to_spirv_rejects_garbage_ir() {
        let err = compile_to_spirv("this is not llvm ir").unwrap_err();
        assert!(!err.is_empty());
    }
}
