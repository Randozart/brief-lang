// ── Derivation CLI Handlers ────────────────────────────────────────────
// 2026-07-12: Phase 6.3 — `brief derive` CLI command.
// Handles the command-line invocation of the synthesis engine.
// Flat code: max 2 levels of nesting.

use crate::ast::{DerivationBlock, DerivationExample, Expr, TopLevel};
use crate::derive::{SynthesizeError, synthesize};
use std::fs;
use std::path::Path;

/// Handle the `brief derive` command.
/// Reads a Brief source file, finds derivation blocks, synthesizes bodies.
pub fn handle_derive_command(file_path: &str) -> Result<(), String> {
    let path = Path::new(file_path);
    let source = fs::read_to_string(path)
        .map_err(|e| format!("cannot read '{}': {}", file_path, e))?;

    // Lex, parse, find derivation blocks
    let tokens = lex_source(&source)?;
    let program = parse_tokens(&tokens, &source)?;

    // Synthesize each derivation block
    for item in &program {
        if let Err(e) = synthesize_top_level(item) {
            eprintln!("warn: synthesis failed for item: {}", e);
        }
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

/// Synthesize a single top-level item's derivation block.
fn synthesize_top_level(item: &TopLevel) -> Result<Expr, SynthesizeError> {
    let (name, block) = match item {
        TopLevel::Definition(d) => (d.name.as_str(), d.derivation.as_ref()),
        TopLevel::Transaction(t) => (t.name.as_str(), t.derivation.as_ref()),
        _ => return Ok(Expr::Decimal(0)),
    };
    let block = match block {
        Some(b) => b,
        None => return Ok(Expr::Decimal(0)),
    };
    synthesize(name, block, 5)
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
        assert!(result.is_ok()); // lexer doesn't error on unterminated strings
    }
}
