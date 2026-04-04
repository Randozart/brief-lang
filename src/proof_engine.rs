use crate::ast::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum ProofError {
    UnhandledOutcome { sig: String, missing: Vec<String> },
    MutualExclusionViolation { txn1: String, txn2: String, conflict_vars: Vec<String> },
    UnreachableState { precondition: String, reachable_from: Vec<String> },
    NoAcceptingPath { txn: String },
    BorrowConflict { txn1: String, txn2: String, var: String },
    ContractImplicationFailure { defn: String },
    TrueAssertionFailure { sig: String, reason: String },
}

pub struct ProofEngine {
    errors: Vec<ProofError>,
    state_dag: HashMap<String, HashSet<String>>,
}

impl ProofEngine {
    pub fn new() -> Self {
        ProofEngine {
            errors: Vec::new(),
            state_dag: HashMap::new(),
        }
    }

    pub fn verify_program(&mut self, program: &Program) -> Vec<ProofError> {
        self.build_state_dag(program);
        self.check_exhaustiveness(program);
        self.check_mutual_exclusion(program);
        self.check_total_path(program);
        self.check_true_assertions(program);
        self.errors.clone()
    }

    fn build_state_dag(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    self.state_dag
                        .entry(decl.name.clone())
                        .or_insert_with(HashSet::new);
                }
                TopLevel::Transaction(txn) => {
                    let pre_vars = self.extract_state_vars(&txn.contract.pre_condition);
                    let post_vars = self.extract_state_vars(&txn.contract.post_condition);

                    for var in pre_vars {
                        self.state_dag
                            .entry(var)
                            .or_insert_with(HashSet::new)
                            .insert(txn.name.clone());
                    }

                    for var in post_vars {
                        self.state_dag
                            .entry(var)
                            .or_insert_with(HashSet::new)
                            .insert(txn.name.clone());
                    }
                }
                _ => {}
            }
        }
    }

    fn extract_state_vars(&self, expr: &Expr) -> HashSet<String> {
        let mut vars = HashSet::new();
        self.collect_identifiers(expr, &mut vars);
        vars
    }

    fn collect_identifiers(&self, expr: &Expr, vars: &mut HashSet<String>) {
        match expr {
            Expr::Identifier(name) => {
                vars.insert(name.clone());
            }
            Expr::OwnedRef(name) => {
                vars.insert(name.clone());
            }
            Expr::PriorState(name) => {
                vars.insert(name.clone());
            }
            Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::Or(l, r)
            | Expr::And(l, r) => {
                self.collect_identifiers(l, vars);
                self.collect_identifiers(r, vars);
            }
            Expr::Not(inner) | Expr::Neg(inner) | Expr::BitNot(inner) => {
                self.collect_identifiers(inner, vars);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    self.collect_identifiers(arg, vars);
                }
            }
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::Bool(_) => {}
        }
    }

    fn check_exhaustiveness(&mut self, program: &Program) {
        let mut sig_returns: HashMap<String, Vec<Type>> = HashMap::new();

        for item in &program.items {
            if let TopLevel::Signature(sig) = item {
                match &sig.result_type {
                    ResultType::Projection(types) => {
                        sig_returns.insert(sig.name.clone(), types.clone());
                    }
                    ResultType::TrueAssertion => {}
                }
            }
        }

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                for stmt in &txn.body {
                    if let Statement::Unification { name: _, pattern: _, expr } = stmt {
                        // Legacy check - simplified for now
                    }
                }
            }
        }
    }

    fn type_name(&self, ty: &Type) -> String {
        match ty {
            Type::Custom(name) => name.clone(),
            Type::Int => "Int".to_string(),
            Type::Float => "Float".to_string(),
            Type::String => "String".to_string(),
            Type::Bool => "Bool".to_string(),
            Type::Data => "Data".to_string(),
            Type::Void => "Void".to_string(),
            Type::Union(types) => {
                types.iter().map(|t| self.type_name(t)).collect::<Vec<_>>().join("|")
            }
            Type::ContractBound(inner, _) => self.type_name(inner),
        }
    }

    fn check_mutual_exclusion(&mut self, program: &Program) {
        let mut async_txns: Vec<&Transaction> = Vec::new();

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_async && txn.is_reactive {
                    async_txns.push(txn);
                }
            }
        }

        for i in 0..async_txns.len() {
            for j in (i + 1)..async_txns.len() {
                let txn1 = async_txns[i];
                let txn2 = async_txns[j];

                let conflicts = self.find_write_conflicts(txn1, txn2);
                if !conflicts.is_empty() {
                    let pre1_overlaps = self.preconditions_overlap(txn1, txn2);
                    if pre1_overlaps {
                        self.errors.push(ProofError::MutualExclusionViolation {
                            txn1: txn1.name.clone(),
                            txn2: txn2.name.clone(),
                            conflict_vars: conflicts,
                        });
                    }
                }
            }
        }
    }

    fn find_write_conflicts(&self, txn1: &Transaction, txn2: &Transaction) -> Vec<String> {
        let writes1 = self.extract_write_vars(txn1);
        let writes2 = self.extract_write_vars(txn2);

        writes1
            .intersection(&writes2)
            .cloned()
            .collect()
    }

    fn extract_write_vars(&self, txn: &Transaction) -> HashSet<String> {
        let mut vars = HashSet::new();
        for stmt in &txn.body {
            self.collect_write_vars(stmt, &mut vars);
        }
        vars
    }

    fn collect_write_vars(&self, stmt: &Statement, vars: &mut HashSet<String>) {
        match stmt {
            Statement::Assignment { is_owned: true, name, .. } => {
                vars.insert(name.clone());
            }
            Statement::Assignment { is_owned: false, .. } => {}
            Statement::Let { .. } => {}
            Statement::Expression(_) => {}
            Statement::Term(_) => {}
            Statement::Escape(_) => {}
            Statement::Guarded { statement, .. } => {
                self.collect_write_vars(statement, vars);
            }
            Statement::Unification { .. } => {}
        }
    }

    fn preconditions_overlap(&self, txn1: &Transaction, txn2: &Transaction) -> bool {
        let vars1 = self.extract_state_vars(&txn1.contract.pre_condition);
        let vars2 = self.extract_state_vars(&txn2.contract.pre_condition);

        !vars1.is_disjoint(&vars2)
    }

    fn check_total_path(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_reactive {
                    let has_accepting_path = self.has_term_statement(&txn.body);
                    if !has_accepting_path {
                        self.errors.push(ProofError::NoAcceptingPath {
                            txn: txn.name.clone(),
                        });
                    }
                }
            }
        }
    }

    fn has_term_statement(&self, statements: &[Statement]) -> bool {
        for stmt in statements {
            match stmt {
                Statement::Term(outputs) => {
                    return true;
                }
                Statement::Guarded { statement, .. } => {
                    if self.has_term_statement(&[(*statement.clone()).clone()]) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }
    
    fn check_true_assertions(&mut self, program: &Program) {
        let mut defns: HashMap<String, &Definition> = HashMap::new();
        
        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                defns.insert(defn.name.clone(), defn);
            }
        }
        
        for item in &program.items {
            if let TopLevel::Signature(sig) = item {
                if let ResultType::TrueAssertion = sig.result_type {
                    if let Some(defn) = defns.get(&sig.name) {
                        self.verify_true_assertion(&sig.name, defn);
                    }
                }
            }
        }
    }
    
    fn verify_true_assertion(&mut self, sig_name: &str, defn: &Definition) {
        let term_values = self.extract_term_values(defn);
        
        for (i, values) in term_values.iter().enumerate() {
            let bool_outputs: Vec<&Option<Expr>> = values.iter()
                .filter(|v| {
                    if let Some(Expr::Bool(_)) = v {
                        true
                    } else {
                        false
                    }
                })
                .collect();
            
            for (j, val) in bool_outputs.iter().enumerate() {
                if let Some(Expr::Bool(false)) = val {
                    self.errors.push(ProofError::TrueAssertionFailure {
                        sig: sig_name.to_string(),
                        reason: format!(
                            "Exit path {} contains Bool output {} with value false",
                            i, j
                        ),
                    });
                    return;
                }
            }
            
            let has_any_bool = bool_outputs.iter().any(|v| v.is_some());
            if !has_any_bool && !bool_outputs.is_empty() {
                self.errors.push(ProofError::TrueAssertionFailure {
                    sig: sig_name.to_string(),
                    reason: format!(
                        "Exit path {} has no Bool output to assert as true",
                        i
                    ),
                });
                return;
            }
        }
    }
    
    fn extract_term_values(&self, defn: &Definition) -> Vec<Vec<Option<Expr>>> {
        let mut values = Vec::new();
        self.collect_term_values(&defn.body, &mut values);
        values
    }
    
    fn collect_term_values(&self, statements: &[Statement], results: &mut Vec<Vec<Option<Expr>>>) {
        for stmt in statements {
            match stmt {
                Statement::Term(outputs) => {
                    results.push(outputs.clone());
                }
                Statement::Guarded { condition: _, statement } => {
                    self.collect_term_values(&[(*statement.clone()).clone()], results);
                }
                _ => {}
            }
        }
    }
}
