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

use crate::ast::{DerivationBlock, DerivationExample, Expr};

/// Synthesize a function body from a derivation block.
/// Tries the fast enumerative engine first, falls back to SMT if needed.
/// Returns a `SynthesizedProgram` with cost for doppelganger/MCMC pipelines.
/// 2026-07-28: Phase I.0 — Changed return type from `Expr` to `SynthesizedProgram`.
pub fn synthesize(name: &str, block: &DerivationBlock, max_depth: usize) -> Result<engine::SynthesizedProgram, SynthesizeError> {
    if block.examples.is_empty() {
        return Err(SynthesizeError::NoExamples(name.to_string()));
    }
    // Try enumerative search first
    match engine::enumerative_search(name, &block.examples, max_depth) {
        Ok(Some(expr)) => {
            let cost = engine::CostModel::default().cost_of_expr(&expr);
            return Ok(engine::SynthesizedProgram { body: vec![expr], cost, depth: max_depth as u8 });
        }
        Ok(None) => {} // fall through to SMT
        Err(e) => return Err(e),
    }
    // Fall back to SMT solver
    match smt::synthesize_via_smt(name, &block.examples) {
        Ok(expr) => {
            let cost = engine::CostModel::default().cost_of_expr(&expr);
            Ok(engine::SynthesizedProgram { body: vec![expr], cost, depth: max_depth as u8 })
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
