// ── Encoding Registry ──────────────────────────────────────────────────
// 2026-07-18: All encoding names are quoted strings resolved through
// config/encodings.toml. No hardcoded PascalCase table — the compiler
// knows zero encoding semantics. Each entry specifies:
//   - char_width: 0 = variable-width (Index# delegates to runtime/stdlib)
//                1+ = fixed-width (Index# emits direct GEP, O(1))
//   - ops.index_at, ops.char_len: stdlib function names for runtime dispatch
//
// Lookup chain:
//   1. config/encodings.toml (all names, quoted)
//   2. char_width = 0 (conservative — delegate to stdlib)

use std::collections::HashMap;
use std::path::Path;

/// How Index# is implemented for this encoding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndexMode {
    /// Fixed-width: emit GEP (O(1)). char_width must be > 0.
    Direct,
    /// Variable-width: emit runtime scan loop or delegate to stdlib.
    Scan,
}

/// Operations that the backend can emit for this encoding.
/// When present, the compiler emits a call to the named stdlib function.
/// When absent, the backend falls back to byte-level ops (length = handle[1],
/// index = byte GEP).
#[derive(Debug, Clone)]
pub struct EncodingOps {
    /// stdlib function for Index#(s, i) -> Int (character at byte offset).
    /// Signature: i64 @fn_name(ptr %s, i64 %i)
    pub index_at: Option<String>,
    /// stdlib function for Length#(s) -> Int (character count).
    /// Signature: i64 @fn_name(ptr %s)
    pub char_len: Option<String>,
}

/// Character width, indexing strategy, and stdlib ops for an encoding.
#[derive(Debug, Clone)]
pub struct EncodingInfo {
    /// Bytes per character. 0 = variable-width (must use Scan or stdlib call).
    pub char_width: u64,
    /// Indexing strategy: Direct (GEP) or Scan (runtime loop).
    pub index_mode: IndexMode,
    /// Optional stdlib function names for encoding-aware operations.
    pub ops: EncodingOps,
}

/// Load config/encodings.toml. Returns empty map if file is missing.
fn config_encodings() -> HashMap<String, EncodingInfo> {
    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => d,
        Err(_) => return HashMap::new(),
    };
    let path = Path::new(&manifest_dir).join("config/encodings.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let raw: HashMap<String, toml::Value> = match toml::from_str(&content) {
        Ok(r) => r,
        Err(_) => return HashMap::new(),
    };
    let mut result = HashMap::new();
    for (key, value) in raw {
        if let Some(enc_key) = key.strip_prefix("encoding.") {
            if let toml::Value::Table(table) = value {
                let char_width = table.get("char_width")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(0) as u64;
                let index_mode = if char_width > 0 { IndexMode::Direct } else { IndexMode::Scan };
                let ops = EncodingOps {
                    index_at: table.get("ops").and_then(|v| v.get("index_at")).and_then(|v| v.as_str().map(|s| s.to_string())),
                    char_len: table.get("ops").and_then(|v| v.get("char_len")).and_then(|v| v.as_str().map(|s| s.to_string())),
                };
                result.insert(enc_key.to_string(), EncodingInfo { char_width, index_mode, ops });
            }
        }
    }
    result
}

/// Resolve an encoding name to its character width, index mode, and stdlib ops.
/// Unknown names default to char_width = 0 (delegate to runtime).
pub fn get_encoding_info(name: &str) -> EncodingInfo {
    let config = config_encodings();
    if let Some(info) = config.get(name) {
        return info.clone();
    }
    EncodingInfo { char_width: 0, index_mode: IndexMode::Scan, ops: EncodingOps { index_at: None, char_len: None } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_UTF8_from_config() {
        let info = get_encoding_info("UTF-8");
        assert_eq!(info.char_width, 0);
        assert!(matches!(info.index_mode, IndexMode::Scan));
    }

    #[test]
    fn test_ASCII_from_config() {
        let info = get_encoding_info("ASCII");
        assert_eq!(info.char_width, 1);
        assert!(matches!(info.index_mode, IndexMode::Direct));
    }

    #[test]
    fn test_unknown_encoding_falls_through() {
        let info = get_encoding_info("nonexistent_encoding");
        assert_eq!(info.char_width, 0);
    }

    #[test]
    fn test_UTF8_has_stdlib_ops() {
        let info = get_encoding_info("UTF-8");
        assert!(info.ops.index_at.is_some());
        assert!(info.ops.char_len.is_some());
    }

    #[test]
    fn test_ASCII_no_stdlib_ops() {
        let info = get_encoding_info("ASCII");
        assert!(info.ops.index_at.is_none());
        assert!(info.ops.char_len.is_none());
    }
}
