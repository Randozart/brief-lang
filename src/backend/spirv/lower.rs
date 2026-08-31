//! SPIR-V statement/expression lowering — the real kernel body emitter.
//!
//! 2026-08-23 (plan 2026-08-23-spirv-kernel-emission §2.1): replaces the
//! placeholder body block (bare Op.Return). Lowers the bounded subset a
//! compute kernel needs: integer arithmetic/logic/comparisons, locals,
//! GetGlobalId#/GetLocalId# builtins, and program-state access through ONE
//! StorageBuffer binding (Block-decorated struct, deterministic member
//! order = sorted by field name).
//!
//! Rule-19 note: element types come from Briev `Type` values via the
//! TypeCache, never from name matches on user types.
//!
//! To undo: revert kernel.rs to placeholder body + delete this file.

use std::collections::HashMap;

use rspirv::dr::{Instruction, Operand};
use rspirv::spirv::{self, StorageClass, Word};

use crate::ast::{Expr, Statement, Type, UnaryOpKind};
use crate::casting::graph::SpirvShape;
use crate::backend::spirv::builder::SpirvBuilder;

/// One program-state field referenced by the kernel body. Collected before
/// emission so the SSBO layout is stable regardless of use order.
#[derive(Debug, Clone)]
pub struct StateField {
    pub name: String,
    pub ty: Type,
}

pub struct FnLowerer<'a> {
    pub builder: &'a mut SpirvBuilder,
    /// Local variables: name → (Function-storage pointer id, type).
    pub vars: HashMap<String, (Word, Type)>,
    /// State fields exposed through the SSBO: sorted name → (type, member idx).
    pub state_fields: Vec<StateField>,
    /// SSBO variable id (StorageBuffer storage class); set by setup_state_buffer.
    pub ssbo_var: Option<Word>,
    /// BuiltIn GlobalInvocationId input variable (pre-threaded or lazy).
    pub global_id_var: Option<Word>,
    /// BuiltIn LocalInvocationId input variable (pre-threaded or lazy).
    pub local_id_var: Option<Word>,
    /// 2026-08-31 (plan abv-gpu-by-default): module consts materialized as
    /// SPIR-V constants — name → (constant id, type). Kernel bodies read
    /// `const dt: Float = 0.001;` etc. directly; non-literal initializers
    /// error at materialization.
    pub consts: HashMap<String, (Word, Type)>,
    /// Set when the body executed a term/endprogram — callers stop
    /// branching afterwards (a block can only have one terminator).
    pub terminated: bool,
}

impl<'a> FnLowerer<'a> {
    pub fn new(builder: &'a mut SpirvBuilder, state_fields: Vec<StateField>) -> Self {
        FnLowerer {
            builder,
            vars: HashMap::new(),
            state_fields,
            ssbo_var: None,
            global_id_var: None,
            local_id_var: None,
            consts: HashMap::new(),
            terminated: false,
        }
    }

    /// 2026-08-31 (plan abv-gpu-by-default): materialize module consts as
    /// SPIR-V constants for direct (no-load) reads in kernel bodies. Only
    /// literal initializers (int/float/bool) are supported — anything else
    /// errors naming the const, since kernels cannot evaluate host-side
    /// expressions at module scope.
    pub fn materialize_consts(&mut self, items: &[crate::ast::TopLevel]) -> Result<(), String> {
        for item in items {
            let crate::ast::TopLevel::Constant(c) = item else { continue };
            let (id, ty) = match &c.expr {
                Expr::Decimal(n) => {
                    let id = self.builder.i64_const(*n as u64);
                    (id, Type::int())
                }
                Expr::Float(v) => {
                    let bits = self.builder.float_bits_of(&c.ty)?;
                    let id = self.builder.float_const(bits, *v);
                    (id, c.ty.clone())
                }
                Expr::Bool(b) => {
                    let bool_ty = self.builder.lower_type(&Type::Bits(1))?;
                    let op = if *b { spirv::Op::ConstantTrue } else { spirv::Op::ConstantFalse };
                    let id = self.builder.gen_id();
                    self.builder.emit_global(Instruction::new(
                        op,
                        Some(bool_ty),
                        Some(id),
                        vec![],
                    ));
                    (id, Type::Bits(1))
                }
                other => {
                    return self.err(format!(
                        "const '{}' has a non-literal initializer ({:?}) — kernels \
                         read literal consts only; inline the expression",
                        c.name, std::mem::discriminant(other)
                    ));
                }
            };
            self.consts.insert(c.name.clone(), (id, ty));
        }
        Ok(())
    }

    /// Force-create both invocation-id builtin variables as module globals.
    /// SPIR-V entry-point interfaces are complete from the start; warming
    /// avoids arm-order dependence.
    pub fn warm_builtins(&mut self) -> Result<(), String> {
        self.global_invocation_id()?;
        self.local_invocation_id()?;
        Ok(())
    }

    fn err<T>(&self, what: impl Into<String>) -> Result<T, String> {
        Err(format!("SPIR-V lowering: {}", what.into()))
    }

    // ── Statements ──────────────────────────────────────────────────────

    pub fn emit_stmt(&mut self, stmt: &Statement) -> Result<(), String> {
        match stmt {
            Statement::Let { name, ty, expr: Some(e), .. } => {
                // 2026-08-23: the VARIABLE itself was pre-declared in the
                // entry block (SPIR-V requires every function-scope
                // OpVariable in the FIRST block); here we only store.
                // 2026-08-31 (VITRIOL GEMM comparison M1): a FLOAT let with a
                // DECIMAL initializer (`let acc: Float = 0;`) needs the zero
                // materialized as the float-typed constant - storing an i64
                // zero into a float variable is an OpStore type mismatch.
                let declared_float = ty
                    .as_ref()
                    .map(|t| self.builder.is_float_type(t).unwrap_or(false))
                    .unwrap_or(false);
                let (val, _ty) = match (declared_float, e) {
                    (true, Expr::Decimal(n)) => {
                        let bits = self.builder.float_bits_of(ty.as_ref().unwrap())?;
                        let c = self.builder.float_const(bits, *n as f64);
                        (c, ty.clone().unwrap())
                    }
                    _ => self.emit_expr(e)?,
                };
                let Some((var, _)) = self.vars.get(name.as_str()) else {
                    return self.err(format!("local '{}' was not pre-declared", name));
                };
                let var = *var;
                self.builder.store(var, val);
                Ok(())
            }
            Statement::Let { expr: None, .. } => {
                self.err("uninitialized let is not supported in kernels")
            }
            Statement::Assign(lhs, rhs) => {
                let (val, _ty) = self.emit_expr(rhs)?;
                let (ptr, _pty) = self.lhs_addr(lhs)?;
                self.builder.emit(Instruction::new(
                    spirv::Op::Store,
                    None,
                    None,
                    vec![Operand::IdRef(ptr), Operand::IdRef(val)],
                ));
                Ok(())
            }
            Statement::Expression(e) => {
                self.emit_expr(e)?;
                Ok(())
            }
            Statement::Term(v) | Statement::EndProgram(v) => {
                if let Some(e) = v {
                    self.emit_expr(e)?;
                }
                // Typed ret — raw Instruction emission leaves the block
                // 'open' in rspirv's state and the next begin_block panics.
                self.builder.ret();
                self.terminated = true;
                Ok(())
            }
            // 2026-08-31 (VITRIOL GEMM comparison M1): a bounded foreach
            // lowers to a STRUCTURED loop (OpLoopMerge) over a Function-scope
            // loop variable. The loop var was pre-declared in the entry block
            // by collect_locals; the body obeys the same scalar/array rules.
            Statement::Foreach { item, list, body } => {
                let Expr::Range { start, end, inclusive } = list.as_ref() else {
                    return self.err(
                        "foreach over a non-range collection - kernel loops iterate `start..end` ranges only",
                    );
                };
                let (start_v, sty) = self.emit_expr(start)?;
                let Some((var, _)) = self.vars.get(item) else {
                    return self.err(format!("foreach item '{}' was not pre-declared", item));
                };
                let var = *var;
                let _ = sty;
                let int_ty = self.type_id(&Type::int())?;
                let bool_ty = self.type_id(&Type::Bits(1))?;
                let cmp_op = if *inclusive { spirv::Op::SLessThanEqual } else { spirv::Op::SLessThan };

                let header = self.builder.gen_id();
                let body_bb = self.builder.gen_id();
                let continue_bb = self.builder.gen_id();
                let merge = self.builder.gen_id();
                let preheader_bb = self.builder.gen_id();
                let cond0 = self.builder.gen_id();
                let cond_next = self.builder.gen_id();

                // Condition on the loop variable vs the range end. Emitted
                // TWICE: once in the preheader (first test) and once at the
                // end of the continue block (re-test) - the header itself
                // only merges the two through a Phi, because OpLoopMerge must
                // immediately precede its branch and a Phi must open a block.
                let emit_cond = |lower: &mut Self, cond: u32| -> Result<(), String> {
                    let v = lower.builder.gen_id();
                    lower.builder.emit(Instruction::new(
                        spirv::Op::Load,
                        Some(int_ty),
                        Some(v),
                        vec![Operand::IdRef(var)],
                    ));
                    let (end_v, _ety) = lower.emit_expr(end)?;
                    lower.builder.emit(Instruction::new(
                        cmp_op,
                        Some(bool_ty),
                        Some(cond),
                        vec![Operand::IdRef(v), Operand::IdRef(end_v)],
                    ));
                    Ok(())
                };

                // Preheader: seed the loop variable, first test, enter header.
                self.builder.store(var, start_v);
                // The current (guard) block must terminate by branching into
                // the preheader - begin_block panics on an unterminated block.
                self.builder.builder.branch(preheader_bb);
                self.builder.begin_block(Some(preheader_bb));
                emit_cond(self, cond0)?;
                self.builder.builder.branch(header);

                // Header: merge annotation + Phi + branch.
                self.builder.begin_block(Some(header));
                // Order inside the header: OpPhi first (phis open a block),
                // then OpLoopMerge, which must immediately precede the
                // OpBranchConditional (SPIR-V structured-loop rule).
                let cond_hdr = self
                    .builder
                    .builder
                    .phi(
                        bool_ty,
                        None,
                        [(cond0, preheader_bb), (cond_next, continue_bb)],
                    )
                    .map_err(|e| format!("loop phi: {:?}", e))?;
                self.builder
                    .builder
                    .loop_merge(merge, continue_bb, rspirv::spirv::LoopControl::NONE, [] as [rspirv::dr::Operand; 0]);
                self.builder
                    .builder
                    .branch_conditional(cond_hdr, body_bb, merge, [] as [u32; 0]);

                // Body: work-item statements, then fall into the continue.
                self.builder.begin_block(Some(body_bb));
                self.vars.insert(item.clone(), (var, Type::int()));
                let prev_terminated = self.terminated;
                self.terminated = false;
                for s in body {
                    if self.terminated {
                        break;
                    }
                    self.emit_stmt(s)?;
                }
                self.builder.builder.branch(continue_bb);

                // Continue: increment, re-test, loop back.
                self.builder.begin_block(Some(continue_bb));
                let step_ty = self.type_id(&Type::int())?;
                let cur = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    spirv::Op::Load,
                    Some(step_ty),
                    Some(cur),
                    vec![Operand::IdRef(var)],
                ));
                let one = self.builder.builder.constant_bit64(step_ty, 1);
                let next = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    spirv::Op::IAdd,
                    Some(step_ty),
                    Some(next),
                    vec![Operand::IdRef(cur), Operand::IdRef(one)],
                ));
                self.builder.emit(Instruction::new(
                    spirv::Op::Store,
                    None,
                    None,
                    vec![Operand::IdRef(var), Operand::IdRef(next)],
                ));
                emit_cond(self, cond_next)?;
                self.builder.builder.branch(header);

                // Merge: execution resumes after the loop.
                self.builder.begin_block(Some(merge));
                self.terminated = prev_terminated;
                Ok(())
            }
            other => self.err(format!(
                "unsupported statement in kernel body ({:?}) — compute kernels \
                 support let/assign/expression/term over integer expressions",
                std::mem::discriminant(other)
            )),
        }
    }

    /// Resolve an assignment target to a storage POINTER id.
    /// Identifiers → Function var; `field[idx]` → SSBO AccessChain.
    fn lhs_addr(&mut self, lhs: &Expr) -> Result<(Word, String), String> {
        match lhs {
            Expr::Identifier(name) => {
                let Some((var, _ty)) = self.vars.get(name) else {
                    return self.err(format!("assignment to unknown '{}'", name));
                };
                Ok((*var, name.clone()))
            }
            Expr::Index(obj, idx) => {
                let Some(field_name) = field_name_of(obj) else {
                    return self.err("only direct state-field indexing is supported");
                };
                let (idx_val, _) = self.emit_expr(idx)?;
                let (elem_ptr, _) = self.state_field_elem_ptr(field_name, idx_val)?;
                Ok((elem_ptr, field_name.to_string()))
            }
            other => self.err(format!(
                "unsupported assignment target ({:?})",
                std::mem::discriminant(other)
            )),
        }
    }

    // ── Expressions ─────────────────────────────────────────────────────

    pub fn emit_expr(&mut self, e: &Expr) -> Result<(Word, Type), String> {
        match e {
            Expr::Decimal(n) => {
                let c = self.builder.i64_const(*n as u64);
                Ok((c, Type::int()))
            }
            // 2026-08-31 (plan abv-gpu-by-default): float literals lower to
            // bit-pattern constants of the default float type (f32 unless the
            // module's Float metadata widens it — resolved via the casting
            // graph, never a name match).
            Expr::Float(v) => {
                let ty = Type::float();
                let bits = self.builder.float_bits_of(&ty)?;
                let c = self.builder.float_const(bits, *v);
                Ok((c, ty))
            }
            Expr::Identifier(name) => {
                if let Some((var, ty)) = self.vars.get(name) {
                    let (var, ty) = (*var, ty.clone());
                    let result_ty = self.type_id(&ty)?;
                    let loaded = self.builder.gen_id();
                    self.builder.emit(Instruction::new(
                        spirv::Op::Load,
                        Some(result_ty),
                        Some(loaded),
                        vec![Operand::IdRef(var)],
                    ));
                    return Ok((loaded, ty));
                }
                // 2026-08-31 (plan abv-gpu-by-default): module consts are
                // SPIR-V constants — the id is used directly, no load.
                if let Some((id, ty)) = self.consts.get(name) {
                    return Ok((*id, ty.clone()));
                }
                // 2026-08-31 (plan abv-gpu-by-default): state SCALARS resolve
                // through the SSBO (e.g. the `[i < nb]` bound read inside the
                // kernel's bounds guard) — AccessChain + Load.
                if self.state_fields.iter().any(|f| &f.name == name) {
                    let (ptr, fty) = self.state_field_scalar_ptr(name)?;
                    let tid = self.type_id(&fty)?;
                    let loaded = self.builder.load(tid, ptr);
                    return Ok((loaded, fty));
                }
                self.err(format!("unknown identifier '{}' in kernel", name))
            }
            // 2026-08-23 (§2.1 read path): state-field element LOADS —
            // out[i] = a[i] + b[i] needs AccessChain + Load on the SSBO.
            Expr::Index(obj, idx) => {
                let Some(fname) = FnLowerer::field_name_of(obj) else {
                    return self.err("only direct state-field indexing reads");
                };
                if !self.state_fields.iter().any(|f| f.name == fname) {
                    return self.err(format!("unknown state field '{}' (declare it as indexed state)", fname));
                }
                let (idx_val, _) = self.emit_expr(idx)?;
                let (ptr, ety) = self.state_field_elem_ptr(fname, idx_val)?;
                let tid = self.type_id(&ety)?;
                let loaded = self.builder.load(tid, ptr);
                Ok((loaded, ety))
            }
            Expr::BinaryOp(kind, l, r) => self.emit_binop(kind, l, r),
            // 2026-08-31 (plan abv-gpu-by-default): `as` casts between scalar
            // numeric kinds. Opcode + signedness derive from the operands'
            // protocol shapes (rule 19); same-shape casts are a passthrough.
            Expr::Cast(e, target) => {
                let (val, src_ty) = self.emit_expr(e)?;
                if std::env::var("BRIEV_SPIRV_DEBUG").is_ok() {
                    eprintln!("[spirv-debug] cast src_ty={src_ty:?} target={target:?}");
                }
                let src_shape = self.builder.shape_of(&src_ty)?;
                let dst_shape = self.builder.shape_of(target)?;
                let op = Self::cast_opcode(&src_shape, &dst_shape)?;;
                match op {
                    None => Ok((val, target.clone())),
                    Some(op) => {
                        let dst_ty_id = self.type_id(target)?;
                        let res = self.builder.gen_id();
                        self.builder.emit(Instruction::new(
                            op,
                            Some(dst_ty_id),
                            Some(res),
                            vec![Operand::IdRef(val)],
                        ));
                        Ok((res, target.clone()))
                    }
                }
            }
            // 2026-08-31 (plan abv-gpu-by-default): unary ops over the same
            // scalar surface — opcode follows the operand's shape (floats
            // negate via 0 − x; SPIR-V has no OpFNegate in the core set
            // without GLSL.std.450 extended instructions).
            Expr::UnaryOp(kind, e) => {
                let (val, ty) = self.emit_expr(e)?;
                let shape = self.builder.shape_of(&ty)?;
                let ty_id = self.type_id(&ty)?;
                match (kind, &shape) {
                    (UnaryOpKind::Neg, SpirvShape::Float { .. }) => {
                        let zero = self.builder.float_const(
                            match &shape { SpirvShape::Float { bits } => *bits, _ => 32 },
                            0.0,
                        );
                        let res = self.builder.gen_id();
                        self.builder.emit(Instruction::new(
                            spirv::Op::FSub,
                            Some(ty_id),
                            Some(res),
                            vec![Operand::IdRef(zero), Operand::IdRef(val)],
                        ));
                        Ok((res, ty))
                    }
                    (UnaryOpKind::Neg, SpirvShape::Int { signed: true, bits }) => {
                        // Zero constant OF THE OPERAND'S type — ISub operands
                        // must share the negated value's width.
                        let zero = match bits {
                            32 => self.builder.builder.constant_bit32(ty_id, 0),
                            64 => self.builder.builder.constant_bit64(ty_id, 0),
                            other => {
                                return self.err(format!(
                                    "negation of {}-bit int — kernels negate 32/64-bit \
                                     values (cast the operand first)",
                                    other
                                ))
                            }
                        };
                        let res = self.builder.gen_id();
                        self.builder.emit(Instruction::new(
                            spirv::Op::ISub,
                            Some(ty_id),
                            Some(res),
                            vec![Operand::IdRef(zero), Operand::IdRef(val)],
                        ));
                        Ok((res, ty))
                    }
                    (UnaryOpKind::Neg, SpirvShape::Int { signed: false, .. }) => {
                        self.err("negating an unsigned value — use a signed type")
                    }
                    (UnaryOpKind::Not, _) => {
                        let bool_ty = self.builder.lower_type(&Type::Bits(1))?;
                        let res = self.builder.gen_id();
                        self.builder.emit(Instruction::new(
                            spirv::Op::LogicalNot,
                            Some(bool_ty),
                            Some(res),
                            vec![Operand::IdRef(val)],
                        ));
                        Ok((res, Type::Bits(1)))
                    }
                    (UnaryOpKind::BitNot, SpirvShape::Int { .. }) => {
                        let res = self.builder.gen_id();
                        self.builder.emit(Instruction::new(
                            spirv::Op::Not,
                            Some(ty_id),
                            Some(res),
                            vec![Operand::IdRef(val)],
                        ));
                        Ok((res, ty))
                    }
                    (UnaryOpKind::Neg, SpirvShape::Bool) => {
                        self.err("negating a Bool — logic needs true/false, not arithmetic")
                    }
                    (UnaryOpKind::BitNot, _) => {
                        self.err("bitwise-not on a non-integer kernel value")
                    }
                }
            }
            Expr::Call(name, args, _) => self.emit_intrinsic_call(name, args),
            other => self.err(format!(
                "unsupported expression in kernel ({:?}) — integer scalar \
                 compute is the supported surface",
                std::mem::discriminant(other)
            )),
        }
    }

    fn emit_intrinsic_call(&mut self, name: &str, args: &[Expr]) -> Result<(Word, Type), String> {
        match name {
            "GetGlobalId#" | "GetLocalId#" => {
                let dim = match args.first() {
                    Some(Expr::Decimal(d)) if *d >= 0 && *d <= 2 => *d as u32,
                    _ => return self.err("builtins take a constant dimension 0..=2"),
                };
                let var = match name {
                    "GetGlobalId#" => self.global_invocation_id()?,
                    _ => self.local_invocation_id()?,
                };
                // Component pointer: AccessChain(var, dim)
                let u32_ty = self.builder.u32_type();
                let ptr_u32 = self.builder.ptr_class(StorageClass::Input, u32_ty);
                let dim_const = self.builder.u32_const(dim);
                let comp = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    spirv::Op::AccessChain,
                    Some(ptr_u32),
                    Some(comp),
                    vec![Operand::IdRef(var), Operand::IdRef(dim_const)],
                ));
                let raw = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    spirv::Op::Load,
                    Some(u32_ty),
                    Some(raw),
                    vec![Operand::IdRef(comp)],
                ));
                // Widen u32 → i64. 2026-08-31 (plan abv-gpu-by-default):
                // Vulkan's Shader environment only allows OpUConvert to an
                // UNSIGNED result — zero-extend to u64 then OpBitcast to the
                // signed i64 the kernel surface uses.
                let int_res_ty = self.builder.lower_type(&Type::int())?;
                let u64_ty = self.builder.builder.type_int(64, 0);
                let wide_u = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    spirv::Op::UConvert,
                    Some(u64_ty),
                    Some(wide_u),
                    vec![Operand::IdRef(raw)],
                ));
                let wide = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    spirv::Op::Bitcast,
                    Some(int_res_ty),
                    Some(wide),
                    vec![Operand::IdRef(wide_u)],
                ));
                Ok((wide, Type::int()))
            }
            "WorkgroupSize#" => {
                // Constant per LocalSize execution mode (64,1,1 set by kernel.rs).
                let dim = match args.first() {
                    Some(Expr::Decimal(d)) if *d >= 0 && *d <= 2 => *d as u32,
                    _ => return self.err("builtins take a constant dimension 0..=2"),
                };
                let sizes = [64u64, 1, 1];
                let c = self.builder.i64_const(sizes[dim as usize]);
                Ok((c, Type::int()))
            }
            "Load#" | "Store#" => {
                // 2026-08-26 (§2.3): raw numeric addresses cannot exist in a
                // Vulkan kernel (no inttoptr over SSBO memory). The honest
                // kernel form of LLVM's raw-address Load#/Store# is an
                // ADDRESS EXPRESSION rooted in the StorageBuffer:
                //   Load#(field) / Load#(field[i]) / Store#(field[i], v)
                // lowered to AccessChain + OpLoad/OpStore on the buffer's
                // typed members. Element widths come from the declared
                // field type, not the byte-count argument.
                if name == "Load#" {
                    let (ptr, elem_ty) = self.emit_addr(args.first().ok_or_else(|| {
                        "SPIR-V lowering: Load# needs an address expression".to_string()
                    })?)?;
                    self.check_width_arg(args.get(1), &elem_ty, "Load#")?;
                    let tid = self.type_id(&elem_ty)?;
                    let loaded = self.builder.load(tid, ptr);
                    Ok((loaded, elem_ty))
                } else {
                    let Some(addr_arg) = args.first() else {
                        return self.err("Store# needs an address expression");
                    };
                    let Some(val_expr) = args.get(1) else {
                        return self.err("Store# needs a value to store");
                    };
                    let (ptr, elem_ty) = self.emit_addr(addr_arg)?;
                    self.check_width_arg(args.get(2), &elem_ty, "Store#")?;
                    let (val, val_ty) = self.emit_expr(val_expr)?;
                    if val_ty != elem_ty {
                        return self.err(format!(
                            "Store# of {:?} into {:?} storage would silently \
                             truncate/convert — cast at the source",
                            val_ty, elem_ty
                        ));
                    }
                    self.builder.store(ptr, val);
                    Ok((val, val_ty))
                }
            }
            other => self.err(format!("unsupported intrinsic '{}'", other)),
        }
    }

    fn emit_binop(&mut self, kind: &crate::ast::BinaryOpKind, l: &Expr, r: &Expr)
        -> Result<(Word, Type), String>
    {
        use crate::ast::BinaryOpKind::*;
        let (lid, lty) = self.emit_expr(l)?;
        let (rid, rty) = self.emit_expr(r)?;
        // 2026-08-31 (plan abv-gpu-by-default): opcode selection is driven by
        // the OPERANDS' protocol category — float-shaped operands get the F*
        // opcode family and their own result type; everything else keeps the
        // integer family. No type names are matched (rule 19).
        let float_lane = self.builder.is_float_type(&lty)?
            || self.builder.is_float_type(&rty)?;
        if float_lane {
            return self.emit_float_binop(kind, lid, lty.clone(), rid, rty.clone());
        }
        let result_int = self.builder.lower_type(&Type::int())?;
        let op = match kind {
            Add => spirv::Op::IAdd,
            Sub => spirv::Op::ISub,
            Mul => spirv::Op::IMul,
            Div => spirv::Op::SDiv,
            Mod => spirv::Op::SRem,
            BitAnd => spirv::Op::BitwiseAnd,
            BitOr => spirv::Op::BitwiseOr,
            BitXor => spirv::Op::BitwiseXor,
            Shl => spirv::Op::ShiftLeftLogical,
            Shr => spirv::Op::ShiftRightArithmetic,
            Lt => spirv::Op::SLessThan,
            Gt => spirv::Op::SGreaterThan,
            Le => spirv::Op::SLessThanEqual,
            Ge => spirv::Op::SGreaterThanEqual,
            Eq => spirv::Op::IEqual,
            Neq => spirv::Op::INotEqual,
            And | Or => {
                // Logical over bool operands.
                let op = if matches!(kind, And) { spirv::Op::LogicalAnd } else { spirv::Op::LogicalOr };
                let bool_ty = self.builder.lower_type(&Type::Bits(1))?;
                let res = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    op,
                    Some(bool_ty),
                    Some(res),
                    vec![Operand::IdRef(lid), Operand::IdRef(rid)],
                ));
                return Ok((res, Type::Bits(1)));
            }
            Concat => return self.err("string concat is not a kernel operation"),
        };
        let is_cmp = matches!(kind, Lt | Gt | Le | Ge | Eq | Neq);
        let res_ty = if is_cmp {
            self.builder.lower_type(&Type::Bits(1))?
        } else {
            result_int
        };
        // Both operands must share the lowered type id (Int vs Bits widths).
        let lid = self.coerce(lid, &lty, &rty)?;
        let rid = self.coerce(rid, &rty, &lty)?;
        let res = self.builder.gen_id();
        self.builder.emit(Instruction::new(
            op,
            Some(res_ty),
            Some(res),
            vec![Operand::IdRef(lid), Operand::IdRef(rid)],
        ));
        Ok((
            res,
            if is_cmp { Type::Bits(1) } else { lty },
        ))
    }

    /// 2026-08-31 (plan abv-gpu-by-default): the float opcode family. Mixed
    /// float/int operands error (cast at the source — same policy as Store#
    /// width checks); bitwise ops on floats are not kernel operations.
    fn emit_float_binop(
        &mut self,
        kind: &crate::ast::BinaryOpKind,
        lid: Word,
        lty: Type,
        rid: Word,
        rty: Type,
    ) -> Result<(Word, Type), String> {
        use crate::ast::BinaryOpKind::*;
        if !self.builder.is_float_type(&rty)? {
            return self.err(format!(
                "mixed float/int arithmetic ({:?} on {:?} and {:?}) — cast the \
                 integer operand to the float type at the source",
                kind, lty, rty
            ));
        }
        let op = match kind {
            Add => spirv::Op::FAdd,
            Sub => spirv::Op::FSub,
            Mul => spirv::Op::FMul,
            Div => spirv::Op::FDiv,
            Mod => spirv::Op::FRem,
            Lt => spirv::Op::FOrdLessThan,
            Gt => spirv::Op::FOrdGreaterThan,
            Le => spirv::Op::FOrdLessThanEqual,
            Ge => spirv::Op::FOrdGreaterThanEqual,
            Eq => spirv::Op::FOrdEqual,
            Neq => spirv::Op::FOrdNotEqual,
            BitAnd | BitOr | BitXor | Shl | Shr => {
                return self.err("bitwise ops on floats are not kernel operations")
            }
            And | Or => return self.err("logical ops need Bool operands, not floats"),
            Concat => return self.err("string concat is not a kernel operation"),
        };
        let is_cmp = matches!(kind, Lt | Gt | Le | Ge | Eq | Neq);
        let res_ty = if is_cmp {
            self.builder.lower_type(&Type::Bits(1))?
        } else {
            // coerce enforces both sides share the lowered type id first.
            let lid = self.coerce(lid, &lty, &rty)?;
            let rid = self.coerce(rid, &rty, &lty)?;
            let ty_id = self.type_id(&lty)?;
            let res = self.builder.gen_id();
            self.builder.emit(Instruction::new(
                op,
                Some(ty_id),
                Some(res),
                vec![Operand::IdRef(lid), Operand::IdRef(rid)],
            ));
            return Ok((res, lty));
        };
        let lid = self.coerce(lid, &lty, &rty)?;
        let rid = self.coerce(rid, &rty, &lty)?;
        let res = self.builder.gen_id();
        self.builder.emit(Instruction::new(
            op,
            Some(res_ty),
            Some(res),
            vec![Operand::IdRef(lid), Operand::IdRef(rid)],
        ));
        Ok((res, Type::Bits(1)))
    }

    /// Zero-width mismatches are a lowering bug; identical types pass through.
    fn coerce(&mut self, id: Word, ty: &Type, other: &Type) -> Result<Word, String> {
        if self.type_id(ty)? == self.type_id(other)? {
            Ok(id)
        } else {
            self.err(format!("operand type mismatch {:?} vs {:?}", ty, other))
        }
    }

/// 2026-08-31 (plan abv-gpu-by-default): scalar cast opcode from the source
/// and destination shapes. `None` = identity (same kind + width — the value
/// passes through). The INT side's signedness picks the S/U conversion
/// family; float width changes pick trunc/ext. Bool mixes have no direct
/// scalar conversion and error naming the fix.
fn cast_opcode(
    src: &crate::casting::graph::SpirvShape,
    dst: &crate::casting::graph::SpirvShape,
) -> Result<Option<spirv::Op>, String> {
    use crate::casting::graph::SpirvShape;
    let same_kind = std::mem::discriminant(src) == std::mem::discriminant(dst);
    Ok(match (src, dst) {
        (SpirvShape::Int { bits: x, .. }, SpirvShape::Int { bits: y, .. }) if same_kind && x == y => None,
        (SpirvShape::Int { signed, .. }, SpirvShape::Int { .. }) if same_kind => {
            Some(if *signed { spirv::Op::SConvert } else { spirv::Op::UConvert })
        }
        (SpirvShape::Float { bits: x }, SpirvShape::Float { bits: y }) if same_kind && x == y => None,
        (SpirvShape::Float { bits: x }, SpirvShape::Float { bits: y }) if same_kind => {
            // SPIR-V has one OpFConvert for both float width directions.
            let _ = (x, y);
            Some(spirv::Op::FConvert)
        }
        (SpirvShape::Bool, SpirvShape::Bool) => None,
        (SpirvShape::Int { signed, .. }, SpirvShape::Float { .. }) => {
            Some(if *signed { spirv::Op::ConvertSToF } else { spirv::Op::ConvertUToF })
        }
        (SpirvShape::Float { .. }, SpirvShape::Int { signed, .. }) => {
            Some(if *signed { spirv::Op::ConvertFToS } else { spirv::Op::ConvertFToU })
        }
        _ => {
            return Err(format!(
                "cast {:?} → {:?} is not a kernel scalar conversion — bool mixes \
                 materialize explicitly (0/1 arithmetic)",
                src, dst
            ))
        }
    })
}

// ── State (SSBO) ────────────────────────────────────────────────────
    /// Declare the StorageBuffer struct over collected fields (sorted by
    /// name — determinism rule) and create its variable. Called BEFORE any
    /// body statement lowers.
    pub fn setup_state_buffer(&mut self) -> Result<(), String> {
        if self.state_fields.is_empty() {
            return Ok(());
        }
        self.state_fields.sort_by(|a, b| a.name.cmp(&b.name));
        let field_types: Vec<Type> =
            self.state_fields.iter().map(|f| f.ty.clone()).collect();
        let mut members = Vec::with_capacity(field_types.len());
        for ty in &field_types {
            members.push(self.type_id(ty)?);
        }
        let member_ids: Vec<Word> = members.clone();
        let struct_ty = self.builder.builder.type_struct(member_ids);
        // Block decoration (required for SSBO interface).
                self.builder
            .decorate_raw(struct_ty, spirv::Decoration::Block, vec![]);
        // Explicit member offsets — Block structs must be fully laid out.
        let mut offset: u32 = 0;
        for (idx, f) in self.state_fields.iter().enumerate() {
            self.builder.builder.member_decorate(
                struct_ty,
                idx as u32,
                spirv::Decoration::Offset,
                [rspirv::dr::Operand::LiteralBit32(offset)],
            );
            // 2026-08-31: member sizes use the element's REAL storage width —
            // arrays are count × elem (matching the ArrayStride layout and the
            // runtime's pack sizes), scalars are their own width. The old
            // fixed-8 sizing mis-slotted every Float32 element.
            let member_bytes = match &f.ty {
                Type::Vector(inner, dims) => {
                    let elems: u32 = dims
                        .iter()
                        .map(|d| match d {
                            crate::ast::Dimension::Anonymous(n) => *n as u32,
                            crate::ast::Dimension::Named(_, n) => *n as u32,
                        })
                        .product::<u32>()
                        .max(1);
                    self.builder.scalar_storage_bytes(inner)? * elems
                }
                other => self.builder.scalar_storage_bytes(other)?,
            };
            offset += member_bytes;
        }
        let struct_ptr = self.builder.ptr_class(StorageClass::StorageBuffer, struct_ty);
        let var = self.builder.gen_id();
        self.builder.emit_global(Instruction::new(
            spirv::Op::Variable,
            Some(struct_ptr),
            Some(var),
            vec![Operand::StorageClass(StorageClass::StorageBuffer)],
        ));
        self.builder.decorate_raw(
            var,
            spirv::Decoration::DescriptorSet,
            vec![rspirv::dr::Operand::LiteralBit32(0)],
        );
        self.builder.decorate_raw(
            var,
            spirv::Decoration::Binding,
            vec![rspirv::dr::Operand::LiteralBit32(0)],
        );
        self.ssbo_var = Some(var);
        Ok(())
    }

    /// AccessChain to `field[idx]` inside the SSBO. Returns (elem ptr, elem ty).
    fn state_field_elem_ptr(&mut self, field: &str, idx: Word) -> Result<(Word, Type), String> {
        let Some(var) = self.ssbo_var else {
            return self.err("kernel touches state but no state fields were collected");
        };
        let Some(pos) = self.state_fields.iter().position(|f| f.name == field) else {
            return self.err(format!("state field '{}' was not declared", field));
        };
        let fty = self.state_fields[pos].ty.clone();
        let elem_ty = match &fty {
            Type::Vector(inner, _) => (**inner).clone(),
            other => other.clone(),
        };
        let elem_id = self.type_id(&elem_ty)?;
        // Chain: ssbo var → member index → element index.
        let member_idx = self.builder.u32_const(pos as u32);
        let ptr_ty = self.builder.ptr_class(StorageClass::StorageBuffer, elem_id);
        let chain = self.builder.gen_id();
        self.builder.emit(Instruction::new(
            spirv::Op::AccessChain,
            Some(ptr_ty),
            Some(chain),
            vec![
                Operand::IdRef(var),
                Operand::IdRef(member_idx),
                Operand::IdRef(idx),
            ],
        ));
        Ok((chain, elem_ty))
    }

    /// 2026-08-26 (§2.3): AccessChain to a SCALAR field inside the SSBO
    /// (member index only — no element subscript). Returns (ptr, field ty).
    fn state_field_scalar_ptr(&mut self, field: &str) -> Result<(Word, Type), String> {
        let Some(var) = self.ssbo_var else {
            return self.err("kernel touches state but no state fields were collected");
        };
        let Some(pos) = self.state_fields.iter().position(|f| f.name == field) else {
            return self.err(format!("state field '{}' was not declared", field));
        };
        let fty = self.state_fields[pos].ty.clone();
        if matches!(fty, Type::Vector(_, _)) {
            return self.err(format!(
                "field '{}' is indexed state — address it as '{}[i]'",
                field, field
            ));
        }
        let elem_id = self.type_id(&fty)?;
        let member_idx = self.builder.u32_const(pos as u32);
        let ptr_ty = self.builder.ptr_class(StorageClass::StorageBuffer, elem_id);
        let chain = self.builder.gen_id();
        self.builder.emit(Instruction::new(
            spirv::Op::AccessChain,
            Some(ptr_ty),
            Some(chain),
            vec![Operand::IdRef(var), Operand::IdRef(member_idx)],
        ));
        Ok((chain, fty))
    }

    /// 2026-08-26 (§2.3): lower an ADDRESS EXPRESSION to an SSBO pointer.
    ///
    /// Valid kernel address forms mirror LLVM's raw-address intent while
    /// staying inside Vulkan's memory model (every address derives from a
    /// buffer base; no numeric addresses):
    /// - `field`      → scalar member pointer (scalar state fields only)
    /// - `field[i]`   → element pointer into indexed (Vector) state
    ///
    /// Anything else is a capability error naming the valid forms.
    fn emit_addr(&mut self, e: &Expr) -> Result<(Word, Type), String> {
        match e {
            Expr::Identifier(name) => self.state_field_scalar_ptr(name),
            Expr::Index(obj, idx) => {
                let Some(fname) = FnLowerer::field_name_of(obj) else {
                    return self.err("only direct state-field indexing forms addresses");
                };
                if !self.state_fields.iter().any(|f| f.name == fname) {
                    return self.err(format!(
                        "unknown state field '{}' (declare it as indexed state)",
                        fname
                    ));
                }
                if !matches!(self.state_fields.iter().find(|f| f.name == fname).unwrap().ty,
                             Type::Vector(_, _))
                {
                    return self.err(format!(
                        "field '{}' is scalar — no '{}'[i]' addressing; use the \
                         field directly",
                        fname, fname
                    ));
                }
                let (idx_val, _) = self.emit_expr(idx)?;
                self.state_field_elem_ptr(fname, idx_val)
            }
            other => self.err(format!(
                "not an address expression ({:?}) — kernel Load#/Store# take \
                 'field' or 'field[i]' rooted in program state",
                std::mem::discriminant(other)
            )),
        }
    }

    /// Byte-count argument check: LLVM Load#/Store# accept a byte width;
    /// over a TYPED buffer the width comes from the declaration. A matching
    /// count passes (source compatibility); anything else names the fix.
    fn check_width_arg(&mut self, arg: Option<&Expr>, elem_ty: &Type, who: &str) -> Result<(), String> {
        let Some(Expr::Decimal(n)) = arg else {
            return Ok(()); // omitted — natural width
        };
        let want = self.builder.scalar_storage_bytes(elem_ty)? as i64;
        if *n != want {
            return self.err(format!(
                "{} byte-width {} does not match the addressed element \
                 ({} bytes by its declared type) — drop the width argument or \
                 fix the field type",
                who, n, want
            ));
        }
        Ok(())
    }

    // ── Small helpers ───────────────────────────────────────────────────

    fn type_id(&mut self, ty: &Type) -> Result<Word, String> {
        self.builder.lower_type(ty)
    }

    fn ptr_to(&mut self, ty: &Type) -> Result<Word, String> {
        let t = self.type_id(ty)?;
        Ok(self.builder.ptr_class(StorageClass::Function, t))
    }

    fn global_invocation_id(&mut self) -> Result<Word, String> {
        if let Some(v) = self.global_id_var {
            return Ok(v);
        }
        let v = self.builtin_input(spirv::BuiltIn::GlobalInvocationId)?;
        self.global_id_var = Some(v);
        Ok(v)
    }

    fn local_invocation_id(&mut self) -> Result<Word, String> {
        if let Some(v) = self.local_id_var {
            return Ok(v);
        }
        let v = self.builtin_input(spirv::BuiltIn::LocalInvocationId)?;
        self.local_id_var = Some(v);
        Ok(v)
    }

    fn builtin_input(&mut self, builtin: spirv::BuiltIn) -> Result<Word, String> {
        // Type: #3 x u32 (vec3<uint>) in Input storage.
        let u32_ty = self.builder.u32_type();
        let vec3 = self.builder.builder.type_vector(u32_ty, 3);
        let ptr = self.builder.ptr_class(StorageClass::Input, vec3);
        let var = self.builder.gen_id();
        self.builder.emit_global(Instruction::new(
            spirv::Op::Variable,
            Some(ptr),
            Some(var),
            vec![Operand::StorageClass(StorageClass::Input)],
        ));
        self.builder.emit_global(Instruction::new(
            spirv::Op::Decorate,
            None,
            None,
            vec![
                Operand::IdRef(var),
                Operand::Decoration(spirv::Decoration::BuiltIn),
                Operand::BuiltIn(builtin),
            ],
        ));
        Ok(var)
    }

    /// Field name of `Ident` or `Field(_, name)` expressions (lhs forms).
    pub fn field_name_of(e: &Expr) -> Option<&str> {
        match e {
            Expr::Identifier(n) => Some(n.as_str()),
            Expr::Field(_, n) => Some(n.as_str()),
            _ => None,
        }
    }
}

/// Free-function twin used by kernel.rs (avoids importing the impl path).
pub fn field_name_of(e: &Expr) -> Option<&str> {
    FnLowerer::field_name_of(e)
}

// Re-export for kernel.rs collection pass.
pub use __collect::collect_state_fields;

mod __collect {
    use super::*;

    /// Walk program top-levels collecting EVERY declared state field used by
    /// the kernel surface (2026-08-26 §2.3: scalars join indexed arrays so
    /// `Load#(scalar)` / `Store#(scalar, v)` have real storage). Sorted +
    /// deduped by setup_state_buffer.
    pub fn collect_state_fields(items: &[crate::ast::TopLevel]) -> Vec<StateField> {
        // 2026-08-31 (plan abv-gpu-by-default): module consts for resolving
        // NAMED array dimensions (`Float[MAXB]` — the AST stores the name
        // with a 0 count; a 0-length OpTypeArray is invalid SPIR-V).
        let consts: HashMap<String, usize> = items
            .iter()
            .filter_map(|i| match i {
                crate::ast::TopLevel::Constant(c) => match &c.expr {
                    Expr::Decimal(n) if *n >= 0 => Some((c.name.clone(), *n as usize)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let resolve_dims = |dims: &[crate::ast::Dimension]| -> Vec<crate::ast::Dimension> {
            dims.iter()
                .map(|d| match d {
                    crate::ast::Dimension::Named(name, n) if *n == 0 => {
                        crate::ast::Dimension::Named(name.clone(), *consts.get(name).unwrap_or(&0))
                    }
                    other => other.clone(),
                })
                .collect()
        };
        let mut fields: Vec<StateField> = Vec::new();
        for item in items {
            match item {
                crate::ast::TopLevel::StateDecl(d) => {
                    let ty = match &d.ty {
                        Type::Vector(inner, dims) => {
                            Type::Vector(inner.clone(), resolve_dims(dims))
                        }
                        other => other.clone(),
                    };
                    fields.push(StateField { name: d.name.clone(), ty });
                }
                // 2026-08-31 (plan abv-gpu-by-default): the real parser emits
                // top-level `let` as Statement(Let) — the same dual form the
                // accel analysis (ProgramInfo::build) and the typechecker
                // consume. Without this arm every pipeline-compiled .abv
                // failed with "no state fields were collected" while
                // hand-built StateDecl fixtures passed. Untyped lets have no
                // kernel storage type and are skipped (referencing one errors
                // naming the field).
                crate::ast::TopLevel::Statement(stmt) => {
                    if let crate::ast::Statement::Let { name, ty: Some(ty), .. } = stmt.as_ref() {
                        let ty = match ty {
                            Type::Vector(inner, dims) => {
                                Type::Vector(inner.clone(), resolve_dims(dims))
                            }
                            other => other.clone(),
                        };
                        fields.push(StateField { name: name.clone(), ty });
                    }
                }
                _ => {}
            }
        }
        fields.sort_by(|a, b| a.name.cmp(&b.name));
        fields.dedup_by(|a, b| a.name == b.name);
        fields
    }
}

/// 2026-08-23: pre-scan a kernel body for typed `let` bindings so their
/// OpVariables can live in the ENTRY block (SPIR-V layout rule). Nested
/// statement lists (guarded bodies) are included.
pub fn collect_locals(body: &[Statement], out: &mut Vec<(String, Type)>) {
    for stmt in body {
        match stmt {
            Statement::Let { name, ty: Some(ty), .. } => {
                out.push((name.clone(), ty.clone()));
            }
            Statement::Guarded(_, inner) | Statement::Block(inner) => {
                collect_locals(inner, out);
            }
            // 2026-08-31 (VITRIOL GEMM comparison M1): the foreach loop
            // variable is a Function-scope long - declared in the entry block
            // like every other local (SPIR-V layout rule); body lets are
            // collected by the recursive call.
            Statement::Foreach { item, body: inner, .. } => {
                out.push((item.clone(), Type::int()));
                collect_locals(inner, out);
            }
            _ => {}
        }
    }
}
