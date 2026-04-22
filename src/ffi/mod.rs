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

//! FFI System Coordinator
//!
//! Coordinates all Foreign Function Interface components:
//! - TOML binding file loading
//! - Binding validation
//! - Path resolution
//! - Type mapping
//! - Function registry

pub mod loader;
pub mod mapper;
pub mod mappers;
pub mod native_mapper;
pub mod orchestrator;
pub mod protocol;
pub mod registry;
pub mod resolver;
pub mod sentinel;
pub mod types;
pub mod validator;

pub use loader::load_binding;
pub use mapper::{create_mapper_registry, find_mapper};
pub use mappers::{MapperInfo, MapperRegistry, MapperType};
pub use native_mapper::NativeMapper;
pub use orchestrator::Orchestrator;
pub use protocol::Mapper;
pub use registry::{FunctionRegistry, FFI_REGISTRY};
pub use resolver::resolve_binding_path;
pub use sentinel::Sentinel;
pub use types::*;
pub use types::{FfiValue, MemoryLayout};
pub use validator::validate_frgn_against_binding;

use crate::ast::ForeignBinding;
use std::path::PathBuf;

/// Error types for FFI operations
#[derive(Debug, Clone)]
pub enum FfiError {
    /// File not found
    FileNotFound(String),

    /// Invalid TOML syntax
    TomlParseError(String),

    /// Missing required field in TOML
    MissingField(String),

    /// Type parsing error
    TypeParseError(String),

    /// Binding validation failed
    ValidationError(String),

    /// Path resolution failed
    PathResolutionError(String),

    /// Mapper not found
    MapperNotFound(String),
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfiError::FileNotFound(path) => write!(f, "FFI binding file not found: {}", path),
            FfiError::TomlParseError(err) => write!(f, "TOML parse error: {}", err),
            FfiError::MissingField(field) => write!(f, "Missing required field in TOML: {}", field),
            FfiError::TypeParseError(err) => write!(f, "Type parse error: {}", err),
            FfiError::ValidationError(err) => write!(f, "Binding validation error: {}", err),
            FfiError::PathResolutionError(err) => write!(f, "Path resolution error: {}", err),
            FfiError::MapperNotFound(name) => write!(f, "Mapper not found: {}", name),
        }
    }
}

impl std::error::Error for FfiError {}

/// Main entry point: Load and parse a TOML binding file
pub fn load_binding_file(
    path: &str,
    project_root: &Option<PathBuf>,
    source_file_path: &Option<PathBuf>,
    no_stdlib: bool,
    custom_stdlib_path: &Option<PathBuf>,
) -> Result<Vec<ForeignBinding>, FfiError> {
    // Resolve the path
    let resolved_path = resolver::resolve_binding_path(
        path,
        project_root,
        source_file_path,
        no_stdlib,
        custom_stdlib_path,
    )?;

    // Load and parse TOML
    loader::load_binding(&resolved_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_error_display() {
        let err = FfiError::FileNotFound("test.toml".to_string());
        assert!(err.to_string().contains("not found"));
    }
}
