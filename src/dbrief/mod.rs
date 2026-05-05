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
pub use parser::parse_dbrief;
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