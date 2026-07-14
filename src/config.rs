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

/// Maps (operation, primitive, bytes) → LLVM IR template.
/// Loaded from config/llvm-ops.toml at compile time.
/// Structure: { op_name: { primitive_name: { bytes: template } } }
/// For operations without a primitive (Malloc, Free, etc.), the primitive level is "_".
#[derive(Debug, Clone)]
pub struct OpConfig {
    op: HashMap<String, HashMap<String, HashMap<String, String>>>,
}

impl OpConfig {
    /// Load the built-in op config file.
    pub fn load() -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/llvm-ops.toml");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
        let raw: HashMap<String, toml::Value> = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e));

        let mut op = HashMap::new();
        for (key, value) in raw {
            if let Some(op_name) = key.strip_prefix("op.") {
                let mut prim_map = HashMap::new();
                if let toml::Value::Table(table) = value {
                    for (prim_or_bytes, bytes_val) in table {
                        if let toml::Value::String(tmpl) = &bytes_val {
                            // Direct: op.Malloc.8 = "template" → no primitive
                            let mut bytes_map = HashMap::new();
                            bytes_map.insert(prim_or_bytes.clone(), tmpl.clone());
                            prim_map.insert("_".to_string(), bytes_map);
                        } else if let toml::Value::Table(bytes_table) = bytes_val {
                            // Nested: op.Add.Int.8 = "template" → has primitive
                            let mut bytes_map = HashMap::new();
                            for (byte_key, byte_val) in bytes_table {
                                if let toml::Value::String(tmpl) = byte_val {
                                    bytes_map.insert(byte_key, tmpl);
                                }
                            }
                            prim_map.insert(prim_or_bytes, bytes_map);
                        }
                    }
                }
                op.insert(op_name.to_string(), prim_map);
            }
        }
        OpConfig { op }
    }

    /// Look up the LLVM IR template for (operation, primitive, bytes).
    /// Tries the specific primitive first, then falls back to "_" for
    /// operations that don't depend on primitive type (Malloc, Free, etc.).
    pub fn lookup(&self, op: &str, primitive: &str, bytes: u64) -> Option<&str> {
        let key = bytes.to_string();
        let result = self.op.get(op).and_then(|prims| {
            prims.get(primitive).or_else(|| prims.get("_"))
                .and_then(|sizes| sizes.get(&key))
        });
        result.map(|s| s.as_str())
    }

    /// Check if an operation is supported for the given type.
    pub fn is_supported(&self, op: &str, primitive: &str, bytes: u64) -> bool {
        self.lookup(op, primitive, bytes).is_some()
    }
}
