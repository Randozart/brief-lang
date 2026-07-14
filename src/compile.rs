// ── Compilation Pipeline ──────────────────────────────────────────────
// 2026-07-12: Phase 7 — Compile a Brief source file end-to-end.
// Pipeline: lex -> parse -> typecheck -> codegen -> output.
// 2026-07-14: Wire real LlvmBackend instead of stub codegen.
//             Add binary compilation via clang. Add --out / --optimize-budget flags.
// 2026-07-14: Plugin path — serialize to BVIR, run external plugins, deserialize.

use std::path::Path;
use std::process::Command;

use brief_compiler::backend::llvm::LlvmBackend;
use brief_compiler::lexer::Token;
use brief_compiler::plugin::runner::run_plugin_chain;
use brief_compiler::plugin::PluginManager;
use brief_compiler::target::{BackendKind, TargetConfig};
use brief_compiler::type_universe::TypeUniverse;

/// Options parsed from the `brief-compiler build` CLI flags.
pub struct BuildOptions {
    pub file_path: String,
    pub emit_ir_only: bool,
    pub out_dir: Option<String>,
    pub optimize_budget: u64,
    pub gpu_offload: bool,
    pub plugin_paths: Vec<String>,
    pub emit_bvir: bool,
    /// Selected backend (resolved from extension + --backend flag).
    pub backend: BackendKind,
}

/// Compile a Brief source file: produce an executable binary (or `.ll` with `--llvm`).
pub fn compile_source(file_path: &str, source: &str, opts: &BuildOptions) -> Result<(), String> {
    let (mut items, mut universe) = parse_and_check(file_path, source)?;

    // Plugin path: serialize to BVIR, run plugins, deserialize.
    let has_plugins = !opts.plugin_paths.is_empty();
    if has_plugins || opts.emit_bvir {
        let bvir_before = brief_compiler::bvir::to_bvir(&items, &universe);
        if opts.emit_bvir {
            let path = format!("{}.bvir.before", file_path.strip_suffix(".bv").unwrap_or(file_path));
            std::fs::write(&path, &bvir_before)
                .map_err(|e| format!("cannot write '{}': {}", path, e))?;
        }

        let bvir_after = if has_plugins {
            run_plugin_chain(&bvir_before, &opts.plugin_paths)?
        } else {
            bvir_before.clone()
        };

        if opts.emit_bvir {
            let path = format!("{}.bvir.after", file_path.strip_suffix(".bv").unwrap_or(file_path));
            std::fs::write(&path, &bvir_after)
                .map_err(|e| format!("cannot write '{}': {}", path, e))?;
        }

        let (restored_items, restored_universe) = brief_compiler::bvir::from_bvir(&bvir_after)?;
        items = restored_items;
        universe = restored_universe;
    }

    // Normalizer pass — annotate AST for the selected backend
    match opts.backend {
        BackendKind::Llvm | BackendKind::Gpu => {
            brief_compiler::backend::llvm::normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Circt => {
            brief_compiler::backend::circt_normalizer::normalize(&mut items, &mut universe)?;
        }
        BackendKind::Webstack => {
            // Webstack normalizer not yet implemented — pass through
        }
    }

    let mut output = String::new();
    let ext = match opts.backend {
        BackendKind::Llvm => {
            let mut b = LlvmBackend::new()
                .with_optimize_budget(opts.optimize_budget)
                .with_type_universe(universe);
            if opts.gpu_offload {
                b = b.with_gpu_offload(true);
            }
            output = b.generate(&items, None);

            // AfterCodegen plugin hook
            if has_plugins {
                let pm = PluginManager::new();
                let action = pm.run_ir_hooks(&mut output);
                if let brief_compiler::plugin::PluginAction::Abort(msg) = action {
                    return Err(msg);
                }
            }
            ".ll"
        }
        BackendKind::Circt => {
            let mut b = brief_compiler::backend::circt::CirctBackend::new();
            output = b.generate(&items);
            ".mlir"
        }
        BackendKind::Webstack => {
            let result = brief_compiler::backend::webstack::WebstackGenerator::new()
                .generate(&items, &[], "program");
            output = result.ts_code;
            ".ts"
        }
        BackendKind::Gpu => {
            let mut b = LlvmBackend::new()
                .with_optimize_budget(opts.optimize_budget)
                .with_type_universe(universe)
                .with_gpu_offload(true);
            output = b.generate(&items, None);
            ".ll"
        }
    };

    let out_path = determine_out_path(file_path, opts.out_dir.as_deref())?;
    let out_path = out_path.replace(".ll", ext);

    std::fs::write(&out_path, &output)
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
    let (_items, _universe) = parse_and_check(file_path, source)?;
    println!("OK");
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
    // 2026-07-14: Same flags used by benchmarks/build_and_bench.sh linking fallback.
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

/// Lex + parse + typecheck a source file, returning the TypeUniverse for the backend.
fn parse_and_check(file_path: &str, source: &str) -> Result<(Vec<brief_compiler::ast::TopLevel>, TypeUniverse), String> {
    let tokens = lex(source)?;
    let items = parse(file_path, &tokens, source)?;
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
        // 2026-07-12: span info from logos — currently stubbed.
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
