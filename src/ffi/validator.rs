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

    Ok(())
}

/// Check if a type is valid for FFI (conservative check)
pub fn is_valid_ffi_type(ty: &Type) -> bool {
    match ty {
        Type::Custom(__t) if __t == "String" || __t == "Int" || __t == "Float" || __t == "Bool" || __t == "Data" => true,
        Type::Void => true,
        Type::Custom(_) => true, // Custom types are structs
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Fallback, FromSpec};
    use std::path::PathBuf;

    #[test]
    fn test_validate_matching_signatures() {
        let frgn = ForeignSignature {
            name: "read_file".to_string(),
            from: FromSpec::Literal(PathBuf::from("std::fs::read_to_string")),
            wasm_impl: None,
            wasm_setup: None,
            inputs: vec![("path".to_string(), Type::string())],
            result_type: ResultType::TrueAssertion,
            span: None,
        };

        let binding = ForeignBinding {
            name: "read_file".to_string(),
            as_name: None,
            from: FromSpec::Literal(PathBuf::from("std::fs::read_to_string")),
            target: crate::ast::ForeignTarget::Native,
            wasm_impl: None,
            wasm_setup: None,
            inputs: vec![("path".to_string(), Type::string())],
            success_output: vec![],
            error_type: "Error".to_string(),
            error_fields: vec![],
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            default_watchdog: None,
            fallback: Fallback::None,
            span: None,
        };

        let binding2 = ForeignBinding {
            name: "write_file".to_string(),
            as_name: None,
            from: FromSpec::Literal(PathBuf::from("test")),
            target: crate::ast::ForeignTarget::Native,
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
            default_watchdog: None,
            fallback: Fallback::None,
            span: None,
        };

        assert!(validate_frgn_against_binding(&frgn, &binding2).is_err());
    }

    #[test]
    fn test_is_valid_ffi_type() {
        assert!(is_valid_ffi_type(&Type::string()));
        assert!(is_valid_ffi_type(&Type::int()));
        assert!(is_valid_ffi_type(&Type::float()));
        assert!(is_valid_ffi_type(&Type::bool_()));
        assert!(is_valid_ffi_type(&Type::Void));
        assert!(is_valid_ffi_type(&Type::Custom("IoError".to_string())));
    }
}
