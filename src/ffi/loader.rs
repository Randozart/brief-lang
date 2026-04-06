//! FFI TOML Binding Loader
//!
//! Loads and parses TOML binding files into ForeignBinding structures

use super::FfiError;
use crate::ast::{ForeignBinding, ForeignTarget, Type};
use std::path::Path;

/// Parse a TOML binding file and extract all function bindings
pub fn load_binding(path: &Path) -> Result<Vec<ForeignBinding>, FfiError> {
    // Read file content
    let content = std::fs::read_to_string(path)
        .map_err(|e| FfiError::FileNotFound(format!("{}: {}", path.display(), e)))?;

    parse_toml_bindings(&content)
}

/// Parse TOML content and extract bindings
fn parse_toml_bindings(content: &str) -> Result<Vec<ForeignBinding>, FfiError> {
    // Use toml crate to parse
    let parsed: toml::Value =
        toml::from_str(content).map_err(|e| FfiError::TomlParseError(e.to_string()))?;

    let mut bindings = Vec::new();

    // Extract functions array
    let functions = parsed
        .get("functions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| FfiError::MissingField("functions".to_string()))?;

    for func_val in functions {
        let func = func_val
            .as_table()
            .ok_or_else(|| FfiError::TomlParseError("Each function must be a table".to_string()))?;

        // Extract required fields
        let name = func
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| FfiError::MissingField("functions[].name".to_string()))?
            .to_string();

        let location = func
            .get("location")
            .and_then(|v| v.as_str())
            .ok_or_else(|| FfiError::MissingField("functions[].location".to_string()))?
            .to_string();

        let target_str = func
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| FfiError::MissingField("functions[].target".to_string()))?;

        let target = match target_str {
            "native" => ForeignTarget::Native,
            "wasm" => ForeignTarget::Wasm,
            "c" => ForeignTarget::C,
            "python" => ForeignTarget::Python,
            "js" => ForeignTarget::Js,
            "swift" => ForeignTarget::Swift,
            "go" => ForeignTarget::Go,
            _ => {
                return Err(FfiError::TomlParseError(format!(
                    "Unknown target: {}",
                    target_str
                )))
            }
        };

        let description = func
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse optional mapper field (required for FFI bindings)
        let mapper = func
            .get("mapper")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse optional path field for explicit mapper location
        let path = func
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse input parameters
        let inputs = parse_toml_table(func.get("input").and_then(|v| v.as_table()))?;

        // Parse success output
        let success_output = parse_toml_table(
            func.get("output")
                .and_then(|v| v.get("success"))
                .and_then(|v| v.as_table()),
        )?;

        // Parse error type
        let error_table = func
            .get("output")
            .and_then(|v| v.get("error"))
            .and_then(|v| v.as_table())
            .ok_or_else(|| FfiError::MissingField("functions[].output.error".to_string()))?;

        let error_type = error_table
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| FfiError::MissingField("functions[].output.error.type".to_string()))?
            .to_string();

        // Extract error fields (excluding 'type')
        let mut error_fields = Vec::new();
        for (key, value) in error_table {
            if key != "type" {
                let type_str = value.as_str().ok_or_else(|| {
                    FfiError::TomlParseError(format!(
                        "Error field {} must be a string type name",
                        key
                    ))
                })?;
                let ty = parse_type_string(type_str)?;
                error_fields.push((key.clone(), ty));
            }
        }

        let binding = ForeignBinding {
            name,
            description,
            location,
            target,
            mapper,
            path,
            inputs,
            success_output,
            error_type,
            error_fields,
        };

        bindings.push(binding);
    }

    Ok(bindings)
}

/// Parse a TOML table into (field_name, Type) pairs
fn parse_toml_table(
    table: Option<&toml::map::Map<String, toml::Value>>,
) -> Result<Vec<(String, Type)>, FfiError> {
    let mut result = Vec::new();

    if let Some(t) = table {
        for (key, value) in t {
            let type_str = value.as_str().ok_or_else(|| {
                FfiError::TomlParseError(format!("Field {} must have a string type name", key))
            })?;
            let ty = parse_type_string(type_str)?;
            result.push((key.clone(), ty));
        }
    }

    Ok(result)
}

/// Parse a type string (e.g., "String", "Int", "[String]") into a Type
fn parse_type_string(type_str: &str) -> Result<Type, FfiError> {
    let type_str = type_str.trim();

    match type_str {
        "String" => Ok(Type::String),
        "Int" => Ok(Type::Int),
        "Float" => Ok(Type::Float),
        "Bool" => Ok(Type::Bool),
        "void" => Ok(Type::Void),
        "Data" => Ok(Type::Data),
        s if s.starts_with('[') && s.ends_with(']') => {
            let inner_str = &s[1..s.len() - 1];
            let inner_type = parse_type_string(inner_str)?;
            // Represent arrays as Data for now
            Ok(Type::Data)
        }
        s => {
            // Custom type
            Ok(Type::Custom(s.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_type_string_basic() {
        assert_eq!(parse_type_string("String").unwrap(), Type::String);
        assert_eq!(parse_type_string("Int").unwrap(), Type::Int);
        assert_eq!(parse_type_string("Float").unwrap(), Type::Float);
        assert_eq!(parse_type_string("Bool").unwrap(), Type::Bool);
        assert_eq!(parse_type_string("void").unwrap(), Type::Void);
    }

    #[test]
    fn test_parse_type_string_custom() {
        match parse_type_string("IoError").unwrap() {
            Type::Custom(name) => assert_eq!(name, "IoError"),
            _ => panic!("Expected custom type"),
        }
    }

    #[test]
    fn test_parse_toml_bindings_minimal() {
        let toml = r#"
[[functions]]
name = "read_file"
location = "std::fs::read_to_string"
target = "native"

[functions.input]
path = "String"

[functions.output.success]
content = "String"

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
        "#;

        let result = parse_toml_bindings(toml);
        assert!(result.is_ok(), "Failed to parse TOML: {:?}", result);

        let bindings = result.unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].name, "read_file");
        assert_eq!(bindings[0].location, "std::fs::read_to_string");
        assert_eq!(bindings[0].error_type, "IoError");
    }
}
