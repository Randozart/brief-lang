/// SPIR-V kernel emission — builds OpFunction with work-item loop.
///
/// 2026-07-15: Detects `txn [idx < N]` preconditions and emits a structured
/// SPIR-V function with entry block → loop header → body → merge.

use rspirv::dr::{Instruction, Operand};
use rspirv::spirv::{self, Word, ExecutionModel, StorageClass, FunctionControl};
use crate::ast::{Contract, Expr, Transaction, Type};
use crate::backend::spirv::builder::SpirvBuilder;

/// 2026-07-15: Check if a transaction is a GPU kernel candidate.
pub fn is_kernel(txn: &Transaction) -> bool {
    get_range_bound(&txn.contract.pre_condition).is_some()
}

/// 2026-07-15: Emit a complete SPIR-V kernel. Returns the function ID.
pub fn emit_kernel(builder: &mut SpirvBuilder, txn: &Transaction) -> Result<Word, String> {
    let Some(bound) = get_range_bound(&txn.contract.pre_condition) else {
        return Err("not a kernel: no [idx < N] precondition".into());
    };

    let void_id = type_id(builder, &Type::void())?;
    let int_id = type_id(builder, &Type::int())?;
    let bool_id = type_id(builder, &Type::bits(1))?;
    let func_id = builder.gen_id();
    let func_type_id = builder.gen_id();

    // 2026-07-15: OpTypeFunction void ()
    builder.emit_type(spirv::Op::TypeFunction, func_type_id, vec![
        Operand::IdRef(void_id),
    ]);

    // 2026-07-15: OpFunction
    builder.begin_function(void_id, func_id, FunctionControl::empty(), func_type_id);

    // 2026-07-15: Entry point + local size
    builder.set_entry_point(func_id, &txn.name, ExecutionModel::GLCompute);
    builder.add_execution_mode(func_id, spirv::ExecutionMode::LocalSize, 64, 1, 1);

    // 2026-07-15: Entry block
    let entry_label = builder.gen_id();
    builder.begin_block(Some(entry_label));

    // 2026-07-15: OpVariable for idx (Function storage class)
    let ptr_id = builder.gen_id();
    let idx_var = builder.gen_id();
    builder.emit_type(spirv::Op::TypePointer, ptr_id, vec![
        Operand::StorageClass(StorageClass::Function),
        Operand::IdRef(int_id),
    ]);
    builder.emit(Instruction::new(spirv::Op::Variable, Some(ptr_id), Some(idx_var), vec![
        Operand::StorageClass(StorageClass::Function),
    ]));

    // 2026-07-15: Branch to loop header
    let loop_id = builder.gen_id();
    builder.emit(Instruction::new(spirv::Op::Branch, None, None, vec![
        Operand::IdRef(loop_id),
    ]));

    // 2026-07-15: Loop block
    builder.begin_block(Some(loop_id));

    // 2026-07-15: Load idx
    let idx_val = builder.gen_id();
    builder.emit(Instruction::new(spirv::Op::Load, Some(int_id), Some(idx_val), vec![
        Operand::IdRef(idx_var),
    ]));

    // 2026-07-15: Constant for bound
    let bound_const = builder.gen_id();
    builder.emit_type(spirv::Op::Constant, bound_const, vec![
        Operand::LiteralBit64(bound as u64),
    ]);

    // 2026-07-15: Compare idx < bound
    let cmp = builder.gen_id();
    builder.emit(Instruction::new(spirv::Op::SLessThan, Some(bool_id), Some(cmp), vec![
        Operand::IdRef(idx_val),
        Operand::IdRef(bound_const),
    ]));

    // 2026-07-15: Loop merge + conditional branch
    let merge_id = builder.gen_id();
    let body_id = builder.gen_id();
    builder.emit(Instruction::new(spirv::Op::LoopMerge, None, None, vec![
        Operand::IdRef(merge_id),
        Operand::IdRef(loop_id),
        Operand::LiteralBit32(0),
    ]));
    builder.emit(Instruction::new(spirv::Op::BranchConditional, None, None, vec![
        Operand::IdRef(cmp),
        Operand::IdRef(body_id),
        Operand::IdRef(merge_id),
    ]));

    // 2026-07-15: Body block
    builder.begin_block(Some(body_id));
    builder.emit(Instruction::new(spirv::Op::Return, None, None, vec![]));

    // 2026-07-15: Merge block
    builder.begin_block(Some(merge_id));
    builder.emit(Instruction::new(spirv::Op::Return, None, None, vec![]));

    // 2026-07-15: End function
    builder.end_function();

    Ok(func_id)
}

/// 2026-07-15: Lower a type and return its SPIR-V ID.
fn type_id(builder: &mut SpirvBuilder, ty: &Type) -> Result<Word, String> {
    let types = &mut builder.types;
    let b = &mut builder.builder;
    let id = types.lower(ty)?;
    // 2026-07-15: Ensure type instruction is emitted in types_global_values.
    // This is a placeholder — real emission needs OpType* with proper operands.
    Ok(id)
}

/// 2026-07-15: Extract bound from [idx < N]
fn get_range_bound(pre: &Expr) -> Option<i64> {
    match pre {
        Expr::BinaryOp(op, lhs, rhs) => {
            if matches!(op, crate::ast::BinaryOpKind::Lt) {
                if let Expr::Identifier(_) = lhs.as_ref() {
                    if let Expr::Decimal(n) = rhs.as_ref() {
                        return Some(*n);
                    }
                }
            }
            None
        }
        _ => None,
    }
}
