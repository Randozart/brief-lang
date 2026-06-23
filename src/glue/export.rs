// GLUE Export Pipeline
//
// `brief export <bridge.bv> <language>` — compiles a Brief bridge to a
// native archive (.a), then invokes the language's `$!` adapter macro to
// generate native wrapper source files alongside the archive.
//
// Pipeline:
//   1. Parse the bridge .bv to collect #export/frgn/meld declarations.
//   2. Compile to .a via `brief build --library` (LLVM backend).
//   3. Read glue.dbvl to find the language adapter entry.
//   4. Validate against glue.dbvs schema.
//   5. Invoke the adapter's `$!macro` which calls emit_file#() to write
//      native source files into the output directory.
//
// Architecture: Adapters are Brief `$!` macros (glue/adapters/<lang>.bv),
// not Rust template engines. The Rust side simply compiles the bridge and
// calls into the macro system. This keeps all language-specific logic in
// Brief code that survives self-hosting.

use crate::ast::{Hashtag, OutputType, Program, ResultType, TopLevel};
use crate::features::macros::context::MacroContext;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

/// Information extracted from a bridge .bv file for adapter consumption.
/// This is the structured representation of everything a language adapter
/// needs to know to generate wrapper code.
#[derive(Debug, Clone)]
pub struct BridgeInfo {
    /// Name of the bridge (derived from filename)
    pub name: String,
    /// Functions exported to the foreign language (via #export pragma)
    pub exports: Vec<ExportDecl>,
    /// Foreign functions called by the bridge (via frgn declarations)
    pub frgns: Vec<FrgnDecl>,
    /// Meld route declarations (for type mapping)
    pub melds: Vec<MeldDecl>,
}

/// A function exported to the foreign language via `#export` pragma.
#[derive(Debug, Clone)]
pub struct ExportDecl {
    pub name: String,
    pub params: Vec<(String, String)>,  // (name, type string)
    pub return_type: String,
}

/// A foreign function declared via `frgn` in the bridge.
#[derive(Debug, Clone)]
pub struct FrgnDecl {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub return_type: String,
    /// If the frgn name matches an Intrinsic variant, list it here
    pub intrinsic_match: Option<String>,
}

/// A meld route declaration for type compatibility.
#[derive(Debug, Clone)]
pub struct MeldDecl {
    pub from_type: String,
    pub to_type: String,
    pub route: String,
}

/// Registry entry for a language adapter, parsed from glue.dbvl.
#[derive(Debug, Clone)]
pub struct AdapterEntry {
    pub language: String,
    pub macro_path: String,
    pub file_extension: String,
    pub type_map: HashMap<String, String>,
}

/// Extract bridge information from a parsed Program.
/// Walks the AST to find #export pragmas, frgn declarations, and meld routes.
pub fn extract_bridge_info(program: &Program, name: &str) -> BridgeInfo {
    BridgeInfo {
        name: name.to_string(),
        exports: extract_exports(program),
        frgns: extract_frgns(program),
        melds: extract_melds(program),
    }
}

fn has_export_modifier(modifiers: &[Hashtag]) -> bool {
    modifiers.iter().any(|m| m.name == "export")
}

fn extract_exports(program: &Program) -> Vec<ExportDecl> {
    let mut exports = Vec::new();
    for item in &program.items {
        if let TopLevel::Definition(defn) = item {
            if !has_export_modifier(&defn.modifiers) {
                continue;
            }
            let params: Vec<(String, String)> = defn.parameters.iter()
                .map(|p| (p.0.clone(), format_type(&p.1)))
                .collect();
            let return_type = defn.output_type.as_ref()
                .map(|ot| format_output_type(ot))
                .unwrap_or_else(|| "Void".to_string());
            exports.push(ExportDecl {
                name: defn.name.clone(),
                params,
                return_type,
            });
        }
    }
    exports
}

fn extract_frgns(program: &Program) -> Vec<FrgnDecl> {
    let mut frgns = Vec::new();
    for item in &program.items {
        if let TopLevel::Signature(sig) = item {
            let return_type = format_result_type(&sig.result_type);
            frgns.push(FrgnDecl {
                name: sig.name.clone(),
                params: sig.params.iter()
                    .map(|p| (p.0.clone(), format_type(&p.1)))
                    .collect(),
                return_type,
                intrinsic_match: None,
            });
        }
    }
    frgns
}

fn extract_melds(program: &Program) -> Vec<MeldDecl> {
    let mut melds = Vec::new();
    for item in &program.items {
        if let TopLevel::Meld(meld) = item {
            for route in &meld.routes {
                melds.push(MeldDecl {
                    from_type: meld.name_a.clone(),
                    to_type: meld.name_b.clone(),
                    route: route.accessor.clone(),
                });
            }
        }
    }
    melds
}

fn format_type(ty: &crate::ast::Type) -> String {
    match ty {
        crate::ast::Type::Int => "Int".to_string(),
        crate::ast::Type::Float => "Float".to_string(),
        crate::ast::Type::Bool => "Bool".to_string(),
        crate::ast::Type::Char => "Char".to_string(),
        crate::ast::Type::String => "String".to_string(),
        crate::ast::Type::Data => "Data".to_string(),
        crate::ast::Type::Custom(name) => name.clone(),
        _ => format!("{:?}", ty),
    }
}

fn format_output_type(ot: &OutputType) -> String {
    match ot {
        OutputType::Single(ty) => format_type(ty),
        OutputType::Tuple(types) => {
            let parts: Vec<String> = types.iter().map(|t| format_output_type(t)).collect();
            parts.join("|")
        }
        OutputType::Union(types) => {
            let parts: Vec<String> = types.iter().map(|t| format_output_type(t)).collect();
            parts.join("|")
        }
        OutputType::Array(ty) => format!("{}[]", format_type(ty)),
        OutputType::Named(name, inner) => format!("{}:{}", name, format_output_type(inner)),
    }
}

fn format_result_type(rt: &ResultType) -> String {
    match rt {
        ResultType::Projection(types) => {
            let parts: Vec<String> = types.iter().map(|t| format_type(t)).collect();
            parts.join("|")
        }
        ResultType::TrueAssertion => "Bool".to_string(),
        ResultType::VoidType => "Void".to_string(),
    }
}

// =========================================================================
// DBVL Serialization — bridge info as D-Brief Lines
//
// Architecture: Bridge info flows from Rust → $!macro adapter via bare
// comma-separated DBVL strings (no JSON, no TOML). Each entry is one line,
// fields separated by commas. No quoting needed — none of our field values
// contain commas. The adapter macro splits by "\n" then by "," to extract.
//
// Format per entry type:
//   exports:  name, param_types_pipe_separated, return_type
//   frgns:    name, param_types_pipe_separated, return_type, intrinsic_match
//   melds:    from_type, to_type, route
// =========================================================================

/// Serialize exports to DBVL format: one line per export.
/// Fields: name, param_types (pipe-separated), return_type
fn serialize_exports_dbvl(exports: &[ExportDecl]) -> String {
    exports.iter().map(|e| {
        let params: Vec<String> = e.params.iter().map(|(_, t)| t.clone()).collect();
        format!("{},{},{}", e.name, params.join("|"), e.return_type)
    }).collect::<Vec<_>>().join("\n")
}

/// Serialize frgns to DBVL format: one line per frgn.
/// Fields: name, param_types (pipe-separated), return_type, intrinsic_match
fn serialize_frgns_dbvl(frgns: &[FrgnDecl]) -> String {
    frgns.iter().map(|f| {
        let params: Vec<String> = f.params.iter().map(|(_, t)| t.clone()).collect();
        let intrinsic = f.intrinsic_match.as_deref().unwrap_or("");
        format!("{},{},{},{}", f.name, params.join("|"), f.return_type, intrinsic)
    }).collect::<Vec<_>>().join("\n")
}

/// Serialize melds to DBVL format: one line per meld.
/// Fields: from_type, to_type, route
fn serialize_melds_dbvl(melds: &[MeldDecl]) -> String {
    melds.iter().map(|m| {
        format!("{},{},{}", m.from_type, m.to_type, m.route)
    }).collect::<Vec<_>>().join("\n")
}

/// Parse a DBVL type map field like "{Int:i64 Float:f64 Bool:bool}"
/// into a HashMap. Matches dbvl_reader::parse_map() behavior.
fn parse_type_map(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let trimmed = s.trim();
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

/// Find a language adapter entry in glue.dbvl.
pub fn find_adapter(language: &str, dbvl_path: &Path) -> Result<AdapterEntry, String> {
    let source = fs::read_to_string(dbvl_path)
        .map_err(|e| format!("Failed to read {}: {}", dbvl_path.display(), e))?;

    let file = crate::glue::dbvl_reader::parse_dbvl(&source);
    for entry in &file.entries {
        let fields = match entry {
            crate::glue::dbvl_reader::DbvlEntry::Raw(tokens) => tokens,
            crate::glue::dbvl_reader::DbvlEntry::Validated { fields, .. } => fields,
        };
        if fields.len() >= 3 && fields[0] == language {
            let type_map = if fields.len() > 4 {
                parse_type_map(&fields[4])
            } else {
                HashMap::new()
            };
            return Ok(AdapterEntry {
                language: language.to_string(),
                macro_path: fields[1].clone(),
                file_extension: fields[2].clone(),
                type_map,
            });
        }
    }
    Err(format!("Adapter not found for language '{}' in {}", language, dbvl_path.display()))
}

/// Run the export pipeline:
/// 1. Extract bridge info from the program
/// 2. Find the language adapter
/// 3. Set up the BRIEF_OUTPUT_DIR env var so emit_file#() writes to the right place
/// 4. Invoke the adapter macro via the $! macro system
pub fn run_export(
    program: &Program,
    bridge_name: &str,
    language: &str,
    out_dir: &Path,
    dbvl_path: &Path,
) -> Result<(), String> {
    // Step 1: Extract bridge info
    let info = extract_bridge_info(program, bridge_name);
    println!("  Bridge '{}': {} exports, {} frgns, {} melds",
        info.name, info.exports.len(), info.frgns.len(), info.melds.len());

    // Step 2: Find the adapter entry
    let adapter = find_adapter(language, dbvl_path)?;
    println!("  Adapter: {} (macro: {})", adapter.language, adapter.macro_path);

    // Step 3: Create output directory
    let output_dir = out_dir.join(format!("{}-bridge", bridge_name));
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    // Step 4: Set BRIEF_OUTPUT_DIR so emit_file#() writes to the right place
    // We use the env var approach because the $! macro system's sandboxed
    // interpreter reads BRIEF_OUTPUT_DIR to determine where to write files.
    // This avoids passing Rust-side state into the macro expansion sandbox.
    // SAFETY: set_var is unsafe in modern Rust (race condition with other
    // threads reading env vars). In the CLI context, this runs during a single
    // compiler invocation with no concurrent env readers, so it is safe.
    unsafe { std::env::set_var("BRIEF_OUTPUT_DIR", output_dir.to_str().unwrap_or(".")); }

    // Step 5: Load and invoke the adapter macro
    // The adapter macro is a .bv file that defines a $!macro taking the
    // bridge info and calling emit_file#() to generate wrapper sources.
    let macro_source = fs::read_to_string(&adapter.macro_path)
        .map_err(|e| format!("Failed to read adapter macro '{}': {}", adapter.macro_path, e))?;

    // Parse and expand the adapter macro
    let mut macro_parser = crate::parser::Parser::new(&macro_source);
    let mut macro_program = macro_parser.parse()
        .map_err(|e| format!("Failed to parse adapter macro: {}", e))?;

    // Create a macro context and register the adapter
    let mut ctx = MacroContext::new();
    // Collect macro definitions from the adapter file
    crate::features::macros::expand::collect_macro_defs(&mut macro_program, &mut ctx);

    // Build a program that calls the adapter macro with the bridge info
    // We construct a synthetic TopLevel that invokes the adapter's macro
    // with the bridge info serialized as D-Brief Lines (.dbvl) format.
    //
    // Architecture: Bridge info uses D-Brief Lines format (not JSON, not TOML)
    // because it is the native Brief data interchange format. The adapter macro
    // receives dbvl-formatted strings and uses Brief string operations (split,
    // trim, etc.) to extract fields. This keeps the entire adapter pipeline in
    // Brief-native data — no JSON dependency in the macro system.
    //
    // DBVL format per line: bare comma-separated fields (no quoting).
    //   exports:  name, param_types|pipe|separated, return_type
    //   frgns:    name, param_types|pipe|separated, return_type, intrinsic_match
    //   melds:    from_type, to_type, route
    //
    let macro_name = format!("generate_{}_wrapper", language);
    let exports_dbvl = serialize_exports_dbvl(&info.exports);
    let frgns_dbvl = serialize_frgns_dbvl(&info.frgns);
    let melds_dbvl = serialize_melds_dbvl(&info.melds);
    let adapter_args = vec![
        crate::ast::Expr::String(info.name.clone()),
        crate::ast::Expr::String(exports_dbvl),
        crate::ast::Expr::String(frgns_dbvl),
        crate::ast::Expr::String(melds_dbvl),
    ];
    let call_stmt = crate::ast::Statement::Expression(
        crate::ast::Expr::MacroCall {
            name: macro_name,
            args: adapter_args,
            block: None,
            span: None,
        },
    );

    // Create a minimal program and expand it
    let mut expand_program = crate::ast::Program {
        items: vec![crate::ast::TopLevel::Statement(Box::new(call_stmt))],
        comments: Vec::new(),
        reactor_speed: None,
        attrs: Vec::new(),
        ffi: None,
        strict_mode: crate::ast::StrictMode::Off,
        dispatch_mode: crate::ast::DispatchMode::default(),
        exit_condition: None,
        out_pragmas: Vec::new(),
        default_sig_modifier: None,
    };

    // Phase 1b: Expand the macro call — this will invoke the adapter's
    // $!macro which in turn calls emit_file#() to write wrapper sources.
    match crate::features::macros::expand::expand_macros(&mut expand_program, &mut ctx) {
        Ok(_) => {
            println!("  Wrapper generated in: {}", output_dir.display());
            Ok(())
        }
        Err(e) => Err(format!("Adapter macro expansion failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_exports_dbvl_empty() {
        let result = serialize_exports_dbvl(&[]);
        assert_eq!(result, "");
    }

    #[test]
    fn test_serialize_exports_dbvl_single() {
        let exports = vec![ExportDecl {
            name: "print_int".to_string(),
            params: vec![("n".to_string(), "Int".to_string())],
            return_type: "Int".to_string(),
        }];
        let result = serialize_exports_dbvl(&exports);
        assert_eq!(result, "print_int,Int,Int");
    }

    #[test]
    fn test_serialize_exports_dbvl_multiple_params() {
        let exports = vec![ExportDecl {
            name: "write_file".to_string(),
            params: vec![
                ("path".to_string(), "String".to_string()),
                ("data".to_string(), "Data".to_string()),
            ],
            return_type: "Int".to_string(),
        }];
        let result = serialize_exports_dbvl(&exports);
        assert_eq!(result, "write_file,String|Data,Int");
    }

    #[test]
    fn test_serialize_frgns_dbvl_with_intrinsic() {
        let frgns = vec![FrgnDecl {
            name: "sqrt".to_string(),
            params: vec![("x".to_string(), "Float".to_string())],
            return_type: "Float".to_string(),
            intrinsic_match: Some("sqrt".to_string()),
        }];
        let result = serialize_frgns_dbvl(&frgns);
        assert_eq!(result, "sqrt,Float,Float,sqrt");
    }

    #[test]
    fn test_serialize_melds_dbvl() {
        let melds = vec![MeldDecl {
            from_type: "CBuffer".to_string(),
            to_type: "RSBuffer".to_string(),
            route: "identity".to_string(),
        }];
        let result = serialize_melds_dbvl(&melds);
        assert_eq!(result, "CBuffer,RSBuffer,identity");
    }

    #[test]
    fn test_parse_type_map() {
        let map = parse_type_map("{Int:i64 Float:f64 Bool:bool}");
        assert_eq!(map.get("Int"), Some(&"i64".to_string()));
        assert_eq!(map.get("Float"), Some(&"f64".to_string()));
        assert_eq!(map.get("Bool"), Some(&"bool".to_string()));
    }

    #[test]
    fn test_parse_type_map_empty() {
        let map = parse_type_map("{}");
        assert!(map.is_empty());
    }
}
