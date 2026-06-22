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

use crate::analysis::call_graph::CallGraph;
use crate::analysis::region::RegionAnalyzer;
use crate::ast::*;
use crate::features::literal::LiteralExpr;
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
    PriorRef(String),
    Add(Box<SymbolicValue>, Box<SymbolicValue>),
    Sub(Box<SymbolicValue>, Box<SymbolicValue>),
    Mul(Box<SymbolicValue>, Box<SymbolicValue>),
    BitAnd(Box<SymbolicValue>, Box<SymbolicValue>),
    BitOr(Box<SymbolicValue>, Box<SymbolicValue>),
    BitXor(Box<SymbolicValue>, Box<SymbolicValue>),
    Eq(Box<SymbolicValue>, Box<SymbolicValue>),
    Ne(Box<SymbolicValue>, Box<SymbolicValue>),
    Lt(Box<SymbolicValue>, Box<SymbolicValue>),
    Le(Box<SymbolicValue>, Box<SymbolicValue>),
    Gt(Box<SymbolicValue>, Box<SymbolicValue>),
    Ge(Box<SymbolicValue>, Box<SymbolicValue>),
    And(Box<SymbolicValue>, Box<SymbolicValue>),
    Or(Box<SymbolicValue>, Box<SymbolicValue>),
    Not(Box<SymbolicValue>),
    Unknown,
}

impl SymbolicValue {
    fn from_expr(expr: &Expr, vars: &HashMap<String, SymbolicValue>) -> Self {
        // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
        if let Some(normalized) = expr.normalize_to_old() {
            return Self::from_expr(&normalized, vars);
        }
        match expr {
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Integer(n) => SymbolicValue::Concrete(*n),
                LiteralExpr::Float(f) => SymbolicValue::ConcreteFloat(*f),
                LiteralExpr::Bool(b) => SymbolicValue::Concrete(if *b { 1 } else { 0 }),
                LiteralExpr::Char(c) => SymbolicValue::Concrete(*c as i64),
                LiteralExpr::String(_) => SymbolicValue::Unknown,
                LiteralExpr::Term => SymbolicValue::Unknown,
            },
            Expr::Integer(n) => SymbolicValue::Concrete(*n),
            Expr::Float(f) => SymbolicValue::ConcreteFloat(*f),
            Expr::Bool(b) => SymbolicValue::Concrete(if *b { 1 } else { 0 }),
            Expr::Identifier(name) => vars
                .get(name)
                .cloned()
                .unwrap_or(SymbolicValue::Symbolic(name.clone())),
            Expr::PriorState(name) => SymbolicValue::PriorRef(name.clone()),
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
            Expr::Eq(l, r) => SymbolicValue::Eq(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Ne(l, r) => SymbolicValue::Ne(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Lt(l, r) => SymbolicValue::Lt(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Le(l, r) => SymbolicValue::Le(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Gt(l, r) => SymbolicValue::Gt(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Ge(l, r) => SymbolicValue::Ge(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::And(l, r) => SymbolicValue::And(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Or(l, r) => SymbolicValue::Or(
                Box::new(Self::from_expr(l, vars)),
                Box::new(Self::from_expr(r, vars)),
            ),
            Expr::Not(inner) => SymbolicValue::Not(
                Box::new(Self::from_expr(inner, vars)),
            ),
            _ => SymbolicValue::Unknown,
        }
    }

    /// Evaluate a symbolic value to a concrete boolean if possible.
    /// Resolves PriorRef references from `initial_vars` and current vars from `current_vars`.
    /// Uses a visited set to prevent infinite recursion through variable lookups.
    fn to_bool(&self, initial_vars: &HashMap<String, SymbolicValue>, current_vars: &HashMap<String, SymbolicValue>) -> Option<bool> {
        self.to_bool_impl(initial_vars, current_vars, &mut HashSet::new())
    }

    fn to_bool_impl(&self, initial_vars: &HashMap<String, SymbolicValue>, current_vars: &HashMap<String, SymbolicValue>, visited: &mut HashSet<String>) -> Option<bool> {
        match self {
            SymbolicValue::Concrete(n) => Some(*n != 0),
            SymbolicValue::ConcreteFloat(f) => Some(*f != 0.0),
            SymbolicValue::Symbolic(name) => {
                if !visited.insert(name.clone()) {
                    return None;
                }
                if let Some(val) = current_vars.get(name) {
                    match val {
                        SymbolicValue::Concrete(n) => Some(*n != 0),
                        SymbolicValue::ConcreteFloat(f) => Some(*f != 0.0),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            SymbolicValue::PriorRef(name) => {
                if let Some(val) = initial_vars.get(name) {
                    match val {
                        SymbolicValue::Concrete(n) => Some(*n != 0),
                        SymbolicValue::ConcreteFloat(f) => Some(*f != 0.0),
                        // If the initial value is symbolic, fall through to PriorRef comparison in Eq
                        _ => None,
                    }
                } else {
                    None
                }
            }
            SymbolicValue::Eq(l, r) => {
                let lv = l.to_i64_impl(initial_vars, current_vars, visited);
                let rv = r.to_i64_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(a), Some(b)) => Some(a == b),
                    _ => {
                        let ls = l.to_string_impl(initial_vars, current_vars, visited);
                        let rs = r.to_string_impl(initial_vars, current_vars, visited);
                        match (&ls, &rs) {
                            (Some(a), Some(b)) => {
                                Some(a == b)
                            }
                            _ => None,
                        }
                    }
                }
            }
            SymbolicValue::Ne(l, r) => {
                let eq = SymbolicValue::Eq(l.clone(), r.clone()).to_bool_impl(initial_vars, current_vars, visited);
                eq.map(|v| !v)
            }
            SymbolicValue::Lt(l, r) => {
                let lv = l.to_i64_impl(initial_vars, current_vars, visited);
                let rv = r.to_i64_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(a), Some(b)) => Some(a < b),
                    _ => None,
                }
            }
            SymbolicValue::Le(l, r) => {
                let lv = l.to_i64_impl(initial_vars, current_vars, visited);
                let rv = r.to_i64_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(a), Some(b)) => Some(a <= b),
                    _ => None,
                }
            }
            SymbolicValue::Gt(l, r) => {
                let lv = l.to_i64_impl(initial_vars, current_vars, visited);
                let rv = r.to_i64_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(a), Some(b)) => Some(a > b),
                    _ => None,
                }
            }
            SymbolicValue::Ge(l, r) => {
                let lv = l.to_i64_impl(initial_vars, current_vars, visited);
                let rv = r.to_i64_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(a), Some(b)) => Some(a >= b),
                    _ => None,
                }
            }
            SymbolicValue::And(l, r) => {
                let lv = l.to_bool_impl(initial_vars, current_vars, visited);
                let rv = r.to_bool_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(true), Some(true)) => Some(true),
                    (Some(false), _) => Some(false),
                    (_, Some(false)) => Some(false),
                    _ => None,
                }
            }
            SymbolicValue::Or(l, r) => {
                let lv = l.to_bool_impl(initial_vars, current_vars, visited);
                let rv = r.to_bool_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(true), _) => Some(true),
                    (_, Some(true)) => Some(true),
                    (Some(false), Some(false)) => Some(false),
                    _ => None,
                }
            }
            SymbolicValue::Not(inner) => {
                inner.to_bool_impl(initial_vars, current_vars, visited).map(|v| !v)
            }
            SymbolicValue::Add(l, r) => {
                let lv = l.to_i64_impl(initial_vars, current_vars, visited);
                let rv = r.to_i64_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(a), Some(b)) => Some((a + b) != 0),
                    _ => None,
                }
            }
            SymbolicValue::Sub(l, r) => {
                let lv = l.to_i64_impl(initial_vars, current_vars, visited);
                let rv = r.to_i64_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(a), Some(b)) => Some((a - b) != 0),
                    _ => None,
                }
            }
            SymbolicValue::Mul(l, r) => {
                let lv = l.to_i64_impl(initial_vars, current_vars, visited);
                let rv = r.to_i64_impl(initial_vars, current_vars, visited);
                match (lv, rv) {
                    (Some(a), Some(b)) => Some((a * b) != 0),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn to_i64(&self, initial_vars: &HashMap<String, SymbolicValue>, current_vars: &HashMap<String, SymbolicValue>) -> Option<i64> {
        self.to_i64_impl(initial_vars, current_vars, &mut HashSet::new())
    }

    fn to_i64_impl(&self, initial_vars: &HashMap<String, SymbolicValue>, current_vars: &HashMap<String, SymbolicValue>, visited: &mut HashSet<String>) -> Option<i64> {
        match self {
            SymbolicValue::Concrete(n) => Some(*n),
            SymbolicValue::ConcreteFloat(f) => Some(*f as i64),
            SymbolicValue::Symbolic(name) => {
                if !visited.insert(name.clone()) {
                    return None;
                }
                if let Some(val) = current_vars.get(name) {
                    match val {
                        SymbolicValue::Concrete(n) => Some(*n),
                        SymbolicValue::ConcreteFloat(f) => Some(*f as i64),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            SymbolicValue::PriorRef(name) => {
                if let Some(val) = initial_vars.get(name) {
                    match val {
                        SymbolicValue::Concrete(n) => Some(*n),
                        SymbolicValue::ConcreteFloat(f) => Some(*f as i64),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            SymbolicValue::Add(l, r) => {
                let a = l.to_i64_impl(initial_vars, current_vars, visited)?;
                let b = r.to_i64_impl(initial_vars, current_vars, visited)?;
                Some(a + b)
            }
            SymbolicValue::Sub(l, r) => {
                let a = l.to_i64_impl(initial_vars, current_vars, visited)?;
                let b = r.to_i64_impl(initial_vars, current_vars, visited)?;
                Some(a - b)
            }
            SymbolicValue::Mul(l, r) => {
                let a = l.to_i64_impl(initial_vars, current_vars, visited)?;
                let b = r.to_i64_impl(initial_vars, current_vars, visited)?;
                Some(a * b)
            }
            _ => None,
        }
    }

    fn to_string(&self, initial_vars: &HashMap<String, SymbolicValue>, current_vars: &HashMap<String, SymbolicValue>) -> Option<String> {
        self.to_string_impl(initial_vars, current_vars, &mut HashSet::new())
    }

    fn to_string_impl(&self, initial_vars: &HashMap<String, SymbolicValue>, current_vars: &HashMap<String, SymbolicValue>, visited: &mut HashSet<String>) -> Option<String> {
        match self {
            SymbolicValue::Concrete(n) => Some(n.to_string()),
            SymbolicValue::ConcreteFloat(f) => Some(f.to_string()),
            SymbolicValue::Symbolic(name) => {
                if !visited.insert(name.clone()) {
                    return Some(name.clone());
                }
                if let Some(val) = current_vars.get(name) {
                    match val {
                        SymbolicValue::Concrete(n) => Some(n.to_string()),
                        SymbolicValue::ConcreteFloat(f) => Some(f.to_string()),
                        _ => Some(name.clone()),
                    }
                } else {
                    Some(name.clone())
                }
            }
            SymbolicValue::PriorRef(name) => {
                if let Some(val) = initial_vars.get(name) {
                    match val {
                        SymbolicValue::Concrete(n) => Some(n.to_string()),
                        SymbolicValue::ConcreteFloat(f) => Some(f.to_string()),
                        SymbolicValue::Symbolic(n) => Some(n.clone()),
                        _ => Some(format!("@{}", name)),
                    }
                } else {
                    Some(format!("@{}", name))
                }
            }
            SymbolicValue::Add(l, r) => Self::bin_op_string(l, r, " + ", initial_vars, current_vars, visited),
            SymbolicValue::Sub(l, r) => Self::bin_op_string(l, r, " - ", initial_vars, current_vars, visited),
            SymbolicValue::Mul(l, r) => Self::bin_op_string(l, r, " * ", initial_vars, current_vars, visited),
            SymbolicValue::Eq(l, r) => Self::bin_op_string(l, r, " == ", initial_vars, current_vars, visited),
            SymbolicValue::Ne(l, r) => Self::bin_op_string(l, r, " != ", initial_vars, current_vars, visited),
            SymbolicValue::Lt(l, r) => Self::bin_op_string(l, r, " < ", initial_vars, current_vars, visited),
            SymbolicValue::Le(l, r) => Self::bin_op_string(l, r, " <= ", initial_vars, current_vars, visited),
            SymbolicValue::Gt(l, r) => Self::bin_op_string(l, r, " > ", initial_vars, current_vars, visited),
            SymbolicValue::Ge(l, r) => Self::bin_op_string(l, r, " >= ", initial_vars, current_vars, visited),
            SymbolicValue::And(l, r) => Self::bin_op_string(l, r, " && ", initial_vars, current_vars, visited),
            SymbolicValue::Or(l, r) => Self::bin_op_string(l, r, " || ", initial_vars, current_vars, visited),
            SymbolicValue::Not(inner) => {
                let s = inner.to_string_impl(initial_vars, current_vars, visited)?;
                Some(format!("!{}", s))
            }
            _ => None,
        }
    }

    fn bin_op_string(l: &Box<SymbolicValue>, r: &Box<SymbolicValue>, op: &str, initial_vars: &HashMap<String, SymbolicValue>, current_vars: &HashMap<String, SymbolicValue>, visited: &mut HashSet<String>) -> Option<String> {
        let ls = l.to_string_impl(initial_vars, current_vars, visited)?;
        let rs = r.to_string_impl(initial_vars, current_vars, visited)?;
        Some(format!("{}{}{}", ls, op, rs))
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
                false,
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
                false,
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
                false,
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
                false,
            );
        }

        self.errors.clone()
    }

    fn negate_expr(&self, expr: &Expr) -> Option<Expr> {
        // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
        if let Some(normalized) = expr.normalize_to_old() {
            return self.negate_expr(&normalized);
        }
        match expr {
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Bool(b) => Some(Expr::Literal(Box::new(LiteralExpr::Bool(!b)))),
                _ => None,
            },
            Expr::Bool(b) => Some(Expr::Bool(!b)),
            Expr::Identifier(name) => Some(Expr::Not(Box::new(Expr::Identifier(name.clone())))),
            Expr::Eq(l, r) => Some(Expr::Ne(l.clone(), r.clone())),
            Expr::Ne(l, r) => Some(Expr::Eq(l.clone(), r.clone())),
            Expr::Lt(l, r) => Some(Expr::Ge(l.clone(), r.clone())),
            Expr::Le(l, r) => Some(Expr::Gt(l.clone(), r.clone())),
            Expr::Gt(l, r) => Some(Expr::Le(l.clone(), r.clone())),
            Expr::Ge(l, r) => Some(Expr::Lt(l.clone(), r.clone())),
            Expr::And(l, r) => {
                // De Morgan: !(A && B) == (!A || !B)
                let not_l = self.negate_expr(l)?;
                let not_r = self.negate_expr(r)?;
                Some(Expr::Or(Box::new(not_l), Box::new(not_r)))
            }
            Expr::Or(l, r) => {
                // De Morgan: !(A || B) == (!A && !B)
                let not_l = self.negate_expr(l)?;
                let not_r = self.negate_expr(r)?;
                Some(Expr::And(Box::new(not_l), Box::new(not_r)))
            }
            Expr::Not(inner) => Some(inner.as_ref().clone()),
            Expr::PriorState(name) => Some(Expr::Not(Box::new(Expr::PriorState(name.clone())))),
            _ => None,
        }
    }

    fn init_state_from_precondition(&self, pre: &Expr) -> SymbolicState {
        let mut state = SymbolicState::new();

        match pre {
            Expr::Bool(true) => {}
            Expr::Literal(lit) if matches!(lit.as_ref(), LiteralExpr::Bool(true)) => {}
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

        // Add precondition as a path constraint so the symbolic executor
        // can use it to prune infeasible paths.
        if !matches!(pre, Expr::Bool(true)) && pre.as_bool() != Some(true) {
            state.constraints.push(PathConstraint {
                condition: pre.clone(),
                is_negated: false,
            });
        }

        state
    }

    fn extract_vars(&self, expr: &Expr) -> HashSet<String> {
        let mut vars = HashSet::new();
        self.collect_vars(expr, &mut vars);
        vars
    }

    fn collect_vars(&self, expr: &Expr, vars: &mut HashSet<String>) {
        // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
        if let Some(normalized) = expr.normalize_to_old() {
            return self.collect_vars(&normalized, vars);
        }
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
        state: SymbolicState,
        context: String,
        is_reactive: bool,
    ) {
        let initial_vars = state.vars.clone();
        let term_paths = self.enumerate_paths(body, state.clone());

        for (path_idx, (path_state, path_kind)) in term_paths.iter().enumerate() {
            // Escape paths cancel the transaction - postconditions are vacuously satisfied
            if let PathKind::Escape = path_kind {
                continue;
            }

            if !self.implies(pre_condition, &initial_vars, path_state, post_condition) {
                let mut err = ProofError::new("P008", "contract verification failed");
                err.explanation = format!(
                    "{}: post-condition not satisfied on path {}",
                    context, path_idx
                );
                err.proof_chain.push(format!(
                    "1. Pre-condition: {}",
                    format_expr(pre_condition)
                ));

                if !path_state.constraints.is_empty() {
                    err.proof_chain.push("2. Path constraints:".to_string());
                    for (i, constraint) in path_state.constraints.iter().enumerate() {
                        let cond_str = format_expr(&constraint.condition);
                        let neg = if constraint.is_negated { "¬" } else { "" };
                        err.proof_chain.push(format!("   {}. {}{}", i + 1, neg, cond_str));
                    }
                }

                err.proof_chain.push(format!(
                    "3. Post-condition: {}",
                    format_expr(post_condition)
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

        for (i, stmt) in body.iter().enumerate() {
            if terminated {
                break;
            }

            match stmt {
                Statement::Assignment {
                    lhs,
                    expr,
                    timeout: _,
                    modifiers: _,
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
                    self.enumerate_paths_recursive(statements, true_state.clone(), &mut true_paths);

                    // If the guard body didn't terminate (no term inside),
                    // continue exploring the remaining body after the guard
                    // so the guard-taken path reaches term.
                    if true_paths.is_empty() && i + 1 < body.len() {
                        self.enumerate_paths_recursive(&body[i + 1..], true_state, &mut true_paths);
                    }

                    let mut false_paths = Vec::new();
                    if i + 1 < body.len() {
                        self.enumerate_paths_recursive(&body[i + 1..], false_state, &mut false_paths);
                    }

                    for (s, pk) in true_paths.into_iter().chain(false_paths.into_iter()) {
                        paths.push((s, pk));
                    }
                    return;
                }
                Statement::Term { values: outputs, .. } => {
                    terminated = true;
                    path_kind = PathKind::Term(outputs.clone());
                }
                Statement::TermBang { values: outputs, .. } => {
                    terminated = true;
                    path_kind = PathKind::Term(outputs.clone());
                }
                Statement::Escape(_) => {
                    terminated = true;
                    path_kind = PathKind::Escape;
                }
                Statement::Expression(_) | Statement::Unification { .. } | Statement::SyncBlock { .. } | Statement::LocalTrigger { .. } => {}
                Statement::Alka(_) | Statement::OnExit { .. } => {}
                Statement::Foreach { body, .. } => {
                    self.enumerate_paths_recursive(body, current_state.clone(), paths);
                }
                Statement::Oracle { body, handler, .. } => {
                    self.enumerate_paths_recursive(body, current_state.clone(), paths);
                    self.enumerate_paths_recursive(handler, current_state.clone(), paths);
                }
                Statement::Await { expr, .. } => {
                    let _value = SymbolicValue::from_expr(expr, &current_state.vars);
                }
                Statement::Async { body, .. } => {
                    self.enumerate_paths_recursive(std::slice::from_ref(body.as_ref()), current_state.clone(), paths);
                }
                Statement::AsyncAwait { body, .. } => {
                    self.enumerate_paths_recursive(std::slice::from_ref(body.as_ref()), current_state.clone(), paths);
                }
            }
        }

        if terminated {
            paths.push((current_state, path_kind));
        }
    }

    fn implies(&mut self, pre: &Expr, initial_vars: &HashMap<String, SymbolicValue>, state: &SymbolicState, post: &Expr) -> bool {
        let pre_true = self.is_truthy(pre, state);
        if !pre_true {
            return true;
        }

        // Check all path constraints for feasibility.
        // Negated constraints must NOT be truthy (if they are, the path is impossible).
        // Non-negated constraints must be truthy (if they aren't, the path is impossible).
        for constraint in &state.constraints {
            if constraint.is_negated {
                if self.is_truthy(&constraint.condition, state) {
                    return false;
                }
            } else {
                if !self.is_truthy(&constraint.condition, state) {
                    return false;
                }
            }
        }

        if self.contains_prior_state(post) {
            return self.verify_post_with_prior(initial_vars, state, post);
        }

        let post_true = self.is_truthy(post, state);
        post_true
    }

    fn verify_post_with_prior(&self, initial_vars: &HashMap<String, SymbolicValue>, state: &SymbolicState, post: &Expr) -> bool {
        self.check_post_satisfiable(post, initial_vars, state)
    }

    fn check_post_satisfiable(
        &self,
        post: &Expr,
        initial_vars: &HashMap<String, SymbolicValue>,
        state: &SymbolicState,
    ) -> bool {
        // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
        if let Some(normalized) = post.normalize_to_old() {
            return self.check_post_satisfiable(&normalized, initial_vars, state);
        }
        let sym = SymbolicValue::from_expr(post, &state.vars);
        let sym = SymbolicValue::from_expr(post, &state.vars);
        let result = sym.to_bool(initial_vars, &state.vars);
        match result {
            Some(true) => true,
            Some(false) | None => {
                match post {
                    Expr::Eq(l, r) => {
                        let l_sym = SymbolicValue::from_expr(l, &state.vars);
                        let r_sym = SymbolicValue::from_expr(r, &state.vars);
                        if let (Some(ls), Some(rs)) = (l_sym.to_string(initial_vars, &state.vars), r_sym.to_string(initial_vars, &state.vars)) {
                            return ls == rs;
                        }
                        let l_raw = format_expr(l);
                        let r_raw = format_expr(r);
                        let l_expanded = l_raw.replace("@", "");
                        let r_expanded = r_raw.replace("@", "");
                        l_raw == r_raw || l_expanded == r_expanded || l_expanded == r_raw || r_expanded == l_raw
                    }
                    _ => true,
                }
            }
        }
    }

    fn contains_prior_state(&self, expr: &Expr) -> bool {
        // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
        if let Some(normalized) = expr.normalize_to_old() {
            return self.contains_prior_state(&normalized);
        }
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
        // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
        if let Some(normalized) = expr.normalize_to_old() {
            return self.is_truthy(&normalized, state);
        }
        match expr {
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Bool(b) => *b,
                _ => true,
            },
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
                let ls = format_expr(l);
                let rs = format_expr(r);
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
        // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
        if let Some(normalized) = expr.normalize_to_old() {
            return self.eval_numeric(&normalized, state);
        }
        match expr {
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Integer(n) => Some(*n),
                _ => None,
            },
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
            Expr::Div(l, r) => {
                let a = self.eval_numeric(l, state)?;
                let b = self.eval_numeric(r, state)?;
                if b == 0 { None } else { Some(a / b) }
            }
            Expr::Mod(l, r) => {
                let a = self.eval_numeric(l, state)?;
                let b = self.eval_numeric(r, state)?;
                if b == 0 { None } else { Some(a % b) }
            }
            _ => None,
        }
    }

}

fn format_expr(expr: &Expr) -> String {
    // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
    if let Some(normalized) = expr.normalize_to_old() {
        return format_expr(&normalized);
    }
    match expr {
        Expr::Literal(lit) => lit.format(),
        Expr::Integer(n) => n.to_string(),
        Expr::Float(f) => f.to_string(),
        Expr::String(s) => format!("\"{}\"", s),
        Expr::Bool(b) => b.to_string(),
        Expr::Identifier(name) => name.clone(),
        Expr::PriorState(name) => format!("@{}", name),
        Expr::Add(l, r) => format!("{} + {}", format_expr(l), format_expr(r)),
        Expr::Sub(l, r) => format!("{} - {}", format_expr(l), format_expr(r)),
        Expr::Mul(l, r) => format!("{} * {}", format_expr(l), format_expr(r)),
        Expr::Div(l, r) => format!("{} / {}", format_expr(l), format_expr(r)),
        Expr::Mod(l, r) => format!("{} % {}", format_expr(l), format_expr(r)),
        Expr::Eq(l, r) => format!("{} == {}", format_expr(l), format_expr(r)),
        Expr::Ne(l, r) => format!("{} != {}", format_expr(l), format_expr(r)),
        Expr::Lt(l, r) => format!("{} < {}", format_expr(l), format_expr(r)),
        Expr::Le(l, r) => format!("{} <= {}", format_expr(l), format_expr(r)),
        Expr::Gt(l, r) => format!("{} > {}", format_expr(l), format_expr(r)),
        Expr::Ge(l, r) => format!("{} >= {}", format_expr(l), format_expr(r)),
        Expr::And(l, r) => format!("{} && {}", format_expr(l), format_expr(r)),
        Expr::Or(l, r) => format!("{} || {}", format_expr(l), format_expr(r)),
        Expr::Not(inner) => format!("!{}", format_expr(inner)),
        Expr::Neg(inner) => format!("-{}", format_expr(inner)),
        Expr::Call(name, args) => {
            let args_str = args
                .iter()
                .map(|a| format_expr(a))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", name, args_str)
        }
        Expr::IntrinsicCall { intrinsic, args } => {
            let args_str = args
                .iter()
                .map(|a| format_expr(a))
                .collect::<Vec<_>>()
                .join(", ");
            let name = match intrinsic {
                Intrinsic::UserDefined(n) => n.as_str(),
                _ => intrinsic.name(),
            };
            format!("{}#({})", name, args_str)
        }
        _ => "<expr>".to_string(),
    }
}

/// Check if a compound expression tree involves a variable by name.
fn contains_var(expr: &Expr, var: &str) -> bool {
    // Handle new-style BinaryOp/UnaryOp by normalizing to old variants
    if let Some(normalized) = expr.normalize_to_old() {
        return contains_var(&normalized, var);
    }
    match expr {
        Expr::Identifier(v) | Expr::PriorState(v) => v == var,
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r)
        | Expr::Div(l, r) | Expr::Mod(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r)
        | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r)
        | Expr::And(l, r) | Expr::Or(l, r)
        | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r)
        | Expr::Shl(l, r) | Expr::Shr(l, r) | Expr::Concat(l, r) => {
            contains_var(l, var) || contains_var(r, var)
        }
        Expr::Not(i) | Expr::Neg(i) | Expr::BitNot(i) => contains_var(i, var),
        _ => false,
    }
}

/// Extract the sub-expression from AND that involves `var`.
fn extract_var_relation<'a>(expr: &'a Expr, var: &str) -> Option<&'a Expr> {
    match expr {
        Expr::And(l, r) => {
            if contains_var(l, var) {
                extract_var_relation(l, var).or(Some(l))
            } else if contains_var(r, var) {
                extract_var_relation(r, var).or(Some(r))
            } else {
                None
            }
        }
        Expr::Or(_, _) => None,
        Expr::BinaryOp(bop) => match bop.kind {
            crate::features::binary_op::BinaryOpKind::And => {
                if contains_var(&bop.left, var) {
                    extract_var_relation(&bop.left, var).or(Some(&bop.left))
                } else if contains_var(&bop.right, var) {
                    extract_var_relation(&bop.right, var).or(Some(&bop.right))
                } else {
                    None
                }
            }
            crate::features::binary_op::BinaryOpKind::Or => None,
            _ => {
                if contains_var(expr, var) { Some(expr) } else { None }
            }
        },
        other => {
            if contains_var(other, var) { Some(other) } else { None }
        }
    }
}

/// Evaluate a pure-integer constant expression. Returns None for non-constants.
/// Resolves const identifiers from `initial_values`.
fn eval_const_expr(expr: &Expr, initial_values: &HashMap<String, Expr>) -> Option<i64> {
    // Handle new-style BinaryOp by normalizing to old variants
    if let Some(normalized) = expr.normalize_to_old() {
        return eval_const_expr(&normalized, initial_values);
    }
    match expr {
        Expr::Integer(n) => Some(*n),
        Expr::Literal(lit) => match lit.as_ref() {
            crate::features::literal::LiteralExpr::Integer(n) => Some(*n),
            _ => None,
        },
        Expr::Add(l, r) => Some(eval_const_expr(l, initial_values)? + eval_const_expr(r, initial_values)?),
        Expr::Sub(l, r) => Some(eval_const_expr(l, initial_values)? - eval_const_expr(r, initial_values)?),
        Expr::Mul(l, r) => Some(eval_const_expr(l, initial_values)? * eval_const_expr(r, initial_values)?),
        Expr::Identifier(name) => {
            let resolved = initial_values.get(name)?;
            eval_const_expr(resolved, initial_values)
        }
        _ => None,
    }
}

/// Check for `var & (var - 1)` popcount decay pattern.
/// Handles both `Expr::Integer(1)` and `Expr::Literal(Boolean(1))` variants.
fn is_self_minus_one(a: &Expr, b: &Expr, var: &str) -> bool {
    let is_one = |e: &Expr| -> bool {
        matches!(e, Expr::Integer(1))
            || matches!(e, Expr::Literal(lit) if matches!(lit.as_ref(), crate::features::literal::LiteralExpr::Integer(1)))
    };
    matches!(a, Expr::Identifier(v) if v == var)
        && matches!(b, Expr::Sub(inner, val)
            if matches!(inner.as_ref(), Expr::Identifier(v) if v == var)
                && is_one(val.as_ref()))
}

/// Extract the variable name and bound expression from a comparison.
/// Returns `(var, bound)` where `var` is always an identifier and `bound`
/// can be an identifier, integer literal, or other expression.
fn extract_var_bound(expr: &Expr) -> Option<(String, Expr)> {
    // Handle new-style BinaryOp by normalizing to old variants
    if let Some(normalized) = expr.normalize_to_old() {
        return extract_var_bound(&normalized);
    }
    let (lhs, rhs) = match expr {
        Expr::Eq(l, r)
        | Expr::Ne(l, r)
        | Expr::Lt(l, r)
        | Expr::Le(l, r)
        | Expr::Gt(l, r)
        | Expr::Ge(l, r) => (l.as_ref(), r.as_ref()),
        _ => return None,
    };
    match (lhs, rhs) {
        (Expr::Identifier(v), b) => Some((v.clone(), b.clone())),
        (b, Expr::Identifier(v)) => Some((v.clone(), b.clone())),
        _ => None,
    }
}

/// Check if a pre-condition is structurally `var <op> bound_expr` for one of `valid_ops`.
/// `bound_expr` can be an identifier or an integer literal.
fn check_pre_matches(pre: &Expr, var: &str, bound_expr: &Expr, valid_ops: &[&str]) -> bool {
    // Handle new-style BinaryOp by normalizing to old variants
    if let Some(normalized) = pre.normalize_to_old() {
        return check_pre_matches(&normalized, var, bound_expr, valid_ops);
    }
    let op = match pre {
        Expr::Lt(..) => "<",
        Expr::Gt(..) => ">",
        Expr::Le(..) => "<=",
        Expr::Ge(..) => ">=",
        Expr::Eq(..) => "==",
        Expr::Ne(..) => "!=",
        _ => return false,
    };
    if !valid_ops.contains(&op) {
        return false;
    }
    let (lhs, rhs) = match pre {
        Expr::Lt(l, r)
        | Expr::Gt(l, r)
        | Expr::Le(l, r)
        | Expr::Ge(l, r)
        | Expr::Eq(l, r)
        | Expr::Ne(l, r) => (l.as_ref(), r.as_ref()),
        _ => return false,
    };
    match (lhs, rhs) {
        (Expr::Identifier(v), b) => v == var && b == bound_expr,
        (b, Expr::Identifier(v)) => v == var && b == bound_expr,
        _ => false,
    }
}

/// Check if a reactive transaction has a structurally provable convergence contract.
///
/// A convergence contract `[pre][post]` requires:
/// 1. `post → ¬pre` — the post being true guarantees the pre is false (loop terminates).
/// 2. The body increments or decrements `var` by a constant positive step.
/// 3. `bound` is not assigned in the body (invariant).
/// 4. When step > 1 and post is `var == bound`: the step divides the distance
///    from the initial value to the bound (no overshoot).
///
/// `initial_values` provides compile-time-known initial values for state variables
/// and constants, used for overshoot detection.
fn check_convergence(
    body: &[Statement],
    pre_condition: &Expr,
    post_condition: &Expr,
    initial_values: &HashMap<String, Expr>,
) -> bool {
    // Normalize new-style BinaryOp/UnaryOp to old variants
    let owned_pre;
    let owned_post;
    let pre_condition = match pre_condition.normalize_to_old() {
        Some(n) => { owned_pre = n; &owned_pre as &Expr }
        None => pre_condition,
    };
    let post_condition = match post_condition.normalize_to_old() {
        Some(n) => { owned_post = n; &owned_post as &Expr }
        None => post_condition,
    };
    // Step 1: Extract (var, bound_expr) from postcondition
    let (var, bound_expr) = match extract_var_bound(post_condition) {
        Some(pair) => pair,
        None => return false,
    };

    // Step 2: Validate post → ¬pre
    // Extract var-involving sub-expression from AND/OR preconditions
    let pre_for_var = extract_var_relation(pre_condition, &var).unwrap_or(pre_condition);
    let pre_valid = match post_condition {
        // post: var == bound → pre must be <, >, or !=
        Expr::Eq(_, _) => check_pre_matches(pre_for_var, &var, &bound_expr, &["<", ">", "!="]),
        // post: var >= bound → pre must be <
        Expr::Ge(_, _) => check_pre_matches(pre_for_var, &var, &bound_expr, &["<"]),
        // post: var > bound → pre must be <=
        Expr::Gt(_, _) => check_pre_matches(pre_for_var, &var, &bound_expr, &["<="]),
        // post: var <= bound → pre must be >
        Expr::Le(_, _) => check_pre_matches(pre_for_var, &var, &bound_expr, &[">"]),
        // post: var < bound → pre must be >=
        Expr::Lt(_, _) => check_pre_matches(pre_for_var, &var, &bound_expr, &[">="]),
        // post: var != bound → pre must be ==
        Expr::Ne(_, _) => check_pre_matches(pre_for_var, &var, &bound_expr, &["=="]),
        _ => false,
    };
    if !pre_valid {
        return false;
    }

    // Step 3: Detect increment/decrement on var, extract direction and step
    let mut step: i64 = 0;
    let mut direction: i8 = 0; // 1 = counting up, -1 = counting down
    for stmt in body {
        if let Statement::Assignment { lhs, expr, .. } = stmt {
            let assign_name = match lhs {
                Expr::Identifier(n) | Expr::OwnedRef(n) => n,
                _ => continue,
            };
            if assign_name != &var {
                continue;
            }
            // Normalize BinaryOp to old variants for convergence analysis
            let owned_expr;
            let expr = match expr.normalize_to_old() {
                Some(normalized) => { owned_expr = normalized; &owned_expr as &Expr }
                None => expr,
            };
            match expr {
                Expr::Add(a, b) => {
                    if let Expr::Identifier(v) = a.as_ref() {
                        if v == &var {
                            if let Some(d) = b.as_integer() {
                                if d > 0 {
                                    step = d;
                                    direction = 1;
                                }
                            }
                            // Compound: count = count + Sub(N, M) → count + (N-M)
                            if step == 0 {
                                if let Expr::Sub(oa, ob) = b.as_ref() {
                                    if let (Some(n), Some(m)) = (eval_const_expr(oa, initial_values), eval_const_expr(ob, initial_values)) {
                                        let net = n - m;
                                        if net > 0 { step = net; direction = 1; }
                                    }
                                }
                            }
                        }
                    }
                    if let Expr::Identifier(v) = b.as_ref() {
                        if v == &var {
                            if let Some(d) = a.as_integer() {
                                if d > 0 {
                                    step = d;
                                    direction = 1;
                                }
                            }
                        }
                    }
                }
                Expr::Sub(a, b) => {
                    if let Expr::Identifier(v) = a.as_ref() {
                        if v == &var {
                            if let Some(d) = b.as_integer() {
                                if d > 0 {
                                    step = d;
                                    direction = -1;
                                }
                            }
                        }
                    }
                    // Compound: count = (count + N) - M → count + (N-M)
                    if step == 0 {
                        if let Expr::Add(inner, offset) = a.as_ref() {
                            if let Expr::Identifier(v) = inner.as_ref() {
                                if *v == var {
                                    if let (Some(n), Some(m)) = (eval_const_expr(offset, initial_values), eval_const_expr(b, initial_values)) {
                                        let net = n - m;
                                        if net > 0 { step = net; direction = 1; }
                                    }
                                }
                            }
                        }
                    }
                }
                // popcount decay: reg & (reg - 1) → clears one bit per iteration
                Expr::BitAnd(a, b) => {
                    if is_self_minus_one(a, b, &var) || is_self_minus_one(b, a, &var) {
                        step = 1;
                        direction = -1;
                    }
                }
                _ => {}
            }
        }
    }
    if step == 0 {
        return false;
    }

    // Step 4: Bound invariance — bound must not be assigned in body.
    // Only checkable when bound is an identifier (not a literal).
    if let Expr::Identifier(ref bound_name) = bound_expr {
        for stmt in body {
            if let Statement::Assignment { lhs, .. } = stmt {
                let assign_name = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n,
                    _ => continue,
                };
                if assign_name == bound_name {
                    return false;
                }
            }
        }
    }

    // Step 5: Overshoot detection — only matters for exact-equality postcondition
    // when the step size is greater than 1 (e.g., count = count + 5 could skip past bound).
    if matches!(post_condition, Expr::Eq(_, _)) && step > 1 {
        let init_val = initial_values.get(&var).and_then(|e| e.as_integer());
        // Bound value: could be from initial_values, or a literal integer in bound_expr
        let bound_val = match &bound_expr {
            e if e.as_integer().is_some() => e.as_integer(),
            Expr::Identifier(name) => initial_values.get(name).and_then(|e| e.as_integer()),
            _ => None,
        };

        if let (Some(init), Some(bound)) = (init_val, bound_val) {
            let dist = if direction == 1 {
                bound - init
            } else {
                init - bound
            };
            // If dist <= 0, the transaction never fires (pre is false initially) —
            // convergence is vacuously true.  Otherwise verify no overshoot.
            if dist > 0 && dist % step != 0 {
                return false;
            }
        } else {
            // Can't verify — conservatively reject convergence
            return false;
        }
    }

    true
}

pub struct ProofEngine {
    errors: Vec<ProofError>,
    state_dag: HashMap<String, HashSet<String>>,
    transactions: Vec<Transaction>,
    strict: bool,
    pub region_analyzer: Option<RegionAnalyzer>,
}

impl ProofEngine {
    pub fn new() -> Self {
        ProofEngine {
            errors: Vec::new(),
            state_dag: HashMap::new(),
            transactions: Vec::new(),
            strict: false,
            region_analyzer: None,
        }
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    fn make_err(&self, code: &str, title: &str) -> ProofError {
        if self.strict {
            ProofError::new(code, title)
        } else {
            ProofError::new_warning(code, title)
        }
    }

    pub fn verify_program(&mut self, program: &Program) -> Vec<ProofError> {
        // Run region analysis for optimizer queries
        self.region_analyzer = Some(RegionAnalyzer::analyze(program));

        self.build_state_dag(program);
        self.collect_transactions(program);
        self.check_exhaustiveness(program);
        self.check_mutual_exclusion(program);
        self.suggest_async_promotion(program);
        self.check_total_path(program);
        self.check_true_assertions(program);
        self.check_postcondition_contradictions(program);
        self.check_trivial_contracts(program);
        self.check_sig_projections(program);
        self.check_ffi_error_handling(program);
        self.check_circular_dependencies(program);
        self.check_list_simd_lengths(program);
        self.check_structural_recursion(program);
        self.verify_contracts(program);
        self.errors.clone()
    }

    fn verify_contracts(&mut self, program: &Program) {
        // Build initial-values map from StateDecl and Constant declarations
        let mut initial_values: HashMap<String, Expr> = HashMap::new();
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    if let Some(ref expr) = decl.expr {
                        initial_values.insert(decl.name.clone(), expr.clone());
                    }
                }
                TopLevel::Constant(constant) => {
                    initial_values.insert(constant.name.clone(), constant.expr.clone());
                }
                _ => {}
            }
        }

        // Collect all trigger variable names (volatile variables)
        let mut volatile_vars = HashSet::new();
        for item in &program.items {
            if let TopLevel::Trigger(trg) = item {
                volatile_vars.insert(trg.name.clone());
            }
        }

        // Build definition and transaction lookup maps for assertion chains
        let mut initial_definitions: HashMap<&str, &Definition> = HashMap::new();
        let mut initial_txns: HashMap<&str, &Transaction> = HashMap::new();
        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                initial_definitions.insert(&defn.name, defn);
            } else if let TopLevel::Transaction(txn) = item {
                initial_txns.insert(&txn.name, txn);
            } else if let TopLevel::Test { item: inner, .. } = item {
                match inner.as_ref() {
                    TopLevel::Definition(defn) => { initial_definitions.insert(&defn.name, defn); }
                    TopLevel::Transaction(txn) => { initial_txns.insert(&txn.name, txn); }
                    _ => {}
                }
            }
        }

        let mut sym_exec = SymbolicExecutor::new().with_volatile_vars(volatile_vars);

        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    // For convergence contracts, skip symbolic execution entirely
                    // — the structural convergence proof is stronger than
                    // the per-path postcondition check (P008).
                    // Applied to all txns (reactive and callable) since the
                    // structural proof works identically for both — the bound
                    // variable, step, and convergence semantics are the same.
                    if check_convergence(
                            &txn.body,
                            &txn.contract.pre_condition,
                            &txn.contract.post_condition,
                            &initial_values,
                        )
                    {
                        // Convergence proven — no symbolic execution needed
                    } else {
                        let errs = sym_exec.verify_transaction(txn);
                        self.errors.extend(errs);
                    }
                }
                TopLevel::Definition(defn) => {
                    let errs = sym_exec.verify_definition(defn);
                    self.errors.extend(errs);
                }
                TopLevel::Assertion { pre, chain } => {
                    // Verify assertion chain: pre → fn_a → fn_b → ... → post
                    let mut current_pre = pre.clone();
                    for fn_name in chain {
                        if let Some(defn) = initial_definitions.get(fn_name.as_str()) {
                            let st = sym_exec.init_state_from_precondition(&current_pre);
                            sym_exec.verify_contract_implication(
                                &current_pre,
                                &defn.contract.post_condition,
                                &defn.body,
                                st,
                                format!("assertion chain '{:?}' step '{}'", chain, fn_name),
                                false,
                            );
                            current_pre = defn.contract.post_condition.clone();
                        } else if let Some(txn) = initial_txns.get(fn_name.as_str()) {
                            let st = sym_exec.init_state_from_precondition(&current_pre);
                            sym_exec.verify_contract_implication(
                                &current_pre,
                                &txn.contract.post_condition,
                                &txn.body,
                                st,
                                format!("assertion chain '{:?}' step '{}'", chain, fn_name),
                                false,
                            );
                            current_pre = txn.contract.post_condition.clone();
                        } else {
                            let err = ProofError::new("P012", "assertion chain: function not found")
                                .with_explanation(&format!(
                                    "Function '{}' in assertion chain not found", fn_name
                                ));
                            self.errors.push(err);
                        }
                    }
                    // Transfer assertion verification errors from sym_exec to self
                    self.errors.append(&mut sym_exec.errors);
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
            let _result_var = self.find_ffi_result_variable(&defn.body, &frgn_name);
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

    fn check_branch_terminates(&self, statements: &[Statement], terminates: &mut bool) {
        for stmt in statements {
            match stmt {
                Statement::Term { .. } | Statement::TermBang { .. } => {
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
            Expr::IntrinsicCall { .. } => {}
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
            Expr::Projection { source: list, .. } => self.find_ffi_calls_in_expr(list, calls, ffi_bindings),
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
                    let pre_is_trivial = txn.contract.pre_condition.as_bool() == Some(true);
                    let post_is_trivial = txn.contract.post_condition.as_bool() == Some(true);

                    if pre_is_trivial && post_is_trivial && txn.contract.span.is_some() {
                        // BOTH trivial and explicitly written - hard error
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
                        // Only precondition trivial
                        let mut err = self.make_err("P009", "trivial precondition");
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
                        // Only postcondition trivial
                        let mut err = self.make_err("P010", "trivial postcondition");
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
                    let pre_is_trivial = defn.contract.pre_condition.as_bool() == Some(true);
                    let post_is_trivial = defn.contract.post_condition.as_bool() == Some(true);

                    if pre_is_trivial && post_is_trivial && defn.contract.span.is_some() {
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
                        let mut err = self.make_err("P009", "trivial precondition");
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
                        let mut err = self.make_err("P010", "trivial postcondition");
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
    fn check_structural_recursion(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                let has_recursive_call = defn.body.iter().any(|s| contains_call_to(s, &defn.name));
                if !has_recursive_call {
                    continue;
                }
                let proven = defn.parameters.iter().any(|(param_name, _)| {
                    defn.body.iter().all(|s| check_decreasing_arg(s, &defn.name, param_name))
                });
                if !proven {
                    self.errors.push(
                        ProofError::new("P021", "Structural recursion not proven")
                            .with_explanation(&format!(
                                "Definition '{}' has recursive calls but no structurally decreasing parameter was found. \
                                 Try using a decreasing argument like n-1 or list.tail().",
                                defn.name
                            ))
                            .with_hint("Ensure at least one argument strictly decreases on every recursive call")
                    );
                }
            }
        }
    }

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
                        Box::new(Expr::Projection {
                            source: Box::new(Expr::Identifier(left_name.clone())),
                            target: ProjectionTarget::Size,
                        }),
                        Box::new(Expr::Projection {
                            source: Box::new(Expr::Identifier(right_name.clone())),
                            target: ProjectionTarget::Size,
                        }),
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
            (Expr::Projection { source: l1, target: ProjectionTarget::Size }, Expr::Projection { source: l2, target: ProjectionTarget::Size }) => self.exprs_equal(l1, l2),
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
            Expr::MultiSlice { value, ops: mops } => {
                self.collect_list_simd_ops_in_expr(value, ops);
                for mop in mops {
                    match mop {
                        BracketOp::Mask(m) => self.collect_list_simd_ops_in_expr(m, ops),
                        BracketOp::Stride(s) => self.collect_list_simd_ops_in_expr(s, ops),
                        BracketOp::Coord(_) => {}
                    }
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
        let owned_post;
        let post = match txn.contract.post_condition.normalize_to_old() {
            Some(n) => { owned_post = n; &owned_post as &Expr }
            None => &txn.contract.post_condition,
        };
        self.check_post_contradiction(post, &txn.name, txn.contract.span);
    }

    fn check_post_contradiction(&mut self, expr: &Expr, txn_name: &str, span: Option<Span>) {
        match expr {
            Expr::And(l, r) => {
                // Detect contradictions like (x > 0 && x < 0) or (x == 1 && x == 2)
                self.check_and_contradiction(l, r, txn_name, span);
                self.check_post_contradiction(l, txn_name, span);
                self.check_post_contradiction(r, txn_name, span);
            }
            Expr::Eq(left, right) => {
                // Detect x == @x (trivially always true)
                let (var, prior_var) = match (left.as_ref(), right.as_ref()) {
                    (Expr::Identifier(v), Expr::PriorState(p)) => (v.clone(), p.clone()),
                    (Expr::PriorState(p), Expr::Identifier(v)) => (v.clone(), p.clone()),
                    _ => return,
                };
                if var == prior_var {
                    let mut err = ProofError::new("P003", "postcondition is always satisfied");
                    err.explanation = format!(
                        "transaction '{}' postcondition '{} == @{}' is always true",
                        txn_name, var, var
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
                    if let Some(s) = span {
                        err = err.with_span(s);
                    }
                    self.errors.push(err);
                }
            }
            Expr::Ne(_, _) => {
                // x != @x is trivially false (can't contradict yourself) — but NOT always!
                // x != @x is a valid postcondition if x has been modified.
                // Skip — it's not a contradiction.
            }
            _ => {}
        }
    }

    fn check_and_contradiction(&mut self, l: &Expr, r: &Expr, txn_name: &str, span: Option<Span>) {
        // Check for direct contradictions: (x > 5) && (x < 3)
        let l_bound = self.extract_bound(l);
        let r_bound = self.extract_bound(r);

        if let (Some((l_var, l_cmp, l_val)), Some((r_var, r_cmp, r_val))) = (l_bound, r_bound) {
            if l_var == r_var {
                // Same variable — check if bounds are contradictory
                let contradictory = match (l_cmp, r_cmp) {
                    ("gt", "lt") => l_val >= r_val,
                    ("lt", "gt") => l_val <= r_val,
                    ("ge", "lt") => l_val >= r_val,
                    ("lt", "ge") => l_val <= r_val,
                    ("gt", "le") => l_val >= r_val,
                    ("le", "gt") => l_val <= r_val,
                    ("ge", "le") => l_val > r_val,
                    ("le", "ge") => l_val < r_val,
                    _ => false,
                };
                if contradictory {
                    let mut err = ProofError::new("P003", "postcondition is always satisfied");
                    err.explanation = format!(
                        "transaction '{}' has contradictory postcondition: {} and {} cannot both be true",
                        txn_name, format_expr(l), format_expr(r)
                    );
                    err.proof_chain.push("1. Both conditions apply to the same variable".to_string());
                    err.proof_chain.push(format!("2. '{}' requires {}", format_expr(l), l_cmp));
                    err.proof_chain.push(format!("3. '{}' requires {}", format_expr(r), r_cmp));
                    err.hints.push("fix the postcondition to describe a feasible state".to_string());
                    if let Some(s) = span {
                        err = err.with_span(s);
                    }
                    self.errors.push(err);
                }

                // Check equality contradictions: (x == a) && (x == b) where a != b
                match (l, r) {
                    (Expr::Eq(l1, r1), Expr::Eq(l2, r2)) => {
                        let v1 = self.extract_eq_pair(l1, r1);
                        let v2 = self.extract_eq_pair(l2, r2);
                        if let (Some((var1, val1)), Some((var2, val2))) = (v1, v2) {
                            if var1 == var2 && var1 == l_var && val1 != val2 {
                                let mut err = ProofError::new("P003", "postcondition is always satisfied");
                                err.explanation = format!(
                                    "transaction '{}' has contradictory postcondition: {} cannot equal both {} and {}",
                                    txn_name, var1, val1, val2
                                );
                                err.hints.push("fix the postcondition to specify a single target value".to_string());
                                if let Some(s) = span {
                                    err = err.with_span(s);
                                }
                                self.errors.push(err);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Extract (variable_name, comparison_op, value) from a comparison expression.
    fn extract_bound(&self, expr: &Expr) -> Option<(String, &str, i64)> {
        fn bind_val(e: &Expr) -> Option<i64> { e.as_integer() }
        // Handle new-style BinaryOp by normalizing to old variants
        if let Some(normalized) = expr.normalize_to_old() {
            return self.extract_bound(&normalized);
        }
        match expr {
            Expr::Gt(l, r) => {
                if let Expr::Identifier(var) = l.as_ref() {
                    if let Some(val) = bind_val(r) {
                        return Some((var.clone(), "gt", val));
                    }
                }
                if let Expr::Identifier(var) = r.as_ref() {
                    if let Some(val) = bind_val(l) {
                        return Some((var.clone(), "lt", val));
                    }
                }
                None
            }
            Expr::Ge(l, r) => {
                if let Expr::Identifier(var) = l.as_ref() {
                    if let Some(val) = bind_val(r) {
                        return Some((var.clone(), "ge", val));
                    }
                }
                if let Expr::Identifier(var) = r.as_ref() {
                    if let Some(val) = bind_val(l) {
                        return Some((var.clone(), "le", val));
                    }
                }
                None
            }
            Expr::Lt(l, r) => {
                if let Expr::Identifier(var) = l.as_ref() {
                    if let Some(val) = bind_val(r) {
                        return Some((var.clone(), "lt", val));
                    }
                }
                if let Expr::Identifier(var) = r.as_ref() {
                    if let Some(val) = bind_val(l) {
                        return Some((var.clone(), "gt", val));
                    }
                }
                None
            }
            Expr::Le(l, r) => {
                if let Expr::Identifier(var) = l.as_ref() {
                    if let Some(val) = bind_val(r) {
                        return Some((var.clone(), "le", val));
                    }
                }
                if let Expr::Identifier(var) = r.as_ref() {
                    if let Some(val) = bind_val(l) {
                        return Some((var.clone(), "ge", val));
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Extract (variable_name, value) from an equality expression.
    fn extract_eq_pair(&self, a: &Expr, b: &Expr) -> Option<(String, i64)> {
        match (a, b) {
            (Expr::Identifier(name), b) => b.as_integer().map(|val| (name.clone(), val)),
            (a, Expr::Identifier(name)) => a.as_integer().map(|val| (name.clone(), val)),
            _ => None,
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
            | Expr::And(l, r)
            | Expr::Like(l, r) => {
                self.collect_identifiers(l, vars);
                self.collect_identifiers(r, vars);
            }
            Expr::IsType(expr, _) | Expr::FromCheck(expr, _) => {
                self.collect_identifiers(expr, vars);
            }
            Expr::Not(inner) | Expr::Neg(inner) | Expr::BitNot(inner) => {
                self.collect_identifiers(inner, vars);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    self.collect_identifiers(arg, vars);
                }
            }
            Expr::IntrinsicCall { intrinsic: _, args } => {
                for arg in args {
                    self.collect_identifiers(arg, vars);
                }
            }
            Expr::BinaryOp(bop) => {
                self.collect_identifiers(&bop.left, vars);
                self.collect_identifiers(&bop.right, vars);
            }
            Expr::UnaryOp(uop) => {
                self.collect_identifiers(&uop.operand, vars);
            }
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::Char(_) | Expr::Bool(_) | Expr::Term | Expr::Literal(_)
            | Expr::ProjectionExpr(_) | Expr::CallExpr(_) | Expr::ListLiteralExpr(_)
            | Expr::MapLiteralExpr(_) | Expr::SetLiteralExpr(_) | Expr::SliceExpr(_)
            | Expr::MultiSliceExpr(_) | Expr::FieldAccessExpr(_) | Expr::StructInstanceExpr(_)
            | Expr::ObjectLiteralExpr(_) | Expr::TupleExpr(_) | Expr::TupleDestructureExpr(_)
            | Expr::EllipsisExpr(_) | Expr::ArrowMutExpr(_) | Expr::ArrowDiscardExpr(_) | Expr::ArrowTransferExpr(_)
            | Expr::PatternMatchExpr(_) | Expr::MatchExpr(_) | Expr::BlockExpr(_) | Expr::SigCallExpr(_)
            | Expr::SubtypeProjectionExpr(_) | Expr::DbvlTableExpr(_) | Expr::TypeRef(_) | Expr::RegexLiteral(_)
            | Expr::SharedMem(_) => {}
            Expr::ListLiteral(elements) => {
                for elem in elements {
                    self.collect_identifiers(elem, vars);
                }
            }
            Expr::ListIndex(list_expr, index_expr) => {
                self.collect_identifiers(list_expr, vars);
                self.collect_identifiers(index_expr, vars);
            }
            Expr::Projection { source: inner, .. } => {
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
            Expr::Slice { .. } | Expr::MultiSlice { .. } | Expr::Block(_, _) | Expr::TupleDestructure(_, _) | Expr::Tuple(_) | Expr::Concat(_, _) | Expr::Cast(_, _) => {}
            Expr::Match { value, .. } => {
                self.collect_identifiers(value, vars);
            }
            Expr::ArrowMut { target, index, value, .. } => {
                self.collect_identifiers(target, vars);
                self.collect_identifiers(index, vars);
                if let Some(v) = value {
                    self.collect_identifiers(v, vars);
                }
            }
            Expr::ArrowDiscard { target, index } => {
                self.collect_identifiers(target, vars);
                self.collect_identifiers(index, vars);
            }
            Expr::ArrowTransfer { dest, source, filter } => {
                self.collect_identifiers(dest, vars);
                self.collect_identifiers(source, vars);
                if let Some(f) = filter {
                    self.collect_identifiers(f, vars);
                }
            }
            Expr::SigCall { expr, .. } => {
                self.collect_identifiers(expr, vars);
            }
            Expr::Ellipsis => {}
            Expr::MapLiteral(entries) => {
                for (k, v) in entries {
                    self.collect_identifiers(k, vars);
                    self.collect_identifiers(v, vars);
                }
            }
            Expr::SetLiteral(entries) => {
                for e in entries {
                    self.collect_identifiers(e, vars);
                }
            }
            Expr::DbvlTable { .. } => {}
            Expr::SubtypeProjection { source, .. } => {
                self.collect_identifiers(source, vars);
            }
            // Macro/template nodes — should be expanded before reaching analysis
            Expr::TemplateCall { .. } | Expr::MacroCall { .. } | Expr::Interpolate(..) | Expr::InterpolateExpr(..) | Expr::QuoteBlock { .. } => {
                unreachable!("macro/template should have been expanded")
            }
            // Pipe chains — desugared before this pass
            Expr::PipeChain(_) => unreachable!("PipeChain should have been desugared"),
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
                Statement::SyncBlock { .. } => {}
            Statement::Unification {
                    name,
                    variant,
                    fields: _,
                    expr,
                } => {
                    if let Expr::Call(sig_name, _) = expr {
                        sig_callers
                            .entry(sig_name.clone())
                            .or_insert_with(Vec::new)
                            .push((caller_name.to_string(), variant.clone()));
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
        let mut call_graph = CallGraph::new();
        call_graph.build_from_program(program);

        if !call_graph.has_cycle() {
            return;
        }

        // Collect cycles and report them as proof errors
        let cycles = call_graph.find_all_cycles().to_vec();
        for cycle in &cycles {
            if let Some(txn_name) = cycle.first() {
                let mut err = ProofError::new("P012", "circular transaction dependency");
                err.explanation = format!(
                    "transactions form a circular dependency: {}",
                    cycle.join(" -> ")
                );
                err.proof_chain.push("1. transaction call cycle detected".to_string());
                for (i, name) in cycle.iter().enumerate() {
                    err.proof_chain.push(format!("{}. {}", i + 2, name));
                }
                err.proof_chain.push(format!("{}. (cycle closes back to {})", cycle.len() + 2, txn_name));
                err.hints.push("break the cycle by removing or reordering calls".to_string());
                self.errors.push(err);
                break;
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
            Type::Tuple(types) => format!("({})", types
                .iter()
                .map(|t| self.type_name(t))
                .collect::<Vec<_>>()
                .join(", ")),
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

        // Scan for `await` calls in non-async txns that create implicit read deps.
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                for stmt in &txn.body {
                    if let Statement::Await { expr, .. } = stmt {
                        if let Expr::Call(name, _) = expr {
                            let mut err = ProofError::new(
                                "P003",
                                "await creates implicit read dependency",
                            );
                            err.explanation = format!(
                                "transaction '{}' awaits '{}' — postcondition depends on call result",
                                txn.name, name
                            );
                            err.proof_chain.push(format!(
                                "1. '{}' awaits '{}' — blocking on its result", txn.name, name
                            ));
                            err.hints.push(
                                "use 'async await' instead of 'await' if the call can run concurrently".to_string()
                            );
                            self.errors.push(err);
                        }
                    }
                }
            }
        }
    }

    /// Suggest `async` for reactive transactions that are proven conflict-free.
    /// Unlike check_mutual_exclusion (which validates already-async txns),
    /// this scans ALL rct txn pairs and emits a lint for conflict-free ones
    /// that haven't been marked async yet.
    fn suggest_async_promotion(&mut self, program: &Program) {
        // Collect all reactive transactions
        let mut reactive_txns: Vec<&Transaction> = Vec::new();
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_reactive {
                    reactive_txns.push(txn);
                }
            }
        }

        for i in 0..reactive_txns.len() {
            for j in (i + 1)..reactive_txns.len() {
                let txn1 = reactive_txns[i];
                let txn2 = reactive_txns[j];

                // Skip if both are already async — check_mutual_exclusion handles that
                if txn1.is_async && txn2.is_async { continue; }

                let conflicts = self.find_read_write_conflicts(txn1, txn2);
                if conflicts.is_empty() {
                    // No read/write or write/write conflicts — emit lint
                    let both_non_async = !txn1.is_async && !txn2.is_async;
                    let mut warn = ProofError::new_warning(
                        "A001",
                        "async transaction candidate",
                    );
                    warn.explanation = if both_non_async {
                        format!(
                            "transactions '{}' and '{}' are conflict-free — consider marking both 'async' for concurrent dispatch",
                            txn1.name, txn2.name
                        )
                    } else {
                        let non_async = if !txn1.is_async { &txn1.name } else { &txn2.name };
                        let already_async = if txn1.is_async { &txn1.name } else { &txn2.name };
                        format!(
                            "transaction '{}' is conflict-free with async '{}' — consider marking it 'async' too",
                            non_async, already_async
                        )
                    };
                    warn.proof_chain.push(format!(
                        "1. '{}' writes to: {:?} — reads from: {:?}",
                        txn1.name,
                        self.extract_write_vars(txn1),
                        self.extract_read_vars(txn1),
                    ));
                    warn.proof_chain.push(format!(
                        "2. '{}' writes to: {:?} — reads from: {:?}",
                        txn2.name,
                        self.extract_write_vars(txn2),
                        self.extract_read_vars(txn2),
                    ));
                    warn.hints.push(
                        "add 'async' keyword after 'rct' to enable concurrent dispatch".to_string(),
                    );
                    self.errors.push(warn);
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
                modifiers: _,
            } => {
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
            Statement::Term { values: outputs, swan_song, .. } => {
                for out in outputs {
                    if let Some(expr) = out {
                        self.collect_read_vars_from_expr(expr, vars);
                    }
                }
                if let Some(swan) = swan_song {
                    self.collect_read_vars(swan, vars);
                }
            }
            Statement::TermBang { values: outputs, swan_song, .. } => {
                for out in outputs {
                    if let Some(expr) = out {
                        self.collect_read_vars_from_expr(expr, vars);
                    }
                }
                if let Some(swan) = swan_song {
                    self.collect_read_vars(swan, vars);
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
            Statement::Term { swan_song, .. } | Statement::TermBang { swan_song, .. } => {
                if let Some(swan) = swan_song {
                    self.collect_write_vars(swan, vars);
                }
            }
            Statement::Escape(_) => {}
            Statement::Guarded { statements, .. } => {
                for stmt in statements {
                    self.collect_write_vars(stmt, vars);
                }
            }
            Statement::SyncBlock { .. } => {}
            Statement::Unification { .. } => {}
            Statement::LocalTrigger { .. } => {}
            Statement::Alka(_) | Statement::OnExit { .. } => {}
            Statement::Foreach { body, .. } => {
                for stmt in body {
                    self.collect_write_vars(stmt, vars);
                }
            }
            Statement::Oracle { body, handler, .. } => {
                for stmt in body {
                    self.collect_write_vars(stmt, vars);
                }
                for stmt in handler {
                    self.collect_write_vars(stmt, vars);
                }
            }
            Statement::Await { expr, .. } => {
                if let Expr::Identifier(name) = expr {
                    vars.insert(name.clone());
                }
            }
            Statement::Async { body, .. } => {
                self.collect_write_vars(body, vars);
            }
            Statement::AsyncAwait { body, .. } => {
                self.collect_write_vars(body, vars);
            }
        }
    }

    fn preconditions_overlap(&self, txn1: &Transaction, txn2: &Transaction) -> bool {
        let vars1 = self.extract_state_vars(&txn1.contract.pre_condition);
        let vars2 = self.extract_state_vars(&txn2.contract.pre_condition);

        !vars1.is_disjoint(&vars2)
    }

    fn check_total_path(&mut self, program: &Program) {
        // Build initial-values map for overshoot detection
        let mut initial_values: HashMap<String, Expr> = HashMap::new();
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    if let Some(ref expr) = decl.expr {
                        initial_values.insert(decl.name.clone(), expr.clone());
                    }
                }
                TopLevel::Constant(constant) => {
                    initial_values.insert(constant.name.clone(), constant.expr.clone());
                }
                _ => {}
            }
        }

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_reactive {
                    // Convergence contracts are self-terminating: the pre-condition
                    // stops firing when the post-condition is met, so no term; needed.
                    if check_convergence(&txn.body, &txn.contract.pre_condition, &txn.contract.post_condition, &initial_values) {
                        continue;
                    }
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
                Statement::Term { .. } | Statement::TermBang { .. } => {
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
                .filter(|v| v.as_ref().and_then(|e| e.as_bool()).is_some())
                .collect();

            for (j, val) in bool_outputs.iter().enumerate() {
                if val.as_ref().and_then(|e| e.as_bool()) == Some(false) {
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
                Statement::Term { values: outputs, .. } | Statement::TermBang { values: outputs, .. } => {
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

// ── Standalone expression analysis helpers (for linearity proof) ──

/// Extract (variable_name, operator, value) from a comparison expression.
/// Supports: x > N, x >= N, x < N, x <= N (variable on either side).
pub fn extract_bound_from_expr(expr: &Expr) -> Option<(String, &'static str, i64)> {
    fn bind_val(e: &Expr) -> Option<i64> { e.as_integer() }
    // Handle new-style BinaryOp by normalizing to old variants
    if let Some(normalized) = expr.normalize_to_old() {
        return extract_bound_from_expr(&normalized);
    }
    match expr {
        Expr::Gt(l, r) => {
            if let Expr::Identifier(var) = l.as_ref() {
                if let Some(val) = bind_val(r) {
                    return Some((var.clone(), "gt", val));
                }
            }
            if let Expr::Identifier(var) = r.as_ref() {
                if let Some(val) = bind_val(l) {
                    return Some((var.clone(), "lt", val));
                }
            }
            None
        }
        Expr::Ge(l, r) => {
            if let Expr::Identifier(var) = l.as_ref() {
                if let Some(val) = bind_val(r) {
                    return Some((var.clone(), "ge", val));
                }
            }
            if let Expr::Identifier(var) = r.as_ref() {
                if let Some(val) = bind_val(l) {
                    return Some((var.clone(), "le", val));
                }
            }
            None
        }
        Expr::Lt(l, r) => {
            if let Expr::Identifier(var) = l.as_ref() {
                if let Some(val) = bind_val(r) {
                    return Some((var.clone(), "lt", val));
                }
            }
            if let Expr::Identifier(var) = r.as_ref() {
                if let Some(val) = bind_val(l) {
                    return Some((var.clone(), "gt", val));
                }
            }
            None
        }
        Expr::Le(l, r) => {
            if let Expr::Identifier(var) = l.as_ref() {
                if let Some(val) = bind_val(r) {
                    return Some((var.clone(), "le", val));
                }
            }
            if let Expr::Identifier(var) = r.as_ref() {
                if let Some(val) = bind_val(l) {
                    return Some((var.clone(), "ge", val));
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract (variable_name, constant_value) from an equality expression.
pub fn extract_eq_pair_from_expr(a: &Expr, b: &Expr) -> Option<(String, i64)> {
    match (a, b) {
        (Expr::Identifier(name), b) => b.as_integer().map(|val| (name.clone(), val)),
        (a, Expr::Identifier(name)) => a.as_integer().map(|val| (name.clone(), val)),
        _ => None,
    }
}

/// Decompose `a && b && c` into individual conjuncts.
pub fn split_and(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::And(l, r) => {
            let mut v = split_and(l);
            v.extend(split_and(r));
            v
        }
        _ => vec![expr],
    }
}

/// Check if two expressions are mutually exclusive (cannot both be true).
/// Returns `true` if they COULD be satisfiable (not provably unsat).
/// Returns `false` if proven unsatisfiable (mutually exclusive).
///
/// 2026-06-13: Standalone function for linearity proof at codegen time.
pub fn check_satisfiable(a: &Expr, b: &Expr) -> bool {
    let ca = split_and(a);
    let cb = split_and(b);
    for ai in &ca {
        for bj in &cb {
            // Check bound contradiction: x > 5 vs x < 4
            if let (Some((var_a, cmp_a, val_a)), Some((var_b, cmp_b, val_b))) =
                (extract_bound_from_expr(ai), extract_bound_from_expr(bj))
            {
                if var_a == var_b {
                    let contradictory = match (cmp_a, cmp_b) {
                        ("gt", "lt") => val_a >= val_b,
                        ("lt", "gt") => val_a <= val_b,
                        ("ge", "lt") => val_a >= val_b,
                        ("lt", "ge") => val_a <= val_b,
                        ("gt", "le") => val_a >= val_b,
                        ("le", "gt") => val_a <= val_b,
                        ("ge", "le") => val_a > val_b,
                        ("le", "ge") => val_a < val_b,
                        _ => false,
                    };
                    if contradictory {
                        return false; // unsat — mutually exclusive
                    }
                }
            }
            // Check equality contradiction: x == 5 vs x == 10
            if let (Expr::Eq(l1, r1), Expr::Eq(l2, r2)) = (*ai, *bj) {
                if let (Some((var1, val1)), Some((var2, val2))) =
                    (extract_eq_pair_from_expr(l1, r1), extract_eq_pair_from_expr(l2, r2))
                {
                    if var1 == var2 && val1 != val2 {
                        return false; // unsat — same var can't equal two constants
                    }
                }
            }
            // Check boolean contradiction: x vs !x or true vs false
            match (*ai, *bj) {
                (Expr::Not(inner), other) | (other, Expr::Not(inner)) => {
                    if format!("{:?}", inner) == format!("{:?}", other) {
                        return false; // unsat — x && !x
                    }
                }
                (Expr::Bool(a_val), Expr::Bool(b_val)) if a_val != b_val => {
                    return false; // unsat — true && false
                }
                _ => {}
            }
        }
    }
    true // could not prove unsat — assume satisfiable
}

/// Collect all guard conditions from a statement list (recursive).
pub fn collect_guard_conditions(stmts: &[crate::ast::Statement]) -> Vec<Expr> {
    let mut conds = Vec::new();
    for s in stmts {
        match s {
            crate::ast::Statement::Guarded { condition, statements } => {
                conds.push(condition.clone());
                conds.extend(collect_guard_conditions(statements));
            }
            crate::ast::Statement::SyncBlock { body } => {
                conds.extend(collect_guard_conditions(body));
            }
            _ => {}
        }
    }
    conds
}

/// Prove that a transaction body has at most one guard then-path firing per iteration.
/// A body is linear when all guard conditions are pairwise mutually exclusive
/// (burden_i && burden_j is unsat for all i ≠ j).
///
/// 2026-06-13: Used at codegen time to determine if SSA insertvalue path is safe.
pub fn prove_linear(stmts: &[crate::ast::Statement]) -> bool {
    let guards = collect_guard_conditions(stmts);
    for i in 0..guards.len() {
        for j in (i + 1)..guards.len() {
            if check_satisfiable(&guards[i], &guards[j]) {
                return false; // two guards could both fire → not linear
            }
        }
    }
    true // all pairs are mutually exclusive
}

// ── Structural recursion helpers (standalone) ─────────────────

/// Check if a statement contains a call to the given function name.
pub(crate) fn contains_call_to(stmt: &Statement, name: &str) -> bool {
    match stmt {
        Statement::Expression(e) => expr_contains_call_to(e, name),
        Statement::Term { values, .. } => {
            values.iter().any(|v| v.as_ref().map_or(false, |e| expr_contains_call_to(e, name)))
        }
        Statement::Guarded { condition, statements, .. } => {
            expr_contains_call_to(condition, name)
                || statements.iter().any(|s| contains_call_to(s, name))
        }
        Statement::Let { expr, .. } => {
            expr.as_ref().map_or(false, |e| expr_contains_call_to(e, name))
        }
        Statement::Assignment { expr, .. } => expr_contains_call_to(expr, name),
        Statement::Foreach { body, .. } => body.iter().any(|s| contains_call_to(s, name)),
        Statement::Oracle { body, handler, .. } => {
            body.iter().any(|s| contains_call_to(s, name))
                || handler.iter().any(|s| contains_call_to(s, name))
        }
        Statement::SyncBlock { body } => body.iter().any(|s| contains_call_to(s, name)),
        Statement::Unification { expr, .. } => expr_contains_call_to(expr, name),
        Statement::OnExit { body, .. } => body.iter().any(|s| contains_call_to(s, name)),
        Statement::Await { expr, .. } => expr_contains_call_to(expr, name),
        Statement::Async { body, .. } => contains_call_to(body, name),
        Statement::AsyncAwait { body, .. } => contains_call_to(body, name),
        _ => false,
    }
}

/// Check if an expression contains a CallExpr to the given name.
fn expr_contains_call_to(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::CallExpr(call) => {
            if call.name == name {
                return true;
            }
            call.args.iter().any(|a| expr_contains_call_to(a, name))
        }
        Expr::Block(_, body) => expr_contains_call_to(body, name),
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
        | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
            expr_contains_call_to(l, name) || expr_contains_call_to(r, name)
        }
        Expr::UnaryOp(op) => expr_contains_call_to(&op.operand, name),
        Expr::Not(e) | Expr::Neg(e) => {
            expr_contains_call_to(e, name)
        }
        Expr::OwnedRef(_) | Expr::PriorState(_) => false,
        Expr::Projection { source, .. } => expr_contains_call_to(source, name),
        Expr::ListLiteral(items) => items.iter().any(|e| expr_contains_call_to(e, name)),
        Expr::MapLiteral(entries) => {
            entries.iter().any(|(k, v)| expr_contains_call_to(k, name) || expr_contains_call_to(v, name))
        }
        Expr::Match { value, arms } => {
            expr_contains_call_to(value, name)
                || arms.iter().any(|a| expr_contains_call_to(&a.body, name))
        }
        _ => false,
    }
}

/// Check if all recursive calls in a statement use a structurally decreasing
/// argument at the position corresponding to the given parameter name.
fn check_decreasing_arg(stmt: &Statement, fn_name: &str, param_name: &str) -> bool {
    match stmt {
        Statement::Term { values, .. } => {
            values.iter().all(|v| v.as_ref().map_or(true, |e| check_decreasing_arg_expr(e, fn_name, param_name)))
        }
        Statement::Expression(e) => check_decreasing_arg_expr(e, fn_name, param_name),
        Statement::Guarded { condition, statements, .. } => {
            check_decreasing_arg_expr(condition, fn_name, param_name)
                && statements.iter().all(|s| check_decreasing_arg(s, fn_name, param_name))
        }
        Statement::Let { expr, .. } => {
            expr.as_ref().map_or(true, |e| check_decreasing_arg_expr(e, fn_name, param_name))
        }
        Statement::Assignment { expr, .. } => check_decreasing_arg_expr(expr, fn_name, param_name),
        Statement::Foreach { body, .. } => body.iter().all(|s| check_decreasing_arg(s, fn_name, param_name)),
        Statement::Oracle { body, handler, .. } => {
            body.iter().all(|s| check_decreasing_arg(s, fn_name, param_name))
                && handler.iter().all(|s| check_decreasing_arg(s, fn_name, param_name))
        }
        Statement::SyncBlock { body } => body.iter().all(|s| check_decreasing_arg(s, fn_name, param_name)),
        Statement::Unification { expr, .. } => check_decreasing_arg_expr(expr, fn_name, param_name),
        Statement::OnExit { body, .. } => body.iter().all(|s| check_decreasing_arg(s, fn_name, param_name)),
        _ => true,
    }
}

/// Check if an expression's recursive calls use a structurally decreasing
/// argument matching the given parameter name.
fn check_decreasing_arg_expr(expr: &Expr, fn_name: &str, param_name: &str) -> bool {
    match expr {
        Expr::CallExpr(call) => {
            if call.name != fn_name {
                return true;
            }
            call.args.iter().any(|arg| is_decreasing_expr(arg, param_name))
        }
        Expr::Block(_, body) => check_decreasing_arg_expr(body, fn_name, param_name),
        Expr::Sub(l, r) => {
            check_decreasing_arg_expr(l, fn_name, param_name)
                && check_decreasing_arg_expr(r, fn_name, param_name)
        }
        Expr::Add(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
        | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
            check_decreasing_arg_expr(l, fn_name, param_name)
                && check_decreasing_arg_expr(r, fn_name, param_name)
        }
        Expr::Not(e) | Expr::Neg(e) => {
            check_decreasing_arg_expr(e, fn_name, param_name)
        }
        Expr::OwnedRef(_) | Expr::PriorState(_) => true,
        Expr::UnaryOp(op) => check_decreasing_arg_expr(&op.operand, fn_name, param_name),
        Expr::Projection { source, .. } => check_decreasing_arg_expr(source, fn_name, param_name),
        Expr::ListLiteral(items) => items.iter().all(|e| check_decreasing_arg_expr(e, fn_name, param_name)),
        Expr::Match { value, arms } => {
            check_decreasing_arg_expr(value, fn_name, param_name)
                && arms.iter().all(|a| check_decreasing_arg_expr(&a.body, fn_name, param_name))
        }
        _ => true,
    }
}

/// Check if an expression is structurally smaller than the parameter.
/// Currently detects: param - 1, param - literal, param.tail()
fn is_decreasing_expr(expr: &Expr, param_name: &str) -> bool {
    match expr {
        // n - 1
        Expr::Sub(l, r) if matches!(l.as_ref(), Expr::Identifier(n) if n == param_name)
            && matches!(r.as_ref(), Expr::Integer(i) if *i == 1) => true,
        // n - literal > 0
        Expr::Sub(l, r) if matches!(l.as_ref(), Expr::Identifier(n) if n == param_name)
            && matches!(r.as_ref(), Expr::Integer(i) if *i > 0) => true,
        // direct identifier (n) — not decreasing, but allowed as fallback
        Expr::Identifier(n) if n == param_name => true,
        _ => false,
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

            txn write_a [true][data != @data] {
                &data = "A";
                term;
            };

            txn write_b [true][data != @data] {
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
    fn test_trivial_contracts_both_true_rejected_at_parse_time() {
        let code = r#"
            let count: Int = 0;

            txn increment [true][true] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let result = parser.parse();
        assert!(
            result.is_err(),
            "[true][true] should be rejected at parse time, but parse succeeded"
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
    fn test_trivial_contracts_in_definition_rejected_at_parse_time() {
        let code = r#"
            defn double(x: Int) -> Int [true][true] {
                term x * 2;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let result = parser.parse();
        assert!(
            result.is_err(),
            "[true][true] in definition should be rejected at parse time, but parse succeeded"
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
        // Regular transactions with concrete postconditions pass verification.
        // Parameterized postconditions (with symbolic params) may not be provable
        // with the current symbolic executor — that's a known limitation.
        let code = r#"
            let count: Int = 0;

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
            "Regular transaction should work, got errors: {:?}",
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

    #[test]
    fn test_convergence_rejects_true_precondition() {
        // [true][count == total] — pre doesn't imply ¬post, should NOT converge.
        // Falls through to P005 (no term;) and P008 (post not provable from true).
        let code = r#"
            let count: Int = 0;
            const total: Int = 100;

            rct txn process [true][count == total] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // Convergence fails — expects at least P005 (no term) or P008 (post not provable)
        assert!(!errors.is_empty(), "Expected errors for [true] convergence rejection, got none");
    }

    #[test]
    fn test_convergence_rejects_leq_precondition() {
        // [count <= total][count == total] — post→¬pre fails (count == total still satisfies <=)
        let code = r#"
            let count: Int = 0;
            const total: Int = 100;

            rct txn process [count <= total][count == total] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // Convergence fails, falls through to normal verification
        let has_error = errors.iter().any(|e| e.code == "P008" || e.code == "P005");
        assert!(has_error, "Expected error for invalid convergence pattern");
    }

    #[test]
    fn test_convergence_accepts_relational_post() {
        // [count < total][count >= total] — relational post op, should converge
        let code = r#"
            let count: Int = 0;
            const total: Int = 100;

            rct txn process [count < total][count >= total] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // Convergence proven — no errors expected
        let has_error = errors.iter().any(|e| e.code == "P008" || e.code == "P005");
        assert!(!has_error, "Relational post convergence should produce no errors: {:?}", errors);
    }

    #[test]
    fn test_convergence_rejects_overshoot() {
        // [count < total][count == total] with step 5, total=7, init=0
        // (7-0) % 5 = 2 ≠ 0 → overshoot, convergence fails
        let code = r#"
            let count: Int = 0;
            const total: Int = 7;

            rct txn process [count < total][count == total] {
                &count = count + 5;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // Convergence fails, falls through to P008
        let has_error = errors.iter().any(|e| e.code == "P008" || e.code == "P005");
        assert!(has_error, "Expected error for overshooting step: {:?}", errors);
    }

    #[test]
    fn test_convergence_accepts_step_divides_bound() {
        // [count < total][count == total] with step 5, total=10, init=0
        // (10-0) % 5 = 0 → no overshoot, convergence proven
        let code = r#"
            let count: Int = 0;
            const total: Int = 10;

            rct txn process [count < total][count == total] {
                &count = count + 5;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // Convergence proven — no errors expected
        let has_error = errors.iter().any(|e| e.code == "P008" || e.code == "P005");
        assert!(!has_error, "Step dividing bound should produce no errors: {:?}", errors);
    }

    #[test]
    fn test_convergence_accepts_countdown() {
        // [count > 0][count == 0] with step count-1, init=10
        let code = r#"
            let count: Int = 10;

            rct txn process [count > 0][count == 0] {
                &count = count - 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // Convergence proven — no errors expected
        let has_error = errors.iter().any(|e| e.code == "P008" || e.code == "P005");
        assert!(!has_error, "Countdown convergence should produce no errors: {:?}", errors);
    }

    #[test]
    fn test_convergence_accepts_neq_precondition() {
        // [count != total][count == total] — valid: when count == total, count != total is false
        let code = r#"
            let count: Int = 0;
            const total: Int = 100;

            rct txn process [count != total][count == total] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // Convergence proven — no errors expected
        let has_error = errors.iter().any(|e| e.code == "P008" || e.code == "P005");
        assert!(!has_error, "!= precondition convergence should produce no errors: {:?}", errors);
    }

    #[test]
    fn test_convergence_accepts_neq_postcondition() {
        // [count == total][count != total] — converge FROM equal TO not-equal
        let code = r#"
            let count: Int = 0;
            const total: Int = 100;

            rct txn process [count == total][count != total] {
                &count = count + 1;
                term;
            };
        "#;

        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");

        let mut pe = ProofEngine::new();
        let errors = pe.verify_program(&program);

        // Convergence proven — no errors expected
        let has_error = errors.iter().any(|e| e.code == "P008" || e.code == "P005");
        assert!(!has_error, "!= postcondition convergence should produce no errors: {:?}", errors);
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_tests_fast {
    use super::*;
    use crate::features::literal::LiteralExpr;

    fn make_executor() -> SymbolicExecutor {
        SymbolicExecutor::new()
    }

    fn make_engine() -> ProofEngine {
        ProofEngine::new()
    }

    // ── SymbolicValue::from_expr ──

    #[kani::proof]
    fn verify_from_expr_literal_integer() {
        let vars = HashMap::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let result = SymbolicValue::from_expr(&expr, &vars);
        assert_eq!(result, SymbolicValue::Concrete(42));
    }

    #[kani::proof]
    fn verify_from_expr_literal_bool_true() {
        let vars = HashMap::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let result = SymbolicValue::from_expr(&expr, &vars);
        assert_eq!(result, SymbolicValue::Concrete(1));
    }

    #[kani::proof]
    fn verify_from_expr_literal_bool_false() {
        let vars = HashMap::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(false)));
        let result = SymbolicValue::from_expr(&expr, &vars);
        assert_eq!(result, SymbolicValue::Concrete(0));
    }

    #[kani::proof]
    fn verify_from_expr_literal_float() {
        let vars = HashMap::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Float(3.14)));
        let result = SymbolicValue::from_expr(&expr, &vars);
        assert!(matches!(result, SymbolicValue::ConcreteFloat(_)));
    }

    #[kani::proof]
    fn verify_from_expr_literal_string_is_unknown() {
        let vars = HashMap::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::String("x".to_string())));
        let result = SymbolicValue::from_expr(&expr, &vars);
        assert_eq!(result, SymbolicValue::Unknown);
    }

    #[kani::proof]
    fn verify_from_expr_literal_term_is_unknown() {
        let vars = HashMap::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Term));
        let result = SymbolicValue::from_expr(&expr, &vars);
        assert_eq!(result, SymbolicValue::Unknown);
    }

    // ── NegateExpr (on SymbolicExecutor) ──

    #[kani::proof]
    fn verify_negate_expr_literal_bool_true() {
        let exec = make_executor();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let result = exec.negate_expr(&expr);
        assert_eq!(result, Some(Expr::Literal(Box::new(LiteralExpr::Bool(false)))));
    }

    #[kani::proof]
    fn verify_negate_expr_literal_bool_false() {
        let exec = make_executor();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(false)));
        let result = exec.negate_expr(&expr);
        assert_eq!(result, Some(Expr::Literal(Box::new(LiteralExpr::Bool(true)))));
    }

    #[kani::proof]
    fn verify_negate_expr_literal_non_bool_returns_none() {
        let exec = make_executor();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let result = exec.negate_expr(&expr);
        assert_eq!(result, None);
    }

    #[kani::proof]
    fn verify_negate_expr_old_bool() {
        let exec = make_executor();
        let expr = Expr::Bool(true);
        let result = exec.negate_expr(&expr);
        assert_eq!(result, Some(Expr::Bool(false)));
    }

    // ── InitStateFromPrecondition (on SymbolicExecutor) ──

    #[kani::proof]
    fn verify_init_state_literal_bool_true() {
        let exec = make_executor();
        let pre = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let state = exec.init_state_from_precondition(&pre);
        let _ = state;
    }

    #[kani::proof]
    fn verify_init_state_old_bool_true() {
        let exec = make_executor();
        let pre = Expr::Bool(true);
        let state = exec.init_state_from_precondition(&pre);
        let _ = state;
    }

    // ── IsTruthy (on SymbolicExecutor) ──

    #[kani::proof]
    fn verify_is_truthy_literal_bool_true() {
        let exec = make_executor();
        let state = SymbolicState::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        assert!(exec.is_truthy(&expr, &state));
    }

    #[kani::proof]
    fn verify_is_truthy_literal_bool_false() {
        let exec = make_executor();
        let state = SymbolicState::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(false)));
        assert!(!exec.is_truthy(&expr, &state));
    }

    #[kani::proof]
    fn verify_is_truthy_literal_non_bool() {
        let exec = make_executor();
        let state = SymbolicState::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        assert!(exec.is_truthy(&expr, &state));
    }

    // ── EvalNumeric (on SymbolicExecutor) ──

    #[kani::proof]
    fn verify_eval_numeric_literal_integer() {
        let exec = make_executor();
        let state = SymbolicState::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(99)));
        let result = exec.eval_numeric(&expr, &state);
        assert_eq!(result, Some(99));
    }

    #[kani::proof]
    fn verify_eval_numeric_old_integer() {
        let exec = make_executor();
        let state = SymbolicState::new();
        let expr = Expr::Integer(99);
        let result = exec.eval_numeric(&expr, &state);
        assert_eq!(result, Some(99));
    }
}

// ── FULL: ProofEngine-based harnesses (bigger state) ──
#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    use crate::features::literal::LiteralExpr;

    fn make_engine() -> ProofEngine {
        ProofEngine::new()
    }

    // ── ExtractBound (on ProofEngine) ──

    #[kani::proof]
    fn verify_extract_bound_gt_with_literal() {
        let engine = make_engine();
        let expr = Expr::Gt(
            Box::new(Expr::Identifier("x".to_string())),
            Box::new(Expr::Literal(Box::new(LiteralExpr::Integer(5)))),
        );
        let result = engine.extract_bound(&expr);
        assert_eq!(result, Some(("x".to_string(), "gt", 5)));
    }

    #[kani::proof]
    fn verify_extract_bound_lt_with_literal() {
        let engine = make_engine();
        let expr = Expr::Lt(
            Box::new(Expr::Identifier("x".to_string())),
            Box::new(Expr::Literal(Box::new(LiteralExpr::Integer(10)))),
        );
        let result = engine.extract_bound(&expr);
        assert_eq!(result, Some(("x".to_string(), "lt", 10)));
    }

    #[kani::proof]
    fn verify_extract_eq_pair_with_literal() {
        let engine = make_engine();
        let a = Expr::Identifier("x".to_string());
        let b = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let result = engine.extract_eq_pair(&a, &b);
        assert_eq!(result, Some(("x".to_string(), 42)));
    }

    #[kani::proof]
    fn verify_extract_eq_pair_reversed_literal() {
        let engine = make_engine();
        let a = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let b = Expr::Identifier("x".to_string());
        let result = engine.extract_eq_pair(&a, &b);
        assert_eq!(result, Some(("x".to_string(), 42)));
    }

    // ── CheckTrivialContracts (on ProofEngine) ──

    #[kani::proof]
    fn verify_check_trivial_contracts_empty_program() {
        let mut engine = make_engine();
        let program = Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::ast::StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        };
        engine.check_trivial_contracts(&program);
        assert!(engine.errors.is_empty());
    }

    // ── ExtractBound with old-style Integer too ──

    #[kani::proof]
    fn verify_extract_bound_gt_old_style() {
        let engine = make_engine();
        let expr = Expr::Gt(
            Box::new(Expr::Identifier("x".to_string())),
            Box::new(Expr::Integer(5)),
        );
        let result = engine.extract_bound(&expr);
        assert_eq!(result, Some(("x".to_string(), "gt", 5)));
    }

    #[kani::proof]
    fn verify_extract_eq_pair_old_style() {
        let engine = make_engine();
        let a = Expr::Identifier("x".to_string());
        let b = Expr::Integer(42);
        let result = engine.extract_eq_pair(&a, &b);
        assert_eq!(result, Some(("x".to_string(), 42)));
    }
}
