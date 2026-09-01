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

/// O3 (plan 2026-08-31-o3-float4-loads.md): a state array retyped as an
/// array of 4-wide vectors. `array` is the member type id; `vector` the
/// element vector type id; `elem` the scalar element type.
#[derive(Clone)]
pub struct Vec4Field {
    pub array: Word,
    pub vector: Word,
    pub elem: Type,
    /// Stage-2 scope: only float groups fuse into Fma.
    pub elem_float: bool,
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
    /// 2026-08-31 (O1): literal const VALUES for unrolling decisions.
    pub const_int_values: HashMap<String, i64>,
    /// 2026-08-31 (O1): loop variables bound to CONSTANTS by unrolling -
    /// reads return the constant id directly (no OpLoad; a load from a
    /// constant id is not a logical pointer).
    pub const_vars: HashMap<String, (Word, Type)>,
    /// Set when the body executed a term/endprogram — callers stop
    /// branching afterwards (a block can only have one terminator).
    pub terminated: bool,
    /// O3 (plan 2026-08-31-o3-float4-loads.md): fields whose SSBO member is
    /// declared as an array of 4-wide vectors (byte-identical layout). Every
    /// scalar access goes through AccessChain(idx >> 2, idx & 3); aligned
    /// groups in the unrolled prefix emit wide loads.
    pub vec4_fields: HashMap<String, Vec4Field>,
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
            const_int_values: HashMap::new(),
            const_vars: HashMap::new(),
            terminated: false,
            vec4_fields: HashMap::new(),
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
                    // O1 unrolling reads the VALUE, not just the constant id.
                    self.const_int_values.insert(c.name.clone(), *n);
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
                let Some((var, _)) = self.vars.get(item) else {
                    return self.err(format!("foreach item '{}' was not pre-declared", item));
                };
                let var = *var;
                let int_ty = self.type_id(&Type::int())?;
                let bool_ty = self.type_id(&Type::Bits(1))?;
                let cmp_op = if *inclusive { spirv::Op::SLessThanEqual } else { spirv::Op::SLessThan };

                // O1 (plan VITRIOL GEMM comparison): constant-trip-count
                // unrolling. `0..K` with K a literal const emits all trip
                // iterations inlined (loop-var reads bound to per-iteration
                // constants) - no loop instructions at all.
                let start_n = match start.as_ref() {
                    Expr::Decimal(n) if *n >= 0 => Some(*n as u64),
                    _ => None,
                };
                let end_n = match end.as_ref() {
                    Expr::Decimal(n) if *n >= 0 => Some(*n as u64),
                    Expr::Identifier(f) => {
                        self.const_int_values.get(f).copied().map(|v| v as u64)
                    }
                    _ => None,
                };

                if let (Some(s0), Some(e0)) = (start_n, end_n) {
                    let factor = crate::config_tuning::ir_lowering().spirv_unroll;
                    let body_cost = body.len().max(1) as u32;
                    // Code-size budget: at most `budget` inlined body copies
                    // total; a large trip keeps a structured loop for the
                    // rest (K=4096 fully inlined produced megabyte modules
                    // and second-long compiles).
                    let budget: u32 = 64;
                    let factor = if body_cost > 0 { factor.clamp(1, budget / body_cost).max(1) } else { factor };
                    let trip = e0.saturating_sub(s0);
                    let unrolled = trip.min(factor as u64);
                    let mut k = s0;

                    // Unrolled prefix: `unrolled` inlined copies. O3: when
                    // the next four iterations form an aligned group with a
                    // vec4-typed field, ONE wide load covers k..k+3.
                    let group_match = match_vec4_fma(body, &item, &self.vec4_fields);
                    while k < s0 + unrolled {
                        if k % 4 == 0
                            && s0 + unrolled - k >= 4
                            && group_match.is_some()
                        {
                            let g = group_match.as_ref().unwrap();
                            self.emit_vec4_fma_group(g, &item, Some(k), k, var)?;
                            k += 4;
                            continue;
                        }
                        let ck = self.builder.builder.constant_bit64(int_ty, k);
                        self.const_vars.insert(item.clone(), (ck, Type::int()));
                        for s in body {
                            if self.terminated {
                                break;
                            }
                            self.emit_stmt(s)?;
                        }
                        k += 1;
                    }

                    // O3 VECTOR REMAINDER LOOP (plan o3-float4-loads.md):
                    // the unrolled prefix is tiny relative to K — the real
                    // win is a step-4 loop whose body emits one wide load
                    // per vec4 field. Runs when the body matched the
                    // mul-add group pattern; a scalar tail loop (below)
                    // covers the last <4 iterations.
                    if let Some(g) = &group_match {
                        let group_end = k + ((e0 - k) / 4) * 4;
                        if group_end > k {
                            let step_ty = self.type_id(&Type::int())?;
                            let header = self.builder.gen_id();
                            let body_bb = self.builder.gen_id();
                            let continue_bb = self.builder.gen_id();
                            let merge = self.builder.gen_id();
                            let preheader_bb = self.builder.gen_id();
                            let cond0 = self.builder.gen_id();
                            let cond_next = self.builder.gen_id();

                            let start_c =
                                self.builder.builder.constant_bit64(int_ty, k);
                            let end_c =
                                self.builder.builder.constant_bit64(int_ty, group_end);
                            self.builder.store(var, start_c);
                            let emit_cond = |lower: &mut Self,
                                             cond: u32|
                             -> Result<(), String> {
                                let v = lower.builder.gen_id();
                                lower.builder.emit(Instruction::new(
                                    spirv::Op::Load,
                                    Some(int_ty),
                                    Some(v),
                                    vec![Operand::IdRef(var)],
                                ));
                                lower.builder.emit(Instruction::new(
                                    cmp_op,
                                    Some(bool_ty),
                                    Some(cond),
                                    vec![Operand::IdRef(v), Operand::IdRef(end_c)],
                                ));
                                Ok(())
                            };
                            self.builder.builder.branch(preheader_bb);
                            self.builder.begin_block(Some(preheader_bb));
                            emit_cond(self, cond0)?;
                            self.builder.builder.branch(header);
                            self.builder.begin_block(Some(header));
                            let cond_hdr = self
                                .builder
                                .builder
                                .phi(
                                    bool_ty,
                                    None,
                                    [(cond0, preheader_bb), (cond_next, continue_bb)],
                                )
                                .map_err(|e| format!("loop phi: {:?}", e))?;
                            self.builder.builder.loop_merge(
                                merge,
                                continue_bb,
                                rspirv::spirv::LoopControl::NONE,
                                [] as [rspirv::dr::Operand; 0],
                            );
                            self.builder
                                .builder
                                .branch_conditional(cond_hdr, body_bb, merge, [] as [u32; 0]);
                            self.builder.begin_block(Some(body_bb));
                            self.vars.insert(item.clone(), (var, Type::int()));
                            let prev_terminated = self.terminated;
                            self.terminated = false;
                            self.emit_vec4_fma_group(g, &item, None, k, var)?;
                            self.builder.builder.branch(continue_bb);
                            self.builder.begin_block(Some(continue_bb));
                            let cur = self.builder.gen_id();
                            self.builder.emit(Instruction::new(
                                spirv::Op::Load,
                                Some(step_ty),
                                Some(cur),
                                vec![Operand::IdRef(var)],
                            ));
                            let four = self.builder.builder.constant_bit64(step_ty, 4);
                            let next = self.builder.gen_id();
                            self.builder.emit(Instruction::new(
                                spirv::Op::IAdd,
                                Some(step_ty),
                                Some(next),
                                vec![Operand::IdRef(cur), Operand::IdRef(four)],
                            ));
                            self.builder.emit(Instruction::new(
                                spirv::Op::Store,
                                None,
                                None,
                                vec![Operand::IdRef(var), Operand::IdRef(next)],
                            ));
                            emit_cond(self, cond_next)?;
                            self.builder.builder.branch(header);
                            self.builder.begin_block(Some(merge));
                            self.terminated = prev_terminated;
                            k = group_end;
                        }
                    }

                    // Structured remainder loop: k in [k, e0).
                    let step_ty = self.type_id(&Type::int())?;
                    if k < e0 {
                        let header = self.builder.gen_id();
                        let body_bb = self.builder.gen_id();
                        let continue_bb = self.builder.gen_id();
                        let merge = self.builder.gen_id();
                        let preheader_bb = self.builder.gen_id();
                        let cond0 = self.builder.gen_id();
                        let cond_next = self.builder.gen_id();

                        let start_c = self.builder.builder.constant_bit64(int_ty, k);
                        let end_c = self.builder.builder.constant_bit64(int_ty, e0);
                        self.builder.store(var, start_c);
                        let emit_cond = |lower: &mut Self, cond: u32| -> Result<(), String> {
                            let v = lower.builder.gen_id();
                            lower.builder.emit(Instruction::new(
                                spirv::Op::Load,
                                Some(int_ty),
                                Some(v),
                                vec![Operand::IdRef(var)],
                            ));
                            lower.builder.emit(Instruction::new(
                                cmp_op,
                                Some(bool_ty),
                                Some(cond),
                                vec![Operand::IdRef(v), Operand::IdRef(end_c)],
                            ));
                            Ok(())
                        };
                        self.builder.builder.branch(preheader_bb);
                        self.builder.begin_block(Some(preheader_bb));
                        emit_cond(self, cond0)?;
                        self.builder.builder.branch(header);
                        self.builder.begin_block(Some(header));
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
                        self.builder.begin_block(Some(body_bb));
                        self.const_vars.remove(item);
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
                        self.builder.begin_block(Some(continue_bb));
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
                        self.builder.begin_block(Some(merge));
                        self.terminated = prev_terminated;
                    }
                    return Ok(());
                }

                // General path: runtime bounds - structured loop.
                let (start_v, _sty) = self.emit_expr(start)?;
                self.builder.store(var, start_v);

                let header = self.builder.gen_id();
                let body_bb = self.builder.gen_id();
                let continue_bb = self.builder.gen_id();
                let merge = self.builder.gen_id();
                let preheader_bb = self.builder.gen_id();
                let cond0 = self.builder.gen_id();
                let cond_next = self.builder.gen_id();

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

                self.builder.builder.branch(preheader_bb);
                self.builder.begin_block(Some(preheader_bb));
                emit_cond(self, cond0)?;
                self.builder.builder.branch(header);
                self.builder.begin_block(Some(header));
                // OpPhi opens the header; OpLoopMerge must immediately
                // precede the OpBranchConditional after it.
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
                // 2026-08-31 (O1 unrolling): unrolled loop variables are
                // CONSTANTS — checked before the variable shadow, so an
                // unrolled body reads the per-iteration constant instead of
                // the stale (never-stored) loop variable.
                if let Some((id, ty)) = self.const_vars.get(name) {
                    return Ok((*id, ty.clone()));
                }
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

    /// O2: lower float `mul + addend` (either order) as a single Fma.
    /// Returns Ok(None) when the shape is not a float mul-add; the caller
    /// then lowers normally (the speculative ids here are dead code).
    fn try_emit_float_fma(&mut self, l: &Expr, r: &Expr)
        -> Result<Option<(Word, Type)>, String>
    {
        use crate::ast::BinaryOpKind::Mul;
        let (mul_operands, addend) = match (l, r) {
            (Expr::BinaryOp(Mul, a, b), c) => ((a.as_ref(), b.as_ref()), c),
            (c, Expr::BinaryOp(Mul, a, b)) => ((a.as_ref(), b.as_ref()), c),
            _ => return Ok(None),
        };
        let (aid, aty) = self.emit_expr(mul_operands.0)?;
        let (bid, bty) = self.emit_expr(mul_operands.1)?;
        if !(self.builder.is_float_type(&aty)? && self.builder.is_float_type(&bty)?) {
            return Ok(None);
        }
        let (cid, cty) = self.emit_expr(addend)?;
        if !self.builder.is_float_type(&cty)? {
            // Mixed float/int add — the generic path owns this error.
            return Ok(None);
        }
        let aid = self.coerce(aid, &aty, &bty)?;
        let bid = self.coerce(bid, &bty, &aty)?;
        let cid = self.coerce(cid, &cty, &aty)?;
        let ty_id = self.type_id(&aty)?;
        let res = self.builder.glsl_fma(ty_id, aid, bid, cid);
        Ok(Some((res, aty)))
    }

    fn emit_binop(&mut self, kind: &crate::ast::BinaryOpKind, l: &Expr, r: &Expr)
        -> Result<(Word, Type), String>
    {
        use crate::ast::BinaryOpKind::*;
        // O2 (plan 2026-08-31-gpu-next): fuse float `a*b + c` into one
        // GLSL.std.450 Fma BEFORE the generic lowering splits it into FMul +
        // FAdd. One rounding (better numerics) and no dependence on driver
        // contraction. Non-matching / non-float shapes fall through
        // untouched; the speculative sub-lowering leaves only dead ids
        // behind (side-effect-free, eliminated by the driver).
        if matches!(kind, Add) {
            if let Some(fused) = self.try_emit_float_fma(l, r)? {
                return Ok(fused);
            }
        }
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
    /// O3 helpers: member byte size (shared by the offset walk) and the
    /// vec4 eligibility / member-type construction.
    fn field_storage_bytes(builder: &mut SpirvBuilder, ty: &Type) -> Result<u32, String> {
        match ty {
            Type::Vector(inner, dims) => {
                let elems: u32 = dims
                    .iter()
                    .map(|d| match d {
                        crate::ast::Dimension::Anonymous(n) => *n as u32,
                        crate::ast::Dimension::Named(_, n) => *n as u32,
                    })
                    .product::<u32>()
                    .max(1);
                Ok(builder.scalar_storage_bytes(inner)? * elems)
            }
            other => builder.scalar_storage_bytes(other),
        }
    }

    fn vec4_eligible(builder: &mut SpirvBuilder, ty: &Type, offset: u32) -> Result<bool, String> {
        if offset % 16 != 0 {
            return Ok(false);
        }
        let Type::Vector(inner, dims) = ty else {
            return Ok(false);
        };
        if builder.scalar_storage_bytes(inner)? != 4 {
            return Ok(false);
        }
        let elems: u32 = dims
            .iter()
            .map(|d| match d {
                crate::ast::Dimension::Anonymous(n) => *n as u32,
                crate::ast::Dimension::Named(_, n) => *n as u32,
            })
            .product::<u32>()
            .max(1);
        Ok(elems % 4 == 0)
    }

    /// OpTypeArray(vec4, N/4) with ArrayStride 16 — byte-identical to the
    /// scalar array it replaces (count % 4 == 0 is the eligibility gate).
    fn vec4_member_type(builder: &mut SpirvBuilder, ty: &Type) -> Result<Word, String> {
        let Type::Vector(inner, dims) = ty else {
            return Err("vec4_member_type on a non-array".into());
        };
        let elems: u32 = dims
            .iter()
            .map(|d| match d {
                crate::ast::Dimension::Anonymous(n) => *n as u32,
                crate::ast::Dimension::Named(_, n) => *n as u32,
            })
            .product::<u32>()
            .max(1);
        let scalar = builder.lower_type(inner)?;
        Ok(builder.vec4_array_type(scalar, elems))
    }

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
        let mut offset_pre: u32 = 0;
        let mut vec4_ids: Vec<(String, Word, Type)> = Vec::new();
        for f in &self.state_fields {
            let member_bytes = Self::field_storage_bytes(self.builder, &f.ty)?;
            if Self::vec4_eligible(self.builder, &f.ty, offset_pre)? {
                let arr_id = Self::vec4_member_type(self.builder, &f.ty)?;
                vec4_ids.push((f.name.clone(), arr_id, f.ty.clone()));
            }
            offset_pre += member_bytes;
        }
        for (name, arr_id, ty) in &vec4_ids {
            // The VECTOR id is the first operand of the array type's
            // OpTypeArray; the scalar element type comes from the field.
            let vector = match self.builder.module_ref()
                .types_global_values
                .iter()
                .find(|i| i.result_id == Some(*arr_id))
                .and_then(|i| i.operands.first())
            {
                Some(rspirv::dr::Operand::IdRef(v)) => *v,
                _ => return Err("vec4 member type lost its element".into()),
            };
            let Type::Vector(inner, _) = ty else {
                return Err("vec4 field on a non-array".into());
            };
            let elem_float = self.builder.is_float_type(inner)?;
            self.vec4_fields.insert(
                name.clone(),
                Vec4Field {
                    array: *arr_id,
                    vector,
                    elem: (**inner).clone(),
                    elem_float,
                },
            );
        }
        let mut members = Vec::with_capacity(field_types.len());
        for (idx, ty) in field_types.iter().enumerate() {
            // Vec4-typed members use their ARRAY-OF-VEC4 id, not the scalar
            // array id — byte-identical layout, wide loads possible.
            match self.vec4_fields.get(&self.state_fields[idx].name) {
                Some(vf) => members.push(vf.array),
                None => members.push(self.type_id(ty)?),
            }
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
        if self.vec4_fields.contains_key(field) {
            // O3: the member is an array of 4-wide vectors (byte-identical
            // layout). The scalar element lives at [idx >> 2][idx & 3].
            let int_ty = self.type_id(&Type::int())?;
            let two = self.builder.i64_const(2);
            let q = self.builder.gen_id();
            self.builder.emit(Instruction::new(
                spirv::Op::ShiftRightArithmetic,
                Some(int_ty),
                Some(q),
                vec![Operand::IdRef(idx), Operand::IdRef(two)],
            ));
            let three = self.builder.i64_const(3);
            let r = self.builder.gen_id();
            self.builder.emit(Instruction::new(
                spirv::Op::BitwiseAnd,
                Some(int_ty),
                Some(r),
                vec![Operand::IdRef(idx), Operand::IdRef(three)],
            ));
            self.builder.emit(Instruction::new(
                spirv::Op::AccessChain,
                Some(ptr_ty),
                Some(chain),
                vec![
                    Operand::IdRef(var),
                    Operand::IdRef(member_idx),
                    Operand::IdRef(q),
                    Operand::IdRef(r),
                ],
            ));
            return Ok((chain, elem_ty));
        }
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

// ── O3 stage 2 (plan 2026-08-31-o3-float4-loads.md): aligned group loads ──

/// A matched float mul-add group: `acc = acc + f1[base + k] * f2[...]`
/// (either mul order), where `f1` is vec4-typed and its index is affine in
/// the loop var with coefficient exactly 1.
struct Vec4Group {
    acc: String,
    vec_field: String,
    vec_side: Expr,
    scalar_side: Expr,
    elem: Type,
}

fn match_vec4_fma(
    body: &[Statement],
    item: &str,
    vec4_fields: &HashMap<String, Vec4Field>,
) -> Option<Vec4Group> {
    use crate::ast::BinaryOpKind::{Add, Mul};
    if body.len() != 1 {
        return None;
    }
    let Statement::Assign(lhs, rhs) = &body[0] else {
        return None;
    };
    let Expr::Identifier(acc) = lhs else {
        return None;
    };
    let rhs_ref: &Expr = rhs;
    let Expr::BinaryOp(Add, a, b) = rhs_ref else {
        return None;
    };
    let mul = match (a.as_ref(), b.as_ref()) {
        (m @ Expr::BinaryOp(Mul, _, _), _) => m,
        (_, m @ Expr::BinaryOp(Mul, _, _)) => m,
        _ => return None,
    };
    let Expr::BinaryOp(Mul, left, right) = mul else {
        return None;
    };
    let (vec_expr, scalar_expr) = match (left.as_ref(), right.as_ref()) {
        (Expr::Index(of, _), _)
            if vec4_fields.contains_key(field_name_of_index(of).unwrap_or("")) =>
        {
            (left.as_ref(), right.as_ref())
        }
        (_, Expr::Index(of, _))
            if vec4_fields.contains_key(field_name_of_index(of).unwrap_or("")) =>
        {
            (right.as_ref(), left.as_ref())
        }
        _ => return None,
    };
    let Expr::Index(vof, vidx) = vec_expr else {
        return None;
    };
    let vfname = field_name_of_index(vof)?;
    let vf = vec4_fields.get(vfname)?;
    // Affine in the loop var with coefficient exactly 1.
    if expr_var_count(vidx, item) != 1 {
        return None;
    }
    if !vf.elem_float {
        return None; // stage-2 scope: float FMA groups only
    }
    Some(Vec4Group {
        acc: acc.clone(),
        vec_field: vfname.to_string(),
        vec_side: vidx.as_ref().clone(),
        scalar_side: scalar_expr.clone(),
        elem: vf.elem.clone(),
    })
}

impl<'a> FnLowerer<'a> {
    /// Emit ONE 4-iteration group of a matched mul-add. `at` is the group
    /// start (constant, % 4 == 0) for the unrolled prefix; `var_storage`
    /// carries the loop phi for the vector-loop form (base = div4(base
    /// literals) + k). The scalar side rebinds the loop var via const_vars.
    fn emit_vec4_fma_group(
        &mut self,
        g: &Vec4Group,
        item: &str,
        at: Option<u64>,
        loop_start: u64,
        var_storage: Word,
    ) -> Result<(), String> {
        let Some(vf) = self.vec4_fields.get(&g.vec_field).cloned() else {
            return Err("vec4 group lost its field".into());
        };
        let Some(ssbo) = self.ssbo_var else {
            return Err("vec4 group without an SSBO".into());
        };
        let member_pos = self
            .state_fields
            .iter()
            .position(|f| f.name == g.vec_field)
            .ok_or_else(|| format!("vec4 field '{}' lost", g.vec_field))?;
        // Group base index: the vec side with the loop var substituted by
        // the group start / k, divided by 4; for the runtime form, the var
        // coefficient is 1 so the phi value IS the group index offset.
        let base_expr = match at {
            Some(k0) => {
                let substituted = subst_var(&g.vec_side, item, k0 as i64);
                let folded = fold_consts(&substituted, &self.const_int_values);
                div4(&folded).ok_or("vec4 group lost alignment")?
            }
            None => {
                // The loop var counts ELEMENTS (stepping 4). The vec4 group
                // index is (base + kv)/4 = div4(base) + (kv - loop_start)/4,
                // and (kv - loop_start) is a multiple of 4 by construction,
                // so the /4 is a shift by 2.
                let zeroed = subst_var(&g.vec_side, item, 0);
                let folded = fold_consts(&zeroed, &self.const_int_values);
                let b0 = div4(&folded).ok_or("vec4 group lost alignment")?;
                let rel = Expr::BinaryOp(
                    crate::ast::BinaryOpKind::Shr,
                    Box::new(Expr::BinaryOp(
                        crate::ast::BinaryOpKind::Sub,
                        Box::new(Expr::Identifier(item.to_string())),
                        Box::new(Expr::Decimal(loop_start as i64)),
                    )),
                    Box::new(Expr::Decimal(2)),
                );
                Expr::BinaryOp(
                    crate::ast::BinaryOpKind::Add,
                    Box::new(b0),
                    Box::new(rel),
                )
            }
        };
        let (base_id, _bty) = self.emit_expr(&base_expr)?;
        let v4_ptr = self
            .builder
            .ptr_class(StorageClass::StorageBuffer, vf.vector);
        let member = self.builder.u32_const(member_pos as u32);
        let group = self.builder.gen_id();
        self.builder.emit(Instruction::new(
            spirv::Op::AccessChain,
            Some(v4_ptr),
            Some(group),
            vec![
                Operand::IdRef(ssbo),
                Operand::IdRef(member),
                Operand::IdRef(base_id),
            ],
        ));
        let v4_val = self.builder.load(vf.vector, group);
        let elem_ty_id = self.type_id(&vf.elem)?;
        let acc_ptr = self
            .vars
            .get(&g.acc)
            .map(|(p, _)| *p)
            .ok_or_else(|| format!("vec4 accumulator '{}' lost", g.acc))?;
        for j in 0u32..4 {
            let comp = self.builder.gen_id();
            self.builder.emit(Instruction::new(
                spirv::Op::CompositeExtract,
                Some(elem_ty_id),
                Some(comp),
                vec![Operand::IdRef(v4_val), Operand::LiteralBit32(j)],
            ));
            let (other_id, _oty) = match at {
                Some(k0) => {
                    let scalar_expr = subst_var(&g.scalar_side, item, (k0 + j as u64) as i64);
                    self.emit_expr(&scalar_expr)?
                }
                // Runtime form: the loop var holds the group's base element
                // index — component j reads base + j.
                None => {
                    let kv_plus_j = Expr::BinaryOp(
                        crate::ast::BinaryOpKind::Add,
                        Box::new(Expr::Identifier(item.to_string())),
                        Box::new(Expr::Decimal(j as i64)),
                    );
                    let scalar_expr =
                        subst_var_expr(&g.scalar_side, item, &kv_plus_j);
                    self.emit_expr(&scalar_expr)?
                }
            };
            let acc_val = self.builder.load(elem_ty_id, acc_ptr);
            let fused = self
                .builder
                .glsl_fma(elem_ty_id, comp, other_id, acc_val);
            self.builder.store(acc_ptr, fused);
        }
        Ok(())
    }
}

/// Number of additive occurrences of `name` in `e`.
fn expr_var_count(e: &Expr, name: &str) -> usize {
    match e {
        Expr::Identifier(n) if n == name => 1,
        Expr::BinaryOp(_, l, r) => expr_var_count(l, name) + expr_var_count(r, name),
        _ => 0,
    }
}

/// Replace Identifier `name` with an arbitrary replacement expression.
fn subst_var_expr(e: &Expr, name: &str, repl: &Expr) -> Expr {
    match e {
        Expr::Identifier(n) if n == name => repl.clone(),
        Expr::BinaryOp(k, l, r) => Expr::BinaryOp(
            *k,
            Box::new(subst_var_expr(l, name, repl)),
            Box::new(subst_var_expr(r, name, repl)),
        ),
        Expr::Index(o, i) => Expr::Index(
            Box::new(subst_var_expr(o, name, repl)),
            Box::new(subst_var_expr(i, name, repl)),
        ),
        other => other.clone(),
    }
}

/// Replace Identifier `name` with Decimal(value).
fn subst_var(e: &Expr, name: &str, value: i64) -> Expr {
    match e {
        Expr::Identifier(n) if n == name => Expr::Decimal(value),
        Expr::BinaryOp(k, l, r) => Expr::BinaryOp(
            *k,
            Box::new(subst_var(l, name, value)),
            Box::new(subst_var(r, name, value)),
        ),
        // The scalar side is typically `x[k]` — the var hides inside Index.
        Expr::Index(o, i) => Expr::Index(
            Box::new(subst_var(o, name, value)),
            Box::new(subst_var(i, name, value)),
        ),
        other => other.clone(),
    }
}

/// Prove `e` (an index expression with the loop var ALREADY substituted —
/// i.e. the group base) is divisible by 4, returning the expression with
/// every literal divided by 4. Structural rule: Decimals must be 4-divisible;
/// Mul(other, Decimal d) divides d; Add recurses; anything else fails.
fn aligned_div4(
    e: &Expr,
    name: &str,
    at: i64,
    consts: &std::collections::HashMap<String, i64>,
) -> Option<Expr> {
    // `at` is the group start (k0 % 4 == 0) — substituting it before the
    // division keeps every literal 4-divisible.
    let at = subst_var(e, name, at);
    let folded = fold_consts(&at, consts);
    div4(&folded)
}

/// Replace const identifiers with their literal values (the unroll alignment
/// proof needs coefficients as literals; emit-time const resolution happens
/// later).
fn fold_consts(e: &Expr, consts: &std::collections::HashMap<String, i64>) -> Expr {
    match e {
        Expr::Identifier(n) => match consts.get(n) {
            Some(v) => Expr::Decimal(*v),
            None => e.clone(),
        },
        Expr::BinaryOp(k, l, r) => Expr::BinaryOp(
            *k,
            Box::new(fold_consts(l, consts)),
            Box::new(fold_consts(r, consts)),
        ),
        other => other.clone(),
    }
}

fn div4(e: &Expr) -> Option<Expr> {
    match e {
        Expr::Decimal(d) => {
            if d % 4 == 0 {
                Some(Expr::Decimal(d / 4))
            } else {
                None
            }
        }
        Expr::BinaryOp(crate::ast::BinaryOpKind::Add, l, r) => Some(Expr::BinaryOp(
            crate::ast::BinaryOpKind::Add,
            Box::new(div4(l)?),
            Box::new(div4(r)?),
        )),
        Expr::BinaryOp(crate::ast::BinaryOpKind::Mul, l, r) => {
            match (l.as_ref(), r.as_ref()) {
                (Expr::Decimal(d), other) | (other, Expr::Decimal(d)) => {
                    if d % 4 != 0 {
                        return None;
                    }
                    Some(Expr::BinaryOp(
                        crate::ast::BinaryOpKind::Mul,
                        Box::new(other.clone()),
                        Box::new(Expr::Decimal(d / 4)),
                    ))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Field name of an `Identifier` (the object of an Index expression).
fn field_name_of_index(e: &Expr) -> Option<&str> {
    match e {
        Expr::Identifier(n) => Some(n),
        _ => None,
    }
}
