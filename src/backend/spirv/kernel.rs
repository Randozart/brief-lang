/// SPIR-V kernel emission — frontend-driven work-item kernels.
///
/// 2026-08-23 (plan §2.2): kernel selection and body content come from the
/// FRONTEND's accel analysis (`AnalysisResults.accel`, built by
/// src/analysis/accel.rs) — the `[idx < N]` string-sniffing is gone. The
/// analyzed shape provides:
///   - `index_var`:    the state counter that becomes the work-item id
///   - `kernel_stmts`: statements PROVEN safe to offload (pure, affine)
///   - read/write buffers → the StorageBuffer surface
///
/// Structure: a GLCompute invocation IS one work item, so there is no
/// induction loop — `index_var` binds to get_global_id(0) and the host
/// sets dispatch dimensions from N.
use rspirv::dr::{Instruction, Operand};
use rspirv::spirv::{self, Word, ExecutionModel, StorageClass, FunctionControl};
use crate::ast::{Expr, Type};
use crate::backend::spirv::builder::SpirvBuilder;
use crate::backend::spirv::lower::{collect_locals, collect_state_fields, FnLowerer};
use crate::analysis::accel::KernelShape;

/// Local workgroup size — matches the WorkgroupSize# intrinsic constants.
const LOCAL_SIZE_X: u32 = 256;

/// Emit one GPU kernel from an analyzed shape. Returns the function id.
pub fn emit_kernel(
    builder: &mut SpirvBuilder,
    kernel_name: &str,
    shape: &KernelShape,
    items: &[crate::ast::TopLevel],
) -> Result<Word, String> {
    let void_id = builder.lower_type(&Type::void())?;
    let func_type_id = builder.gen_id();
    builder.builder.type_function_id(Some(func_type_id), void_id, []);

    // ── Module globals: state SSBO + invocation-id builtins. Ids thread
    // into the body lowerer so nothing is created twice.
    let state_fields = collect_state_fields(items);
    let (ssbo_var, global_id_var, local_id_var) = {
        let mut warm = FnLowerer::new(builder, state_fields.clone());
        warm.materialize_consts(items)?;
        warm.warm_builtins()?;
        warm.setup_state_buffer()?;
        (warm.ssbo_var, warm.global_id_var, warm.local_id_var)
    };
    // Types referenced by the function must precede it in the module.

    // All direct-builder work happens BEFORE the body lowerer borrows it:
    // ids, function, entry block, and every function-scope OpVariable (they
    // must be the first instructions of the entry block).
    let func_id = builder.gen_id();
    let entry_id = builder.gen_id();
    let int_ptr = {
        let int_ty = builder.lower_type(&Type::int())?;
        builder.ptr_class(StorageClass::Function, int_ty)
    };
    let index_var = builder.gen_id();

    let mut collected: Vec<(String, Type)> = Vec::new();
    collect_locals(&shape.kernel_stmts, &mut collected);
    let local_vars: Vec<(String, Word, Type)> = collected
        .into_iter()
        .map(|(name, ty)| {
            let elem = builder.lower_type(&ty)?;
            let ptr = builder.ptr_class(StorageClass::Function, elem);
            let var = builder.gen_id();
            builder.instr(
                spirv::Op::Variable,
                Some(ptr),
                Some(var),
                vec![Operand::StorageClass(StorageClass::Function)],
            );
            Ok((name, var, ty))
        })
        .collect::<Result<Vec<_>, String>>()?;

    builder.begin_function(void_id, func_id, FunctionControl::empty(), func_type_id);
    builder.begin_block(Some(entry_id));
    builder.instr(
        spirv::Op::Variable,
        Some(int_ptr),
        Some(index_var),
        vec![Operand::StorageClass(StorageClass::Function)],
    );
    for (_, var, ty) in &local_vars {
        let elem = builder.lower_type(ty)?;
        let ptr = builder.ptr_class(StorageClass::Function, elem);
        builder.instr(
            spirv::Op::Variable,
            Some(ptr),
            Some(*var),
            vec![Operand::StorageClass(StorageClass::Function)],
        );
    }

    let mut lower = FnLowerer::new(builder, state_fields);
    lower.ssbo_var = ssbo_var;
    lower.global_id_var = global_id_var;
    lower.local_id_var = local_id_var;
    lower.materialize_consts(items)?;
    lower
        .vars
        .insert(shape.index_var.clone(), (index_var, Type::int()));
    for (name, var, ty) in &local_vars {
        lower.vars.insert(name.clone(), (*var, ty.clone()));
    }

    bind_work_item_index(&mut lower, index_var, shape.work_cols);

    // 2026-08-31 (plan abv-gpu-by-default): BOUNDS GUARD. The host dispatches
    // ceil(N / LocalSize) workgroups, so up to LocalSize-1 extra invocations
    // run; each must exit before touching state when its global id exceeds
    // the work-item count (a runtime field or a literal — exactly the bound
    // the eligibility proof extracted from `[i < N]`).
    //
    // 2D (plan 2026-08-31-gpu-next §2b): the flat-launch tail argument only
    // holds when N is a multiple of the workgroup size. With 2D geometry the
    // tail can reach cols-1 items, and even a literal count need not be a
    // multiple — so a 2D shape ALWAYS carries the guard. (Found while
    // wiring this up: a literal count not divisible by 64 had the same hole
    // in pure 1D; the literal%64 check closes that too.)
    let count_is_literal = matches!(shape.count_expr, Some(Expr::Decimal(_)));
    let count_multiple_of_workgroup = match shape.count_expr {
        Some(Expr::Decimal(n)) => n % LOCAL_SIZE_X as i64 == 0,
        _ => false,
    };
    let needs_guard = !count_is_literal
        || shape.work_cols.is_some()
        || !count_multiple_of_workgroup;
    let exit_bb = lower.builder.gen_id();
    if needs_guard {
        let body_bb = lower.builder.gen_id();
        let bound_expr = shape
            .count_expr
            .clone()
            .unwrap_or(Expr::Decimal(0));
        let (bound, _bty) = lower.emit_expr(&bound_expr)?;
        let int_ty = lower.builder.lower_type(&Type::int())?;
        let gid_reg = lower.builder.gen_id();
        lower.builder.emit(Instruction::new(
            spirv::Op::Load,
            Some(int_ty),
            Some(gid_reg),
            vec![Operand::IdRef(index_var)],
        ));
        let bool_ty = lower.builder.lower_type(&Type::Bits(1))?;
        let in_bounds = lower.builder.gen_id();
        lower.builder.emit(Instruction::new(
            spirv::Op::ULessThan,
            Some(bool_ty),
            Some(in_bounds),
            vec![Operand::IdRef(gid_reg), Operand::IdRef(bound)],
        ));
        // Vulkan requires structured selection: OpSelectionMerge before the
        // conditional branch.
        lower
            .builder
            .builder
            .selection_merge(exit_bb, rspirv::spirv::SelectionControl::NONE);
        lower
            .builder
            .builder
            .branch_conditional(in_bounds, body_bb, exit_bb, [] as [u32; 0]);
        lower.builder.begin_block(Some(body_bb));
    }

    for stmt in &shape.kernel_stmts {
        if lower.terminated {
            break;
        }
        lower.emit_stmt(stmt)?;
    }

    lower.builder.builder.branch(exit_bb);
    lower.builder.begin_block(Some(exit_bb));
    builder.ret();
    builder.end_function();

    // Entry-point interface lists every Input/Output + SSBO variable.
    let interface: Vec<Word> = [global_id_var, local_id_var]
        .into_iter()
        .flatten()
        .chain(ssbo_var.into_iter())
        .collect();
    builder.set_entry_point(func_id, kernel_name, ExecutionModel::GLCompute, &interface);
    builder.add_execution_mode(
        func_id,
        spirv::ExecutionMode::LocalSize,
        LOCAL_SIZE_X,
        1,
        1,
    );

    Ok(func_id)
}

/// BIND the work-item index (2026-08-31, plan abv-gpu-by-default): the doc
/// once claimed "index_var binds to get_global_id(0)" but nothing stored it —
/// the standalone kernels were never executed, so every invocation read an
/// undefined index. Widening u32→i64 mirrors the builtin path.
///
/// 2D (plan 2026-08-31-gpu-next §2b): when the shape carries a dispatch
/// width, reconstruct `i = gid.y * cols + gid.x`. The values are IDENTICAL
/// to a 1D linearization for every covered item, so any launcher that
/// covers the total count stays correct (a flat 1D launch has gid.y == 0
/// and gid.x spanning the count); the 2D shape exists so the launcher can
/// hand the row/col split to the hardware.
fn bind_work_item_index(
    lower: &mut FnLowerer,
    index_var: spirv::Word,
    work_cols: Option<u64>,
) -> Result<(), String> {
    let idx_expr = match work_cols {
        Some(cols) if cols > 1 => Expr::BinaryOp(
            crate::ast::BinaryOpKind::Add,
            Box::new(Expr::BinaryOp(
                crate::ast::BinaryOpKind::Mul,
                Box::new(Expr::Call(
                    "GetGlobalId#".into(),
                    vec![Expr::Decimal(1)],
                    None,
                )),
                Box::new(Expr::Decimal(cols as i64)),
            )),
            Box::new(Expr::Call(
                "GetGlobalId#".into(),
                vec![Expr::Decimal(0)],
                None,
            )),
        ),
        _ => Expr::Call("GetGlobalId#".into(), vec![Expr::Decimal(0)], None),
    };
    let (gid64, _t) = lower.emit_expr(&idx_expr)?;
    lower.builder.store(index_var, gid64);
    Ok(())
}
