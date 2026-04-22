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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Cache directory error: {0}")]
    DirectoryError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCache {
    pub source_hash: String,
    pub interface_hash: String,
    pub ast_valid: bool,
    pub types_valid: bool,
    pub proofs_valid: bool,
    pub last_modified: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleCache {
    pub source_hash: String,
    pub interface_hash: String,
    pub ast: Option<String>,
    pub types: Option<String>,
    pub proofs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheManifest {
    pub version: String,
    pub files: HashMap<String, FileCache>,
    pub interface_hashes: HashMap<String, String>,
}

pub struct CacheManager {
    cache_dir: PathBuf,
    manifest: CacheManifest,
}

impl CacheManager {
    pub fn new(cache_dir: PathBuf) -> Result<Self, CacheError> {
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)?;
        }

        let modules_dir = cache_dir.join("modules");
        if !modules_dir.exists() {
            fs::create_dir_all(&modules_dir)?;
        }

        let manifest_path = cache_dir.join("manifest.json");
        let manifest = if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            CacheManifest::default()
        };

        Ok(CacheManager {
            cache_dir,
            manifest,
        })
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn source_hash(&self, source: &str) -> String {
        let hash = blake3::hash(source.as_bytes());
        hash.to_hex().to_string()
    }

    pub fn interface_hash(&self, exports: &str) -> String {
        let hash = blake3::hash(exports.as_bytes());
        hash.to_hex().to_string()
    }

    pub fn is_file_cache_valid(&self, file_path: &str, current_source: &str) -> bool {
        let current_hash = self.source_hash(current_source);
        if let Some(cache) = self.manifest.files.get(file_path) {
            cache.source_hash == current_hash
        } else {
            false
        }
    }

    pub fn is_interface_changed(&self, file_path: &str, new_interface_hash: &str) -> bool {
        if let Some(cached) = self.manifest.interface_hashes.get(file_path) {
            cached != new_interface_hash
        } else {
            true
        }
    }

    pub fn update_file_cache(&mut self, file_path: String, source: &str, interface_hash: String) {
        let source_hash = self.source_hash(source);

        let file_cache = FileCache {
            source_hash: source_hash.clone(),
            interface_hash: interface_hash.clone(),
            ast_valid: true,
            types_valid: true,
            proofs_valid: true,
            last_modified: std::time::SystemTime::now(),
        };

        self.manifest.files.insert(file_path.clone(), file_cache);
        self.manifest
            .interface_hashes
            .insert(file_path, interface_hash);
    }

    pub fn invalidate_file(&mut self, file_path: &str) {
        if let Some(cache) = self.manifest.files.get_mut(file_path) {
            cache.ast_valid = false;
            cache.types_valid = false;
            cache.proofs_valid = false;
        }
    }

    pub fn invalidate_importers(&mut self, importers: &[String]) {
        for importer in importers {
            self.invalidate_file(importer);
        }
    }

    pub fn get_module_cache(&self, name: &str) -> Option<ModuleCache> {
        let path = self
            .cache_dir
            .join("modules")
            .join(format!("{}.json", name));
        if path.exists() {
            let content = fs::read_to_string(&path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    pub fn save_module_cache(&self, name: &str, cache: &ModuleCache) -> Result<(), CacheError> {
        let path = self
            .cache_dir
            .join("modules")
            .join(format!("{}.json", name));
        let content = serde_json::to_string_pretty(cache)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), CacheError> {
        let manifest_path = self.cache_dir.join("manifest.json");
        let content = serde_json::to_string_pretty(&self.manifest)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        fs::write(&manifest_path, content)?;
        Ok(())
    }

    pub fn get_dependents(&self, file_path: &str) -> Vec<String> {
        self.manifest
            .files
            .iter()
            .filter(|(k, _)| *k != file_path)
            .filter(|(_, cache)| self.is_interface_changed(file_path, &cache.interface_hash))
            .map(|(k, _)| k.clone())
            .collect()
    }

    pub fn clear(&mut self) {
        self.manifest = CacheManifest::default();
        let modules_dir = self.cache_dir.join("modules");
        if modules_dir.exists() {
            let _ = fs::remove_dir_all(&modules_dir);
            let _ = fs::create_dir_all(&modules_dir);
        }
    }

    pub fn manifest(&self) -> &CacheManifest {
        &self.manifest
    }
}

impl Default for CacheManifest {
    fn default() -> Self {
        CacheManifest {
            version: env!("CARGO_PKG_VERSION").to_string(),
            files: HashMap::new(),
            interface_hashes: HashMap::new(),
        }
    }
}

pub struct InterfaceHasher {
    exports: Vec<String>,
    signatures: Vec<String>,
    types: Vec<String>,
}

impl InterfaceHasher {
    pub fn new() -> Self {
        InterfaceHasher {
            exports: Vec::new(),
            signatures: Vec::new(),
            types: Vec::new(),
        }
    }

    pub fn add_export(&mut self, name: &str) {
        self.exports.push(name.to_string());
    }

    pub fn add_signature(&mut self, sig: &str) {
        self.signatures.push(sig.to_string());
    }

    pub fn add_type(&mut self, ty: &str) {
        self.types.push(ty.to_string());
    }

    pub fn compute_hash(&self) -> String {
        let mut combined = self.exports.join("\n");
        combined.push('\n');
        combined.push_str(&self.signatures.join("\n"));
        combined.push('\n');
        combined.push_str(&self.types.join("\n"));

        blake3::hash(combined.as_bytes()).to_hex().to_string()
    }
}

impl Default for InterfaceHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_source_hashing() {
        let tmp = TempDir::new().unwrap();
        let cache = CacheManager::new(tmp.path().to_path_buf()).unwrap();

        let hash1 = cache.source_hash("let x = 1;");
        let hash2 = cache.source_hash("let x = 1;");
        let hash3 = cache.source_hash("let x = 2;");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_cache_validity() {
        let tmp = TempDir::new().unwrap();
        let mut cache = CacheManager::new(tmp.path().to_path_buf()).unwrap();

        let source = "let x = 1;";
        assert!(!cache.is_file_cache_valid("test.bv", source));

        cache.update_file_cache("test.bv".to_string(), source, "iface1".to_string());
        assert!(cache.is_file_cache_valid("test.bv", source));

        let new_source = "let x = 2;";
        assert!(!cache.is_file_cache_valid("test.bv", new_source));
    }

    #[test]
    fn test_interface_change_detection() {
        let tmp = TempDir::new().unwrap();
        let mut cache = CacheManager::new(tmp.path().to_path_buf()).unwrap();

        cache.update_file_cache("test.bv".to_string(), "source", "iface_v1".to_string());

        assert!(!cache.is_interface_changed("test.bv", "iface_v1"));
        assert!(cache.is_interface_changed("test.bv", "iface_v2"));
    }
}
