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

use crate::ast::*;
use crate::errors::{Diagnostic, Severity, Span};
use crate::sig_casting;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ProofError {
    pub code: String,
    pub title: String,
    pub explanation: String,
    pub proof_chain: Vec<String>,
    pub examples: Vec<String>,
    pub hints: Vec<String>,
    pub is_warning: bool,
    pub span: Option<Span>,
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
            is_warning: false,
            span: None,
        }
    }

    pub fn new_warning(code: &str, title: &str) -> Self {
        ProofError {
            code: code.to_string(),
            title: title.to_string(),
            explanation: String::new(),
            proof_chain: Vec::new(),
            examples: Vec::new(),
            hints: Vec::new(),
            is_warning: true,
            span: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
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

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicValue {
    Concrete(i64),
    ConcreteFloat(f64),
    Symbolic(String),
    Add(Box<SymbolicValue>, Box<SymbolicValue>),
    Sub(Box<SymbolicValue>, Box<SymbolicValue>),
    Mul(Box<SymbolicValue>, Box<SymbolicValue>),
    BitAnd(Box<SymbolicValue>, Box<SymbolicValue>),
    BitOr(Box<SymbolicValue>, Box<SymbolicValue>),
    BitXor(Box<SymbolicValue>, Box<SymbolicValue>),
    Unknown,
}

impl SymbolicValue {
    fn from_expr(expr: &Expr, vars: &HashMap<String, SymbolicValue>) -> Self {
        match expr {
            Expr::Integer(n) => SymbolicValue::Concrete(*n),
            Expr::Float(f) => SymbolicValue::ConcreteFloat(*f),
            Expr::Bool(b) => SymbolicValue::Concrete(if *b { 1 } else { 0 }),
            Expr::Identifier(name) => vars
                .get(name)
                .cloned()
                .unwrap_or(SymbolicValue::Symbolic(name.clone())),
            Expr::PriorState(name) => SymbolicValue::Symbolic(format!("@{}", name)),
            Expr::Add(l, r) => SymbolicValue::Add(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Sub(l, r) => SymbolicValue::Sub(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Mul(l, r) => SymbolicValue::Mul(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::BitAnd(l, r) => SymbolicValue::BitAnd(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::BitOr(l, r) => SymbolicValue::BitOr(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::BitXor(l, r) => SymbolicValue::BitXor(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            _ => SymbolicValue::Unknown,
        }
    }
}
#[derive(Debug, Clone)]
pub struct PathConstraint {
    pub condition: Expr,
    pub is_negated: bool,
}

#[derive(Debug, Clone)]
pub struct SymbolicState {
    pub vars: HashMap<String, SymbolicValue>,
    pub constraints: Vec<PathConstraint>,
}

impl SymbolicState {
    pub fn new() -> Self {
        SymbolicState {
            vars: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    fn with_constraint(mut self, condition: Expr, is_negated: bool) -> Self {
        self.constraints.push(PathConstraint {
            condition,
            is_negated,
        });
        self
    }

    fn with_assignment(&mut self, name: &str, value: SymbolicValue) {
        self.vars.insert(name.to_string(), value);
    }
}

pub struct SymbolicExecutor {
    errors: Vec<ProofError>,
}

impl SymbolicExecutor {
    pub fn new() -> Self {
        SymbolicExecutor { errors: Vec::new() }
    }

    pub fn verify_transaction(&mut self, txn: &Transaction) -> Vec<ProofError> {
        if txn.is_lambda {
            let pre = &txn.contract.pre_condition;
            let post = &txn.contract.post_condition;

            let mut state = self.init_state_from_precondition(pre);
            // ADDED: Inject parameters into symbolic state
            for (p_name, _) in &txn.parameters {
                state.vars.insert(p_name.clone(), SymbolicValue::Symbolic(p_name.clone()));
            }
            
            self.verify_contract_implication(
                pre,
                post,
                &[],
                state,
                format!("lambda transaction '{}'", txn.name),
            );

            let mut state = self.init_state_from_precondition(pre);
            for (p_name, _) in &txn.parameters {
                state.vars.insert(p_name.clone(), SymbolicValue::Symbolic(p_name.clone()));
            }

            if let Some(neg_post) = self.negate_expr(post) {
                let pre_vars = self.extract_vars(pre);
                let post_vars = self.extract_vars(post);
                if !pre_vars.is_empty() && !post_vars.is_empty() {
                    self.errors.push(
                        ProofError::new("P016", "Lambda transaction requires provable postcondition")
                            .with_explanation(&format!(
                                "Lambda transaction '{}' has no body. Ensure the postcondition can be proven from the precondition alone.",
                                txn.name
                            ))
                            .with_hint("Consider adding a body or simplifying the postcondition")
                            .with_span(txn.span.unwrap_or(Span::dummy()))
                    );
                }
            }
        } else {
            let mut state = self.init_state_from_precondition(&txn.contract.pre_condition);
            for (p_name, _) in &txn.parameters {
                state.vars.insert(p_name.clone(), SymbolicValue::Symbolic(p_name.clone()));
            }

            self.verify_contract_implication(
                &txn.contract.pre_condition,
                &txn.contract.post_condition,
                &txn.body,
                state,
                format!("transaction '{}'", txn.name),
            );
        }

        self.errors.clone()
    }

    pub fn verify_definition(&mut self, defn: &Definition) -> Vec<ProofError> {
        // Lambda-style: verify postcondition is provable from precondition alone
        if defn.is_lambda {
            let pre = &defn.contract.pre_condition;
            let post = &defn.contract.post_condition;

            // Check if post is entailed by pre (pre => post is always true)
            let state = self.init_state_from_precondition(pre);
            self.verify_contract_implication(
                pre,
                post,
                &[], // No body - just check if post follows from pre
                state,
                format!("lambda definition '{}'", defn.name),
            );

            // Additional check: if pre is true, post must be true (no counterexample possible)
            // We do this by checking that (pre && !post) is unsatisfiable
            let mut state = self.init_state_from_precondition(pre);
            if let Some(neg_post) = self.negate_expr(post) {
                // Simplified check: if both pre and !post reference same variables, warn
                let pre_vars = self.extract_vars(pre);
                let post_vars = self.extract_vars(post);
                if !pre_vars.is_empty() && !post_vars.is_empty() {
                    // Variables exist in both - need actual verification
                    // For now, add a warning that lambda requires manual proof
                    self.errors.push(
                        ProofError::new("P015", "Lambda definition requires provable postcondition")
                            .with_explanation(&format!(
                                "Lambda definition '{}' has no body. Ensure the postcondition can be proven from the precondition alone.",
                                defn.name
                            ))
                            .with_hint("Consider adding a body or simplifying the postcondition")
                            .with_span(defn.contract.span.unwrap_or(Span::dummy()))
                    );
                }
            }
        } else {
            let mut state = self.init_state_from_precondition(&defn.contract.pre_condition);

            self.verify_contract_implication(
                &defn.contract.pre_condition,
                &defn.contract.post_condition,
                &defn.body,
                state,
                format!("definition '{}'", defn.name),
            );
        }

        self.errors.clone()
    }

    fn negate_expr(&self, expr: &Expr) -> Option<Expr> {
        match expr {
            Expr::Bool(b) => Some(Expr::Bool(!b)),
            Expr::Identifier(name) => Some(Expr::Not(Box::new(Expr::Identifier(name.clone())))),
            _ => None,
        }
    }

    fn init_state_from_precondition(&self, pre: &Expr) -> SymbolicState {
        let mut state = SymbolicState::new();

        match pre {
            Expr::Bool(true) => {}
            Expr::And(l, r) | Expr::Or(l, r) => {
                let left_vars = self.extract_vars(l);
                let right_vars = self.extract_vars(r);
                for var in left_vars.iter().chain(right_vars.iter()) {
                    state
                        .vars
                        .insert(var.clone(), SymbolicValue::Symbolic(var.clone()));
                }
            }
            _ => {
                let vars = self.extract_vars(pre);
                for var in &vars {
                    state
                        .vars
                        .insert(var.clone(), SymbolicValue::Symbolic(var.clone()));
                }
            }
        }

        state
    }

    fn extract_vars(&self, expr: &Expr) -> HashSet<String> {
        let mut vars = HashSet::new();
        self.collect_vars(expr, &mut vars);
        vars
    }

    fn collect_vars(&self, expr: &Expr, vars: &mut HashSet<String>) {
        match expr {
            Expr::Identifier(name) => {
                vars.insert(name.clone());
            }
            Expr::PriorState(name) => {
                vars.insert(name.clone());
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.collect_vars(l, vars);
                self.collect_vars(r, vars);
            }
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r) => {
                self.collect_vars(l, vars);
                self.collect_vars(r, vars);
            }
            Expr::And(l, r) | Expr::Or(l, r) => {
                self.collect_vars(l, vars);
                self.collect_vars(r, vars);
            }
            Expr::Not(inner) => self.collect_vars(inner, vars),
            _ => {}
        }
    }

    fn verify_contract_implication(
        &mut self,
        pre_condition: &Expr,
        post_condition: &Expr,
        body: &[Statement],
        mut state: SymbolicState,
        context: String,
    ) {
        let term_paths = self.enumerate_paths(body, state.clone());

        for (path_idx, (path_state, term_outputs)) in term_paths.iter().enumerate() {
            if !self.implies(pre_condition, path_state, post_condition) {
                let mut err = ProofError::new("P008", "contract verification failed");
                err.explanation = format!(
                    "{}: post-condition not satisfied on path {}",
                    context, path_idx
                );
                err.proof_chain.push(format!(
                    "1. Pre-condition: {}",
                    self.format_expr(pre_condition)
                ));

                if !path_state.constraints.is_empty() {
                    err.proof_chain.push("2. Path constraints:".to_string());
                    for (i, constraint) in path_state.constraints.iter().enumerate() {
                        let cond_str = self.format_expr(&constraint.condition);
                        err.proof_chain.push(format!("   {}. {}", i + 1, cond_str));
                    }
                }

                err.proof_chain.push(format!(
                    "3. Post-condition: {}",
                    self.format_expr(post_condition)
                ));

                err.hints.push(format!(
                    "ensure the transaction/definition can reach a satisfying post-condition from the pre-condition"
                ));

                self.errors.push(err);
            }
        }
    }

    fn enumerate_paths(
        &self,
        body: &[Statement],
        state: SymbolicState,
    ) -> Vec<(SymbolicState, Vec<Option<Expr>>)> {
        let mut paths = Vec::new();
        self.enumerate_paths_recursive(body, state, &mut paths);
        paths
    }

    fn enumerate_paths_recursive(
        &self,
        body: &[Statement],
        state: SymbolicState,
        paths: &mut Vec<(SymbolicState, Vec<Option<Expr>>)>,
    ) {
        let mut current_state = state;
        let mut terminated = false;
        let mut term_outputs: Vec<Option<Expr>> = Vec::new();

        for stmt in body {
            if terminated {
                break;
            }

            match stmt {
                Statement::Assignment {
                    lhs,
                    expr,
                    timeout: _,
                } => {
                    let value = SymbolicValue::from_expr(expr, &current_state.vars);
                    if let Expr::Identifier(name) | Expr::OwnedRef(name) = lhs {
                        current_state.vars.insert(name.clone(), value);
                    } else if let Expr::ListIndex(list_expr, _) = lhs {
                        if let Expr::Identifier(name) | Expr::OwnedRef(name) = &**list_expr {
                            current_state.vars.insert(name.clone(), value);
                        }
                    }
                }
                Statement::Let { name, expr, .. } => {
                    if let Some(e) = expr {
                        let value = SymbolicValue::from_expr(e, &current_state.vars);
                        current_state.vars.insert(name.clone(), value);
                    }
                }
                Statement::Guarded {
                    condition,
                    statements,
                } => {
                    let true_state = current_state
                        .clone()
                        .with_constraint(condition.clone(), false);
                    let false_state = current_state
                        .clone()
                        .with_constraint(condition.clone(), true);

                    let mut true_paths = Vec::new();
                    self.enumerate_paths_recursive(statements, true_state, &mut true_paths);

                    let mut false_paths = Vec::new();
                    self.enumerate_paths_recursive(&body[1..], false_state, &mut false_paths);

                    for (s, outputs) in true_paths.into_iter().chain(false_paths.into_iter()) {
                        paths.push((s, outputs));
                    }
                    return;
                }
                Statement::Term(outputs) => {
                    terminated = true;
                    term_outputs = outputs.clone();
                }
                Statement::Escape(_) => {
                    terminated = true;
                }
                Statement::Expression(_) | Statement::Unification { .. } => {}
            }
        }

        if terminated {
            paths.push((current_state, term_outputs));
        }
    }

    fn implies(&mut self, pre: &Expr, state: &SymbolicState, post: &Expr) -> bool {
        let pre_true = self.is_truthy(pre, state);
        if !pre_true {
            return true;
        }

        for constraint in &state.constraints {
            if constraint.is_negated {
                if self.is_truthy(&constraint.condition, state) {
                    return false;
                }
            }
        }

        if self.contains_prior_state(post) {
            return self.verify_post_with_prior(state, post);
        }

        let post_true = self.is_truthy(post, state);
        post_true
    }

    fn verify_post_with_prior(&self, state: &SymbolicState, post: &Expr) -> bool {
        let changed_vars: HashSet<String> = state.vars.keys().cloned().collect();

        self.check_post_satisfiable(post, state, &changed_vars)
    }

    fn check_post_satisfiable(
        &self,
        post: &Expr,
        state: &SymbolicState,
        _changed_vars: &HashSet<String>,
    ) -> bool {
        match post {
            Expr::Eq(l, r) => {
                let l_has_prior = self.contains_prior_state(l);
                let r_has_prior = self.contains_prior_state(r);

                if l_has_prior || r_has_prior {
                    return true;
                }

                self.is_truthy(post, state)
            }
            _ => true,
        }
    }

    fn contains_prior_state(&self, expr: &Expr) -> bool {
        match expr {
            Expr::PriorState(_) => true,
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.contains_prior_state(l) || self.contains_prior_state(r)
            }
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r) => self.contains_prior_state(l) || self.contains_prior_state(r),
            Expr::And(l, r) | Expr::Or(l, r) => {
                self.contains_prior_state(l) || self.contains_prior_state(r)
            }
            Expr::Not(inner) => self.contains_prior_state(inner),
            _ => false,
        }
    }

    fn is_truthy(&self, expr: &Expr, state: &SymbolicState) -> bool {
        match expr {
            Expr::Bool(b) => *b,
            Expr::Identifier(name) => {
                if let Some(val) = state.vars.get(name) {
                    match val {
                        SymbolicValue::Concrete(n) => *n != 0,
                        SymbolicValue::ConcreteFloat(f) => *f != 0.0,
                        _ => true,
                    }
                } else {
                    true
                }
            }
            Expr::And(l, r) => self.is_truthy(l, state) && self.is_truthy(r, state),
            Expr::Or(l, r) => self.is_truthy(l, state) || self.is_truthy(r, state),
            Expr::Not(inner) => !self.is_truthy(inner, state),
            Expr::Eq(l, r) => self.eval_eq(l, r, state),
            Expr::Ne(l, r) => !self.eval_eq(l, r, state),
            Expr::Lt(l, r) => self.eval_cmp(l, r, state, |a, b| a < b),
            Expr::Le(l, r) => self.eval_cmp(l, r, state, |a, b| a <= b),
            Expr::Gt(l, r) => self.eval_cmp(l, r, state, |a, b| a > b),
            Expr::Ge(l, r) => self.eval_cmp(l, r, state, |a, b| a >= b),
            _ => true,
        }
    }

    fn eval_eq(&self, l: &Expr, r: &Expr, state: &SymbolicState) -> bool {
        let lv = self.eval_numeric(l, state);
        let rv = self.eval_numeric(r, state);
        match (lv, rv) {
            (Some(a), Some(b)) => a == b,
            _ => {
                let ls = self.format_expr(l);
                let rs = self.format_expr(r);
                ls == rs
            }
        }
    }

    fn eval_cmp<F>(&self, l: &Expr, r: &Expr, state: &SymbolicState, op: F) -> bool
    where
        F: Fn(i64, i64) -> bool,
    {
        let lv = self.eval_numeric(l, state);
        let rv = self.eval_numeric(r, state);
        match (lv, rv) {
            (Some(a), Some(b)) => op(a, b),
            _ => true,
        }
    }

    fn eval_numeric(&self, expr: &Expr, state: &SymbolicState) -> Option<i64> {
        match expr {
            Expr::Integer(n) => Some(*n),
            Expr::Identifier(name) => {
                if let Some(val) = state.vars.get(name) {
                    match val {
                        SymbolicValue::Concrete(n) => Some(*n),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            Expr::Add(l, r) => {
                let a = self.eval_numeric(l, state)?;
                let b = self.eval_numeric(r, state)?;
                Some(a + b)
            }
            Expr::Sub(l, r) => {
                let a = self.eval_numeric(l, state)?;
                let b = self.eval_numeric(r, state)?;
                Some(a - b)
            }
            Expr::Mul(l, r) => {
                let a = self.eval_numeric(l, state)?;
                let b = self.eval_numeric(r, state)?;
                Some(a * b)
            }
            _ => None,
        }
    }

    fn format_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::String(s) => format!("\"{}\"", s),
            Expr::Bool(b) => b.to_string(),
            Expr::Identifier(name) => name.clone(),
            Expr::PriorState(name) => format!("@{}", name),
            Expr::Add(l, r) => format!("{} + {}", self.format_expr(l), self.format_expr(r)),
            Expr::Sub(l, r) => format!("{} - {}", self.format_expr(l), self.format_expr(r)),
            Expr::Mul(l, r) => format!("{} * {}", self.format_expr(l), self.format_expr(r)),
            Expr::Div(l, r) => format!("{} / {}", self.format_expr(l), self.format_expr(r)),
            Expr::Eq(l, r) => format!("{} == {}", self.format_expr(l), self.format_expr(r)),
            Expr::Ne(l, r) => format!("{} != {}", self.format_expr(l), self.format_expr(r)),
            Expr::Lt(l, r) => format!("{} < {}", self.format_expr(l), self.format_expr(r)),
            Expr::Le(l, r) => format!("{} <= {}", self.format_expr(l), self.format_expr(r)),
            Expr::Gt(l, r) => format!("{} > {}", self.format_expr(l), self.format_expr(r)),
            Expr::Ge(l, r) => format!("{} >= {}", self.format_expr(l), self.format_expr(r)),
            Expr::And(l, r) => format!("{} && {}", self.format_expr(l), self.format_expr(r)),
            Expr::Or(l, r) => format!("{} || {}", self.format_expr(l), self.format_expr(r)),
            Expr::Not(inner) => format!("!{}", self.format_expr(inner)),
            Expr::Neg(inner) => format!("-{}", self.format_expr(inner)),
            Expr::Call(name, args) => {
                let args_str = args
                    .iter()
                    .map(|a| self.format_expr(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args_str)
            }
            _ => "<expr>".to_string(),
        }
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
        self.check_trivial_contracts(program);
        self.check_sig_projections(program);
        self.check_ffi_error_handling(program);
        self.verify_contracts(program);
        self.errors.clone()
    }

    fn verify_contracts(&mut self, program: &Program) {
        let mut sym_exec = SymbolicExecutor::new();

        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    let errs = sym_exec.verify_transaction(txn);
                    self.errors.extend(errs);
                }
                TopLevel::Definition(defn) => {
                    let errs = sym_exec.verify_definition(defn);
                    self.errors.extend(errs);
                }
                _ => {}
            }
        }
    }

    fn check_sig_projections(&mut self, program: &Program) {
        // Build a map of definitions by name for quick lookup
        let mut definitions: HashMap<String, &Definition> = HashMap::new();
        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                definitions.insert(defn.name.clone(), defn);
            }
        }

        // Verify each signature's projections against its corresponding definition
        for item in &program.items {
            if let TopLevel::Signature(sig) = item {
                if let Some(source_name) = &sig.source {
                    if let Some(defn) = definitions.get(source_name) {
                        // Feature B: Verify sig casting
                        match sig_casting::verify_sig_projection(sig, defn) {
                            Ok(()) => {
                                // Projection is valid
                            }
                            Err(err_msg) => {
                                let mut proof_err =
                                    ProofError::new("B001", "invalid sig projection");
                                proof_err.explanation = format!("Sig '{}': {}", sig.name, err_msg);
                                proof_err.proof_chain.push(format!(
                                    "1. Signature '{}' projects from definition '{}'",
                                    sig.name, source_name
                                ));
                                proof_err.proof_chain.push(format!(
                                    "2. Requested types: {:?}",
                                    match &sig.result_type {
                                        ResultType::Projection(types) => types.clone(),
                                        ResultType::TrueAssertion => vec![],
                                        ResultType::VoidType => vec![],
                                    }
                                ));
                                if let Some(ref output_type) = defn.output_type {
                                    proof_err.proof_chain.push(format!(
                                        "3. Available types from definition: {:?}",
                                        output_type.all_types()
                                    ));
                                }
                                proof_err.hints.push(
                                    "ensure all requested types are produced by the definition"
                                        .to_string(),
                                );
                                self.errors.push(proof_err);
                            }
                        }
                    }
                }
            }
        }
    }

    fn check_ffi_error_handling(&mut self, program: &Program) {
        // Build a map of FFI bindings for verification
        let mut ffi_bindings: HashMap<String, ForeignSignature> = HashMap::new();
        for item in &program.items {
            if let TopLevel::ForeignBinding {
                name, signature, ..
            } = item
            {
                ffi_bindings.insert(name.clone(), signature.clone());
            }
        }

        // If no FFI bindings, nothing to verify
        if ffi_bindings.is_empty() {
            return;
        }

        // Check all definitions for proper FFI error handling
        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                self.verify_ffi_error_handling_in_definition(defn, &ffi_bindings);
            }
        }
    }

    fn verify_ffi_error_handling_in_definition(
        &mut self,
        defn: &Definition,
        ffi_bindings: &HashMap<String, ForeignSignature>,
    ) {
        // Find all FFI calls in the definition
        let ffi_calls = self.find_ffi_calls_in_body(&defn.body, ffi_bindings);

        for (frgn_name, _call_context) in ffi_calls {
            // For each FFI call, verify that there's proper error handling
            // This is a placeholder for more sophisticated verification
            // In a full implementation, we would:
            // 1. Check if the result is destructured into Success and Error
            // 2. Verify both branches are non-empty
            // 3. Check that error handling is reachable and not contradictory
        }
    }

    fn find_ffi_calls_in_body(
        &self,
        body: &[Statement],
        ffi_bindings: &HashMap<String, ForeignSignature>,
    ) -> Vec<(String, String)> {
        let mut calls = Vec::new();

        for stmt in body {
            match stmt {
                Statement::Let { name: _, expr, .. } => {
                    if let Some(e) = expr {
                        self.find_ffi_calls_in_expr(e, &mut calls, ffi_bindings);
                    }
                }
                Statement::Assignment { expr, lhs, .. } => {
                    self.find_ffi_calls_in_expr(expr, &mut calls, ffi_bindings);
                    self.find_ffi_calls_in_expr(lhs, &mut calls, ffi_bindings);
                }
                Statement::Expression(e) => {
                    self.find_ffi_calls_in_expr(e, &mut calls, ffi_bindings);
                }
                Statement::Guarded { statements, .. } => {
                    calls.extend(self.find_ffi_calls_in_body(statements, ffi_bindings));
                }
                _ => {}
            }
        }

        calls
    }

    fn find_ffi_calls_in_expr(
        &self,
        expr: &Expr,
        calls: &mut Vec<(String, String)>,
        ffi_bindings: &HashMap<String, ForeignSignature>,
    ) {
        match expr {
            Expr::Call(name, _args) => {
                if ffi_bindings.contains_key(name) {
                    calls.push((name.clone(), "frgn call".to_string()));
                }
            }
            Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::BitAnd(l, r)
            | Expr::BitOr(l, r)
            | Expr::BitXor(l, r) => {
                self.find_ffi_calls_in_expr(l, calls, ffi_bindings);
                self.find_ffi_calls_in_expr(r, calls, ffi_bindings);
            }
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r) => {
                self.find_ffi_calls_in_expr(l, calls, ffi_bindings);
                self.find_ffi_calls_in_expr(r, calls, ffi_bindings);
            }
            Expr::And(l, r) | Expr::Or(l, r) => {
                self.find_ffi_calls_in_expr(l, calls, ffi_bindings);
                self.find_ffi_calls_in_expr(r, calls, ffi_bindings);
            }
            Expr::Not(inner) => self.find_ffi_calls_in_expr(inner, calls, ffi_bindings),
            Expr::Neg(inner) => self.find_ffi_calls_in_expr(inner, calls, ffi_bindings),
            Expr::BitNot(inner) => self.find_ffi_calls_in_expr(inner, calls, ffi_bindings),
            Expr::FieldAccess(inner, _) => self.find_ffi_calls_in_expr(inner, calls, ffi_bindings),
            Expr::ListIndex(list, index) => {
                self.find_ffi_calls_in_expr(list, calls, ffi_bindings);
                self.find_ffi_calls_in_expr(index, calls, ffi_bindings);
            }
            Expr::ListLen(list) => self.find_ffi_calls_in_expr(list, calls, ffi_bindings),
            _ => {}
        }
    }

    fn check_postcondition_contradictions(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.analyze_postcondition(txn);
            }
        }
    }

    fn check_trivial_contracts(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    let pre_is_trivial = matches!(&txn.contract.pre_condition, Expr::Bool(true));
                    let post_is_trivial = matches!(&txn.contract.post_condition, Expr::Bool(true));

                    if pre_is_trivial && post_is_trivial {
                        let mut err = ProofError::new("P009", "trivial precondition");
                        err.explanation = format!(
                            "transaction '{}' has precondition '[true]' which is always satisfied",
                            txn.name
                        );
                        err.proof_chain
                            .push("1. '[true]' accepts any state".to_string());
                        err.proof_chain
                            .push("2. this provides no compile-time safety".to_string());
                        err.hints.push(format!(
                            "specify what state is required before '{}' runs",
                            txn.name
                        ));
                        err.hints
                            .push("e.g., '[count > 0]' instead of '[true]'".to_string());
                        self.errors.push(err);

                        let mut err = ProofError::new("P010", "trivial postcondition");
                        err.explanation = format!(
                            "transaction '{}' has postcondition '[true]' which is always satisfied",
                            txn.name
                        );
                        err.proof_chain
                            .push("1. '[true]' accepts any state".to_string());
                        err.proof_chain
                            .push("2. this provides no compile-time safety".to_string());
                        err.hints.push(format!(
                            "specify what state '{}' guarantees after running",
                            txn.name
                        ));
                        err.hints
                            .push("e.g., '[count == @count + 1]' instead of '[true]'".to_string());
                        self.errors.push(err);
                    } else if pre_is_trivial {
                        let mut err = ProofError::new_warning("P009", "trivial precondition");
                        err.explanation = format!(
                            "transaction '{}' has precondition '[true]' which is always satisfied",
                            txn.name
                        );
                        err.proof_chain
                            .push("1. '[true]' accepts any state".to_string());
                        err.proof_chain
                            .push("2. consider specifying actual preconditions".to_string());
                        err.hints.push(format!(
                            "specify what state is required before '{}' runs",
                            txn.name
                        ));
                        err.hints
                            .push("e.g., '[count > 0]' instead of '[true]'".to_string());
                        self.errors.push(err);
                    } else if post_is_trivial {
                        let mut err = ProofError::new_warning("P010", "trivial postcondition");
                        err.explanation = format!(
                            "transaction '{}' has postcondition '[true]' which is always satisfied",
                            txn.name
                        );
                        err.proof_chain
                            .push("1. '[true]' accepts any state".to_string());
                        err.proof_chain
                            .push("2. consider specifying actual postconditions".to_string());
                        err.hints.push(format!(
                            "specify what state '{}' guarantees after running",
                            txn.name
                        ));
                        err.hints
                            .push("e.g., '[count == @count + 1]' instead of '[true]'".to_string());
                        self.errors.push(err);
                    }
                }
                TopLevel::Definition(defn) => {
                    let pre_is_trivial = matches!(&defn.contract.pre_condition, Expr::Bool(true));
                    let post_is_trivial = matches!(&defn.contract.post_condition, Expr::Bool(true));

                    if pre_is_trivial && post_is_trivial {
                        let mut err = ProofError::new("P009", "trivial precondition");
                        err.explanation = format!(
                            "definition '{}' has precondition '[true]' which is always satisfied",
                            defn.name
                        );
                        err.proof_chain
                            .push("1. '[true]' accepts any state".to_string());
                        err.proof_chain
                            .push("2. this provides no compile-time safety".to_string());
                        err.hints.push(format!(
                            "specify what state is required before '{}' runs",
                            defn.name
                        ));
                        err.hints
                            .push("e.g., '[x > 0]' instead of '[true]'".to_string());
                        self.errors.push(err);

                        let mut err = ProofError::new("P010", "trivial postcondition");
                        err.explanation = format!(
                            "definition '{}' has postcondition '[true]' which is always satisfied",
                            defn.name
                        );
                        err.proof_chain
                            .push("1. '[true]' accepts any state".to_string());
                        err.proof_chain
                            .push("2. this provides no compile-time safety".to_string());
                        err.hints.push(format!(
                            "specify what state '{}' guarantees after running",
                            defn.name
                        ));
                        err.hints
                            .push("e.g., '[result > 0]' instead of '[true]'".to_string());
                        self.errors.push(err);
                    } else if pre_is_trivial {
                        let mut err = ProofError::new_warning("P009", "trivial precondition");
                        err.explanation = format!(
                            "definition '{}' has precondition '[true]' which is always satisfied",
                            defn.name
                        );
                        err.proof_chain
                            .push("1. '[true]' accepts any state".to_string());
                        err.proof_chain
                            .push("2. consider specifying actual preconditions".to_string());
                        err.hints.push(format!(
                            "specify what state is required before '{}' runs",
                            defn.name
                        ));
                        err.hints
                            .push("e.g., '[x > 0]' instead of '[true]'".to_string());
                        self.errors.push(err);
                    } else if post_is_trivial {
                        let mut err = ProofError::new_warning("P010", "trivial postcondition");
                        err.explanation = format!(
                            "definition '{}' has postcondition '[true]' which is always satisfied",
                            defn.name
                        );
                        err.proof_chain
                            .push("1. '[true]' accepts any state".to_string());
                        err.proof_chain
                            .push("2. consider specifying actual postconditions".to_string());
                        err.hints.push(format!(
                            "specify what state '{}' guarantees after running",
                            defn.name
                        ));
                        err.hints
                            .push("e.g., '[result > 0]' instead of '[true]'".to_string());
                        self.errors.push(err);
                    }
                }
                _ => {}
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
            | Expr::BitAnd(l, r)
            | Expr::BitOr(l, r)
            | Expr::BitXor(l, r)
            | Expr::Shl(l, r)
            | Expr::Shr(l, r)
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
            Expr::StructInstance(_, fields) => {
                for (_, expr) in fields {
                    self.collect_identifiers(expr, vars);
                }
            }
            Expr::ObjectLiteral(fields) => {
                for (_, v) in fields {
                    self.collect_identifiers(v, vars);
                }
            }
            Expr::PatternMatch { value, .. } => {
                self.collect_identifiers(value, vars);
            }
            Expr::Slice { .. } | Expr::ForAll { .. } | Expr::Exists { .. } => {}
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
                    ResultType::VoidType => {}
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
            Type::Sig(name) => format!("sig {}", name),
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
            Type::Option(inner) => {
                format!("Option<{}>", self.type_name(inner))
            }
            Type::Enum(name) => name.clone(),
            Type::UInt => "UInt".to_string(),
            Type::Vector(inner, size) => format!("Vector<{}>[{}]", self.type_name(inner), size),
            Type::Constrained(inner, _) => self.type_name(inner),
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

                let conflicts = self.find_read_write_conflicts(txn1, txn2);
                if !conflicts.is_empty() {
                    let pre1_overlaps = self.preconditions_overlap(txn1, txn2);
                    if pre1_overlaps {
                        for (var, description) in &conflicts {
                            let mut err =
                                ProofError::new("P001", "ownership conflict in async transactions");
                            err.explanation = format!(
                                "transactions '{}' and '{}' have conflicting access to '{}'",
                                txn1.name, txn2.name, var
                            );
                            err.proof_chain.push(format!(
                                "1. '{}' is async reactive (can run concurrently)",
                                txn1.name
                            ));
                            err.proof_chain.push(format!(
                                "2. '{}' is async reactive (can run concurrently)",
                                txn2.name
                            ));
                            err.proof_chain.push(format!("3. {}", description));
                            err.proof_chain.push(
                                "4. Brief: when one writes, no other may read or write".to_string(),
                            );
                            err.hints
                                .push("make pre-conditions mutually exclusive".to_string());
                            self.errors.push(err);
                        }
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

    fn find_read_write_conflicts(
        &self,
        txn1: &Transaction,
        txn2: &Transaction,
    ) -> Vec<(String, String)> {
        let mut conflicts = Vec::new();

        let writes1 = self.extract_write_vars(txn1);
        let reads1 = self.extract_read_vars(txn1);
        let writes2 = self.extract_write_vars(txn2);
        let reads2 = self.extract_read_vars(txn2);

        for w in &writes1 {
            if writes2.contains(w) {
                conflicts.push((
                    w.clone(),
                    format!("{} writes while {} writes", txn2.name, txn1.name),
                ));
            }
        }

        for w in &writes1 {
            if reads2.contains(w) {
                conflicts.push((
                    w.clone(),
                    format!("{} reads while {} writes", txn2.name, txn1.name),
                ));
            }
        }

        for w in &writes2 {
            if reads1.contains(w) {
                conflicts.push((
                    w.clone(),
                    format!("{} reads while {} writes", txn1.name, txn2.name),
                ));
            }
        }

        conflicts
    }

    fn extract_read_vars(&self, txn: &Transaction) -> HashSet<String> {
        let mut vars = HashSet::new();
        for stmt in &txn.body {
            self.collect_read_vars(stmt, &mut vars);
        }
        vars
    }

    fn collect_read_vars_from_expr(&self, expr: &Expr, vars: &mut HashSet<String>) {
        match expr {
            Expr::Identifier(name) => {
                vars.insert(name.clone());
            }
            Expr::PriorState(name) => {
                vars.insert(name.clone());
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.collect_read_vars_from_expr(l, vars);
                self.collect_read_vars_from_expr(r, vars);
            }
            Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r) => {
                self.collect_read_vars_from_expr(l, vars);
                self.collect_read_vars_from_expr(r, vars);
            }
            Expr::And(l, r) | Expr::Or(l, r) => {
                self.collect_read_vars_from_expr(l, vars);
                self.collect_read_vars_from_expr(r, vars);
            }
            Expr::Not(inner) => self.collect_read_vars_from_expr(inner, vars),
            _ => {}
        }
    }

    fn collect_read_vars(&self, stmt: &Statement, vars: &mut HashSet<String>) {
        match stmt {
            Statement::Assignment {
                lhs,
                expr,
                timeout: _,
            } => {
                self.collect_read_vars_from_expr(expr, vars);
                self.collect_read_vars_from_expr(lhs, vars);
            }
            Statement::Let { name, expr, .. } => {
                if let Some(e) = expr {
                    self.collect_read_vars_from_expr(e, vars);
                }
            }
            Statement::Expression(expr) => {
                self.collect_read_vars_from_expr(expr, vars);
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                self.collect_read_vars_from_expr(condition, vars);
                for stmt in statements {
                    self.collect_read_vars(stmt, vars);
                }
            }
            Statement::Term(outputs) => {
                for out in outputs {
                    if let Some(expr) = out {
                        self.collect_read_vars_from_expr(expr, vars);
                    }
                }
            }
            _ => {}
        }
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
            Statement::Assignment { lhs, .. } => {
                if let Expr::OwnedRef(name) = lhs {
                    vars.insert(name.clone());
                } else if let Expr::ListIndex(inner, _) = lhs {
                    if let Expr::OwnedRef(name) = &**inner {
                        vars.insert(name.clone());
                    }
                }
            }
            Statement::Let { .. } => {}
            Statement::Expression(_) => {}
            Statement::Term(_) => {}
            Statement::Escape(_) => {}
            Statement::Guarded { statements, .. } => {
                for stmt in statements {
                    self.collect_write_vars(stmt, vars);
                }
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
                Statement::Guarded { statements, .. } => {
                    if self.has_term_statement(statements) {
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
                    // Try to resolve the source definition
                    let source_name = sig.source.as_ref().unwrap_or(&sig.name);

                    if let Some(defn) = defns.get(source_name) {
                        // Use Feature C assertion verification
                        match crate::assertion_verify::verify_true_assertion(sig, defn) {
                            Ok(()) => {
                                // Assertion verified successfully
                            }
                            Err(err_msg) => {
                                let mut proof_err =
                                    ProofError::new("C001", "true assertion verification failed");
                                proof_err.explanation = format!(
                                    "Signature '{}' asserts '-> true' but verification failed: {}",
                                    sig.name, err_msg
                                );
                                proof_err.proof_chain.push(format!(
                                    "1. Signature '{}' declares it returns Bool = true",
                                    sig.name
                                ));
                                proof_err.proof_chain.push(format!(
                                    "2. Definition '{}' was analyzed for this assertion",
                                    defn.name
                                ));
                                proof_err
                                    .proof_chain
                                    .push(format!("3. Verification failure: {}", err_msg));
                                proof_err.hints.push(
                                    "ensure all execution paths produce Bool = true".to_string(),
                                );
                                self.errors.push(proof_err);
                            }
                        }

                        // Also run the old verification logic for compatibility
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
                    statements,
                } => {
                    self.collect_term_values(statements, results);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutual_exclusion_detects_conflict() {
        let code = r#"
            let data: String = "";
            let busy: Bool = false;

            rct async txn write_a [ready && !busy][busy == true] {
                &data = "A";
                &busy = false;
                term;
            };

            rct async txn write_b [ready && !busy][busy == true] {
                &data = "B";
                &busy = false;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_ownership_conflict = errors.iter().any(|e| e.code == "P001");
        assert!(
            has_ownership_conflict,
            "Expected P001 ownership conflict error, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_mutual_exclusion_no_conflict_different_vars() {
        let code = r#"
            let a: Int = 0;
            let b: Int = 0;

            rct async txn inc_a [true][a == @a + 1] {
                &a = a + 1;
                term;
            };

            rct async txn inc_b [true][b == @b + 1] {
                &b = b + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_ownership_conflict = errors.iter().any(|e| e.code == "P001");
        assert!(
            !has_ownership_conflict,
            "Should NOT have ownership conflict for different variables, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_mutual_exclusion_no_conflict_non_async() {
        let code = r#"
            let data: String = "";

            txn write_a [true] {
                &data = "A";
                term;
            };

            txn write_b [true] {
                &data = "B";
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_ownership_conflict = errors.iter().any(|e| e.code == "P001");
        assert!(
            !has_ownership_conflict,
            "Should NOT have ownership conflict for non-async txns, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_trivial_precondition_with_non_trivial_post_warning() {
        let code = r#"
            let count: Int = 0;

            txn increment [true][count == @count + 1] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_trivial_pre_warning = errors.iter().any(|e| e.code == "P009" && e.is_warning);
        let has_trivial_pre_error = errors.iter().any(|e| e.code == "P009" && !e.is_warning);
        assert!(
            has_trivial_pre_warning && !has_trivial_pre_error,
            "Expected P009 warning (not error) when post is non-trivial, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_trivial_postcondition_with_non_trivial_pre_warning() {
        let code = r#"
            let count: Int = 0;

            txn increment [count >= 0][true] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_trivial_post_warning = errors.iter().any(|e| e.code == "P010" && e.is_warning);
        let has_trivial_post_error = errors.iter().any(|e| e.code == "P010" && !e.is_warning);
        assert!(
            has_trivial_post_warning && !has_trivial_post_error,
            "Expected P010 warning (not error) when pre is non-trivial, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_trivial_contracts_both_true() {
        let code = r#"
            let count: Int = 0;

            txn increment [true][true] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_trivial_pre = errors.iter().any(|e| e.code == "P009");
        let has_trivial_post = errors.iter().any(|e| e.code == "P010");
        assert!(
            has_trivial_pre && has_trivial_post,
            "Expected both P009 and P010 errors, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_non_trivial_contracts_no_error() {
        let code = r#"
            let count: Int = 0;

            txn increment [count >= 0][count == @count + 1] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_trivial_pre = errors.iter().any(|e| e.code == "P009");
        let has_trivial_post = errors.iter().any(|e| e.code == "P010");
        assert!(
            !has_trivial_pre && !has_trivial_post,
            "Should NOT have trivial contract errors, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_trivial_contracts_in_definition() {
        let code = r#"
            defn double(x: Int) -> Int [true][true] {
                term x * 2;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_trivial_pre = errors.iter().any(|e| e.code == "P009");
        let has_trivial_post = errors.iter().any(|e| e.code == "P010");
        assert!(
            has_trivial_pre && has_trivial_post,
            "Expected both P009 and P010 errors for definition, got: {:?}",
            errors
        );
    }
}
