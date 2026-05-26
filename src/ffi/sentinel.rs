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

//! FFI Sentinel
//!
//! Validates pre-conditions and post-conditions for FFI calls.

use super::types::FfiValue;
use crate::ast::ForeignBinding;

/// Validate that a precondition holds for the given input values.
pub fn validate_precondition(
    expr: &str,
    inputs: &[FfiValue],
    param_names: &[String],
) -> Result<(), String> {
    let trimmed = expr.trim().trim_start_matches('[').trim_end_matches(']');

    if trimmed == "true" || trimmed.is_empty() {
        return Ok(());
    }

    if let Some(name) = trimmed
        .strip_suffix(" != null")
        .or_else(|| trimmed.strip_suffix("!= null"))
    {
        let name = name.trim();
        if let Some(idx) = param_names.iter().position(|n| n == name) {
            if let Some(val) = inputs.get(idx) {
                if matches!(val, FfiValue::Void) {
                    return Err(format!(
                        "Precondition failed: {} must not be null",
                        name
                    ));
                }
                return Ok(());
            }
        }
    }

    if let Some(name) = trimmed
        .strip_suffix(" > 0")
        .or_else(|| trimmed.strip_suffix("> 0"))
    {
        let name = name.trim();
        if let Some(idx) = param_names.iter().position(|n| n == name) {
            if let Some(val) = inputs.get(idx) {
                let ok = match val {
                    FfiValue::Int(v) => *v > 0,
                    FfiValue::Float(v) => *v > 0.0,
                    _ => false,
                };
                if !ok {
                    return Err(format!(
                        "Precondition failed: {} must be positive",
                        name
                    ));
                }
                return Ok(());
            }
        }
    }

    Ok(())
}

/// Validate that a postcondition holds after execution.
pub fn validate_postcondition(
    expr: &str,
    inputs: &[FfiValue],
    output: &FfiValue,
    param_names: &[String],
) -> Result<(), String> {
    let trimmed = expr.trim().trim_start_matches('[').trim_end_matches(']');

    if trimmed == "true" || trimmed.is_empty() {
        return Ok(());
    }

    if trimmed.contains("result.is_ok") || trimmed.contains("result.is_ok()") {
        if matches!(output, FfiValue::Void) {
            return Err("Postcondition failed: result should be Ok".to_string());
        }
        return Ok(());
    }

    if trimmed == "result != null" {
        if matches!(output, FfiValue::Void) {
            return Err("Postcondition failed: result should not be null".to_string());
        }
        return Ok(());
    }

    if trimmed.starts_with("result:") {
        if matches!(output, FfiValue::Void) {
            return Err(format!("Postcondition failed: {}", expr));
        }
        return Ok(());
    }

    Ok(())
}

pub struct Sentinel;

impl Sentinel {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_precondition(
        &self,
        binding: &ForeignBinding,
        args: &[FfiValue],
    ) -> Result<(), String> {
        if let Some(pre) = &binding.precondition {
            let names: Vec<String> = binding.inputs.iter().map(|(n, _)| n.clone()).collect();
            validate_precondition(pre, args, &names)
        } else {
            Ok(())
        }
    }

    pub fn validate_postcondition(
        &self,
        binding: &ForeignBinding,
        result: &FfiValue,
    ) -> Result<(), String> {
        if let Some(post) = &binding.postcondition {
            let names: Vec<String> = binding.inputs.iter().map(|(n, _)| n.clone()).collect();
            let args: Vec<FfiValue> = vec![]; // args not available on this method
            validate_postcondition(post, &args, result, &names)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precondition_true_passes() {
        let result = validate_precondition("true", &[], &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_precondition_bracketed_true_passes() {
        let result = validate_precondition("[true]", &[], &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_precondition_empty_passes() {
        let result = validate_precondition("", &[], &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_precondition_not_null_passes() {
        let result = validate_precondition(
            "x != null",
            &[FfiValue::Int(42)],
            &["x".to_string()],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_precondition_not_null_fails() {
        let result = validate_precondition(
            "x != null",
            &[FfiValue::Void],
            &["x".to_string()],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not be null"));
    }

    #[test]
    fn test_precondition_positive_passes() {
        let result = validate_precondition(
            "x > 0",
            &[FfiValue::Int(5)],
            &["x".to_string()],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_precondition_positive_fails() {
        let result = validate_precondition(
            "x > 0",
            &[FfiValue::Int(-3)],
            &["x".to_string()],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be positive"));
    }

    #[test]
    fn test_precondition_positive_float_passes() {
        let result = validate_precondition(
            "x > 0",
            &[FfiValue::Float(1.5)],
            &["x".to_string()],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_postcondition_true_passes() {
        let result = validate_postcondition("true", &[], &FfiValue::Int(0), &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_postcondition_result_not_null_passes() {
        let result = validate_postcondition(
            "result != null",
            &[],
            &FfiValue::Int(42),
            &[],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_postcondition_result_not_null_fails() {
        let result = validate_postcondition(
            "result != null",
            &[],
            &FfiValue::Void,
            &[],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("should not be null"));
    }

    #[test]
    fn test_postcondition_is_ok_passes() {
        let result = validate_postcondition(
            "result.is_ok()",
            &[],
            &FfiValue::Int(1),
            &[],
        );
        assert!(result.is_ok());
    }
}