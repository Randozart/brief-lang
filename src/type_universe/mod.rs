// ── Type Universe — Central Type Registry ──────────────────────────────
// 2026-07-12: Phase 2.0 — Type definition registry.
// All types are resolved to Bits(N) with metadata overlays.
// The TypeUniverse is built during the type-checking pass.

mod operators;
mod resolve;
mod validate;

pub use operators::*;
pub use resolve::*;
pub use validate::*;

use crate::ast_new::Type;
use std::collections::HashMap;

/// Resolved metadata for a single type in the universe.
#[derive(Debug, Clone)]
pub struct ResolvedType {
    pub name: String,
    pub base: String,
    pub bytes: u64,
    pub alignment: u64,
    pub llvm_type: String,
    pub properties: HashMap<String, crate::ast_new::PropertyValue>,
}

/// Central type definition registry.
/// Built during the type-checking pass from all `TopLevel::TypeDef` items.
#[derive(Debug, Clone)]
pub struct TypeUniverse {
    pub types: HashMap<String, ResolvedType>,
}

impl TypeUniverse {
    pub fn new() -> Self {
        TypeUniverse {
            types: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedType> {
        self.types.get(name)
    }

    pub fn register(&mut self, resolved: ResolvedType) {
        self.types.insert(resolved.name.clone(), resolved);
    }

    pub fn contains(&self, name: &str) -> bool {
        self.types.contains_key(name)
    }

    /// Look up the `Formatting` property for a type.
    pub fn get_formatting(&self, ty: &Type) -> crate::ast_new::Formatting {
        let name = match ty {
            Type::Custom(name) => name,
            _ => return crate::ast_new::Formatting::None,
        };
        self.types
            .get(name)
            .and_then(|rt| rt.properties.get("formatting"))
            .and_then(|pv| {
                if let crate::ast_new::PropertyValue::Identifier(s) = pv {
                    crate::ast_new::Formatting::from_name(s)
                } else {
                    None
                }
            })
            .unwrap_or(crate::ast_new::Formatting::None)
    }
}
