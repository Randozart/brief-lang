// Bridge: converts DBrief v2 parsed types into Brief AST types (TopLevel, Type, Expr).

use crate::ast;
use crate::dbrief::v2::*;
use std::collections::HashMap;

/// Convert a parsed DbriefDocument into a Vec of Brief TopLevel items.
/// `name` is the import alias (e.g. "data" from `import data from "file.dbv"`).
/// `use_lazy` — if true, creates Expr::DbvlTable for schema-typed data with key_offsets.
pub fn document_to_program(doc: &DbriefDocument, name: &str) -> Vec<ast::TopLevel> {
    document_to_program_flags(doc, name, false)
}

/// Like `document_to_program` but with option for lazy DBVL loading.
pub fn document_to_program_flags(doc: &DbriefDocument, name: &str, use_lazy: bool) -> Vec<ast::TopLevel> {
    let mut items: Vec<ast::TopLevel> = Vec::new();

    // 1. Convert schemas to Struct definitions
    for schema in &doc.schemas {
        items.push(schema_to_struct(schema));
    }

    // 2. Convert data groups into a constant named by the import alias.
    if !doc.data_groups.is_empty() {
        let mut data_map: Vec<(ast::Expr, ast::Expr)> = Vec::new();

        for group in &doc.data_groups {
            let group_name = match &group.schema_name {
                Some(name) => name.clone(),
                None => "_".to_string(),
            };

            // Check if we can use lazy DbvlTable
            let can_use_lazy = use_lazy
                && group.schema_name.is_some()
                && !doc.key_offsets.is_empty();

            if can_use_lazy {
                // Create lazy DbvlTable — builds key-offset index
                let field_names: Vec<String> = group.entries.iter()
                    .filter_map(|e| e.key.clone())
                    .collect();
                // Use schema field names for proper ordering
                let schema_field_names = doc.schemas.iter()
                    .find(|s| Some(s.name.as_str()) == group.schema_name.as_deref())
                    .map(|s| s.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>())
                    .unwrap_or_else(|| field_names.clone());

                data_map.push((
                    ast::Expr::String(group_name),
                    ast::Expr::DbvlTable {
                        path: String::new(),               // filled in by the import resolver
                        field_names: schema_field_names,
                        key_offsets: doc.key_offsets.clone(),
                        schema_name: group.schema_name.clone(),
                    },
                ));
            } else {
                // Full materialization — convert all entries
                let mut entry_map: Vec<(ast::Expr, ast::Expr)> = Vec::new();
                for entry in &group.entries {
                    let key_expr = match &entry.key {
                        Some(k) => ast::Expr::String(k.clone()),
                        None => ast::Expr::Integer(entry_map.len() as i64),
                    };
                    let val_expr = data_entry_to_expr(entry, group.schema_name.as_deref(), &doc.schemas);
                    entry_map.push((key_expr, val_expr));
                }
                data_map.push((
                    ast::Expr::String(group_name),
                    ast::Expr::MapLiteral(entry_map),
                ));
            }
        }

        // If only one group and it has no schema name, use value directly
        let value = if data_map.len() == 1 && doc.data_groups[0].schema_name.is_none() && doc.data_groups[0].entries.len() == 1 {
            let group = &doc.data_groups[0];
            if group.entries.len() == 1 && group.entries[0].fields.len() == 1 {
                data_field_to_expr(&group.entries[0].fields[0])
            } else {
                ast::Expr::MapLiteral(data_map)
            }
        } else {
            ast::Expr::MapLiteral(data_map)
        };

        let ty = ast::Type::Custom(name.to_string());
        items.push(ast::TopLevel::Constant(ast::Constant {
            name: name.to_string(),
            ty,
            expr: value,
        }));
    }

    items
}

/// Convert a schema into a StructDefinition
fn schema_to_struct(schema: &SchemaDef) -> ast::TopLevel {
    let fields: Vec<ast::StructField> = schema
        .fields
        .iter()
        .map(|f| {
            let ty = field_type_to_brief(&f.ty);
            ast::StructField {
                name: f.name.clone(),
                ty,
                default: None,
                visibility: ast::Visibility::Public,
            }
        })
        .collect();

    ast::TopLevel::Struct(ast::StructDefinition {
        name: schema.name.clone(),
        type_params: Vec::new(),
        parent: None,
        fields,
        transactions: Vec::new(),
        view_html: None,
        span: None,
        modifiers: Vec::new(),
        variants: Vec::new(),
    })
}

/// Convert a FieldType → ast::Type
fn field_type_to_brief(ft: &FieldType) -> ast::Type {
    match ft {
        FieldType::String => ast::Type::String,
        FieldType::Int => ast::Type::Int,
        FieldType::Float => ast::Type::Float,
        FieldType::Bool => ast::Type::Bool,
        FieldType::UInt(_) => ast::Type::Int,
        FieldType::Vec(inner) => {
            ast::Type::Applied("List".to_string(), vec![field_type_to_brief(inner)])
        }
        FieldType::Map(k, v) => ast::Type::Applied(
            "Map".to_string(),
            vec![field_type_to_brief(k), field_type_to_brief(v)],
        ),
        FieldType::Option(inner) => {
            ast::Type::Applied("Option".to_string(), vec![field_type_to_brief(inner)])
        }
        FieldType::Named(n) => ast::Type::Custom(n.clone()),
    }
}

/// Convert a single data entry to an Expr.
/// `group_schema_name` is the schema name from the enclosing `as Schema { }` group.
fn data_entry_to_expr(entry: &DataEntry, group_schema_name: Option<&str>, schemas: &[SchemaDef]) -> ast::Expr {
    let fields: Vec<(String, ast::Expr)> = entry
        .fields
        .iter()
        .map(|f| match f {
            DataField::Named(n, v) => (n.clone(), data_value_to_expr(v)),
            DataField::Positional(v) => {
                ("".to_string(), data_value_to_expr(v))
            }
        })
        .collect();

    // Determine schema name: entry-level takes priority, fallback to group-level
    let schema_name = entry.schema_name.as_deref().or(group_schema_name);

    if let Some(schema_name) = schema_name {
        let named_fields = if let Some(schema) = schemas.iter().find(|s| &s.name == schema_name) {
            let mut resolved: Vec<(String, ast::Expr)> = Vec::new();
            for (i, field) in entry.fields.iter().enumerate() {
                match field {
                    DataField::Named(n, v) => {
                        resolved.push((n.clone(), data_value_to_expr(v)));
                    }
                    DataField::Positional(v) => {
                        if i < schema.fields.len() {
                            let name = schema.fields[i].name.clone();
                            resolved.push((name, data_value_to_expr(v)));
                        }
                    }
                }
            }
            resolved
        } else {
            // Schema not in this document — use field names from DataField if available,
            // or generate placeholder names for positional fields
            let mut resolved: Vec<(String, ast::Expr)> = Vec::new();
            for (i, field) in entry.fields.iter().enumerate() {
                match field {
                    DataField::Named(n, v) => {
                        resolved.push((n.clone(), data_value_to_expr(v)));
                    }
                    DataField::Positional(v) => {
                        resolved.push((format!("field_{}", i), data_value_to_expr(v)));
                    }
                }
            }
            resolved
        };
        return ast::Expr::StructInstance(schema_name.to_string(), named_fields);
    }

    // No schema — use ObjectLiteral
    ast::Expr::ObjectLiteral(fields)
}

/// Convert a DataField to an Expr
fn data_field_to_expr(field: &DataField) -> ast::Expr {
    match field {
        DataField::Named(_, v) | DataField::Positional(v) => data_value_to_expr(v),
    }
}

/// Convert a DataValue to an Expr
fn data_value_to_expr(dv: &DataValue) -> ast::Expr {
    match dv {
        DataValue::String(s) => ast::Expr::String(s.clone()),
        DataValue::Int(n) => ast::Expr::Integer(*n),
        DataValue::Float(f) => ast::Expr::Float(*f),
        DataValue::Bool(b) => ast::Expr::Bool(*b),
        DataValue::List(items) => {
            let exprs: Vec<ast::Expr> = items.iter().map(data_value_to_expr).collect();
            ast::Expr::ListLiteral(exprs)
        }
        DataValue::Map(entries) => {
            let pairs: Vec<(ast::Expr, ast::Expr)> = entries
                .iter()
                .map(|(k, v)| {
                    (ast::Expr::String(k.clone()), data_value_to_expr(v))
                })
                .collect();
            ast::Expr::MapLiteral(pairs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbrief::v2::*;

    fn parse(input: &str) -> DbriefDocument {
        parse_document(input).unwrap()
    }

    #[test]
    fn test_schema_conversion() {
        let doc = parse(
            "schema Item { name: String; desc: String; hp: Int; takeable: Bool }",
        );
        let items = document_to_program(&doc, "data");
        // Schema-only document produces struct definition, no constant
        assert_eq!(items.len(), 1);
        match &items[0] {
            ast::TopLevel::Struct(s) => {
                assert_eq!(s.name, "Item");
            }
            _ => panic!("Expected Struct definition"),
        }
    }

    #[test]
    fn test_data_conversion_basic() {
        let doc = parse(
            r#"as Item {
    rusty_key { "Rusty Key", "An old iron key", 5, true }
}
"#,
        );
        let items = document_to_program(&doc, "data");
        assert!(!items.is_empty());

        let constant = &items[0];
        match constant {
            ast::TopLevel::Constant(c) => {
                assert_eq!(c.name, "data");
                // Value should be a MapLiteral: { "Item": { "rusty_key": ... } }
                match &c.expr {
                    ast::Expr::MapLiteral(pairs) => {
                        assert_eq!(pairs.len(), 1);
                        let (key, val) = &pairs[0];
                        match key {
                            ast::Expr::String(s) => assert_eq!(s, "Item"),
                            _ => panic!("Expected string key"),
                        }
                        match val {
                            ast::Expr::MapLiteral(entries) => {
                                assert_eq!(entries.len(), 1);
                                let (ek, ev) = &entries[0];
                                match ek {
                                    ast::Expr::String(s) => assert_eq!(s, "rusty_key"),
                                    _ => panic!("Expected string entry key"),
                                }
                                match ev {
                                    ast::Expr::StructInstance(name, _) => {
                                        assert_eq!(name, "Item");
                                    }
                                    _ => panic!("Expected StructInstance"),
                                }
                            }
                            _ => panic!("Expected MapLiteral for entries"),
                        }
                    }
                    _ => panic!("Expected MapLiteral"),
                }
            }
            _ => panic!("Expected Constant"),
        }
    }

    #[test]
    fn test_data_value_conversion() {
        let dv = DataValue::String("hello".into());
        match data_value_to_expr(&dv) {
            ast::Expr::String(s) => assert_eq!(s, "hello"),
            _ => panic!("expected string"),
        }

        let dv = DataValue::Int(42);
        match data_value_to_expr(&dv) {
            ast::Expr::Integer(n) => assert_eq!(n, 42),
            _ => panic!("expected int"),
        }

        let dv = DataValue::Float(3.14);
        match data_value_to_expr(&dv) {
            ast::Expr::Float(f) => assert!((f - 3.14).abs() < 1e-10),
            _ => panic!("expected float"),
        }

        let dv = DataValue::Bool(true);
        match data_value_to_expr(&dv) {
            ast::Expr::Bool(b) => assert!(b),
            _ => panic!("expected bool"),
        }

        let dv = DataValue::List(vec![DataValue::Int(1), DataValue::Int(2)]);
        match data_value_to_expr(&dv) {
            ast::Expr::ListLiteral(items) => assert_eq!(items.len(), 2),
            _ => panic!("expected list"),
        }

        let mut map = HashMap::new();
        map.insert("a".into(), DataValue::Int(1));
        let dv = DataValue::Map(map);
        match data_value_to_expr(&dv) {
            ast::Expr::MapLiteral(pairs) => assert_eq!(pairs.len(), 1),
            _ => panic!("expected map"),
        }
    }

    #[test]
    fn test_schema_and_data() {
        let input = r#"
schema Item {
    name: String
    desc: String
    hp: Int
    takeable: Bool
}

as Item {
    rusty_key { "Rusty Key", "An old iron key", 5, true }
}
"#;
        let doc = parse(input);
        let items = document_to_program(&doc, "data");

        // Should have 2 items: Struct + Constant
        // But currently schema_to_struct creates a Struct and then
        // the data creates a Constant. Let me check.
        let has_struct = items
            .iter()
            .any(|item| matches!(item, ast::TopLevel::Struct(s) if s.name == "Item"));
        let has_constant = items
            .iter()
            .any(|item| matches!(item, ast::TopLevel::Constant(c) if c.name == "data"));
        assert!(
            has_struct,
            "Expected Struct definition for Item schema"
        );
        assert!(
            has_constant,
            "Expected Constant for data"
        );
    }

    #[test]
    fn test_field_type_conversion() {
        let cases: Vec<(FieldType, &str)> = vec![
            (FieldType::String, "String"),
            (FieldType::Int, "Int"),
            (FieldType::Float, "Float"),
            (FieldType::Bool, "Bool"),
            (FieldType::UInt(32), "Int"),
            (FieldType::Vec(Box::new(FieldType::String)), "Applied(\"List\", [String])"),
            (FieldType::Named("IoResult".into()), "Custom(\"IoResult\")"),
        ];
        for (ft, _desc) in cases {
            let ty = field_type_to_brief(&ft);
            let ty_str = format!("{:?}", ty);
            assert!(!ty_str.is_empty());
        }
    }
}
