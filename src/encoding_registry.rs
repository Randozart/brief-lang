// ── Encoding Registry ──────────────────────────────────────────────────
// 2026-07-18: Resolves encoding names to char_width and index mode.
// PascalCase names are hardcoded (compiler-known char_width).
// Quoted names fall through to config/encodings.toml.
// Unknown names default to char_width = 0 (delegate to runtime scan).
//
// Lookup chain:
//   1. Hardcoded PascalCase match (ASCII, UTF8, UTF16, UTF32, Latin1)
//   2. config/encodings.toml (quoted strings like "shift_jis")
//   3. char_width = 0 (conservative — delegate to stdlib scan)

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

/// Character width and indexing strategy for an encoding.
#[derive(Debug, Clone)]
pub struct EncodingInfo {
    /// Bytes per character. 0 = variable-width (must use Scan).
    pub char_width: u64,
    /// Indexing strategy: Direct (GEP) or Scan (runtime loop).
    pub index_mode: IndexMode,
}

/// Hardcoded PascalCase encodings — compiler guarantees char_width.
fn hardcoded_encodings() -> HashMap<&'static str, EncodingInfo> {
    let mut m = HashMap::new();
    m.insert("ASCII",  EncodingInfo { char_width: 1, index_mode: IndexMode::Direct });
    m.insert("Latin1", EncodingInfo { char_width: 1, index_mode: IndexMode::Direct });
    m.insert("UTF16",  EncodingInfo { char_width: 0, index_mode: IndexMode::Scan });
    m.insert("UTF32",  EncodingInfo { char_width: 4, index_mode: IndexMode::Direct });
    m.insert("UTF8",   EncodingInfo { char_width: 0, index_mode: IndexMode::Scan });
    m
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
                result.insert(enc_key.to_string(), EncodingInfo { char_width, index_mode });
            }
        }
    }
    result
}

/// Resolve an encoding name to its character width and index mode.
/// PascalCase names are hardcoded. Quoted names fall through to config.
/// Unknown names default to char_width = 0 (delegate to runtime).
pub fn get_encoding_info(name: &str) -> EncodingInfo {
    let hardcoded = hardcoded_encodings();
    if let Some(info) = hardcoded.get(name) {
        return info.clone();
    }
    let config = config_encodings();
    if let Some(info) = config.get(name) {
        return info.clone();
    }
    EncodingInfo { char_width: 0, index_mode: IndexMode::Scan }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_pascal_case() {
        let info = get_encoding_info("ASCII");
        assert_eq!(info.char_width, 1);
        assert!(matches!(info.index_mode, IndexMode::Direct));
    }

    #[test]
    fn test_utf8_pascal_case() {
        let info = get_encoding_info("UTF8");
        assert_eq!(info.char_width, 0);
        assert!(matches!(info.index_mode, IndexMode::Scan));
    }

    #[test]
    fn test_utf32_direct() {
        let info = get_encoding_info("UTF32");
        assert_eq!(info.char_width, 4);
        assert!(matches!(info.index_mode, IndexMode::Direct));
    }

    #[test]
    fn test_unknown_encoding_falls_through() {
        let info = get_encoding_info("nonexistent_encoding");
        assert_eq!(info.char_width, 0);
        assert!(matches!(info.index_mode, IndexMode::Scan));
    }
}
