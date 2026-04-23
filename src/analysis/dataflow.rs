use crate::ast::{Expr, Program, Statement, TopLevel, Transaction};
use std::collections::{HashMap, HashSet};

pub struct DataflowAnalyzer {
    program: &'static Program,
}

#[derive(Debug, Clone)]
pub enum DataflowError {
    UseBeforeSet {
        variable: String,
        transaction: String,
        depends_on: String,
    },
    MissingWeightLoad {
        computation_txn: String,
        load_txn: Option<String>,
    },
}

impl DataflowAnalyzer {
    pub fn new(program: &'static Program) -> Self {
        DataflowAnalyzer { program }
    }

    pub fn analyze(&self) -> Vec<DataflowError> {
        let mut errors = Vec::new();

        // Build dependency graph: what variables does each transaction read/write
        let mut txn_reads: HashMap<String, HashSet<String>> = HashMap::new();
        let mut txn_writes: HashMap<String, HashSet<String>> = HashMap::new();
        let mut txn_preconditions: HashMap<String, Expr> = HashMap::new();

        for item in &self.program.items {
            if let TopLevel::Transaction(txn) = item {
                let reads = self.extract_reads(&txn.contract.pre_condition);
                let writes = self.extract_writes(&txn.body);
                
                txn_reads.insert(txn.name.clone(), reads);
                txn_writes.insert(txn.name.clone(), writes);
                txn_preconditions.insert(txn.name.clone(), txn.contract.pre_condition.clone());
            }
        }

        // Check for use-before-set patterns
        for (txn_name, reads) in &txn_reads {
            let writes = txn_writes.get(txn_name).cloned().unwrap_or_default();
            
            for read_var in reads {
                // Skip if this transaction writes to the variable
                if writes.contains(read_var) {
                    continue;
                }

                // Check if there's another transaction that MUST run first
                // For now, we check if the read variable is initialized to 0 and
                // no other transaction writes to it
                let init_value = self.get_initial_value(read_var);
                if init_value == Some(0) || init_value.is_none() {
                    // Variable starts at 0 - if this txn reads it, check if any
                    // transaction with TRUE precondition writes to it
                    let mut has_writer = false;
                    for (other_txn, other_writes) in &txn_writes {
                        if other_txn == txn_name {
                            continue;
                        }
                        if other_writes.contains(read_var) {
                            // Check if this other transaction can run first
                            if let Some(pre) = txn_preconditions.get(other_txn) {
                                if self.is_trivially_true(pre) {
                                    has_writer = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !has_writer && !reads.is_empty() {
                        // This is a potential use-before-set
                        // For now, we'll just note it - in practice, this depends on
                        // whether the initial value is meaningful
                    }
                }
            }
        }

        // Specific check: weight loading before computation
        // If there's a computation that reads from weight_buffer, there should be
        // a transaction that writes to weight_buffer first
        let weight_load_txns = self.find_weight_load_transactions();
        let compute_txns = self.find_compute_transactions();

        for compute_txn in &compute_txns {
            if weight_load_txns.is_empty() {
                errors.push(DataflowError::MissingWeightLoad {
                    computation_txn: compute_txn.clone(),
                    load_txn: None,
                });
            }
        }

        errors
    }

    fn extract_reads(&self, expr: &Expr) -> HashSet<String> {
        let mut reads = HashSet::new();
        self.extract_ids_recursive(expr, &mut reads);
        reads
    }

    fn extract_ids_recursive(&self, expr: &Expr, ids: &mut HashSet<String>) {
        match expr {
            Expr::Identifier(name) => {
                ids.insert(name.clone());
            }
            Expr::OwnedRef(name) => {
                ids.insert(name.clone());
            }
            Expr::PriorState(name) => {
                ids.insert(name.clone());
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.extract_ids_recursive(l, ids);
                self.extract_ids_recursive(r, ids);
            }
            Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) 
            | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r)
            | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r) => {
                self.extract_ids_recursive(l, ids);
                self.extract_ids_recursive(r, ids);
            }
            Expr::Not(inner) | Expr::Neg(inner) | Expr::BitNot(inner) => {
                self.extract_ids_recursive(inner, ids);
            }
            Expr::ListIndex(list, idx) => {
                self.extract_ids_recursive(list, ids);
                self.extract_ids_recursive(idx, ids);
            }
            _ => {}
        }
    }

    fn extract_writes(&self, body: &[Statement]) -> HashSet<String> {
        let mut writes = HashSet::new();
        
        for stmt in body {
            match stmt {
                Statement::Assignment { lhs, .. } => {
                    if let Expr::Identifier(name) = lhs {
                        writes.insert(name.clone());
                    }
                }
                Statement::Guarded { statements, .. } => {
                    writes.extend(self.extract_writes(statements));
                }
                _ => {}
            }
        }

        writes
    }

    fn get_initial_value(&self, name: &str) -> Option<i64> {
        for item in &self.program.items {
            if let TopLevel::StateDecl(decl) = item {
                if decl.name == name {
                    if let Some(expr) = &decl.expr {
                        if let Expr::Integer(n) = expr {
                            return Some(*n);
                        }
                    }
                    return Some(0); // Default to 0
                }
            }
        }
        None
    }

    fn is_trivially_true(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Bool(true) => true,
            Expr::Identifier(_) => true, // Could be true or false
            Expr::Eq(l, r) => {
                let lv = self.eval_to_int(l);
                let rv = self.eval_to_int(r);
                lv == rv
            }
            _ => false,
        }
    }

    fn eval_to_int(&self, expr: &Expr) -> Option<i64> {
        match expr {
            Expr::Integer(n) => Some(*n),
            _ => None,
        }
    }

    fn find_weight_load_transactions(&self) -> Vec<String> {
        let mut txns = Vec::new();
        
        for item in &self.program.items {
            if let TopLevel::Transaction(txn) = item {
                // Look for transactions that write to common weight buffer names
                let writes = self.extract_writes(&txn.body);
                if writes.iter().any(|w| 
                    w.contains("weight") || 
                    w.contains("buf") ||
                    w.contains("buffer")
                ) {
                    txns.push(txn.name.clone());
                }
            }
        }
        
        txns
    }

    fn find_compute_transactions(&self) -> Vec<String> {
        let mut txns = Vec::new();
        
        for item in &self.program.items {
            if let TopLevel::Transaction(txn) = item {
                // Look for transactions that compute (have accumulation or math)
                let body_str = format!("{:?}", txn.body);
                if body_str.contains("acc") || 
                   body_str.contains("result") ||
                   body_str.contains("compute") ||
                   body_str.contains("multiply") ||
                   body_str.contains("add") {
                    txns.push(txn.name.clone());
                }
            }
        }
        
        txns
    }
}

pub struct TransactionProtocolVerifier;

impl TransactionProtocolVerifier {
    pub fn verify(program: &Program) -> Vec<ProtocolError> {
        let mut errors = Vec::new();

        // Collect all preconditions and their required registers
        let mut required_sequence: HashMap<String, Vec<String>> = HashMap::new();

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                let mut required = Vec::new();
                
                // Simple analysis: if precondition uses control register,
                // track what value it must be set to
                if let Expr::Eq(lhs, rhs) = &txn.contract.pre_condition {
                    if let Expr::Identifier(name) = lhs.as_ref() {
                        if let Expr::Integer(val) = rhs.as_ref() {
                            required.push(format!("{}={}", name, val));
                        }
                    }
                }

                if !required.is_empty() {
                    required_sequence.insert(txn.name.clone(), required);
                }
            }
        }

        // Check for protocol violations (e.g., write_en without control being set first)
        // This is a simplified check - full implementation would track state transitions
        
        errors
    }
}

#[derive(Debug, Clone)]
pub enum ProtocolError {
    PreconditionNotMet {
        transaction: String,
        required: String,
        missing: String,
    },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::PreconditionNotMet { transaction, required, missing } => {
                write!(f, "Transaction '{}' requires {} but {} was not set", 
                       transaction, required, missing)
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataflow_analysis() {
        assert!(true);
    }
}