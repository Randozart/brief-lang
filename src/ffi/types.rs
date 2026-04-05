//! FFI Type System
//!
//! Type mapping and conversion between Brief types and foreign language types

use crate::ast::Type;

/// FFI type representation
#[derive(Debug, Clone, PartialEq)]
pub enum FfiType {
    /// String type
    String,

    /// 64-bit integer
    Int,

    /// 64-bit float
    Float,

    /// Boolean
    Bool,

    /// Unit/void type
    Void,

    /// Array type
    Array(Box<FfiType>),

    /// Struct type with named fields
    Struct(String, Vec<(String, FfiType)>),

    /// Generic type
    Generic(String, Vec<FfiType>),
}

impl std::fmt::Display for FfiType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfiType::String => write!(f, "String"),
            FfiType::Int => write!(f, "Int"),
            FfiType::Float => write!(f, "Float"),
            FfiType::Bool => write!(f, "Bool"),
            FfiType::Void => write!(f, "void"),
            FfiType::Array(t) => write!(f, "[{}]", t),
            FfiType::Struct(name, _) => write!(f, "{}", name),
            FfiType::Generic(name, args) => {
                write!(f, "{}<", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ">")
            }
        }
    }
}

/// Convert Brief type to FFI type
pub fn brief_type_to_ffi(t: &Type) -> Result<FfiType, String> {
    match t {
        Type::String => Ok(FfiType::String),
        Type::Int => Ok(FfiType::Int),
        Type::Float => Ok(FfiType::Float),
        Type::Bool => Ok(FfiType::Bool),
        Type::Void => Ok(FfiType::Void),
        Type::Custom(name) => {
            // Custom types are treated as struct names
            Ok(FfiType::Struct(name.clone(), vec![]))
        }
        _ => Err(format!("Unsupported Brief type for FFI: {:?}", t)),
    }
}

/// Convert FFI type back to Brief type
pub fn ffi_type_to_brief(t: &FfiType) -> Type {
    match t {
        FfiType::String => Type::String,
        FfiType::Int => Type::Int,
        FfiType::Float => Type::Float,
        FfiType::Bool => Type::Bool,
        FfiType::Void => Type::Void,
        FfiType::Struct(name, _) => Type::Custom(name.clone()),
        FfiType::Generic(name, _) => Type::Custom(name.clone()),
        FfiType::Array(_) => Type::Data, // Use Data as generic array type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brief_type_to_ffi_basic() {
        assert_eq!(brief_type_to_ffi(&Type::String).unwrap(), FfiType::String);
        assert_eq!(brief_type_to_ffi(&Type::Int).unwrap(), FfiType::Int);
        assert_eq!(brief_type_to_ffi(&Type::Float).unwrap(), FfiType::Float);
        assert_eq!(brief_type_to_ffi(&Type::Bool).unwrap(), FfiType::Bool);
        assert_eq!(brief_type_to_ffi(&Type::Void).unwrap(), FfiType::Void);
    }

    #[test]
    fn test_ffi_type_roundtrip() {
        let ffi = FfiType::String;
        let brief = ffi_type_to_brief(&ffi);
        assert_eq!(brief, Type::String);
    }

    #[test]
    fn test_ffi_type_display() {
        assert_eq!(FfiType::String.to_string(), "String");
        assert_eq!(FfiType::Int.to_string(), "Int");
        assert_eq!(
            FfiType::Array(Box::new(FfiType::String)).to_string(),
            "[String]"
        );
    }
}
