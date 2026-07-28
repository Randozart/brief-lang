// ── Derivation CLI Handlers ────────────────────────────────────────────
// 2026-07-12: Phase 6.3 — `brief derive` CLI command.
// 2026-07-28: Phase I.0 — Added DeriveConfig, flag parsing, MCMC + doppelganger output.
// Flat code: max 2 levels of nesting.

use crate::ast::{DerivationBlock, Expr, TopLevel};
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
    let tokens = lex_source(&source)?;
    let program = parse_tokens(&tokens, &source)?;

    // Collect derivation blocks with their names
    let mut derivations: Vec<(String, DerivationBlock)> = Vec::new();
    for item in &program {
        match item {
            TopLevel::Definition(d) => {
                if let Some(ref block) = d.derivation {
                    derivations.push((d.name.clone(), block.clone()));
                }
            }
            TopLevel::Transaction(t) => {
                if let Some(ref block) = t.derivation {
                    derivations.push((t.name.clone(), block.clone()));
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
    for (name, block) in &derivations {
        eprintln!("[derive] synthesizing '{}' (depth={})...", name, config.enumerative_depth);
        match synthesize(name, block, config.enumerative_depth) {
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
                .find(|(n, _)| n == name)
                .map(|(_, b)| b);
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
        crate::derive::doppelganger::write_doppelganger(path, source.as_bytes(), &mcmc_syntheses, &derivations, &opt_path)?;
        eprintln!("[derive] wrote {}", opt_path.display());
    } else {
        // Write .derive.bv with synthesis results
        let derive_path = crate::derive::doppelganger::Doppelganger::derive_path_for(path);
        crate::derive::doppelganger::write_doppelganger(path, source.as_bytes(), &syntheses, &derivations, &derive_path)?;
        eprintln!("[derive] wrote {}", derive_path.display());
    }

    Ok(())
}

/// Lex a Brief source file into tokens.
fn lex_source(source: &str) -> Result<Vec<crate::lexer::Token>, String> {
    let lexer = { use logos::Logos; crate::lexer::Token::lexer(source) };
    let tokens: Result<Vec<_>, _> = lexer.collect();
    tokens.map_err(|_| "lex error".to_string())
}

/// Parse tokens into a program (using the new parser).
fn parse_tokens(
    tokens: &[crate::lexer::Token],
    source: &str,
) -> Result<Vec<TopLevel>, String> {
    let token_spans: Vec<_> = tokens.iter()
        .map(|t| (t.clone(), 0..0))
        .collect();
    let mut parser = crate::parser::Parser::new(token_spans, source);
    parser.parse_program().map_err(|e| format!("parse error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_source_simple() {
        let source = "defn add(a: Int, b: Int) -> Int";
        let result = lex_source(source);
        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert!(!tokens.is_empty());
    }

    #[test]
    fn test_lex_source_empty() {
        let result = lex_source("");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_lex_source_error() {
        let result = lex_source("\"unterminated string");
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
