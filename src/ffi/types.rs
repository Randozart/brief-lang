// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

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

/// Endianness for memory mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Native,
    Little,
    Big,
}

/// Memory layout for a collection of fields
#[derive(Debug, Clone)]
pub struct MemoryLayout {
    pub size_bytes: usize,
    pub alignment: usize,
    pub fields: Vec<FieldDescriptor>,
    pub endian: Endian,
}

/// Description of a single field in memory
#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    pub name: String,
    pub offset: usize,
    pub size_bytes: usize,
    pub element_size: Option<usize>, // For arrays
    pub count: Option<usize>,        // For arrays
    pub endian: Option<Endian>,      // Per-field override
}

/// A value that can be passed through a memory pipe.
#[derive(Debug, Clone, PartialEq)]
pub enum FfiValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Data(Vec<u8>),
    List(Vec<FfiValue>),
    Struct(String, std::collections::HashMap<String, FfiValue>),
    Variant(String, String, std::collections::HashMap<String, FfiValue>), // (type_name, variant_name, fields)
    Void,
}

impl FfiValue {
    pub fn from_interpreter_value(v: &crate::interpreter::Value) -> Self {
        match v {
            crate::interpreter::Value::Int(i) => FfiValue::Int(*i),
            crate::interpreter::Value::Float(f) => FfiValue::Float(*f),
            crate::interpreter::Value::Bool(b) => FfiValue::Bool(*b),
            crate::interpreter::Value::String(s) => FfiValue::String(s.clone()),
            crate::interpreter::Value::Data(d) => FfiValue::Data(d.clone()),
            crate::interpreter::Value::List(l) => {
                FfiValue::List(l.iter().map(Self::from_interpreter_value).collect())
            }
            crate::interpreter::Value::Instance { typename, fields } => {
                let mut ffi_fields = std::collections::HashMap::new();
                for (k, val) in fields {
                    ffi_fields.insert(k.clone(), Self::from_interpreter_value(val));
                }
                FfiValue::Struct(typename.clone(), ffi_fields)
            }
            crate::interpreter::Value::Enum(name, variant, fields) => {
                let mut ffi_fields = std::collections::HashMap::new();
                for (k, val) in fields {
                    ffi_fields.insert(k.clone(), Self::from_interpreter_value(val));
                }
                FfiValue::Variant(name.clone(), variant.clone(), ffi_fields)
            }
            crate::interpreter::Value::Void => FfiValue::Void,
            _ => FfiValue::Void, // Fallback for types we can't map easily
        }
    }

    pub fn to_interpreter_value(&self) -> crate::interpreter::Value {
        match self {
            FfiValue::Int(i) => crate::interpreter::Value::Int(*i),
            FfiValue::Float(f) => crate::interpreter::Value::Float(*f),
            FfiValue::Bool(b) => crate::interpreter::Value::Bool(*b),
            FfiValue::String(s) => crate::interpreter::Value::String(s.clone()),
            FfiValue::Data(d) => crate::interpreter::Value::Data(d.clone()),
            FfiValue::List(l) => crate::interpreter::Value::List(
                l.iter().map(|v| v.to_interpreter_value()).collect(),
            ),
            FfiValue::Struct(name, fields) => {
                let mut int_fields = std::collections::HashMap::new();
                for (k, val) in fields {
                    int_fields.insert(k.clone(), val.to_interpreter_value());
                }
                crate::interpreter::Value::Instance {
                    typename: name.clone(),
                    fields: int_fields,
                }
            }
            FfiValue::Variant(name, variant, fields) => {
                let mut int_fields = std::collections::HashMap::new();
                for (k, val) in fields {
                    int_fields.insert(k.clone(), val.to_interpreter_value());
                }
                crate::interpreter::Value::Enum(name.clone(), variant.clone(), int_fields)
            }
            FfiValue::Void => crate::interpreter::Value::Void,
        }
    }
}

impl MemoryLayout {
    pub fn new() -> Self {
        Self {
            size_bytes: 0,
            alignment: 8,
            fields: Vec::new(),
            endian: Endian::Native,
        }
    }
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
