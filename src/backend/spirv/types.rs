/// SPIR-V type lowering — maps Briev types to SPIR-V type IDs.
///
/// 2026-07-15: Each Briev type is Bits(N) or Void or Ptr(T). We map
/// to OpTypeVoid, OpTypeBool, OpTypeInt, OpTypePointer, etc.
/// Results are cached by hash to avoid duplicate instructions.

use crate::ast::Type;
use rspirv::dr::Operand;
use rspirv::spirv::{self, Word};
use std::collections::HashMap;

/// 2026-07-15: Caches lowered type IDs. Emits type instructions into a
/// provided `types: &mut Vec<Instruction>` arena.
pub struct TypeCache {
    cache: HashMap<u64, Word>,
    /// 2026-08-23: pub(crate) — SpirvBuilder reserves a disjoint high range
    /// for cache ids so they never collide with dr::Builder ids (id-unification).
    pub(crate) next_id: Word,
    /// 2026-07-21: Accumulated type instructions (OpType*).
    pub types_arena: Vec<rspirv::dr::Instruction>,
}

impl TypeCache {
    /// 2026-07-15: Create empty cache. IDs start at 100 (0-99 reserved).
    pub fn new() -> Self {
        TypeCache {
            cache: HashMap::new(),
            next_id: 100,
            types_arena: Vec::new(),
        }
    }

    /// 2026-07-15: Allocate a fresh SPIR-V result ID.
    pub fn alloc_id(&mut self) -> Word {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// 2026-07-15: Lower a Briev Type to a SPIR-V type Word.
    /// Returns Ok(id) on success.
    pub fn lower(&mut self, ty: &Type) -> Result<Word, String> {
        let key = hash_type(ty);
        if let Some(&id) = self.cache.get(&key) {
            return Ok(id);
        }
        let id = self.lower_fresh(ty)?;
        self.cache.insert(key, id);
        Ok(id)
    }

    /// 2026-07-21: Push an OpType* instruction into the types arena.
    fn push_type(&mut self, op: spirv::Op, result_id: Word, operands: Vec<Operand>) {
        self.types_arena.push(
            rspirv::dr::Instruction::new(op, None, Some(result_id), operands)
        );
    }

    fn lower_fresh(&mut self, ty: &Type) -> Result<Word, String> {
        match ty {
            Type::Void => {
                let id = self.alloc_id();
                self.push_type(spirv::Op::TypeVoid, id, vec![]);
                Ok(id)
            }
            Type::Bits(bytes) => {
                let id = self.alloc_id();
                if *bytes == 1 {
                    self.push_type(spirv::Op::TypeBool, id, vec![]);
                } else if *bytes == 8 {
                    // Bits(8) for i8/u8 — narrower than default i64
                    self.push_type(spirv::Op::TypeInt, id, vec![
                        Operand::LiteralBit32(8),
                        Operand::LiteralBit32(0), // unsigned
                    ]);
                } else {
                    self.push_type(spirv::Op::TypeInt, id, vec![
                        Operand::LiteralBit32(*bytes as u32),
                        Operand::LiteralBit32(0), // unsigned
                    ]);
                }
                Ok(id)
            }
            // 2026-08-23 (§2.1): fixed-size arrays for indexed state.
            // Vulkan requires ArrayStride on arrays inside SSBOs; the stride
            // is the element's byte size (scalars only in the supported
            // surface: i64 → 8).
            Type::Vector(inner, dims) => {
                let inner_id = self.lower(inner)?;
                let elem_bytes = 8; // supported surface: i64 elements
                let mut cur = inner_id;
                let mut stride = elem_bytes;
                // Build innermost-out so each level's stride covers its tail.
                                let mut dim_sizes: Vec<usize> = dims
                    .iter()
                    .map(|d| match d {
                        crate::ast::Dimension::Anonymous(n) => *n,
                        crate::ast::Dimension::Named(_, n) => *n,
                    })
                    .collect();
                dim_sizes.reverse();
                for n in dim_sizes {
                    let u32_id = self.alloc_id();
                    self.push_type(spirv::Op::TypeInt, u32_id, vec![
                        Operand::LiteralBit32(32),
                        Operand::LiteralBit32(0),
                    ]);
                    let len_const = self.alloc_id();
                    self.push_type(spirv::Op::Constant, len_const, vec![
                        Operand::IdRef(u32_id),
                        Operand::LiteralBit32(n as u32),
                    ]);
                    let arr = self.alloc_id();
                    self.push_type(spirv::Op::TypeArray, arr, vec![
                        Operand::IdRef(cur),
                        Operand::IdRef(len_const),
                    ]);
                    self.push_type(spirv::Op::Decorate, 0, vec![
                        Operand::IdRef(arr),
                        Operand::Decoration(spirv::Decoration::ArrayStride),
                        Operand::LiteralBit32(stride as u32),
                    ]);
                    cur = arr;
                    stride *= n;
                }
                Ok(cur)
            }
            Type::Ptr(elem) => {
                let elem_id = self.lower(elem)?;
                let id = self.alloc_id();
                self.push_type(spirv::Op::TypePointer, id, vec![
                    Operand::StorageClass(spirv::StorageClass::Function),
                    Operand::IdRef(elem_id),
                ]);
                Ok(id)
            }
            // 2026-07-15: Resolve well-known custom types to their bit width
            Type::Custom(name) if name == "Int" => {
                let id = self.alloc_id();
                self.push_type(spirv::Op::TypeInt, id, vec![
                    Operand::LiteralBit32(64),
                    Operand::LiteralBit32(0), // unsigned
                ]);
                Ok(id)
            }
            Type::Custom(name) if name == "Float" => {
                let id = self.alloc_id();
                self.push_type(spirv::Op::TypeFloat, id, vec![
                    Operand::LiteralBit32(32),
                ]);
                Ok(id)
            }
            Type::Custom(name) if name == "Float64" => {
                let id = self.alloc_id();
                self.push_type(spirv::Op::TypeFloat, id, vec![
                    Operand::LiteralBit32(64),
                ]);
                Ok(id)
            }
            Type::Custom(name) if name == "Bool" => {
                let id = self.alloc_id();
                self.push_type(spirv::Op::TypeBool, id, vec![]);
                Ok(id)
            }
            Type::Custom(name) if name == "String" => {
                // String is a 24-byte struct in Briev
                let ids: Vec<Word> = (0..3).map(|_| self.alloc_id()).collect();
                for &sub_id in &ids {
                    self.push_type(spirv::Op::TypeInt, sub_id, vec![
                        Operand::LiteralBit32(64),
                        Operand::LiteralBit32(0),
                    ]);
                }
                let id = self.alloc_id();
                let struct_members: Vec<Operand> = ids.iter().map(|&i| Operand::IdRef(i)).collect();
                self.push_type(spirv::Op::TypeStruct, id, struct_members);
                Ok(id)
            }
            _ => Err(format!("SPIR-V: unsupported type {:?}", ty)),
        }
    }
}

/// 2026-07-15: Quick hash for type dedup — not cryptographic.
fn hash_type(ty: &Type) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", ty).hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lower_void() {
        let mut cache = TypeCache::new();
        let id = cache.lower(&Type::void()).unwrap();
        assert!(id >= 100);
    }

    #[test]
    fn test_lower_caches() {
        let mut cache = TypeCache::new();
        let a = cache.lower(&Type::int()).unwrap();
        let b = cache.lower(&Type::int()).unwrap();
        assert_eq!(a, b);
    }
}
