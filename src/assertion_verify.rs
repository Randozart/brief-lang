/// Feature C: Assertion Verification with `sig -> true`
///
/// Enables compile-time verification that functions always return Bool = true.
/// Example: sig always_succeeds: String -> true; asserts the function always succeeds.
use crate::ast::{Definition, Expr, ResultType, Signature, Statement, Type};
use std::collections::HashMap;

/// Verify that a sig's `-> true` assertion is valid
pub fn verify_true_assertion(sig: &Signature, defn: &Definition) -> Result<(), String> {
    // Check that sig has TrueAssertion result type
    if !matches!(sig.result_type, ResultType::TrueAssertion) {
        return Ok(()); // Not an assertion, nothing to verify
    }

    // Check that definition produces Bool
    if !defn.outputs.is_empty() && defn.outputs[0] != Type::Bool {
        return Err(format!(
            "Assertion '{}' requires Bool output, but definition produces {:?}",
            sig.name, defn.outputs[0]
        ));
    }

    // Check all execution paths for Bool = true guarantee
    verify_all_paths_produce_true(defn)
}

/// Check if all paths through the definition produce Bool = true
fn verify_all_paths_produce_true(defn: &Definition) -> Result<(), String> {
    // Start with a symbolic state from the precondition
    let mut vars: HashMap<String, Expr> = HashMap::new();

    // Extract variables from precondition
    extract_vars_from_expr(&defn.contract.pre_condition, &mut vars);

    // Walk through body and check termination conditions
    check_all_paths(&defn.body, vars, defn)
}

/// Check that all execution paths produce true
fn check_all_paths(
    body: &[Statement],
    mut vars: HashMap<String, Expr>,
    defn: &Definition,
) -> Result<(), String> {
    let mut found_term = false;
    let mut found_true_path = false;

    for stmt in body {
        match stmt {
            Statement::Assignment { name, expr, .. } => {
                // Track assignments
                vars.insert(name.clone(), expr.clone());
            }

            Statement::Guarded {
                condition,
                statements,
            } => {
                // Check guarded branch
                let mut branch_vars = vars.clone();
                // The condition is now known to be true in this branch
                branch_vars.insert(
                    format!("__guard_{}", format!("{:?}", condition)),
                    Expr::Bool(true),
                );

                match check_all_paths(statements, branch_vars, defn) {
                    Ok(()) => {
                        found_true_path = true;
                    }
                    Err(_) => {
                        // If guarded branch fails, we might have other branches or failures
                        // For now, we require all paths to succeed (conservative)
                        return Err("Guarded branch may not produce Bool = true".to_string());
                    }
                }
            }

            Statement::Term(outputs) => {
                found_term = true;
                // Check if this term produces true
                if let Some(Some(expr)) = outputs.first() {
                    if is_provably_true(expr, &vars) {
                        found_true_path = true;
                    } else {
                        return Err(format!(
                            "Termination expression is not provably true in definition '{}'",
                            defn.name
                        ));
                    }
                } else {
                    return Err("Term has no output expression".to_string());
                }
            }

            _ => {}
        }
    }

    if !found_term {
        return Err("Definition body has no termination".to_string());
    }

    if !found_true_path {
        return Err("No execution path produces Bool = true in definition body".to_string());
    }

    Ok(())
}

/// Check if an expression is provably true given current symbolic state
fn is_provably_true(expr: &Expr, vars: &HashMap<String, Expr>) -> bool {
    match expr {
        Expr::Bool(b) => *b,

        Expr::Identifier(name) => {
            // Check if this variable is known to be true
            match vars.get(name) {
                Some(Expr::Bool(true)) => true,
                _ => false,
            }
        }

        Expr::PriorState(name) => {
            // Check prior state value
            let prior_name = format!("@{}", name);
            match vars.get(&prior_name) {
                Some(Expr::Bool(true)) => true,
                _ => false,
            }
        }

        _ => false, // Conservative: unknown expressions not provably true
    }
}

/// Extract variables mentioned in an expression and add to state
fn extract_vars_from_expr(expr: &Expr, vars: &mut HashMap<String, Expr>) {
    match expr {
        Expr::Identifier(name) => {
            vars.entry(name.clone()).or_insert(Expr::Bool(false));
        }
        Expr::PriorState(name) => {
            let prior_name = format!("@{}", name);
            vars.entry(prior_name).or_insert(Expr::Bool(false));
        }
        Expr::And(l, r) | Expr::Or(l, r) | Expr::Eq(l, r) | Expr::Ne(l, r) => {
            extract_vars_from_expr(l, vars);
            extract_vars_from_expr(r, vars);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Contract, Definition, ResultType, Signature, Type};

    #[test]
    fn test_literal_true_assertion() {
        let sig = Signature {
            name: "always_true".to_string(),
            input_types: vec![],
            result_type: ResultType::TrueAssertion,
            source: Some("always_true_defn".to_string()),
            alias: None,
            bound_defn: None,
        };

        let defn = Definition {
            name: "always_true_defn".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![Type::Bool],
            output_type: None,
            output_names: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                span: None,
            },
            body: vec![Statement::Term(vec![Some(Expr::Bool(true))])],
            is_lambda: false,
        };

        assert!(verify_true_assertion(&sig, &defn).is_ok());
    }

    #[test]
    fn test_false_assertion_fails() {
        let sig = Signature {
            name: "always_false".to_string(),
            input_types: vec![],
            result_type: ResultType::TrueAssertion,
            source: Some("always_false_defn".to_string()),
            alias: None,
            bound_defn: None,
        };

        let defn = Definition {
            name: "always_false_defn".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![Type::Bool],
            output_type: None,
            output_names: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                span: None,
            },
            body: vec![Statement::Term(vec![Some(Expr::Bool(false))])],
            is_lambda: false,
        };

        assert!(verify_true_assertion(&sig, &defn).is_err());
    }

    #[test]
    fn test_variable_assigned_true() {
        let sig = Signature {
            name: "check_x".to_string(),
            input_types: vec![Type::Bool],
            result_type: ResultType::TrueAssertion,
            source: Some("check_x_defn".to_string()),
            alias: None,
            bound_defn: None,
        };

        let defn = Definition {
            name: "check_x_defn".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::Bool)],
            outputs: vec![Type::Bool],
            output_type: None,
            output_names: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    is_owned: false,
                    name: "result".to_string(),
                    expr: Expr::Bool(true),
                    timeout: None,
                },
                Statement::Term(vec![Some(Expr::Identifier("result".to_string()))]),
            ],
            is_lambda: false,
        };

        assert!(verify_true_assertion(&sig, &defn).is_ok());
    }

    #[test]
    fn test_non_bool_output_fails() {
        let sig = Signature {
            name: "not_bool".to_string(),
            input_types: vec![],
            result_type: ResultType::TrueAssertion,
            source: Some("not_bool_defn".to_string()),
            alias: None,
            bound_defn: None,
        };

        let defn = Definition {
            name: "not_bool_defn".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![Type::String],
            output_type: None,
            output_names: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                span: None,
            },
            body: vec![Statement::Term(vec![Some(Expr::String(
                "not bool".to_string(),
            ))])],
            is_lambda: false,
        };

        assert!(verify_true_assertion(&sig, &defn).is_err());
    }

    #[test]
    fn test_no_assertion_type_skipped() {
        let sig = Signature {
            name: "regular_sig".to_string(),
            input_types: vec![],
            result_type: ResultType::Projection(vec![Type::Bool]),
            source: Some("regular_sig_defn".to_string()),
            alias: None,
            bound_defn: None,
        };

        let defn = Definition {
            name: "regular_sig_defn".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![Type::Bool],
            output_type: None,
            output_names: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                span: None,
            },
            body: vec![Statement::Term(vec![Some(Expr::Bool(false))])],
            is_lambda: false,
        };

        // Should be OK because this is not a TrueAssertion
        assert!(verify_true_assertion(&sig, &defn).is_ok());
    }
}
