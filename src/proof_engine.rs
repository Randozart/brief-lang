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
pub enum PathKind {
    Term(Vec<Option<Expr>>),
    Escape,
}

#[derive(Debug, Clone)]
pub struct SymbolicState {
    pub vars: HashMap<String, SymbolicValue>,
    pub constraints: Vec<PathConstraint>,
    /// Variables marked as triggers (volatile) - each read creates a new symbolic value
    pub volatile_vars: HashSet<String>,
    /// Counter for generating unique symbolic variable names for volatile reads
    pub volatile_read_counter: HashMap<String, usize>,
}

impl SymbolicState {
    pub fn new() -> Self {
        SymbolicState {
            vars: HashMap::new(),
            constraints: Vec::new(),
            volatile_vars: HashSet::new(),
            volatile_read_counter: HashMap::new(),
        }
    }

    /// Mark a variable as volatile (trigger-marked). Reads will create new symbolic values.
    pub fn mark_volatile(&mut self, name: &str) {
        self.volatile_vars.insert(name.to_string());
    }

    /// Check if a variable is volatile (trigger-marked)
    pub fn is_volatile(&self, name: &str) -> bool {
        self.volatile_vars.contains(name)
    }

    /// Get the symbolic value for a variable. For volatile variables, creates a new
    /// symbolic value each time (simulating that the value may have changed).
    pub fn get_value(&mut self, name: &str) -> SymbolicValue {
        if self.is_volatile(name) {
            // Each read of a volatile variable gets a unique symbolic name
            let counter = self.volatile_read_counter.entry(name.to_string()).or_insert(0);
            *counter += 1;
            let volatile_name = format!("{}@t{}", name, counter);
            SymbolicValue::Symbolic(volatile_name)
        } else {
            // Stable variable: return the stored value or create a symbolic one
            self.vars
                .get(name)
                .cloned()
                .unwrap_or_else(|| SymbolicValue::Symbolic(name.to_string()))
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
    /// Variables marked as triggers (volatile) - each read creates a new symbolic value
    volatile_vars: HashSet<String>,
}

impl SymbolicExecutor {
    pub fn new() -> Self {
        SymbolicExecutor { errors: Vec::new(), volatile_vars: HashSet::new() }
    }

    /// Set the set of volatile (trigger-marked) variable names
    pub fn with_volatile_vars(mut self, vars: HashSet<String>) -> Self {
        self.volatile_vars = vars;
        self
    }

    pub fn verify_transaction(&mut self, txn: &Transaction) -> Vec<ProofError> {
        if txn.is_lambda {
            let pre = &txn.contract.pre_condition;
            let post = &txn.contract.post_condition;

            let mut state = self.init_state_from_precondition(pre);
            // Mark volatile (trigger) variables
            for var in &self.volatile_vars {
                state.mark_volatile(var);
            }
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
            for var in &self.volatile_vars {
                state.mark_volatile(var);
            }
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
            for var in &self.volatile_vars {
                state.mark_volatile(var);
            }
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
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r) => {
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

        for (path_idx, (path_state, path_kind)) in term_paths.iter().enumerate() {
            // Escape paths cancel the transaction - postconditions are vacuously satisfied
            if let PathKind::Escape = path_kind {
                continue;
            }

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
    ) -> Vec<(SymbolicState, PathKind)> {
        let mut paths = Vec::new();
        self.enumerate_paths_recursive(body, state, &mut paths);
        paths
    }

    fn enumerate_paths_recursive(
        &self,
        body: &[Statement],
        state: SymbolicState,
        paths: &mut Vec<(SymbolicState, PathKind)>,
    ) {
        let mut current_state = state;
        let mut terminated = false;
        let mut path_kind: PathKind = PathKind::Term(Vec::new());

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
                Statement::InlineAsm { .. } => {}
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

                    for (s, pk) in true_paths.into_iter().chain(false_paths.into_iter()) {
                        paths.push((s, pk));
                    }
                    return;
                }
                Statement::Term(outputs) => {
                    terminated = true;
                    path_kind = PathKind::Term(outputs.clone());
                }
                Statement::Escape(_) => {
                    terminated = true;
                    path_kind = PathKind::Escape;
                }
                Statement::Expression(_) | Statement::Unification { .. } | Statement::LocalTrigger { .. } => {}
            }
        }

        if terminated {
            paths.push((current_state, path_kind));
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
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r) => {
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
                // Volatile (trigger) variables are never provably truthy
                if state.is_volatile(name) {
                    return false;
                }
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
        // Two reads of the same volatile variable are NOT guaranteed equal
        if let (Expr::Identifier(ln), Expr::Identifier(rn)) = (l, r) {
            if ln == rn && state.is_volatile(ln) {
                return false; // Volatile: each read may differ
            }
        }
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
                // Volatile (trigger) variables are never concretely evaluable
                if state.is_volatile(name) {
                    return None;
                }
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
            Expr::Mod(l, r) => format!("{} % {}", self.format_expr(l), self.format_expr(r)),
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
        self.check_circular_dependencies(program);
        self.check_list_simd_lengths(program);
        self.verify_contracts(program);
        self.errors.clone()
    }

    fn verify_contracts(&mut self, program: &Program) {
        // Collect all trigger variable names (volatile variables)
        let mut volatile_vars = HashSet::new();
        for item in &program.items {
            if let TopLevel::Trigger(trg) = item {
                volatile_vars.insert(trg.name.clone());
            }
        }

        let mut sym_exec = SymbolicExecutor::new().with_volatile_vars(volatile_vars);

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
        let ffi_calls = self.find_ffi_calls_in_body(&defn.body, ffi_bindings);

        for (frgn_name, _call_context) in ffi_calls {
            let result_var = self.find_ffi_result_variable(&defn.body, &frgn_name);
            if let Some(var_name) = result_var {
                self.verify_ffi_result_handling(defn, &var_name, &frgn_name);
            }
        }
    }

    fn find_ffi_result_variable(&self, body: &[Statement], target_fn: &str) -> Option<String> {
        for stmt in body {
            match stmt {
                Statement::Let { name, expr, .. } => {
                    if let Some(e) = expr {
                        if let Expr::Call(fn_name, _) = e {
                            if fn_name == target_fn {
                                return Some(name.clone());
                            }
                        }
                    }
                }
                Statement::Assignment { lhs, expr, .. } => {
                    if let Expr::Call(fn_name, _) = expr {
                        if fn_name == target_fn {
                            if let Expr::Identifier(name) = lhs {
                                return Some(name.clone());
                            }
                        }
                    }
                }
                Statement::Guarded { statements, .. } => {
                    return self.find_ffi_result_variable(statements, target_fn);
                }
                _ => {}
            }
        }
        None
    }

    fn verify_ffi_result_handling(&mut self, defn: &Definition, result_var: &str, frgn_name: &str) {
        let mut has_success_path = false;
        let mut has_error_path = false;
        let mut success_terminates = false;
        let mut error_terminates = false;

        self.check_ffi_branch_handling(&defn.body, result_var, &mut has_success_path, &mut has_error_path, &mut success_terminates, &mut error_terminates);

        if !has_success_path {
            let mut err = ProofError::new("F101", "FFI call missing success handling");
            err.explanation = format!(
                "FFI call '{}' result '{}' has no success branch handling",
                frgn_name, result_var
            );
            err.proof_chain.push(format!("1. '{}' returns Result<T, Error>", frgn_name));
            err.proof_chain.push(format!("2. caller '{}' must handle both Success and Error", defn.name));
            err.hints.push("add a success branch (e.g., let Success(val) = result;)".to_string());
            self.errors.push(err);
        }

        if !has_error_path {
            let mut err = ProofError::new("F102", "FFI call missing error handling");
            err.explanation = format!(
                "FFI call '{}' result '{}' has no error branch handling",
                frgn_name, result_var
            );
            err.proof_chain.push(format!("1. '{}' returns Result<T, Error>", frgn_name));
            err.proof_chain.push(format!("2. caller '{}' must handle both Success and Error", defn.name));
            err.hints.push("add an error branch (e.g., let Error(e) = result;)".to_string());
            self.errors.push(err);
        }

        if has_success_path && has_error_path && !success_terminates && !error_terminates {
            let mut err = ProofError::new_warning("F103", "FFI result may not be properly terminated");
            err.explanation = format!(
                "FFI call '{}' has both branches but neither terminates (escape/term)",
                frgn_name
            );
            err.proof_chain.push("1. both branches should either escape or return".to_string());
            err.proof_chain.push("2. otherwise the remaining code may execute unexpectedly".to_string());
            err.hints.push("ensure each branch either escapes or has a term statement".to_string());
            self.errors.push(err);
        }
    }

    fn check_ffi_branch_handling(
        &self,
        body: &[Statement],
        result_var: &str,
        has_success: &mut bool,
        has_error: &mut bool,
        success_terminates: &mut bool,
        error_terminates: &mut bool,
    ) {
        for stmt in body {
            match stmt {
                Statement::Guarded { condition, statements } => {
                    if self.is_success_variant_check(condition, result_var) {
                        *has_success = true;
                        self.check_branch_terminates(statements, success_terminates);
                    } else if self.is_error_variant_check(condition, result_var) {
                        *has_error = true;
                        self.check_branch_terminates(statements, error_terminates);
                    } else {
                        self.check_ffi_branch_handling(statements, result_var, has_success, has_error, success_terminates, error_terminates);
                    }
                }
                _ => {}
            }
        }
    }

    fn is_success_variant_check(&self, condition: &Expr, _result_var: &str) -> bool {
        match condition {
            Expr::Call(name, args) => {
                name == "Success" || name == "Ok" || (name == "is_ok" && args.is_empty())
            }
            _ => false
        }
    }

    fn is_error_variant_check(&self, condition: &Expr, _result_var: &str) -> bool {
        match condition {
            Expr::Call(name, args) => {
                name == "Error" || name == "Err" || (name == "is_err" && args.is_empty())
            }
            _ => false
        }
    }

    fn check_branch_terminates(&self, statements: &[Statement], terminates: &mut bool) {
        for stmt in statements {
            match stmt {
                Statement::Term(_) => {
                    *terminates = true;
                    return;
                }
                Statement::Escape(_) => {
                    *terminates = true;
                    return;
                }
                Statement::Guarded { statements: inner, .. } => {
                    self.check_branch_terminates(inner, terminates);
                }
                _ => {}
            }
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
                        // BOTH trivial - hard error
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
                        // Only precondition trivial - warning (post is non-trivial)
                        let mut err = ProofError::new_warning("P009", "trivial precondition");
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
                    } else if post_is_trivial {
                        // Only postcondition trivial - warning (pre is non-trivial)
                        let mut err = ProofError::new_warning("P010", "trivial postcondition");
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

    /// Check that List SIMD operations have provable length equality
    /// When two Lists are used in a binary operation (e.g., list_a * list_b),
    /// their lengths must be provably equal from the precondition.
    fn check_list_simd_lengths(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    self.check_list_simd_lengths_in_body(&txn.body, &txn.contract.pre_condition, &format!("transaction '{}'", txn.name));
                }
                TopLevel::Definition(defn) => {
                    self.check_list_simd_lengths_in_body(&defn.body, &defn.contract.pre_condition, &format!("definition '{}'", defn.name));
                }
                _ => {}
            }
        }
    }

    fn check_list_simd_lengths_in_body(&mut self, body: &[Statement], precondition: &Expr, context: &str) {
        let mut list_ops = Vec::new();
        self.collect_list_simd_ops(body, &mut list_ops);

        for (left, right, span) in list_ops {
            // Extract list names from the expressions
            let left_list = self.extract_list_name(&left);
            let right_list = self.extract_list_name(&right);

            if let (Some(left_name), Some(right_name)) = (left_list, right_list) {
                if left_name != right_name {
                    // Different lists - check if length equality is provable from precondition
                    let len_expr = Expr::Eq(
                        Box::new(Expr::ListLen(Box::new(Expr::Identifier(left_name.clone())))),
                        Box::new(Expr::ListLen(Box::new(Expr::Identifier(right_name.clone())))),
                    );

                    // Check if precondition implies length equality
                    if !self.expr_implies(precondition, &len_expr) {
                        let mut err = ProofError::new("P020", "List SIMD length mismatch");
                        err.explanation = format!(
                            "{}: SIMD operation between '{}' and '{}' requires provable length equality",
                            context, left_name, right_name
                        );
                        err.proof_chain.push(format!("1. '{}' and '{}' are different lists", left_name, right_name));
                        err.proof_chain.push("2. Length equality cannot be proven from precondition".to_string());
                        err.hints.push(format!("Add precondition: ['{}.len() == {}.len()']", left_name, right_name));
                        err.hints.push("Or use slicing to ensure equal lengths: let safe_a = a[0..min(a.len(), b.len())]".to_string());
                        if let Some(s) = span {
                            err = err.with_span(s.clone());
                        }
                        self.errors.push(err);
                    }
                }
            }
        }
    }

    fn extract_list_name(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(name) => Some(name.clone()),
            Expr::FieldAccess(obj, _) => self.extract_list_name(obj),
            Expr::ListIndex(inner, _) => self.extract_list_name(inner),
            _ => None,
        }
    }

    /// Check if precondition implies the given expression
    /// Simple check: if the expression or its negation appears in the precondition
    fn expr_implies(&self, precondition: &Expr, target: &Expr) -> bool {
        // Direct match
        if self.exprs_equal(precondition, target) {
            return true;
        }

        // Check if target is part of an AND chain in precondition
        if let Expr::And(left, right) = precondition {
            return self.expr_implies(left, target) || self.expr_implies(right, target);
        }

        false
    }

    fn exprs_equal(&self, a: &Expr, b: &Expr) -> bool {
        match (a, b) {
            (Expr::Eq(l1, r1), Expr::Eq(l2, r2)) => {
                self.exprs_equal(l1, l2) && self.exprs_equal(r1, r2)
            }
            (Expr::ListLen(l1), Expr::ListLen(l2)) => self.exprs_equal(l1, l2),
            (Expr::Identifier(n1), Expr::Identifier(n2)) => n1 == n2,
            _ => false,
        }
    }

    fn collect_list_simd_ops(&self, body: &[Statement], ops: &mut Vec<(Expr, Expr, Option<Span>)>) {
        for stmt in body {
            match stmt {
                Statement::Assignment { expr, .. } => {
                    self.collect_list_simd_ops_in_expr(expr, ops);
                }
                Statement::Guarded { condition, statements, .. } => {
                    self.collect_list_simd_ops_in_expr(condition, ops);
                    self.collect_list_simd_ops(statements, ops);
                }
                _ => {}
            }
        }
    }

    fn collect_list_simd_ops_in_expr(&self, expr: &Expr, ops: &mut Vec<(Expr, Expr, Option<Span>)>) {
        match expr {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r) => {
                // Check if both sides could be List types
                if self.could_be_list(l) && self.could_be_list(r) {
                    ops.push((l.as_ref().clone(), r.as_ref().clone(), None));
                }
                self.collect_list_simd_ops_in_expr(l, ops);
                self.collect_list_simd_ops_in_expr(r, ops);
            }
            Expr::ListLiteral(elems) => {
                for elem in elems {
                    self.collect_list_simd_ops_in_expr(elem, ops);
                }
            }
            Expr::Slice { value, mask, .. } => {
                self.collect_list_simd_ops_in_expr(value, ops);
                if let Some(m) = mask {
                    self.collect_list_simd_ops_in_expr(m, ops);
                }
            }
            Expr::MultiSlice { value, mask, .. } => {
                self.collect_list_simd_ops_in_expr(value, ops);
                if let Some(m) = mask {
                    self.collect_list_simd_ops_in_expr(m, ops);
                }
            }
            _ => {}
        }
    }

    fn could_be_list(&self, expr: &Expr) -> bool {
        // Only consider expressions that are clearly list-like
        // This is conservative to avoid false positives on scalar types
        match expr {
            Expr::ListLiteral(_) => true,
            Expr::ListIndex(inner, _) => self.could_be_list(inner),
            Expr::Slice { .. } | Expr::MultiSlice { .. } => true,
            // Don't assume identifiers are lists - they could be scalars
            // Only flag if we see explicit list operations
            _ => false,
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
            | Expr::Mod(l, r)
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
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::Char(_) | Expr::Bool(_) => {}
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
            Expr::Slice { .. } | Expr::MultiSlice { .. } | Expr::ForAll { .. } | Expr::Exists { .. } | Expr::Block(_, _) | Expr::TupleDestructure(_, _) | Expr::Tuple(_) => {}
        }
    }

    fn check_exhaustiveness(&mut self, program: &Program) {
        let mut sig_outputs: HashMap<String, usize> = HashMap::new();
        let mut sig_callers: HashMap<String, Vec<(String, String)>> = HashMap::new();

        for item in &program.items {
            if let TopLevel::Signature(sig) = item {
                let output_count = match &sig.result_type {
                    ResultType::Projection(types) => types.len(),
                    ResultType::TrueAssertion => 1,
                    ResultType::VoidType => 0,
                };
                sig_outputs.insert(sig.name.clone(), output_count);
            }
        }

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.find_sig_unifications_in_body(&txn.body, &txn.name, &mut sig_callers);
            }
            if let TopLevel::Definition(defn) = item {
                self.find_sig_unifications_in_body(&defn.body, &defn.name, &mut sig_callers);
            }
        }

        for (sig_name, callers) in &sig_callers {
            if let Some(total_outputs) = sig_outputs.get(sig_name) {
                if *total_outputs <= 1 {
                    continue;
                }

                let mut handled_outputs: HashSet<usize> = HashSet::new();
                for (caller, pattern) in callers {
                    if let Some(idx) = pattern.parse::<usize>().ok() {
                        handled_outputs.insert(idx);
                    }
                }

                let mut missing = Vec::new();
                for i in 0..*total_outputs {
                    if !handled_outputs.contains(&i) {
                        missing.push(i);
                    }
                }

                if !missing.is_empty() {
                    let mut err = ProofError::new("P011", "unhandled signature output");
                    err.explanation = format!(
                        "signature '{}' has {} outputs but callers only handle: {:?}. Missing: {:?}",
                        sig_name, total_outputs, handled_outputs, missing
                    );
                    err.proof_chain.push(format!(
                        "1. '{}' is a multi-output signature with {} outputs",
                        sig_name, total_outputs
                    ));
                    for (caller, pattern) in callers {
                        err.proof_chain.push(format!(
                            "2. caller '{}' handles output {}",
                            caller, pattern
                        ));
                    }
                    err.proof_chain.push(format!(
                        "3. outputs {:?} are not handled by any caller",
                        missing
                    ));
                    err.hints.push(format!(
                        "ensure all outputs from '{}' are handled via unification",
                        sig_name
                    ));
                    err.hints.push("e.g., 'call_sig(output_idx)' for each output index".to_string());
                    self.errors.push(err);
                }
            }
        }
    }

    fn find_sig_unifications_in_body(
        &self,
        body: &[Statement],
        caller_name: &str,
        sig_callers: &mut HashMap<String, Vec<(String, String)>>,
    ) {
        for stmt in body {
            match stmt {
                Statement::Unification {
                    name,
                    pattern,
                    expr,
                } => {
                    if let Expr::Call(sig_name, _) = expr {
                        sig_callers
                            .entry(sig_name.clone())
                            .or_insert_with(Vec::new)
                            .push((caller_name.to_string(), pattern.clone()));
                    }
                }
                Statement::Guarded { statements, .. } => {
                    self.find_sig_unifications_in_body(statements, caller_name, sig_callers);
                }
                _ => {}
            }
        }
    }

    fn check_circular_dependencies(&mut self, program: &Program) {
        let mut call_graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_txn_names: HashSet<String> = HashSet::new();
        let mut txn_spans: HashMap<String, Option<Span>> = HashMap::new();

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                all_txn_names.insert(txn.name.clone());
                txn_spans.insert(txn.name.clone(), txn.span);
                let called_txns = self.extract_called_transactions(&txn.body);
                call_graph.entry(txn.name.clone()).or_insert_with(Vec::new).extend(called_txns);
            }
        }

        for txn_name in &all_txn_names {
            let mut visited: HashSet<String> = HashSet::new();
            let mut path: Vec<String> = Vec::new();
            if self.detect_cycle(txn_name, &call_graph, &mut visited, &mut path) {
                let mut err = ProofError::new("P012", "circular transaction dependency");
                err.explanation = format!(
                    "transactions form a circular dependency: {}",
                    path.join(" -> ")
                );
                err.proof_chain.push("1. transaction call cycle detected".to_string());
                for (i, name) in path.iter().enumerate() {
                    err.proof_chain.push(format!("{}. {}", i + 2, name));
                }
                err.proof_chain.push(format!("{}. (cycle closes back to {})", path.len() + 2, txn_name));
                err.hints.push("break the cycle by removing or reordering calls".to_string());
                if let Some(span) = txn_spans.get(txn_name).and_then(|s| *s) {
                    err.span = Some(span);
                }
                self.errors.push(err);
                break;
            }
        }
    }

    fn extract_called_transactions(&self, body: &[Statement]) -> Vec<String> {
        let mut called = Vec::new();
        for stmt in body {
            match stmt {
                Statement::Assignment { expr, .. } => {
                    self.collect_call_names(expr, &mut called);
                }
                Statement::Let { expr, .. } => {
                    if let Some(e) = expr {
                        self.collect_call_names(e, &mut called);
                    }
                }
                Statement::Expression(e) => {
                    self.collect_call_names(e, &mut called);
                }
                Statement::Guarded { statements, .. } => {
                    called.extend(self.extract_called_transactions(statements));
                }
                _ => {}
            }
        }
        called
    }

    fn collect_call_names(&self, expr: &Expr, called: &mut Vec<String>) {
        match expr {
            Expr::Call(name, args) => {
                called.push(name.clone());
                for arg in args {
                    self.collect_call_names(arg, called);
                }
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r) => {
                self.collect_call_names(l, called);
                self.collect_call_names(r, called);
            }
            Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r) => {
                self.collect_call_names(l, called);
                self.collect_call_names(r, called);
            }
            Expr::And(l, r) | Expr::Or(l, r) => {
                self.collect_call_names(l, called);
                self.collect_call_names(r, called);
            }
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => {
                self.collect_call_names(e, called);
            }
            Expr::FieldAccess(e, _) => {
                self.collect_call_names(e, called);
            }
            Expr::ListLiteral(elems) => {
                for elem in elems {
                    self.collect_call_names(elem, called);
                }
            }
            _ => {}
        }
    }

    fn detect_cycle(
        &self,
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        let node_str = node.to_string();
        if path.iter().any(|n| *n == node_str) {
            if let Some(pos) = path.iter().position(|n| *n == node_str) {
                let cycle_start = pos;
                path.push(node_str.clone());
                for i in cycle_start..path.len() {
                    if path[i] == node_str && i > cycle_start {
                        return true;
                    }
                }
                path.pop();
            }
            return true;
        }

        if visited.contains(node) {
            return false;
        }

        visited.insert(node.to_string());
        path.push(node.to_string());

        if let Some(edges) = graph.get(node) {
            for next in edges {
                if self.detect_cycle(next, graph, visited, path) {
                    return true;
                }
            }
        }

        path.pop();
        false
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
            Type::Tuple(types) => format!("({})", types
                .iter()
                .map(|t| self.type_name(t))
                .collect::<Vec<_>>()
                .join(", ")),
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
            Type::Enum(name) => name.clone(),
            Type::UInt => "UInt".to_string(),
            Type::Char => "Char".to_string(),
            // Note: HashMap, HashSet, StringBuilder, Stack, Queue, Option
            // are regular structs/enums defined in stdlib, handled via
            // Custom/Applied/Enum variants.
            Type::Vector(inner, dims) => {
                let dims_str: Vec<String> = dims.iter().map(|d| match d {
                    crate::ast::Dimension::Anonymous(s) => format!("{}", s),
                    crate::ast::Dimension::Named(n, s) => format!("{}:{}", n, s),
                }).collect();
                format!("Vector<{}, {}>", self.type_name(inner), dims_str.join(", "))
            }
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
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r) => {
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
                Statement::InlineAsm { .. } => {}
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
            Statement::InlineAsm { .. } => {}
            Statement::Expression(_) => {}
            Statement::Term(_) => {}
            Statement::Escape(_) => {}
            Statement::Guarded { statements, .. } => {
                for stmt in statements {
                    self.collect_write_vars(stmt, vars);
                }
            }
            Statement::Unification { .. } => {}
            Statement::LocalTrigger { .. } => {}
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

    #[test]
    fn test_escape_skips_postcondition_check() {
        let code = r#"
            let count: Int = 0;

            txn maybe_increment [count >= 0][count == @count + 1] {
                [count > 0] { &count = count + 1; term; };
                escape;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_contract_error = errors.iter().any(|e| e.code == "P008");
        assert!(
            !has_contract_error,
            "Escape path should not trigger postcondition check, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_term_path_still_checked() {
        // Term paths should still be checked against postcondition
        // Using a case the symbolic executor can detect
        let code = r#"
            let count: Int = 0;

            txn bad_increment [count >= 0][count == 0] {
                [count > 0] { &count = 5; term; };
                escape;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // The term path sets count = 5, which violates count == 0
        // The symbolic executor should detect this
        let has_contract_error = errors.iter().any(|e| e.code == "P008");
        assert!(
            has_contract_error,
            "Term path should be checked against postcondition, got: {:?}",
            errors
        );
    }

    #[test]
    fn test_rct_txn_no_parameters() {
        let code = r#"
            let count: Int = 0;

            rct txn bad_rct(x: Int) [true][true] {
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let result = parser.parse();

        assert!(
            result.is_err(),
            "rct transaction with parameters should fail to parse"
        );
    }

     #[test]
    fn test_regular_txn_optional_parameters() {
        let code = r#"
            let count: Int = 0;

            txn with_param(x: Int) [x > 0][count == @count + x] {
                &count = count + x;
                term;
            };

            txn without_param [count >= 0][count == @count + 1] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        let has_error = errors.iter().any(|e| !e.is_warning);
        assert!(
            !has_error,
            "Regular transactions with/without parameters should work, got errors: {:?}",
            errors
        );
    }

    #[test]
    fn test_volatile_trigger_variables() {
        // Test that trigger variables are marked as volatile
        let mut state = SymbolicState::new();
        state.mark_volatile("sensor");
        state.vars.insert("sensor".to_string(), SymbolicValue::Concrete(42));
        state.vars.insert("stable".to_string(), SymbolicValue::Concrete(42));

        // Volatile variable should be recognized as volatile
        assert!(state.is_volatile("sensor"));
        assert!(!state.is_volatile("stable"));

        // eval_numeric should return None for volatile variables
        let exec = SymbolicExecutor::new();
        let sensor_expr = Expr::Identifier("sensor".to_string());
        let stable_expr = Expr::Identifier("stable".to_string());

        assert!(
            exec.eval_numeric(&sensor_expr, &state).is_none(),
            "Volatile variable should not be concretely evaluable"
        );
        assert!(
            exec.eval_numeric(&stable_expr, &state).is_some(),
            "Stable variable should be concretely evaluable"
        );
    }

    #[test]
    fn test_volatile_eq_not_assumed() {
        // Test that x == x is NOT assumed true for volatile variables
        let mut state = SymbolicState::new();
        state.mark_volatile("trigger");

        let exec = SymbolicExecutor::new();
        let x = Expr::Identifier("trigger".to_string());
        let y = Expr::Identifier("trigger".to_string());

        // For volatile variables, x == x should NOT be provable
        assert!(
            !exec.eval_eq(&x, &y, &state),
            "Two reads of volatile variable should not be assumed equal"
        );

        // For non-volatile variables, x == x should be true
        assert!(
            exec.eval_eq(&x, &y, &SymbolicState::new()),
            "Two reads of stable variable should be equal"
        );
    }
}
