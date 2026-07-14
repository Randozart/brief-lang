// ── Compilation Pipeline ──────────────────────────────────────────────
// 2026-07-12: Phase 7 — Compile a Brief source file end-to-end.
// Pipeline: lex -> parse -> typecheck -> codegen -> output.
// 2026-07-14: Wire real LlvmBackend instead of stub codegen.
//             Add binary compilation via clang. Add --out / --optimize-budget flags.

use std::path::Path;
use std::process::Command;

use brief_compiler::backend::llvm::LlvmBackend;
use brief_compiler::lexer::Token;
use brief_compiler::type_universe::TypeUniverse;

/// Options parsed from the `brief-compiler build` CLI flags.
pub struct BuildOptions {
    /// Path to the source `.bv` file.
    pub file_path: String,
    /// Emit `.ll` only; skip binary compilation (`--llvm`).
    pub emit_ir_only: bool,
    /// Output directory (`--out <dir>`). `None` → same directory as input file.
    pub out_dir: Option<String>,
    /// Max simulation steps before runtime fallback (`--optimize-budget <N>`).
    pub optimize_budget: u64,
    /// Enable GPU offload codegen (`--gpu-offload`).
    pub gpu_offload: bool,
}

/// Compile a Brief source file: produce an executable binary (or `.ll` with `--llvm`).
pub fn compile_source(file_path: &str, source: &str, opts: &BuildOptions) -> Result<(), String> {
    let (items, universe) = parse_and_check(file_path, source)?;

    let mut backend = LlvmBackend::new()
        .with_optimize_budget(opts.optimize_budget)
        .with_type_universe(universe);

    if opts.gpu_offload {
        backend = backend.with_gpu_offload(true);
    }

    let llvm_ir = backend.generate(&items, None);

    let ll_path = determine_out_path(file_path, opts.out_dir.as_deref())?;

    std::fs::write(&ll_path, &llvm_ir)
        .map_err(|e| format!("cannot write '{}': {}", ll_path, e))?;
    println!("wrote {}", ll_path);

    if !opts.emit_ir_only {
        // 2026-07-14: Compile .ll to binary using clang.
        // The `.ll` file is kept for debugging.
        let binary_path = ll_path.strip_suffix(".ll").unwrap_or(&ll_path);
        compile_to_binary(&ll_path, binary_path)?;
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
fn compile_to_binary(ll_path: &str, binary_path: &str) -> Result<(), String> {
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
