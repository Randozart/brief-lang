// ── Type Config — Source-Driven LLVM Type Mappings ──────────────────────
// 2026-07-17: Reads (ctd, bytes) → LLVM type string from
// config/ctd-llvm-mappings.toml.
// CTD replaces the old `primitive` property. Every type's LLVM representation
// is derived from CTD metadata.

use std::collections::HashMap;
use std::path::Path;

/// Maps (ctd, bytes) → LLVM type string.
/// Loaded from config/ctd-llvm-mappings.toml at compile time.
/// Inner keys are TOML bare-key integers (stored as String, parsed at lookup).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TypeConfig {
    ctd: HashMap<String, HashMap<String, String>>,
}

impl TypeConfig {
    /// Load the built-in config file (compile-time baked fallback).
    pub fn load() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/ctd-llvm-mappings.toml");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
        toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path.display(), e))
    }

    /// 2026-07-16: P1 — load from a concrete file path.
    pub fn load_from(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;
        toml::from_str(&content)
            .map_err(|e| format!("parse error in '{}': {}", path.display(), e))
    }

    /// Look up the LLVM type string for (ctd, bytes).
    /// Returns None if no mapping exists.
    /// 2026-07-17: Parameter renamed from primitive to ctd.
    pub fn lookup(&self, ctd: &str, bytes: u64) -> Option<&str> {
        let key = bytes.to_string();
        self.ctd
            .get(ctd)
            .and_then(|sizes| sizes.get(&key))
            .map(|s| s.as_str())
    }
}

/// Pure function: derive LLVM type string from (ctd, bytes).
/// Fallback is i{N*8} for raw Bits(N).
/// 2026-07-17: Accepts CTD (PascalCase) instead of old primitive values.
pub fn derive_llvm_type(ctd: Option<&str>, bytes: u64, config: &TypeConfig) -> String {
    if let Some(entry) = config.lookup(ctd.unwrap_or("Int"), bytes) {
        entry.to_string()
    } else {
        format!("i{}", bytes * 8)
    }
}

/// 2026-07-17: Derive ALU from CTD. The primordial sets ALU directly now
/// (via default_alu or override), but this fallback supports backend tests
/// that construct types without going through the universe.
/// Maps to SPIR-V OpType* variants: Int → OpTypeInt, Float → OpTypeFloat.
pub fn derive_alu_type(ctd: Option<&str>, bytes: u64, config: &TypeConfig) -> String {
    // 2026-07-17: ALU is now a direct property on the type. This function is
    // a backward-compat fallback using the same hardcoded mapping as before,
    // but driven by CTD instead of the old primitive values.
    match ctd {
        Some("Float") | Some("Double") => "Float".to_string(),
        Some("Bool") => "Bool".to_string(),
        Some("Ptr") => "Ptr".to_string(),
        _ => "Int".to_string(),
    }
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
    /// Load the built-in op config file (config/llvm-ops.toml).
    pub fn load() -> Self {
        Self::load_from_path(&Path::new(env!("CARGO_MANIFEST_DIR")).join("config/llvm-ops.toml"))
            .expect("Failed to load config/llvm-ops.toml")
    }

    /// 2026-07-15: Load an op config file by name from config/ directory.
    /// Example: OpConfig::load_from("spirv-ops.toml")
    pub fn load_from(name: &str) -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config").join(name);
        Self::load_from_path(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e))
    }

    /// 2026-07-16: P1 — load op config from a concrete file path.
    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;
        let raw: HashMap<String, toml::Value> = toml::from_str(&content)
            .map_err(|e| format!("parse error in '{}': {}", path.display(), e))?;

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
        Ok(OpConfig { op })
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
