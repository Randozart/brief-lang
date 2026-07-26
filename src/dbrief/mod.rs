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

pub mod ast;
pub mod parser;
pub mod v2;
pub mod bridge;

pub use ast::*;
pub use parser::{parse_dbrief, parse_dbvl};
pub use v2::*;

/// Backward-compat JSON serialization for CLI — will be phased out
pub fn dbvl_to_json(program: &DbvlProgram, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    } else {
        serde_json::to_string(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }
}

pub fn dbrief_to_json(program: &ast::DbriefProgram, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    } else {
        serde_json::to_string(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }
}