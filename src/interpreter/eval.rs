// ── Expression Evaluation ──────────────────────────────────────────────
// 2026-07-12: Phase 3.1 — Flat dispatch, one arm per Expr variant.
// Call with # suffix dispatches to execute_intrinsic().
// Each complex arm is extracted into a named helper for flat code.

use crate::ast::*;
use crate::errors::RuntimeError;
use crate::interpreter::{
    bool_to_bits, execute_intrinsic, f64_to_bits, i64_to_bits, zero_bits, Value, VirtualHeap,
};
use std::collections::HashMap;

/// Evaluate an expression to a Value.
/// Flat dispatch: one match arm per Expr variant.
pub fn eval_expr(
    expr: &Expr,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match expr {
        // ── Literals ────────────────────────────────────────────
        Expr::Decimal(n) => Ok(i64_to_bits(*n)),
        Expr::Float(f) => Ok(f64_to_bits(*f)),
        Expr::Bool(b) => Ok(bool_to_bits(*b)),
        Expr::Quoted(bytes) => Ok(Value::bits(bytes.clone())),

        // ── References ──────────────────────────────────────────
        Expr::Identifier(name) => bindings
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedVariable { name: name.clone() }),

        // ── Calls ───────────────────────────────────────────────
        Expr::Call(name, args) => eval_call(name, args, heap, bindings),

        // ── Binary operators ─────────────────────────────────────
        Expr::BinaryOp(kind, lhs, rhs) => eval_binary_op(kind, lhs, rhs, heap, bindings),

        // ── Unary operators ──────────────────────────────────────
        Expr::UnaryOp(kind, expr) => eval_unary_op(kind, expr, heap, bindings),

        // ── Block ────────────────────────────────────────────────
        Expr::Block(stmts) => eval_block(stmts, heap, bindings),

        // ── If ───────────────────────────────────────────────────
        Expr::If(cond, then, else_) => eval_if(cond, then, else_, heap, bindings),

        // ── Tuple ────────────────────────────────────────────────
        Expr::Tuple(exprs) => {
            let values: Result<Vec<Value>, _> =
                exprs.iter().map(|e| eval_expr(e, heap, bindings)).collect();
            // Return first element for single-element tuples (simplified)
            Ok(values?.into_iter().next().unwrap_or(Value::Void))
        }

        // ── List ─────────────────────────────────────────────────
        Expr::List(exprs) => {
            let _values: Result<Vec<Value>, _> =
                exprs.iter().map(|e| eval_expr(e, heap, bindings)).collect();
            Ok(zero_bits(8)) // placeholder
        }

        // ── Field access ─────────────────────────────────────────
        Expr::Field(obj, _name) => eval_expr(obj, heap, bindings),

        // ── Index ────────────────────────────────────────────────
        Expr::Index(obj, index) => {
            let _obj = eval_expr(obj, heap, bindings)?;
            let _idx = eval_expr(index, heap, bindings)?;
            Ok(zero_bits(8)) // placeholder
        }

        // ── Cast ─────────────────────────────────────────────────
        Expr::Cast(expr, _ty) => eval_expr(expr, heap, bindings),

        // ── IsType ───────────────────────────────────────────────
        Expr::IsType(_, _) => Ok(bool_to_bits(true)),

        // ── Within ───────────────────────────────────────────────
        Expr::Within(expr, _scope) => eval_expr(expr, heap, bindings),

        // ── Match ────────────────────────────────────────────────
        Expr::Match(_, arms) => {
            if let Some(first) = arms.first() {
                eval_expr(&first.body, heap, bindings)
            } else {
                Ok(Value::Void)
            }
        }

        // ── Lambda ───────────────────────────────────────────────
        Expr::Lambda(_, _) => Ok(Value::Void),

        // ── Derivation block ─────────────────────────────────────
        Expr::DerivationBlock(_) => Ok(Value::Void),

        // ── Dereference ──────────────────────────────────────────
        Expr::Deref(inner) => eval_expr(inner, heap, bindings),
        // ── Address-of ───────────────────────────────────────────
        Expr::AddrOf(inner) => eval_expr(inner, heap, bindings),

        // ── Property get ─────────────────────────────────────────
        Expr::PropertyGet(_) => Ok(Value::Void),

        // ── Formatting annotation ────────────────────────────────
        Expr::FormattingAnnotation(_) => Ok(Value::Void),
    }
}

/// Evaluate a function/intrinsic call.
fn eval_call(
    name: &str,
    args: &[Expr],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let evaluated: Vec<Value> = args
        .iter()
        .map(|a| eval_expr(a, heap, bindings))
        .collect::<Result<Vec<_>, _>>()?;

    if name.ends_with('#') {
        execute_intrinsic(name, &evaluated, heap)
    } else {
        // User function call (simplified: looks up binding)
        bindings
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedVariable { name: name.into() })
    }
}

/// Evaluate a binary operation.
fn eval_binary_op(
    kind: &BinaryOpKind,
    lhs: &Expr,
    rhs: &Expr,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let lv = eval_expr(lhs, heap, bindings)?;
    let rv = eval_expr(rhs, heap, bindings)?;

    match kind {
        BinaryOpKind::Add => {
            // Simplified: try arithmetic, fall back to intrinsic
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(i64_to_bits(a.wrapping_add(b))),
                _ => execute_intrinsic("Add#", &[lv, rv], heap),
            }
        }
        BinaryOpKind::Sub => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(i64_to_bits(a.wrapping_sub(b))),
                _ => execute_intrinsic("Sub#", &[lv, rv], heap),
            }
        }
        BinaryOpKind::Mul => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(i64_to_bits(a.wrapping_mul(b))),
                _ => execute_intrinsic("Mul#", &[lv, rv], heap),
            }
        }
        BinaryOpKind::Eq => Ok(bool_to_bits(lv.as_i64() == rv.as_i64())),
        BinaryOpKind::Lt => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(bool_to_bits(a < b)),
                _ => Ok(bool_to_bits(false)),
            }
        }
        BinaryOpKind::Gt => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(bool_to_bits(a > b)),
                _ => Ok(bool_to_bits(false)),
            }
        }
        _ => {
            // Pass through unknown operators as intrinsic calls
            let op_name = format!("{:?}#", kind);
            execute_intrinsic(&op_name, &[lv, rv], heap)
        }
    }
}

/// Evaluate a unary operation.
fn eval_unary_op(
    kind: &UnaryOpKind,
    expr: &Expr,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let val = eval_expr(expr, heap, bindings)?;
    match kind {
        UnaryOpKind::Neg => {
            let n = val.as_i64().unwrap_or(0);
            Ok(i64_to_bits(n.wrapping_neg()))
        }
        UnaryOpKind::Not => {
            let b = val.is_true();
            Ok(bool_to_bits(!b))
        }
        UnaryOpKind::BitNot => {
            let n = val.as_i64().unwrap_or(0);
            Ok(i64_to_bits(!n))
        }
    }
}

/// Evaluate a block of statements.
fn eval_block(
    stmts: &[Statement],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let mut result = Value::Void;
    for stmt in stmts {
        result = eval_statement(stmt, heap, bindings)?;
    }
    Ok(result)
}

/// Evaluate an if expression.
fn eval_if(
    cond: &Expr,
    then: &Expr,
    else_: &Option<Box<Expr>>,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let cv = eval_expr(cond, heap, bindings)?;
    if cv.is_true() {
        eval_expr(then, heap, bindings)
    } else if let Some(else_) = else_ {
        eval_expr(else_, heap, bindings)
    } else {
        Ok(Value::Void)
    }
}

/// Evaluate a statement.
pub fn eval_statement(
    stmt: &Statement,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match stmt {
        Statement::Let { name, expr, .. } => {
            if let Some(expr) = expr {
                let val = eval_expr(expr, heap, bindings)?;
                bindings.insert(name.clone(), val);
            }
            Ok(Value::Void)
        }
        Statement::Assign(lhs, rhs) => {
            let val = eval_expr(rhs, heap, bindings)?;
            if let Expr::Identifier(name) = lhs {
                bindings.insert(name.clone(), val);
            }
            Ok(Value::Void)
        }
        Statement::Expression(expr) => eval_expr(expr, heap, bindings),
        Statement::Term(val) => {
            if let Some(val) = val {
                eval_expr(val, heap, bindings)
            } else {
                Ok(Value::Void)
            }
        }
        Statement::Guarded(cond, body) => {
            let cv = eval_expr(cond, heap, bindings)?;
            if cv.is_true() {
                let mut result = Value::Void;
                for stmt in body {
                    result = eval_statement(stmt, heap, bindings)?;
                }
                Ok(result)
            } else {
                Ok(Value::Void)
            }
        }
        Statement::If(cond, then, else_) => {
            let cv = eval_expr(cond, heap, bindings)?;
            if cv.is_true() {
                let mut result = Value::Void;
                for stmt in then {
                    result = eval_statement(stmt, heap, bindings)?;
                }
                Ok(result)
            } else {
                let mut result = Value::Void;
                for stmt in else_ {
                    result = eval_statement(stmt, heap, bindings)?;
                }
                Ok(result)
            }
        }
        Statement::Block(stmts) => {
            let mut result = Value::Void;
            for stmt in stmts {
                result = eval_statement(stmt, heap, bindings)?;
            }
            Ok(result)
        }
        Statement::TermBang(val) => {
            if let Some(val) = val {
                eval_expr(val, heap, bindings)
            } else {
                Ok(Value::Void)
            }
        }
        Statement::Return(val) => {
            if let Some(val) = val {
                eval_expr(val, heap, bindings)
            } else {
                Ok(Value::Void)
            }
        }
        Statement::Escape(_) => Ok(Value::Void),
        Statement::MetadataAssignment(_, _) => Ok(Value::Void),
        Statement::InlineAsm { .. } => Ok(Value::Void),
        Statement::SyncBlock(body) => {
            let mut result = Value::Void;
            for stmt in body {
                result = eval_statement(stmt, heap, bindings)?;
            }
            Ok(result)
        }
        Statement::Foreach { .. } => Ok(Value::Void),
        Statement::TrgBinding { .. } => Ok(Value::Void),
    }
}
