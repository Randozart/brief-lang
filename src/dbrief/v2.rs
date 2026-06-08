// DBrief v2 — Redesigned parser for DBV/DBVS/DBVL files
//
// Produces clean document types (not native Brief values).
// Conversion to native Value happens at import time (Phase B).

use std::collections::HashMap;
use serde::Serialize;

// ============================================================================
// Types
// ============================================================================

/// Parsed DBrief document — can represent .dbv, .dbvs, or .dbvl content
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DbriefDocument {
    pub imports: Vec<String>,
    pub schemas: Vec<SchemaDef>,
    pub data_groups: Vec<DataGroup>,
    pub rules: Vec<RuleDef>,
}

/// A schema definition (from .dbvs or inline in .dbv)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SchemaDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

/// A single field in a schema
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldDef {
    pub name: String,
    pub ty: FieldType,
    /// Raw constraint expression text (e.g. `!= ""`, `>= 0`), without brackets
    pub constraint: Option<String>,
    /// True if field is optional (marked with ? suffix)
    pub optional: bool,
}

/// Field type expression
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum FieldType {
    String,
    Int,
    Float,
    Bool,
    UInt(usize),
    Vec(Box<FieldType>),
    Map(Box<FieldType>, Box<FieldType>),
    Option(Box<FieldType>),
    /// Reference to another schema by name
    Named(String),
}

/// A named data group (e.g. `as Item { ... }`)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DataGroup {
    pub schema_name: Option<String>,
    pub entries: Vec<DataEntry>,
}

/// A single data entry with optional key and optional schema reference
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DataEntry {
    pub key: Option<String>,
    /// Schema name when using inline `key as Schema { ... }` form
    pub schema_name: Option<String>,
    pub fields: Vec<DataField>,
}

/// A field value — either positional or named
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DataField {
    Positional(DataValue),
    Named(String, DataValue),
}

/// A data value expression
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DataValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<DataValue>),
    Map(HashMap<String, DataValue>),
}

/// A query/validation rule
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuleDef {
    pub name: String,
    pub params: Vec<(String, FieldType)>,
    /// Raw body expression text
    pub body: String,
}

// ============================================================================
// Parser
// ============================================================================

pub fn parse_document(input: &str) -> Result<DbriefDocument, String> {
    let mut parser = Parser::new(input.to_string());
    parser.parse()
}

struct Parser {
    input: String,
    pos: usize,
}

impl Parser {
    fn new(input: String) -> Self {
        Parser { input, pos: 0 }
    }

    fn parse(&mut self) -> Result<DbriefDocument, String> {
        let mut doc = DbriefDocument {
            imports: Vec::new(),
            schemas: Vec::new(),
            data_groups: Vec::new(),
            rules: Vec::new(),
        };

        loop {
            self.skip_ws_and_comments();
            if self.is_eof() {
                break;
            }

            let c = self.peek_char().unwrap();
            match c {
                // import "path"
                'i' | 'I' if self.starts_with_ignore_case("import") => {
                    let path = self.parse_import()?;
                    doc.imports.push(path);
                }
                // schema Name { ... }
                's' | 'S' if self.starts_with_ignore_case("schema") => {
                    let schema = self.parse_schema()?;
                    doc.schemas.push(schema);
                }
                // rule name(params) { body }
                'r' | 'R' if self.starts_with_ignore_case("rule") => {
                    let rule = self.parse_rule()?;
                    doc.rules.push(rule);
                }
                // as Schema { ... } — grouped data
                'a' | 'A' if self.starts_with_ignore_case("as") => {
                    let group = self.parse_grouped_data()?;
                    doc.data_groups.push(group);
                }
                // key as Schema { ... } — keyed data entry
                // or { ... } — schema-less data
                '{' => {
                    let group = self.parse_schema_less_block()?;
                    doc.data_groups.push(group);
                }
                // Must be a data entry line (positional or keyed)
                _ => {
                    let entry = self.parse_data_line()?;
                    // Propagate entry's schema_name to the group if set
                    let group = DataGroup {
                        schema_name: entry.schema_name.clone(),
                        entries: vec![entry],
                    };
                    doc.data_groups.push(group);
                }
            }
        }

        Ok(doc)
    }

    // ========================================================================
    // Import
    // ========================================================================

    fn parse_import(&mut self) -> Result<String, String> {
        self.consume_keyword_ignore_case("import")?;
        self.skip_ws();
        let path = self.parse_string()?;
        self.skip_ws();
        if self.peek_char() == Some(';') {
            self.advance();
        }
        Ok(path)
    }

    // ========================================================================
    // Schema
    // ========================================================================

    fn parse_schema(&mut self) -> Result<SchemaDef, String> {
        self.consume_keyword_ignore_case("schema")?;
        self.skip_ws();
        let name = self.parse_identifier()?;
        self.skip_ws();
        self.expect_char('{')?;

        let mut fields = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.peek_char() == Some('}') {
                self.advance();
                break;
            }
            if self.is_eof() {
                return Err("Unexpected end of input in schema body".into());
            }
            fields.push(self.parse_field_def()?);
        }

        Ok(SchemaDef { name, fields })
    }

    fn parse_field_def(&mut self) -> Result<FieldDef, String> {
        self.skip_ws_and_comments();

        // Parse optional constraint: [expr]
        let constraint = if self.peek_char() == Some('[') {
            self.advance(); // consume '['
            let mut depth = 1u32;
            let mut expr = String::new();
            loop {
                if self.is_eof() {
                    return Err("Unterminated constraint bracket".into());
                }
                let c = self.advance().unwrap();
                if c == '[' {
                    depth += 1;
                    expr.push(c);
                } else if c == ']' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    expr.push(c);
                } else {
                    expr.push(c);
                }
            }
            let trimmed = expr.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        } else {
            None
        };

        self.skip_ws();

        // Field name
        let name = self.parse_identifier()?;
        self.skip_ws();

        // Optional ? suffix
        let optional = self.peek_char() == Some('?');
        if optional {
            self.advance();
            self.skip_ws();
        }

        // : Type
        self.expect_char(':')?;
        self.skip_ws();

        let ty = self.parse_field_type()?;

        // Optional trailing comma or semicolon
        self.skip_ws();
        if self.peek_char() == Some(',') || self.peek_char() == Some(';') {
            self.advance();
        }

        Ok(FieldDef {
            name,
            ty,
            constraint,
            optional,
        })
    }

    fn parse_field_type(&mut self) -> Result<FieldType, String> {
        self.skip_ws();

        if self.starts_with_ignore_case("String") {
            self.advance_n(6);
            Ok(FieldType::String)
        } else if self.starts_with_ignore_case("Int") {
            self.advance_n(3);
            Ok(FieldType::Int)
        } else if self.starts_with_ignore_case("Float") {
            self.advance_n(5);
            Ok(FieldType::Float)
        } else if self.starts_with_ignore_case("Bool") {
            self.advance_n(4);
            Ok(FieldType::Bool)
        } else if self.starts_with_ignore_case("UInt") {
            self.advance_n(4);
            if self.peek_char() == Some('[') {
                self.advance();
                let width_str = self.parse_while(|c| c.is_ascii_digit());
                let width: usize = width_str
                    .parse()
                    .map_err(|_| format!("Invalid UInt width: {}", width_str))?;
                self.skip_ws();
                self.expect_char(']')?;
                Ok(FieldType::UInt(width))
            } else {
                Ok(FieldType::UInt(64))
            }
        } else if self.starts_with_ignore_case("Vec") || self.starts_with_ignore_case("List") {
            if self.starts_with_ignore_case("Vec") {
                self.advance_n(3);
            } else {
                self.advance_n(4);
            }
            self.skip_ws();
            self.expect_char('[')?;
            self.skip_ws();
            let inner = self.parse_field_type()?;
            self.skip_ws();
            self.expect_char(']')?;
            Ok(FieldType::Vec(Box::new(inner)))
        } else if self.starts_with_ignore_case("Map") {
            self.advance_n(3);
            self.skip_ws();
            self.expect_char('[')?;
            self.skip_ws();
            let key_t = self.parse_field_type()?;
            self.skip_ws();
            self.expect_char(',')?;
            self.skip_ws();
            let val_t = self.parse_field_type()?;
            self.skip_ws();
            self.expect_char(']')?;
            Ok(FieldType::Map(Box::new(key_t), Box::new(val_t)))
        } else if self.starts_with_ignore_case("Option") {
            self.advance_n(6);
            self.skip_ws();
            self.expect_char('[')?;
            self.skip_ws();
            let inner = self.parse_field_type()?;
            self.skip_ws();
            self.expect_char(']')?;
            Ok(FieldType::Option(Box::new(inner)))
        } else {
            let name = self.parse_identifier()?;
            Ok(FieldType::Named(name))
        }
    }

    // ========================================================================
    // Grouped data: as SchemaName { ... }
    // ========================================================================

    fn parse_grouped_data(&mut self) -> Result<DataGroup, String> {
        self.consume_keyword_ignore_case("as")?;
        self.skip_ws();

        // Schema name
        let schema_name = Some(self.parse_identifier()?);
        self.skip_ws();
        self.expect_char('{')?;
        self.skip_ws_and_comments();

        let mut entries = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.peek_char() == Some('}') {
                self.advance();
                break;
            }
            if self.is_eof() {
                return Err("Unexpected end of input in data group".into());
            }

            entries.push(self.parse_data_entry_in_group()?);
        }

        Ok(DataGroup {
            schema_name,
            entries,
        })
    }

    /// Parse an entry within an `as Schema { ... }` block
    /// Either `key { vals }` or just `{ vals }`
    fn parse_data_entry_in_group(&mut self) -> Result<DataEntry, String> {
        self.skip_ws_and_comments();

        // Could be a key followed by { ... }, or directly { ... }
        let key = if self.peek_char() != Some('{') {
            let k = Some(self.parse_identifier()?);
            self.skip_ws();
            k
        } else {
            None
        };

        self.expect_char('{')?;
        let fields = self.parse_data_fields()?;
        self.expect_char('}')?;

        Ok(DataEntry {
            key,
            schema_name: None,
            fields,
        })
    }

    // ========================================================================
    // Schema-less block: { field: val, field: val }
    // ========================================================================

    fn parse_schema_less_block(&mut self) -> Result<DataGroup, String> {
        self.expect_char('{')?;
        let fields = self.parse_named_fields_map()?;
        self.expect_char('}')?;

        Ok(DataGroup {
            schema_name: None,
            entries: vec![DataEntry {
                key: None,
                schema_name: None,
                fields,
            }],
        })
    }

    // ========================================================================
    // Data line: key as Schema { vals } or just positional values (for dbvl)
    // ========================================================================

    /// Parse a line of data — could be:
    /// - `key as Schema { ... }`
    /// - `key: val, val, val` (dbvl keyed line)
    /// - `val, val, val` (dbvl positional line)
    /// - `{ field: val }` (inline object)
    fn parse_data_line(&mut self) -> Result<DataEntry, String> {
        self.skip_ws_and_comments();
        if self.is_eof() {
            return Err("Unexpected end of input".into());
        }

        // Check for inline object
        if self.peek_char() == Some('{') {
            self.advance();
            let fields = self.parse_data_fields();
            match fields {
                Ok(f) => {
                    self.expect_char('}')?;
                    return Ok(DataEntry { key: None, schema_name: None, fields: f });
                }
                Err(_) => {
                    // Not a data field list — could be schema-less named
                    // But we already consumed '{'. Rewind? No, try named fields.
                    // Actually, let's handle differently: restart with named fields.
                }
            }
        }

        // Read until we hit 'as' or '{' or end of meaningful content
        // Strategy: peek ahead for " as " pattern
        let save = self.pos;
        let first_ident = self.try_parse_identifier();
        self.pos = save;

        if let Some(key) = first_ident {
            self.pos = save;
            self.advance_n(key.len());
            self.skip_ws();

            // Check for 'as' keyword
            if self.starts_with_ignore_case("as") {
                self.consume_keyword_ignore_case("as")?;
                self.skip_ws();
                let schema_name = Some(self.parse_identifier()?);
                self.skip_ws();
                self.expect_char('{')?;
                let fields = self.parse_data_fields()?;
                self.expect_char('}')?;
                let sname = schema_name.clone();
                return Ok(DataEntry {
                    key: Some(key),
                    schema_name: sname,
                    fields,
                });
            }

            // Not "as" — could be dbvl keyed line: key: val, val, val
            if self.peek_char() == Some(':') {
                self.advance(); // consume ':'
                self.skip_ws();
                let fields = self.parse_positional_values()?;
                return Ok(DataEntry {
                    key: Some(key),
                    schema_name: None,
                    fields,
                });
            }

            // Just positional values — treat as unnamed
            self.pos = save; // rewind
        }

        // Default: positional values
        let fields = self.parse_positional_values()?;
        Ok(DataEntry {
            key: None,
            schema_name: None,
            fields,
        })
    }

    // ========================================================================
    // Rule
    // ========================================================================

    fn parse_rule(&mut self) -> Result<RuleDef, String> {
        self.consume_keyword_ignore_case("rule")?;
        self.skip_ws();
        let name = self.parse_identifier()?;
        self.skip_ws();
        self.expect_char('(')?;

        let mut params = Vec::new();
        loop {
            self.skip_ws();
            if self.peek_char() == Some(')') {
                self.advance();
                break;
            }
            if !params.is_empty() {
                self.expect_char(',')?;
                self.skip_ws();
            }
            let pname = self.parse_identifier()?;
            self.skip_ws();
            self.expect_char(':')?;
            self.skip_ws();
            let ptype = self.parse_field_type()?;
            params.push((pname, ptype));
            self.skip_ws();
        }

        self.skip_ws();
        self.expect_char('{')?;
        let body_start = self.pos;
        let mut depth = 1u32;
        loop {
            if self.is_eof() {
                return Err("Unterminated rule body".into());
            }
            let c = self.advance().unwrap();
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
        }
        let body = self.input[body_start..self.pos - 1].trim().to_string();

        Ok(RuleDef {
            name,
            params,
            body,
        })
    }

    // ========================================================================
    // Data field list: name: val, name: val or val, val, val
    // ========================================================================

    fn parse_data_fields(&mut self) -> Result<Vec<DataField>, String> {
        let mut fields = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.peek_char() == Some('}') || self.peek_char() == Some(')') || self.is_eof() {
                break;
            }

            if !fields.is_empty() {
                if self.peek_char() == Some(',') {
                    self.advance();
                    self.skip_ws_and_comments();
                } else {
                    break;
                }
            }

            // Peek: if next token is "ident:" then it's a named field
            if let Some(field) = self.try_parse_named_field() {
                fields.push(field);
            } else {
                let val = self.parse_value()?;
                fields.push(DataField::Positional(val));
            }
        }
        Ok(fields)
    }

    /// Try to parse `name: value` — returns None if not a named field.
    /// Supports both `ident:` and `"quoted string":` keys (JSONL compat).
    fn try_parse_named_field(&mut self) -> Option<DataField> {
        let save = self.pos;
        // Try quoted string key first (JSONL)
        let name = if self.peek_char() == Some('"') {
            match self.parse_string() {
                Ok(s) => s,
                Err(_) => {
                    self.pos = save;
                    return None;
                }
            }
        } else {
            match self.try_parse_identifier() {
                Some(s) => s,
                None => return None,
            }
        };
        self.skip_ws();
        if self.peek_char() == Some(':') {
            self.advance(); // consume ':'
            self.skip_ws();
            match self.parse_value() {
                Ok(val) => Some(DataField::Named(name, val)),
                Err(_) => {
                    self.pos = save;
                    None
                }
            }
        } else {
            self.pos = save;
            None
        }
    }

    /// Parse named fields as a Map: `{ name: val, name: val }` body
    fn parse_named_fields_map(&mut self) -> Result<Vec<DataField>, String> {
        let mut fields = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.peek_char() == Some('}') || self.is_eof() {
                break;
            }
            if !fields.is_empty() {
                if self.peek_char() == Some(',') {
                    self.advance();
                    self.skip_ws_and_comments();
                } else {
                    break;
                }
            }
            match self.try_parse_named_field() {
                Some(f) => fields.push(f),
                None => {
                    return Err(format!(
                        "Expected named field at position {}",
                        self.pos
                    ))
                }
            }
        }
        Ok(fields)
    }

    /// Parse comma-separated positional values in a dbvl line
    fn parse_positional_values(&mut self) -> Result<Vec<DataField>, String> {
        let mut fields = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.is_eof() || self.peek_char() == Some('\n') {
                break;
            }
            if !fields.is_empty() {
                if self.peek_char() == Some(',') {
                    self.advance();
                    self.skip_ws_and_comments();
                } else {
                    break;
                }
            }
            let val = self.parse_value()?;
            fields.push(DataField::Positional(val));
        }
        Ok(fields)
    }

    // ========================================================================
    // Value expression parsing
    // ========================================================================

    fn parse_value(&mut self) -> Result<DataValue, String> {
        self.skip_ws_and_comments();

        match self.peek_char() {
            Some('"') => {
                let s = self.parse_string()?;
                Ok(DataValue::String(s))
            }
            Some('t') if self.starts_with("true") => {
                self.advance_n(4);
                Ok(DataValue::Bool(true))
            }
            Some('f') if self.starts_with("false") => {
                self.advance_n(5);
                Ok(DataValue::Bool(false))
            }
            Some('{') => {
                self.advance();
                let mut map = HashMap::new();
                loop {
                    self.skip_ws_and_comments();
                    if self.peek_char() == Some('}') {
                        self.advance();
                        break;
                    }
                    if !map.is_empty() {
                        if self.peek_char() == Some(',') {
                            self.advance();
                            self.skip_ws_and_comments();
                        } else {
                            break;
                        }
                    }
                    let k = self.parse_string_or_ident()?;
                    self.skip_ws();
                    self.expect_char(':')?;
                    self.skip_ws();
                    let v = self.parse_value()?;
                    map.insert(k, v);
                }
                Ok(DataValue::Map(map))
            }
            Some('[') => {
                self.advance();
                let mut list = Vec::new();
                loop {
                    self.skip_ws_and_comments();
                    if self.peek_char() == Some(']') {
                        self.advance();
                        break;
                    }
                    if !list.is_empty() {
                        if self.peek_char() == Some(',') {
                            self.advance();
                            self.skip_ws_and_comments();
                        } else {
                            break;
                        }
                    }
                    let val = self.parse_value()?;
                    list.push(val);
                }
                Ok(DataValue::List(list))
            }
            Some(c) if c.is_ascii_digit() || c == '-' => {
                let num_str = self.parse_while(|c| c.is_ascii_digit() || c == '.' || c == '-');
                if num_str.contains('.') {
                    let f: f64 = num_str
                        .parse()
                        .map_err(|_| format!("Invalid float: {}", num_str))?;
                    Ok(DataValue::Float(f))
                } else {
                    let n: i64 = num_str
                        .parse()
                        .map_err(|_| format!("Invalid integer: {}", num_str))?;
                    Ok(DataValue::Int(n))
                }
            }
            Some(c) if c.is_alphabetic() || c == '_' => {
                let ident = self.parse_identifier()?;
                // Could be a bare identifier reference (e.g., schema name as value)
                // Treat as string for now — the type resolver will catch mismatches
                Ok(DataValue::String(ident))
            }
            Some(c) => Err(format!("Unexpected character '{}' at position {}", c, self.pos)),
            None => Err("Unexpected end of input while parsing value".into()),
        }
    }

    fn parse_string_or_ident(&mut self) -> Result<String, String> {
        if self.peek_char() == Some('"') {
            self.parse_string()
        } else {
            self.parse_identifier()
        }
    }

    // ========================================================================
    // Primitives
    // ========================================================================

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect_char('"')?;
        let mut s = String::new();
        loop {
            match self.advance() {
                None => return Err("Unterminated string literal".into()),
                Some('"') => break,
                Some('\\') => match self.advance() {
                    None => return Err("Unterminated escape sequence".into()),
                    Some('"') => s.push('"'),
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some(c) => {
                        s.push('\\');
                        s.push(c);
                    }
                },
                Some(c) => s.push(c),
            }
        }
        Ok(s)
    }

    fn parse_identifier(&mut self) -> Result<String, String> {
        let s = self.parse_while(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-');
        if s.is_empty() {
            Err(format!("Expected identifier at position {}", self.pos))
        } else {
            Ok(s)
        }
    }

    /// Try to parse an identifier without consuming on failure
    fn try_parse_identifier(&mut self) -> Option<String> {
        let save = self.pos;
        let s = self.parse_while(|c| c.is_alphanumeric() || c == '_');
        if s.is_empty() {
            self.pos = save;
            None
        } else {
            Some(s)
        }
    }

    /// Parse characters matching a predicate
    fn parse_while<F>(&mut self, pred: F) -> String
    where
        F: Fn(char) -> bool,
    {
        let mut s = String::new();
        while let Some(c) = self.peek_char() {
            if pred(c) {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        s
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() && c != '\n' {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            // Skip whitespace (including newlines)
            while let Some(c) = self.peek_char() {
                if c.is_whitespace() {
                    self.advance();
                } else {
                    break;
                }
            }
            // Skip line comments
            if self.starts_with("//") {
                while let Some(c) = self.peek_char() {
                    if c == '\n' {
                        self.advance();
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        self.input[self.pos..].starts_with(s)
    }

    fn starts_with_ignore_case(&self, s: &str) -> bool {
        self.input[self.pos..].to_lowercase().starts_with(&s.to_lowercase())
    }

    fn consume_keyword_ignore_case(&mut self, kw: &str) -> Result<(), String> {
        if self.starts_with_ignore_case(kw) {
            self.pos += kw.len();
            // Ensure not part of a longer word
            if let Some(c) = self.peek_char() {
                if c.is_alphanumeric() || c == '_' {
                    return Err(format!("'{}' followed by alphanumeric — not a keyword", kw));
                }
            }
            Ok(())
        } else {
            Err(format!("Expected keyword '{}'", kw))
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), String> {
        self.skip_ws_and_comments();
        match self.advance() {
            Some(c) if c == expected => Ok(()),
            Some(c) => Err(format!(
                "Expected '{}' but found '{}' at position {}",
                expected, c, self.pos - 1
            )),
            None => Err(format!(
                "Expected '{}' but reached end of input",
                expected
            )),
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.input[self.pos..].chars().next()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn advance_n(&mut self, n: usize) {
        self.pos = std::cmp::min(self.pos + n, self.input.len());
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn consume(&mut self, ch: char) -> Result<(), String> {
        if self.peek_char() == Some(ch) {
            self.advance();
            Ok(())
        } else {
            Err(format!("Expected '{}' at position {}", ch, self.pos))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Schema Tests ----

    #[test]
    fn test_schema_basic() {
        let input = r#"
schema Item {
    [ != "" ] id: String
    [ != "" ] desc: String
    [ >= 0 ] hp: Int
    takeable: Bool
    location: String
}
"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.schemas.len(), 1);
        let schema = &doc.schemas[0];
        assert_eq!(schema.name, "Item");
        assert_eq!(schema.fields.len(), 5);

        assert_eq!(schema.fields[0].name, "id");
        assert_eq!(schema.fields[0].ty, FieldType::String);
        assert_eq!(schema.fields[0].constraint.as_deref(), Some("!= \"\""));
        assert!(!schema.fields[0].optional);

        assert_eq!(schema.fields[1].name, "desc");
        assert_eq!(schema.fields[2].name, "hp");
        assert_eq!(schema.fields[2].ty, FieldType::Int);
        assert_eq!(
            schema.fields[2].constraint.as_deref(),
            Some(">= 0")
        );

        assert_eq!(schema.fields[3].name, "takeable");
        assert_eq!(schema.fields[3].ty, FieldType::Bool);
        assert!(schema.fields[3].constraint.is_none());

        assert_eq!(schema.fields[4].name, "location");
        assert_eq!(schema.fields[4].ty, FieldType::String);
    }

    #[test]
    fn test_schema_with_types() {
        let input = r#"
schema AllTypes {
    a: String
    b: Int
    c: Float
    d: Bool
    e: UInt[32]
    f: Vec[String]
    g: Map[String, Int]
    h: Option[Bool]
    i: IoResult
}
"#;
        let doc = parse_document(input).unwrap();
        let s = &doc.schemas[0];
        assert_eq!(s.fields.len(), 9);
        assert_eq!(s.fields[0].ty, FieldType::String);
        assert_eq!(s.fields[1].ty, FieldType::Int);
        assert_eq!(s.fields[2].ty, FieldType::Float);
        assert_eq!(s.fields[3].ty, FieldType::Bool);
        assert_eq!(s.fields[4].ty, FieldType::UInt(32));
        assert_eq!(s.fields[5].ty, FieldType::Vec(Box::new(FieldType::String)));
        assert_eq!(
            s.fields[6].ty,
            FieldType::Map(Box::new(FieldType::String), Box::new(FieldType::Int))
        );
        assert_eq!(s.fields[7].ty, FieldType::Option(Box::new(FieldType::Bool)));
        assert_eq!(s.fields[8].ty, FieldType::Named("IoResult".into()));
    }

    #[test]
    fn test_optional_field() {
        let input = r#"
schema Opt {
    name: String
    desc?: String
}
"#;
        let doc = parse_document(input).unwrap();
        let s = &doc.schemas[0];
        assert!(!s.fields[0].optional);
        assert!(s.fields[1].optional);
    }

    // ---- Data Tests ----

    #[test]
    fn test_positional_data_entry() {
        let input = r#"rusty_key as Item { "Rusty Key", "An old iron key", 5, true, "start" }"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.data_groups.len(), 1);
        let group = &doc.data_groups[0];
        assert_eq!(group.entries.len(), 1);
        let entry = &group.entries[0];
        assert_eq!(entry.key.as_deref(), Some("rusty_key"));
        assert!(group.schema_name.is_some());
        assert_eq!(group.schema_name.as_deref(), Some("Item"));
        assert_eq!(entry.fields.len(), 5);
        match &entry.fields[0] {
            DataField::Positional(DataValue::String(s)) => assert_eq!(s, "Rusty Key"),
            _ => panic!("Expected positional string"),
        }
        match &entry.fields[4] {
            DataField::Positional(DataValue::String(s)) => assert_eq!(s, "start"),
            _ => panic!("Expected positional string"),
        }
    }

    #[test]
    fn test_named_data_entry() {
        let input = r#"cheese as Item { name: "Moldy Cheese", desc: "Smelly.", hp: 1, location: "pantry" }"#;
        let doc = parse_document(input).unwrap();
        let group = &doc.data_groups[0];
        let entry = &group.entries[0];
        assert_eq!(entry.key.as_deref(), Some("cheese"));
        assert_eq!(entry.fields.len(), 4);
        match &entry.fields[0] {
            DataField::Named(n, DataValue::String(v)) => {
                assert_eq!(n, "name");
                assert_eq!(v, "Moldy Cheese");
            }
            _ => panic!("Expected named field"),
        }
    }

    #[test]
    fn test_grouped_data() {
        let input = r#"
as Item {
    rusty_key { "Rusty Key", "An old iron key", 5, true, "start" }
    candle { "Wax Candle", "A stubby candle", 3, true, "kitchen" }
}
"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.data_groups.len(), 1);
        let group = &doc.data_groups[0];
        assert_eq!(group.schema_name.as_deref(), Some("Item"));
        assert_eq!(group.entries.len(), 2);
        assert_eq!(group.entries[0].key.as_deref(), Some("rusty_key"));
        assert_eq!(group.entries[1].key.as_deref(), Some("candle"));
    }

    #[test]
    fn test_schema_less_block() {
        let input = r#"{ dom_state: "...", timestamp: 1234567890 }"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.data_groups.len(), 1);
        let group = &doc.data_groups[0];
        assert!(group.schema_name.is_none());
        assert_eq!(group.entries.len(), 1);
        let entry = &group.entries[0];
        assert!(entry.key.is_none());
        assert_eq!(entry.fields.len(), 2);
        match &entry.fields[0] {
            DataField::Named(n, DataValue::String(v)) => {
                assert_eq!(n, "dom_state");
                assert_eq!(v, "...");
            }
            _ => panic!("Expected named field"),
        }
    }

    // ---- Import Tests ----

    #[test]
    fn test_import() {
        let input = r#"import "game.dbvs""#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.imports.len(), 1);
        assert_eq!(doc.imports[0], "game.dbvs");
    }

    #[test]
    fn test_import_with_semicolon() {
        let input = r#"import "std.dbvs";"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.imports.len(), 1);
        assert_eq!(doc.imports[0], "std.dbvs");
    }

    // ---- Rule Tests ----

    #[test]
    fn test_rule() {
        let input = r#"
rule can_go(from: String, to: String) {
    Room[from].exits -> contains(to)
}
"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.rules.len(), 1);
        let rule = &doc.rules[0];
        assert_eq!(rule.name, "can_go");
        assert_eq!(rule.params.len(), 2);
        assert_eq!(rule.params[0].0, "from");
        assert_eq!(rule.params[0].1, FieldType::String);
        assert_eq!(rule.params[1].0, "to");
        assert!(rule.body.contains("Room[from].exits -> contains(to)"));
    }

    // ---- DBVL Tests ----

    #[test]
    fn test_dbvl_positional_line() {
        let input = r#""Rusty Key", "An old iron key", 5, true, start"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.data_groups.len(), 1);
        let entry = &doc.data_groups[0].entries[0];
        assert!(entry.key.is_none());
        assert_eq!(entry.fields.len(), 5);
        match &entry.fields[0] {
            DataField::Positional(DataValue::String(s)) => assert_eq!(s, "Rusty Key"),
            _ => panic!("Expected positional string"),
        }
    }

    #[test]
    fn test_dbvl_keyed_line() {
        let input = r#"rusty_key: "Rusty Key", "An old iron key", 5, true, start"#;
        let doc = parse_document(input).unwrap();
        let entry = &doc.data_groups[0].entries[0];
        assert_eq!(entry.key.as_deref(), Some("rusty_key"));
    }

    #[test]
    fn test_dbvl_json_line() {
        let input = r#"{"id": "cellar", "name": "The Cellar", "desc": "Dark and dusty..."}"#;
        let doc = parse_document(input).unwrap();
        let group = &doc.data_groups[0];
        assert!(group.schema_name.is_none());
        let entry = &group.entries[0];
        assert!(entry.key.is_none());
        let fields: Vec<&str> = entry
            .fields
            .iter()
            .filter_map(|f| match f {
                DataField::Named(n, _) => Some(n.as_str()),
                _ => None,
            })
            .collect();
        assert!(fields.contains(&"id"));
        assert!(fields.contains(&"name"));
        assert!(fields.contains(&"desc"));
    }

    // ---- Combined Tests ----

    #[test]
    fn test_complex_dbv() {
        let input = r#"
import "game.dbvs"
import "ffi_core.dbvs"

schema Custom {
    x: Int
    y: Int
}

as Item {
    rusty_key { "Rusty Key", "An old iron key", 5, true, "start" }
}

as FnBinding {
    print { "print", [String], IoResult, "libruntime", 0 }
}

rule visible_items(room: String) {
    Item -> FILTER location == room
}
"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.imports.len(), 2);
        assert_eq!(doc.schemas.len(), 1);
        assert_eq!(doc.data_groups.len(), 2);
        assert_eq!(doc.rules.len(), 1);
    }

    // ---- Error Tests ----

    #[test]
    fn test_unterminated_string() {
        let result = parse_document(r#"key as S { "unclosed }"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_schema_name() {
        let result = parse_document(r#"schema { }"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_unterminated_bracket() {
        let result = parse_document(r#"
schema Bad {
    [ != "" id: String
}
"#);
        assert!(result.is_err());
    }

    // ---- Value Tests ----

    #[test]
    fn test_value_types() {
        let input = r#"
test as Vals {
    "string",
    42,
    3.14,
    true,
    false,
    [1, 2, 3],
    { a: 1, b: "two" }
}
"#;
        let doc = parse_document(input).unwrap();
        let entry = &doc.data_groups[0].entries[0];
        assert_eq!(entry.fields.len(), 7);
        match &entry.fields[0] {
            DataField::Positional(DataValue::String(s)) => assert_eq!(s, "string"),
            _ => panic!("expected string"),
        }
        match &entry.fields[1] {
            DataField::Positional(DataValue::Int(n)) => assert_eq!(*n, 42),
            _ => panic!("expected int"),
        }
        match &entry.fields[2] {
            DataField::Positional(DataValue::Float(f)) => assert!((*f - 3.14).abs() < 1e-10),
            _ => panic!("expected float"),
        }
        match &entry.fields[3] {
            DataField::Positional(DataValue::Bool(b)) => assert!(*b),
            _ => panic!("expected bool true"),
        }
        match &entry.fields[4] {
            DataField::Positional(DataValue::Bool(b)) => assert!(!*b),
            _ => panic!("expected bool false"),
        }
        match &entry.fields[5] {
            DataField::Positional(DataValue::List(l)) => assert_eq!(l.len(), 3),
            _ => panic!("expected list"),
        }
        match &entry.fields[6] {
            DataField::Positional(DataValue::Map(m)) => {
                assert_eq!(m.len(), 2);
            }
            _ => panic!("expected map"),
        }
    }

    #[test]
    fn test_empty_document() {
        let doc = parse_document("").unwrap();
        assert_eq!(doc.imports.len(), 0);
        assert_eq!(doc.schemas.len(), 0);
        assert_eq!(doc.data_groups.len(), 0);
        assert_eq!(doc.rules.len(), 0);
    }

    #[test]
    fn test_only_comments() {
        let doc = parse_document("// just a comment\n// another one\n").unwrap();
        assert_eq!(doc.schemas.len(), 0);
    }

    #[test]
    fn test_negative_int() {
        let input = r#"k as Vals { -42 }"#;
        let doc = parse_document(input).unwrap();
        let entry = &doc.data_groups[0].entries[0];
        match &entry.fields[0] {
            DataField::Positional(DataValue::Int(n)) => assert_eq!(*n, -42),
            _ => panic!("expected -42"),
        }
    }

    #[test]
    fn test_multiple_imports() {
        let input = r#"
import "a.dbvs"
import "b.dbvs"
import "c.dbvs"
"#;
        let doc = parse_document(input).unwrap();
        assert_eq!(doc.imports.len(), 3);
    }

    #[test]
    fn test_escape_sequences() {
        let input = r#"k as S { "hello\nworld" }"#;
        let doc = parse_document(input).unwrap();
        let entry = &doc.data_groups[0].entries[0];
        match &entry.fields[0] {
            DataField::Positional(DataValue::String(s)) => {
                assert_eq!(s, "hello\nworld");
            }
            _ => panic!("expected string with newline"),
        }
    }
}
