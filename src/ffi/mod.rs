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
pub mod registry;
pub mod resolver;
pub mod types;
pub mod validator;

pub use loader::load_binding;
pub use mapper::{create_mapper_registry, find_mapper};
pub use mappers::{MapperInfo, MapperRegistry, MapperType};
pub use registry::{FunctionRegistry, FFI_REGISTRY};
pub use resolver::resolve_binding_path;
pub use types::*;
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
) -> Result<Vec<ForeignBinding>, FfiError> {
    // Resolve the path
    let resolved_path = resolver::resolve_binding_path(path, project_root)?;

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
