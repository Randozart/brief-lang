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
use crate::glue::config::GlueTarget;
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
    /// 2026-08-03: Whether the emitted C-ABI signature carries a leading
    /// `ptr %state` parameter (body-dependent ABI, export_abi analysis).
    /// Wrappers/bindings must pass/omit the state handle to match.
    pub needs_state: bool,
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
    // 2026-08-03: Body-dependent ABI — whether each export carries the
    // leading state param. Shared with the backend (src/analysis/export_abi.rs).
    let needs_state = crate::analysis::export_abi::compute_export_needs_state(items);
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
                        needs_state: needs_state.get(&defn.name).copied().unwrap_or(false),
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
                    needs_state: needs_state.get(&defn.name).copied().unwrap_or(false),
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
        // 2026-08-03: 5th field = needs_state (leading state param in the
        // emitted C ABI). Consumers use it to pass/omit the state handle.
        let ns = if e.needs_state { "state" } else { "pure" };
        lines.push(format!("export,{},{},{},{}", e.name, params.join("|"), e.return_type, ns));
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

/// Per-export template variables for wrapper/bindings rendering.
///
/// 2026-08-03: The state handle (s_param/s_ffi_param/s_ffi_type) is computed
/// from the body-dependent ABI (needs_state) and the target's config-driven
/// state representation. Boundary conversions come from target.conversions
/// (config), never hardcoded in Rust.
fn export_template_vars(export: &ExportDecl, target: &GlueTarget) -> HashMap<String, String> {
    let mut fn_vars: HashMap<String, String> = HashMap::new();
    fn_vars.insert("name".to_string(), export.name.clone());

    let params: Vec<String> = export.params.iter()
        .map(|(name, ty)| {
            let (native, _) = resolve_protocol(ty, &target.protocols);
            format!("{}: {}", name, native)
        })
        .collect();
    let ffi_params: Vec<String> = export.params.iter()
        .map(|(name, ty)| {
            let (_, c_abi) = resolve_protocol(ty, &target.protocols);
            // 2026-08-03: Config-driven param decl format (C uses `type name`,
            // python/rust use `name: type`).
            target.param_decl.replace("{name}", name).replace("{type}", &c_abi)
        })
        .collect();
    let args: Vec<String> = export.params.iter()
        .map(|(name, _)| name.clone())
        .collect();
    let c_types: Vec<String> = export.params.iter()
        .map(|(_, ty)| {
            let (_, c_abi) = resolve_protocol(ty, &target.protocols);
            c_abi
        })
        .collect();

    let args_abi = to_abi_args(export, target);
    let return_expr = from_abi_return(export, target);
    let (state_arg, state_ffi_param, state_ffi_type) =
        state_vars(export, target, !args_abi.is_empty(), !ffi_params.is_empty(), !c_types.is_empty());

    fn_vars.insert("s_param".to_string(), state_arg);
    fn_vars.insert("s_ffi_param".to_string(), state_ffi_param);
    fn_vars.insert("s_ffi_type".to_string(), state_ffi_type);
    let (native_ret, c_ret) = resolve_protocol(&export.return_type, &target.protocols);
    fn_vars.insert("return".to_string(), native_ret.clone());
    fn_vars.insert("c_return".to_string(), c_ret.clone());

    fn_vars.insert("params".to_string(), params.join(", "));
    fn_vars.insert("ffi_params".to_string(), ffi_params.join(", "));
    fn_vars.insert("c_types".to_string(), c_types.join(", "));
    fn_vars.insert("args".to_string(), args.join(", "));
    fn_vars.insert("args_abi".to_string(), args_abi.join(", "));
    fn_vars.insert("return_expr".to_string(), return_expr);
    fn_vars
}

/// `to_abi`: render each argument's boundary form from the config expression
/// (`{name}` placeholder); identity when the target has no conversion.
fn to_abi_args(export: &ExportDecl, target: &GlueTarget) -> Vec<String> {
    export.params.iter()
        .map(|(name, ty)| {
            let proto = format!("#{}", ty);
            target.conversions.to_abi.get(&proto)
                .map(|expr| expr.replace("{name}", name))
                .unwrap_or_else(|| name.clone())
        })
        .collect()
}

/// `from_abi`: render the raw boundary `result_abi` back to a native value.
fn from_abi_return(export: &ExportDecl, target: &GlueTarget) -> String {
    let proto = format!("#{}", export.return_type);
    target.conversions.from_abi.get(&proto)
        .cloned()
        .unwrap_or_else(|| "result_abi".to_string())
}

/// Per-export state-handle variables, joined WITHOUT a dangling separator
/// when there are no user params (C rejects `(BriefState* state, )`).
fn state_vars(
    export: &ExportDecl,
    target: &GlueTarget,
    has_args: bool,
    has_ffi_params: bool,
    has_c_types: bool,
) -> (String, String, String) {
    let needs = export.needs_state;
    (
        state_fragment(needs, has_args, &target.state.arg),
        state_fragment(needs, has_ffi_params, &target.state.decl),
        state_fragment(needs, has_c_types, &target.state.ffi_type),
    )
}

/// `needs` && more-fields → `"{s}, "`; `needs` only → `s`; else `""`.
fn state_fragment(needs: bool, has_more: bool, s: &str) -> String {
    if needs && has_more {
        format!("{}, ", s)
    } else if needs {
        s.to_string()
    } else {
        String::new()
    }
}

/// Render the per-export ffi declarations for a template set, joined by a
/// separator (newline). Shared by wrapper and bindings generation.
fn render_ffi_decls(
    exports: &[ExportDecl],
    target: &GlueTarget,
    template_vars: &HashMap<String, String>,
    ffi_template: &str,
) -> String {
    let mut ffi_buf = String::new();
    for (i, export) in exports.iter().enumerate() {
        if i > 0 { ffi_buf.push('\n'); }
        let fn_vars = export_template_vars(export, target);
        let mut merged = template_vars.clone();
        merged.extend(fn_vars);
        ffi_buf.push_str(&render_template(ffi_template, &merged));
    }
    ffi_buf
}

/// `brief bindings <bridge.bv> <language> [--out <dir>]` — render only the
/// language's `bindings.*` templates (e.g. brief_types.h, brief_bindings.rs)
/// for the exported functions, without the full wrapper crate.
///
/// 2026-08-03: Config-driven — the bindings templates and per-export ABI
/// (state param from needs_state) live in config/glue.dbvl. No compiler-side
/// language knowledge.
pub fn run_bindings_cli(file_path: &str, language: &str, out_dir: &str) -> Result<(), String> {
    let source = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Cannot read '{}': {}", file_path, e))?;
    let bridge_name = std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bridge");
    let (items, universe) = crate::library::parse_and_check(file_path, &source)?;

    let glue_targets = crate::glue::config::load_glue_config(None)?;
    let target = glue_targets.get(language).ok_or_else(|| {
        format!("Unknown bindings target '{}'.\n  Add an entry to config/glue.dbvl: {}", language,
            glue_targets.keys().cloned().collect::<Vec<_>>().join(", "))
    })?;
    let info = extract_bridge_info(&items, bridge_name);

    let mut template_vars: HashMap<String, String> = HashMap::new();
    template_vars.insert("bridge_name".to_string(), bridge_name.to_string());

    let bindings_ffi = target.templates.get("bindings.ffi_template")
        .or_else(|| target.templates.get("ffi_template"))
        .ok_or_else(|| format!("target '{}' has no bindings.ffi_template in config/glue.dbvl", language))?;
    let ffi_decls = render_ffi_decls(&info.exports, target, &template_vars, bindings_ffi);
    template_vars.insert("ffi_decls".to_string(), ffi_decls);

    let output_dir = std::path::Path::new(out_dir).join(format!("{}-bindings", bridge_name));
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output dir '{}': {}", output_dir.display(), e))?;

    let mut written = 0;
    for (filename, template) in &target.templates {
        if filename == "bindings.ffi_template" || filename == "ffi_template" || filename == "fn_template" {
            continue;
        }
        if !filename.starts_with("bindings.") {
            continue;
        }
        let rel = &filename["bindings.".len()..];
        let rendered = render_template(template, &template_vars);
        let output_path = output_dir.join(rel);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir '{}': {}", parent.display(), e))?;
        }
        std::fs::write(&output_path, &rendered)
            .map_err(|e| format!("Failed to write '{}': {}", output_path.display(), e))?;
        println!("  Written: {}", output_path.display());
        written += 1;
    }
    if written == 0 {
        return Err(format!("target '{}' has no bindings.* templates in config/glue.dbvl", language));
    }
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
        format!("Unknown export target '{}'.\n  Add an entry to config/glue.dbvl or use a supported language: {}",
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

    // 2026-07-22: Step 6 — compile to .o via llc.
    // 2026-08-03: -relocation-model=pic — the .o must link into a shared
    // library for c_abi hosts (python/node); without it the linker rejects
    // R_X86_64_32 relocations against .rodata.
    let o_path = output_dir.join("bridge.o");
    let llc_status = std::process::Command::new("llc")
        .arg("-filetype=obj")
        .arg("-relocation-model=pic")
        .arg("-o")
        .arg(&o_path)
        .arg(&ll_path)
        .status()
        .map_err(|e| format!("Failed to run llc: {}", e))?;
    if !llc_status.success() {
        return Err("llc failed".to_string());
    }
    println!("  Object: {}", o_path.display());

    // 2026-07-22: Step 7 — generate language wrappers from config templates.
    let mut template_vars: HashMap<String, String> = HashMap::new();
    template_vars.insert("bridge_name".to_string(), bridge_name.to_string());
    // 2026-08-03: The global s_init differs by convention; the per-export
    // s_param/s_ffi_param/s_argtypes are computed per export from its
    // needs_state (body-dependent ABI) — pure exports get no state handle.
    if target.calling_convention == "c_abi" {
        template_vars.insert("s_init".to_string(), String::new());
    } else {
        // LTO path (Rust): init_state is declared as an FFI function
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

        let fn_vars = export_template_vars(export, target);

        if let Some(ft) = fn_template {
            // 2026-08-03: per-function render sees both fn_vars and the
            // global template_vars (s_init, bridge_name, …). The earlier
            // render passed only fn_vars, silently dropping {{s_param}}.
            let mut merged = template_vars.clone();
            merged.extend(fn_vars.clone());
            let rendered = render_template(ft, &merged);
            exports_buf.push_str(&rendered);
        }
        if let Some(ffit) = ffi_template {
            let mut merged = template_vars.clone();
            merged.extend(fn_vars.clone());
            let rendered = render_template(ffit, &merged);
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
            needs_state: false,
        }];
        let result = serialize_exports_tagged(&exports);
        assert_eq!(result, "export,print_int,Int,Int,pure");
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
            needs_state: true,
        }];
        let result = serialize_exports_tagged(&exports);
        assert_eq!(result, "export,write_file,String|Data,Int,state");
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
    fn test_serialize_exports_tagged_multiple_separate() {
        let exports = vec![
            ExportDecl {
                name: "add".to_string(),
                params: vec![("a".to_string(), "Int".to_string()), ("b".to_string(), "Int".to_string())],
                return_type: "Int".to_string(),
                needs_state: false,
            },
            ExportDecl {
                name: "greet".to_string(),
                params: vec![("name".to_string(), "String".to_string())],
                return_type: "String".to_string(),
                needs_state: true,
            },
        ];
        let result = serialize_exports_tagged(&exports);
        assert_eq!(result, "export,add,Int|Int,Int,pure\nexport,greet,String,String,state");
    }
}
