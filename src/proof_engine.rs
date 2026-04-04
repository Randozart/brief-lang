use crate::ast::*;
use crate::errors::{Diagnostic, Severity, Span};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ProofError {
    pub code: String,
    pub title: String,
    pub explanation: String,
    pub proof_chain: Vec<String>,
    pub examples: Vec<String>,
    pub hints: Vec<String>,
}

impl ProofError {
    pub fn new(code: &str, title: &str) -> Self {
        ProofError {
            code: code.to_string(),
            title: title.to_string(),
            explanation: String::new(),
            proof_chain: Vec::new(),
            examples: Vec::new(),
            hints: Vec::new(),
        }
    }

    pub fn with_explanation(mut self, text: &str) -> Self {
        self.explanation = text.to_string();
        self
    }

    pub fn with_proof_step(mut self, step: &str) -> Self {
        self.proof_chain.push(step.to_string());
        self
    }

    pub fn with_example(mut self, example: &str) -> Self {
        self.examples.push(example.to_string());
        self
    }

    pub fn with_hint(mut self, hint: &str) -> Self {
        self.hints.push(hint.to_string());
        self
    }
}

pub struct ProofEngine {
    errors: Vec<ProofError>,
    state_dag: HashMap<String, HashSet<String>>,
    transactions: Vec<Transaction>,
}

impl ProofEngine {
    pub fn new() -> Self {
        ProofEngine {
            errors: Vec::new(),
            state_dag: HashMap::new(),
            transactions: Vec::new(),
        }
    }

    pub fn verify_program(&mut self, program: &Program) -> Vec<ProofError> {
        self.build_state_dag(program);
        self.collect_transactions(program);
        self.check_exhaustiveness(program);
        self.check_mutual_exclusion(program);
        self.check_total_path(program);
        self.check_true_assertions(program);
        self.check_postcondition_contradictions(program);
        self.errors.clone()
    }

    fn check_postcondition_contradictions(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.analyze_postcondition(txn);
            }
        }
    }

    fn analyze_postcondition(&mut self, txn: &Transaction) {
        let post = &txn.contract.post_condition;

        if let Expr::Eq(left, right) = post {
            let (var, prior_var) = match (left.as_ref(), right.as_ref()) {
                (Expr::Identifier(v), Expr::PriorState(p)) => (v.clone(), p.clone()),
                (Expr::PriorState(p), Expr::Identifier(v)) => (v.clone(), p.clone()),
                _ => return,
            };

            if var == prior_var {
                let mut err = ProofError::new("P003", "postcondition is always satisfied");
                err.explanation = format!(
                    "transaction '{}' postcondition '{} == @{}' is always true",
                    txn.name, var, var
                );
                err.proof_chain.push(format!(
                    "1. '@{}' refers to the value of '{}' at transaction start",
                    var, var
                ));
                err.proof_chain
                    .push(format!("2. postcondition requires: {} == @{}", var, var));
                err.proof_chain
                    .push(format!("3. this is always true (any value equals itself)"));
                err.hints
                    .push("did you mean to modify the variable?".to_string());
                self.errors.push(err);
            }
        }
    }

    fn collect_transactions(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.transactions.push(txn.clone());
            }
        }
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
            Expr::ListLiteral(elements) => {
                for elem in elements {
                    self.collect_identifiers(elem, vars);
                }
            }
            Expr::ListIndex(list_expr, index_expr) => {
                self.collect_identifiers(list_expr, vars);
                self.collect_identifiers(index_expr, vars);
            }
            Expr::ListLen(inner) => {
                self.collect_identifiers(inner, vars);
            }
            Expr::FieldAccess(obj, _) => {
                self.collect_identifiers(obj, vars);
            }
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
                    if let Statement::Unification {
                        name: _,
                        pattern: _,
                        expr,
                    } = stmt
                    {
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
            Type::Union(types) => types
                .iter()
                .map(|t| self.type_name(t))
                .collect::<Vec<_>>()
                .join("|"),
            Type::ContractBound(inner, _) => self.type_name(inner),
            Type::TypeVar(name) => name.clone(),
            Type::Generic(name, type_args) => {
                format!(
                    "{}<{}>",
                    name,
                    type_args
                        .iter()
                        .map(|t| self.type_name(t))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Type::Applied(name, type_args) => {
                format!(
                    "{}<{}>",
                    name,
                    type_args
                        .iter()
                        .map(|t| self.type_name(t))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
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
                        let mut err =
                            ProofError::new("P001", "concurrent mutation without synchronization");
                        err.explanation = format!(
                            "both transactions '{}' and '{}' can mutate the same state variables",
                            txn1.name, txn2.name
                        );
                        err.proof_chain.push(format!(
                            "1. '{}' is reactive (fires automatically)",
                            txn1.name
                        ));
                        err.proof_chain.push(format!(
                            "2. '{}' is reactive (fires automatically)",
                            txn2.name
                        ));
                        err.proof_chain.push(format!(
                            "3. both mutate shared variable(s): {}",
                            conflicts.join(", ")
                        ));
                        err.examples.push(format!(
                            "race condition scenario: if {} runs first and sets {} to X, \
                            then {} runs and expects a different value, the result is undefined",
                            txn1.name, conflicts[0], txn2.name
                        ));
                        err.hints.push(format!(
                            "to make these mutually exclusive, add a guard: txn {} [...][...] {{ |{}| ... }}",
                            txn1.name, txn2.name
                        ));
                        err.hints.push(
                            "or use sequential transactions instead of reactive ones".to_string(),
                        );
                        self.errors.push(err);
                    }
                }
            }
        }
    }

    fn find_write_conflicts(&self, txn1: &Transaction, txn2: &Transaction) -> Vec<String> {
        let writes1 = self.extract_write_vars(txn1);
        let writes2 = self.extract_write_vars(txn2);

        writes1.intersection(&writes2).cloned().collect()
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
            Statement::Assignment {
                is_owned: true,
                name,
                ..
            } => {
                vars.insert(name.clone());
            }
            Statement::Assignment {
                is_owned: false, ..
            } => {}
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
                        let mut err =
                            ProofError::new("P005", "transaction has no valid termination");
                        err.explanation = format!(
                            "transaction '{}' has no 'term' statement, so it can never complete",
                            txn.name
                        );
                        err.proof_chain
                            .push(format!("1. '{}' is declared as reactive (rct)", txn.name));
                        err.proof_chain.push(
                            "2. reactive transactions must have a 'term' to settle".to_string(),
                        );
                        err.proof_chain
                            .push("3. without 'term', the reactor will wait forever".to_string());
                        err.hints.push(format!(
                            "add 'term;' at the end of transaction '{}'",
                            txn.name
                        ));
                        err.hints
                            .push("or use 'term expr1, expr2, ...;' to return values".to_string());
                        self.errors.push(err);
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
            let bool_outputs: Vec<&Option<Expr>> = values
                .iter()
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
                    let mut err = ProofError::new("P006", "true assertion failed");
                    err.explanation = format!(
                        "signature '{}' declares '-> true' but exit path {} returns false",
                        sig_name, i
                    );
                    err.proof_chain.push(format!(
                        "1. '{}' declares it returns true (verified by compiler)",
                        sig_name
                    ));
                    err.proof_chain
                        .push(format!("2. definition '{}' has exit path {}", defn.name, i));
                    err.proof_chain
                        .push(format!("3. Bool output slot {} returns false", j));
                    err.examples
                        .push(format!("when this path executes, the contract is violated"));
                    err.hints
                        .push("ensure all code paths return true for Bool outputs".to_string());
                    self.errors.push(err);
                    return;
                }
            }

            let has_any_bool = bool_outputs.iter().any(|v| v.is_some());
            if !has_any_bool && !bool_outputs.is_empty() {
                let mut err = ProofError::new("P007", "true assertion cannot be verified");
                err.explanation = format!(
                    "signature '{}' declares '-> true' but exit path {} has no Bool output",
                    sig_name, i
                );
                err.proof_chain.push(format!(
                    "1. '-> true' requires a Bool output that is always true for '{}'",
                    sig_name
                ));
                err.proof_chain
                    .push(format!("2. exit path {} has no Bool in its outputs", i));
                err.hints.push(format!(
                    "ensure definition '{}' returns a Bool value on all paths",
                    defn.name
                ));
                self.errors.push(err);
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
                Statement::Guarded {
                    condition: _,
                    statement,
                } => {
                    self.collect_term_values(&[(*statement.clone()).clone()], results);
                }
                _ => {}
            }
        }
    }
}
