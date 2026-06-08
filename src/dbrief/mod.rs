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
pub use parser::{parse_dbrief, parse_dbvs, parse_dbvl};
pub use v2::*;

// Backward-compat aliases for hardware_validator and main.rs — will be phased out
pub use ast::{DbriefType, DbriefAddress, DbriefAlias, DbriefProgram, DbriefRegister,
    DbriefStruct, DbriefEnum, DbriefLiteral, DbriefRecord, DbvlRecord, DbvlProgram, ImportStmt,
    DbriefExpr, DbriefContract, DbvsProgram};

pub fn compile_dbvs(input: &str) -> Result<ast::DbvsProgram, String> {
    parse_dbvs(input)
}

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

pub fn dbvs_to_json(program: &DbvsProgram, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    } else {
        serde_json::to_string(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }
}

pub fn dbrief_to_json(program: &DbriefProgram, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    } else {
        serde_json::to_string(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }
}

/// Backward-compat engine for hardware_validator
pub struct DbvsEngine {
    program: DbvsProgram,
}

impl DbvsEngine {
    pub fn new(program: DbvsProgram) -> Self {
        DbvsEngine { program }
    }

    pub fn get_alias(&self, name: &str) -> Option<&DbriefAlias> {
        self.program.aliases.iter().find(|a| a.name == name)
    }

    pub fn get_struct(&self, name: &str) -> Option<&DbriefStruct> {
        self.program.structs.iter().find(|s| s.name == name)
    }

    pub fn get_enum(&self, name: &str) -> Option<&DbriefEnum> {
        self.program.enums.iter().find(|e| e.name == name)
    }
}