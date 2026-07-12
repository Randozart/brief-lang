// ── Intrinsic Signature Registry ────────────────────────────────────────
// 2026-07-12: Phase 0.7 — One flat match arm per # intrinsic.
// Adding a new intrinsic: add one arm here + one arm in execute_intrinsic().
// The _ => None fallthrough must remain unchanged.

use crate::ast_new::Type;

/// The signature of a compiler-known # intrinsic.
pub struct Signature {
    pub name: &'static str,
    pub parameters: Vec<(&'static str, Type)>,
    pub return_type: Option<Type>,
}

/// Look up the signature of a # intrinsic by name.
/// Returns None if the name is unknown (it's either a user function or an error).
pub fn get_intrinsic_signature(name: &str) -> Option<Signature> {
    match name {
        "AddI64#" => Some(Signature {
            name: "AddI64#",
            parameters: vec![("a", Type::int()), ("b", Type::int())],
            return_type: Some(Type::int()),
        }),
        "SubI64#" => Some(Signature {
            name: "SubI64#",
            parameters: vec![("a", Type::int()), ("b", Type::int())],
            return_type: Some(Type::int()),
        }),
        "MulI64#" => Some(Signature {
            name: "MulI64#",
            parameters: vec![("a", Type::int()), ("b", Type::int())],
            return_type: Some(Type::int()),
        }),
        "DivI64#" => Some(Signature {
            name: "DivI64#",
            parameters: vec![("a", Type::int()), ("b", Type::int())],
            return_type: Some(Type::int()),
        }),
        "RemI64#" => Some(Signature {
            name: "RemI64#",
            parameters: vec![("a", Type::int()), ("b", Type::int())],
            return_type: Some(Type::int()),
        }),
        "EqI64#" => Some(Signature {
            name: "EqI64#",
            parameters: vec![("a", Type::int()), ("b", Type::int())],
            return_type: Some(Type::bool_()),
        }),
        "LtI64#" => Some(Signature {
            name: "LtI64#",
            parameters: vec![("a", Type::int()), ("b", Type::int())],
            return_type: Some(Type::bool_()),
        }),
        "FAddF64#" => Some(Signature {
            name: "FAddF64#",
            parameters: vec![("a", Type::float()), ("b", Type::float())],
            return_type: Some(Type::float()),
        }),
        "FSubF64#" => Some(Signature {
            name: "FSubF64#",
            parameters: vec![("a", Type::float()), ("b", Type::float())],
            return_type: Some(Type::float()),
        }),
        "FMulF64#" => Some(Signature {
            name: "FMulF64#",
            parameters: vec![("a", Type::float()), ("b", Type::float())],
            return_type: Some(Type::float()),
        }),
        "FDivF64#" => Some(Signature {
            name: "FDivF64#",
            parameters: vec![("a", Type::float()), ("b", Type::float())],
            return_type: Some(Type::float()),
        }),
        "FEqF64#" => Some(Signature {
            name: "FEqF64#",
            parameters: vec![("a", Type::float()), ("b", Type::float())],
            return_type: Some(Type::bool_()),
        }),
        "FLtF64#" => Some(Signature {
            name: "FLtF64#",
            parameters: vec![("a", Type::float()), ("b", Type::float())],
            return_type: Some(Type::bool_()),
        }),
        "EqI1#" => Some(Signature {
            name: "EqI1#",
            parameters: vec![("a", Type::bool_()), ("b", Type::bool_())],
            return_type: Some(Type::bool_()),
        }),
        "EqI32#" => Some(Signature {
            name: "EqI32#",
            parameters: vec![("a", Type::char_()), ("b", Type::char_())],
            return_type: Some(Type::bool_()),
        }),
        "Sqrt#" => Some(Signature {
            name: "Sqrt#",
            parameters: vec![("x", Type::float())],
            return_type: Some(Type::float()),
        }),
        "Malloc#" => Some(Signature {
            name: "Malloc#",
            parameters: vec![("size", Type::int())],
            return_type: Some(Type::ptr(Type::bits(1))),
        }),
        "Free#" => Some(Signature {
            name: "Free#",
            parameters: vec![("ptr", Type::ptr(Type::bits(1)))],
            return_type: None,
        }),
        "PrintInt#" => Some(Signature {
            name: "PrintInt#",
            parameters: vec![("n", Type::int())],
            return_type: None,
        }),
        "PrintFloat#" => Some(Signature {
            name: "PrintFloat#",
            parameters: vec![("f", Type::float())],
            return_type: None,
        }),
        "PrintString#" => Some(Signature {
            name: "PrintString#",
            parameters: vec![("s", Type::string())],
            return_type: None,
        }),
        "GetEnvInt#" => Some(Signature {
            name: "GetEnvInt#",
            parameters: vec![("name", Type::string())],
            return_type: Some(Type::int()),
        }),
        "GetEnvString#" => Some(Signature {
            name: "GetEnvString#",
            parameters: vec![("name", Type::string())],
            return_type: Some(Type::string()),
        }),
        "Memcpy#" => Some(Signature {
            name: "Memcpy#",
            parameters: vec![
                ("dst", Type::ptr(Type::bits(1))),
                ("src", Type::ptr(Type::bits(1))),
                ("n", Type::int()),
            ],
            return_type: None,
        }),
        "Memset#" => Some(Signature {
            name: "Memset#",
            parameters: vec![
                ("ptr", Type::ptr(Type::bits(1))),
                ("val", Type::int()),
                ("n", Type::int()),
            ],
            return_type: None,
        }),
        "StringConcat#" => Some(Signature {
            name: "StringConcat#",
            parameters: vec![("a", Type::string()), ("b", Type::string())],
            return_type: Some(Type::string()),
        }),
        "StringLength#" => Some(Signature {
            name: "StringLength#",
            parameters: vec![("s", Type::string())],
            return_type: Some(Type::int()),
        }),
        "StringEq#" => Some(Signature {
            name: "StringEq#",
            parameters: vec![("a", Type::string()), ("b", Type::string())],
            return_type: Some(Type::bool_()),
        }),
        "FloatToInt#" => Some(Signature {
            name: "FloatToInt#",
            parameters: vec![("f", Type::float())],
            return_type: Some(Type::int()),
        }),
        "IntToFloat#" => Some(Signature {
            name: "IntToFloat#",
            parameters: vec![("n", Type::int())],
            return_type: Some(Type::float()),
        }),
        "IntToString#" => Some(Signature {
            name: "IntToString#",
            parameters: vec![("n", Type::int())],
            return_type: Some(Type::string()),
        }),
        "FloatToString#" => Some(Signature {
            name: "FloatToString#",
            parameters: vec![("f", Type::float())],
            return_type: Some(Type::string()),
        }),
        "CharToInt#" => Some(Signature {
            name: "CharToInt#",
            parameters: vec![("c", Type::char_())],
            return_type: Some(Type::int()),
        }),
        "IntToChar#" => Some(Signature {
            name: "IntToChar#",
            parameters: vec![("n", Type::int())],
            return_type: Some(Type::char_()),
        }),
        "GetGlobalId#" => Some(Signature {
            name: "GetGlobalId#",
            parameters: vec![("dim", Type::int())],
            return_type: Some(Type::int()),
        }),
        "GetGlobalSize#" => Some(Signature {
            name: "GetGlobalSize#",
            parameters: vec![("dim", Type::int())],
            return_type: Some(Type::int()),
        }),
        "GetLocalId#" => Some(Signature {
            name: "GetLocalId#",
            parameters: vec![("dim", Type::int())],
            return_type: Some(Type::int()),
        }),
        "ListInsert#" => Some(Signature {
            name: "ListInsert#",
            parameters: vec![
                ("list", Type::ptr(Type::Custom("List".into()))),
                ("index", Type::int()),
                ("val", Type::bits(8)),
            ],
            return_type: None,
        }),
        "ListGet#" => Some(Signature {
            name: "ListGet#",
            parameters: vec![
                ("list", Type::ptr(Type::Custom("List".into()))),
                ("index", Type::int()),
            ],
            return_type: Some(Type::bits(8)),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_intrinsics_have_signatures() {
        let intrinsics = [
            "AddI64#",
            "SubI64#",
            "MulI64#",
            "DivI64#",
            "RemI64#",
            "EqI64#",
            "LtI64#",
            "FAddF64#",
            "FSubF64#",
            "FMulF64#",
            "FDivF64#",
            "FEqF64#",
            "FLtF64#",
            "EqI1#",
            "EqI32#",
            "Sqrt#",
            "Malloc#",
            "Free#",
            "PrintInt#",
            "PrintFloat#",
            "PrintString#",
            "GetEnvInt#",
            "GetEnvString#",
            "Memcpy#",
            "Memset#",
            "StringConcat#",
            "StringLength#",
            "StringEq#",
            "FloatToInt#",
            "IntToFloat#",
            "IntToString#",
            "FloatToString#",
            "CharToInt#",
            "IntToChar#",
            "GetGlobalId#",
            "GetGlobalSize#",
            "GetLocalId#",
            "ListInsert#",
            "ListGet#",
        ];
        for name in &intrinsics {
            let sig = get_intrinsic_signature(name);
            assert!(sig.is_some(), "missing signature for {}", name);
        }
    }

    #[test]
    fn test_unknown_intrinsic_returns_none() {
        assert!(get_intrinsic_signature("NonExistent#").is_none());
        assert!(get_intrinsic_signature("").is_none());
        assert!(get_intrinsic_signature("user_function").is_none());
    }

    #[test]
    fn test_arithmetic_intrinsic_parameter_count() {
        let add = get_intrinsic_signature("AddI64#").unwrap();
        assert_eq!(add.parameters.len(), 2);
        let sqrt = get_intrinsic_signature("Sqrt#").unwrap();
        assert_eq!(sqrt.parameters.len(), 1);
        let memcpy = get_intrinsic_signature("Memcpy#").unwrap();
        assert_eq!(memcpy.parameters.len(), 3);
    }

    #[test]
    fn test_void_intrinsics_have_no_return() {
        let void_intrinsics = [
            "Free#",
            "PrintInt#",
            "PrintFloat#",
            "PrintString#",
            "Memcpy#",
            "Memset#",
            "ListInsert#",
        ];
        for name in &void_intrinsics {
            let sig = get_intrinsic_signature(name).unwrap();
            assert!(
                sig.return_type.is_none(),
                "{} should have no return type",
                name
            );
        }
    }

    #[test]
    fn test_intrinsic_signature_consistency() {
        let add = get_intrinsic_signature("AddI64#").unwrap();
        assert_eq!(add.name, "AddI64#");
        assert_eq!(add.parameters[0].0, "a");
        assert_eq!(add.parameters[1].0, "b");
    }
}
