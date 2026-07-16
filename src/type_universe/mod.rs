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
        let mut universe = TypeUniverse {
            types: HashMap::new(),
            melds: HashMap::new(),
        };
        universe.seed_primordial_types();
        universe
    }

    /// 2026-07-16: Seed the universe with primordial type entries so that
    /// `Int`, `Float`, etc. are available without stdlib import. User
    /// `type X <: Bits { ... }` declarations override these via register().
    fn seed_primordial_types(&mut self) {
        // Table: (name, bytes, alignment, primitive, llvm_type)
        const PRIMORDIALS: &[(&str, u64, u64, &str, &str)] = &[
            ("Int",    8, 8, "signed",   "i64"),
            ("UInt",   8, 8, "unsigned", "i64"),
            ("Int8",   1, 1, "signed",   "i8"),
            ("UInt8",  1, 1, "unsigned", "i8"),
            ("Int16",  2, 2, "signed",   "i16"),
            ("UInt16", 2, 2, "unsigned", "i16"),
            ("Int32",  4, 4, "signed",   "i32"),
            ("UInt32", 4, 4, "unsigned", "i32"),
            ("Int64",  8, 8, "signed",   "i64"),
            ("UInt64", 8, 8, "unsigned", "i64"),
            ("Float",  4, 4, "float",    "float"),
            ("Float32",4, 4, "float",    "float"),
            ("Float64",8, 8, "float",    "double"),
            ("Double", 8, 8, "float",    "double"),
            ("Bool",   1, 1, "unsigned", "i8"),
            ("Char",   4, 4, "unsigned", "i32"),
            ("Data",   8, 8, "pointer",  "i8*"),
            ("Void",   0, 0, "void",     "void"),
        ];
        for &(name, bytes, alignment, primitive, llvm_type) in PRIMORDIALS {
            let mut properties = std::collections::HashMap::new();
            properties.insert("primitive".into(), crate::ast::PropertyValue::Identifier(primitive.to_string()));
            properties.insert("llvm_type".into(), crate::ast::PropertyValue::String(llvm_type.to_string()));
            properties.insert("alignment".into(), crate::ast::PropertyValue::Int(alignment as i64));
            self.types.insert(name.to_string(), ResolvedType {
                name: name.to_string(),
                base: "Bits".to_string(),
                bytes,
                alignment,
                properties,
            });
        }
        // String — special case: explicit %String llvm type + field annotations
        {
            let mut p = std::collections::HashMap::new();
            p.insert("primitive".into(), crate::ast::PropertyValue::Identifier("struct".to_string()));
            p.insert("llvm_type".into(), crate::ast::PropertyValue::String("%String".to_string()));
            p.insert("alignment".into(), crate::ast::PropertyValue::Int(8));
            p.insert("field.ptr.offset".into(), crate::ast::PropertyValue::Int(0));
            p.insert("field.ptr.width".into(), crate::ast::PropertyValue::Int(64));
            p.insert("field.len.offset".into(), crate::ast::PropertyValue::Int(64));
            p.insert("field.len.width".into(), crate::ast::PropertyValue::Int(64));
            p.insert("field.codec.offset".into(), crate::ast::PropertyValue::Int(128));
            p.insert("field.codec.width".into(), crate::ast::PropertyValue::Int(8));
            self.types.insert("String".to_string(), ResolvedType {
                name: "String".to_string(),
                base: "Bits".to_string(),
                bytes: 24,
                alignment: 8,
                properties: p,
            });
        }
    }

    /// Look up a meld between two types (checks both orderings).
    pub fn find_meld(&self, a: &str, b: &str) -> Option<&MeldDeclaration> {
        self.melds.get(&(a.to_string(), b.to_string()))
            .or_else(|| self.melds.get(&(b.to_string(), a.to_string())))
    }

    /// 2026-07-16: P2 — Look up "String.c" from base "String" and extension "c".
    pub fn get_extension(&self, base: &str, ext: &str) -> Option<&ResolvedType> {
        self.types.get(&format!("{}.{}", base, ext))
    }

    /// 2026-07-16: P2 — Find meld between base type and an extension type directly.
    pub fn find_ext_meld(&self, base: &str, ext: &str) -> Option<&MeldDeclaration> {
        let ext_name = format!("{}.{}", base, ext);
        self.find_meld(base, &ext_name)
    }

    /// 2026-07-16: P2 — Find a meld from `ty` to any type ending in `.ext`.
    /// Priority:
    ///   1. Direct meld T -> T.ext  (exact match)
    ///   2. Direct meld T -> Any.ext  (custom → standard extension)
    ///   3. T.ext exists with auto-generated identity meld
    ///   4. None — no meld possible
    pub fn find_meld_to_extension(&self, ty: &str, ext: &str) -> Option<(String, MeldDeclaration)> {
        let exact = format!("{}.{}", ty, ext);
        if let Some(decl) = self.find_ext_meld(ty, ext) {
            return Some((exact, decl.clone()));
        }
        for ((a, b), decl) in &self.melds {
            if a == ty && b.ends_with(&format!(".{}", ext)) {
                return Some((b.clone(), decl.clone()));
            }
            if b == ty && a.ends_with(&format!(".{}", ext)) {
                return Some((a.clone(), decl.clone()));
            }
        }
        if self.types.contains_key(&exact) {
            return Some((exact.clone(), MeldDeclaration {
                name_a: ty.to_string(),
                name_b: exact,
                routes: vec![],
                span: None,
            }));
        }
        None
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::top::MeldRouteDef;
    use crate::ast::Expr;

    #[test]
    fn test_get_extension_exists() {
        let mut u = TypeUniverse::new();
        u.types.insert("String.c".into(), ResolvedType {
            name: "String.c".into(), base: "String".into(), bytes: 8, alignment: 8,
            properties: HashMap::new(),
        });
        assert!(u.get_extension("String", "c").is_some());
    }

    #[test]
    fn test_get_extension_not_found() {
        let u = TypeUniverse::new();
        assert!(u.get_extension("String", "c").is_none());
    }

    #[test]
    fn test_find_ext_meld_direct() {
        let mut u = TypeUniverse::new();
        u.melds.insert(("String".into(), "String.c".into()), MeldDeclaration {
            name_a: "String".into(), name_b: "String.c".into(),
            routes: vec![], span: None,
        });
        u.melds.insert(("String.c".into(), "String".into()), MeldDeclaration {
            name_a: "String.c".into(), name_b: "String".into(),
            routes: vec![], span: None,
        });
        assert!(u.find_ext_meld("String", "c").is_some());
    }

    #[test]
    fn test_find_meld_to_extension_priority1() {
        let mut u = TypeUniverse::new();
        u.melds.insert(("String".into(), "String.c".into()), MeldDeclaration {
            name_a: "String".into(), name_b: "String.c".into(),
            routes: vec![], span: None,
        });
        u.melds.insert(("String.c".into(), "String".into()), MeldDeclaration {
            name_a: "String.c".into(), name_b: "String".into(),
            routes: vec![], span: None,
        });
        let result = u.find_meld_to_extension("String", "c");
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, "String.c");
    }

    #[test]
    fn test_find_meld_to_extension_priority2() {
        let mut u = TypeUniverse::new();
        u.melds.insert(("MyType".into(), "String.c".into()), MeldDeclaration {
            name_a: "MyType".into(), name_b: "String.c".into(),
            routes: vec![], span: None,
        });
        u.melds.insert(("String.c".into(), "MyType".into()), MeldDeclaration {
            name_a: "String.c".into(), name_b: "MyType".into(),
            routes: vec![], span: None,
        });
        let result = u.find_meld_to_extension("MyType", "c");
        assert!(result.is_some());
    }

    #[test]
    fn test_find_meld_to_extension_priority3_identity() {
        let mut u = TypeUniverse::new();
        u.types.insert("String.c".into(), ResolvedType {
            name: "String.c".into(), base: "String".into(), bytes: 8, alignment: 8,
            properties: HashMap::new(),
        });
        let result = u.find_meld_to_extension("String", "c");
        assert!(result.is_some());
        let (name, decl) = result.unwrap();
        assert_eq!(name, "String.c");
        assert_eq!(decl.name_a, "String");
        assert_eq!(decl.name_b, "String.c");
    }

    #[test]
    fn test_find_meld_to_extension_none() {
        let u = TypeUniverse::new();
        assert!(u.find_meld_to_extension("String", "c").is_none());
    }
}
