// ── Compilation Pipeline ──────────────────────────────────────────────
// 2026-07-12: Phase 7 — Compile a Brief source file end-to-end.
// Pipeline: lex -> parse -> typecheck -> codegen -> output.
// 2026-07-14: Wire real LlvmBackend instead of stub codegen.
//             Add binary compilation via clang. Add --out / --optimize-budget flags.
// 2026-07-14: Plugin path — serialize to BVIR, run external plugins, deserialize.
// 2026-07-15: Phase 2 — Wire per-stage plugin dispatch into pipeline.
//             Front: on_ast after parse, Mid: on_ast after typecheck,
//             Post/Back: on_ir after codegen. Per-extension plugin selection
//             from config/targets.toml. System plugin discovery from
//             plugins/{front,mid,post,back}/.

use std::path::{Path, PathBuf};
use std::process::Command;

use brief_compiler::backend::llvm::LlvmBackend;
use brief_compiler::lexer::Token;
use brief_compiler::plugin::loader::{discover_system_plugins, extract_inline_stage_blocks};
use brief_compiler::plugin::PluginManager;
use brief_compiler::target::{BackendKind, TargetConfig, get_extension};
use brief_compiler::type_universe::TypeUniverse;

/// Re-export the LLVM backend's TrgUnresolvedAction for CLI flag parsing.
/// 2026-07-15: Phase 7i — Defined in the backend to avoid circular deps.
pub use brief_compiler::backend::llvm::TrgUnresolvedAction;

/// Pipeline stage at which to emit a BVIR snapshot.
/// 2026-07-15: Phase 7 — Used by --emit-bvir for metaprogramming introspection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BvirStage {
    Ast,
    Mid,
    Post,
}

impl std::str::FromStr for BvirStage {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ast" => Ok(BvirStage::Ast),
            "mid" => Ok(BvirStage::Mid),
            "post" => Ok(BvirStage::Post),
            _ => Err(format!("unknown BVIR stage '{}'. Use: ast, mid, post", s)),
        }
    }
}

/// Options parsed from the `brief-compiler build` CLI flags.
pub struct BuildOptions {
    pub config_dir: Option<String>,
    pub file_path: String,
    pub emit_ir_only: bool,
    pub out_dir: Option<String>,
    pub optimize_budget: u64,
    pub gpu_offload: bool,
    /// BVIR snapshot stages to emit (--emit-bvir). Empty = no emission.
    pub emit_bvir_stages: Vec<BvirStage>,
    /// Selected backend (resolved from extension + --backend flag).
    pub backend: BackendKind,
    /// Disable automatic stdlib import (for bare-metal/no-OS targets).
    pub no_stdlib: bool,
    /// Override stdlib search path.
    pub stdlib_path: Option<String>,
    /// Plugin names to disable (CLI --disable-plugin).
    pub disable_plugins: Vec<String>,
    /// Plugin names to enable exclusively (CLI --enable-plugin).
    pub enable_plugins: Vec<String>,
    /// Action on unresolved dynamic trigger target (--error-unresolved-trg).
    pub trg_unresolved_action: TrgUnresolvedAction,
    /// 2026-07-16: P4 — Pre-compiled .o / .so / .a objects linked into the binary.
    pub extra_objects: Vec<PathBuf>,
    /// 2026-07-18: Build a shared library (.so) instead of an executable.
    pub shared: bool,
    /// 2026-07-18: Phase B — Enable SSO (Short String Optimization) for String types.
    /// When ON, String is a 2-field \`{ i64, i64 }\` struct with inline storage for ≤6
    /// bytes, heap for longer. When OFF (default), String is passed as \`i8*\` (legacy).
    pub feature_sso_strings: bool,
    /// 2026-07-18: SVO (Small Vector Optimization) — inline storage for
    /// small List<T> elements (≤ N where N is from svo <~ N metadata).
    pub feature_svo: bool,
    /// 2026-07-18: Maximum size in bytes for stack allocation (alloca).
    /// Allocations exceeding this threshold fall back to heap (malloc).
    /// Used by the runtime fallback check in emit_dynamic_alloc.
    /// Default 4096 (4KB) — safe for most stack frames.
    pub stack_threshold: u64,
}

/// Compile a Brief source file: produce an executable binary (or `.ll` with `--llvm`).
pub fn compile_source(file_path: &str, source: &str, opts: &BuildOptions) -> Result<(), String> {
    // ── Front stage: source transformation ────────────────────────────
    let mut source = source.to_string();
    let mut pm = build_plugin_manager(file_path, opts);
    pm.run_front_source(&mut source)?;

    // ── Parse ─────────────────────────────────────────────────────────
    let tokens = lex(&source)?;
    let mut items = parse(file_path, &tokens, &source)?;

    // Extract inline $(Stage) blocks from the AST — they are plugins,
    // not runtime code.
    extract_inline_stage_blocks(&mut items, &mut pm);

    // ── Front stage: AST transformation (before import resolution) ────
    {
        let mut front_universe = TypeUniverse::new();
        pm.run_front_ast(&mut items, &mut front_universe)?;
    }

    // BVIR snapshot at Ast stage (after parse + front, for metaprogramming)
    {
        let snapshot_universe = TypeUniverse::new();
        emit_bvir_snapshot(file_path, BvirStage::Ast, &items, &snapshot_universe, opts)?;
    }

    // ── Import resolution ─────────────────────────────────────────────
    let mut resolver = brief_compiler::import_resolver::ImportResolver::new();
    if let Some(ref stdlib_path) = opts.stdlib_path {
        resolver = resolver.with_stdlib_path(Some(std::path::PathBuf::from(stdlib_path)));
    }
    items = resolver.resolve_imports(items, &std::path::PathBuf::from(file_path))?;

    // ── Type check ────────────────────────────────────────────────────
    let mut universe = TypeUniverse::new();
    check_types(&items, &universe)?;

    // ── Mid stage: AST transformation (after type check) ──────────────
    pm.run_mid_ast(&mut items, &mut universe)?;

    // BVIR snapshot at Mid stage (for metaprogrammer inspection)
    emit_bvir_snapshot(file_path, BvirStage::Mid, &items, &universe, opts)?;

    // ── Normalizer pass ───────────────────────────────────────────────
    match opts.backend {
        BackendKind::Llvm | BackendKind::Gpu => {
            brief_compiler::backend::llvm::normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Circt => {
            brief_compiler::backend::circt_normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Webstack => {
            brief_compiler::backend::webstack_normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Spirv => {
            // 2026-07-15: SPIR-V normalizer resolves types, flags kernels
            brief_compiler::backend::spirv::normalizer::normalize(&mut items, &mut universe)?;
        }
    }

    // BVIR snapshot at Post stage (after normalizer, before codegen)
    emit_bvir_snapshot(file_path, BvirStage::Post, &items, &universe, opts)?;

    // 2026-07-18: Run allocation strategy analysis before codegen.
    // Assigns strategies to Alloc# call sites — the codegen reads the
    // analysis output to select Arena/Alloca/Malloc instead of guessing.
    let alloc_strategies = brief_compiler::analysis::allocation::analyze_alloc_strategies(&mut items);

    // 2026-07-18: Dangling pointer detection — check each txn for local
    // pointer stored in state field (e.g. `&state_ptr = &local_var`).
    use brief_compiler::analysis::provenance::{check_dangling_ptrs, collect_local_names};
    for item in &items {
        if let brief_compiler::ast::TopLevel::Transaction(txn) = item {
            let local_names = collect_local_names(&txn.body, &txn.parameters);
            let warnings = check_dangling_ptrs(&txn.body, &local_names);
            for w in &warnings {
                eprintln!("{}", w);
            }
        }
    }

    // 2026-07-16: P4 — Collect extra objects from ForeignBinding FromSpec paths
    // for linking into the final binary.
    let extra_objects = collect_extra_objects(&items, &resolver)?;

    // ── Code generation ───────────────────────────────────────────────
    let (codegen_output, ext) = codegen(&items, &mut universe, &pm, opts, alloc_strategies)?;

    // ── Write output ──────────────────────────────────────────────────
    let out_path = determine_out_path(file_path, opts.out_dir.as_deref())?;
    let out_path = out_path.replace(".ll", ext);

    // 2026-07-15: SPIR-V writes inside codegen (binary format), skip outer write
    if opts.backend != BackendKind::Spirv {
        std::fs::write(&out_path, &codegen_output)
            .map_err(|e| format!("cannot write '{}': {}", out_path, e))?;
        println!("wrote {}", out_path);
    }

    if !opts.emit_ir_only {
        let binary_base = out_path.strip_suffix(ext).unwrap_or(&out_path);
        let binary_path = if opts.shared {
            format!("{}.so", binary_base)
        } else {
            binary_base.to_string()
        };
        if opts.backend == BackendKind::Llvm || opts.backend == BackendKind::Gpu {
            // Merge CLI-provided extra_objects with ones collected from frgn declarations
            let mut all_objects = opts.extra_objects.clone();
            all_objects.extend(extra_objects);
            compile_ll_to_binary(&out_path, &binary_path, &all_objects, opts.shared)?;
        }
    }

    Ok(())
}

/// Type-check only: don't generate code.
pub fn check_source(file_path: &str, source: &str) -> Result<(), String> {
    let default_opts = BuildOptions {
        config_dir: None,
        file_path: file_path.to_string(),
        emit_ir_only: false,
        out_dir: None,
        optimize_budget: 0,
        gpu_offload: false,
        emit_bvir_stages: vec![],
        backend: BackendKind::Llvm,
        no_stdlib: false,
        stdlib_path: None,
        disable_plugins: vec![],
        enable_plugins: vec![],
        trg_unresolved_action: TrgUnresolvedAction::Warn,
        extra_objects: vec![],
        shared: false,
        feature_sso_strings: false,
        feature_svo: false,
        stack_threshold: 4096,
    };
    let (_items, _universe) = parse_and_check(file_path, source, &default_opts)?;
    println!("OK");
    Ok(())
}

/// Build the plugin manager for a given file and opts.
/// 2026-07-15: Phase 2 — Discovers system plugins, applies per-extension
/// filtering, and applies CLI overrides. The caller then runs stages at
/// the appropriate pipeline points.
fn build_plugin_manager(file_path: &str, opts: &BuildOptions) -> PluginManager {
    let mut pm = PluginManager::new();

    // Discover system plugins from plugins/{front,mid,post,back}/
    discover_system_plugins(&mut pm);

    // Register built-in Rust plugins
    pm.register(Box::new(brief_compiler::plugin::env_plugin::EnvPlugin));
    pm.register(Box::new(brief_compiler::plugin::print_plugin::PrintPlugin));

    // Apply per-extension filtering from config/targets.toml
    let ext = get_extension(file_path);
    let config = load_target_config(opts);
    pm.filter_for_extension(&ext, &config);

    // Apply CLI overrides
    if !opts.enable_plugins.is_empty() {
        pm = pm.with_enabled_only(opts.enable_plugins.clone());
    }
    if !opts.disable_plugins.is_empty() {
        pm = pm.with_disabled(opts.disable_plugins.clone());
    }
    // --no-std is equivalent to --disable-plugin prelude
    if opts.no_stdlib {
        pm = pm.with_disabled(vec!["prelude".to_string()]);
    }

    pm
}

/// Code generation: dispatch to the selected backend, run Post/Back
/// plugin IR stages, and return (output_text, extension).
/// 2026-07-15: Phase 2 — Extracted from compile_source for flat flow.
fn codegen(
    items: &[brief_compiler::ast::TopLevel],
    universe: &mut TypeUniverse,
    pm: &PluginManager,
    opts: &BuildOptions,
    alloc_strategies: std::collections::HashMap<usize, brief_compiler::backend::llvm::AllocStrategy>,
) -> Result<(String, &'static str), String> {
    // 2026-07-20: Extract operator definitions from AST for backend dispatch.
    let mut operator_defs: std::collections::HashMap<String, Vec<brief_compiler::ast::top::OperatorDef>> = std::collections::HashMap::new();
    for item in items.iter() {
        if let brief_compiler::ast::TopLevel::TypeDef(td) = item {
            if !td.body.operators.is_empty() {
                operator_defs.insert(td.name.clone(), td.body.operators.clone());
            }
        }
    }

    let mut output;
    let ext: &str = match opts.backend {
        BackendKind::Llvm => {
            let mut b = LlvmBackend::new()
                .with_alloc_strategies(alloc_strategies)
                .with_sso_strings(opts.feature_sso_strings)
                .with_svo(opts.feature_svo)
                .with_shared_lib(opts.shared)
                .with_stack_threshold(opts.stack_threshold)
                .with_optimize_budget(opts.optimize_budget)
                .with_type_universe(universe.clone())
                .with_operator_defs(operator_defs)
                .with_trg_unresolved_action(opts.trg_unresolved_action);
            if opts.gpu_offload {
                b = b.with_gpu_offload(true);
                b = b.with_svo(opts.feature_svo);
            }
            // Apply target config if available
            let ext = get_extension(&opts.file_path);
            let target_config = load_target_config(opts);
            if let Some(entry) = target_config.lookup(&ext) {
                if let Some(ref triple) = entry.target_triple {
                    b = b.with_target_triple(triple);
                }
                if let Some(ref dl) = entry.data_layout {
                    b = b.with_data_layout(dl);
                }
            }
            output = b.generate(items, None);
            ".ll"
        }
        BackendKind::Circt => {
            let mut b = brief_compiler::backend::circt::CirctBackend::new();
            output = b.generate(items);
            ".mlir"
        }
        BackendKind::Webstack => {
            let result = brief_compiler::backend::webstack::WebstackGenerator::new()
                .generate(items, &[], "program");
            output = result.ts_code;
            ".ts"
        }
        BackendKind::Gpu => {
            let mut b = LlvmBackend::new()
                .with_alloc_strategies(alloc_strategies)
                .with_sso_strings(opts.feature_sso_strings)
                .with_svo(opts.feature_svo)
                .with_shared_lib(opts.shared)
                .with_stack_threshold(opts.stack_threshold)
                .with_optimize_budget(opts.optimize_budget)
                .with_type_universe(universe.clone())
                .with_trg_unresolved_action(opts.trg_unresolved_action);
            if opts.gpu_offload {
                b = b.with_gpu_offload(true);
            }
            // Apply target config (same logic as Llvm)
            let ext = get_extension(&opts.file_path);
            let target_config = load_target_config(opts);
            if let Some(entry) = target_config.lookup(&ext) {
                if let Some(ref triple) = entry.target_triple {
                    b = b.with_target_triple(triple);
                }
                if let Some(ref dl) = entry.data_layout {
                    b = b.with_data_layout(dl);
                }
            }
            output = b.generate(items, None);
            ".ll"
        }
        BackendKind::Spirv => {
            // 2026-07-15: SPIR-V backend compiles kernels to binary
            let binary = brief_compiler::backend::spirv::compile_spirv(items, "main")?;
            let out = determine_out_path(&opts.file_path, opts.out_dir.as_deref())?;
            let out_path = out.replace(".ll", ".spv");
            std::fs::write(&out_path, &binary)
                .map_err(|e| format!("cannot write '{}': {}", out_path, e))?;
            println!("wrote {}", out_path);
            output = String::new();
            ".spv"
        }
    };

    // Post stage: validate or modify IR after codegen
    pm.run_post_ir(&mut output)?;

    // Back stage: final validation before writing
    pm.run_back_ir(&mut output)?;

    Ok((output, ext))
}

/// Write a BVIR snapshot at the given pipeline stage, if --emit-bvir includes it.
/// 2026-07-15: Phase 7 — Metaprogrammer introspection tool.
fn emit_bvir_snapshot(
    file_path: &str,
    stage: BvirStage,
    items: &[brief_compiler::ast::TopLevel],
    universe: &TypeUniverse,
    opts: &BuildOptions,
) -> Result<(), String> {
    if !opts.emit_bvir_stages.contains(&stage) {
        return Ok(());
    }
    let stage_name = match stage {
        BvirStage::Ast => "ast",
        BvirStage::Mid => "mid",
        BvirStage::Post => "post",
    };
    let bvir = brief_compiler::bvir::to_bvir(items, universe);
    let path = format!(
        "{}.bvir.{}",
        file_path.strip_suffix(".bv").unwrap_or(file_path),
        stage_name
    );
    std::fs::write(&path, &bvir)
        .map_err(|e| format!("cannot write '{}': {}", path, e))?;
    eprintln!("wrote BVIR snapshot: {}", path);
    Ok(())
}

/// Determine the output `.ll` file path from the input path and optional output directory.
fn determine_out_path(file_path: &str, out_dir: Option<&str>) -> Result<String, String> {
    let p = Path::new(file_path);
    let base = p.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("cannot determine output path from '{}'", file_path))?;

    let parent = match out_dir {
        Some(dir) => dir.trim_end_matches('/').to_string(),
        None => p.parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string()),
    };

    Ok(format!("{}/{}.ll", parent, base))
}

/// 2026-07-16: P4 — Collect extra object files from ForeignBinding FromSpec paths.
/// Each frgn declaration with a .c/.so/.a/etc. path triggers compilation or direct
/// inclusion. The resolver is used to resolve compiler-relative <name> paths.
fn collect_extra_objects(items: &[brief_compiler::ast::TopLevel], resolver: &brief_compiler::import_resolver::ImportResolver) -> Result<Vec<PathBuf>, String> {
    let cache_dir = get_ffi_cache_dir();
    let mut objects = Vec::new();
    for item in items {
        let fb = match item {
            brief_compiler::ast::TopLevel::ForeignBinding(fb) => fb,
            _ => continue,
        };
        let ext = fb.from.extension();
        let resolved_path = || -> PathBuf {
            resolver.resolve_stdlib_relative_path(&fb.from.as_str())
                .unwrap_or_else(|| PathBuf::from(fb.from.as_str()))
        };
        match ext.as_deref() {
            Some("c") | Some("cpp") | Some("cc") | Some("cxx") | Some("m") => {
                let src = resolved_path();
                let obj = compile_source_to_object(&src, &cache_dir)?;
                objects.push(obj);
            }
            Some("so") | Some("dylib") | Some("a") | Some("o") => {
                objects.push(resolved_path());
            }
            _ => {}
        }
    }
    Ok(objects)
}

/// 2026-07-16: P4 — Get or create the FFI object cache directory.
fn get_ffi_cache_dir() -> PathBuf {
    let base = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("brief-compiler")
        .join("ffi");
    std::fs::create_dir_all(&base).ok();
    base
}

/// 2026-07-16: P4 — Compile a C/C++ source to a .o object file.
/// Content-hash cached at ~/.cache/brief-compiler/ffi/<hash>.o.
fn compile_source_to_object(source_path: &Path, cache_dir: &Path) -> Result<PathBuf, String> {
    let content = std::fs::read(source_path)
        .map_err(|e| format!("cannot read '{}': {}", source_path.display(), e))?;
    let hash = blake3::hash(&content);
    let cache_path = cache_dir.join(format!("{}.o", hash.to_hex()));
    if cache_path.exists() {
        return Ok(cache_path);
    }
    let ext = source_path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let lang_flag = match ext {
        "c" | "m" => "c",
        "cpp" | "cc" | "cxx" => "c++",
        _ => return Err(format!("unknown source extension '{}' for '{}'", ext, source_path.display())),
    };
    let status = Command::new("clang")
        .args([
            "-O3", "-march=native", "-ffast-math",
            "-x", lang_flag,
            "-c",
            source_path.to_str().unwrap(),
            "-o", cache_path.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("failed to invoke clang (is it installed?): {}", e))?;
    if !status.success() {
        return Err(format!("clang failed to compile '{}'", source_path.display()));
    }
    Ok(cache_path)
}

/// Compile a `.ll` file to a binary using clang.
fn compile_ll_to_binary(ll_path: &str, binary_path: &str, extra_objects: &[PathBuf], shared: bool) -> Result<(), String> {
    let mut cmd = Command::new("clang");
    let rt_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("lib/runtime/brief_rt.c");
    let rt_str = rt_path.to_string_lossy().to_string();
    if shared {
        cmd.args(["-O3", "-shared", "-fPIC", ll_path, &rt_str]);
    } else {
        cmd.args(["-O3", "-march=native", "-ffast-math", ll_path, &rt_str]);
    }
    for obj in extra_objects {
        cmd.arg(obj.as_os_str());
    }
    cmd.args(["-o", binary_path, "-lm"]);
    let status = cmd.status()
        .map_err(|e| format!(
            "failed to invoke clang: {} (is clang installed? use --llvm to emit IR only)",
            e
        ))?;

    if !status.success() {
        return Err(format!(
            "clang failed to compile '{}' to binary '{}'",
            ll_path, binary_path,
        ));
    }

    println!("wrote {}", binary_path);
    Ok(())
}

/// Lex + parse + resolve imports + typecheck, returning items and universe.
fn parse_and_check(file_path: &str, source: &str, opts: &BuildOptions) -> Result<(Vec<brief_compiler::ast::TopLevel>, TypeUniverse), String> {
    let tokens = lex(source)?;
    let items = parse(file_path, &tokens, source)?;

    let mut resolver = brief_compiler::import_resolver::ImportResolver::new();
    if let Some(ref stdlib_path) = opts.stdlib_path {
        resolver = resolver.with_stdlib_path(Some(std::path::PathBuf::from(stdlib_path)));
    }
    let items = resolver.resolve_imports(items, &std::path::PathBuf::from(file_path))?;

    let universe = TypeUniverse::new();
    check_types(&items, &universe)?;
    Ok((items, universe))
}

/// Load TargetConfig, respecting --config-dir when set in opts.
/// 2026-07-16: P1 — Runtime config directory overrides compile-time baked.
fn load_target_config(opts: &BuildOptions) -> TargetConfig {
    match &opts.config_dir {
        Some(dir) => {
            let path = Path::new(dir).join("targets.toml");
            TargetConfig::load_from(&path).unwrap_or_else(|e| {
                eprintln!("warning: cannot load '{}': {} — using baked fallback", path.display(), e);
                TargetConfig::load()
            })
        }
        None => TargetConfig::load(),
    }
}

/// Lex the source into tokens with source spans.
/// 2026-07-16: Fixed — use actual spans from logos instead of 0..0.
/// The spans are needed by read_layout_body for byte-level slicing.
fn lex(source: &str) -> Result<Vec<(Token, std::ops::Range<usize>)>, String> {
    use logos::Logos;
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    while let Some(result) = lexer.next() {
        let token = result.map_err(|_| "lex error".to_string())?;
        let span = lexer.span();
        tokens.push((token, span));
    }
    Ok(tokens)
}

/// Parse tokens into an AST.
fn parse(file_path: &str, tokens: &[(Token, std::ops::Range<usize>)], source: &str) -> Result<Vec<brief_compiler::ast::TopLevel>, String> {
    let mut parser = brief_compiler::parser::Parser::new(tokens.to_vec(), source);
    parser.parse_program().map_err(|e| format!("{}: parse error: {}", file_path, e))
}

/// Type-check the program against a TypeUniverse.
fn check_types(items: &[brief_compiler::ast::TopLevel], universe: &TypeUniverse) -> Result<(), String> {
    brief_compiler::typechecker::check_program(items, universe)
        .map_err(|errors| {
            let msgs: Vec<String> = errors.iter().map(|e| format!("{}", e)).collect();
            format!("type errors:\n  {}", msgs.join("\n  "))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a temporary file with given content, run a function on its path.
    fn with_temp_file<F>(content: &str, f: F)
    where F: FnOnce(&Path)
    {
        let dir = std::env::temp_dir().join("brief_compile_test");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join(format!("test_{}.c", std::process::id()));
        std::fs::write(&path, content).ok();
        f(&path);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_compile_source_to_object_cached() {
        let content = "int foo() { return 42; }";
        with_temp_file(content, |path| {
            let cache_dir = get_ffi_cache_dir();
            // First compilation
            let result1 = compile_source_to_object(path, &cache_dir);
            assert!(result1.is_ok(), "first compile failed: {:?}", result1);
            let obj1 = result1.unwrap();
            assert!(obj1.exists(), "object file not created");
            // Same source → same hash → returns cached path (identical)
            let result2 = compile_source_to_object(path, &cache_dir);
            assert!(result2.is_ok(), "second compile failed: {:?}", result2);
            let obj2 = result2.unwrap();
            assert_eq!(obj1, obj2, "cached path should match");
        });
    }

    #[test]
    fn test_get_ffi_cache_dir_creates_dir() {
        let dir = get_ffi_cache_dir();
        assert!(dir.exists(), "cache directory should be created");
    }

    #[test]
    fn test_compile_source_to_object_bad_ext() {
        let path = Path::new("/tmp/test_bad_ext.xyz");
        std::fs::write(path, "hello").ok();
        let result = compile_source_to_object(path, &get_ffi_cache_dir());
        assert!(result.is_err(), "expected compile error for unknown extension");
        let _ = std::fs::remove_file(path);
    }
}
