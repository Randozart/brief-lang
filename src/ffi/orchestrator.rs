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

//! FFI Orchestrator
//!
//! Manages the memory pipe and coordinates the FFI call flow.

use super::native_mapper::NativeMapper;
use super::protocol::Mapper;
use super::sentinel::Sentinel;
use super::types::{FfiValue, MemoryLayout};
use crate::ast::ForeignBinding;
use crate::interpreter::{ForeignFn, RuntimeError, Value};

pub struct Orchestrator {
    mapper: NativeMapper,
    sentinel: Sentinel,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            mapper: NativeMapper,
            sentinel: Sentinel::new(),
        }
    }

    pub fn call(
        &self,
        binding: &ForeignBinding,
        args: Vec<Value>,
        foreign_fn: ForeignFn,
    ) -> Result<Value, RuntimeError> {
        // 1. Convert interpreter values to FFI values
        let ffi_args: Vec<FfiValue> = args
            .iter()
            .map(|v| FfiValue::from_interpreter_value(v))
            .collect();

        // 2. Validate pre-conditions
        self.sentinel
            .validate_precondition(binding, &ffi_args)
            .map_err(|e| RuntimeError::ContractViolation(e))?;

        // 3. Allocate buffer (Metro Pipe)
        let input_layout = binding.input_layout.as_ref().ok_or_else(|| {
            RuntimeError::UndefinedForeignFunction(format!(
                "Missing input layout for {}",
                binding.name
            ))
        })?;
        let output_layout = binding.output_layout.as_ref().ok_or_else(|| {
            RuntimeError::UndefinedForeignFunction(format!(
                "Missing output layout for {}",
                binding.name
            ))
        })?;

        let buffer_size = input_layout.size_bytes.max(output_layout.size_bytes);
        let mut buffer = vec![0u8; buffer_size];

        // 4. Drop input into the pipe
        self.mapper
            .drop(&mut buffer, input_layout, &ffi_args)
            .map_err(|e| RuntimeError::UnhandledOutcome(e))?;

        // 5. Execute foreign function
        // Note: For now, we still use the old ForeignFn signature which takes Vec<Value>.
        // In a true Metro system, we would pass the buffer pointer.
        // We simulate this by passing the buffer as Value::Data.
        let result_value = foreign_fn(vec![Value::Data(buffer)])?;

        // 6. Fetch result from the pipe
        // The foreign function might have written directly to a buffer it received,
        // or returned a new buffer.
        let result_buffer = match result_value {
            Value::Data(d) => d,
            _ => return Ok(result_value), // Fallback for old functions that return direct values
        };

        let ffi_result = self
            .mapper
            .fetch(&result_buffer, output_layout)
            .map_err(|e| RuntimeError::UnhandledOutcome(e))?;

        // 7. Validate post-conditions
        self.sentinel
            .validate_postcondition(binding, &ffi_result)
            .map_err(|e| RuntimeError::ContractViolation(e))?;

        // 8. Convert back to interpreter value
        let mut result_val = ffi_result.to_interpreter_value();

        // 9. Wrap in Result (v2 "logically closed" pattern)
        let error_fields = &binding.error_fields;
        let error_type_name = &binding.error_type;

        if let Value::Instance {
            typename: _,
            mut fields,
        } = result_val
        {
            let mut err_fields = std::collections::HashMap::new();
            let mut has_error = false;

            for (field_name, _) in error_fields {
                if let Some(val) = fields.get(field_name) {
                    if !is_empty_value(val) {
                        err_fields.insert(field_name.clone(), val.clone());
                        has_error = true;
                    }
                }
            }

            if has_error {
                let error_variant =
                    Value::Enum(error_type_name.clone(), error_type_name.clone(), err_fields);

                // Metro v2 pattern: Failure triggers transaction escape
                return Err(RuntimeError::ContractViolation(format!(
                    "FFI Error({}): {:?}",
                    error_type_name, error_variant
                )));
            }

            // If only one success field, return it directly (Extraction pattern)
            if binding.success_output.len() == 1 {
                let first_field = &binding.success_output[0].0;
                if let Some(val) = fields.remove(first_field) {
                    return Ok(val);
                }
            }

            Ok(Value::Instance {
                typename: "Success".to_string(),
                fields,
            })
        } else {
            // If it's a simple value, return it directly
            Ok(result_val)
        }
    }
}

fn is_empty_value(value: &Value) -> bool {
    match value {
        Value::Int(0) => true,
        Value::Float(0.0) => true,
        Value::String(s) => s.is_empty(),
        Value::Bool(false) => true,
        Value::List(l) => l.is_empty(),
        Value::Instance { fields, .. } => fields.is_empty(),
        Value::Void => true,
        Value::Data(d) => d.is_empty(),
        _ => false,
    }
}
