// ── SPIR-V Normalizer — AST Annotation Pass ──────────────────────────
// 2026-07-15: Resolves types via TypeUniverse, attaches `alu` metadata,
// flags kernels, validates against SPIR-V supported ops.
//
// Runs after Mid plugins, before codegen. Follows LLVM normalizer pattern.

use crate::ast::*;
use crate::backend::normalizer;
use crate::config::{derive_alu_type, OpConfig, TypeConfig};
use crate::type_universe::TypeUniverse;
use std::collections::HashSet;

/// 2026-07-15: Normalize AST for SPIR-V backend emission.
/// Resolves Custom types to Bits(N), attaches `alu` metadata,
/// flags kernel transactions, validates operators against spirv-ops.toml.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
    let prim_config = TypeConfig::load();

    // 2026-07-15: Attach `alu` property to every type based on primitive
    for rt in universe.types.values_mut() {
        let prim = rt.primitive();
        let bytes = rt.bytes;
        let alu = derive_alu_type(prim, bytes, &prim_config);
        rt.properties.insert("alu".into(), PropertyValue::String(alu));
    }

    // 2026-07-15: Flag kernel transactions ([idx < N] pattern)
    for item in items.iter_mut() {
        if let TopLevel::Transaction(txn) = item {
            if is_kernel_txn(txn) {
                txn.metadata.insert("is_kernel".into(), PropertyValue::Bool(true));
            }
        }
    }

    // 2026-07-15: Validate operators against SPIR-V supported set
    let op_config = OpConfig::load_from("spirv-ops.toml");
    let supported = build_supported_ops(&op_config);
    let errors = normalizer::validate_intrinsics(items, &supported);
    if !errors.is_empty() {
        return Err(format!("SPIR-V normalizer:\n  {}", errors.join("\n  ")));
    }

    // 2026-07-15: Strip irrelevant metadata
    let keep: HashSet<String> = ["alu", "bytes", "encoding", "is_kernel"]
        .iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        rt.properties.retain(|k, _| keep.contains(k));
    }

    Ok(())
}

/// 2026-07-15: Detect kernel transaction by [idx < N] precondition.
fn is_kernel_txn(txn: &Transaction) -> bool {
    match &txn.contract.pre_condition {
        Expr::BinaryOp(kind, lhs, rhs) => {
            if !matches!(kind, BinaryOpKind::Lt) { return false; }
            if !matches!(lhs.as_ref(), Expr::Identifier(_)) { return false; }
            matches!(rhs.as_ref(), Expr::Decimal(_))
        }
        _ => false,
    }
}

/// 2026-07-15: Build supported intrinsic set for SPIR-V backend.
fn build_supported_ops(_config: &OpConfig) -> HashSet<String> {
    let mut ops = HashSet::new();
    // 2026-07-15: Arithmetic intrinsics from spirv-ops.toml
    ops.insert("Add#".into());
    ops.insert("Sub#".into());
    ops.insert("Mul#".into());
    ops.insert("Div#".into());
    ops.insert("Rem#".into());
    ops.insert("Neg#".into());
    ops.insert("Abs#".into());
    // 2026-07-15: Comparison
    ops.insert("Eq#".into());
    ops.insert("Neq#".into());
    ops.insert("Lt#".into());
    ops.insert("Gt#".into());
    ops.insert("Le#".into());
    ops.insert("Ge#".into());
    // 2026-07-15: Bitwise
    ops.insert("BitAnd#".into());
    ops.insert("BitOr#".into());
    ops.insert("BitXor#".into());
    ops.insert("Shl#".into());
    ops.insert("Shr#".into());
    // 2026-07-15: GPU-specific
    ops.insert("GetGlobalId#".into());
    ops.insert("GetGlobalSize#".into());
    ops.insert("GetLocalId#".into());
    ops.insert("WorkgroupSize#".into());
    // 2026-07-15: Memory
    ops.insert("Load#".into());
    ops.insert("Store#".into());
    ops
}
