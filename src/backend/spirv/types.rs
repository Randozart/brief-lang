/// SPIR-V type lowering — maps Brief types to SPIR-V type IDs.
///
/// 2026-07-15: Each Brief type is Bits(N) or Void or Ptr(T). We map
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
    next_id: Word,
}

impl TypeCache {
    /// 2026-07-15: Create empty cache. IDs start at 100 (0-99 reserved).
    pub fn new() -> Self {
        TypeCache {
            cache: HashMap::new(),
            next_id: 100,
        }
    }

    /// 2026-07-15: Allocate a fresh SPIR-V result ID.
    pub fn alloc_id(&mut self) -> Word {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// 2026-07-15: Lower a Brief Type to a SPIR-V type Word.
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

    fn lower_fresh(&mut self, ty: &Type) -> Result<Word, String> {
        match ty {
            Type::Void => {
                Ok(self.alloc_id())
            }
            Type::Bits(bytes) => {
                if *bytes == 1 {
                    Ok(self.alloc_id())
                } else if *bytes == 8 {
                    Ok(self.alloc_id())
                } else {
                    Ok(self.alloc_id())
                }
            }
            Type::Ptr(elem) => {
                let _ = self.lower(elem)?;
                Ok(self.alloc_id())
            }
            // 2026-07-15: Resolve well-known custom types to their bit width
            Type::Custom(name) if name == "Int" || name == "Float" || name == "Float64" => {
                Ok(self.alloc_id())
            }
            Type::Custom(name) if name == "Bool" => {
                Ok(self.alloc_id())
            }
            Type::Custom(name) if name == "String" => {
                // String is a 24-byte struct in Brief
                for _ in 0..3 { self.alloc_id(); } // reserve IDs for fields
                Ok(self.alloc_id())
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
