// ── Intrinsic Signature Registry ────────────────────────────────────────
// 2026-07-14: Generic operations. Types inferred from arguments.
// Adding a new intrinsic: add one arm here + one arm in execute_intrinsic().
// Every generic op uses vec![] for param_types — type is inferred.

use crate::ast::Type;

/// The signature of a compiler-known # intrinsic.
pub struct Signature {
    pub name: &'static str,
    pub parameters: Vec<(&'static str, Type)>,
    pub return_type: Option<Type>,
    /// If true, this intrinsic has observable side effects (DCE guard).
    pub observable: bool,
}

/// Look up the signature of a # intrinsic by name.
pub fn get_intrinsic_signature(name: &str) -> Option<Signature> {
    match name {
        // ── Arithmetic (type-inferred) ──────────────────────────────
        "Add#" => Some(Signature { name: "Add#", parameters: vec![], return_type: None, observable: false }),
        "Sub#" => Some(Signature { name: "Sub#", parameters: vec![], return_type: None, observable: false }),
        "Mul#" => Some(Signature { name: "Mul#", parameters: vec![], return_type: None, observable: false }),
        "Div#" => Some(Signature { name: "Div#", parameters: vec![], return_type: None, observable: false }),
        "Rem#" => Some(Signature { name: "Rem#", parameters: vec![], return_type: None, observable: false }),
        "Neg#" => Some(Signature { name: "Neg#", parameters: vec![], return_type: None, observable: false }),
        "Abs#" => Some(Signature { name: "Abs#", parameters: vec![], return_type: None, observable: false }),

        // ── Comparison (type-inferred) ──────────────────────────────
        "Eq#"  => Some(Signature { name: "Eq#",  parameters: vec![], return_type: None, observable: false }),
        "Neq#" => Some(Signature { name: "Neq#", parameters: vec![], return_type: None, observable: false }),
        "Lt#"  => Some(Signature { name: "Lt#",  parameters: vec![], return_type: None, observable: false }),
        "Gt#"  => Some(Signature { name: "Gt#",  parameters: vec![], return_type: None, observable: false }),
        "Le#"  => Some(Signature { name: "Le#",  parameters: vec![], return_type: None, observable: false }),
        "Ge#"  => Some(Signature { name: "Ge#",  parameters: vec![], return_type: None, observable: false }),

        // ── Float math (type-inferred, float-specific) ──────────────
        "Sqrt#"  => Some(Signature { name: "Sqrt#",  parameters: vec![], return_type: None, observable: false }),
        "Sin#"   => Some(Signature { name: "Sin#",   parameters: vec![], return_type: None, observable: false }),
        "Cos#"   => Some(Signature { name: "Cos#",   parameters: vec![], return_type: None, observable: false }),
        "Fabs#"  => Some(Signature { name: "Fabs#",  parameters: vec![], return_type: None, observable: false }),
        "Ceil#"  => Some(Signature { name: "Ceil#",  parameters: vec![], return_type: None, observable: false }),
        "Floor#" => Some(Signature { name: "Floor#", parameters: vec![], return_type: None, observable: false }),
        "Pow#"   => Some(Signature { name: "Pow#",   parameters: vec![], return_type: None, observable: false }),

        // ── Memory (observable) ─────────────────────────────────────
        "Malloc#"  => Some(Signature { name: "Malloc#",  parameters: vec![("size", Type::int())], return_type: Some(Type::ptr(Type::bits(1))), observable: true }),
        "Free#"    => Some(Signature { name: "Free#",    parameters: vec![("ptr", Type::ptr(Type::bits(1)))], return_type: None, observable: true }),
        "Memcpy#"  => Some(Signature { name: "Memcpy#",  parameters: vec![], return_type: None, observable: true }),
        "Memset#"  => Some(Signature { name: "Memset#",  parameters: vec![], return_type: None, observable: true }),

        // ── I/O (observable) ─────────────────────────────────────────
        "Print#"  => Some(Signature { name: "Print#",  parameters: vec![], return_type: None, observable: true }),
        "GetEnv#" => Some(Signature { name: "GetEnv#", parameters: vec![], return_type: None, observable: true }),

        // ── String (type-inferred) ──────────────────────────────────
        "Concat#"    => Some(Signature { name: "Concat#",    parameters: vec![], return_type: None, observable: false }),
        "Length#"    => Some(Signature { name: "Length#",    parameters: vec![], return_type: None, observable: false }),
        "ToInt#"     => Some(Signature { name: "ToInt#",     parameters: vec![], return_type: None, observable: false }),
        "ToFloat#"   => Some(Signature { name: "ToFloat#",   parameters: vec![], return_type: None, observable: false }),
        "ToString#"  => Some(Signature { name: "ToString#",  parameters: vec![], return_type: None, observable: false }),

        // ── Collection (observable due to mutation) ─────────────────
        "Get#"    => Some(Signature { name: "Get#",    parameters: vec![], return_type: None, observable: false }),
        "Insert#" => Some(Signature { name: "Insert#", parameters: vec![], return_type: None, observable: true }),

        // ── GPU (observable due to side channels) ───────────────────
        "GetGlobalId#"   => Some(Signature { name: "GetGlobalId#",   parameters: vec![], return_type: None, observable: false }),
        "GetGlobalSize#" => Some(Signature { name: "GetGlobalSize#", parameters: vec![], return_type: None, observable: false }),
        "GetLocalId#"    => Some(Signature { name: "GetLocalId#",    parameters: vec![], return_type: None, observable: false }),

        // ── Pointers (compile-time address resolution) ──────────────
        "AddressOf#" => Some(Signature {
            name: "AddressOf#",
            parameters: vec![("id", Type::string())],
            return_type: Some(Type::ptr(Type::bits(8))),
            observable: false,
        }),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_generic_intrinsics_have_signatures() {
        let intrinsics = [
            "Add#", "Sub#", "Mul#", "Div#", "Rem#", "Neg#", "Abs#",
            "Eq#", "Neq#", "Lt#", "Gt#", "Le#", "Ge#",
            "Sqrt#", "Sin#", "Cos#", "Fabs#", "Ceil#", "Floor#", "Pow#",
            "Malloc#", "Free#", "Memcpy#", "Memset#",
            "Print#", "GetEnv#",
            "Concat#", "Length#", "ToInt#", "ToFloat#", "ToString#",
            "Get#", "Insert#",
            "GetGlobalId#", "GetGlobalSize#", "GetLocalId#",
            "AddressOf#",
        ];
        for name in &intrinsics {
            let sig = get_intrinsic_signature(name);
            assert!(sig.is_some(), "missing signature for {}", name);
        }
    }

    #[test]
    fn test_unknown_intrinsic_returns_none() {
        assert!(get_intrinsic_signature("NonExistent#").is_none());
        assert!(get_intrinsic_signature("AddI64#").is_none());
        assert!(get_intrinsic_signature("").is_none());
        assert!(get_intrinsic_signature("user_function").is_none());
    }

    #[test]
    fn test_observable_intrinsics() {
        assert!(get_intrinsic_signature("Print#").unwrap().observable);
        assert!(get_intrinsic_signature("Malloc#").unwrap().observable);
        assert!(get_intrinsic_signature("Free#").unwrap().observable);
        assert!(get_intrinsic_signature("Memcpy#").unwrap().observable);
        assert!(get_intrinsic_signature("GetEnv#").unwrap().observable);
        assert!(get_intrinsic_signature("Insert#").unwrap().observable);
        assert!(!get_intrinsic_signature("Add#").unwrap().observable);
        assert!(!get_intrinsic_signature("Eq#").unwrap().observable);
    }

    #[test]
    fn test_address_of_signature() {
        let sig = get_intrinsic_signature("AddressOf#").unwrap();
        assert_eq!(sig.name, "AddressOf#");
        assert_eq!(sig.parameters.len(), 1);
        assert_eq!(sig.parameters[0].0, "id");
        assert!(!sig.observable);
        assert!(sig.return_type.is_some());
    }
}
