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

use std::path::Path;
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
    }

    // BVIR snapshot at Post stage (after normalizer, before codegen)
    emit_bvir_snapshot(file_path, BvirStage::Post, &items, &universe, opts)?;

    // ── Code generation ───────────────────────────────────────────────
    let (codegen_output, ext) = codegen(&items, &mut universe, &pm, opts)?;

    // ── Write output ──────────────────────────────────────────────────
    let out_path = determine_out_path(file_path, opts.out_dir.as_deref())?;
    let out_path = out_path.replace(".ll", ext);

    std::fs::write(&out_path, &codegen_output)
        .map_err(|e| format!("cannot write '{}': {}", out_path, e))?;
    println!("wrote {}", out_path);

    if !opts.emit_ir_only {
        let binary_path = out_path.strip_suffix(ext).unwrap_or(&out_path);
        if opts.backend == BackendKind::Llvm || opts.backend == BackendKind::Gpu {
            compile_ll_to_binary(&out_path, binary_path)?;
        }
    }

    Ok(())
}

/// Type-check only: don't generate code.
pub fn check_source(file_path: &str, source: &str) -> Result<(), String> {
    let default_opts = BuildOptions {
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

    // Apply per-extension filtering from config/targets.toml
    let ext = get_extension(file_path);
    let config = TargetConfig::load();
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
) -> Result<(String, &'static str), String> {
    let mut output;
    let ext: &str = match opts.backend {
        BackendKind::Llvm => {
            let mut b = LlvmBackend::new()
                .with_optimize_budget(opts.optimize_budget)
                .with_type_universe(universe.clone())
                .with_trg_unresolved_action(opts.trg_unresolved_action);
            if opts.gpu_offload {
                b = b.with_gpu_offload(true);
            }
            // Apply target config if available
            let ext = get_extension(&opts.file_path);
            let target_config = TargetConfig::load();
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
                .with_optimize_budget(opts.optimize_budget)
                .with_type_universe(universe.clone())
                .with_gpu_offload(true);
            // Apply target config (same logic as Llvm)
            let ext = get_extension(&opts.file_path);
            let target_config = TargetConfig::load();
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

/// Compile a `.ll` file to a binary using clang.
fn compile_ll_to_binary(ll_path: &str, binary_path: &str) -> Result<(), String> {
    let status = Command::new("clang")
        .args([
            "-O3",
            "-march=native",
            "-ffast-math",
            ll_path,
            "-o",
            binary_path,
            "-lm",
        ])
        .status()
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

/// Lex the source into tokens.
fn lex(source: &str) -> Result<Vec<(Token, std::ops::Range<usize>)>, String> {
    use logos::Logos;
    let lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    for result in lexer {
        let token = result.map_err(|_| "lex error".to_string())?;
        tokens.push((token, 0..0));
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
