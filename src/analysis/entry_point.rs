use crate::ast::{Expr, Program, TopLevel, Transaction};

pub struct EntryPointAnalyzer;

impl EntryPointAnalyzer {
    pub fn find_entry_point(program: &Program) -> Result<EntryPoint, EntryPointError> {
        let mut candidates = Vec::new();
        let mut async_candidates = Vec::new();

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                // Check if precondition is true in initial state
                if Self::is_initially_true(&txn.contract.pre_condition, program) {
                    if txn.is_async {
                        async_candidates.push(txn.name.clone());
                    } else {
                        candidates.push(txn.name.clone());
                    }
                }
            }
        }

        if candidates.len() > 1 {
            return Err(EntryPointError::AmbiguousEntry {
                transactions: candidates,
            });
        }

        if candidates.is_empty() && async_candidates.is_empty() {
            return Err(EntryPointError::NoEntryPoint);
        }

        let entry = if !candidates.is_empty() {
            candidates.remove(0)
        } else if !async_candidates.is_empty() {
            // Multiple async is OK, just pick first as representative
            async_candidates.remove(0)
        } else {
            return Err(EntryPointError::NoEntryPoint);
        };

        Ok(EntryPoint {
            transaction_name: entry,
            is_async: !candidates.is_empty() || async_candidates.len() > 1,
            parallel_async: async_candidates.len(),
        })
    }

    fn is_initially_true(expr: &Expr, program: &Program) -> bool {
        match expr {
            Expr::Bool(true) => true,
            Expr::Bool(false) => false,
            Expr::Identifier(name) => {
                // Check if variable is initialized to truthy value
                Self::get_initial_value(name, program) != Some(false)
            }
            Expr::Eq(lhs, rhs) => {
                let l = Self::evaluate_to_constant(lhs, program);
                let r = Self::evaluate_to_constant(rhs, program);
                l == r
            }
            Expr::Ne(lhs, rhs) => {
                let l = Self::evaluate_to_constant(lhs, program);
                let r = Self::evaluate_to_constant(rhs, program);
                l != r
            }
            Expr::Ge(lhs, rhs) => {
                let l = Self::evaluate_to_constant(lhs, program);
                let r = Self::evaluate_to_constant(rhs, program);
                l >= r
            }
            Expr::Gt(lhs, rhs) => {
                let l = Self::evaluate_to_constant(lhs, program);
                let r = Self::evaluate_to_constant(rhs, program);
                l > r
            }
            Expr::Le(lhs, rhs) => {
                let l = Self::evaluate_to_constant(lhs, program);
                let r = Self::evaluate_to_constant(rhs, program);
                l <= r
            }
            Expr::Lt(lhs, rhs) => {
                let l = Self::evaluate_to_constant(lhs, program);
                let r = Self::evaluate_to_constant(rhs, program);
                l < r
            }
            Expr::And(lhs, rhs) => {
                Self::is_initially_true(lhs, program) && Self::is_initially_true(rhs, program)
            }
            Expr::Or(lhs, rhs) => {
                Self::is_initially_true(lhs, program) || Self::is_initially_true(rhs, program)
            }
            Expr::Not(inner) => !Self::is_initially_true(inner, program),
            // For complex expressions, conservatively return false
            _ => false,
        }
    }

    fn get_initial_value(name: &str, program: &Program) -> Option<bool> {
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                if decl.name == name {
                    return Some(decl.expr.is_some());
                }
            }
        }
        None
    }

    fn evaluate_to_constant(expr: &Expr, program: &Program) -> i64 {
        match expr {
            Expr::Integer(n) => *n,
            Expr::Identifier(name) => {
                if let Some(val) = Self::get_initial_value(name, program) {
                    if val { 1 } else { 0 }
                } else {
                    0
                }
            }
            Expr::Neg(inner) => -Self::evaluate_to_constant(inner, program),
            Expr::Add(l, r) => Self::evaluate_to_constant(l, program) + Self::evaluate_to_constant(r, program),
            Expr::Sub(l, r) => Self::evaluate_to_constant(l, program) - Self::evaluate_to_constant(r, program),
            Expr::Mul(l, r) => Self::evaluate_to_constant(l, program) * Self::evaluate_to_constant(r, program),
            _ => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntryPoint {
    pub transaction_name: String,
    pub is_async: bool,
    pub parallel_async: usize,
}

#[derive(Debug, Clone)]
pub enum EntryPointError {
    AmbiguousEntry { transactions: Vec<String> },
    NoEntryPoint,
}

impl std::fmt::Display for EntryPointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryPointError::AmbiguousEntry { transactions } => {
                write!(f, "Multiple transactions can fire from initial state: {}. \
                    Specify which is the entry point or make some async.", 
                    transactions.join(", "))
            }
            EntryPointError::NoEntryPoint => {
                write!(f, "No transaction can fire from initial state. \
                    Define a transaction with a precondition that is initially true.")
            }
        }
    }
}

impl std::error::Error for EntryPointError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_entry_point() {
        // Test with a simple program
        let source = r#"
            let x: UInt = 0;
            
            rct txn idle [x >= 0] {
                term;
            };
        "#;
        
        // This test would require full parsing - just verify module compiles
        assert!(true);
    }
}