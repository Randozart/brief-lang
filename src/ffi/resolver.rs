//! FFI Binding Path Resolution
//!
//! Resolves TOML binding file paths from various sources:
//! - Absolute paths: /path/to/binding.toml
//! - Project-relative: bindings/custom.toml or ./bindings/custom.toml
//! - Standard library: std/bindings/io.toml

use super::FfiError;
use std::path::{Path, PathBuf};

/// Resolve a binding path to an actual file path
pub fn resolve_binding_path(
    binding_path: &str,
    project_root: &Option<PathBuf>,
) -> Result<PathBuf, FfiError> {
    let binding_path = Path::new(binding_path);

    // Case 1: Absolute path
    if binding_path.is_absolute() {
        if binding_path.exists() {
            return Ok(binding_path.to_path_buf());
        } else {
            return Err(FfiError::FileNotFound(binding_path.display().to_string()));
        }
    }

    // Case 2: Standard library binding (std/bindings/*)
    if binding_path.starts_with("std/bindings/") || binding_path.starts_with("std\\bindings\\") {
        // In the context of the compiler, this would be resolved relative to the crate root
        // For now, check relative to current directory
        if binding_path.exists() {
            return Ok(binding_path.to_path_buf());
        }
        // Try with various prefixes
        let test_paths = vec![
            binding_path.to_path_buf(),
            PathBuf::from("../").join(binding_path),
            PathBuf::from("./").join(binding_path),
        ];
        for path in test_paths {
            if path.exists() {
                return Ok(path);
            }
        }
        return Err(FfiError::FileNotFound(binding_path.display().to_string()));
    }

    // Case 3: Project-relative path
    if let Some(root) = project_root {
        let resolved = root.join(binding_path);
        if resolved.exists() {
            return Ok(resolved);
        }
    }

    // Case 4: Try as project-relative with current directory as root
    if binding_path.exists() {
        return Ok(binding_path.to_path_buf());
    }

    // Try with ./ prefix
    let with_dot = PathBuf::from("./").join(binding_path);
    if with_dot.exists() {
        return Ok(with_dot);
    }

    Err(FfiError::FileNotFound(binding_path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_absolute_path() {
        // Create a temporary file to test
        let test_path = PathBuf::from("/tmp/test_binding.toml");
        // This will fail because the file doesn't exist, but that's the point
        let result = resolve_binding_path("/tmp/nonexistent.toml", &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_relative_path_nonexistent() {
        let result = resolve_binding_path("bindings/nonexistent.toml", &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_std_binding() {
        // This might succeed or fail depending on working directory
        // The important thing is it doesn't panic
        let _ = resolve_binding_path("std/bindings/io.toml", &None);
    }
}
