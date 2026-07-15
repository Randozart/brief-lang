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
        SpirvBuilder {
            builder: b,
            types: TypeCache::new(),
        }
    }

    /// 2026-07-15: Finalize module and assemble to SPIR-V binary.
    pub fn build(self) -> Result<Vec<u8>, String> {
        let module = self.builder.module();
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

    /// 2026-07-15: Emit an instruction into the current block.
    pub fn emit(&mut self, inst: rspirv::dr::Instruction) {
        self.builder.insert_into_block(rspirv::dr::InsertPoint::End, inst);
    }

    /// 2026-07-15: Get mutable access to the underlying module for
    /// direct Function manipulation (used by kernel.rs).
    pub fn module_mut(&mut self) -> &mut rspirv::dr::Module {
        self.builder.module_mut()
    }
}
