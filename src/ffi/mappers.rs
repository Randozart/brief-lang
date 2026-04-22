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

//! FFI Mapper Module
//!
//! Handles mapper discovery and loading for the FFI system.
//! Mappers translate between foreign language types and Brief types.

use std::collections::HashMap;
use std::path::PathBuf;

/// Information about a discovered mapper
#[derive(Debug, Clone)]
pub struct MapperInfo {
    pub name: String,
    pub path: PathBuf,
    pub mapper_type: MapperType,
}

/// Type of mapper
#[derive(Debug, Clone, PartialEq)]
pub enum MapperType {
    Brief,
    Rust,
}

/// Mapper registry for discovering mappers
pub struct MapperRegistry {
    /// Registered mappers
    mappers: HashMap<String, MapperInfo>,

    /// Search paths in priority order
    search_paths: Vec<PathBuf>,
}

impl MapperRegistry {
    /// Create new registry with default search paths
    pub fn new() -> Self {
        let mut registry = MapperRegistry {
            mappers: HashMap::new(),
            search_paths: Vec::new(),
        };

        // Default search paths
        registry.add_search_path(PathBuf::from("lib/mappers"));
        registry.add_search_path(PathBuf::from("lib/ffi/mappers"));

        registry
    }

    /// Add a search path
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Find mapper by name, with optional explicit path override
    ///
    /// Search order:
    /// 1. If custom_path provided, use that exact path
    /// 2. Search lib/mappers/<name>/
    /// 3. Search lib/ffi/mappers/<name>/
    pub fn find_mapper(&self, name: &str, custom_path: Option<&str>) -> Option<MapperInfo> {
        // Use explicit path if provided
        if let Some(path) = custom_path {
            let path = PathBuf::from(path);
            if path.exists() || path.extension().is_some() {
                let mapper_type = self.detect_mapper_type(&path);
                return Some(MapperInfo {
                    name: name.to_string(),
                    path,
                    mapper_type,
                });
            }
            return None;
        }

        // Search default paths
        for base in &self.search_paths {
            // Check directory form: lib/mappers/rust/
            let dir_path = base.join(name);
            if dir_path.exists() {
                return Some(MapperInfo {
                    name: name.to_string(),
                    path: dir_path,
                    mapper_type: MapperType::Brief,
                });
            }

            // Check file form: lib/mappers/rust_mapper.bv
            let file_path = base.join(format!("{}_mapper.bv", name));
            if file_path.exists() {
                return Some(MapperInfo {
                    name: name.to_string(),
                    path: file_path,
                    mapper_type: MapperType::Brief,
                });
            }

            // Check direct .bv: lib/mappers/rust.bv
            let direct_path = base.join(format!("{}.bv", name));
            if direct_path.exists() {
                return Some(MapperInfo {
                    name: name.to_string(),
                    path: direct_path,
                    mapper_type: MapperType::Brief,
                });
            }
        }

        None
    }

    /// Detect mapper type from path
    fn detect_mapper_type(&self, path: &PathBuf) -> MapperType {
        if path.extension().map(|e| e == "bv").unwrap_or(false) {
            return MapperType::Brief;
        }
        if path.join("Cargo.toml").exists() {
            return MapperType::Rust;
        }
        if path.is_dir() {
            // Check for .bv files inside
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if entry.path().extension().map(|e| e == "bv").unwrap_or(false) {
                        return MapperType::Brief;
                    }
                }
            }
        }
        MapperType::Brief
    }

    /// Register a mapper manually
    pub fn register(&mut self, info: MapperInfo) {
        self.mappers.insert(info.name.clone(), info);
    }

    /// Get all registered mappers
    pub fn all_mappers(&self) -> Vec<&MapperInfo> {
        self.mappers.values().collect()
    }
}

impl Default for MapperRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the global mapper registry
pub fn create_registry() -> MapperRegistry {
    MapperRegistry::new()
}

/// Errors from mapper operations
#[derive(Debug)]
pub enum MapperError {
    NotFound(String),
    InvalidPath(String),
    LoadFailed(String),
}

impl std::fmt::Display for MapperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "Mapper not found: {}", name),
            Self::InvalidPath(path) => write!(f, "Invalid mapper path: {}", path),
            Self::LoadFailed(msg) => write!(f, "Failed to load mapper: {}", msg),
        }
    }
}
