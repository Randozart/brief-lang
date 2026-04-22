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

//! FFI Binding Validation
//!
//! Validates that frgn declarations match their corresponding TOML bindings

use super::FfiError;
use crate::ast::{ForeignBinding, ForeignSignature, ResultType, Type};

/// Validate that a frgn signature matches its TOML binding
pub fn validate_frgn_against_binding(
    frgn: &ForeignSignature,
    binding: &ForeignBinding,
) -> Result<(), FfiError> {
    // Check name matches
    if frgn.name != binding.name {
        return Err(FfiError::ValidationError(format!(
            "Name mismatch: frgn '{}' vs binding '{}'",
            frgn.name, binding.name
        )));
    }

    // Check input parameter count
    if frgn.inputs.len() != binding.inputs.len() {
        return Err(FfiError::ValidationError(format!(
            "Input parameter count mismatch for '{}': frgn has {}, binding has {}",
            frgn.name,
            frgn.inputs.len(),
            binding.inputs.len()
        )));
    }

    // Check input types match
    for (i, (frgn_param, binding_param)) in
        frgn.inputs.iter().zip(binding.inputs.iter()).enumerate()
    {
        if frgn_param.1 != binding_param.1 {
            return Err(FfiError::ValidationError(format!(
                "Parameter {} type mismatch in '{}': frgn {:?}, binding {:?}",
                i, frgn.name, frgn_param.1, binding_param.1
            )));
        }
    }

    // Check success output count
    if frgn.success_output.len() != binding.success_output.len() {
        return Err(FfiError::ValidationError(format!(
            "Success output count mismatch for '{}': frgn has {}, binding has {}",
            frgn.name,
            frgn.success_output.len(),
            binding.success_output.len()
        )));
    }

    // Check success output types match
    for (frgn_out, binding_out) in frgn
        .success_output
        .iter()
        .zip(binding.success_output.iter())
    {
        if frgn_out.1 != binding_out.1 {
            return Err(FfiError::ValidationError(format!(
                "Success output type mismatch in '{}': frgn {:?}, binding {:?}",
                frgn.name, frgn_out.1, binding_out.1
            )));
        }
    }

    // Check error type name matches
    if frgn.error_type_name != binding.error_type {
        return Err(FfiError::ValidationError(format!(
            "Error type name mismatch in '{}': frgn '{}', binding '{}'",
            frgn.name, frgn.error_type_name, binding.error_type
        )));
    }

    // Check error fields match
    if frgn.error_fields.len() != binding.error_fields.len() {
        return Err(FfiError::ValidationError(format!(
            "Error field count mismatch in '{}': frgn has {}, binding has {}",
            frgn.name,
            frgn.error_fields.len(),
            binding.error_fields.len()
        )));
    }

    for (frgn_field, binding_field) in frgn.error_fields.iter().zip(binding.error_fields.iter()) {
        if frgn_field.1 != binding_field.1 {
            return Err(FfiError::ValidationError(format!(
                "Error field type mismatch in '{}' field {}: frgn {:?}, binding {:?}",
                frgn.name, frgn_field.0, frgn_field.1, binding_field.1
            )));
        }
    }

    Ok(())
}

/// Check if a type is valid for FFI (conservative check)
pub fn is_valid_ffi_type(ty: &Type) -> bool {
    match ty {
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Void | Type::Data => true,
        Type::Custom(_) => true, // Custom types are structs
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_matching_signatures() {
        let frgn = ForeignSignature {
            name: "read_file".to_string(),
            location: "std::fs::read_to_string".to_string(),
            wasm_impl: None,
            wasm_setup: None,
            inputs: vec![("path".to_string(), Type::String)],
            success_output: vec![("content".to_string(), Type::String)],
            result_type: ResultType::TrueAssertion,
            error_type_name: "IoError".to_string(),
            error_fields: vec![
                ("code".to_string(), Type::Int),
                ("message".to_string(), Type::String),
            ],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            ffi_kind: None,
            span: None,
        };

        let binding = ForeignBinding {
            name: "read_file".to_string(),
            description: Some("Read file".to_string()),
            location: "std::fs::read_to_string".to_string(),
            target: crate::ast::ForeignTarget::Native,
            mapper: Some("rust".to_string()),
            path: None,
            wasm_impl: None,
            wasm_setup: None,
            inputs: vec![("path".to_string(), Type::String)],
            success_output: vec![("content".to_string(), Type::String)],
            error_type: "IoError".to_string(),
            error_fields: vec![
                ("code".to_string(), Type::Int),
                ("message".to_string(), Type::String),
            ],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
        };

        assert!(validate_frgn_against_binding(&frgn, &binding).is_ok());
    }

    #[test]
    fn test_validate_name_mismatch() {
        let mut frgn = ForeignSignature {
            name: "read_file".to_string(),
            location: "test".to_string(),
            wasm_impl: None,
            wasm_setup: None,
            inputs: vec![],
            success_output: vec![],
            result_type: ResultType::TrueAssertion,
            error_type_name: "Error".to_string(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            ffi_kind: None,
            span: None,
        };

        let binding = ForeignBinding {
            name: "write_file".to_string(),
            description: None,
            location: "test".to_string(),
            target: crate::ast::ForeignTarget::Native,
            mapper: Some("rust".to_string()),
            path: None,
            wasm_impl: None,
            wasm_setup: None,
            inputs: vec![],
            success_output: vec![],
            error_type: "Error".to_string(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
        };

        assert!(validate_frgn_against_binding(&frgn, &binding).is_err());
    }

    #[test]
    fn test_is_valid_ffi_type() {
        assert!(is_valid_ffi_type(&Type::String));
        assert!(is_valid_ffi_type(&Type::Int));
        assert!(is_valid_ffi_type(&Type::Float));
        assert!(is_valid_ffi_type(&Type::Bool));
        assert!(is_valid_ffi_type(&Type::Void));
        assert!(is_valid_ffi_type(&Type::Custom("IoError".to_string())));
    }
}
