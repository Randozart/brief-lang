// ── Derivation Module — Program Synthesis from Examples ───────────────
// 2026-07-12: Phase 6 — Enumerative and SMT-guided program synthesis.
// Generates function bodies from `:=` derivation blocks.
// 2026-07-28: Phase B — Added assertion verification module.
// Flat code: each function max 2 levels of nesting.

mod engine;
mod smt;
mod cli;
mod assert;
mod doppelganger;
mod mcmc;
mod mutate;
mod equivalence;
mod pareto;
mod accept;
mod verify;

pub use engine::*;
pub use smt::*;
pub use cli::*;
pub use assert::*;
pub use doppelganger::*;
pub use mcmc::*;
pub use mutate::*;
pub use equivalence::*;
pub use pareto::*;
pub use accept::*;
pub use verify::*;

use crate::ast::{DerivationBlock, DerivationExample, Expr, Type};

/// Synthesize a function body from a derivation block.
/// Tries the fast enumerative engine first, falls back to SMT if needed.
/// Returns a `SynthesizedProgram` with cost for doppelganger/MCMC pipelines.
/// 2026-07-28: Phase I.0 — Changed return type from `Expr` to `SynthesizedProgram`.
/// 2026-07-28: Added `params` so synthesized expressions use actual param names.
/// 2026-07-28: Added `verify_samples` for Tier 2/3 overfitting prevention.
pub fn synthesize(
    name: &str,
    block: &DerivationBlock,
    params: &[(String, Type)],
    max_depth: usize,
    verify_samples: usize,
) -> Result<engine::SynthesizedProgram, SynthesizeError> {
    if block.examples.is_empty() {
        return Err(SynthesizeError::NoExamples(name.to_string()));
    }
    // Try enumerative search first
    if let Ok(Some(expr)) = engine::enumerative_search(name, params, &block.examples, max_depth) {
        // Tier 2 + 3: Verify candidate against random inputs + postcondition
        if verify_samples > 0 {
            match verify::verify_candidate(&expr, params, None, verify_samples) {
                verify::VerifyResult::Pass => { /* accept */ }
                verify::VerifyResult::Fail(_, reason) => {
                    // Candidate overfitted — reject and fall through to SMT
                    eprintln!("  verify: '{}' rejected ({}) — trying SMT", name, reason);
                    // Fall through to SMT below
                    return synthesize_via_smt_with_verify(name, params, &block.examples, verify_samples);
                }
            }
        }
        let cost = engine::CostModel::default().cost_of_expr(&expr);
        return Ok(engine::SynthesizedProgram { body: vec![expr], cost, depth: max_depth as u8 });
    }
    // Fall back to SMT solver
    synthesize_via_smt_with_verify(name, params, &block.examples, verify_samples)
}

/// Call SMT synthesis then verify the result.
fn synthesize_via_smt_with_verify(
    name: &str,
    params: &[(String, Type)],
    examples: &[DerivationExample],
    verify_samples: usize,
) -> Result<engine::SynthesizedProgram, SynthesizeError> {
    match smt::synthesize_via_smt(name, params, examples) {
        Ok(expr) => {
            if verify_samples > 0 {
                match verify::verify_candidate(&expr, params, None, verify_samples) {
                    verify::VerifyResult::Pass => { /* accept */ }
                    verify::VerifyResult::Fail(_, reason) => {
                        eprintln!("  verify: SMT result for '{}' rejected ({})", name, reason);
                        return Err(SynthesizeError::NoSolution(
                            format!("SMT result for '{}' rejected by verification: {}", name, reason)
                        ));
                    }
                }
            }
            let cost = engine::CostModel::default().cost_of_expr(&expr);
            Ok(engine::SynthesizedProgram { body: vec![expr], cost, depth: 0 })
        }
        Err(e) => Err(e),
    }
}

/// Error types for the synthesis engine.
#[derive(Debug, Clone)]
pub enum SynthesizeError {
    NoExamples(String),
    TypeMismatch(String),
    DepthExceeded(String, usize),
    SolverError(String),
    SolverUnavailable(String),
    NoSolution(String),
}

impl std::fmt::Display for SynthesizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SynthesizeError::NoExamples(name) => {
                write!(f, "derivation block '{}' has no examples", name)
            }
            SynthesizeError::TypeMismatch(msg) => write!(f, "type mismatch: {}", msg),
            SynthesizeError::DepthExceeded(name, depth) => {
                write!(f, "synthesis of '{}' exceeded depth {}", name, depth)
            }
            SynthesizeError::SolverError(msg) => write!(f, "SMT solver error: {}", msg),
            SynthesizeError::SolverUnavailable(name) => {
                write!(f, "SMT solver is not available; derivation of '{}' requires it", name)
            }
            SynthesizeError::NoSolution(name) => {
                write!(f, "no solution found for '{}'", name)
            }
        }
    }
}
