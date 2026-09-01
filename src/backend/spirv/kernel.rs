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

/// Shared context for the cooperative vec4 loop emission — bundles the
/// per-function state so helpers stay under the parameter-count limit.
struct Vec4LoopCtx<'a> {
    item: &'a str,
    repl: &'a Expr,
    fbody: &'a [Statement],
    field_data: &'a [(String, crate::backend::spirv::lower::Vec4Field, usize)],
    all_indices: &'a [(String, Expr)],
    stride: u64,
    inner_len: u64,
    row_id: Word,
}

/// Collect (field, index_expr) pairs the body loads through vec4-eligible
/// fields, deduplicated by (field, index shape). Used by vec4 detection and
/// by the vec4 loop body substitution.
fn collect_dedup_vec4_indices(
    lower: &FnLowerer,
    fbody: &[Statement],
    item: &str,
) -> Vec<(String, Expr)> {
    let mut indices: Vec<(String, Expr)> = Vec::new();
    for stmt in fbody {
        if let Statement::Assign(_, rhs) = stmt {
            crate::backend::spirv::lower::collect_vec4_indices(
                rhs, &lower.vec4_fields, item, &mut indices,
            );
        }
    }
    indices.sort_by(|a, b| a.0.cmp(&b.0).then(format!("{:?}", a.1).cmp(&format!("{:?}", b.1))));
    indices.dedup_by(|a, b| a.0 == b.0 && format!("{:?}", a.1) == format!("{:?}", b.1));
    indices
}

/// Split the kernel statements at the Foreach: statements BEFORE it (e.g. the
/// `acc = 0` initialization) must be emitted before the cooperative loop,
/// not after it (emitting `acc = 0` in the loop merge wiped the accumulator
/// before the subgroup reduce — 2026-09-01 gemv FAIL root cause).
fn split_at_foreach(stmts: &[Statement]) -> (Vec<&Statement>, Vec<&Statement>) {
    let mut pre = Vec::new();
    let mut post = Vec::new();
    let mut seen_foreach = false;
    for stmt in stmts {
        if matches!(stmt, Statement::Foreach { .. }) {
            seen_foreach = true;
            continue;
        }
        if seen_foreach { post.push(stmt); } else { pre.push(stmt); }
    }
    (pre, post)
}

/// Emit the kernel statements that FOLLOW the cooperative loop: the final
/// store becomes a subgroup reduction. Shared by the vec4 and scalar paths.
/// The counter increment is dropped — the runner fast-forwards the counter;
/// a cooperative row kernel does not advance it.
fn emit_coop_reduce_store(
    lower: &mut FnLowerer,
    stmts: &[&Statement],
    shape: &KernelShape,
) -> Result<(), String> {
    for stmt in stmts {
        match stmt {
            Statement::Assign(lhs, Expr::Identifier(name))
                if shape.reduction.as_ref().is_some() && lower.vars.contains_key(name) =>
            {
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
            Statement::Assign(Expr::Identifier(n), _) if *n == shape.index_var => {}
            other => lower.emit_stmt(other)?,
        }
    }
    Ok(())
}

/// Emit ONE vec4 load per vec4-eligible field at the cooperative base index
/// `row*(K/4) + lane + t*(stride/4)`, extracting the 4 components into
/// synthetic `__vec4_<field>_<j>` variables the unrolled body reads.
fn emit_vec4_field_loads(
    lower: &mut FnLowerer,
    ctx: &Vec4LoopCtx,
    loop_var: Word,
    int_ty: Word,
    ssbo: Word,
) -> Result<(), String> {
    // The caller resolved inner_len to a literal (or hard-errored), so the
    // vec4 count per row is just inner_len / 4.
    let k_div4 = ctx.inner_len / 4;

    // row * (K/4)
    let row_val = lower.builder.gen_id();
    lower.builder.emit(Instruction::new(
        spirv::Op::Load, Some(int_ty), Some(row_val),
        vec![Operand::IdRef(ctx.row_id)],
    ));
    let row_mul = lower.builder.gen_id();
    let k4_c = lower.builder.builder.constant_bit64(int_ty, k_div4);
    lower.builder.emit(Instruction::new(
        spirv::Op::IMul, Some(int_ty), Some(row_mul),
        vec![Operand::IdRef(row_val), Operand::IdRef(k4_c)],
    ));

    // lane = gid & 31 — emit_expr handles the u32→i64 widening.
    let (gid64, _) = lower.emit_expr(&Expr::Call(
        "GetGlobalId#".into(), vec![Expr::Decimal(0)], None,
    ))?;
    let lane_id = lower.builder.gen_id();
    let mask = lower.builder.i64_const(31);
    lower.builder.emit(Instruction::new(
        spirv::Op::BitwiseAnd, Some(int_ty), Some(lane_id),
        vec![Operand::IdRef(gid64), Operand::IdRef(mask)],
    ));

    // partial = lane + row*(K/4)
    let partial = lower.builder.gen_id();
    lower.builder.emit(Instruction::new(
        spirv::Op::IAdd, Some(int_ty), Some(partial),
        vec![Operand::IdRef(lane_id), Operand::IdRef(row_mul)],
    ));

    // t * (stride/4)
    let t_val = lower.builder.gen_id();
    lower.builder.emit(Instruction::new(
        spirv::Op::Load, Some(int_ty), Some(t_val),
        vec![Operand::IdRef(loop_var)],
    ));
    let t_mul = lower.builder.gen_id();
    let s4_c = lower.builder.builder.constant_bit64(int_ty, ctx.stride / 4);
    lower.builder.emit(Instruction::new(
        spirv::Op::IMul, Some(int_ty), Some(t_mul),
        vec![Operand::IdRef(t_val), Operand::IdRef(s4_c)],
    ));

    // base = partial + t_mul
    let base = lower.builder.gen_id();
    lower.builder.emit(Instruction::new(
        spirv::Op::IAdd, Some(int_ty), Some(base),
        vec![Operand::IdRef(partial), Operand::IdRef(t_mul)],
    ));

    for (fname, vf, member_pos) in ctx.field_data {
        let v4_ptr = lower.builder.ptr_class(
            rspirv::spirv::StorageClass::StorageBuffer,
            vf.vector,
        );
        let member = lower.builder.u32_const(*member_pos as u32);
        let group = lower.builder.gen_id();
        lower.builder.emit(Instruction::new(
            spirv::Op::AccessChain,
            Some(v4_ptr),
            Some(group),
            vec![
                Operand::IdRef(ssbo),
                Operand::IdRef(member),
                Operand::IdRef(base),
            ],
        ));
        let v4_val = lower.builder.load(vf.vector, group);
        let elem_ty_id = lower.builder.lower_type(&vf.elem)?;
        for jj in 0u32..4 {
            let comp = lower.builder.gen_id();
            lower.builder.emit(Instruction::new(
                spirv::Op::CompositeExtract,
                Some(elem_ty_id),
                Some(comp),
                vec![Operand::IdRef(v4_val), Operand::LiteralBit32(jj)],
            ));
            let synthetic_name = format!("__vec4_{}_{}", fname, jj);
            lower.vec4_component_vars.insert(synthetic_name, (comp, vf.elem.clone()));
        }
    }
    Ok(())
}

/// Emit the body once per vec4 component: fields read their synthetic
/// component var; the scalar side substitutes the loop var with the FULL
/// cooperative index `repl + j` (= lane*4 + t*stride + j after binding).
fn emit_vec4_unrolled_body(
    lower: &mut FnLowerer,
    ctx: &Vec4LoopCtx,
    j: u32,
) -> Result<(), String> {
    let subst_j = Expr::BinaryOp(
        crate::ast::BinaryOpKind::Add,
        Box::new(ctx.repl.clone()),
        Box::new(crate::ast::Expr::Decimal(j as i64)),
    );
    let mut body_j = ctx.fbody.to_vec();
    for (fname, _, _) in ctx.field_data {
        let idx_expr = &ctx.all_indices.iter().find(|(f, _)| f == fname).unwrap().1;
        let lowered_idx = crate::backend::spirv::lower::subst_var_deep(
            idx_expr, ctx.item, &crate::ast::Expr::Identifier(ctx.item.to_string()),
        );
        let synthetic_var = crate::ast::Expr::Identifier(format!("__vec4_{}_{}", fname, j));
        for stmt in &mut body_j {
            *stmt = crate::backend::spirv::lower::replace_index_in_stmt(
                stmt, fname, &lowered_idx, &synthetic_var,
            );
        }
    }
    for stmt in &body_j {
        if lower.terminated { break; }
        let st = crate::backend::spirv::lower::subst_stmt_var(stmt, ctx.item, &subst_j);
        lower.emit_stmt(&st)?;
    }
    Ok(())
}

/// Basic-block set for the hand-built structured cooperative loop.
struct CoopLoopBBs {
    header_bb: Word,
    continue_bb: Word,
    merge_bb: Word,
}

/// Begin the hand-built structured loop (preheader → header), leaving the
/// builder positioned at the start of the body block. Mirrors the Foreach
/// emission in lower.rs; splitting begin/end lets the caller interleave the
/// body emission (vec4 loads depend on the loop variable).
fn begin_structured_loop(
    lower: &mut FnLowerer,
    loop_var: Word,
    int_ty: Word,
    bool_ty: Word,
    groups: i64,
) -> Result<(CoopLoopBBs, Word, Word), String> {
    let header_bb = lower.builder.gen_id();
    let body_bb = lower.builder.gen_id();
    let continue_bb = lower.builder.gen_id();
    let merge_bb = lower.builder.gen_id();
    let preheader_bb = lower.builder.gen_id();
    let cond0 = lower.builder.gen_id();
    let cond_next = lower.builder.gen_id();

    // Loop var starts at 0.
    let start_c = lower.builder.builder.constant_bit64(int_ty, 0);
    lower.builder.store(loop_var, start_c);

    let emit_cond = |lower: &mut FnLowerer, cond_id: Word| -> Result<(), String> {
        let v = lower.builder.gen_id();
        lower.builder.emit(Instruction::new(
            spirv::Op::Load, Some(int_ty), Some(v),
            vec![Operand::IdRef(loop_var)],
        ));
        let end_c = lower.builder.builder.constant_bit64(int_ty, groups as u64);
        lower.builder.emit(Instruction::new(
            spirv::Op::SLessThan, Some(bool_ty), Some(cond_id),
            vec![Operand::IdRef(v), Operand::IdRef(end_c)],
        ));
        Ok(())
    };

    lower.builder.builder.branch(preheader_bb);
    lower.builder.begin_block(Some(preheader_bb));
    emit_cond(lower, cond0)?;
    lower.builder.builder.branch(header_bb);

    lower.builder.begin_block(Some(header_bb));
    let cond_hdr = lower.builder.builder.phi(
        bool_ty,
        None,
        [(cond0, preheader_bb), (cond_next, continue_bb)],
    ).map_err(|e| format!("loop phi: {:?}", e))?;
    lower.builder.builder.loop_merge(
        merge_bb,
        continue_bb,
        rspirv::spirv::LoopControl::NONE,
        [] as [rspirv::dr::Operand; 0],
    );
    lower.builder.builder.branch_conditional(cond_hdr, body_bb, merge_bb, [] as [u32; 0]);
    lower.builder.begin_block(Some(body_bb));
    Ok((CoopLoopBBs { header_bb, continue_bb, merge_bb }, cond_next, cond0))
}

/// Close the structured loop: continue block (increment + re-check), branch
/// back to the header, then position the builder at the merge block.
fn end_structured_loop(
    lower: &mut FnLowerer,
    bbs: &CoopLoopBBs,
    loop_var: Word,
    int_ty: Word,
    groups: i64,
    cond_next: Word,
) -> Result<(), String> {
    lower.builder.builder.branch(bbs.continue_bb);
    lower.builder.begin_block(Some(bbs.continue_bb));
    let cur = lower.builder.gen_id();
    lower.builder.emit(Instruction::new(
        spirv::Op::Load, Some(int_ty), Some(cur),
        vec![Operand::IdRef(loop_var)],
    ));
    let one = lower.builder.builder.constant_bit64(int_ty, 1);
    let next = lower.builder.gen_id();
    lower.builder.emit(Instruction::new(
        spirv::Op::IAdd, Some(int_ty), Some(next),
        vec![Operand::IdRef(cur), Operand::IdRef(one)],
    ));
    lower.builder.emit(Instruction::new(
        spirv::Op::Store, None, None,
        vec![Operand::IdRef(loop_var), Operand::IdRef(next)],
    ));
    let v = lower.builder.gen_id();
    lower.builder.emit(Instruction::new(
        spirv::Op::Load, Some(int_ty), Some(v),
        vec![Operand::IdRef(loop_var)],
    ));
    let end_c = lower.builder.builder.constant_bit64(int_ty, groups as u64);
    let bool_ty = lower.builder.lower_type(&crate::ast::Type::Bits(1))?;
    lower.builder.emit(Instruction::new(
        spirv::Op::SLessThan,
        Some(bool_ty),
        Some(cond_next),
        vec![Operand::IdRef(v), Operand::IdRef(end_c)],
    ));
    lower.builder.builder.branch(bbs.header_bb);
    lower.builder.begin_block(Some(bbs.merge_bb));
    Ok(())
}

/// Resolve (field, Vec4Field, SSBO member position) triples for every vec4
/// index the cooperative body loads through.
fn collect_vec4_field_data(
    lower: &FnLowerer,
    all_indices: &[(String, Expr)],
) -> Result<Vec<(String, crate::backend::spirv::lower::Vec4Field, usize)>, String> {
    let mut field_data = Vec::new();
    for (fname, _) in all_indices {
        let vf = lower.vec4_fields.get(fname)
            .ok_or_else(|| format!("vec4 field '{}' lost", fname))?
            .clone();
        let member_pos = lower.state_fields.iter().position(|f| f.name == *fname)
            .ok_or_else(|| format!("vec4 field '{}' not in state", fname))?;
        field_data.push((fname.clone(), vf, member_pos));
    }
    Ok(field_data)
}

/// Vec4 cooperative path: a hand-built structured loop so the vec4 loads
/// execute INSIDE each iteration (they depend on the loop variable). The
/// body is unrolled 4x — one vec4 load feeding 4 scalar FMAs.
fn emit_cooperative_vec4(
    lower: &mut FnLowerer,
    shape: &KernelShape,
    item: &str,
    inner_len: u64,
    stride: u64,
    repl: &Expr,
) -> Result<(), String> {
    let ssbo = lower.ssbo_var.ok_or("cooperative vec4 without SSBO")?;
    let fbody = match shape.kernel_stmts.iter().find_map(|s| match s {
        Statement::Foreach { body, .. } => Some(body.clone()),
        _ => None,
    }) {
        Some(b) => b,
        None => return Err("cooperative kernel lost its foreach".into()),
    };
    let all_indices = collect_dedup_vec4_indices(lower, &fbody, item);
    let field_data = collect_vec4_field_data(lower, &all_indices)?;
    let (pre_loop, post_loop) = split_at_foreach(&shape.kernel_stmts);
    for stmt in &pre_loop {
        if lower.terminated { break; }
        lower.emit_stmt(stmt)?;
    }

    let groups = (inner_len / stride) as i64;
    let int_ty = lower.builder.lower_type(&crate::ast::Type::int())?;
    let bool_ty = lower.builder.lower_type(&crate::ast::Type::Bits(1))?;
    let loop_var = lower.vars.get(item).map(|(v, _)| *v)
        .ok_or_else(|| format!("loop var '{}' not pre-declared", item))?;

    let (bbs, cond_next, _cond0) =
        begin_structured_loop(lower, loop_var, int_ty, bool_ty, groups)?;

    lower.const_vars.remove(item);
    lower.vars.insert(item.to_string(), (loop_var, crate::ast::Type::int()));
    let prev_terminated = lower.terminated;
    lower.terminated = false;

    let ctx = Vec4LoopCtx {
        item,
        repl,
        fbody: &fbody,
        field_data: &field_data,
        all_indices: &all_indices,
        stride,
        inner_len,
        row_id: *lower.vars.get(&shape.index_var).map(|(v, _)| v)
            .ok_or("row var not found")?,
    };
    for j in 0..4u32 {
        emit_vec4_field_loads(lower, &ctx, loop_var, int_ty, ssbo)?;
        emit_vec4_unrolled_body(lower, &ctx, j)?;
    }

    end_structured_loop(lower, &bbs, loop_var, int_ty, groups, cond_next)?;
    lower.terminated = prev_terminated;

    emit_coop_reduce_store(lower, &post_loop, shape)
}

/// Scalar cooperative path: substitute the loop var with `lane + t*32` and
/// emit through the ordinary Foreach machinery (which unrolls the tail).
fn emit_cooperative_scalar(
    lower: &mut FnLowerer,
    shape: &KernelShape,
    item: &str,
    fbody: &[Statement],
    inner_len: u64,
    repl: &Expr,
) -> Result<(), String> {
    let new_body: Vec<Statement> = fbody.iter()
        .map(|st| crate::backend::spirv::lower::subst_stmt_var(st, item, repl))
        .collect();
    let groups = (inner_len / 32) as i64;
    let synthesized = Statement::Foreach {
        item: item.to_string(),
        list: Box::new(Expr::Range {
            start: Box::new(crate::ast::Expr::Decimal(0)),
            end: Box::new(crate::ast::Expr::Decimal(groups)),
            inclusive: false,
        }),
        body: new_body,
    };
    let (pre_loop, post_loop) = split_at_foreach(&shape.kernel_stmts);
    for stmt in &pre_loop {
        if lower.terminated { break; }
        lower.emit_stmt(stmt)?;
    }
    lower.emit_stmt(&synthesized)?;
    emit_coop_reduce_store(lower, &post_loop, shape)
}

/// Synthesize the cooperative body for a recognized dot-product reduction:
/// the foreach iterates `t in 0..K/stride` with the original loop var mapped
/// to `lane*4 + t*stride` (coalesced stride) when vec4-eligible fields are
/// present, or `lane + t*32` otherwise. The accumulator ends in a subgroup
/// FAdd, and the counter increment is dropped (the runner fast-forwards).
fn emit_cooperative_reduce(
    lower: &mut FnLowerer,
    shape: &KernelShape,
    inner_len: u64,
) -> Result<(), String> {
    let (item, fbody) = match shape.kernel_stmts.iter().find_map(|s| match s {
        Statement::Foreach { item, body, .. } => Some((item.clone(), body.clone())),
        _ => None,
    }) {
        Some(v) => v,
        None => return Err("cooperative kernel lost its foreach".into()),
    };

    // Detect vec4-eligible fields in the body. When present, each lane loads
    // 4 consecutive floats per iteration — one vec4 load instead of 4 scalar
    // loads. The stride becomes 128 (4 elements × 32 lanes) instead of 32.
    let vec4_indices = collect_dedup_vec4_indices(lower, &fbody, &item);
    let use_vec4 = !vec4_indices.is_empty()
        && vec4_indices.iter().all(|(fname, _)| {
            lower.vec4_fields.get(fname).map(|vf| vf.elem_float).unwrap_or(false)
        });
    let stride: u64 = if use_vec4 { 128 } else { 32 };

    // The strided loop REUSES the original loop-var name (it is the one
    // pre-declared local collect_locals saw); the replacement inserts the
    // same name as the group index — subst inserts the replacement without
    // re-processing it, so this is safe.
    let lane: Expr = Expr::BinaryOp(
        crate::ast::BinaryOpKind::BitAnd,
        Box::new(Expr::Call("GetGlobalId#".into(), vec![Expr::Decimal(0)], None)),
        Box::new(Expr::Decimal(31)),
    );
    let lane_term = if use_vec4 {
        Expr::BinaryOp(
            crate::ast::BinaryOpKind::Mul,
            Box::new(lane),
            Box::new(Expr::Decimal(4)),
        )
    } else {
        lane
    };
    let repl = Expr::BinaryOp(
        crate::ast::BinaryOpKind::Add,
        Box::new(lane_term),
        Box::new(Expr::BinaryOp(
            crate::ast::BinaryOpKind::Mul,
            Box::new(Expr::Identifier(item.clone())),
            Box::new(Expr::Decimal(stride as i64)),
        )),
    );

    if use_vec4 {
        emit_cooperative_vec4(lower, shape, &item, inner_len, stride, &repl)
    } else {
        emit_cooperative_scalar(lower, shape, &item, &fbody, inner_len, &repl)
    }
}
