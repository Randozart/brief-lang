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

//! Target Spec TOML system for universal transpilation adapter.
//!
//! Defines the TargetSpec struct and related types that drive both FFI
//! call generation and code generation from declarative TOML files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod loader;

/// Main TargetSpec struct - can contain FFI, Codegen, or both
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetSpec {
    #[serde(default)]
    pub ffi: Option<FfiSection>,
    #[serde(default)]
    pub codegen: Option<CodegenSection>,
}

/// FFI section from TOML profile
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FfiSection {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub language: LanguageMeta,
    #[serde(default)]
    pub types: HashMap<String, TypeDef>,
    #[serde(default)]
    pub mapping: HashMap<String, String>,
    #[serde(default)]
    pub conventions: Conventions,
    #[serde(default)]
    pub overrides: Vec<TypeOverride>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LanguageMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub endianness: String,
    #[serde(default, rename = "pointer_size")]
    pub pointer_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TypeDef {
    #[serde(default)]
    pub representation: String,
    #[serde(default)]
    pub size: usize,
    #[serde(default)]
    pub signed: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Conventions {
    #[serde(default)]
    pub alignment: usize,
    #[serde(default, rename = "call_conv")]
    pub call_conv: String,
    #[serde(default, rename = "pointer_size")]
    pub pointer_size: usize,
    #[serde(default)]
    pub error: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TypeOverride {
    pub name: String,
    pub value: String,
}

/// Codegen section from TOML profile
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodegenSection {
    pub backend: String,
    #[serde(default)]
    pub extension: String,
    #[serde(default, rename = "state_allocation")]
    pub state_allocation: StateAllocation,
    #[serde(default)]
    pub templates: CodegenTemplates,
    #[serde(default, rename = "entry_point")]
    pub entry_point: EntryPointConfig,
    #[serde(default)]
    pub validation: HashMap<String, String>,
    #[serde(default)]
    pub inference: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StateAllocation {
    Static,
    Dynamic,
    ReachParticipant,
    WasmMemory,
    #[serde(other)]
    Other,
}

impl Default for StateAllocation {
    fn default() -> Self {
        StateAllocation::Other
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CodegenTemplates {
    #[serde(default)]
    pub header: Option<String>,
    #[serde(default)]
    pub footer: Option<String>,
    #[serde(default, rename = "start_template")]
    pub start_template: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EntryPointConfig {
    #[serde(default)]
    pub style: String,
    #[serde(default, rename = "init_txn")]
    pub init_txn: Option<String>,
    #[serde(default, rename = "exit_txn")]
    pub exit_txn: Option<String>,
}

impl TargetSpec {
    /// Validate program against this spec's validation rules
    pub fn validate_program(&self, _program: &crate::ast::Program) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        // Check for blocked types in [codegen.validation]
        if let Some(codegen) = &self.codegen {
            for (type_name, rule) in &codegen.validation {
                if rule.starts_with("error:") {
                    // In a real implementation, check if the program uses this type
                    // For now, just record that validation is active
                    errors.push(format!(
                        "Type '{}' validation rule: {}", 
                        type_name, rule
                    ));
                }
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Get the inference rule for a given key
    pub fn inference(&self, key: &str) -> Option<&str> {
        self.codegen.as_ref()
            .and_then(|c| c.inference.get(key))
            .map(|s| s.as_str())
    }
}

#[derive(Debug)]
pub enum LoadError {
    SpecNotFound(String),
    IoError(std::io::Error),
    ParseError(String),
}

impl From<std::io::Error> for LoadError {
    fn from(err: std::io::Error) -> Self {
        LoadError::IoError(err)
    }
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::SpecNotFound(path) => write!(f, "Spec not found: {}", path),
            LoadError::IoError(e) => write!(f, "IO error: {}", e),
            LoadError::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for LoadError {}
