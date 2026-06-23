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

use crate::ast::{Program, TopLevel};
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

fn extract_exports(program: &Program) -> Vec<ExportDecl> {
    let mut exports = Vec::new();
    for item in &program.items {
        match item {
            TopLevel::Definition(defn) if defn.export_name.is_some() => {
                let export_name = defn.export_name.clone().unwrap_or(defn.name.clone());
                let params: Vec<(String, String)> = defn.params.iter()
                    .map(|p| (p.0.clone(), format_type(&p.1)))
                    .collect();
                let return_type = format_type(&defn.return_type);
                exports.push(ExportDecl {
                    name: export_name,
                    params,
                    return_type,
                });
            }
            _ => {}
        }
    }
    exports
}

fn extract_frgns(program: &Program) -> Vec<FrgnDecl> {
    let mut frgns = Vec::new();
    for item in &program.items {
        if let TopLevel::Signature(sig) = item {
            frgns.push(FrgnDecl {
                name: sig.name.clone(),
                params: sig.params.iter()
                    .map(|p| (p.0.clone(), format_type(&p.1)))
                    .collect(),
                return_type: format_type(&sig.return_type.as_ref().unwrap_or(&crate::ast::Type::Infer)),
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
                    from_type: meld.from_type.name.clone(),
                    to_type: meld.to_type.name.clone(),
                    route: route.expression.to_string(),
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

/// Find a language adapter entry in glue.dbvl.
pub fn find_adapter(language: &str, dbvl_path: &Path) -> Result<AdapterEntry, String> {
    let source = fs::read_to_string(dbvl_path)
        .map_err(|e| format!("Failed to read {}: {}", dbvl_path.display(), e))?;

    let lines = crate::glue::dbvl_reader::parse_dbvl_lines(&source, false);
    for line in lines {
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if fields.len() >= 4 && fields[0].trim_matches('"') == language {
            let mut type_map = HashMap::new();
            if fields.len() > 4 {
                let map_str = fields[4].trim();
                if map_str.starts_with('{') && map_str.ends_with('}') {
                    let inner = &map_str[1..map_str.len()-1];
                    for pair in inner.split(',') {
                        let parts: Vec<&str> = pair.split(':').collect();
                        if parts.len() == 2 {
                            type_map.insert(
                                parts[0].trim().trim_matches('"').to_string(),
                                parts[1].trim().trim_matches('"').to_string(),
                            );
                        }
                    }
                }
            }
            return Ok(AdapterEntry {
                language: language.to_string(),
                macro_path: fields[1].trim_matches('"').to_string(),
                file_extension: fields[2].trim_matches('"').to_string(),
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
    std::env::set_var("BRIEF_OUTPUT_DIR", output_dir.to_str().unwrap_or("."));

    // Step 5: Load and invoke the adapter macro
    // The adapter macro is a .bv file that defines a $!macro taking the
    // bridge info and calling emit_file#() to generate wrapper sources.
    let macro_source = fs::read_to_string(&adapter.macro_path)
        .map_err(|e| format!("Failed to read adapter macro '{}': {}", adapter.macro_path, e))?;

    // Parse and expand the adapter macro
    let mut macro_parser = crate::parser::Parser::new(&macro_source);
    let macro_program = macro_parser.parse()
        .map_err(|e| format!("Failed to parse adapter macro: {}", e))?;

    // Create a macro context and register the adapter
    let mut ctx = MacroContext::new();
    // Collect macro definitions from the adapter file
    crate::features::macros::expand::collect_macro_defs(&macro_program, &mut ctx);

    // Build a program that calls the adapter macro with the bridge info
    // We construct a synthetic TopLevel that invokes the adapter's macro
    // with the bridge info serialized as D-Brief Lines (.dbvl) format.
    //
    // Architecture: Bridge info uses D-Brief Lines format (not JSON, not TOML)
    // because it is the native Brief data interchange format. The adapter macro
    // receives dbvl-formatted strings and uses Brief string operations (split,
    // trim_matches, etc.) to extract fields. This keeps the entire adapter
    // pipeline in Brief-native data — no JSON dependency in the macro system.
    //
    // DBVL format per line: quoted comma-separated fields.
    //   exports dbvl:   "name","param_types","return_type"
    //   frgns dbvl:     "name","param_types","return_type","intrinsic_match"
    //   melds dbvl:     "from_type","to_type","route"
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
        crate::ast::Expr::MacroCall(macro_name, adapter_args, None),
    );

    // Create a minimal program and expand it
    let mut expand_program = crate::ast::Program {
        items: vec![crate::ast::TopLevel::Statement(Box::new(call_stmt))],
        imports: Vec::new(),
        link_deps: Vec::new(),
        spans: Vec::new(),
        module_path: String::new(),
        target: None,
        exported_items: Vec::new(),
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
