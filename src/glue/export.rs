// GLUE Export Pipeline
//
// `brief export <bridge.bv> <language>` — reads a Brief bridge file,
// extracts #export, frgn, and meld declarations, then writes a
// bridge-exports.dbvl metadata file alongside the compiled LLVM IR module.
//
// Pipeline:
//   1. Parse the bridge .bv to collect #export/frgn/meld declarations.
//   2. Read glue.dbvl to find the language target entry (types_module,
//      llvm_triple, c_type_map).
//   3. Write bridge-exports.dbvl with tagged entries:
//      - export lines: function signatures crossing the boundary
//      - meld lines: type-layout compatibility proofs for the boundary
//      - ctype lines: Brief type → C ABI type mappings (from glue.dbvl)
//   4. (Future) Compile bridge to .ll via `brief build --library`.
//
// 2026-07-10: GLUE v2. Replaced the $!macro adapter pipeline (which
// generated C ABI wrapper crates) with direct bridge-exports.dbvl output
// consumed by foreign build systems. The foreign build.rs or script reads
// the .dbvl to generate bindings. Two interop paths exist:
//   Path A (LLVM LTO) — for LLVM targets (Rust, C, Swift, Zig). Brief's
//     .ll merges with the host's .ll before optimization.
//   Path B (Meld) — for managed runtimes (Python, Node). C ABI transport
//     + meld projections for zero-copy data access.

use crate::ast::{Annotation, OutputType, TopLevel};
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

/// Registry entry for a GLUE target language, parsed from glue.dbvl.
///
/// 2026-07-10: GLUE v2. Removed macro_path (adapter $!macro system is
/// gone). Added types_module (path to .bv with foreign type declarations)
/// and llvm_triple (target triple for LLVM compilation, "any" for non-LLVM).
/// Renamed type_map to c_type_map — now maps Brief types to C ABI types
/// (int64_t, double, etc.) instead of language-specific type names.
#[derive(Debug, Clone)]
pub struct AdapterEntry {
    /// Target language name (rust, python, etc.)
    pub language: String,
    /// Path to .bv file declaring foreign type names for Brief's type universe
    pub types_module: String,
    /// Native source file extension without dot (rs, py, js)
    pub file_extension: String,
    /// LLVM target triple ("any" for non-LLVM targets like Python/Node)
    pub llvm_triple: String,
    /// Brief type name → C ABI type name mapping (e.g., Int → int64_t)
    pub c_type_map: HashMap<String, String>,
}

/// Extract bridge information from a parsed program.
/// Walks the AST to find #export pragmas, frgn declarations, and meld routes.
pub fn extract_bridge_info(items: &[TopLevel], name: &str) -> BridgeInfo {
    BridgeInfo {
        name: name.to_string(),
        exports: extract_exports(items),
        frgns: extract_frgns(items),
        melds: extract_melds(items),
    }
}

fn has_export_modifier(modifiers: &[Annotation]) -> bool {
    modifiers.iter().any(|m| m.name == "export")
}

fn extract_exports(items: &[TopLevel]) -> Vec<ExportDecl> {
    let mut exports = Vec::new();
    for item in items {
        match item {
            // 2026-07-22: Form A — `export defn name(...) { ... }` keyword form
            TopLevel::Export(export) => {
                if let TopLevel::Definition(defn) = export.inner.as_ref() {
                    let params: Vec<(String, String)> = defn.parameters.iter()
                        .map(|p| (p.0.clone(), format_type(&p.1)))
                        .collect();
                    let return_type = defn.output_type.as_ref()
                        .map(|ot| format_output_type(ot))
                        .or_else(|| {
                            if !defn.outputs.is_empty() {
                                Some(defn.outputs.iter().map(|t| format_type(t)).collect::<Vec<_>>().join("|"))
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| "Void".to_string());
                    exports.push(ExportDecl {
                        name: defn.name.clone(),
                        params,
                        return_type,
                    });
                }
            }
            // 2026-07-22: Form B — `#export` modifier on defn
            TopLevel::Definition(defn) if has_export_modifier(&defn.modifiers) => {
                let params: Vec<(String, String)> = defn.parameters.iter()
                    .map(|p| (p.0.clone(), format_type(&p.1)))
                    .collect();
                let return_type = defn.output_type.as_ref()
                    .map(|ot| format_output_type(ot))
                    .or_else(|| {
                        if !defn.outputs.is_empty() {
                            Some(defn.outputs.iter().map(|t| format_type(t)).collect::<Vec<_>>().join("|"))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "Void".to_string());
                exports.push(ExportDecl {
                    name: defn.name.clone(),
                    params,
                    return_type,
                });
            }
            _ => {}
        }
    }
    // 2026-07-22: Deduplicate by name — if both forms declare the same function, keep one.
    exports.sort_by_key(|e| e.name.clone());
    exports.dedup_by_key(|e| e.name.clone());
    exports
}

fn extract_frgns(items: &[TopLevel]) -> Vec<FrgnDecl> {
    let mut frgns = Vec::new();
    for item in items {
        if let TopLevel::Signature(sig) = item {
            let return_type = format_result_type(&sig.outputs);
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

fn extract_melds(items: &[TopLevel]) -> Vec<MeldDecl> {
    let mut melds = Vec::new();
    for item in items {
        if let TopLevel::Meld(meld) = item {
            for (key, _val) in &meld.bindings {
                melds.push(MeldDecl {
                    from_type: meld.target.clone(),
                    to_type: meld.name.clone(),
                    route: key.clone(),
                });
            }
        }
    }
    melds
}

fn format_type(ty: &crate::ast::Type) -> String {
    match ty {
        crate::ast::Type::Custom(__t) if __t == "Int" => "Int".to_string(),
        crate::ast::Type::Custom(__t) if __t == "Float" => "Float".to_string(),
        crate::ast::Type::Custom(__t) if __t == "Bool" => "Bool".to_string(),
        crate::ast::Type::Custom(__t) if __t == "Char" => "Char".to_string(),
        crate::ast::Type::Custom(__t) if __t == "String" => "String".to_string(),
        crate::ast::Type::Custom(__t) if __t == "Data" => "Data".to_string(),
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
        OutputType::Array(ty) => format!("{}[]", format_output_type(ty)),
        OutputType::Named(name, inner) => format!("{}:{}", name, format_output_type(inner)),
    }
}

fn format_result_type(outputs: &[crate::ast::Type]) -> String {
    if outputs.is_empty() {
        "Void".to_string()
    } else {
        let parts: Vec<String> = outputs.iter().map(|t| format_type(t)).collect();
        parts.join("|")
    }
}

// =========================================================================
// DBVL Serialization — bridge-exports.dbvl output format
//
// Architecture: Brief export produces a .dbvl metadata file alongside the
// compiled .ll module. Each line is tagged with a discriminator field so
// the consumer (build.rs, Python script) can dispatch on entry type:
//
//   export:  export, name, param_types|pipe|separated, return_type
//   meld:    meld, from_type, to_type, route
//   ctype:   ctype, brief_type, c_type
//
// No quoting needed — none of our field values contain commas.
// The consumer splits by "\n" then by "," and switches on field[0].
//
// 2026-07-10: Tagged format replaces the old bare-entries format that was
// passed to $!macro adapters. Tagged lines allow a single file to carry
// multiple entry types without schema switching.
// =========================================================================

fn serialize_exports_tagged(exports: &[ExportDecl]) -> String {
    let mut lines = Vec::new();
    for e in exports {
        let params: Vec<String> = e.params.iter().map(|(_, t)| t.clone()).collect();
        lines.push(format!("export,{},{},{}", e.name, params.join("|"), e.return_type));
    }
    lines.join("\n")
}

fn serialize_melds_tagged(melds: &[MeldDecl]) -> String {
    let mut lines = Vec::new();
    for m in melds {
        lines.push(format!("meld,{},{},{}", m.from_type, m.to_type, m.route));
    }
    lines.join("\n")
}

fn serialize_ctypes_dbvl(c_type_map: &HashMap<String, String>) -> String {
    let mut lines: Vec<String> = c_type_map.iter()
        .map(|(brief_type, c_type)| format!("ctype,{},{}", brief_type, c_type))
        .collect();
    lines.sort();
    lines.join("\n")
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

/// Find a language target entry in glue.dbvl.
///
/// 2026-07-10: GLUE v2 field layout — language(0), types_module(1),
/// file_extension(2), llvm_triple(3), c_type_map(4). Returns an error
/// if fewer than 4 fields or the language doesn't match.
pub fn find_adapter(language: &str, dbvl_path: &Path) -> Result<AdapterEntry, String> {
    let source = fs::read_to_string(dbvl_path)
        .map_err(|e| format!("Failed to read {}: {}", dbvl_path.display(), e))?;

    let file = crate::glue::dbvl_reader::parse_dbvl(&source);
    for entry in &file.entries {
        let fields = match entry {
            crate::glue::dbvl_reader::DbvlEntry::Raw(tokens) => tokens,
            crate::glue::dbvl_reader::DbvlEntry::Validated { fields, .. } => fields,
        };
        if fields.len() < 4 {
            continue;
        }
        if fields[0] != language {
            continue;
        }
        let c_type_map = if fields.len() > 4 {
            parse_type_map(&fields[4])
        } else {
            HashMap::new()
        };
        return Ok(AdapterEntry {
            language: language.to_string(),
            types_module: fields[1].clone(),
            file_extension: fields[2].clone(),
            llvm_triple: fields[3].clone(),
            c_type_map,
        });
    }
    Err(format!("Target not found for language '{}' in {}", language, dbvl_path.display()))
}

/// Run the export pipeline:
/// 1. Extract bridge info from the program
/// 2. Find the language target entry in glue.dbvl
/// 3. Write bridge-exports.dbvl (tagged lines: export/meld/ctype)
///
/// 2026-07-10: GLUE v2. Replaced the $!macro adapter invocation with
/// direct bridge-exports.dbvl output. The foreign build system reads
/// this .dbvl to generate bindings — no adapter .bv macros needed.
pub fn run_export(
    program: &[TopLevel],
    bridge_name: &str,
    language: &str,
    out_dir: &Path,
    dbvl_path: &Path,
) -> Result<(), String> {
    // Step 1: Extract bridge info
    let info = extract_bridge_info(program, bridge_name);
    println!("  Bridge '{}': {} exports, {} frgns, {} melds",
        info.name, info.exports.len(), info.frgns.len(), info.melds.len());

    // Step 2: Find the language target entry
    let adapter = find_adapter(language, dbvl_path)?;
    println!("  Target: {} (types: {}, llvm_triple: {})",
        adapter.language, adapter.types_module, adapter.llvm_triple);

    // Step 3: Create output directory
    let output_dir = out_dir.join(format!("{}-bridge", bridge_name));
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    // Step 4: Write bridge-exports.dbvl with tagged entries
    // Each line has a discriminator field so the consumer can dispatch on
    // entry type: "export, ..." for functions, "meld, ..." for type
    // compatibility proofs, "ctype, ..." for C ABI type mappings.
    let mut dbvl_content = String::new();

    let exports_str = serialize_exports_tagged(&info.exports);
    if !exports_str.is_empty() {
        dbvl_content.push_str(&exports_str);
        dbvl_content.push('\n');
    }

    let melds_str = serialize_melds_tagged(&info.melds);
    if !melds_str.is_empty() {
        dbvl_content.push_str(&melds_str);
        dbvl_content.push('\n');
    }

    let ctypes_str = serialize_ctypes_dbvl(&adapter.c_type_map);
    if !ctypes_str.is_empty() {
        dbvl_content.push_str(&ctypes_str);
        dbvl_content.push('\n');
    }

    let dbvl_path = output_dir.join("bridge-exports.dbvl");
    fs::write(&dbvl_path, dbvl_content.trim_end())
        .map_err(|e| format!("Failed to write bridge-exports.dbvl: {}", e))?;

    println!("  Metadata: {}", dbvl_path.display());
    println!("  LLVM IR:  (future) {}/bridge.ll", output_dir.display());
    Ok(())
}

/// CLI entry point for `brief export <bridge.bv> <language> [--out <dir>]`.
///
/// 2026-07-22: Reads a Brief bridge file, extracts exports and foreign
/// declarations, generates LLVM IR with C-compatible wrappers, and writes
/// bridge metadata alongside the compiled output.
///
/// Steps:
///   1. Read and parse the .bv file
///   2. Extract bridge info (exports, frgns, melds)
///   3. Find the language target in the GLUE registry
///   4. Generate LLVM IR with C wrappers for exported functions
///   5. Compile to .o via llc
///   6. Generate native wrapper source files (Python, Rust, etc.)
///   7. Write bridge-exports.dbvl metadata
pub fn run_export_cli(file_path: &str, language: &str, out_dir: &str) -> Result<(), String> {
    // 2026-07-22: Step 1 — read and parse the bridge .bv file
    let source = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Cannot read '{}': {}", file_path, e))?;
    let bridge_name = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bridge");
    let (items, universe) = crate::library::parse_and_check(file_path, &source)?;

    // 2026-07-22: Step 2 — find language target and extract bridge info
    let glue_targets = crate::glue::config::load_glue_config(None)?;
    let target = glue_targets.get(language).ok_or_else(|| {
        format!("Unknown export target '{}'.\n  Add an entry to lib/glue.toml or use a supported language: {}",
            language, glue_targets.keys().cloned().collect::<Vec<_>>().join(", "))
    })?;
    let info = extract_bridge_info(&items, bridge_name);
    println!("  Bridge '{}': {} exports, {} frgns, {} melds",
        info.name, info.exports.len(), info.frgns.len(), info.melds.len());
    println!("  Target: {} (types: {}, bridge: {})",
        target.language, target.types_module.display(), target.bridge_kind);

    // 2026-07-22: Step 3 — collect resolved frgns for dispatch
    let mut resolved_frgns = std::collections::HashMap::new();
    for item in &items {
        if let crate::ast::TopLevel::ForeignBinding(fb) = item {
            let ext = fb.from.extension().unwrap_or_default();
            if let Ok(dispatch) = crate::analysis::frgn_dispatch::resolve_single_frgn(
                fb, &ext, &glue_targets, crate::target::BackendKind::Llvm, Some(&universe),
            ) {
                resolved_frgns.insert(fb.effective_brief_name().to_string(), dispatch);
            }
        }
    }

    // 2026-07-22: Step 3 — generate LLVM IR with the full backend (real function bodies)
    let llvm_ir = {
        use crate::backend::llvm::LlvmBackend;
        let mut b = LlvmBackend::new()
            .with_type_universe(universe)
            .with_resolved_frgns(resolved_frgns);
        b.generate(&items, None)
    };

    // 2026-07-22: Step 5 — create output directory and write files
    let output_dir = std::path::Path::new(out_dir).join(format!("{}-bridge", bridge_name));
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output dir '{}': {}", output_dir.display(), e))?;

    // Write LLVM IR
    let ll_path = output_dir.join("bridge.ll");
    std::fs::write(&ll_path, &llvm_ir)
        .map_err(|e| format!("Failed to write '{}': {}", ll_path.display(), e))?;
    println!("  LLVM IR: {}", ll_path.display());

    // 2026-07-22: Step 6 — compile to .o via llc
    let o_path = output_dir.join("bridge.o");
    let llc_status = std::process::Command::new("llc")
        .arg("-filetype=obj")
        .arg("-o")
        .arg(&o_path)
        .arg(&ll_path)
        .status()
        .map_err(|e| format!("Failed to run llc: {}", e))?;
    if !llc_status.success() {
        return Err("llc failed".to_string());
    }
    println!("  Object: {}", o_path.display());

    // 2026-07-22: Step 7 — generate language wrappers from TOML templates.
    let mut template_vars: HashMap<String, String> = HashMap::new();
    template_vars.insert("bridge_name".to_string(), bridge_name.to_string());
    // Populate state parameter based on calling convention
    if target.calling_convention == "c_abi" {
        template_vars.insert("s_param".to_string(), "_STATE, ".to_string());
        template_vars.insert("s_init".to_string(), String::new());
    } else {
        // LTO path (Rust): init_state is declared as an FFI function
        template_vars.insert("s_param".to_string(), String::new());
        template_vars.insert("s_init".to_string(), "    pub fn init_state(state: *mut c_void);\n".to_string());
    }

    // Render all exports + FFI declarations
    let fn_template = target.templates.get("fn_template");
    let ffi_template = target.templates.get("ffi_template");
    let mut exports_buf = String::new();
    let mut ffi_buf = String::new();

    for (i, export) in info.exports.iter().enumerate() {
        if i > 0 { exports_buf.push('\n'); }
        if i > 0 { ffi_buf.push('\n'); }

        // Build per-function variables
        let mut fn_vars: HashMap<String, String> = HashMap::new();
        fn_vars.insert("name".to_string(), export.name.clone());
        let (native_ret, c_ret) = resolve_protocol(&export.return_type, &target.protocols);
        fn_vars.insert("return".to_string(), native_ret.clone());
        fn_vars.insert("c_return".to_string(), c_ret.clone());

        // Build parameter lists
        let params: Vec<String> = export.params.iter()
            .map(|(name, ty)| {
                let (native, _) = resolve_protocol(ty, &target.protocols);
                format!("{}: {}", name, native)
            })
            .collect();
        let ffi_params: Vec<String> = export.params.iter()
            .map(|(name, ty)| {
                let (_, c_abi) = resolve_protocol(ty, &target.protocols);
                format!("{}: {}", name, c_abi)
            })
            .collect();
        let args: Vec<String> = export.params.iter()
            .map(|(name, _)| name.clone())
            .collect();

        // Build ABI conversion expressions from target.protocols.
        // to_abi: {{name}} → {{name}}_abi
        // from_abi: return value → safe type
        let args_abi: Vec<String> = export.params.iter()
            .map(|(name, ty)| {
                let (native, c_abi) = resolve_protocol(ty, &target.protocols);
                if native == c_abi {
                    format!("{}", name)
                } else if c_abi == "i64" && native.contains('*') {
                    format!("{} as i64", name)
                } else if native == "i64" && c_abi.contains('*') {
                    format!("{} as *mut u8", name)
                } else {
                    format!("{} as {}", name, c_abi)
                }
            })
            .collect();
        let (_, ret_c_abi) = resolve_protocol(&export.return_type, &target.protocols);
        let (ret_native, _) = resolve_protocol(&export.return_type, &target.protocols);
        let return_expr = if ret_native == ret_c_abi {
            "result_abi".to_string()
        } else if ret_c_abi == "i64" && ret_native.contains('*') {
            format!("result_abi as {}", ret_native)
        } else if ret_native == "i64" && ret_c_abi.contains('*') {
            format!("result_abi as {}", ret_c_abi)
        } else {
            format!("result_abi as {}", ret_native)
        };

        fn_vars.insert("params".to_string(), params.join(", "));
        fn_vars.insert("ffi_params".to_string(), ffi_params.join(", "));
        fn_vars.insert("c_types".to_string(), export.params.iter()
            .map(|(_, ty)| {
                let (_, c_abi) = resolve_protocol(ty, &target.protocols);
                c_abi
            })
            .collect::<Vec<_>>().join(", "));
        fn_vars.insert("args".to_string(), args.join(", "));
        fn_vars.insert("args_abi".to_string(), args_abi.join(", "));
        fn_vars.insert("return_expr".to_string(), return_expr);

        if let Some(ft) = fn_template {
            let rendered = render_template(ft, &fn_vars);
            exports_buf.push_str(&rendered);
        }
        if let Some(ffit) = ffi_template {
            let rendered = render_template(ffit, &fn_vars);
            ffi_buf.push_str(&rendered);
        }
    }

    template_vars.insert("exports".to_string(), exports_buf);
    template_vars.insert("ffi_decls".to_string(), ffi_buf);

    // Write each file template
    for (filename, template) in &target.templates {
        if filename == "fn_template" || filename == "ffi_template" {
            continue;
        }
        let rendered = render_template(template, &template_vars);
        let output_path = output_dir.join(filename);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir '{}': {}", parent.display(), e))?;
        }
        std::fs::write(&output_path, &rendered)
            .map_err(|e| format!("Failed to write '{}': {}", output_path.display(), e))?;
        println!("  Written: {}", output_path.display());
    }

    // 2026-07-22: Step 8 — write bridge-exports.dbvl metadata
    let adapter = crate::glue::export::AdapterEntry {
        language: language.to_string(),
        types_module: target.types_module.to_string_lossy().to_string(),
        file_extension: target.extension.clone(),
        llvm_triple: "x86_64-unknown-linux-gnu".to_string(),
        // 2026-07-26: c_abi is optional — fall back to native type if absent
        c_type_map: target.protocols.iter()
            .map(|(k, v)| (k.clone(), v.c_abi.clone().unwrap_or_else(|| v.native.clone())))
            .collect(),
    };
    let mut dbvl_content = String::new();
    let exports_str = serialize_exports_tagged(&info.exports);
    if !exports_str.is_empty() {
        dbvl_content.push_str(&exports_str);
        dbvl_content.push('\n');
    }
    let melds_str = serialize_melds_tagged(&info.melds);
    if !melds_str.is_empty() {
        dbvl_content.push_str(&melds_str);
        dbvl_content.push('\n');
    }
    let ctypes_str = serialize_ctypes_dbvl(&adapter.c_type_map);
    if !ctypes_str.is_empty() {
        dbvl_content.push_str(&ctypes_str);
        dbvl_content.push('\n');
    }
    let dbvl_path = output_dir.join("bridge-exports.dbvl");
    std::fs::write(&dbvl_path, dbvl_content.trim_end())
        .map_err(|e| format!("Failed to write bridge-exports.dbvl: {}", e))?;
    println!("  Metadata: {}", dbvl_path.display());
    Ok(())
}

/// Look up a Brief type's protocol mapping in a target's protocols map.
/// Returns (native_type, c_abi_or_wasm_abi_type) or falls back to the input type name.
/// 2026-07-26: c_abi is optional — for wasm_import targets, use wasm_abi instead.
fn resolve_protocol(
    brief_type_name: &str,
    protocols: &HashMap<String, crate::glue::config::ProtocolEntry>,
) -> (String, String) {
    let protocol_key = format!("#{}", brief_type_name);
    if let Some(entry) = protocols.get(&protocol_key) {
        let abi = entry.c_abi.clone()
            .or_else(|| entry.wasm_abi.clone())
            .unwrap_or_else(|| entry.native.clone());
        (entry.native.clone(), abi)
    } else {
        (brief_type_name.to_string(), brief_type_name.to_string())
    }
}


/// Simple mustache-like template substitution.
///
/// Replaces `{{variable}}` with values from `vars`.
/// No blocks, no loops — just single variable substitution.
/// Per-function repetition is handled by the caller (see `run_export_cli`).
fn render_template(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        // Push everything before the `{{`
        result.push_str(&rest[..start]);
        let after_open = &rest[start + 2..];
        if let Some(end) = after_open.find("}}") {
            let var_name = &after_open[..end];
            let value = vars.get(var_name).map(|s| s.as_str()).unwrap_or("");
            result.push_str(value);
            rest = &after_open[end + 2..];
        } else {
            // Unclosed `{{` — push rest as-is
            result.push_str(&rest[start..]);
            break;
        }
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_exports_tagged_empty() {
        let result = serialize_exports_tagged(&[]);
        assert_eq!(result, "");
    }

    #[test]
    fn test_serialize_exports_tagged_single() {
        let exports = vec![ExportDecl {
            name: "print_int".to_string(),
            params: vec![("n".to_string(), "Int".to_string())],
            return_type: "Int".to_string(),
        }];
        let result = serialize_exports_tagged(&exports);
        assert_eq!(result, "export,print_int,Int,Int");
    }

    #[test]
    fn test_serialize_exports_tagged_multiple_params() {
        let exports = vec![ExportDecl {
            name: "write_file".to_string(),
            params: vec![
                ("path".to_string(), "String".to_string()),
                ("data".to_string(), "Data".to_string()),
            ],
            return_type: "Int".to_string(),
        }];
        let result = serialize_exports_tagged(&exports);
        assert_eq!(result, "export,write_file,String|Data,Int");
    }

    #[test]
    fn test_serialize_melds_tagged() {
        let melds = vec![MeldDecl {
            from_type: "CBuffer".to_string(),
            to_type: "RSBuffer".to_string(),
            route: "identity".to_string(),
        }];
        let result = serialize_melds_tagged(&melds);
        assert_eq!(result, "meld,CBuffer,RSBuffer,identity");
    }

    #[test]
    fn test_serialize_ctypes_dbvl() {
        let mut map = HashMap::new();
        map.insert("Int".to_string(), "int64_t".to_string());
        map.insert("Float".to_string(), "double".to_string());
        let result = serialize_ctypes_dbvl(&map);
        assert!(result.contains("ctype,Int,int64_t"));
        assert!(result.contains("ctype,Float,double"));
    }

    #[test]
    fn test_serialize_ctypes_dbvl_empty() {
        let map = HashMap::new();
        let result = serialize_ctypes_dbvl(&map);
        assert_eq!(result, "");
    }

    #[test]
    fn test_parse_type_map() {
        let map = parse_type_map("{Int:int64_t Float:double Bool:bool}");
        assert_eq!(map.get("Int"), Some(&"int64_t".to_string()));
        assert_eq!(map.get("Float"), Some(&"double".to_string()));
        assert_eq!(map.get("Bool"), Some(&"bool".to_string()));
    }

    #[test]
    fn test_parse_type_map_empty() {
        let map = parse_type_map("{}");
        assert!(map.is_empty());
    }

    #[test]
    fn test_parse_type_map_no_braces() {
        let map = parse_type_map("Int:int64_t Float:double");
        assert_eq!(map.get("Int"), Some(&"int64_t".to_string()));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_serialize_exports_tagged_multiple_separate() {
        let exports = vec![
            ExportDecl {
                name: "add".to_string(),
                params: vec![("a".to_string(), "Int".to_string()), ("b".to_string(), "Int".to_string())],
                return_type: "Int".to_string(),
            },
            ExportDecl {
                name: "greet".to_string(),
                params: vec![("name".to_string(), "String".to_string())],
                return_type: "String".to_string(),
            },
        ];
        let result = serialize_exports_tagged(&exports);
        assert_eq!(result, "export,add,Int|Int,Int\nexport,greet,String,String");
    }

    #[test]
    fn test_find_adapter_new_format() {
        let dir = std::env::temp_dir();
        let dbvl_path = dir.join("test_glue_export.dbvl");
        let dbvl_content = "schema lib/glue.dbvs;\nrust, glue/rust/types.bv, rs, x86_64-unknown-linux-gnu, {Int:int64_t Float:double}";
        fs::write(&dbvl_path, dbvl_content).unwrap();

        let result = find_adapter("rust", &dbvl_path);
        assert!(result.is_ok());
        let adapter = result.unwrap();
        assert_eq!(adapter.language, "rust");
        assert_eq!(adapter.types_module, "glue/rust/types.bv");
        assert_eq!(adapter.file_extension, "rs");
        assert_eq!(adapter.llvm_triple, "x86_64-unknown-linux-gnu");
        assert_eq!(adapter.c_type_map.get("Int"), Some(&"int64_t".to_string()));
        assert_eq!(adapter.c_type_map.get("Float"), Some(&"double".to_string()));

        let _ = fs::remove_file(&dbvl_path);
    }

    #[test]
    fn test_find_adapter_language_not_found() {
        let dir = std::env::temp_dir();
        let dbvl_path = dir.join("test_glue_missing.dbvl");
        let dbvl_content = "schema lib/glue.dbvs;\nrust, glue/rust/types.bv, rs, x86_64";
        fs::write(&dbvl_path, dbvl_content).unwrap();

        let result = find_adapter("python", &dbvl_path);
        assert!(result.is_err());

        let _ = fs::remove_file(&dbvl_path);
    }

    #[test]
    fn test_find_adapter_too_few_fields() {
        let dir = std::env::temp_dir();
        let dbvl_path = dir.join("test_glue_too_few.dbvl");
        let dbvl_content = "schema lib/glue.dbvs;\nrust, glue/rust/types.bv, rs";
        fs::write(&dbvl_path, dbvl_content).unwrap();

        let result = find_adapter("rust", &dbvl_path);
        assert!(result.is_err(), "should reject entries with < 4 fields");

        let _ = fs::remove_file(&dbvl_path);
    }
}
