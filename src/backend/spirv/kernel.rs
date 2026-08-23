/// SPIR-V kernel emission — full statement/expression lowering.
///
/// 2026-07-15: Detects `txn [idx < N]` preconditions, emits a structured
/// GLCompute kernel with an induction loop.
/// 2026-08-23 (plan 2026-08-23-spirv-kernel-emission §2.1): the body block
/// lowers REAL statements via lower::FnLowerer — integer scalar compute
/// over locals, invocation-id builtins, and one StorageBuffer binding for
/// indexed state. The placeholder Op.Return body is gone.
/// 2026-08-23 (follow-up): builder-driven block construction so lowering
/// can emit incrementally (hand-assembled Block vecs could not).

use rspirv::dr::Operand;
use rspirv::spirv::{self, Word, ExecutionModel, StorageClass, FunctionControl};
use crate::ast::{Expr, Transaction, Type};
use crate::backend::spirv::builder::SpirvBuilder;
use crate::backend::spirv::lower::{collect_state_fields, FnLowerer};

/// Local workgroup size — matches the WorkgroupSize# intrinsic constants.
const LOCAL_SIZE_X: u32 = 64;

/// 2026-07-15: Check if a transaction is a GPU kernel candidate.
pub fn is_kernel(txn: &Transaction) -> bool {
    get_range_bound(&txn.contract.pre_condition).is_some()
}

/// Module-section i64 constant (usable before/without a FnLowerer).
fn const_i64(builder: &mut SpirvBuilder, v: u64) -> Word {
    let int_ty = builder.types.lower(&Type::int()).expect("int type");
    let c = builder.gen_id();
    builder.emit_type(
        spirv::Op::Constant,
        c,
        vec![Operand::IdRef(int_ty), Operand::LiteralBit64(v)],
    );
    c
}

/// Emit a complete SPIR-V kernel with a lowered body.
///
/// Structure:
/// ```text
/// entry:  idx_var = 0; br loop
/// loop:   idx' = load idx; cond = idx' < N; LoopMerge; br body|merge
/// body:   <lowered statements>; unless term: idx += 1; br continue
/// cont:   br loop
/// merge:  ret   (or unreachable when the body always returned)
/// ```
pub fn emit_kernel(
    builder: &mut SpirvBuilder,
    txn: &Transaction,
    items: &[crate::ast::TopLevel],
) -> Result<Word, String> {
    let Some(bound) = get_range_bound(&txn.contract.pre_condition) else {
        return Err("not a kernel: no [idx < N] precondition".into());
    };

    let void_id = builder.types.lower(&Type::void())?;
    let int_ty = builder.types.lower(&Type::int())?;
    let bool_ty = builder.types.lower(&Type::Bits(1))?;
    let func_id = builder.gen_id();
    let func_type_id = builder.gen_id();
    builder.emit_type(spirv::Op::TypeFunction, func_type_id, vec![
        Operand::IdRef(void_id),
    ]);

    // Induction variable type + ids for constants and blocks, allocated
    // BEFORE the body lowerer borrows the builder.
    let ptr_int = builder.gen_id();
    builder.emit_type(spirv::Op::TypePointer, ptr_int, vec![
        Operand::StorageClass(StorageClass::Function),
        Operand::IdRef(int_ty),
    ]);
    let idx_var = builder.gen_id();
    let entry_id = builder.gen_id();
    let loop_id = builder.gen_id();
    let body_id = builder.gen_id();
    let continue_id = builder.gen_id();
    let merge_id = builder.gen_id();
    let zero = const_i64(builder, 0);
    let bound_const = const_i64(builder, bound as u64);
    let one = const_i64(builder, 1);

    // ── Module globals: state SSBO + invocation-id builtins. Globals live in
    // the module section, so emitting them via a short-lived lowerer here is
    // order-independent; the ssbo var id carries into the body lowerer.
    let state_fields = collect_state_fields(items, &txn.name);
    let ssbo_var = {
        let mut warm = FnLowerer::new(builder, state_fields.clone());
        if std::env::var("BRIEV_NO_BUILTIN").is_err() { warm.warm_builtins()?; }
        if std::env::var("BRIEV_NO_SSBO").is_err() { warm.setup_state_buffer()?; }
        warm.ssbo_var
    };
    // Types referenced by the function must precede it in the module.
    builder.flush_types();

    builder.begin_function(void_id, func_id, FunctionControl::empty(), func_type_id);

    // ── entry: idx_var = 0
    builder.begin_block(Some(entry_id));
    // Function-scope variables must be the first instructions of entry.
    builder.instr(
        spirv::Op::Variable,
        Some(ptr_int),
        Some(idx_var),
        vec![Operand::StorageClass(StorageClass::Function)],
    );
    builder.store(idx_var, zero);
    builder.branch(loop_id);

    // ── loop header: cond = idx < N
    builder.begin_block(Some(loop_id));
    let cur = builder.load(int_ty, idx_var);
    let cond = builder.instr(
        spirv::Op::SLessThan,
        Some(bool_ty),
        None,
        vec![Operand::IdRef(cur), Operand::IdRef(bound_const)],
    );
    builder.loop_header_tail(cond, merge_id, continue_id, body_id);

    // ── body: lowered statements (the lowerer owns the builder here).
    builder.begin_block(Some(body_id));
    let terminated = {
        let mut lower = FnLowerer::new(builder, state_fields.clone());
        lower.ssbo_var = ssbo_var;
        lower.vars.insert("idx".to_string(), (idx_var, Type::int()));
        for stmt in &txn.body {
            if lower.terminated {
                break;
            }
            lower.emit_stmt(stmt)?;
        }
        lower.terminated
    };
    if !terminated {
        let cur = builder.load(int_ty, idx_var);
        let next = builder.instr(
            spirv::Op::IAdd,
            Some(int_ty),
            None,
            vec![Operand::IdRef(cur), Operand::IdRef(one)],
        );
        builder.store(idx_var, next);
        builder.branch(continue_id);

        builder.begin_block(Some(continue_id));
        builder.branch(loop_id);
    }

    // ── merge: exit point. When every body path already returned, the block
    // is unreachable but must still EXIST (LoopMerge names it).
    builder.begin_block(Some(merge_id));
    if terminated {
        builder.instr(spirv::Op::Unreachable, None, None, vec![]);
    } else {
        builder.ret();
    }
    builder.end_function();

    builder.set_entry_point(func_id, &txn.name, ExecutionModel::GLCompute);
    builder.add_execution_mode(func_id, spirv::ExecutionMode::LocalSize, LOCAL_SIZE_X, 1, 1);

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
