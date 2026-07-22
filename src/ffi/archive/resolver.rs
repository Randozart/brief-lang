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

//! FFI Binding Path Resolution
//!
//! Resolves TOML binding file paths from various sources:
//! - Relative to declaring source file
//! - Absolute paths: /path/to/binding.toml
//! - Standard library: std/bindings/io.toml
//! - Project-relative: bindings/custom.toml or ./bindings/custom.toml

use super::FfiError;
use std::path::{Path, PathBuf};

/// Standard library bindings directory
/// This is resolved at runtime based on the compiler's installation location
fn std_lib_path() -> Option<PathBuf> {
    // Try environment variable first (explicit override)
    if let Ok(path) = std::env::var("BRIEF_STDLIB_PATH") {
        let stdlib_path = PathBuf::from(path);
        if stdlib_path.exists() {
            return Some(stdlib_path);
        }
    }

    // Try to find the standard library relative to the running executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // Check for typical installation layouts
            let possible_paths = vec![
                // Development: brief-compiler/target/release/
                exe_dir.join("../../lib/ffi/bindings"),
                exe_dir.join("../lib/ffi/bindings"),
                // Installed: ~/.local/bin/ -> ~/.local/share/brief/ffi/bindings
                exe_dir.join("../share/brief/ffi/bindings"),
            ];

            for path in possible_paths {
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    // Try OS-specific data directory
    if let Some(data_dir) = dirs::data_dir() {
        let brief_data = data_dir.join("brief").join("ffi").join("bindings");
        if brief_data.exists() {
            return Some(brief_data);
        }
    }

    None
}

/// Resolve a binding path to an actual file path
pub fn resolve_binding_path(
    binding_path: &str,
    project_root: &Option<PathBuf>,
    source_file_path: &Option<PathBuf>,
    no_stdlib: bool,
    custom_stdlib_path: &Option<PathBuf>,
) -> Result<PathBuf, FfiError> {
    let binding_path = Path::new(binding_path);

    // Case 0: Relative to the declaring source file
    if let Some(source_path) = source_file_path {
        if let Some(source_dir) = source_path.parent() {
            let resolved = source_dir.join(binding_path);
            if resolved.exists() {
                return Ok(resolved);
            }
        }
    }

    // Case 1: Absolute path
    if binding_path.is_absolute() {
        if binding_path.exists() {
            return Ok(binding_path.to_path_buf());
        } else {
            return Err(FfiError::FileNotFound(binding_path.display().to_string()));
        }
    }

    // Case 2: Standard library binding (std/bindings/*)
    let is_std_path =
        binding_path.starts_with("std/bindings/") || binding_path.starts_with("std\\bindings\\");

    if is_std_path {
        if no_stdlib {
            return Err(FfiError::FileNotFound(format!(
                "{} (standard library disabled)",
                binding_path.display()
            )));
        }

        // Strip the std/bindings/ prefix to get just the filename
        let stripped = if let Ok(stripped) = binding_path.strip_prefix("std/bindings/") {
            Some(stripped)
        } else if let Ok(stripped) = binding_path.strip_prefix("std\\bindings\\") {
            Some(stripped)
        } else {
            None
        };

        if let Some(filename) = stripped {
            // Try custom stdlib path first
            if let Some(custom_path) = custom_stdlib_path {
                let resolved = custom_path.join(filename);
                if resolved.exists() {
                    return Ok(resolved);
                }
            }

            // Try standard library path
            if let Some(stdlib_dir) = std_lib_path() {
                let resolved = stdlib_dir.join(filename);
                if resolved.exists() {
                    return Ok(resolved);
                }
            }

            // Try relative to current directory (for development)
            if binding_path.exists() {
                return Ok(binding_path.to_path_buf());
            }

            // Try with ./ prefix
            let with_dot = PathBuf::from("./").join(binding_path);
            if with_dot.exists() {
                return Ok(with_dot);
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
        let result = resolve_binding_path("/tmp/nonexistent.toml", &None, &None, false, &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_relative_path_nonexistent() {
        let result = resolve_binding_path("bindings/nonexistent.toml", &None, &None, false, &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_std_binding() {
        // This might succeed or fail depending on working directory
        // The important thing is it doesn't panic
        let _ = resolve_binding_path("std/bindings/io.toml", &None, &None, false, &None);
    }
}
