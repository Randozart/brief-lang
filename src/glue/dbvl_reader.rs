// Data Brief Lines (.dbvl) parser
//
// Each line is a self-contained entry. Lines are split by commas,
// respecting " " quoted strings and { } map blocks.
// `schema <path>;` directives set the active schema for subsequent lines.
// Without a schema, lines are returned as raw string vectors (scraping mode).

use std::collections::HashMap;

/// A parsed .dbvl entry: either raw tokens (scraping mode) or
/// validated field-token pairs (schema mode).
#[derive(Debug, Clone)]
pub enum DbvlEntry {
    /// Raw string tokens from comma-splitting, no schema validation.
    Raw(Vec<String>),
    /// Schema-validated tokens — each position has a known meaning.
    /// The schema path tells which .dbvs file was used.
    Validated {
        schema: String,
        fields: Vec<String>,
    },
}

/// Parsed .dbvl file — a vector of entries plus the active schema path.
#[derive(Debug)]
pub struct DbvlFile {
    pub entries: Vec<DbvlEntry>,
    pub schema_path: Option<String>,
}

/// Parse a .dbvl-formatted string into structured entries.
/// Rules:
///   1. Split by newlines → lines
///   2. Skip blank lines and `//` comment lines
///   3. If line starts with `schema <path>;`, set active schema
///   4. Otherwise split by `,` (respecting `" "` and `{ }` boundaries)
///   5. Return Vec<DbvlEntry> with optional schema
pub fn parse_dbvl(input: &str) -> DbvlFile {
    let mut entries = Vec::new();
    let mut schema_path: Option<String> = None;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        // Check for schema directive: schema <path>;
        if let Some(path) = trimmed.strip_prefix("schema ") {
            if let Some(path) = path.strip_suffix(';') {
                schema_path = Some(path.trim().to_string());
                continue;
            }
        }

        // Split the line into tokens by comma, respecting quotes and braces
        let tokens = split_line_by_commas(trimmed);
        if tokens.is_empty() {
            continue;
        }

        if let Some(ref schema) = schema_path {
            entries.push(DbvlEntry::Validated {
                schema: schema.clone(),
                fields: tokens,
            });
        } else {
            entries.push(DbvlEntry::Raw(tokens));
        }
    }

    DbvlFile { entries, schema_path }
}

/// Split a line by commas, respecting:
/// 1. `" "` quoted strings — commas inside quotes are literal
/// 2. `{ }` curly brace blocks — commas inside braces are literal
/// 3. Empty fields are preserved as empty strings
fn split_line_by_commas(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut brace_depth: i32 = 0;

    for c in line.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            '{' if !in_quotes => {
                brace_depth += 1;
                current.push(c);
            }
            '}' if !in_quotes => {
                brace_depth -= 1;
                current.push(c);
            }
            ',' if !in_quotes && brace_depth == 0 => {
                tokens.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }

    // Push the last token
    let last = current.trim().to_string();
    tokens.push(last);

    tokens
}

/// Check if a value is quoted (starts and ends with "), and unquote it.
pub fn unquote(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Parse a curly-brace-delimited map like `{Int:i64 Float:f32}`.
/// Returns key-value pairs. The braces are stripped before parsing.
pub fn parse_map(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let trimmed = s.trim();

    // Strip outer braces if present
    let inner = if trimmed.starts_with('{') && trimmed.ends_with('}') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    for pair in inner.split_whitespace() {
        if let Some(pos) = pair.find(':') {
            let key = pair[..pos].trim().to_string();
            let value = pair[pos + 1..].trim().to_string();
            if !key.is_empty() {
                map.insert(key, value);
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_line() {
        let input = "rust, rs, static, {Int:i64 Float:f32}, templates/rust/";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 1);
        match &result.entries[0] {
            DbvlEntry::Raw(tokens) => {
                assert_eq!(tokens.len(), 5);
                assert_eq!(tokens[0], "rust");
                assert_eq!(tokens[1], "rs");
                assert_eq!(tokens[3], "{Int:i64 Float:f32}");
            }
            _ => panic!("expected Raw entry"),
        }
    }

    #[test]
    fn test_parse_quoted_value() {
        let input = "\"my, weird language\", ext, {Int:i64}";
        let result = parse_dbvl(input);
        match &result.entries[0] {
            DbvlEntry::Raw(tokens) => {
                assert_eq!(tokens.len(), 3);
                assert_eq!(unquote(&tokens[0]), "my, weird language");
            }
            _ => panic!("expected Raw entry"),
        }
    }

    #[test]
    fn test_schema_directive() {
        let input = "schema glue.dbvs;\nrust, rs, static, {Int:i64}";
        let result = parse_dbvl(input);
        assert_eq!(result.schema_path, Some("glue.dbvs".to_string()));
        assert_eq!(result.entries.len(), 1);
        match &result.entries[0] {
            DbvlEntry::Validated { schema, .. } => {
                assert_eq!(schema, "glue.dbvs");
            }
            _ => panic!("expected Validated entry"),
        }
    }

    #[test]
    fn test_schema_switch() {
        let input = "schema a.dbvs;\nentry_a\nschema b.dbvs;\nentry_b";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 2);
        if let DbvlEntry::Validated { schema, .. } = &result.entries[0] {
            assert_eq!(schema, "a.dbvs");
        } else {
            panic!("expected Validated entry for first");
        }
        if let DbvlEntry::Validated { schema, .. } = &result.entries[1] {
            assert_eq!(schema, "b.dbvs");
        } else {
            panic!("expected Validated entry for second");
        }
    }

    #[test]
    fn test_parse_map() {
        let input = "{Int:i64 Float:f32 Bool:bool}";
        let map = parse_map(input);
        assert_eq!(map.get("Int"), Some(&"i64".to_string()));
        assert_eq!(map.get("Float"), Some(&"f32".to_string()));
        assert_eq!(map.get("Bool"), Some(&"bool".to_string()));
    }

    #[test]
    fn test_comments_and_blank_lines() {
        let input = "// comment\n\nfirst, entry\n\n// another comment\nsecond, entry";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 2);
    }

    #[test]
    fn test_no_schema_scraping_mode() {
        let input = "raw, line, data";
        let result = parse_dbvl(input);
        assert!(result.schema_path.is_none());
        assert_eq!(result.entries.len(), 1);
    }
}
