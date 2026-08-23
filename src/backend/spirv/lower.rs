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

use crate::ast::{Expr, Statement, Type};
use crate::backend::spirv::builder::SpirvBuilder;
use crate::backend::spirv::types::TypeCache;

/// One program-state field referenced by the kernel body. Collected before
/// emission so the SSBO layout is stable regardless of use order.
#[derive(Debug, Clone)]
pub struct StateField {
    pub name: String,
    pub ty: Type,
}

pub struct FnLowerer<'a> {
    pub builder: &'a mut SpirvBuilder,
    /// int / bool type ids (cached once).
    pub int_ty: Word,
    pub bool_ty: Word,
    /// Local variables: name → (Function-storage pointer id, type).
    pub vars: HashMap<String, (Word, Type)>,
    /// State fields exposed through the SSBO: sorted name → (type, member idx).
    pub state_fields: Vec<StateField>,
    /// SSBO variable id (StorageBuffer storage class); set by setup_state_buffer.
    pub ssbo_var: Option<Word>,
    /// BuiltIn GlobalInvocationId input variable (lazily created).
    global_id_var: Option<Word>,
    /// BuiltIn LocalInvocationId input variable (lazily created).
    local_id_var: Option<Word>,
    /// Set when the body executed a term/endprogram — callers stop
    /// branching afterwards (a block can only have one terminator).
    pub terminated: bool,
}

impl<'a> FnLowerer<'a> {
    pub fn new(builder: &'a mut SpirvBuilder, state_fields: Vec<StateField>) -> Self {
        let int_ty = builder.types.lower(&Type::int()).expect("int type");
        let bool_ty = builder
            .types
            .lower(&Type::Bits(1))
            .expect("bool type");
        FnLowerer {
            builder,
            int_ty,
            bool_ty,
            vars: HashMap::new(),
            state_fields,
            ssbo_var: None,
            global_id_var: None,
            local_id_var: None,
            terminated: false,
        }
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
            Statement::Let { name, expr: Some(e), .. } => {
                let (val, ty) = self.emit_expr(e)?;
                let ptr_ty = self.ptr_to(&ty)?;
                let var = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    spirv::Op::Variable,
                    Some(ptr_ty),
                    Some(var),
                    vec![Operand::StorageClass(StorageClass::Function)],
                ));
                self.builder.emit(Instruction::new(
                    spirv::Op::Store,
                    None,
                    None,
                    vec![Operand::IdRef(var), Operand::IdRef(val)],
                ));
                self.vars.insert(name.clone(), (var, ty));
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
                let c = self.int_const(*n);
                Ok((c, Type::int()))
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
                self.err(format!("unknown identifier '{}' in kernel", name))
            }
            Expr::BinaryOp(kind, l, r) => self.emit_binop(kind, l, r),
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
                let u32_ty = self.u32_ty()?;
                let ptr_u32 = self.ptr_type(StorageClass::Input, u32_ty);
                let dim_const = self.const_u32(dim);
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
                // Widen u32 → i64 (values are small; zero-extension matches
                // GLSL uint→int64 semantics for invocation ids).
                let wide = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    spirv::Op::UConvert,
                    Some(self.int_ty),
                    Some(wide),
                    vec![Operand::IdRef(raw)],
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
                let c = self.i64_const(sizes[dim as usize]);
                Ok((c, Type::int()))
            }
            "Load#" | "Store#" => self.err(
                "raw-address Load#/Store# are not meaningful in a Vulkan \
                 kernel without a memory-model decision — use state fields",
            ),
            other => self.err(format!("unsupported intrinsic '{}'", other)),
        }
    }

    fn emit_binop(&mut self, kind: &crate::ast::BinaryOpKind, l: &Expr, r: &Expr)
        -> Result<(Word, Type), String>
    {
        use crate::ast::BinaryOpKind::*;
        let (lid, lty) = self.emit_expr(l)?;
        let (rid, rty) = self.emit_expr(r)?;
        let result_int = self.int_ty;
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
                let res = self.builder.gen_id();
                self.builder.emit(Instruction::new(
                    op,
                    Some(self.bool_ty),
                    Some(res),
                    vec![Operand::IdRef(lid), Operand::IdRef(rid)],
                ));
                return Ok((res, Type::Bits(1)));
            }
            Concat => return self.err("string concat is not a kernel operation"),
        };
        let is_cmp = matches!(kind, Lt | Gt | Le | Ge | Eq | Neq);
        let res_ty = if is_cmp { self.bool_ty } else { result_int };
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

    /// Zero-width mismatches are a lowering bug; identical types pass through.
    fn coerce(&mut self, id: Word, ty: &Type, other: &Type) -> Result<Word, String> {
        if self.type_id(ty)? == self.type_id(other)? {
            Ok(id)
        } else {
            self.err(format!("operand type mismatch {:?} vs {:?}", ty, other))
        }
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
        let struct_ty = self.builder.gen_id();
        self.builder.emit_type(
            spirv::Op::TypeStruct,
            struct_ty,
            members.iter().map(|&m| Operand::IdRef(m)).collect(),
        );
        // Block decoration (required for SSBO interface).
        self.decorate(struct_ty, spirv::Decoration::Block, 0);
        let struct_ptr = self.ptr_type(StorageClass::StorageBuffer, struct_ty);
        let var = self.builder.gen_id();
        self.builder.emit_global(Instruction::new(
            spirv::Op::Variable,
            Some(struct_ptr),
            Some(var),
            vec![Operand::StorageClass(StorageClass::StorageBuffer)],
        ));
        self.decorate(var, spirv::Decoration::DescriptorSet, 0);
        self.decorate(var, spirv::Decoration::Binding, 0);
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
        let member_idx = self.const_u32(pos as u32);
        let ptr_ty = self.ptr_type(StorageClass::StorageBuffer, elem_id);
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

    // ── Small helpers ───────────────────────────────────────────────────

    fn type_id(&mut self, ty: &Type) -> Result<Word, String> {
        self.builder.types.lower(ty)
    }

    fn ptr_to(&mut self, ty: &Type) -> Result<Word, String> {
        let t = self.type_id(ty)?;
        Ok(self.ptr_type(StorageClass::Function, t))
    }

    fn ptr_type(&mut self, class: StorageClass, pointee: Word) -> Word {
        // Duplicate OpTypePointer instructions are legal SPIR-V; per-call
        // ids keep this simple (dedup would need a side map on FnLowerer).
        let id = self.builder.gen_id();
        self.builder.emit_type(
            spirv::Op::TypePointer,
            id,
            vec![Operand::StorageClass(class), Operand::IdRef(pointee)],
        );
        id
    }

    fn decorate(&mut self, target: Word, decoration: spirv::Decoration, literal: u32) {
        self.builder.emit_global(Instruction::new(
            spirv::Op::Decorate,
            None,
            None,
            vec![
                Operand::IdRef(target),
                Operand::Decoration(decoration),
                Operand::LiteralBit32(literal),
            ],
        ));
    }

    fn const_u32(&mut self, v: u32) -> Word {
        let u32_ty = self.u32_ty().unwrap_or_else(|_| self.int_ty);
        let c = self.builder.gen_id();
        self.builder.emit_type(
            spirv::Op::Constant,
            c,
            vec![Operand::IdRef(u32_ty), Operand::LiteralBit32(v)],
        );
        c
    }

    fn i64_const(&mut self, v: u64) -> Word {
        let c = self.builder.gen_id();
        self.builder.emit_type(
            spirv::Op::Constant,
            c,
            vec![Operand::IdRef(self.int_ty), Operand::LiteralBit64(v)],
        );
        c
    }

    pub fn int_const(&mut self, v: i64) -> Word {
        self.i64_const(v as u64)
    }

    fn u32_ty(&mut self) -> Result<Word, String> {
        // 32-bit unsigned int type, created directly (not via Briev Types —
        // builtins are u32 by spec).
        let id = self.builder.gen_id();
        self.builder.emit_type(
            spirv::Op::TypeInt,
            id,
            vec![Operand::LiteralBit32(32), Operand::LiteralBit32(0)],
        );
        Ok(id)
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
        let u32_ty = self.u32_ty()?;
        let vec3 = self.builder.gen_id();
        self.builder.emit_type(
            spirv::Op::TypeVector,
            vec3,
            vec![Operand::IdRef(u32_ty), Operand::LiteralBit32(3)],
        );
        let ptr = self.ptr_type(StorageClass::Input, vec3);
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

    /// Walk a txn body collecting every state-field reference used with an
    /// index (the SSBO surface). Sorted + deduped by setup_state_buffer.
    pub fn collect_state_fields(
        items: &[crate::ast::TopLevel],
        kernel: &str,
    ) -> Vec<StateField> {
        let mut fields: Vec<StateField> = Vec::new();
        for item in items {
            if let crate::ast::TopLevel::StateDecl(d) = item {
                // Only indexed/array state becomes SSBO members.
                if matches!(d.ty, Type::Vector(_, _)) {
                    fields.push(StateField {
                        name: d.name.clone(),
                        ty: d.ty.clone(),
                    });
                }
            }
        }
        let _ = kernel;
        fields.sort_by(|a, b| a.name.cmp(&b.name));
        fields.dedup_by(|a, b| a.name == b.name);
        fields
    }
}

// Silence unused import when Instruction re-export shifts.
#[allow(unused)]
fn _t(_: Instruction) {}
#[allow(unused)]
fn _t2(_: TypeCache) {}
