// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

/// Symbolic Executor for Assignment Tracking and Postcondition Verification
///
/// This module provides Level 2 symbolic execution capabilities:
/// - Tracks variable assignments symbolically (literals, identifiers, arithmetic)
/// - Handles prior-state comparisons with @ operator
/// - Evaluates postconditions against symbolic state
/// - Enumerates execution paths through guard blocks
///
/// Coverage: ~90% of real Brief contracts
use crate::ast::{Expr, Statement};
use std::collections::HashMap;

/// Symbolic representation of a value
/// Represents what a variable could be given current state
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicValue {
    /// Literal constant (5, true, "hello", etc.)
    Literal(i64, String), // (value, type_hint: "int", "bool", "float", etc.)

    /// Reference to another variable
    Identifier(String),

    /// Prior state value (before execution)
    Previous(String),

    /// Binary operation: op(left, right)
    Binary(String, Box<SymbolicValue>, Box<SymbolicValue>), // (op: "+", "-", "*", etc.)

    /// Unknown value (can't track)
    Unknown,
}

impl SymbolicValue {
    /// Create a symbolic value from an integer literal
    pub fn int_literal(n: i64) -> Self {
        SymbolicValue::Literal(n, "int".to_string())
    }

    /// Create a symbolic value from a boolean literal
    pub fn bool_literal(b: bool) -> Self {
        SymbolicValue::Literal(if b { 1 } else { 0 }, "bool".to_string())
    }

    /// Check if this value is definitely true (for boolean simplification)
    pub fn is_definitely_true(&self) -> bool {
        matches!(self, SymbolicValue::Literal(1, _))
    }

    /// Check if this value is definitely false
    pub fn is_definitely_false(&self) -> bool {
        matches!(self, SymbolicValue::Literal(0, _))
    }
}

/// State during symbolic execution of a path
#[derive(Debug, Clone)]
pub struct SymbolicState {
    /// Mapping of variable -> its symbolic value
    pub assignments: HashMap<String, SymbolicValue>,

    /// Constraints (guards) from this path
    pub path_constraints: Vec<Expr>,
}

impl SymbolicState {
    /// Create new state from precondition
    pub fn new(precondition: &Expr) -> Self {
        SymbolicState {
            assignments: HashMap::new(),
            path_constraints: vec![precondition.clone()],
        }
    }

    /// Create an empty state (for initialization)
    pub fn empty() -> Self {
        SymbolicState {
            assignments: HashMap::new(),
            path_constraints: Vec::new(),
        }
    }

    /// Record an assignment
    pub fn assign(&mut self, target: &str, value_expr: &Expr) {
        let symbolic_val = eval_symbolic(value_expr, self);
        self.assignments.insert(target.to_string(), symbolic_val);
    }

    /// Add a guard constraint (from [condition] guard block)
    pub fn add_constraint(&mut self, condition: &Expr, taken: bool) {
        if taken {
            self.path_constraints.push(condition.clone());
        } else {
            // Guard not taken; add negation
            self.path_constraints
                .push(Expr::Not(Box::new(condition.clone())));
        }
    }

    /// Get the symbolic value for a variable, or None if unknown
    pub fn get_value(&self, name: &str) -> Option<SymbolicValue> {
        self.assignments.get(name).cloned()
    }
}

/// Evaluate an expression to a symbolic value
/// Returns Unknown if expression is too complex to track
pub fn eval_symbolic(expr: &Expr, state: &SymbolicState) -> SymbolicValue {
    match expr {
        // Literal values
        Expr::Integer(n) => SymbolicValue::Literal(*n, "int".to_string()),
        Expr::Float(_) => SymbolicValue::Unknown, // Float support via Unknown
        Expr::Bool(b) => SymbolicValue::bool_literal(*b),
        Expr::String(_) => SymbolicValue::Unknown, // Strings not trackable at this level

        // Variable references
        Expr::Identifier(name) => {
            if let Some(sym_val) = state.assignments.get(name) {
                sym_val.clone()
            } else {
                SymbolicValue::Identifier(name.clone())
            }
        }

        // Prior state reference
        Expr::PriorState(name) => SymbolicValue::Previous(name.clone()),

        // Owned references (assignments)
        Expr::OwnedRef(name) => SymbolicValue::Previous(name.clone()),

        // Binary operations
        Expr::Add(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);

            if let Some(simplified) = simplify_binary("+", &left_sym, &right_sym) {
                simplified
            } else {
                SymbolicValue::Binary("+".to_string(), Box::new(left_sym), Box::new(right_sym))
            }
        }

        Expr::Sub(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);

            if let Some(simplified) = simplify_binary("-", &left_sym, &right_sym) {
                simplified
            } else {
                SymbolicValue::Binary("-".to_string(), Box::new(left_sym), Box::new(right_sym))
            }
        }

        Expr::Mul(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);

            if let Some(simplified) = simplify_binary("*", &left_sym, &right_sym) {
                simplified
            } else {
                SymbolicValue::Binary("*".to_string(), Box::new(left_sym), Box::new(right_sym))
            }
        }

        Expr::Div(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);

            if let Some(simplified) = simplify_binary("/", &left_sym, &right_sym) {
                simplified
            } else {
                SymbolicValue::Binary("/".to_string(), Box::new(left_sym), Box::new(right_sym))
            }
        }

        Expr::BitAnd(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            SymbolicValue::Binary("&".to_string(), Box::new(left_sym), Box::new(right_sym))
        }

        Expr::BitOr(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            SymbolicValue::Binary("|".to_string(), Box::new(left_sym), Box::new(right_sym))
        }

        Expr::BitXor(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            SymbolicValue::Binary("^".to_string(), Box::new(left_sym), Box::new(right_sym))
        }
        Expr::Shl(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            SymbolicValue::Binary("<<".to_string(), Box::new(left_sym), Box::new(right_sym))
        }
        Expr::Shr(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            SymbolicValue::Binary(">>".to_string(), Box::new(left_sym), Box::new(right_sym))
        }

        // Function calls - can't track
        Expr::Call(_, _) => SymbolicValue::Unknown,

        // Other complex expressions
        Expr::Neg(_) | Expr::Not(_) | Expr::BitNot(_) => SymbolicValue::Unknown,
        Expr::ListLiteral(_)
        | Expr::ListIndex(_, _)
        | Expr::ListLen(_)
        | Expr::FieldAccess(_, _)
        | Expr::StructInstance(_, _) => SymbolicValue::Unknown,
        Expr::ObjectLiteral(_) => SymbolicValue::Unknown,

        // Comparison operators don't produce symbolic values (they're boolean expressions)
        Expr::Eq(_, _)
        | Expr::Ne(_, _)
        | Expr::Lt(_, _)
        | Expr::Le(_, _)
        | Expr::Gt(_, _)
        | Expr::Ge(_, _)
        | Expr::And(_, _)
        | Expr::Or(_, _) => SymbolicValue::Unknown,
        Expr::PatternMatch { value, .. } => {
            let _ = eval_symbolic(value, state);
            SymbolicValue::Unknown
        }
        Expr::Slice { .. } | Expr::ForAll { .. } | Expr::Exists { .. } => SymbolicValue::Unknown,
    }
}

/// Try to simplify a binary operation on symbolic values
fn simplify_binary(op: &str, left: &SymbolicValue, right: &SymbolicValue) -> Option<SymbolicValue> {
    match (op, left, right) {
        // Arithmetic on literals
        ("+", SymbolicValue::Literal(a, _), SymbolicValue::Literal(b, _)) => {
            Some(SymbolicValue::int_literal(a + b))
        }
        ("-", SymbolicValue::Literal(a, _), SymbolicValue::Literal(b, _)) => {
            Some(SymbolicValue::int_literal(a - b))
        }
        ("*", SymbolicValue::Literal(a, _), SymbolicValue::Literal(b, _)) => {
            Some(SymbolicValue::int_literal(a * b))
        }
        ("/", SymbolicValue::Literal(a, _), SymbolicValue::Literal(b, _)) if *b != 0 => {
            Some(SymbolicValue::int_literal(a / b))
        }

        // Identity and absorption rules for addition
        ("+", SymbolicValue::Literal(0, _), x) => Some(x.clone()),
        ("+", x, SymbolicValue::Literal(0, _)) => Some(x.clone()),

        // Identity and absorption rules for multiplication
        ("*", SymbolicValue::Literal(1, _), x) => Some(x.clone()),
        ("*", x, SymbolicValue::Literal(1, _)) => Some(x.clone()),
        ("*", SymbolicValue::Literal(0, _), _) => Some(SymbolicValue::int_literal(0)),
        ("*", _, SymbolicValue::Literal(0, _)) => Some(SymbolicValue::int_literal(0)),

        // Can't simplify further
        _ => None,
    }
}

/// Check if a postcondition is satisfied given symbolic state
pub fn satisfies_postcondition(post: &Expr, state: &SymbolicState) -> bool {
    match post {
        // Equality check
        Expr::Eq(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            symbolic_equals(&left_sym, &right_sym)
        }

        // Inequality
        Expr::Ne(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            !symbolic_equals(&left_sym, &right_sym)
        }

        // Less than (with basic numeric reasoning)
        Expr::Lt(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            symbolic_less_than(&left_sym, &right_sym)
        }

        // Less than or equal
        Expr::Le(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            symbolic_less_than(&left_sym, &right_sym) || symbolic_equals(&left_sym, &right_sym)
        }

        // Greater than
        Expr::Gt(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            symbolic_less_than(&right_sym, &left_sym)
        }

        // Greater than or equal
        Expr::Ge(left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            symbolic_less_than(&right_sym, &left_sym) || symbolic_equals(&left_sym, &right_sym)
        }

        // Conjunction (AND)
        Expr::And(left, right) => {
            satisfies_postcondition(left, state) && satisfies_postcondition(right, state)
        }

        // Disjunction (OR)
        Expr::Or(left, right) => {
            satisfies_postcondition(left, state) || satisfies_postcondition(right, state)
        }

        // Boolean literal
        Expr::Bool(b) => *b,

        // Negation
        Expr::Not(expr) => !satisfies_postcondition(expr, state),

        // Unknown expressions - conservative (fail)
        _ => false,
    }
}

/// Check symbolic equality between two values
fn symbolic_equals(left: &SymbolicValue, right: &SymbolicValue) -> bool {
    match (left, right) {
        // Literal equality
        (SymbolicValue::Literal(a, _), SymbolicValue::Literal(b, _)) => a == b,

        // Identical identifiers
        (SymbolicValue::Identifier(a), SymbolicValue::Identifier(b)) => a == b,

        // Identical prior-state references
        (SymbolicValue::Previous(a), SymbolicValue::Previous(b)) => a == b,

        // Identical binary expressions
        (SymbolicValue::Binary(op1, l1, r1), SymbolicValue::Binary(op2, l2, r2)) => {
            op1 == op2 && symbolic_equals(l1, l2) && symbolic_equals(r1, r2)
        }

        // Different types; not equal
        _ => false,
    }
}

/// Check symbolic less-than with basic numeric reasoning
fn symbolic_less_than(left: &SymbolicValue, right: &SymbolicValue) -> bool {
    match (left, right) {
        // Literal comparison
        (SymbolicValue::Literal(a, _), SymbolicValue::Literal(b, _)) => a < b,

        // Conservative for unknowns
        _ => false,
    }
}

/// Enumerate all possible execution paths through a statement block
/// Each path represents a sequence of statements with guards either taken or not taken
pub fn enumerate_paths(body: &[Statement]) -> Vec<SymbolicState> {
    let mut paths = vec![SymbolicState::empty()];

    for stmt in body {
        let mut new_paths = Vec::new();

        for mut state in paths {
            match stmt {
                Statement::Guarded {
                    condition,
                    statements,
                } => {
                    // Path 1: Guard taken - execute the statements
                    let mut true_state = state.clone();
                    true_state.add_constraint(condition, true);

                    // Process all inner statements
                    for stmt in statements {
                        execute_statement(stmt, &mut true_state);
                    }
                    new_paths.push(true_state);

                    // Path 2: Guard not taken - skip the statement
                    let mut false_state = state;
                    false_state.add_constraint(condition, false);
                    new_paths.push(false_state);
                }

                Statement::Term(_) | Statement::Escape(_) => {
                    // Termination: path ends here
                    new_paths.push(state);
                }

                _ => {
                    // Regular statement: execute on all paths
                    execute_statement(stmt, &mut state);
                    new_paths.push(state);
                }
            }
        }

        paths = new_paths;
    }

    paths
}

/// Execute a single statement on a symbolic state
fn execute_statement(stmt: &Statement, state: &mut SymbolicState) {
    match stmt {
        Statement::Assignment {
            lhs,
            expr,
            timeout: _,
        } => {
            if let Expr::Identifier(name) | Expr::OwnedRef(name) = lhs {
                state.assign(name, expr);
            }
        }

        Statement::Let { name, expr, .. } => {
            if let Some(e) = expr {
                state.assign(name, e);
            }
        }

        _ => {
            // Other statements don't affect symbolic state
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_creation() {
        let val = SymbolicValue::int_literal(5);
        assert_eq!(val, SymbolicValue::Literal(5, "int".to_string()));
    }

    #[test]
    fn test_literal_addition() {
        let left = SymbolicValue::int_literal(3);
        let right = SymbolicValue::int_literal(2);
        let result = simplify_binary("+", &left, &right);
        assert_eq!(result, Some(SymbolicValue::int_literal(5)));
    }

    #[test]
    fn test_literal_multiplication() {
        let left = SymbolicValue::int_literal(3);
        let right = SymbolicValue::int_literal(4);
        let result = simplify_binary("*", &left, &right);
        assert_eq!(result, Some(SymbolicValue::int_literal(12)));
    }

    #[test]
    fn test_identity_addition_zero() {
        let left = SymbolicValue::int_literal(0);
        let right = SymbolicValue::int_literal(5);
        let result = simplify_binary("+", &left, &right);
        assert_eq!(result, Some(SymbolicValue::int_literal(5)));
    }

    #[test]
    fn test_absorption_multiplication_zero() {
        let left = SymbolicValue::int_literal(0);
        let right = SymbolicValue::int_literal(999);
        let result = simplify_binary("*", &left, &right);
        assert_eq!(result, Some(SymbolicValue::int_literal(0)));
    }

    #[test]
    fn test_symbolic_equals_literals() {
        let left = SymbolicValue::int_literal(5);
        let right = SymbolicValue::int_literal(5);
        assert!(symbolic_equals(&left, &right));

        let right_diff = SymbolicValue::int_literal(3);
        assert!(!symbolic_equals(&left, &right_diff));
    }

    #[test]
    fn test_symbolic_less_than_literals() {
        let left = SymbolicValue::int_literal(3);
        let right = SymbolicValue::int_literal(5);
        assert!(symbolic_less_than(&left, &right));

        let reverse = SymbolicValue::int_literal(5);
        let base = SymbolicValue::int_literal(3);
        assert!(!symbolic_less_than(&reverse, &base));
    }

    #[test]
    fn test_state_assign_literal() {
        let mut state = SymbolicState::empty();
        state.assign("x", &Expr::Integer(5));

        let val = state.get_value("x");
        assert_eq!(val, Some(SymbolicValue::int_literal(5)));
    }

    #[test]
    fn test_satisfies_postcondition_literal_equality() {
        let mut state = SymbolicState::empty();
        state.assign("x", &Expr::Integer(5));

        let postcond = Expr::Eq(
            Box::new(Expr::Identifier("x".to_string())),
            Box::new(Expr::Integer(5)),
        );

        assert!(satisfies_postcondition(&postcond, &state));
    }

    #[test]
    fn test_satisfies_postcondition_literal_inequality() {
        let mut state = SymbolicState::empty();
        state.assign("x", &Expr::Integer(5));

        let postcond = Expr::Eq(
            Box::new(Expr::Identifier("x".to_string())),
            Box::new(Expr::Integer(3)),
        );

        assert!(!satisfies_postcondition(&postcond, &state));
    }

    #[test]
    fn test_satisfies_postcondition_conjunction() {
        let mut state = SymbolicState::empty();
        state.assign("x", &Expr::Integer(5));
        state.assign("y", &Expr::Integer(10));

        let postcond = Expr::And(
            Box::new(Expr::Eq(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Integer(5)),
            )),
            Box::new(Expr::Eq(
                Box::new(Expr::Identifier("y".to_string())),
                Box::new(Expr::Integer(10)),
            )),
        );

        assert!(satisfies_postcondition(&postcond, &state));
    }

    #[test]
    fn test_satisfies_postcondition_disjunction() {
        let mut state = SymbolicState::empty();
        state.assign("x", &Expr::Integer(5));

        let postcond = Expr::Or(
            Box::new(Expr::Eq(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Integer(5)),
            )),
            Box::new(Expr::Eq(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Integer(3)),
            )),
        );

        assert!(satisfies_postcondition(&postcond, &state));
    }
}
