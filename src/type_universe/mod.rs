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

use crate::ast::Type;
use crate::ast::top::MeldDeclaration;
use std::collections::HashMap;

/// Resolved metadata for a single type in the universe.
/// llvm_type is NOT stored here — it is derived at query time from
/// (primitive, bytes) via config/llvm-primitives.toml.
#[derive(Debug, Clone)]
pub struct ResolvedType {
    pub name: String,
    pub base: String,
    pub bytes: u64,
    pub alignment: u64,
    pub properties: HashMap<String, crate::ast::PropertyValue>,
}

impl ResolvedType {
    /// Read the `primitive` metadata property, if set.
    pub fn primitive(&self) -> Option<&str> {
        self.properties.get("primitive").and_then(|pv| {
            if let crate::ast::PropertyValue::Identifier(name) = pv {
                Some(name.as_str())
            } else {
                None
            }
        })
    }
}

/// Central type definition registry.
/// Built during the type-checking pass from all `TopLevel::TypeDef` items.
/// Also holds meld declarations for cross-type field derivations.
#[derive(Debug, Clone)]
pub struct TypeUniverse {
    pub types: HashMap<String, ResolvedType>,
    /// Melds keyed by (type_a, type_b). Both orderings are stored.
    pub melds: HashMap<(String, String), MeldDeclaration>,
}

impl TypeUniverse {
    pub fn new() -> Self {
        TypeUniverse {
            types: HashMap::new(),
            melds: HashMap::new(),
        }
    }

    /// Look up a meld between two types (checks both orderings).
    pub fn find_meld(&self, a: &str, b: &str) -> Option<&MeldDeclaration> {
        self.melds.get(&(a.to_string(), b.to_string()))
            .or_else(|| self.melds.get(&(b.to_string(), a.to_string())))
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
    pub fn get_formatting(&self, ty: &Type) -> crate::ast::Formatting {
        let name = match ty {
            Type::Custom(name) => name,
            _ => return crate::ast::Formatting::None,
        };
        self.types
            .get(name)
            .and_then(|rt| rt.properties.get("formatting"))
            .and_then(|pv| {
                if let crate::ast::PropertyValue::Identifier(s) = pv {
                    crate::ast::Formatting::from_name(s)
                } else {
                    None
                }
            })
            .unwrap_or(crate::ast::Formatting::None)
    }
}
