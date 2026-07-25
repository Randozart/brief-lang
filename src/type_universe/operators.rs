// ── Operator Resolution ────────────────────────────────────────────────
// 2026-07-12: Phase 2.3 — Resolve runes (+ - * / == [] etc.) to op bindings.
// This is the most critical function in the compiler (Risk #1 from the plan).
// Flat code: each function is max 2 levels, extracted helpers where needed.

use crate::ast::{OpBinding, Type};
use crate::type_universe::TypeUniverse;

/// The rune-to-op-name mapping. These are the same across all types.
fn rune_to_op_name(rune: &str) -> Option<&'static str> {
    Some(match rune {
        "+" => "Add",
        "-" => "Sub",
        "*" => "Mul",
        "/" => "Div",
        "%" => "Rem",
        "==" => "Eq",
        "!=" => "Neq",
        "<" => "Lt",
        ">" => "Gt",
        "<=" => "Le",
        ">=" => "Ge",
        "&&" => "And",
        "||" => "Or",
        "&" => "BitAnd",
        "|" => "BitOr",
        "^" => "BitXor",
        "<<" => "Shl",
        ">>" => "Shr",
        // 2026-07-14: String concatenation operator was missing
        "++" => "Concat",
        "[]" => "ExtractFrom",
        "[]=" => "InsertAt",
        "()" => "Call",
        _ => return None,
    })
}

/// Resolve a rune to an OpBinding for a given type.
/// Returns None if the type has no binding for the given operator.
///
/// 2026-07-20: Old metadata["op.Add"] lookup removed. Only uses
/// builtin_operator_binding() — a hardcoded table of standard
/// operator-to-intrinsic mappings for well-known types.
/// Hashword OperatorDef from the AST (used for custom types) is
/// resolved separately in emit_expr.rs and intrinsics.rs.
pub fn get_operator_intrinsic(universe: &TypeUniverse, rune: &str, ty: &Type) -> Option<OpBinding> {
    builtin_operator_binding(rune, ty)
}

/// Get the string name of a type for property lookup.
fn type_name_str(ty: &Type) -> Option<&str> {
    match ty {
        Type::Custom(name) => Some(name.as_str()),
        Type::Applied(name, _) => Some(name.as_str()),
        _ => None,
    }
}

/// Get the intrinsic name for an operator on a type.
/// Returns a pretty-printed description for error messages if not found.
pub fn resolve_operator_or_error(
    universe: &TypeUniverse,
    rune: &str,
    ty: &Type,
) -> Result<OpBinding, String> {
    get_operator_intrinsic(universe, rune, ty)
        .ok_or_else(|| format!("no op '{}' for type {}", rune, display_type_short(ty)))
}

/// Short display of a type for error messages.
fn display_type_short(ty: &Type) -> String {
    match ty {
        Type::Custom(name) => name.clone(),
        Type::Applied(name, _) => format!("{}<...>", name),
        Type::Bits(n) => format!("Bits({})", n),
        Type::Void => "Void".into(),
        Type::Ptr(inner) => format!("Ptr<{}>", display_type_short(inner)),
        Type::Tuple(types) => {
            let inner: Vec<String> = types.iter().map(display_type_short).collect();
            format!("({})", inner.join(", "))
        }
        _ => "unknown".into(),
    }
}

/// Hardcoded operator bindings for built-in types (when not in the universe yet).
/// Phase 2A: minimal set. Extended as new types are registered.
pub fn builtin_operator_binding(rune: &str, ty: &Type) -> Option<OpBinding> {
    let op_name = rune_to_op_name(rune)?;
    let type_name = type_name_str(ty)?;

    match (type_name, op_name) {
        ("Int", "Add") => Some(OpBinding::Intrinsic("AddI64#".into())),
        ("Int", "Sub") => Some(OpBinding::Intrinsic("SubI64#".into())),
        ("Int", "Mul") => Some(OpBinding::Intrinsic("MulI64#".into())),
        ("Int", "Div") => Some(OpBinding::Intrinsic("DivI64#".into())),
        ("Int", "Rem") => Some(OpBinding::Intrinsic("RemI64#".into())),
        ("Int", "BitAnd") => Some(OpBinding::Intrinsic("BitAndI64#".into())),
        ("Int", "BitOr") => Some(OpBinding::Intrinsic("BitOrI64#".into())),
        ("Int", "BitXor") => Some(OpBinding::Intrinsic("BitXorI64#".into())),
        ("Int", "Shl") => Some(OpBinding::Intrinsic("ShlI64#".into())),
        ("Int", "Shr") => Some(OpBinding::Intrinsic("ShrI64#".into())),
        ("Int", "Eq") => Some(OpBinding::Intrinsic("EqI64#".into())),
        ("Int", "Lt") => Some(OpBinding::Intrinsic("LtI64#".into())),
        ("Float", "Add") => Some(OpBinding::Intrinsic("FAddF64#".into())),
        ("Float", "Sub") => Some(OpBinding::Intrinsic("FSubF64#".into())),
        ("Float", "Mul") => Some(OpBinding::Intrinsic("FMulF64#".into())),
        ("Float", "Div") => Some(OpBinding::Intrinsic("FDivF64#".into())),
        ("Float", "Eq") => Some(OpBinding::Intrinsic("FEqF64#".into())),
        ("Float", "Lt") => Some(OpBinding::Intrinsic("FLtF64#".into())),
        ("Bool", "Eq") => Some(OpBinding::Intrinsic("EqI1#".into())),
        ("Bool", "And") => Some(OpBinding::Intrinsic("AndI1#".into())),
        ("Bool", "Or") => Some(OpBinding::Intrinsic("OrI1#".into())),
        ("Char", "Eq") => Some(OpBinding::Intrinsic("EqI32#".into())),
        ("String", "Concat") => Some(OpBinding::Intrinsic("StringConcat#".into())),
        ("String", "Eq") => Some(OpBinding::Intrinsic("StringEq#".into())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Type;

    fn empty_universe() -> TypeUniverse {
        TypeUniverse::new()
    }

    #[test]
    fn test_rune_to_op_name() {
        assert_eq!(rune_to_op_name("+"), Some("Add"));
        assert_eq!(rune_to_op_name("=="), Some("Eq"));
        assert_eq!(rune_to_op_name("[]"), Some("ExtractFrom"));
        assert_eq!(rune_to_op_name("???"), None);
    }

    #[test]
    fn test_builtin_int_add() {
        let binding = builtin_operator_binding("+", &Type::int());
        assert_eq!(binding, Some(OpBinding::Intrinsic("AddI64#".into())));
    }

    #[test]
    fn test_builtin_float_mul() {
        let binding = builtin_operator_binding("*", &Type::float());
        assert_eq!(binding, Some(OpBinding::Intrinsic("FMulF64#".into())));
    }

    #[test]
    fn test_builtin_bool_eq() {
        let binding = builtin_operator_binding("==", &Type::bool_());
        assert_eq!(binding, Some(OpBinding::Intrinsic("EqI1#".into())));
    }

    #[test]
    fn test_builtin_string_concat() {
        let binding = builtin_operator_binding("++", &Type::string());
        assert_eq!(binding, Some(OpBinding::Intrinsic("StringConcat#".into())));
    }

    #[test]
    fn test_missing_operator() {
        let binding = builtin_operator_binding("*", &Type::bool_());
        assert_eq!(binding, None);
    }

    #[test]
    fn test_missing_type() {
        let binding = builtin_operator_binding("+", &Type::Custom("Nonexistent".into()));
        assert_eq!(binding, None);
    }

    #[test]
    fn test_universe_lookup_empty() {
        let uni = empty_universe();
        // 2026-07-18: Phase 0 — falls back to builtin_operator_binding
        // when no universe property exists for the type.
        let result = get_operator_intrinsic(&uni, "+", &Type::int());
        assert_eq!(result, Some(OpBinding::Intrinsic("AddI64#".into())));
    }
}
