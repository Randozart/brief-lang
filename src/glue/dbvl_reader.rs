// Data Brief Lines (.dbvl) parser — new syntax
//
// 2026-07-26: Rewritten for the new Data Brief syntax:
//   - `;` is the universal field separator (replaces `,`)
//   - `>` is the directive prefix (replaces `schema <path>;`)
//   - Bare tokens are the default; no `" "` quote handling
//   - Maps use `{ key: value; key: value; }` with `;` separator
// See docs/architecture/data-brief.md for the full spec.

use std::collections::HashMap;

/// A parsed .dbvl entry.
#[derive(Debug, Clone)]
pub struct DbvlEntry {
    pub fields: Vec<String>,
}

/// Parsed .dbvl file — a vector of entries plus the active schema path.
#[derive(Debug)]
pub struct DbvlFile {
    pub entries: Vec<DbvlEntry>,
    pub schema_path: Option<String>,
}

/// Parse a .dbvl-formatted string into structured entries.
/// Rules:
///   1. Split by newlines -> lines
///   2. Skip blank lines and `//` comment lines
///   3. If line starts with `>schema`, set active schema
///   4. Otherwise split by `;` (fields are bare tokens, no quotes)
///   5. Return Vec<DbvlEntry> with optional schema
pub fn parse_dbvl(input: &str) -> DbvlFile {
    let mut entries = Vec::new();
    let mut schema_path: Option<String> = None;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        // Check for directive: >schema <path>
        if let Some(rest) = trimmed.strip_prefix('>') {
            let rest = rest.trim();
            if let Some(path) = rest.strip_prefix("schema ") {
                let path = path.trim();
                // Strip trailing ; if present
                let path = path.strip_suffix(';').unwrap_or(path);
                schema_path = Some(path.trim().to_string());
                continue;
            }
            // Unknown directive — skip
            continue;
        }

        // Split the line into tokens by semicolon
        let tokens = split_line_by_semicolons(trimmed);
        if tokens.is_empty() {
            continue;
        }

        entries.push(DbvlEntry { fields: tokens });
    }

    DbvlFile { entries, schema_path }
}

/// Split a line by semicolons.
/// 2026-07-26: Replaced comma-splitting with semicolons.
/// No quote handling — bare tokens are the default.
/// Maps `{ }` preserve their internal semicolons (bounded by braces).
fn split_line_by_semicolons(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut brace_depth: i32 = 0;

    for c in line.chars() {
        match c {
            '{' if brace_depth == 0 => {
                brace_depth += 1;
                current.push(c);
            }
            '}' if brace_depth == 1 => {
                brace_depth -= 1;
                current.push(c);
            }
            ';' if brace_depth == 0 => {
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

/// Parse a curly-brace-delimited map like `{ Int: value; Key2: value2; }`.
/// Returns key-value pairs. The braces are stripped before parsing.
/// 2026-07-26: Updated for new syntax — uses `;` separator inside `{}`.
pub fn parse_map(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let trimmed = s.trim();

    // Strip outer braces if present
    let inner = if trimmed.starts_with('{') && trimmed.ends_with('}') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    for pair in inner.split(';') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
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
        let input = "rust; rs; static; { Int: i64; Float: f32; }; templates/rust/";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 1);
        let entry = &result.entries[0];
        assert_eq!(entry.fields.len(), 5);
        assert_eq!(entry.fields[0], "rust");
        assert_eq!(entry.fields[1], "rs");
        assert_eq!(entry.fields[3], "{ Int: i64; Float: f32; }");
    }

    #[test]
    fn test_semicolons_in_line() {
        let input = "rust; glue/rust/types.bv; rs; x86_64; { Int: int64_t; Float: double; }";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 1);
        let entry = &result.entries[0];
        assert_eq!(entry.fields.len(), 5);
        assert_eq!(entry.fields[4], "{ Int: int64_t; Float: double; }");
    }

    #[test]
    fn test_schema_directive() {
        let input = ">schema glue.dbv\nrust; rs; static; { Int: i64; Float: f32; }";
        let result = parse_dbvl(input);
        assert_eq!(result.schema_path, Some("glue.dbv".to_string()));
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_schema_directive_with_semicolon() {
        let input = ">schema glue.dbv;\nrust; rs; static; { Int: i64; Float: f32; }";
        let result = parse_dbvl(input);
        assert_eq!(result.schema_path, Some("glue.dbv".to_string()));
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_schema_switch() {
        let input = ">schema a.dbv\nentry_a; 1;\n>schema b.dbv\nentry_b; 2;";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 2);
    }

    #[test]
    fn test_parse_map() {
        let input = "{ Int: i64; Float: f32; Bool: bool; }";
        let map = parse_map(input);
        assert_eq!(map.get("Int"), Some(&"i64".to_string()));
        assert_eq!(map.get("Float"), Some(&"f32".to_string()));
        assert_eq!(map.get("Bool"), Some(&"bool".to_string()));
    }

    #[test]
    fn test_comments_and_blank_lines() {
        let input = "// comment\n\nfirst; entry\n\n// another comment\nsecond; entry";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 2);
    }

    #[test]
    fn test_no_schema_scraping_mode() {
        let input = "raw; line; data";
        let result = parse_dbvl(input);
        assert!(result.schema_path.is_none());
        assert_eq!(result.entries.len(), 1);
    }

    #[test]
    fn test_empty_fields() {
        let input = "a; ; b;";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 1);
        let entry = &result.entries[0];
        assert_eq!(entry.fields.len(), 4);
        assert_eq!(entry.fields[0], "a");
        assert_eq!(entry.fields[1], "");
        assert_eq!(entry.fields[2], "b");
        assert_eq!(entry.fields[3], "");
    }

    #[test]
    fn test_trailing_semicolon() {
        let input = "a; b;";
        let result = parse_dbvl(input);
        assert_eq!(result.entries.len(), 1);
        let entry = &result.entries[0];
        assert_eq!(entry.fields.len(), 3);
        assert_eq!(entry.fields[2], "");
    }
}
