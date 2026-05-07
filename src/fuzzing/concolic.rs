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

//! Concolic Fuzzer (Proof-Guided)
//!
//! Uses the proof engine's path constraints to generate concrete inputs
//! that satisfy them, eliminating impossible state permutations.
//!
//! Concolic = Concrete + Symbolic execution combined.
//! The symbolic engine identifies path constraints, then the fuzzer
//! generates concrete values that satisfy those constraints.

use crate::ast::*;
use crate::proof_engine::{ProofEngine, SymbolicState, SymbolicValue};
use std::collections::HashMap;

/// Extract path constraints from a transaction's body
pub fn extract_path_constraints(txn: &Transaction) -> Vec<PathConstraintInfo> {
    let mut constraints = Vec::new();
    collect_constraints_recursive(&txn.body, &mut constraints, Vec::new());
    constraints
}

/// Information about a path constraint
#[derive(Debug, Clone)]
pub struct PathConstraintInfo {
    /// The condition expression
    pub condition: Expr,
    /// Depth in the statement tree (how nested)
    pub depth: usize,
    /// Whether this is a guard condition or a contract condition
    pub kind: ConstraintKind,
    /// Whether the condition involves trigger (volatile) variables
    pub involves_trigger: bool,
}

#[derive(Debug, Clone)]
pub enum ConstraintKind {
    /// Guard condition: [condition] { ... }
    Guard,
    /// Transaction precondition
    Precondition,
    /// Transaction postcondition
    Postcondition,
    /// Local trigger expression
    TriggerExpr,
}

fn collect_constraints_recursive(
    stmts: &[Statement],
    constraints: &mut Vec<PathConstraintInfo>,
    path: Vec<Expr>,
) {
    for stmt in stmts {
        match stmt {
            Statement::Guarded { condition, statements } => {
                let involves_trigger = expr_involves_trigger(condition);
                constraints.push(PathConstraintInfo {
                    condition: condition.clone(),
                    depth: path.len(),
                    kind: ConstraintKind::Guard,
                    involves_trigger,
                });

                // Recurse into guarded body with this condition on the path
                let mut new_path = path.clone();
                new_path.push(condition.clone());
                collect_constraints_recursive(statements, constraints, new_path);
            }
            Statement::LocalTrigger { expr, .. } => {
                if let Some(e) = expr {
                    constraints.push(PathConstraintInfo {
                        condition: e.clone(),
                        depth: path.len(),
                        kind: ConstraintKind::TriggerExpr,
                        involves_trigger: expr_involves_trigger(e),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Check if an expression involves a trigger variable
fn expr_involves_trigger(expr: &Expr) -> bool {
    match expr {
        Expr::Identifier(name) => {
            // Heuristic: trigger variables often have signal-like names
            is_trigger_like_name(name)
        }
        Expr::PriorState(name) => is_trigger_like_name(name),
        Expr::OwnedRef(name) => is_trigger_like_name(name),
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Mod(l, r) | Expr::Eq(l, r) | Expr::Ne(l, r)
        | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r)
        | Expr::And(l, r) | Expr::Or(l, r)
        | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r)
        | Expr::Shl(l, r) | Expr::Shr(l, r) => {
            expr_involves_trigger(l) || expr_involves_trigger(r)
        }
        Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => expr_involves_trigger(e),
        Expr::Call(_, args) => args.iter().any(expr_involves_trigger),
        Expr::ListIndex(list, idx) => {
            expr_involves_trigger(list) || expr_involves_trigger(idx)
        }
        _ => false,
    }
}

/// Heuristic: check if a variable name looks like a trigger/signal
fn is_trigger_like_name(name: &str) -> bool {
    let trigger_prefixes = [
        "sig", "signal", "trg", "trigger", "event", "irq", "interrupt",
        "input", "sensor", "button", "key", "click", "tick", "clock",
        "stdin", "stdout", "network", "socket", "file_", "fs_",
    ];
    
    let name_lower = name.to_lowercase();
    trigger_prefixes.iter().any(|prefix| name_lower.starts_with(prefix))
        || name_lower.ends_with("_trg")
        || name_lower.ends_with("_trigger")
        || name_lower.ends_with("_signal")
}

/// Generate concrete values that satisfy a path constraint
/// Returns a list of (variable_name, concrete_value) assignments
pub fn generate_concrete_values(
    constraint: &PathConstraintInfo,
    state: &SymbolicState,
) -> Vec<(String, i64)> {
    let mut assignments = Vec::new();
    
    match &constraint.condition {
        Expr::Eq(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), *val));
            }
        }
        Expr::Gt(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), val + 1));
            }
        }
        Expr::Lt(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), val.saturating_sub(1)));
            }
        }
        Expr::Ge(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), *val));
            }
        }
        Expr::Le(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), *val));
            }
        }
        Expr::Ne(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), val + 1));
            }
        }
        Expr::And(l, r) => {
            // Both must be true
            let mut l_assignments = generate_concrete_for_expr(l, state);
            let mut r_assignments = generate_concrete_for_expr(r, state);
            assignments.append(&mut l_assignments);
            assignments.append(&mut r_assignments);
        }
        _ => {
            // For complex expressions, try to extract any integer comparisons
            extract_integer_comparisons(&constraint.condition, &mut assignments);
        }
    }
    
    assignments
}

fn generate_concrete_for_expr(expr: &Expr, _state: &SymbolicState) -> Vec<(String, i64)> {
    let mut assignments = Vec::new();
    extract_integer_comparisons(expr, &mut assignments);
    assignments
}

fn extract_integer_comparisons(expr: &Expr, assignments: &mut Vec<(String, i64)>) {
    match expr {
        Expr::Eq(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), *val));
            }
        }
        Expr::Gt(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), val + 1));
            }
        }
        Expr::Lt(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), val.saturating_sub(1)));
            }
        }
        Expr::Ge(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), *val));
            }
        }
        Expr::Le(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), *val));
            }
        }
        Expr::Ne(l, r) => {
            if let (Expr::Identifier(var), Expr::Integer(val)) = (&**l, &**r) {
                assignments.push((var.clone(), val + 1));
            }
        }
        Expr::And(l, r) | Expr::Or(l, r) => {
            extract_integer_comparisons(l, assignments);
            extract_integer_comparisons(r, assignments);
        }
        _ => {}
    }
}

/// Filter out constraints that involve trigger variables (unpredictable)
/// Returns only constraints that can be pre-evaluated
pub fn filter_pre_evaluable_constraints(
    constraints: &[PathConstraintInfo],
) -> Vec<&PathConstraintInfo> {
    constraints.iter().filter(|c| !c.involves_trigger).collect()
}

/// Generate a concolic test case from a transaction
/// Returns a list of concrete input assignments that exercise different paths
pub fn generate_concolic_test_cases(txn: &Transaction) -> Vec<Vec<(String, i64)>> {
    let constraints = extract_path_constraints(txn);
    let pre_evaluable = filter_pre_evaluable_constraints(&constraints);
    
    let mut test_cases = Vec::new();
    
    // Generate test case for each pre-evaluable constraint
    // Each test case exercises one specific path
    let mut state = SymbolicState::new();
    
    for constraint in pre_evaluable {
        let values = generate_concrete_values(constraint, &state);
        if !values.is_empty() {
            test_cases.push(values);
        }
    }
    
    // Also generate a "default" test case with zero values
    let param_defaults: Vec<(String, i64)> = txn.parameters.iter()
        .map(|(name, _)| (name.clone(), 0))
        .collect();
    if !param_defaults.is_empty() {
        test_cases.push(param_defaults);
    }
    
    test_cases
}

/// Run concolic analysis on a program and return test cases
pub fn run_concolic_analysis(program: &Program) -> Vec<TransactionTestCases> {
    let mut results = Vec::new();
    
    for item in &program.items {
        if let TopLevel::Transaction(txn) = item {
            let test_cases = generate_concolic_test_cases(txn);
            let constraints = extract_path_constraints(txn);
            
            results.push(TransactionTestCases {
                txn_name: txn.name.clone(),
                test_cases,
                total_constraints: constraints.len(),
                pre_evaluable: filter_pre_evaluable_constraints(&constraints).len(),
                trigger_constraints: constraints.iter().filter(|c| c.involves_trigger).count(),
            });
        }
    }
    
    results
}

/// Test cases for a single transaction
#[derive(Debug)]
pub struct TransactionTestCases {
    pub txn_name: String,
    pub test_cases: Vec<Vec<(String, i64)>>,
    pub total_constraints: usize,
    pub pre_evaluable: usize,
    pub trigger_constraints: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_extract_path_constraints_simple() {
        let code = r#"
            let x: Int = 0;
            txn foo [x < 100][x == @x + 1] {
                [x > 0] {
                    &x = x + 1;
                };
                [x == 0] {
                    &x = 1;
                };
                term;
            };
        "#;
        
        let mut parser = Parser::new(code);
        let program = parser.parse().expect("Failed to parse");
        
        if let TopLevel::Transaction(txn) = &program.items[1] {
            let constraints = extract_path_constraints(txn);
            assert_eq!(constraints.len(), 2, "Should find 2 guard constraints");
            
            // Both guards don't involve triggers
            for c in &constraints {
                assert!(!c.involves_trigger, "x is not a trigger variable");
            }
        } else {
            panic!("Expected transaction");
        }
    }

    #[test]
    fn test_extract_path_constraints_with_trigger() {
        let code = r#"
            let state: Int = 0;
            trg button: Bool;
            
            txn handle_click [button == true][state == @state + 1] {
                [button && state > 0] {
                    &state = state + 1;
                };
                term;
            };
        "#;
        
        let mut parser = Parser::new(code);
        let program = parser.parse().expect("Failed to parse");
        
        if let TopLevel::Transaction(txn) = &program.items[2] {
            let constraints = extract_path_constraints(txn);
            assert_eq!(constraints.len(), 1, "Should find 1 guard constraint");
            
            // The guard involves 'button' which is trigger-like
            assert!(constraints[0].involves_trigger, "Guard involves trigger variable");
        } else {
            panic!("Expected transaction");
        }
    }

    #[test]
    fn test_filter_pre_evaluable_constraints() {
        let code = r#"
            let x: Int = 0;
            trg sensor: Int;
            
            txn process [x > 0][x == @x + 1] {
                [x > 10] {
                    &x = x + 1;
                };
                [sensor > 0] {
                    &x = x + 2;
                };
                term;
            };
        "#;
        
        let mut parser = Parser::new(code);
        let program = parser.parse().expect("Failed to parse");
        
        if let TopLevel::Transaction(txn) = &program.items[2] {
            let constraints = extract_path_constraints(txn);
            let pre_evaluable = filter_pre_evaluable_constraints(&constraints);
            
            assert_eq!(constraints.len(), 2);
            assert_eq!(pre_evaluable.len(), 1, "Only x > 10 is pre-evaluable");
            assert!(!pre_evaluable[0].involves_trigger);
        } else {
            panic!("Expected transaction");
        }
    }

    #[test]
    fn test_generate_concrete_values() {
        let state = SymbolicState::new();
        
        // Test x > 5
        let constraint = PathConstraintInfo {
            condition: Expr::Gt(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Integer(5)),
            ),
            depth: 0,
            kind: ConstraintKind::Guard,
            involves_trigger: false,
        };
        
        let values = generate_concrete_values(&constraint, &state);
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], ("x".to_string(), 6));
        
        // Test x == 10
        let constraint = PathConstraintInfo {
            condition: Expr::Eq(
                Box::new(Expr::Identifier("y".to_string())),
                Box::new(Expr::Integer(10)),
            ),
            depth: 0,
            kind: ConstraintKind::Guard,
            involves_trigger: false,
        };
        
        let values = generate_concrete_values(&constraint, &state);
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], ("y".to_string(), 10));
    }

    #[test]
    fn test_generate_concolic_test_cases() {
        let code = r#"
            let counter: Int = 0;
            
            txn increment(amount: Int) [amount > 0 && counter < 100][counter == @counter + amount] {
                [amount > 10] {
                    &counter = counter + amount;
                };
                [amount <= 10] {
                    &counter = counter + amount;
                };
                term;
            };
        "#;
        
        let mut parser = Parser::new(code);
        let program = parser.parse().expect("Failed to parse");
        
        if let TopLevel::Transaction(txn) = &program.items[1] {
            let test_cases = generate_concolic_test_cases(txn);
            
            // Should have test cases for amount > 10, amount <= 10, and default
            assert!(!test_cases.is_empty(), "Should generate at least one test case");
            
            // Verify test cases have valid values
            for case in &test_cases {
                for (name, val) in case {
                    assert!(!name.is_empty());
                    // Values should be reasonable
                    assert!(*val >= -1000 && *val <= 10000, "Value {} out of range", val);
                }
            }
        } else {
            panic!("Expected transaction");
        }
    }

    #[test]
    fn test_run_concolic_analysis() {
        let code = r#"
            let x: Int = 0;
            
            txn inc [x < 100][x == @x + 1] {
                [x > 50] {
                    &x = x + 2;
                };
                &x = x + 1;
                term;
            };
            
            txn dec [x > 0][x == @x - 1] {
                [x < 10] {
                    &x = x - 2;
                };
                &x = x - 1;
                term;
            };
        "#;
        
        let mut parser = Parser::new(code);
        let program = parser.parse().expect("Failed to parse");
        
        let results = run_concolic_analysis(&program);
        
        assert_eq!(results.len(), 2, "Should have results for 2 transactions");
        
        for result in &results {
            assert!(!result.txn_name.is_empty());
            // Each transaction has a guard, so should have at least one test case
            assert!(result.total_constraints >= 1, "Should have at least one constraint");
        }
    }

    #[test]
    fn test_is_trigger_like_name() {
        assert!(is_trigger_like_name("sigint"));
        assert!(is_trigger_like_name("signal_handler"));
        assert!(is_trigger_like_name("trg_button"));
        assert!(is_trigger_like_name("event_queue"));
        assert!(is_trigger_like_name("irq_line"));
        assert!(is_trigger_like_name("sensor_value"));
        assert!(is_trigger_like_name("button_state"));
        assert!(is_trigger_like_name("clock_tick"));
        assert!(is_trigger_like_name("stdin_buffer"));
        assert!(is_trigger_like_name("network_data"));
        assert!(is_trigger_like_name("file_event"));
        assert!(is_trigger_like_name("my_trg"));
        assert!(is_trigger_like_name("my_trigger"));
        assert!(is_trigger_like_name("my_signal"));
        
        assert!(!is_trigger_like_name("counter"));
        assert!(!is_trigger_like_name("balance"));
        assert!(!is_trigger_like_name("result"));
        assert!(!is_trigger_like_name("x"));
        assert!(!is_trigger_like_name("temp"));
    }

    proptest::proptest! {
        #[test]
        fn test_concolic_never_panics(code in "[a-zA-Z0-9_ \\t\\n\\r;{}\\[\\]().,=+\\-*/<>!&|]{0,500}") {
            // Concolic analysis should never panic on any input
            let mut parser = Parser::new(&code);
            if let Ok(program) = parser.parse() {
                let _ = run_concolic_analysis(&program);
            }
        }
    }
}
