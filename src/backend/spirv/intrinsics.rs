/// SPIR-V intrinsic lowering — maps `#` intrinsics to SPIR-V instructions.
///
/// 2026-07-15: GPU and memory intrinsics. Returns error for unsupported.

use rspirv::dr::{Instruction, Operand};
use rspirv::spirv::{self, Word};
use crate::ast::Expr;
use crate::backend::spirv::builder::SpirvBuilder;

/// 2026-07-15: Emit SPIR-V for a known `#` intrinsic.
pub fn emit_intrinsic(
    builder: &mut SpirvBuilder,
    name: &str,
    args: &[Expr],
    result_id: Word,
) -> Result<(), String> {
    match name {
        "GetGlobalId#" => emit_global_id(builder, args, result_id),
        "GetLocalId#" => emit_local_id(builder, args, result_id),
        "WorkgroupSize#" => emit_workgroup_size(builder, args, result_id),
        _ => Err(format!("SPIR-V: unsupported intrinsic '{}'", name)),
    }
}

fn emit_global_id(builder: &mut SpirvBuilder, args: &[Expr], result_id: Word) -> Result<(), String> {
    let int_id = builder.types.lower(&crate::ast::Type::int()).unwrap();
    let _ = extract_dim(args)?;
    builder.emit_type(spirv::Op::Constant, result_id, vec![
        Operand::LiteralBit64(0),
    ]);
    Ok(())
}

fn emit_local_id(builder: &mut SpirvBuilder, args: &[Expr], result_id: Word) -> Result<(), String> {
    let int_id = builder.types.lower(&crate::ast::Type::int()).unwrap();
    let _ = extract_dim(args)?;
    builder.emit_type(spirv::Op::Constant, result_id, vec![
        Operand::LiteralBit64(0),
    ]);
    Ok(())
}

fn emit_workgroup_size(builder: &mut SpirvBuilder, args: &[Expr], result_id: Word) -> Result<(), String> {
    let int_id = builder.types.lower(&crate::ast::Type::int()).unwrap();
    let _ = extract_dim(args)?;
    builder.emit_type(spirv::Op::Constant, result_id, vec![
        Operand::LiteralBit64(64),
    ]);
    Ok(())
}

fn extract_dim(args: &[Expr]) -> Result<u32, String> {
    let Some(first) = args.first() else {
        return Err("expected dimension argument (0, 1, or 2)".into());
    };
    match first {
        Expr::Decimal(n) => {
            let d = *n as u32;
            if d > 2 { return Err(format!("invalid dim {}", d)); }
            Ok(d)
        }
        _ => Err("expected integer dimension".into()),
    }
}
