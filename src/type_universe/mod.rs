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
/// CTD and ALU are set by the primordial, llvm_type by the normalizer.
/// All three are read via properties — no hardcoded getter methods.
#[derive(Debug, Clone)]
pub struct ResolvedType {
    pub name: String,
    pub base: String,
    pub bytes: u64,
    pub alignment: u64,
    pub properties: HashMap<String, crate::ast::PropertyValue>,
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

/// Return the default ALU for a given Common Type Definition.
// 2026-07-17: ALU describes what hardware computes with values of this type.
// PascalCase = known to all backends; lowercase-quoted = backend-specific.
fn default_alu(ctd: &str) -> &'static str {
    match ctd {
        // Float and Double use the FPU; Bool uses boolean logic;
        // everything else (Int, UInt, Char, String, Data, Ptr, Void) uses integer ALU.
        "Float" | "Double" => "Float",
        "Bool" => "Bool",
        _ => "Int",
    }
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
    ///
    /// 2026-07-17: Types now store `ctd` (Common Type Definition — what the
    /// type is semantically) and `alu` (what hardware computes with it) instead
    /// of `primitive` + `llvm_type`. llvm_type is set by the backend normalizer.
    fn seed_primordial_types(&mut self) {
        // Table: (name, bytes, alignment, ctd)
        // CTD is a PascalCase identifier from the exhaustive set:
        //   Int, UInt, Float, Double, Bool, Char, String, Data, Ptr, Void
        // ALU is derived from CTD via default_alu() and can be overridden.
        const PRIMORDIALS: &[(&str, u64, u64, &str)] = &[
            ("Int",    8, 8, "Int"),
            ("UInt",   8, 8, "UInt"),
            ("Int8",   1, 1, "Int"),
            ("UInt8",  1, 1, "UInt"),
            ("Int16",  2, 2, "Int"),
            ("UInt16", 2, 2, "UInt"),
            ("Int32",  4, 4, "Int"),
            ("UInt32", 4, 4, "UInt"),
            ("Int64",  8, 8, "Int"),
            ("UInt64", 8, 8, "UInt"),
            ("Float",  4, 4, "Float"),
            ("Float32",4, 4, "Float"),
            ("Float64",8, 8, "Double"),
            ("Double", 8, 8, "Double"),
            ("Bool",   1, 1, "Bool"),
            ("Char",   4, 4, "Char"),
            ("Data",   8, 8, "Data"),
            ("Void",   0, 0, "Void"),
        ];
        for &(name, bytes, alignment, ctd) in PRIMORDIALS {
            let mut properties = std::collections::HashMap::new();
            properties.insert("ctd".into(), crate::ast::PropertyValue::Identifier(ctd.to_string()));
            // 2026-07-17: Default ALU per CTD. User types can override via alu ~> ...;
            properties.insert("alu".into(), crate::ast::PropertyValue::Identifier(default_alu(ctd).to_string()));
            properties.insert("alignment".into(), crate::ast::PropertyValue::Int(alignment as i64));
            self.types.insert(name.to_string(), ResolvedType {
                name: name.to_string(),
                base: "Bits".to_string(),
                bytes,
                alignment,
                properties,
            });
        }
        // String — special case: heap-allocated struct with ptr+len+codec fields
        // CTD = String tells the normalizer to map to "ptr" at ABI boundaries.
        {
            let mut p = std::collections::HashMap::new();
            p.insert("ctd".into(), crate::ast::PropertyValue::Identifier("String".to_string()));
            p.insert("alu".into(), crate::ast::PropertyValue::Identifier("Int".to_string()));
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
