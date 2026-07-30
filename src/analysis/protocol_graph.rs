// 2026-07-30: Protocol round-trip and cross-op equivalence verification.
// The ProtocolGraph struct was merged into CastingGraph (src/casting/graph.rs).
// Only verification functions remain here.

use crate::ast::top::{CastDirection, CastEdge, ProtocolDef, TopLevel};
use crate::ast::{Contract, Expr, PropertyValue, Statement, Type};

/// Find a defn body by name in the top-level items.
/// Returns the body expression if it's a single-term defn.
pub fn find_defn_body<'a>(name: &str, items: &'a [TopLevel]) -> Option<&'a Expr> {
    for item in items {
        if let TopLevel::Definition(d) = item {
            if d.name == name {
                for stmt in &d.body {
                    if let Statement::Term(val) = stmt {
                        if let Some(expr) = val {
                            return Some(expr);
                        }
                    }
                }
                return None;
            }
        }
    }
    None
}

/// Verify round-trip identity for a protocol declaration.
/// For matching CastTo/CastFrom pairs, proves that
/// CastFrom(CastTo(x)) == x via symbolic evaluation or SMT.
pub fn verify_protocol_roundtrip(
    pd: &ProtocolDef,
    items: &[TopLevel],
) -> Result<(), String> {
    let to = pd.cast_edges.iter().find(|e| {
        matches!(e.direction, CastDirection::CastTo) && e.binding.is_some()
    });
    let from = pd.cast_edges.iter().find(|e| {
        matches!(e.direction, CastDirection::CastFrom) && e.binding.is_some()
    });

    let (to_edge, from_edge) = match (to, from) {
        (Some(t), Some(f)) if t.target_category == f.target_category => (t, f),
        _ => return Ok(()),
    };

    let to_fn = to_edge.binding.as_ref().map(|b| &b.fn_name).unwrap();
    let from_fn = from_edge.binding.as_ref().map(|b| &b.fn_name).unwrap();

    let forward_body = find_defn_body(to_fn, items);
    let inverse_body = find_defn_body(from_fn, items);

    match (forward_body, inverse_body) {
        (Some(fwd), Some(inv)) => {
            let sym_input = {
                let mut m = std::collections::HashMap::new();
                m.insert("#L".to_string(), crate::symbolic::SymbolicValue::Identifier("__x".into()));
                m
            };

            let mid = crate::symbolic::eval_symbolic_expr(fwd, &sym_input);
            let mut inv_input = std::collections::HashMap::new();
            inv_input.insert("#L".to_string(), mid.clone());
            let output = crate::symbolic::eval_symbolic_expr(inv, &inv_input);

            if symbolic_deep_equals(&output, &crate::symbolic::SymbolicValue::Identifier("__x".into())) {
                return Ok(());
            }

            eprintln!("warning: symbolic round-trip inconclusive for '{}', trying SMT", pd.name);
            let formula = build_roundtrip_smt(fwd, inv);
            let result = crate::proof_engine::smt::prove_smt_formula(&formula, 1000);
            match result {
                crate::proof_engine::smt::SmtResult::Unsat => Ok(()),
                _ => Err(format!(
                    "round-trip proof failed for protocol '{}': {} and {} are not inverses",
                    pd.name, to_fn, from_fn
                )),
            }
        }
        _ => {
            eprintln!(
                "warning: round-trip proof skipped for '{}' — {:?}/{:?} bodies not found",
                pd.name, to_fn, from_fn
            );
            Ok(())
        }
    }
}

/// Verify cross-op equivalence for a protocol declaration.
/// For each cross-op override, proves that the custom implementation
/// matches the default CastTo -> op -> CastFrom round-trip.
pub fn verify_crossop_equivalence(
    pd: &ProtocolDef,
    items: &[TopLevel],
) -> Result<(), String> {
    let to = pd.cast_edges.iter().find(|e| {
        matches!(e.direction, CastDirection::CastTo) && e.binding.is_some()
    });
    let from = pd.cast_edges.iter().find(|e| {
        matches!(e.direction, CastDirection::CastFrom) && e.binding.is_some()
    });

    let (to_edge, from_edge) = match (to, from) {
        (Some(t), Some(f)) => (t, f),
        _ => return Ok(()),
    };

    for op in &pd.cross_ops {
        let Some(ref impl_args) = op.impl_args else { continue };

        let custom_fn = format!("{:?}", impl_args);
        let Some(custom_body) = find_defn_body(&custom_fn.trim_matches('"'), items) else {
            eprintln!("warning: cross-op '{}' body not found, skipping equivalence check", custom_fn);
            continue;
        };

        let to_fn = to_edge.binding.as_ref().map(|b| &b.fn_name).unwrap();
        let from_fn = from_edge.binding.as_ref().map(|b| &b.fn_name).unwrap();
        let Some(forward_body) = find_defn_body(to_fn, items) else { continue; };
        let Some(inverse_body) = find_defn_body(from_fn, items) else { continue; };

        let formula = build_crossop_smt(forward_body, inverse_body, custom_body);
        let result = crate::proof_engine::smt::prove_smt_formula(&formula, 1000);
        match result {
            crate::proof_engine::smt::SmtResult::Unsat => {},
            _ => return Err(format!(
                "cross-op equivalence proof failed for protocol '{}': op '{}' not equivalent to round-trip",
                pd.name, op.op
            )),
        }
    }

    Ok(())
}

/// Build an SMT-LIB formula proving the round-trip identity.
fn build_roundtrip_smt(forward: &Expr, inverse: &Expr) -> String {
    let fwd_smt = crate::proof_engine::smt::encode_expr_smt_for_proof(forward);
    let inv_smt = crate::proof_engine::smt::encode_expr_smt_for_proof(inverse);
    format!(
        "(set-logic QF_BV)\n\
         (declare-const x (_ BitVec 64))\n\
         (define-fun forward ((x (_ BitVec 64))) (_ BitVec 64) {})\n\
         (define-fun inverse ((x (_ BitVec 64))) (_ BitVec 64) {})\n\
         (assert (not (= (inverse (forward x)) x)))\n\
         (check-sat)\n",
        fwd_smt, inv_smt
    )
}

/// Build an SMT-LIB formula proving cross-op equivalence.
fn build_crossop_smt(forward: &Expr, inverse: &Expr, custom: &Expr) -> String {
    let fwd_smt = crate::proof_engine::smt::encode_expr_smt_for_proof(forward);
    let inv_smt = crate::proof_engine::smt::encode_expr_smt_for_proof(inverse);
    let custom_smt = crate::proof_engine::smt::encode_expr_smt_for_proof(custom);
    format!(
        "(set-logic QF_BV)\n\
         (declare-const x (_ BitVec 64))\n\
         (declare-const y (_ BitVec 64))\n\
         (define-fun forward ((x (_ BitVec 64))) (_ BitVec 64) {})\n\
         (define-fun inverse ((x (_ BitVec 64))) (_ BitVec 64) {})\n\
         (define-fun default_path ((x (_ BitVec 64)) (y (_ BitVec 64))) (_ BitVec 64)\n\
           (inverse (bvadd (forward x) y)))\n\
         (define-fun custom_path ((x (_ BitVec 64)) (y (_ BitVec 64))) (_ BitVec 64) {})\n\
         (assert (not (= (default_path x y) (custom_path x y))))\n\
         (check-sat)\n",
        fwd_smt, inv_smt, custom_smt
    )
}

/// Simple symbolic comparison (deep equals).
fn symbolic_deep_equals(a: &crate::symbolic::SymbolicValue, b: &crate::symbolic::SymbolicValue) -> bool {
    use crate::symbolic::SymbolicValue;
    match (a, b) {
        (SymbolicValue::Identifier(an), SymbolicValue::Identifier(bn)) => an == bn,
        (SymbolicValue::Literal(av, _), SymbolicValue::Literal(bv, _)) => av == bv,
        (SymbolicValue::Binary(op_a, la, ra), SymbolicValue::Binary(op_b, lb, rb)) => {
            op_a == op_b && symbolic_deep_equals(la, lb) && symbolic_deep_equals(ra, rb)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_edge(dir: CastDirection, cat: &str, var: &str) -> CastEdge {
        CastEdge {
            direction: dir,
            target_category: cat.to_string(),
            target_variant: var.to_string(),
            binding: None,
        }
    }

    #[test]
    fn test_find_defn_body_not_found() {
        let items: Vec<TopLevel> = vec![];
        assert!(find_defn_body("nonexistent", &items).is_none());
    }

    #[test]
    fn test_verify_skip_no_pair() {
        let pd = ProtocolDef {
            name: "Test".to_string(),
            category: "String".to_string(),
            contract: None,
            cast_edges: vec![make_edge(CastDirection::CastTo, "String", "UTF8")],
            cross_ops: vec![],
            span: None,
        };
        let items: Vec<TopLevel> = vec![];
        assert!(verify_protocol_roundtrip(&pd, &items).is_ok());
    }
}