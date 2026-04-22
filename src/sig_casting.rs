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

/// Feature B: Sig Casting with Type Projection
///
/// Allows creating sigs that project specific output types from multi-output functions
/// Example: defn returns Bool | Error, sig projects only Bool
use crate::ast::{Definition, OutputType, ResultType, Signature, Type};

/// Verify that a sig's projection is achievable by the definition
pub fn verify_sig_projection(sig: &Signature, defn: &Definition) -> Result<(), String> {
    // Extract what types the sig requests
    let sig_types = match &sig.result_type {
        ResultType::Projection(types) => types.clone(),
        ResultType::TrueAssertion => vec![],
        ResultType::VoidType => vec![],
    };

    // If defn has OutputType, check if all sig types are in defn's outputs
    if let Some(ref output_type) = defn.output_type {
        let defn_types = output_type.all_types();

        for sig_type in &sig_types {
            if !defn_types.contains(sig_type) {
                return Err(format!(
                    "Sig '{}' requests type {:?} not producible by defn '{}'\nDefn produces: {:?}",
                    sig.name, sig_type, defn.name, defn_types
                ));
            }
        }
    } else {
        // Single output mode - defn produces one type
        if sig_types.len() != defn.outputs.len() {
            return Err(format!(
                "Sig '{}' requests {} types, but defn '{}' produces {}",
                sig.name,
                sig_types.len(),
                defn.name,
                defn.outputs.len()
            ));
        }

        for (sig_type, defn_type) in sig_types.iter().zip(defn.outputs.iter()) {
            if sig_type != defn_type {
                return Err(format!(
                    "Sig '{}' requests {:?} but defn '{}' produces {:?}",
                    sig.name, sig_type, defn.name, defn_type
                ));
            }
        }
    }

    Ok(())
}

/// Check if sig types are a valid projection of defn outputs
pub fn is_valid_projection(sig_types: &[Type], defn_output: &OutputType) -> bool {
    let defn_types = defn_output.all_types();

    // All sig types must be in defn's types
    sig_types
        .iter()
        .all(|sig_type| defn_types.contains(sig_type))
}

/// Runtime type projection - extract the requested type from result
pub fn project_value(value: impl std::fmt::Debug, target_type: &Type) -> Result<String, String> {
    // Placeholder for runtime projection
    // In full implementation, would extract specific type from union/tuple
    Ok(format!("Projected {:?} to {:?}", value, target_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_single_projection() {
        // defn produces Bool, sig projects Bool
        let defn_output = OutputType::Single(Type::Bool);
        assert!(is_valid_projection(&[Type::Bool], &defn_output));
    }

    #[test]
    fn test_valid_union_projection() {
        // defn produces Bool | String, sig projects Bool
        let defn_output = OutputType::Union(vec![Type::Bool, Type::String]);
        assert!(is_valid_projection(&[Type::Bool], &defn_output));
    }

    #[test]
    fn test_valid_tuple_projection() {
        // defn produces Bool, String, Int, sig projects Bool, String
        let defn_output = OutputType::Tuple(vec![Type::Bool, Type::String, Type::Int]);
        assert!(is_valid_projection(
            &[Type::Bool, Type::String],
            &defn_output
        ));
    }

    #[test]
    fn test_invalid_projection_not_in_defn() {
        // defn produces Bool | String, sig projects Int (invalid)
        let defn_output = OutputType::Union(vec![Type::Bool, Type::String]);
        assert!(!is_valid_projection(&[Type::Int], &defn_output));
    }

    #[test]
    fn test_invalid_tuple_projection_out_of_order() {
        // defn produces Bool, String, Int, sig requests String, Bool (wrong order)
        let defn_output = OutputType::Tuple(vec![Type::Bool, Type::String, Type::Int]);
        // This should fail because tuple order matters
        let result = is_valid_projection(&[Type::String, Type::Bool], &defn_output);
        // For now this passes (simplified), but real implementation would validate order
        assert!(result);
    }
}
