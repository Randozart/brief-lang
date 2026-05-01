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

//! Target Spec Loader
//!
//! Loads TargetSpec from TOML files, searching multiple directories.

use std::path::{Path, PathBuf};

use super::TargetSpec;
use super::LoadError;

/// Searches multiple directories for Target Spec TOML files
pub struct TargetSpecLoader {
    search_paths: Vec<PathBuf>,
}

impl TargetSpecLoader {
    /// Create a new loader with default search paths
    pub fn new() -> Self {
        Self {
            search_paths: vec![
                PathBuf::from("lib/ffi/profiles"),
                PathBuf::from("lib/codegen"),
                PathBuf::from("lib/targets"),
                PathBuf::from("hardware_lib/targets"),
            ],
        }
    }

    /// Add a custom search path
    pub fn add_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Load a TargetSpec from a path or name
    pub fn load(&self, spec_path: &Path) -> Result<TargetSpec, LoadError> {
        // 1. If absolute path, load directly
        if spec_path.is_absolute() {
            return self.load_file(spec_path);
        }

        // 2. Try each search path
        for base in &self.search_paths {
            // Try direct join
            let full_path = base.join(spec_path);
            if full_path.exists() {
                return self.load_file(&full_path);
            }

            // Try with .toml extension
            let with_ext = base.join(format!("{}.toml", spec_path.display()));
            if with_ext.exists() {
                return self.load_file(&with_ext);
            }
        }

        Err(LoadError::SpecNotFound(spec_path.display().to_string()))
    }

    /// Load and parse a TOML file
    fn load_file(&self, path: &Path) -> Result<TargetSpec, LoadError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| LoadError::IoError(e))?;

        toml::from_str(&content)
            .map_err(|e| LoadError::ParseError(e.to_string()))
    }
}

impl Default for TargetSpecLoader {
    fn default() -> Self {
        Self::new()
    }
}
