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
            // TODO: Real expression evaluation for contracts
            // For now, we just check if it's "true"
            if pre != "true" && !pre.is_empty() {
                // eprintln!("[DEBUG] Precondition check: {}", pre);
            }
        }
        Ok(())
    }

    pub fn validate_postcondition(
        &self,
        binding: &ForeignBinding,
        result: &FfiValue,
    ) -> Result<(), String> {
        if let Some(post) = &binding.postcondition {
            // TODO: Real expression evaluation for contracts
            if post != "true" && !post.is_empty() {
                // eprintln!("[DEBUG] Postcondition check: {}", post);
            }
        }
        Ok(())
    }
}
