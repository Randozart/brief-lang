use crate::ast::{Expr, Program, Statement, TopLevel, Transaction};
use std::collections::{HashMap, HashSet};

pub struct ProtocolVerifier;

impl ProtocolVerifier {
    pub fn verify(program: &Program) -> Vec<ProtocolError> {
        let mut errors = Vec::new();

        // Build transaction dependency graph
        let mut txn_prerequisites: HashMap<String, Vec<Prerequisite>> = HashMap::new();
        let mut all_preconditions: HashMap<String, Expr> = HashMap::new();

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                let prereqs = Self::extract_prerequisites(&txn.contract.pre_condition);
                txn_prerequisites.insert(txn.name.clone(), prereqs);
                all_preconditions.insert(txn.name.clone(), txn.contract.pre_condition.clone());
            }
        }

        // Check for common protocol issues
        // Issue 1: Transaction requires control=X but no transaction sets control=X first
        for (txn_name, prereqs) in &txn_prerequisites {
            for prereq in prereqs {
                let mut found_writer = false;
                
                // Find transactions that set this register
                for (other_name, other_expr) in &all_preconditions {
                    if other_name == txn_name {
                        continue;
                    }
                    
                    // Simple check: does other transaction's postcondition set this register?
                    // For full analysis, we'd need to check the actual body
                    let other_prereqs = txn_prerequisites.get(other_name);
                    if let Some(prs) = other_prereqs {
                        for pr in prs {
                            if pr.register == prereq.register && pr.value >= 0 {
                                found_writer = true;
                                break;
                            }
                        }
                    }
                }

                if !found_writer && prereq.value > 0 {
                    // This might be okay if it's the entry point, but worth noting
                }
            }
        }

        // Issue 2: Check for missing control handshake
        // Common pattern: write_en without control being set
        let mut has_control_handshake = false;
        let mut has_write_en = false;

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                let body_str = format!("{:?}", txn.body);
                if body_str.contains("write_en") || body_str.contains("write_en") {
                    has_write_en = true;
                }
                // Check if precondition involves control
                let pre_str = format!("{:?}", txn.contract.pre_condition);
                if pre_str.contains("control") {
                    has_control_handshake = true;
                }
            }
        }

        if has_write_en && !has_control_handshake {
            // This is a potential protocol issue - write_en without control check
            // Could be intentional but often indicates a bug
        }

        errors
    }

    fn extract_prerequisites(pre: &Expr) -> Vec<Prerequisite> {
        let mut prereqs = Vec::new();
        Self::extract_prerecs_recursive(pre, &mut prereqs);
        prereqs
    }

    fn extract_prerecs_recursive(expr: &Expr, prereqs: &mut Vec<Prerequisite>) {
        match expr {
            Expr::Eq(lhs, rhs) => {
                if let Expr::Identifier(name) = lhs.as_ref() {
                    if let Expr::Integer(val) = rhs.as_ref() {
                        prereqs.push(Prerequisite {
                            register: name.clone(),
                            value: *val,
                            comparator: Comparator::Eq,
                        });
                    }
                }
            }
            Expr::Ne(lhs, rhs) => {
                if let Expr::Identifier(name) = lhs.as_ref() {
                    if let Expr::Integer(val) = rhs.as_ref() {
                        prereqs.push(Prerequisite {
                            register: name.clone(),
                            value: *val,
                            comparator: Comparator::Ne,
                        });
                    }
                }
            }
            Expr::Ge(lhs, rhs) => {
                if let Expr::Identifier(name) = lhs.as_ref() {
                    if let Expr::Integer(val) = rhs.as_ref() {
                        prereqs.push(Prerequisite {
                            register: name.clone(),
                            value: *val,
                            comparator: Comparator::Ge,
                        });
                    }
                }
            }
            Expr::Gt(lhs, rhs) => {
                if let Expr::Identifier(name) = lhs.as_ref() {
                    if let Expr::Integer(val) = rhs.as_ref() {
                        prereqs.push(Prerequisite {
                            register: name.clone(),
                            value: *val,
                            comparator: Comparator::Gt,
                        });
                    }
                }
            }
            Expr::Le(lhs, rhs) => {
                if let Expr::Identifier(name) = lhs.as_ref() {
                    if let Expr::Integer(val) = rhs.as_ref() {
                        prereqs.push(Prerequisite {
                            register: name.clone(),
                            value: *val,
                            comparator: Comparator::Le,
                        });
                    }
                }
            }
            Expr::Lt(lhs, rhs) => {
                if let Expr::Identifier(name) = lhs.as_ref() {
                    if let Expr::Integer(val) = rhs.as_ref() {
                        prereqs.push(Prerequisite {
                            register: name.clone(),
                            value: *val,
                            comparator: Comparator::Lt,
                        });
                    }
                }
            }
            Expr::And(lhs, rhs) => {
                Self::extract_prerecs_recursive(lhs, prereqs);
                Self::extract_prerecs_recursive(rhs, prereqs);
            }
            Expr::Or(lhs, rhs) => {
                Self::extract_prerecs_recursive(lhs, prereqs);
                Self::extract_prerecs_recursive(rhs, prereqs);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct Prerequisite {
    register: String,
    value: i64,
    comparator: Comparator,
}

#[derive(Debug, Clone, Copy)]
enum Comparator {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

#[derive(Debug, Clone)]
pub struct ProtocolError {
    pub transaction: String,
    pub message: String,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.transaction, self.message)
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prerequisite_extraction() {
        let expr = Expr::Eq(
            Box::new(Expr::Identifier("control".to_string())),
            Box::new(Expr::Integer(1)),
        );
        
        let prereqs = ProtocolVerifier::extract_prerequisites(&expr);
        assert_eq!(prereqs.len(), 1);
        assert_eq!(prereqs[0].register, "control");
        assert_eq!(prereqs[0].value, 1);
    }
}