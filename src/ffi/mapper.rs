//! FFI Mapper Integration
//!
//! Provides the bridge between FFI bindings and the mapper registry.

use super::mappers::{MapperInfo, MapperRegistry, MapperType};

/// Find a mapper for a given binding
///
/// Uses the mapper name and optional path from the binding to locate the appropriate mapper.
///
/// # Arguments
/// * `mapper_name` - The name of the mapper (e.g., "rust", "c", "wasm")
/// * `custom_path` - Optional explicit path to the mapper
/// * `registry` - The mapper registry to search
///
/// # Returns
/// * `Some(MapperInfo)` if mapper found
/// * `None` if no mapper found
pub fn find_mapper(
    mapper_name: &str,
    custom_path: Option<&str>,
    registry: &MapperRegistry,
) -> Option<MapperInfo> {
    registry.find_mapper(mapper_name, custom_path)
}

/// Create a new mapper registry with default search paths
pub fn create_mapper_registry() -> MapperRegistry {
    MapperRegistry::new()
}

/// Load all default mappers into the registry
pub fn load_default_mappers(registry: &mut MapperRegistry) {
    // The registry is pre-configured with default search paths
    // Additional default mappers can be registered here if needed
}

/// Get mapper type description
pub fn describe_mapper_type(info: &MapperInfo) -> &'static str {
    match info.mapper_type {
        MapperType::Brief => "Brief mapper (.bv file)",
        MapperType::Rust => "Rust mapper (Cargo crate)",
    }
}

/// Resolve mapper path for a given binding
pub fn resolve_mapper_path(
    binding_mapper: &Option<String>,
    binding_path: &Option<String>,
    registry: &MapperRegistry,
) -> Result<MapperInfo, super::FfiError> {
    // Use explicit path if provided
    if let Some(path) = binding_path {
        let info = registry.find_mapper(binding_mapper.as_deref().unwrap_or("rust"), Some(path));
        return info.ok_or_else(|| {
            super::FfiError::MapperNotFound(format!("Explicit path mapper not found: {}", path))
        });
    }

    // Otherwise use mapper name
    let mapper_name = binding_mapper
        .as_ref()
        .ok_or_else(|| super::FfiError::MissingField("mapper".to_string()))?;

    let info = registry.find_mapper(mapper_name, None);

    info.ok_or_else(|| {
        super::FfiError::MapperNotFound(format!(
            "Mapper not found: {} (searched lib/mappers/ and lib/ffi/mappers/)",
            mapper_name
        ))
    })
}
