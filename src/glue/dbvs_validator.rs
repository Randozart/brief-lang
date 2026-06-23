// Data Brief Schema (.dbvs) parser and validator
//
// A .dbvs file defines the positional schema for .dbvl and .dbv entries.
// Each `entry Name { field: Type; ... };` block describes one schema.
// Field types: String, Int, Enum<A,B>, List<T; delimiter=X>, Map<K,V; ...>, Optional<T>.
// Semicolons terminate declarations and entry bodies.

use std::collections::HashMap;

/// A field type in a .dbvs schema.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    String,
    Int,
    Enum(Vec<String>),
    List {
        inner: Box<FieldType>,
        delimiter: Option<String>,
    },
    Map {
        key_type: Box<FieldType>,
        val_type: Box<FieldType>,
        pair_separator: Option<String>,
        value_delimiter: Option<String>,
        brace_required: bool,
    },
    Optional(Box<FieldType>),
}

/// One field declaration in a .dbvs schema entry.
#[derive(Debug, Clone)]
pub struct SchemaField {
    pub name: String,
    pub ty: FieldType,
}

/// A parsed .dbvs schema — an ordered list of field declarations.
#[derive(Debug, Clone)]
pub struct Schema {
    pub name: String,
    pub fields: Vec<SchemaField>,
}

/// Parse a .dbvs schema string into a Schema struct.
/// Currently supports the subset needed for GLUE adapter validation:
///   entry Name { field: Type; ... };
/// Field types: String, Int, Enum<A, B, C>, Optional<T>,
///   List<T; delimiter=X>, Map<K, V; pair_separator=:, value_delimiter=space, brace=required>
pub fn parse_schema(input: &str) -> Result<Schema, String> {
    // Strip // line comments before parsing
    let input: String = input.lines()
        .filter(|line| !line.trim().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let input = input.trim();

    // Strip outer `entry Name { ... };`
    let without_entry = input
        .strip_prefix("entry ")
        .ok_or_else(|| "schema must start with 'entry'".to_string())?;

    // Extract name
    let name_end = without_entry.find(|c: char| c == '{' || c.is_whitespace())
        .ok_or_else(|| "cannot find schema name".to_string())?;
    let name = without_entry[..name_end].trim().to_string();
    if name.is_empty() {
        return Err("empty schema name".to_string());
    }

    // Find opening brace
    let body_start = without_entry.find('{')
        .ok_or_else(|| "missing '{' in schema entry".to_string())?;
    let body = &without_entry[body_start + 1..];

    // Find closing brace + semicolon
    let body_end = body.rfind("};")
        .ok_or_else(|| "missing '};' at end of schema entry".to_string())?;
    let body = body[..body_end].trim();

    if body.is_empty() {
        return Ok(Schema { name, fields: Vec::new() });
    }

    // Parse field declarations: semicolon-terminated, respecting < > angle brackets
    let mut fields = Vec::new();
    let mut decl = String::new();
    let mut angle_depth: i32 = 0;
    for c in body.chars() {
        match c {
            '<' => { angle_depth += 1; decl.push(c); }
            '>' => { angle_depth -= 1; decl.push(c); }
            ';' if angle_depth == 0 => {
                // End of field declaration
                let trimmed = decl.trim();
                if !trimmed.is_empty() {
                    let colon_pos = trimmed.find(':')
                        .ok_or_else(|| format!("missing ':' in field declaration '{}'", trimmed))?;
                    let field_name = trimmed[..colon_pos].trim().to_string();
                    let type_str = trimmed[colon_pos + 1..].trim();
                    let ty = parse_field_type(type_str)?;
                    fields.push(SchemaField { name: field_name, ty });
                }
                decl = String::new();
            }
            _ => { decl.push(c); }
        }
    }
    // Handle any trailing declaration without trailing semicolon
    let trimmed = decl.trim();
    if !trimmed.is_empty() {
        let colon_pos = trimmed.find(':')
            .ok_or_else(|| format!("missing ':' in field declaration '{}'", trimmed))?;
        let field_name = trimmed[..colon_pos].trim().to_string();
        let type_str = trimmed[colon_pos + 1..].trim();
        let ty = parse_field_type(type_str)?;
        fields.push(SchemaField { name: field_name, ty });
    }

    Ok(Schema { name, fields })
}

/// Parse a field type string into a FieldType enum.
fn parse_field_type(type_str: &str) -> Result<FieldType, String> {
    let type_str = type_str.trim();

    if type_str == "String" {
        return Ok(FieldType::String);
    }
    if type_str == "Int" {
        return Ok(FieldType::Int);
    }

    // Enum<A, B, C>
    if let Some(inner) = type_str.strip_prefix("Enum<") {
        let inner = inner.strip_suffix('>')
            .ok_or_else(|| "missing '>' in Enum".to_string())?;
        let variants: Vec<String> = inner.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        return Ok(FieldType::Enum(variants));
    }

    // Optional<T>
    if let Some(inner) = type_str.strip_prefix("Optional<") {
        let inner = inner.strip_suffix('>')
            .ok_or_else(|| "missing '>' in Optional".to_string())?;
        let inner_ty = parse_field_type(inner)?;
        return Ok(FieldType::Optional(Box::new(inner_ty)));
    }

    // List<T; delimiter=X>
    if let Some(inner) = type_str.strip_prefix("List<") {
        let inner = inner.strip_suffix('>')
            .ok_or_else(|| "missing '>' in List".to_string())?;
        let (ty_part, params) = inner.split_once(';')
            .ok_or_else(|| "missing ';' in List params".to_string())?;
        let inner_ty = parse_field_type(ty_part.trim())?;
        let delimiter = params.split('=')
            .nth(1).map(|s| s.trim().to_string());
        return Ok(FieldType::List { inner: Box::new(inner_ty), delimiter });
    }

    // Map<K, V; pair_separator=:, value_delimiter=space, brace=required>
    if let Some(inner) = type_str.strip_prefix("Map<") {
        let inner = inner.strip_suffix('>')
            .ok_or_else(|| "missing '>' in Map".to_string())?;

        // Parse key-value types from the first ';'
        let (kv_types, params_str) = inner.split_once(';')
            .ok_or_else(|| "missing ';' in Map params".to_string())?;

        let kv_types = kv_types.trim();
        let (key_str, val_str) = kv_types.split_once(',')
            .ok_or_else(|| "missing ',' in Map key,val".to_string())?;

        let key_ty = parse_field_type(key_str.trim())?;
        let val_ty = parse_field_type(val_str.trim())?;

        // Parse additional params
        let mut pair_separator: Option<String> = Some(":".to_string());
        let mut value_delimiter: Option<String> = Some("space".to_string());
        let mut brace_required = true;

        for param in params_str.split(',') {
            let param = param.trim();
            if let Some(val) = param.strip_prefix("pair_separator=") {
                pair_separator = Some(val.trim().to_string());
            } else if let Some(val) = param.strip_prefix("value_delimiter=") {
                value_delimiter = Some(val.trim().to_string());
            } else if let Some(val) = param.strip_prefix("brace=") {
                brace_required = val.trim() == "required";
            }
        }

        return Ok(FieldType::Map {
            key_type: Box::new(key_ty),
            val_type: Box::new(val_ty),
            pair_separator,
            value_delimiter,
            brace_required,
        });
    }

    Err(format!("unknown field type '{}'", type_str))
}

/// Validate a list of fields against a schema.
/// Returns Ok(()) if valid, Err with description if not.
pub fn validate_fields(schema: &Schema, fields: &[String]) -> Result<(), String> {
    if fields.len() != schema.fields.len() {
        return Err(format!(
            "expected {} fields (schema '{}') but got {}: {:?}",
            schema.fields.len(), schema.name, fields.len(), fields
        ));
    }
    // Type-specific validation can be added here
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entry_string() {
        let input = "entry Test { name: String; };";
        let schema = parse_schema(input).unwrap();
        assert_eq!(schema.name, "Test");
        assert_eq!(schema.fields.len(), 1);
        assert_eq!(schema.fields[0].name, "name");
        assert_eq!(schema.fields[0].ty, FieldType::String);
    }

    #[test]
    fn test_parse_entry_enum() {
        let input = "entry Test { link: Enum<static, dynamic>; };";
        let schema = parse_schema(input).unwrap();
        assert_eq!(schema.fields.len(), 1);
        match &schema.fields[0].ty {
            FieldType::Enum(variants) => {
                assert_eq!(variants.len(), 2);
                assert!(variants.contains(&"static".to_string()));
                assert!(variants.contains(&"dynamic".to_string()));
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_parse_entry_map() {
        let input = "entry Test { types: Map<String, String; pair_separator=:, value_delimiter=space, brace=required>; };";
        let schema = parse_schema(input).unwrap();
        assert_eq!(schema.fields.len(), 1);
        match &schema.fields[0].ty {
            FieldType::Map { brace_required, .. } => {
                assert!(*brace_required);
            }
            _ => panic!("expected Map"),
        }
    }

    #[test]
    fn test_validate_correct_fields() {
        let input = "entry Test { a: String; b: Int; };";
        let schema = parse_schema(input).unwrap();
        let fields = vec!["hello".to_string(), "42".to_string()];
        assert!(validate_fields(&schema, &fields).is_ok());
    }

    #[test]
    fn test_validate_wrong_field_count() {
        let input = "entry Test { a: String; b: Int; };";
        let schema = parse_schema(input).unwrap();
        let fields = vec!["hello".to_string()];
        assert!(validate_fields(&schema, &fields).is_err());
    }

    #[test]
    fn test_parse_optional() {
        let input = "entry Test { build: Optional<String>; };";
        let schema = parse_schema(input).unwrap();
        assert_eq!(schema.fields.len(), 1);
        match &schema.fields[0].ty {
            FieldType::Optional(inner) => {
                assert_eq!(**inner, FieldType::String);
            }
            _ => panic!("expected Optional"),
        }
    }

    #[test]
    fn test_empty_schema() {
        let input = "entry Empty { };";
        let schema = parse_schema(input).unwrap();
        assert_eq!(schema.name, "Empty");
        assert_eq!(schema.fields.len(), 0);
    }
}
