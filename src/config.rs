// ── Type Config — Source-Driven LLVM Type Mappings ──────────────────────
// 2026-07-14: Reads (primitive, bytes) → LLVM type string from
// config/llvm-primitives.toml. Zero hardcoded Rust match arms.
// Every type's LLVM representation is derived from source metadata.

use std::collections::HashMap;
use std::path::Path;

/// Maps (primitive name, bytes) → LLVM type string.
/// Loaded from config/llvm-primitives.toml at compile time.
/// Inner keys are TOML bare-key integers (stored as String, parsed at lookup).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TypeConfig {
    primitive: HashMap<String, HashMap<String, String>>,
}

impl TypeConfig {
    /// Load the built-in config file.
    pub fn load() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/llvm-primitives.toml");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
        toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
    }

    /// Look up the LLVM type string for (primitive, bytes).
    /// Returns None if no mapping exists.
    pub fn lookup(&self, primitive: &str, bytes: u64) -> Option<&str> {
        let key = bytes.to_string();
        self.primitive
            .get(primitive)
            .and_then(|sizes| sizes.get(&key))
            .map(|s| s.as_str())
    }
}

/// Pure function: derive LLVM type string from (primitive, bytes).
/// Fallback is i{N*8} for raw Bits(N).
pub fn derive_llvm_type(primitive: Option<&str>, bytes: u64, config: &TypeConfig) -> String {
    if let Some(prim) = primitive {
        if let Some(llvm_ty) = config.lookup(prim, bytes) {
            return llvm_ty.to_string();
        }
    }
    format!("i{}", bytes * 8)
}
