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
use crate::ast::Statement;

/// Local workgroup size — matches the WorkgroupSize# intrinsic constants.
const LOCAL_SIZE_X: u32 = 256;

/// Emit one GPU kernel from an analyzed shape. Returns the function id.
pub fn emit_kernel(
    builder: &mut SpirvBuilder,
    kernel_name: &str,
    shape: &KernelShape,
    items: &[crate::ast::TopLevel],
    cooperative: bool,
) -> Result<Word, String> {
    let void_id = builder.lower_type(&Type::void())?;
    let func_type_id = builder.gen_id();
    builder.builder.type_function_id(Some(func_type_id), void_id, []);

    // ── Module globals: state SSBO + invocation-id builtins. Ids thread
    // into the body lowerer so nothing is created twice.
    let state_fields = collect_state_fields(items);
    let (ssbo_var, global_id_var, local_id_var, vec4_fields) = {
        let mut warm = FnLowerer::new(builder, state_fields.clone());
        warm.materialize_consts(items)?;
        warm.warm_builtins()?;
        warm.setup_state_buffer()?;
        (
            warm.ssbo_var,
            warm.global_id_var,
            warm.local_id_var,
            warm.vec4_fields,
        )
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
    lower.vec4_fields = vec4_fields;
    lower.materialize_consts(items)?;
    lower
        .vars
        .insert(shape.index_var.clone(), (index_var, Type::int()));
    for (name, var, ty) in &local_vars {
        lower.vars.insert(name.clone(), (*var, ty.clone()));
    }

    if cooperative {
        // Row = gid.y; the lane is GetGlobalId#(0) inside the body.
        bind_work_item_row(&mut lower, index_var)?;
    } else {
        bind_work_item_index(&mut lower, index_var, shape.work_cols);
    }

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

    if cooperative {
        let red = shape
            .reduction
            .as_ref()
            .ok_or("cooperative kernel without a recognized reduction")?;
        let inner_len = match &red.inner {
            Expr::Identifier(name) => *lower
                .const_int_values
                .get(name)
                .ok_or_else(|| format!("reduction length '{}' is not a literal const", name))?,
            Expr::Decimal(n) => *n,
            other => {
                return Err(format!(
                    "reduction length {:?} must be a literal const for the cooperative path",
                    other
                ))
            }
        };
        if inner_len <= 0 || inner_len % 32 != 0 {
            return Err(format!(
                "cooperative reduction needs a length divisible by 32 (got {})",
                inner_len
            ));
        }
        emit_cooperative_reduce(&mut lower, shape, inner_len as u64)?;
    } else {
        for stmt in &shape.kernel_stmts {
            if lower.terminated {
                break;
            }
            lower.emit_stmt(stmt)?;
        }
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
    // Cooperative row kernels (plan 2026-09-01-cooperative-row-kernels):
    // ONE 32-lane workgroup per row — the subgroup IS the row's team.
    let local_x = if cooperative { 32 } else { LOCAL_SIZE_X };
    builder.add_execution_mode(
        func_id,
        spirv::ExecutionMode::LocalSize,
        local_x,
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

/// Cooperative row kernels (plan 2026-09-01-cooperative-row-kernels): bind
/// the work-item index to `GetGlobalId#(1)` — the ROW. The lane is
/// `GetGlobalId#(0)`, referenced inside the synthesized body.
fn bind_work_item_row(lower: &mut FnLowerer, index_var: spirv::Word) -> Result<(), String> {
    // The grid is FLATTENED into X (the driver dispatches rows workgroups of
    // 32 lanes along X only — the Y dimension proved inert on this driver),
    // so the row is gid.x >> 5 and the lane is gid.x & 31.
    let (gid64, _t) = lower.emit_expr(&Expr::BinaryOp(
        crate::ast::BinaryOpKind::Shr,
        Box::new(Expr::Call("GetGlobalId#".into(), vec![Expr::Decimal(0)], None)),
        Box::new(Expr::Decimal(5)),
    ))?;
    lower.builder.store(index_var, gid64);
    Ok(())
}

/// Synthesize the cooperative body for a recognized dot-product reduction:
/// the foreach iterates `t in 0..K/32` with the original loop var mapped to
/// `lane + t*32` (coalesced stride), the accumulator ends in a subgroup
/// FAdd, and the counter increment is dropped (the runner fast-forwards).
fn emit_cooperative_reduce(
    lower: &mut FnLowerer,
    shape: &KernelShape,
    inner_len: u64,
) -> Result<(), String> {
    // Locate the recognized foreach and its item name.
    let mut foreach: Option<(String, Vec<Statement>)> = None;
    for stmt in &shape.kernel_stmts {
        if let Statement::Foreach { item, body, .. } = stmt {
            foreach = Some((item.clone(), body.clone()));
            break;
        }
    }
    let Some((item, fbody)) = foreach else {
        return Err("cooperative kernel lost its foreach".into());
    };
    // The strided loop REUSES the original loop-var name (it is the one
    // pre-declared local collect_locals saw); the replacement inserts the
    // same name as the group index — subst inserts the replacement without
    // re-processing it, so this is safe.
    let lane: Expr = Expr::BinaryOp(
        crate::ast::BinaryOpKind::BitAnd,
        Box::new(Expr::Call("GetGlobalId#".into(), vec![Expr::Decimal(0)], None)),
        Box::new(Expr::Decimal(31)),
    );
    let repl = Expr::BinaryOp(
        crate::ast::BinaryOpKind::Add,
        Box::new(lane),
        Box::new(Expr::BinaryOp(
            crate::ast::BinaryOpKind::Mul,
            Box::new(Expr::Identifier(item.clone())),
            Box::new(Expr::Decimal(32)),
        )),
    );
    let new_body: Vec<Statement> = fbody
        .iter()
        .map(|st| crate::backend::spirv::lower::subst_stmt_var(st, &item, &repl))
        .collect();

    let groups = (inner_len / 32) as i64;
    let synthesized = Statement::Foreach {
        item: item.clone(),
        list: Box::new(Expr::Range {
            start: Box::new(Expr::Decimal(0)),
            end: Box::new(Expr::Decimal(groups)),
            inclusive: false,
        }),
        body: new_body,
    };
    for stmt in &shape.kernel_stmts {
        match stmt {
            Statement::Foreach { .. } => {
                lower.emit_stmt(&synthesized)?;
            }
            // The final store: `y[i] = acc` → reduce across the subgroup.
            Statement::Assign(lhs, Expr::Identifier(name))
                if shape.reduction.as_ref().is_some() && lower.vars.contains_key(name) =>
            {
                // Only the accumulator's store is wrapped — identified by the
                // var being a LOCAL (not the index var / a state field).
                if *name == shape.index_var {
                    lower.emit_stmt(stmt)?;
                } else {
                    let reduced = Expr::Call(
                        "SubgroupFAdd#".into(),
                        vec![Expr::Identifier(name.clone())],
                        None,
                    );
                    lower.emit_stmt(&Statement::Assign(lhs.clone(), reduced))?;
                }
            }
            // Drop the counter increment: the runner fast-forwards the
            // counter; a cooperative row kernel does not advance it.
            Statement::Assign(Expr::Identifier(n), _)
                if *n == shape.index_var =>
            {
                continue;
            }
            other => lower.emit_stmt(other)?,
        }
    }
    Ok(())
}
