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
pub mod eval;
pub mod alloc;

pub use ast::*;
pub use parser::{parse_dbrief, parse_dbvs, parse_dbvl};
pub use eval::*;
pub use alloc::*;

use std::collections::HashMap;

pub struct DbriefEngine {
    program: DbriefProgram,
    records: HashMap<DbriefAddress, DbriefRecord>,
}

impl DbriefEngine {
    pub fn new(program: DbriefProgram) -> Self {
        let mut records = HashMap::new();
        for record in &program.records {
            records.insert(record.address.clone(), record.clone());
        }
        DbriefEngine { program, records }
    }

    pub fn query(&self, addr: &DbriefAddress) -> Option<&DbriefRecord> {
        self.records.get(addr)
    }

    pub fn verify_contracts(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        for check in &self.program.checks {
            for cond in &check.conditions {
                if !self.eval_condition(cond) {
                    errors.push("Contract violation".to_string());
                }
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn eval_condition(&self, _expr: &DbriefExpr) -> bool {
        true
    }

    pub fn resolve_alias(&self, name: &str) -> Option<&DbriefAddress> {
        for alias in &self.program.aliases {
            if alias.name == name {
                return alias.address.as_ref();
            }
        }
        None
    }
}

pub fn compile_dbrief(input: &str) -> Result<DbriefEngine, String> {
    let program = parse_dbrief(input)?;
    Ok(DbriefEngine::new(program))
}

/// Schema engine for .dbvs files - provides template definitions for hardware
pub struct DbvsEngine {
    pub program: DbvsProgram,
}

impl DbvsEngine {
    pub fn new(program: DbvsProgram) -> Self {
        DbvsEngine { program }
    }

    pub fn get_register(&self, name: &str) -> Option<&DbriefRegister> {
        self.program.registers.iter().find(|r| {
            if let Some(n) = &r.name {
                n == name
            } else {
                false
            }
        })
    }

    pub fn get_struct(&self, name: &str) -> Option<&DbriefStruct> {
        self.program.structs.iter().find(|s| s.name == name)
    }

    pub fn get_enum(&self, name: &str) -> Option<&DbriefEnum> {
        self.program.enums.iter().find(|e| e.name == name)
    }

    pub fn get_alias(&self, name: &str) -> Option<&DbriefAlias> {
        self.program.aliases.iter().find(|a| a.name == name)
    }
}

pub fn compile_dbvs(input: &str) -> Result<DbvsEngine, String> {
    let program = parse_dbvs(input)?;
    Ok(DbvsEngine::new(program))
}

/// Mutable database engine for .dbvl files - line-based mutable records
pub struct DbvlEngine {
    pub program: DbvlProgram,
    records: HashMap<DbriefAddress, DbvlRecord>,
}

impl DbvlEngine {
    pub fn new(program: DbvlProgram) -> Self {
        let mut records = HashMap::new();
        for record in &program.records {
            records.insert(record.address.clone(), record.clone());
        }
        DbvlEngine { program, records }
    }

    pub fn insert(&mut self, address: DbriefAddress, fields: Vec<(String, DbriefLiteral)>) {
        let record = DbvlRecord { address: address.clone(), fields };
        self.records.insert(address, record);
    }

    pub fn update(&mut self, address: &DbriefAddress, fields: Vec<(String, DbriefLiteral)>) {
        if let Some(record) = self.records.get_mut(address) {
            for (key, value) in fields {
                if let Some(existing) = record.fields.iter_mut().find(|(n, _)| n == &key) {
                    *existing = (key, value);
                } else {
                    record.fields.push((key, value));
                }
            }
        }
    }

    pub fn delete(&mut self, address: &DbriefAddress) {
        self.records.remove(address);
    }

    pub fn get(&self, address: &DbriefAddress) -> Option<&DbvlRecord> {
        self.records.get(address)
    }

    pub fn all_records(&self) -> Vec<&DbvlRecord> {
        self.records.values().collect()
    }
}

pub fn compile_dbvl(input: &str) -> Result<DbvlEngine, String> {
    let program = parse_dbvl(input)?;
    Ok(DbvlEngine::new(program))
}

/// Serialize a DbvlProgram to JSON
pub fn dbvl_to_json(program: &DbvlProgram, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    } else {
        serde_json::to_string(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }
}

/// Serialize a DbriefProgram to JSON (full program with schema + data)
pub fn dbrief_to_json(program: &DbriefProgram, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    } else {
        serde_json::to_string(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }
}

/// Serialize a DbvsProgram to JSON (schema only)
pub fn dbvs_to_json(program: &DbvsProgram, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    } else {
        serde_json::to_string(program)
            .map_err(|e| format!("JSON serialization error: {}", e))
    }
}