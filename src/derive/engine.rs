// ── Phase C — Full Enumerative Synthesis Engine ─────────────────────────
// 2026-07-12: Phase 6.1 — Original depth-bounded enumerative search.
// 2026-07-28: Phase C — Rewritten: type-aware generation, interpreter-backed
// evaluation, Occam cost model with constant burden, cost-pruned search.
// Flat code: each function max 2 levels of nesting.

use crate::ast::{BinaryOpKind, DerivationExample, Expr, UnaryOpKind};
use crate::derive::SynthesizeError;
use crate::interpreter::values_within_tolerance;
use crate::interpreter::Value;

// ── C.2 — Occam Cost Model ────────────────────────────────────────────

/// 2026-07-28: Phase C.2 — Cost model for enumerative synthesis.
/// Occam's razor: simpler programs (lower cost) are preferred.
/// Constants beyond trivial values incur a per-bit burden.
#[derive(Debug, Clone)]
pub struct CostModel {
    pub constant: u64,
    pub variable: u64,
    pub unary_op: u64,
    pub binary_op: u64,
    pub branch: u64,
    pub constant_burden: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        CostModel {
            constant: 1,
            variable: 1,
            unary_op: 2,
            binary_op: 3,
            branch: 5,
            constant_burden: 0.1,
        }
    }
}

impl CostModel {
    #[allow(dead_code)]
    pub fn cost_of_constant(&self, val: &Value) -> u64 {
        let bits = match val {
            Value::Int(n) => n.checked_abs().map(|a| 64 - a.leading_zeros()).unwrap_or(0) as u64,
            Value::Float(_) => 64,
            Value::Bits(bytes) if bytes.len() <= 1 => 1,
            _ => 8,
        };
        self.constant + (bits as f64 * self.constant_burden).ceil() as u64
    }

    pub fn cost_of_expr(&self, expr: &Expr) -> u64 {
        match expr {
            Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_) => self.constant,
            Expr::Identifier(_) => self.variable,
            Expr::UnaryOp(_, inner) => self.unary_op + self.cost_of_expr(inner),
            Expr::BinaryOp(_, lhs, rhs) => self.binary_op + self.cost_of_expr(lhs) + self.cost_of_expr(rhs),
            Expr::If(cond, then_, else_) => {
                let else_cost = else_.as_ref().map(|e| self.cost_of_expr(e)).unwrap_or(0);
                self.branch + self.cost_of_expr(cond) + self.cost_of_expr(then_) + else_cost
            }
            _ => 10,
        }
    }
}

// ── C.1 — Synthesis Evaluation Context ─────────────────────────────────

/// 2026-07-28: Phase C.1 — Lightweight evaluation context for synthesized
/// expressions during enumerative search. Maps variable names to Values.
#[derive(Debug, Clone)]
pub struct SynthesisEvalContext {
    pub bindings: std::collections::HashMap<String, Value>,
}

impl SynthesisEvalContext {
    pub fn new() -> Self {
        SynthesisEvalContext {
            bindings: std::collections::HashMap::new(),
        }
    }

    pub fn bind(&mut self, name: &str, val: Value) {
        self.bindings.insert(name.to_string(), val);
    }
}

/// 2026-07-28: Phase C.1 — Error type for synthesis expression evaluation.
#[derive(Debug, Clone)]
pub enum SynthesisEvalError {
    UndefinedVariable(String),
    DivisionByZero,
    TypeMismatch(String),
}

/// Evaluate a synthesized expression against concrete input values.
/// 2026-07-28: Phase C.1 — replaces the old matches_pattern stub.
pub fn evaluate_synthesized(
    expr: &Expr,
    ctx: &mut SynthesisEvalContext,
) -> Result<Value, SynthesisEvalError> {
    match expr {
        Expr::Decimal(n) => Ok(Value::Int(*n)),
        Expr::Float(f) => Ok(Value::Float(*f)),
        Expr::Bool(b) => Ok(Value::Bits(vec![if *b { 1u8 } else { 0u8 }])),
        Expr::Identifier(name) => ctx
            .bindings
            .get(name)
            .cloned()
            .ok_or(SynthesisEvalError::UndefinedVariable(name.clone())),
        Expr::UnaryOp(kind, inner) => {
            let val = evaluate_synthesized(inner, ctx)?;
            eval_unary(kind, val)
        }
        Expr::BinaryOp(kind, lhs, rhs) => {
            let l = evaluate_synthesized(lhs, ctx)?;
            let r = evaluate_synthesized(rhs, ctx)?;
            eval_binary(kind, l, r)
        }
        Expr::If(cond, then_, else_) => {
            let c = evaluate_synthesized(cond, ctx)?;
            let cbool = match c {
                Value::Bits(ref bytes) => bytes.iter().any(|b| *b != 0),
                Value::Int(n) => n != 0,
                Value::Float(f) => f != 0.0,
                _ => return Err(SynthesisEvalError::TypeMismatch("non-boolean condition".into())),
            };
            if cbool {
                evaluate_synthesized(then_, ctx)
            } else if let Some(e) = else_ {
                evaluate_synthesized(e, ctx)
            } else {
                Ok(Value::Bits(vec![0]))
            }
        }
        _ => Err(SynthesisEvalError::TypeMismatch(format!(
            "unsupported expression in synthesis: {:?}",
            expr
        ))),
    }
}

fn eval_unary(kind: &UnaryOpKind, val: Value) -> Result<Value, SynthesisEvalError> {
    match kind {
        UnaryOpKind::Neg => match val {
            Value::Int(n) => Ok(Value::Int(-n)),
            Value::Float(f) => Ok(Value::Float(-f)),
            _ => Err(SynthesisEvalError::TypeMismatch("negation requires numeric type".into())),
        },
        UnaryOpKind::Not => match val {
            Value::Bits(ref bytes) => Ok(Value::Bits(vec![if bytes.iter().all(|b| *b == 0) { 1 } else { 0 }])),
            Value::Int(n) => Ok(Value::Int(if n == 0 { 1 } else { 0 })),
            _ => Err(SynthesisEvalError::TypeMismatch("not requires boolean type".into())),
        },
        UnaryOpKind::BitNot => {
            Err(SynthesisEvalError::TypeMismatch("bitwise not not supported in synthesis".into()))
        }
    }
}

fn eval_binary(kind: &BinaryOpKind, l: Value, r: Value) -> Result<Value, SynthesisEvalError> {
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => eval_binary_int(kind, a, b),
        (Value::Float(a), Value::Float(b)) => eval_binary_float(kind, a, b),
        (Value::Int(a), Value::Float(b)) => eval_binary_float(kind, a as f64, b),
        (Value::Float(a), Value::Int(b)) => eval_binary_float(kind, a, b as f64),
        _ => match kind {
            BinaryOpKind::Eq => Ok(Value::Bits(vec![0])),
            BinaryOpKind::Neq => Ok(Value::Bits(vec![1])),
            _ => Err(SynthesisEvalError::TypeMismatch("incompatible types for binary op".into())),
        },
    }
}

fn eval_binary_int(kind: &BinaryOpKind, a: i64, b: i64) -> Result<Value, SynthesisEvalError> {
    match kind {
        BinaryOpKind::Add => Ok(Value::Int(a.wrapping_add(b))),
        BinaryOpKind::Sub => Ok(Value::Int(a.wrapping_sub(b))),
        BinaryOpKind::Mul => Ok(Value::Int(a.wrapping_mul(b))),
        BinaryOpKind::Div => {
            if b == 0 {
                return Err(SynthesisEvalError::DivisionByZero);
            }
            Ok(Value::Int(a.wrapping_div(b)))
        }
        BinaryOpKind::Mod => {
            if b == 0 {
                return Err(SynthesisEvalError::DivisionByZero);
            }
            Ok(Value::Int(a % b))
        }
        BinaryOpKind::Eq => Ok(Value::Bits(vec![if a == b { 1 } else { 0 }])),
        BinaryOpKind::Neq => Ok(Value::Bits(vec![if a != b { 1 } else { 0 }])),
        BinaryOpKind::Lt => Ok(Value::Bits(vec![if a < b { 1 } else { 0 }])),
        BinaryOpKind::Gt => Ok(Value::Bits(vec![if a > b { 1 } else { 0 }])),
        BinaryOpKind::Le => Ok(Value::Bits(vec![if a <= b { 1 } else { 0 }])),
        BinaryOpKind::Ge => Ok(Value::Bits(vec![if a >= b { 1 } else { 0 }])),
        BinaryOpKind::And => Ok(Value::Int(if a != 0 && b != 0 { 1 } else { 0 })),
        BinaryOpKind::Or => Ok(Value::Int(if a != 0 || b != 0 { 1 } else { 0 })),
        BinaryOpKind::BitXor => Ok(Value::Int(a ^ b)),
        BinaryOpKind::Shl => Ok(Value::Int(a.wrapping_shl(b as u32))),
        BinaryOpKind::Shr => Ok(Value::Int(a.wrapping_shr(b as u32))),
        BinaryOpKind::BitAnd => Ok(Value::Int(a & b)),
        BinaryOpKind::BitOr => Ok(Value::Int(a | b)),
        BinaryOpKind::Concat => Err(SynthesisEvalError::TypeMismatch("concat not supported on int".into())),
    }
}

fn eval_binary_float(kind: &BinaryOpKind, a: f64, b: f64) -> Result<Value, SynthesisEvalError> {
    match kind {
        BinaryOpKind::Add => Ok(Value::Float(a + b)),
        BinaryOpKind::Sub => Ok(Value::Float(a - b)),
        BinaryOpKind::Mul => Ok(Value::Float(a * b)),
        BinaryOpKind::Div => Ok(Value::Float(a / b)),
        BinaryOpKind::Mod => Ok(Value::Float(a % b)),
        BinaryOpKind::Eq => Ok(Value::Bits(vec![if a == b { 1 } else { 0 }])),
        BinaryOpKind::Neq => Ok(Value::Bits(vec![if a != b { 1 } else { 0 }])),
        BinaryOpKind::Lt => Ok(Value::Bits(vec![if a < b { 1 } else { 0 }])),
        BinaryOpKind::Gt => Ok(Value::Bits(vec![if a > b { 1 } else { 0 }])),
        BinaryOpKind::Le => Ok(Value::Bits(vec![if a <= b { 1 } else { 0 }])),
        BinaryOpKind::Ge => Ok(Value::Bits(vec![if a >= b { 1 } else { 0 }])),
        BinaryOpKind::And | BinaryOpKind::Or
        | BinaryOpKind::BitAnd | BinaryOpKind::BitOr | BinaryOpKind::BitXor
        | BinaryOpKind::Shl | BinaryOpKind::Shr | BinaryOpKind::Concat => {
            Err(SynthesisEvalError::TypeMismatch("integer-only binary op on float".into()))
        }
    }
}

// ── C.0 — Type-Aware Expression Generation ────────────────────────────

/// 2026-07-28: Phase C.0 — Operator type compatibility for synthesis.
/// Returns the result type if the operator is valid for the given operand types.
fn op_result_type(op: &BinaryOpKind, lhs_ty: &str, rhs_ty: &str) -> Option<&'static str> {
    if lhs_ty != rhs_ty {
        return None;
    }
    match op {
        BinaryOpKind::Add | BinaryOpKind::Sub | BinaryOpKind::Mul
        | BinaryOpKind::Div | BinaryOpKind::Mod => {
            if lhs_ty == "Int" || lhs_ty == "Float" {
                if lhs_ty == "Int" { Some("Int") } else { Some("Float") }
            } else {
                None
            }
        }
        BinaryOpKind::Eq | BinaryOpKind::Neq
        | BinaryOpKind::Lt | BinaryOpKind::Gt
        | BinaryOpKind::Le | BinaryOpKind::Ge => Some("Bool"),
        BinaryOpKind::And | BinaryOpKind::Or | BinaryOpKind::BitXor => {
            if lhs_ty == "Bool" {
                Some("Bool")
            } else {
                None
            }
        }
        BinaryOpKind::Shl | BinaryOpKind::Shr => {
            if lhs_ty == "Int" {
                Some("Int")
            } else {
                None
            }
        }
        BinaryOpKind::BitAnd | BinaryOpKind::BitOr | BinaryOpKind::Concat => None,
        _ => None,
    }
}

/// 2026-07-28: Phase C.0 — Type-aware expression generator.
/// Generates all well-typed expressions up to a given depth.
fn generate_typed_expressions(
    param_names: &[String],
    param_types: &[String],
    ret_type: &str,
    depth: u8,
) -> Vec<Expr> {
    if depth == 0 || param_names.len() != param_types.len() {
        return Vec::new();
    }

    let mut result = Vec::new();

    // Constants at any depth (but prefer shallow)
    push_typed_constants(ret_type, &mut result);

    // Variables that match the return type
    for (name, ty) in param_names.iter().zip(param_types.iter()) {
        if ty == ret_type {
            result.push(Expr::Identifier(name.clone()));
        }
    }

    if depth <= 1 {
        return result;
    }

    let sub = generate_typed_expressions(param_names, param_types, ret_type, depth - 1);

    // Unary ops
    if ret_type == "Int" || ret_type == "Float" {
        for e in &sub {
            if let Some(neg_ty) = unary_result_type(&UnaryOpKind::Neg, ret_type) {
                if neg_ty == ret_type {
                    result.push(Expr::UnaryOp(UnaryOpKind::Neg, Box::new(e.clone())));
                }
            }
        }
    }
    if ret_type == "Bool" {
        for e in &sub {
            result.push(Expr::UnaryOp(UnaryOpKind::Not, Box::new(e.clone())));
        }
    }

    // Binary ops
    for lhs_ty_str in &["Int", "Float", "Bool"] {
        let lhs_candidates = generate_typed_expressions(param_names, param_types, lhs_ty_str, depth - 1);
        if lhs_candidates.is_empty() {
            continue;
        }
        let rhs_candidates = generate_typed_expressions(param_names, param_types, lhs_ty_str, depth - 1);
        if rhs_candidates.is_empty() {
            continue;
        }
        for op in &[
            BinaryOpKind::Add, BinaryOpKind::Sub, BinaryOpKind::Mul,
            BinaryOpKind::Div, BinaryOpKind::Mod, BinaryOpKind::Eq,
            BinaryOpKind::Neq, BinaryOpKind::Lt, BinaryOpKind::Gt,
            BinaryOpKind::Le, BinaryOpKind::Ge,
        ] {
            if let Some(ty) = op_result_type(op, lhs_ty_str, lhs_ty_str) {
                if ty != ret_type {
                    continue;
                }
                for lhs_expr in &lhs_candidates {
                    for rhs_expr in &rhs_candidates {
                        result.push(Expr::BinaryOp(*op, Box::new(lhs_expr.clone()), Box::new(rhs_expr.clone())));
                    }
                }
            }
        }
    }

    // Bool-specific: logical ops only when ret_type == Bool
    if ret_type == "Bool" {
        let bool_candidates = generate_typed_expressions(param_names, param_types, "Bool", depth - 1);
        if !bool_candidates.is_empty() {
            for op in &[BinaryOpKind::And, BinaryOpKind::Or, BinaryOpKind::BitXor] {
                if op_result_type(op, "Bool", "Bool") == Some("Bool") {
                    for lhs_expr in &bool_candidates {
                        for rhs_expr in &bool_candidates {
                            result.push(Expr::BinaryOp(*op, Box::new(lhs_expr.clone()), Box::new(rhs_expr.clone())));
                        }
                    }
                }
            }
        }
    }

    result
}

fn unary_result_type(kind: &UnaryOpKind, ty: &str) -> Option<&'static str> {
    match kind {
        UnaryOpKind::Neg => {
            if ty == "Int" || ty == "Float" {
                Some("Int")
            } else {
                None
            }
        }
        UnaryOpKind::Not => {
            if ty == "Bool" {
                Some("Bool")
            } else {
                None
            }
        }
        UnaryOpKind::BitNot => None,
    }
}

fn push_typed_constants(ty: &str, result: &mut Vec<Expr>) {
    match ty {
        "Int" => {
            result.push(Expr::Decimal(0));
            result.push(Expr::Decimal(1));
            result.push(Expr::Decimal(-1));
        }
        "Float" => {
            result.push(Expr::Float(0.0));
            result.push(Expr::Float(1.0));
        }
        "Bool" => {
            result.push(Expr::Bool(true));
            result.push(Expr::Bool(false));
        }
        _ => {}
    }
}

// ── C.3 — Depth-Bounded Search with Cost Pruning ──────────────────────

/// 2026-07-28: Phase C.3 — A successfully synthesized program with its cost.
#[derive(Debug, Clone)]
pub struct SynthesizedProgram {
    pub body: Vec<Expr>,
    pub cost: u64,
    pub depth: u8,
}

/// 2026-07-28: Phase C.3 — Enumerate all programs up to max_depth,
/// evaluate against all examples, return lowest-cost match.
pub fn synthesize_enumerative(
    param_types: &[String],
    ret_type: &str,
    param_names: &[String],
    examples: &[DerivationExample],
    cost_model: &CostModel,
    max_depth: u8,
) -> Result<SynthesizedProgram, SynthesizeError> {
    if examples.is_empty() {
        return Err(SynthesizeError::NoExamples("synthesize_enumerative".into()));
    }

    let mut best: Option<SynthesizedProgram> = None;

    for depth in 1..=max_depth {
        let candidates = generate_typed_expressions(param_names, param_types, ret_type, depth);
        if candidates.is_empty() {
            continue;
        }

        for candidate in &candidates {
            let cost = cost_model.cost_of_expr(candidate);
            if best.as_ref().map_or(false, |b| cost >= b.cost) {
                continue;
            }
            let all_match = examples.iter().all(|ex| {
                let input_values = example_inputs_to_values(ex, param_names);
                let mut ctx = SynthesisEvalContext::new();
                for (name, val) in param_names.iter().zip(input_values.iter()) {
                    ctx.bind(name, val.clone());
                }
                let result = evaluate_synthesized(candidate, &mut ctx);
                let expected_input_ctx = || -> SynthesisEvalContext {
                    let mut c = SynthesisEvalContext::new();
                    c
                };
                let expected = evaluate_synthesized(&ex.output, &mut expected_input_ctx());
                match (result, expected) {
                    (Ok(actual), Ok(exp)) => {
                        let tol = ex.tolerance.unwrap_or(0.0);
                        values_within_tolerance(&actual, &exp, tol)
                    }
                    _ => false,
                }
            });
            if all_match {
                if best.as_ref().map_or(true, |b| cost < b.cost) {
                    best = Some(SynthesizedProgram {
                        body: vec![candidate.clone()],
                        cost,
                        depth,
                    });
                }
            }
        }
    }

    best.ok_or_else(|| SynthesizeError::NoSolution("enumerative search failed".into()))
}

/// Convert the input expressions in a DerivationExample to Values.
/// Uses `expr_to_value` to evaluate each input expression.
fn example_inputs_to_values(ex: &DerivationExample, _param_names: &[String]) -> Vec<Value> {
    ex.inputs.iter().map(|e| expr_to_value(e)).collect()
}

/// Convert an expression to a Value (for constant inputs from examples).
fn expr_to_value(expr: &Expr) -> Value {
    match expr {
        Expr::Decimal(n) => Value::Int(*n),
        Expr::Float(f) => Value::Float(*f),
        Expr::Bool(b) => Value::Bits(vec![if *b { 1 } else { 0 }]),
        Expr::UnaryOp(UnaryOpKind::Neg, inner) => match expr_to_value(inner) {
            Value::Int(n) => Value::Int(-n),
            Value::Float(f) => Value::Float(-f),
            _ => Value::Void,
        },
        Expr::Identifier(_) => Value::Int(0),
        _ => Value::Int(0),
    }
}

// ── Legacy Compatibility ──────────────────────────────────────────────

/// Legacy compatibility wrapper. Infers all parameters as Int, return as Int.
/// 2026-07-12: Original entry point. 2026-07-28: Delegates to synthesize_enumerative.
pub fn enumerative_search(
    name: &str,
    examples: &[DerivationExample],
    max_depth: usize,
) -> Result<Option<Expr>, SynthesizeError> {
    if examples.is_empty() {
        return Err(SynthesizeError::NoExamples(name.to_string()));
    }
    let param_names: Vec<String> = (0..examples[0].inputs.len())
        .map(|i| format!("x{}", i))
        .collect();
    let param_types: Vec<String> = vec!["Int".to_string(); param_names.len()];
    let max_depth_u8 = max_depth.min(8) as u8;
    let cost_model = CostModel::default();

    match synthesize_enumerative(&param_types, "Int", &param_names, examples, &cost_model, max_depth_u8) {
        Ok(prog) => Ok(prog.body.into_iter().next()),
        Err(SynthesizeError::NoSolution(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Span;

    fn dummy_span() -> Span {
        Span::dummy()
    }

    fn example(inputs: Vec<Expr>, output: Expr, tolerance: Option<f64>) -> DerivationExample {
        DerivationExample {
            inputs,
            output: Box::new(output),
            tolerance,
            span: dummy_span(),
        }
    }

    // ── C.0 — Type-aware expression generation ─────────────────────

    #[test]
    fn test_generate_type_int() {
        let names: Vec<String> = vec!["x".into(), "y".into()];
        let types: Vec<String> = vec!["Int".into(), "Int".into()];
        let exprs = generate_typed_expressions(&names, &types, "Int", 2);
        assert!(!exprs.is_empty(), "should generate Int expressions");
        // Should include x, y, 0, 1, -1
        assert!(exprs.iter().any(|e| matches!(e, Expr::Identifier(n) if n == "x")));
        assert!(exprs.iter().any(|e| matches!(e, Expr::Decimal(0))));
        assert!(exprs.iter().any(|e| matches!(e, Expr::Decimal(1))));
        assert!(exprs.iter().any(|e| matches!(e, Expr::Decimal(-1))));
    }

    #[test]
    fn test_generate_type_bool() {
        let names: Vec<String> = vec!["x".into()];
        let types: Vec<String> = vec!["Int".into()];
        let exprs = generate_typed_expressions(&names, &types, "Bool", 2);
        assert!(!exprs.is_empty(), "should generate Bool expressions");
        // Should include true, false, comparisons
        assert!(exprs.iter().any(|e| matches!(e, Expr::Bool(true))));
        assert!(exprs.iter().any(|e| matches!(e, Expr::Bool(false))));
    }

    #[test]
    fn test_generate_type_float() {
        let names: Vec<String> = vec!["x".into()];
        let types: Vec<String> = vec!["Float".into()];
        let exprs = generate_typed_expressions(&names, &types, "Float", 2);
        assert!(!exprs.is_empty(), "should generate Float expressions");
        assert!(exprs.iter().any(|e| matches!(e, Expr::Float(f) if (*f - 0.0).abs() < 1e-10)));
        assert!(exprs.iter().any(|e| matches!(e, Expr::Float(f) if (*f - 1.0).abs() < 1e-10)));
    }

    #[test]
    fn test_generate_type_mismatch_rejected() {
        let names: Vec<String> = vec!["x".into()];
        let types: Vec<String> = vec!["Int".into()];
        // x + true should not be generated (type mismatch)
        let exprs = generate_typed_expressions(&names, &types, "Int", 3);
        for e in &exprs {
            if let Expr::BinaryOp(_, lhs, rhs) = e {
                let lhs_is_bool = matches!(lhs.as_ref(), Expr::Bool(_) | Expr::UnaryOp(UnaryOpKind::Not, _));
                let rhs_is_bool = matches!(rhs.as_ref(), Expr::Bool(_) | Expr::UnaryOp(UnaryOpKind::Not, _));
                assert!(!(lhs_is_bool || rhs_is_bool), "Int-typed expression should not contain Bool operands: {:?}", e);
            }
        }
    }

    #[test]
    fn test_generate_with_typed_constants() {
        let mut result = Vec::new();
        push_typed_constants("Float", &mut result);
        assert!(result.iter().any(|e| matches!(e, Expr::Float(_))));
        assert!(!result.iter().any(|e| matches!(e, Expr::Decimal(_))));
    }

    // ── C.1 — Interpreter-based evaluation ─────────────────────────

    #[test]
    fn test_evaluate_add_expr() {
        let expr = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x".into())),
            Box::new(Expr::Identifier("y".into())),
        );
        let mut ctx = SynthesisEvalContext::new();
        ctx.bind("x", Value::Int(2));
        ctx.bind("y", Value::Int(3));
        let result = evaluate_synthesized(&expr, &mut ctx).unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_evaluate_cond_expr() {
        let expr = Expr::If(
            Box::new(Expr::BinaryOp(
                BinaryOpKind::Gt,
                Box::new(Expr::Identifier("x".into())),
                Box::new(Expr::Decimal(0)),
            )),
            Box::new(Expr::Identifier("x".into())),
            Some(Box::new(Expr::UnaryOp(UnaryOpKind::Neg, Box::new(Expr::Identifier("x".into()))))),
        );
        let mut ctx = SynthesisEvalContext::new();
        ctx.bind("x", Value::Int(-3));
        let result = evaluate_synthesized(&expr, &mut ctx).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_evaluate_nested_expr() {
        let expr = Expr::BinaryOp(
            BinaryOpKind::Mul,
            Box::new(Expr::BinaryOp(
                BinaryOpKind::Add,
                Box::new(Expr::Identifier("x".into())),
                Box::new(Expr::Identifier("y".into())),
            )),
            Box::new(Expr::Identifier("z".into())),
        );
        let mut ctx = SynthesisEvalContext::new();
        ctx.bind("x", Value::Int(2));
        ctx.bind("y", Value::Int(3));
        ctx.bind("z", Value::Int(4));
        let result = evaluate_synthesized(&expr, &mut ctx).unwrap();
        assert_eq!(result, Value::Int(20));
    }

    #[test]
    fn test_evaluate_undefined_var() {
        let expr = Expr::Identifier("undefined".into());
        let mut ctx = SynthesisEvalContext::new();
        let result = evaluate_synthesized(&expr, &mut ctx);
        assert!(matches!(result, Err(SynthesisEvalError::UndefinedVariable(_))));
    }

    // ── C.2 — Cost Model ───────────────────────────────────────────

    #[test]
    fn test_cost_constant_small() {
        let model = CostModel::default();
        let small = model.cost_of_expr(&Expr::Decimal(0));
        let large = model.cost_of_expr(&Expr::Decimal(65536));
        assert!(small <= large, "small constant should cost <= large constant");
    }

    #[test]
    fn test_cost_constant_defaults() {
        let model = CostModel::default();
        assert_eq!(model.constant, 1);
        assert_eq!(model.variable, 1);
        assert_eq!(model.unary_op, 2);
        assert_eq!(model.binary_op, 3);
    }

    #[test]
    fn test_cost_binary_op_greater_than_variable() {
        let model = CostModel::default();
        let var_cost = model.cost_of_expr(&Expr::Identifier("x".into()));
        let binop = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x".into())),
            Box::new(Expr::Identifier("y".into())),
        );
        let binop_cost = model.cost_of_expr(&binop);
        assert!(binop_cost > var_cost, "x + y should cost more than x");
    }

    #[test]
    fn test_cost_prefers_simple() {
        let model = CostModel::default();
        let simple = Expr::Identifier("x".into());
        let complex = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x".into())),
            Box::new(Expr::Decimal(0)),
        );
        assert!(
            model.cost_of_expr(&simple) < model.cost_of_expr(&complex),
            "simpler expression should have lower cost"
        );
    }

    // ── C.3 — Depth-bounded search ─────────────────────────────────

    #[test]
    fn test_enumerative_simple_add() {
        let examples = vec![
            example(vec![Expr::Decimal(2), Expr::Decimal(3)], Expr::Decimal(5), None),
            example(vec![Expr::Decimal(0), Expr::Decimal(5)], Expr::Decimal(5), None),
        ];
        let names: Vec<String> = vec!["x".into(), "y".into()];
        let types: Vec<String> = vec!["Int".into(), "Int".into()];
        let model = CostModel::default();
        let result = synthesize_enumerative(&types, "Int", &names, &examples, &model, 3).unwrap();
        assert_eq!(result.cost, model.cost_of_expr(&Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x".into())),
            Box::new(Expr::Identifier("y".into())),
        )));
    }

    #[test]
    fn test_enumerative_identity() {
        let examples = vec![
            example(vec![Expr::Decimal(0)], Expr::Decimal(0), None),
            example(vec![Expr::Decimal(42)], Expr::Decimal(42), None),
        ];
        let names: Vec<String> = vec!["x".into()];
        let types: Vec<String> = vec!["Int".into()];
        let model = CostModel::default();
        let result = synthesize_enumerative(&types, "Int", &names, &examples, &model, 2).unwrap();
        assert_eq!(
            result.cost,
            model.cost_of_expr(&Expr::Identifier("x".into())),
            "identity should be cheaper than adding 0"
        );
    }

    #[test]
    fn test_enumerative_constant() {
        let examples = vec![
            example(vec![Expr::Decimal(0)], Expr::Decimal(1), None),
            example(vec![Expr::Decimal(100)], Expr::Decimal(1), None),
        ];
        let names: Vec<String> = vec!["x".into()];
        let types: Vec<String> = vec!["Int".into()];
        let model = CostModel::default();
        // The constant 1 (in the default pool) should be found
        let result = synthesize_enumerative(&types, "Int", &names, &examples, &model, 2);
        assert!(result.is_ok(), "should find constant 1: {:?}", result.err());
    }

    #[test]
    fn test_enumerative_no_solution() {
        let examples = vec![
            example(vec![Expr::Decimal(1)], Expr::Decimal(999), None),
        ];
        let names: Vec<String> = vec!["x".into()];
        let types: Vec<String> = vec!["Int".into()];
        let model = CostModel::default();
        let result = synthesize_enumerative(&types, "Int", &names, &examples, &model, 2);
        assert!(result.is_err(), "999 should not be found at depth 2");
    }

    #[test]
    fn test_enumerative_with_tolerance() {
        let examples = vec![
            example(
                vec![Expr::Float(1.0), Expr::Float(2.0)],
                Expr::Float(3.0),
                Some(0.01),
            ),
        ];
        let names: Vec<String> = vec!["x".into(), "y".into()];
        let types: Vec<String> = vec!["Float".into(), "Float".into()];
        let model = CostModel::default();
        let result = synthesize_enumerative(&types, "Float", &names, &examples, &model, 3);
        assert!(result.is_ok(), "should find x + y with tolerance: {:?}", result.err());
    }

    #[test]
    fn test_enumerative_empty_examples() {
        let names: Vec<String> = vec!["x".into()];
        let types: Vec<String> = vec!["Int".into()];
        let model = CostModel::default();
        let result = synthesize_enumerative(&types, "Int", &names, &[], &model, 2);
        assert!(matches!(result, Err(SynthesizeError::NoExamples(_))));
    }

    #[test]
    fn test_enumerative_cost_pruning() {
        // With many examples, expensive candidates should be skipped
        // if a cheap one already matches.
        let examples = vec![
            example(vec![Expr::Decimal(0)], Expr::Decimal(0), None),
            example(vec![Expr::Decimal(42)], Expr::Decimal(42), None),
        ];
        let names: Vec<String> = vec!["x".into()];
        let types: Vec<String> = vec!["Int".into()];
        let model = CostModel::default();
        // identity (x) should be found before x + 0
        let result = synthesize_enumerative(&types, "Int", &names, &examples, &model, 3).unwrap();
        assert!(
            result.body.iter().any(|e| matches!(e, Expr::Identifier(_))),
            "should prefer identity over x + 0: {:?}",
            result.body
        );
    }

    // ── Legacy compatibility ───────────────────────────────────────

    #[test]
    fn test_enumerative_search_identity() {
        let examples = vec![
            example(vec![Expr::Decimal(42)], Expr::Decimal(42), None),
        ];
        let result = enumerative_search("id", &examples, 3).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_empty_examples() {
        let result = enumerative_search("f", &[], 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_solution() {
        let examples = vec![
            example(vec![Expr::Decimal(1)], Expr::Decimal(999), None),
        ];
        let result = enumerative_search("f", &examples, 2).unwrap();
        assert!(result.is_none());
    }
}
