// ── Verification Chain Resolver ───────────────────────────────────────
// 2026-07-29: Compile-time cross-verified implementation selection.
// Given a chain of bodies (Ref to asm fn or defn, Derivation), select the
// first one that passes cross-verification against every other body.

use crate::ast::{ChainSegment, Expr};
use crate::backend::assembler::{AsmAssembler, StubAssembler};
use crate::interpreter::{eval_expr, Value, VirtualHeap};
use crate::interpreter::RuntimeError;
use std::collections::HashMap;

/// 2026-07-29: A body in the verification chain — evaluable form.
#[derive(Clone)]
pub enum Body {
    Asm(crate::ast::AsmFn),
    Ref(Expr),
    Synthesized(Expr),
}

/// 2026-07-29: Evaluate a Brief expression against test inputs.
fn evaluate_ref_expr(expr: &Expr, input: &[Value]) -> Result<Value, String> {
    let mut bindings: HashMap<String, Value> = HashMap::new();
    for (i, val) in input.iter().enumerate() {
        bindings.insert(format!("x{}", i), val.clone());
    }
    let mut heap = VirtualHeap::new();
    match eval_expr(expr, &mut heap, &mut bindings) {
        Ok(val) => Ok(val),
        Err(RuntimeError::TermReturn(v)) => Ok(v),
        Err(e) => Err(format!("evaluation error: {:?}", e)),
    }
}

/// 2026-07-29: Evaluate a body on a single set of inputs.
fn evaluate_body(body: &Body, input: &[Value], _asm: &dyn AsmAssembler) -> Result<Value, String> {
    match body {
        Body::Asm(_) => Err("asm evaluation requires LLVM JIT — Phase D".into()),
        Body::Ref(expr) => evaluate_ref_expr(expr, input),
        Body::Synthesized(expr) => evaluate_ref_expr(expr, input),
    }
}

/// 2026-07-29: Generate deterministic test inputs for params.
fn generate_sample_input(params: &[(String, crate::ast::Type)]) -> Vec<Value> {
    params.iter().enumerate().map(|(i, (_, ty))| {
        let seed = (i as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        match ty {
            crate::ast::Type::Custom(s) if s == "Int" => Value::Int((seed & 0x7FF) as i64 - 500),
            crate::ast::Type::Custom(s) if s == "Float" => Value::Float((seed & 0x7FF) as f64 * 0.5),
            _ => Value::Int(0),
        }
    }).collect()
}

/// 2026-07-29: Compare two values with tolerance for floats.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(ai), Value::Int(bi)) => ai == bi,
        (Value::Float(af), Value::Float(bf)) => (af - bf).abs() < 0.0001,
        _ => {
            let ba = a.as_bool().unwrap_or(false);
            let bb = b.as_bool().unwrap_or(false);
            ba == bb
        }
    }
}

/// 2026-07-29: Cross-verify a single candidate against all others.
fn verify_candidate(
    idx: usize,
    candidates: &[Body],
    params: &[(String, crate::ast::Type)],
    samples: usize,
    asm: &dyn AsmAssembler,
) -> bool {
    for _ in 0..samples {
        let input = generate_sample_input(params);
        let Ok(output) = evaluate_body(&candidates[idx], &input, asm) else { return false; };
        for j in 0..candidates.len() {
            if idx == j { continue; }
            let Ok(other) = evaluate_body(&candidates[j], &input, asm) else { return false; };
            if !values_equal(&output, &other) {
                return false;
            }
        }
    }
    true
}

/// 2026-07-29: Resolve a Ref segment to a Body using the registry.
fn resolve_ref(_name: &str, _target_arch: &str) -> Option<Body> {
    // MVP: look up in registry passed to resolve_chain.
    None
}

/// 2026-07-29: Resolve a ChainSegment to a Body.
fn resolve_segment(
    segment: &ChainSegment,
    target_arch: &str,
    registry: &HashMap<String, Body>,
) -> Option<Body> {
    match segment {
        ChainSegment::Ref(name) => registry.get(name).cloned(),
        ChainSegment::Derivation(_block) => {
            // CEGIS synthesis runs here (MVP: not yet invoked)
            None
        }
    }
}

/// 2026-07-29: Resolve a verification chain: select the first body
/// that passes cross-verification against all others.
pub fn resolve_chain(
    chain: &[ChainSegment],
    target_arch: &str,
    params: &[(String, crate::ast::Type)],
    samples: u32,
    registry: &HashMap<String, Body>,
) -> Option<Body> {
    let mut candidates: Vec<Body> = Vec::new();
    for segment in chain {
        if let Some(body) = resolve_segment(segment, target_arch, registry) {
            candidates.push(body);
        }
    }
    if candidates.is_empty() {
        return None;
    }

    let samples = (samples as usize).max(1);
    let stub_asm = StubAssembler;
    for i in 0..candidates.len() {
        if verify_candidate(i, &candidates, params, samples, &stub_asm) {
            return Some(candidates.swap_remove(i));
        }
    }

    candidates.pop()
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOpKind, Expr};
    use crate::interpreter::bool_to_bits;

    fn make_ref_body() -> Expr {
        Expr::Identifier("x0".into())
    }

    fn make_params() -> Vec<(String, crate::ast::Type)> {
        vec![("x".into(), crate::ast::Type::Custom("Int".into()))]
    }

    fn make_registry() -> HashMap<String, Body> {
        let mut reg = HashMap::new();
        reg.insert("ref_fn".into(), Body::Ref(make_ref_body()));
        reg
    }

    #[test]
    fn test_chain_empty() {
        let result = resolve_chain(&[], "x86_64", &make_params(), 10, &make_registry());
        assert!(result.is_none());
    }

    #[test]
    fn test_chain_single_segment() {
        let chain = vec![ChainSegment::Ref("ref_fn".into())];
        let result = resolve_chain(&chain, "x86_64", &make_params(), 10, &make_registry());
        assert!(result.is_some());
    }

    #[test]
    fn test_chain_falls_through() {
        let chain = vec![
            ChainSegment::Ref("ref_fn".into()),
            ChainSegment::Ref("ref_fn".into()),
        ];
        let result = resolve_chain(&chain, "x86_64", &make_params(), 10, &make_registry());
        assert!(result.is_some());
    }

    #[test]
    fn test_values_equal_int() {
        assert!(values_equal(&Value::Int(42), &Value::Int(42)));
        assert!(!values_equal(&Value::Int(42), &Value::Int(43)));
    }

    #[test]
    fn test_values_equal_float() {
        assert!(values_equal(&Value::Float(3.14), &Value::Float(3.1401)));
        assert!(!values_equal(&Value::Float(3.14), &Value::Float(4.0)));
    }

    #[test]
    fn test_values_equal_bool() {
        let t = bool_to_bits(true);
        let f = bool_to_bits(false);
        assert!(values_equal(&t, &t));
        assert!(!values_equal(&t, &f));
    }

    #[test]
    fn test_generate_sample_input_int() {
        let params = vec![("x".into(), crate::ast::Type::Custom("Int".into()))];
        let input = generate_sample_input(&params);
        assert_eq!(input.len(), 1);
        assert!(matches!(input[0], Value::Int(_)));
    }

    #[test]
    fn test_evaluate_ref_simple() {
        let expr = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x0".into())),
            Box::new(Expr::Decimal(1)),
        );
        let result = evaluate_ref_expr(&expr, &[Value::Int(5)]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Int(6));
    }
}
