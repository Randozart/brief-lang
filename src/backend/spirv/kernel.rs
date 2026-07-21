/// SPIR-V kernel emission — builds OpFunction with structured blocks.
///
/// 2026-07-15: Detects `txn [idx < N]` preconditions and emits a proper
/// rspirv::dr::Function with label + instructions per block.

use rspirv::dr::{Block, Function, Instruction, Operand};
use rspirv::spirv::{self, Word, ExecutionModel, StorageClass, FunctionControl};
use crate::ast::{Expr, Transaction, Type};
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

    let void_id = builder.types.lower(&Type::void())?;
    let int_id = builder.types.lower(&Type::int())?;
    let bool_id = builder.types.lower(&Type::bits(1))?;
    let func_id = builder.gen_id();
    let func_type_id = builder.gen_id();

    // 2026-07-15: OpTypeFunction void ()
    builder.emit_type(spirv::Op::TypeFunction, func_type_id, vec![
        Operand::IdRef(void_id),
    ]);

    // 2026-07-15: OpVariable for idx ptr (type cache)
    let ptr_type_id = builder.gen_id();
    builder.emit_type(spirv::Op::TypePointer, ptr_type_id, vec![
        Operand::StorageClass(StorageClass::Function),
        Operand::IdRef(int_id),
    ]);
    let idx_var_id = builder.gen_id();

    // 2026-07-15: OpConstant for bound (result type before value)
    let bound_const = builder.gen_id();
    builder.emit_type(spirv::Op::Constant, bound_const, vec![
        Operand::IdRef(int_id),
        Operand::LiteralBit64(bound as u64),
    ]);

    // 2026-07-15: Entry block — label + OpVariable + Branch
    let entry_id = builder.gen_id();
    let entry_label = Instruction::new(spirv::Op::Label, None, Some(entry_id), vec![]);
    let var_inst = Instruction::new(spirv::Op::Variable, Some(ptr_type_id), Some(idx_var_id), vec![
        Operand::StorageClass(StorageClass::Function),
    ]);
    let loop_id = builder.gen_id();
    let br_inst = Instruction::new(spirv::Op::Branch, None, None, vec![
        Operand::IdRef(loop_id),
    ]);
    let entry_block = Block {
        label: Some(entry_label),
        instructions: vec![var_inst, br_inst],
    };

    // 2026-07-15: Loop block — label + Load + SLessThan + LoopMerge + BranchConditional
    let loop_label = Instruction::new(spirv::Op::Label, None, Some(loop_id), vec![]);
    let idx_val = builder.gen_id();
    let load_inst = Instruction::new(spirv::Op::Load, Some(int_id), Some(idx_val), vec![
        Operand::IdRef(idx_var_id),
    ]);
    let cmp = builder.gen_id();
    let cmp_inst = Instruction::new(spirv::Op::SLessThan, Some(bool_id), Some(cmp), vec![
        Operand::IdRef(idx_val),
        Operand::IdRef(bound_const),
    ]);
    let merge_id = builder.gen_id();
    let body_id = builder.gen_id();
    let merge_inst = Instruction::new(spirv::Op::LoopMerge, None, None, vec![
        Operand::IdRef(merge_id),
        Operand::IdRef(loop_id),
        Operand::LiteralBit32(0),
    ]);
    let branch_inst = Instruction::new(spirv::Op::BranchConditional, None, None, vec![
        Operand::IdRef(cmp),
        Operand::IdRef(body_id),
        Operand::IdRef(merge_id),
    ]);
    let loop_block = Block {
        label: Some(loop_label),
        instructions: vec![load_inst, cmp_inst, merge_inst, branch_inst],
    };

    // 2026-07-15: Body block — placeholder, just return
    let body_label = Instruction::new(spirv::Op::Label, None, Some(body_id), vec![]);
    let body_return = Instruction::new(spirv::Op::Return, None, None, vec![]);
    let body_block = Block {
        label: Some(body_label),
        instructions: vec![body_return],
    };

    // 2026-07-15: Merge block — return
    let merge_label = Instruction::new(spirv::Op::Label, None, Some(merge_id), vec![]);
    let merge_return = Instruction::new(spirv::Op::Return, None, None, vec![]);
    let merge_block = Block {
        label: Some(merge_label),
        instructions: vec![merge_return],
    };

    // 2026-07-15: Assemble the function
    let func_def = Instruction::new(spirv::Op::Function, Some(void_id), Some(func_id), vec![
        Operand::FunctionControl(FunctionControl::empty()),
        Operand::IdRef(func_type_id),
    ]);
    let func_end = Instruction::new(spirv::Op::FunctionEnd, None, None, vec![]);
    let function = Function {
        def: Some(func_def),
        end: Some(func_end),
        parameters: vec![],
        blocks: vec![entry_block, loop_block, body_block, merge_block],
    };
    builder.module_mut().functions.push(function);

    // 2026-07-15: Set entry point + execution mode
    builder.set_entry_point(func_id, &txn.name, ExecutionModel::GLCompute);
    builder.add_execution_mode(func_id, spirv::ExecutionMode::LocalSize, 64, 1, 1);

    Ok(func_id)
}

/// 2026-07-15: Extract range bound from [idx < N]
fn get_range_bound(pre: &Expr) -> Option<i64> {
    match pre {
        Expr::BinaryOp(op, lhs, rhs) => {
            if !matches!(op, crate::ast::BinaryOpKind::Lt) { return None; }
            if !matches!(lhs.as_ref(), Expr::Identifier(_)) { return None; }
            if let Expr::Decimal(n) = rhs.as_ref() { Some(*n) } else { None }
        }
        _ => None,
    }
}
