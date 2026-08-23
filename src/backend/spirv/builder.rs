/// SPIR-V module builder — wraps `rspirv::dr::build::Builder` with type cache.
///
/// 2026-07-15: Sets capabilities, memory model, provides id and type helpers.

use rspirv::dr::Builder;
use rspirv::dr::Operand;
use rspirv::binary::Assemble;
use rspirv::spirv::{self, Word, ExecutionModel};
use crate::backend::spirv::types::TypeCache;

/// 2026-07-15: Combines rspirv Builder with type/id management.
pub struct SpirvBuilder {
    pub builder: Builder,
    pub types: TypeCache,
}

impl SpirvBuilder {
    /// 2026-07-15: Create builder, set Shader + Int64 + Float64 + GLSL450.
    pub fn new() -> Self {
        let mut b = Builder::new();
        b.capability(spirv::Capability::Shader);
        b.capability(spirv::Capability::Int64);
        b.capability(spirv::Capability::Float64);
        b.memory_model(spirv::AddressingModel::Logical, spirv::MemoryModel::GLSL450);
        let mut types = TypeCache::new();
        // 2026-08-23 (id-unification bugfix): the dr::Builder assigns ids
        // from its own counter (starting at 1) while TypeCache started at
        // 100 — the ranges overlapped and modules referenced duplicated ids
        // ("Type Id 10 is not a type", spirv-val). Reserve a disjoint high
        // range for the cache and widen the module bound accordingly.
        types.next_id = 1_000_000;
        let mut sb = SpirvBuilder {
            builder: b,
            types,
        };
        sb
    }

    /// 2026-08-23: move queued cache-type instructions into the module's
    /// types/global-values section. MUST be called before begin_function —
    /// types referenced inside a function have to precede it, and appending
    /// after assembly ordering broke spirv-val ('Id defined more than once'
    /// / reference-before-definition). To undo: restore the drain inside
    /// build().
    pub fn flush_types(&mut self) {
        for inst in std::mem::take(&mut self.types.types_arena) {
            self.builder
                .insert_types_global_values(rspirv::dr::InsertPoint::End, inst);
        }
    }

    /// Raise the module's id bound (call again if high-range ids grow).
    pub fn raise_bound(&mut self, min: u32) {
        if let Some(header) = self.builder.module_mut().header.as_mut() {
            if header.bound < min {
                header.bound = min;
            }
        }
    }

    /// 2026-07-15: Finalize module and assemble to SPIR-V binary.
    /// 2026-07-21: Inserts type instructions from TypeCache.types_arena
    /// into the module's types/global values section before assembly.
    pub fn build(mut self) -> Result<Vec<u8>, String> {
        // Cache-range id floor. NOTE: Builder::module() (below) OVERWRITES
        // header.bound with its own next_id, so the max is applied AFTER it.
        let cache_floor = self.types.next_id + 1;
        // Transfer type instructions from TypeCache into the module
        for inst in &self.types.types_arena {
            eprintln!("[arena] op={:?} rid={:?} ops={} ", inst.class.opcode, inst.result_id, inst.operands.len());
        }
        {
            let m = self.builder.module_ref();
            eprintln!("[pre] tgv={} fns={}", m.types_global_values.len(), m.functions.len());
            for (i, g) in m.types_global_values.iter().enumerate().take(60) {
                eprintln!("[tgv {:02}] {:?} rid={:?}", i, g.class.opcode, g.result_id);
            }
        }
        self.flush_types();
        let mut module = self.builder.module();
        if let Some(header) = module.header.as_mut() {
            header.bound = header.bound.max(cache_floor);
        }
        let words = module.assemble();
        let mut binary = Vec::with_capacity(words.len() * 4);
        for w in &words {
            binary.extend_from_slice(&w.to_le_bytes());
        }
        Ok(binary)
    }

    /// 2026-07-15: Allocate a fresh result ID.
    pub fn gen_id(&mut self) -> Word {
        self.builder.id()
    }

    /// 2026-07-15: Set entry point via builder.
    pub fn set_entry_point(&mut self, func_id: Word, name: &str, execution_model: ExecutionModel) {
        self.builder.entry_point(execution_model, func_id, name, vec![]);
    }

    /// 2026-07-15: Add execution mode via builder.
    pub fn add_execution_mode(&mut self, func_id: Word, mode: spirv::ExecutionMode, x: u32, y: u32, z: u32) {
        self.builder.execution_mode(func_id, mode, &[x, y, z]);
    }

    /// 2026-07-15: Begin a new function.
    pub fn begin_function(&mut self, return_type: Word, func_id: Word, control: spirv::FunctionControl, func_type: Word) {
        self.builder.begin_function(return_type, Some(func_id), control, func_type).unwrap();
    }

    /// 2026-07-15: End current function.
    pub fn end_function(&mut self) {
        self.builder.end_function().unwrap();
    }

    /// 2026-07-15: Begin a new block with optional label ID.
    pub fn begin_block(&mut self, label_id: Option<Word>) {
        self.builder.begin_block(label_id).unwrap();
    }

    /// 2026-07-15: Emit a type instruction (OpType*).
    pub fn emit_type(&mut self, op: spirv::Op, result_id: Word, operands: Vec<Operand>) {
        let inst = rspirv::dr::Instruction::new(op, None, Some(result_id), operands);
        self.builder.insert_types_global_values(rspirv::dr::InsertPoint::End, inst);
    }

    /// 2026-08-23: Emit a MODULE-GLOBAL instruction (OpVariable in
    /// StorageBuffer/Input classes, decorations) — these live outside any
    /// function, so they must not go through emit() (current block).
    pub fn emit_global(&mut self, inst: rspirv::dr::Instruction) {
        self.builder
            .insert_types_global_values(rspirv::dr::InsertPoint::End, inst);
    }

    /// 2026-08-23: Store value into pointer (inside current block).
    pub fn store(&mut self, ptr: Word, val: Word) {
        let inst = rspirv::dr::Instruction::new(
            spirv::Op::Store,
            None,
            None,
            vec![Operand::IdRef(ptr), Operand::IdRef(val)],
        );
        self.builder.insert_into_block(rspirv::dr::InsertPoint::End, inst);
    }

    /// 2026-08-23: Branch to label. Uses rspirv's TYPED terminator so the
    /// builder's selected-block state closes — raw insert_into_block leaves
    /// the block 'open' and the next begin_block panics (NestedBlock).
    /// To undo: revert to raw Instruction emission.
    pub fn branch(&mut self, label: Word) {
        self.builder.branch(label).expect("branch");
    }

    /// 2026-08-23: Typed return (see branch note).
    pub fn ret(&mut self) {
        self.builder.ret().expect("ret");
    }

    /// 2026-08-23: Typed loop merge + conditional branch pair (closes block).
    pub fn loop_header_tail(
        &mut self,
        cond: Word,
        merge: Word,
        continue_target: Word,
        body: Word,
    ) {
        self.builder
            .loop_merge(merge, continue_target, spirv::LoopControl::empty(), [])
            .expect("loop_merge");
        self.builder
            .branch_conditional(cond, body, merge, [])
            .expect("branch_conditional");
    }

    /// 2026-08-23: Load from pointer into fresh id of result type.
    pub fn load(&mut self, result_ty: Word, ptr: Word) -> Word {
        self.instr(spirv::Op::Load, Some(result_ty), None, vec![Operand::IdRef(ptr)])
    }

    /// 2026-08-23: Generic instruction into current block.
    pub fn instr(&mut self, op: spirv::Op, result_ty: Option<Word>, result_id: Option<Word>, operands: Vec<Operand>) -> Word {
        let id = result_id.unwrap_or_else(|| self.gen_id());
        let inst = rspirv::dr::Instruction::new(op, result_ty, Some(id), operands);
        self.builder.insert_into_block(rspirv::dr::InsertPoint::End, inst);
        id
    }

    /// 2026-07-15: Emit an instruction into the current block.
    pub fn emit(&mut self, inst: rspirv::dr::Instruction) {
        self.builder.insert_into_block(rspirv::dr::InsertPoint::End, inst);
    }

    /// 2026-07-15: Get mutable access to the underlying module for
    /// direct Function manipulation (used by kernel.rs).
    pub fn module_mut(&mut self) -> &mut rspirv::dr::Module {
        self.builder.module_mut()
    }

    /// 2026-08-23: read-only module view for tests/inspection.
    pub fn module_ref(&self) -> &rspirv::dr::Module {
        self.builder.module_ref()
    }
}
