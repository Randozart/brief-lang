// ── Derivation CLI Handlers ────────────────────────────────────────────
// 2026-07-12: Phase 6.3 — `brief derive` CLI command.
// 2026-07-28: Phase I.0 — Added DeriveConfig, flag parsing, MCMC + doppelganger output.
// Flat code: max 2 levels of nesting.

use crate::ast::{DerivationBlock, Expr, TopLevel, Type};
use crate::derive::engine::{CostModel, SynthesizedProgram};
use crate::derive::{synthesize, SynthesizeError};
use std::fs;
use std::path::Path;

/// Configuration for `brief derive` command.
/// 2026-07-28: Phase I.0 — Flags parsed from CLI args.
#[derive(Debug, Clone)]
pub struct DeriveConfig {
    pub stochastic: bool,
    pub iterations: usize,
    pub temperature: f64,
    pub enumerative_depth: usize,
    pub process_all: bool,
    // 2026-07-28: Tier 2/3 — verification samples (0 = disable verification)
    pub verify_samples: usize,
}

impl Default for DeriveConfig {
    fn default() -> Self {
        DeriveConfig {
            stochastic: false,
            iterations: 10_000,
            temperature: 1.0,
            // 2026-07-28: Default depth 3 (not 5) to keep synthesis fast.
            // Depth 5 generates ~10k+ candidates for (Int, Int) -> Int,
            // causing multi-minute CPU hangs. Users who need deeper search
            // can pass --enumerative-depth 5 explicitly.
            enumerative_depth: 3,
            process_all: false,
            // 2026-07-28: Default 50 verification samples. Disable with 0.
            // Each sample evaluates the candidate against random inputs and
            // checks for evaluation errors, constant-output, and postconditions.
            verify_samples: 50,
        }
    }
}

/// Parse `--key=value` or `--key value` style flags from args.
/// Returns (DeriveConfig, positional_args).
/// 2026-07-28: Phase I.0 — Hand-written flag parser (no clap dependency).
pub fn parse_derive_flags(args: &[String]) -> Result<(DeriveConfig, Vec<String>), String> {
    let mut config = DeriveConfig::default();
    let mut positional = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--stochastic" {
            config.stochastic = true;
            i += 1;
        } else if arg == "--all" {
            config.process_all = true;
            i += 1;
        } else if arg == "--iterations" {
            i += 1;
            let val = args.get(i).ok_or("--iterations requires a value")?;
            config.iterations = val.parse::<usize>()
                .map_err(|_| format!("invalid --iterations value '{}'", val))?;
            i += 1;
        } else if arg == "--temperature" {
            i += 1;
            let val = args.get(i).ok_or("--temperature requires a value")?;
            config.temperature = val.parse::<f64>()
                .map_err(|_| format!("invalid --temperature value '{}'", val))?;
            i += 1;
        } else if arg == "--enumerative-depth" {
            i += 1;
            let val = args.get(i).ok_or("--enumerative-depth requires a value")?;
            config.enumerative_depth = val.parse::<usize>()
                .map_err(|_| format!("invalid --enumerative-depth value '{}'", val))?;
            i += 1;
        } else if arg == "--verify-samples" {
            i += 1;
            let val = args.get(i).ok_or("--verify-samples requires a value")?;
            config.verify_samples = val.parse::<usize>()
                .map_err(|_| format!("invalid --verify-samples value '{}'", val))?;
            i += 1;
        } else if arg.starts_with("--") {
            return Err(format!("unknown flag '{}'", arg));
        } else {
            positional.push(arg.clone());
            i += 1;
        }
    }
    Ok((config, positional))
}

/// Handle the `brief derive` command.
/// Reads a Brief source file, finds derivation blocks, synthesizes bodies,
/// optionally runs MCMC superoptimization, writes doppelganger output.
pub fn handle_derive_command(config: &DeriveConfig, file_path: &str) -> Result<(), String> {
    let path = Path::new(file_path);
    let source = fs::read_to_string(path)
        .map_err(|e| format!("cannot read '{}': {}", file_path, e))?;

    // Lex, parse, find derivation blocks
    let token_spans = lex_source_with_spans(&source)?;
    let program = parse_tokens(&token_spans, &source)?;

    // Collect derivation blocks with their names and parameter types
    // 2026-07-28: Include params so synthesized expressions use actual param names
    let mut derivations: Vec<(String, Vec<(String, Type)>, DerivationBlock)> = Vec::new();
    for item in &program {
        match item {
            TopLevel::Definition(d) => {
                if let Some(ref block) = d.derivation {
                    derivations.push((d.name.clone(), d.parameters.clone(), block.clone()));
                }
            }
            TopLevel::Transaction(t) => {
                if let Some(ref block) = t.derivation {
                    derivations.push((t.name.clone(), t.parameters.clone(), block.clone()));
                }
            }
            _ => {}
        }
    }

    if derivations.is_empty() {
        eprintln!("[derive] no derivation blocks found in '{}'", file_path);
        return Ok(());
    }

    // Synthesize each derivation block
    let mut syntheses: Vec<(String, SynthesizedProgram)> = Vec::new();
    for (name, params, block) in &derivations {
        eprintln!("[derive] synthesizing '{}' (depth={})...", name, config.enumerative_depth);
        // Use Type::int() as default return type for derivation blocks
        // TODO: extract actual return type from function signature
        let ret_type = Type::int();
        match synthesize(name, block, params, &ret_type, config.enumerative_depth, config.verify_samples, block.postcondition.as_ref(), block.precondition.as_ref()) {
            Ok(prog) => {
                eprintln!("[derive] '{}': synthesized body with cost {}", name, prog.cost);
                syntheses.push((name.clone(), prog));
            }
            Err(e) => {
                eprintln!("warn: synthesis failed for '{}': {}", name, e);
            }
        }
    }

    if syntheses.is_empty() {
        return Err("synthesis failed for all derivation blocks".into());
    }

    // 2026-07-28: Extract (name, block) pairs for doppelganger writer
    let derivations_blocks: Vec<(String, DerivationBlock)> = derivations.iter()
        .map(|(n, _, b)| (n.clone(), b.clone()))
        .collect();

    // Optionally run MCMC superoptimization
    if config.stochastic {
        eprintln!("[derive] running MCMC superoptimization...");
        let mcmc_config = crate::derive::mcmc::McmcConfig {
            initial_temperature: config.temperature,
            max_iterations: config.iterations,
            ..crate::derive::mcmc::McmcConfig::default()
        };
        let mut mcmc_syntheses: Vec<(String, SynthesizedProgram)> = Vec::new();
        for (name, prog) in &syntheses {
            // Find the derivation block for examples
            let block = derivations.iter()
                .find(|(n, _, _)| n == name)
                .map(|(_, _, b)| b);
            if let Some(block) = block {
                match crate::derive::mcmc::optimize(prog.clone(), &block.examples, &mcmc_config) {
                    Ok(improved) => {
                        eprintln!("[derive] '{}': MCMC optimized cost {} -> {}", name, prog.cost, improved.cost);
                        mcmc_syntheses.push((name.clone(), improved));
                    }
                    Err(e) => {
                        eprintln!("warn: MCMC failed for '{}': {} — keeping synthesized", name, e);
                        mcmc_syntheses.push((name.clone(), prog.clone()));
                    }
                }
            }
        }
        // Write .opt.bv with MCMC results
        let opt_path = crate::derive::doppelganger::Doppelganger::opt_path_for(path);
        crate::derive::doppelganger::write_doppelganger(path, source.as_bytes(), &mcmc_syntheses, &derivations_blocks, &opt_path)?;
        eprintln!("[derive] wrote {}", opt_path.display());
    } else {
        // Write .derive.bv with synthesis results
        let derive_path = crate::derive::doppelganger::Doppelganger::derive_path_for(path);
        crate::derive::doppelganger::write_doppelganger(path, source.as_bytes(), &syntheses, &derivations_blocks, &derive_path)?;
        eprintln!("[derive] wrote {}", derive_path.display());
    }

    Ok(())
}

/// Lex a Brief source into (Token, byte_range) pairs using logos.
/// 2026-07-28: Preserves byte spans so derivation block positions are correct.
fn lex_source_with_spans(source: &str) -> Result<Vec<(crate::lexer::Token, std::ops::Range<usize>)>, String> {
    use logos::Logos;
    let mut lexer = crate::lexer::Token::lexer(source);
    let mut result = Vec::new();
    while let Some(token_result) = lexer.next() {
        match token_result {
            Ok(token) => {
                let span = lexer.span();
                result.push((token, span));
            }
            Err(_) => return Err("lex error".to_string()),
        }
    }
    Ok(result)
}

/// Parse tokens with spans into a program.
fn parse_tokens(
    token_spans: &[(crate::lexer::Token, std::ops::Range<usize>)],
    source: &str,
) -> Result<Vec<TopLevel>, String> {
    let mut parser = crate::parser::Parser::new(token_spans.to_vec(), source);
    parser.parse_program().map_err(|e| format!("parse error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_source_simple() {
        let source = "defn add(a: Int, b: Int) -> Int";
        let result = lex_source_with_spans(source);
        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_lex_source_empty() {
        let result = lex_source_with_spans("");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_lex_source_error() {
        let result = lex_source_with_spans("\"unterminated string");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_flags_default() {
        let args: Vec<String> = vec!["foo.bv".into()];
        let (config, positional) = parse_derive_flags(&args).unwrap();
        assert!(!config.stochastic);
        assert_eq!(config.iterations, 10_000);
        assert_eq!(config.temperature, 1.0);
        // 2026-07-28: Default depth changed from 5 to 3 for performance
        assert_eq!(config.enumerative_depth, 3);
        assert!(!config.process_all);
        assert_eq!(positional, vec!["foo.bv"]);
    }

    #[test]
    fn test_parse_flags_stochastic() {
        let args: Vec<String> = vec!["--stochastic".into(), "foo.bv".into()];
        let (config, positional) = parse_derive_flags(&args).unwrap();
        assert!(config.stochastic);
        assert_eq!(positional, vec!["foo.bv"]);
    }

    #[test]
    fn test_parse_flags_custom_values() {
        let args: Vec<String> = vec![
            "--stochastic".into(),
            "--iterations".into(), "5000".into(),
            "--temperature".into(), "0.5".into(),
            "--enumerative-depth".into(), "8".into(),
            "--all".into(),
            "foo.bv".into(),
        ];
        let (config, positional) = parse_derive_flags(&args).unwrap();
        assert!(config.stochastic);
        assert_eq!(config.iterations, 5000);
        assert!((config.temperature - 0.5).abs() < 1e-9);
        assert_eq!(config.enumerative_depth, 8);
        assert!(config.process_all);
        assert_eq!(positional, vec!["foo.bv"]);
    }

    #[test]
    fn test_parse_flags_unknown_yields_error() {
        let args: Vec<String> = vec!["--nonexistent".into(), "foo.bv".into()];
        let result = parse_derive_flags(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_flags_missing_value() {
        let args: Vec<String> = vec!["--iterations".into()];
        let result = parse_derive_flags(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_flags_invalid_value() {
        let args: Vec<String> = vec!["--iterations".into(), "not_a_number".into()];
        let result = parse_derive_flags(&args);
        assert!(result.is_err());
    }
}
