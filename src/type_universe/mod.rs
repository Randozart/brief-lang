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
/// Properties, fields, and ops are all stored here — the flat property bag
/// holds ops (keyed as "op.Add" etc.) and general annotations, while the
/// `fields` vec stores ordered struct-like field declarations.
///
/// 2026-07-18: Added `fields` — struct field declarations populated from
/// TypeDef.body.slots by the normalizer (register_typedefs). For primitive
/// types seeded via seed_primordial_types(), fields is empty unless the type
/// has a struct-like shape (e.g. String has [("data", Int), ("len", Int)]).
/// Codegen uses this to drive LLVM struct lowering and state slot width,
/// replacing hardcoded "String" → "ptr" matches with shape+encoding checks.
#[derive(Debug, Clone)]
pub struct ResolvedType {
    pub name: String,
    pub base: String,
    /// Exact byte width (for fixed-width types like Int32, Float).
    /// For flexible types (Int), use max_bits instead.
    pub bytes: u64,
    /// Minimum bit width (0 = unknown/flexible). The compiler may narrow
    /// the type to any width between min_bits and max_bits.
    /// 2026-07-24: Added for value-range narrowing.
    pub min_bits: u64,
    /// Maximum bit width (upper bound). The type MUST fit in this width.
    /// For fixed types like Int32, min_bits == max_bits == 32.
    /// For flexible types like Int, min_bits=0, max_bits=64.
    pub max_bits: u64,
    pub alignment: u64,
    pub properties: HashMap<String, crate::ast::PropertyValue>,
    /// 2026-07-18: Struct field declarations — (name, type) pairs from
    /// TypeDef.body.slots or primordial seed table. Empty for scalars.
    /// Drives LLVM struct type lowering and is_string_like() checks.
    pub fields: Vec<(String, crate::ast::Type)>,
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
    /// `type X { ... }` declarations override these via register().
    ///
    /// 2026-07-20: Simplified for hashword protocol architecture.
    /// No `ctd`, `alu`, or `encoding` properties. Types get `llvm_type`
    /// set directly here so the normalizer (which only derives "i{N*8}"
    /// from bytes) doesn't re-derive well-known types as raw integers.
    /// Hashword op signatures are the new dispatch mechanism.
    fn seed_primordial_types(&mut self) {
        // 2026-07-30: Bit is the axiomatic anchor — NOT a primordial.
        // It cannot be overloaded or redeclared. Primordials are
        // overrideable; Bit is the compiler's sole hardcoded constant.
        self.types.insert("Bit".to_string(), ResolvedType {
            name: "Bit".to_string(),
            base: "Bit".to_string(),
            bytes: 0,
            min_bits: 0,
            max_bits: 0,
            alignment: 0,
            properties: {
                let mut p = std::collections::HashMap::new();
                p.insert("Cast.#Bit".into(), crate::ast::PropertyValue::Bool(true));
                p
            },
            fields: vec![],
        });

        // Table: (name, bytes, min_bits, max_bits, alignment, &[(&str, &str)])
        // bytes is the exact width for fixed types; min_bits/max_bits is
        // the range for flexible types (Int has max_bits=64, min_bits=0).
        // 2026-07-30: These are overrideable by stdlib — adding a type
        // declaration of the same name in bootstrap.bv replaces the
        // primordial entry without error.
        // 2026-07-30: No llvm_type column — LLVM type is resolved by the
        // casting graph from (protocol, metadata). See resolve_llvm_type().
        const PRIMORDIALS: &[(&str, u64, u64, u64, u64, &[(&str, &str)])] = &[
            // 2026-07-29: Flexible protocol types — all fields resolved by normalizer from int_bits.
            // No baked-in width, alignment, or bytes. Every value is 0 = "not yet resolved."
            ("Int",    0, 0, 0,  0, &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
            ("UInt",   0, 0, 0,  0, &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
            // Fixed-width integer types — exact bit width is absolute
            ("Int8",   1, 8, 8,  1, &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
            ("UInt8",  1, 8, 8,  1, &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
            ("Int16",  2, 16, 16, 2, &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
            ("UInt16", 2, 16, 16, 2, &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
            ("Int32",  4, 32, 32, 4, &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
            ("UInt32", 4, 32, 32, 4, &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
            ("Int64",  8, 64, 64, 8, &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
            ("UInt64", 8, 64, 64, 8, &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
            ("Int128", 16, 128, 128, 16, &[("Cast.#Int", "true"), ("Cast.#Bit", "true")]),
            ("UInt128",16, 128, 128, 16, &[("Cast.#UInt", "true"), ("Cast.#Bit", "true")]),
            // Floating-point types — bit-width is accuracy, not maximum storage.
            // Each float type carries an explicit bits property for the normalizer.
            ("Half",   2, 16, 16, 2, &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "16")]),
            ("BFloat", 2, 16, 16, 2, &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "16")]),
            ("Float",  4, 32, 32, 4, &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "32")]),
            ("Float32",4, 32, 32, 4, &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "32")]),
            ("Float64",8, 64, 64, 8, &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "64")]),
            ("Double", 8, 64, 64, 8, &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "64")]),
            ("X86_FP80",10, 80, 80, 4, &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "80")]),
            ("FP128",  16, 128, 128, 16, &[("Cast.#Float", "true"), ("Cast.#Bit", "true"), ("bits", "128")]),
            // Other
            ("Bool",   1, 8, 8,  1, &[("Cast.#Bool", "true"), ("Cast.#Bit", "true")]),
            ("Char",   4, 32, 32, 4, &[("Cast.#Bit", "true")]),
            ("Data",   8, 64, 64, 8, &[("Cast.#Data", "true"), ("Cast.#Bit", "true")]),
            ("Void",   0, 0,  0,  0, &[]),
        ];
        for &(name, bytes, min_bits, max_bits, alignment, extras) in PRIMORDIALS {
            let mut properties = std::collections::HashMap::new();
            properties.insert("alignment".into(), crate::ast::PropertyValue::Int(alignment as i64));
            for &(k, v) in extras {
                // 2026-07-30: Numeric extras (bits, maxbits) are stored as Int so
                if let Ok(n) = v.parse::<i64>() {
                    properties.insert(k.to_string(), crate::ast::PropertyValue::Int(n));
                } else {
                    properties.insert(k.to_string(), crate::ast::PropertyValue::String(v.to_string()));
                }
            }
            self.types.insert(name.to_string(), ResolvedType {
                name: name.to_string(),
                base: "Bit".to_string(),
                bytes,
                min_bits,
                max_bits,
                alignment,
                properties,
                fields: vec![],
            });
        }
        // 2026-07-18: String primordial — 2-field struct (data: Int, len: Int)
        // The casting graph resolves String's LLVM type as Fixed("{ i64, i64 }")
        // from #String protocol membership. No llvm_type property needed.
        {
            let mut p = std::collections::HashMap::new();
            p.insert("alignment".into(), crate::ast::PropertyValue::Int(8));
            self.types.insert("String".to_string(), ResolvedType {
                name: "String".to_string(),
                base: "Bit".to_string(),
                bytes: 16,
                min_bits: 128,
                max_bits: 128,
                alignment: 8,
                properties: p,
                fields: vec![
                    ("data".into(), crate::ast::Type::int()),
                    ("len".into(), crate::ast::Type::int()),
                ],
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

    /// 2026-07-18: Check if a type is string-like by shape (2 Int fields).
    /// A type with `{ data: Int; len: Int; }` structure is string-like
    /// regardless of encoding or CTD. Hashword op signatures provide the
    /// backend with the specific encoding variant.
    /// 2026-07-20: Removed CTD property check and encoding property check.
    /// Structure alone determines layout — protocol ops determine behavior.
    pub fn is_string_like(&self, ty: &Type) -> bool {
        let name = match ty {
            Type::Custom(n) | Type::Applied(n, _) => n,
            _ => return false,
        };
        let Some(rt) = self.types.get(name) else { return false; };
        rt.fields.len() == 2
            && rt.fields[0].1 == Type::int()
            && rt.fields[1].1 == Type::int()
    }

    /// 2026-07-18: Check if a type is a vector-like type eligible for SVO.
    /// Detects types with `op.SVO <~ N` metadata (typically List<T>).
    /// N specifies the inline capacity in elements (e.g. N=3 means 3 elements
    /// stored inline before promoting to heap).
    pub fn is_vector_like(&self, ty: &Type) -> bool {
        let name = match ty {
            Type::Custom(n) | Type::Applied(n, _) => n,
            _ => return false,
        };
        let Some(rt) = self.types.get(name) else { return false; };
        rt.properties.contains_key("op.SVO")
    }

    /// 2026-07-18: Get the SVO inline capacity N for a vector-like type.
    /// Returns 0 if not a vector-like type or no capacity metadata.
    pub fn svo_capacity(&self, ty: &Type) -> usize {
        let name = match ty {
            Type::Custom(n) | Type::Applied(n, _) => n,
            _ => return 0,
        };
        let Some(rt) = self.types.get(name) else { return 0; };
        match rt.properties.get("op.SVO") {
            Some(crate::ast::PropertyValue::Identifier(s)) => s.parse().unwrap_or(0),
            Some(crate::ast::PropertyValue::String(s)) => s.parse().unwrap_or(0),
            _ => 0,
        }
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
            min_bits: 64, max_bits: 64,
            properties: HashMap::new(), fields: vec![],
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
            min_bits: 64, max_bits: 64,
            properties: HashMap::new(), fields: vec![],
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
