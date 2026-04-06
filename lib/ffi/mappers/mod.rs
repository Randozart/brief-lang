//! Mapper Registry
//!
//! Discovers and loads mappers from:
//! 1. lib/mappers/<name>/ (user-defined)
//! 2. lib/ffi/mappers/<name>/ (default mappers)
//!
//! Mappers can be:
//! - Brief files (.bv)
//! - Rust crates

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// Mapper registry for discovering and managing mappers
pub struct MapperRegistry {
    mappers: HashMap<String, MapperInfo>,
    search_paths: Vec<PathBuf>,
}

impl MapperRegistry {
    /// Create a new registry with default search paths
    pub fn new() -> Self {
        let mut registry = MapperRegistry {
            mappers: HashMap::new(),
            search_paths: Vec::new(),
        };

        // Set up default search paths
        registry.add_search_path("lib/mappers".into());
        registry.add_search_path("lib/ffi/mappers".into());

        registry
    }

    /// Add a search path
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Find a mapper by name and optional explicit path
    ///
    /// If `custom_path` is provided, uses that exact path.
    /// Otherwise searches in order:
    /// 1. lib/mappers/<name>/
    /// 2. lib/ffi/mappers/<name>/
    pub fn find_mapper(&self, name: &str, custom_path: Option<&str>) -> Option<MapperInfo> {
        // If explicit path provided, use it directly
        if let Some(path) = custom_path {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(MapperInfo {
                    name: name.to_string(),
                    path,
                    mapper_type: self.detect_mapper_type(&path),
                });
            }
            return None;
        }

        // Search default paths in order
        for base_path in &self.search_paths {
            let mapper_path = base_path.join(name);

            // Check if directory exists
            if mapper_path.is_dir() {
                return Some(MapperInfo {
                    name: name.to_string(),
                    path: mapper_path,
                    mapper_type: self.detect_mapper_type(&mapper_path),
                });
            }

            // Also check for single file (e.g., rust_mapper.bv)
            let single_file = base_path.join(format!("{}.bv", name));
            if single_file.exists() {
                return Some(MapperInfo {
                    name: name.to_string(),
                    path: single_file,
                    mapper_type: MapperType::Brief,
                });
            }
        }

        None
    }

    /// Detect the type of mapper based on files present
    fn detect_mapper_type(&self, path: &Path) -> MapperType {
        // Check for Rust files
        if path.join("Cargo.toml").exists() {
            return MapperType::Rust;
        }

        // Check for Brief files
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "bv" {
                        return MapperType::Brief;
                    }
                }
            }
        }

        // Default to Brief for .bv files at root level
        if let Some(ext) = path.extension() {
            if ext == "bv" {
                return MapperType::Brief;
            }
        }

        // Default to Brief
        MapperType::Brief
    }

    /// Register a mapper explicitly
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

/// Errors that can occur during mapper discovery
#[derive(Debug)]
pub enum MapperError {
    NotFound(String),
    InvalidPath(String),
    LoadError(String),
}

impl std::fmt::Display for MapperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapperError::NotFound(name) => write!(f, "Mapper not found: {}", name),
            MapperError::InvalidPath(path) => write!(f, "Invalid mapper path: {}", path),
            MapperError::LoadError(msg) => write!(f, "Failed to load mapper: {}", msg),
        }
    }
}
