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

use crate::ast::{Contract, Expr, Program, Statement, TopLevel};
use crate::interpreter::{Interpreter, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ReactiveTransaction {
    pub name: String,
    pub contract: Contract,
    pub body: Vec<Statement>,
    pub is_async: bool,
    pub dependencies: HashSet<String>,
}

#[derive(Debug)]
pub struct Reactor {
    pub transactions: Vec<ReactiveTransaction>,
    pub dirty_preconditions: HashSet<usize>,
    pub dependency_map: HashMap<String, HashSet<usize>>,
}

impl Reactor {
    pub fn new() -> Self {
        Reactor {
            transactions: Vec::new(),
            dirty_preconditions: HashSet::new(),
            dependency_map: HashMap::new(),
        }
    }

    pub fn build_from_program(&mut self, program: &Program) {
        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                if txn.is_reactive {
                    let deps: HashSet<String> = txn.dependencies.iter().cloned().collect();
                    let rtxn = ReactiveTransaction {
                        name: txn.name.clone(),
                        contract: txn.contract.clone(),
                        body: txn.body.clone(),
                        is_async: txn.is_async,
                        dependencies: deps.clone(),
                    };
                    self.transactions.push(rtxn);
                    let txn_idx = self.transactions.len() - 1;
                    for var in deps {
                        self.dependency_map
                            .entry(var)
                            .or_insert_with(HashSet::new)
                            .insert(txn_idx);
                    }
                    self.dirty_preconditions.insert(txn_idx);
                }
            }
        }
    }

    pub fn mark_dirty(&mut self, variable: &str) {
        if let Some(txn_indices) = self.dependency_map.get(variable) {
            for &idx in txn_indices {
                self.dirty_preconditions.insert(idx);
            }
        }
    }

    pub fn get_dirty_transactions(&self) -> Vec<usize> {
        self.dirty_preconditions.iter().cloned().collect()
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_preconditions.clear();
    }

    /// Pre-evaluation guard: Check if a transaction will provably escape
    /// before running it. This avoids firing FFI calls in transactions that
    /// would just roll back anyway.
    ///
    /// Returns true if any escape guard condition is currently true.
    fn will_escape(&self, txn: &ReactiveTransaction, interp: &mut Interpreter) -> bool {
        for stmt in &txn.body {
            if self.contains_escape_guard(stmt, interp) {
                return true;
            }
        }
        false
    }

    /// Recursively check if a statement contains a guarded escape that would fire
    fn contains_escape_guard(&self, stmt: &Statement, interp: &mut Interpreter) -> bool {
        match stmt {
            Statement::Guarded { condition, statements } => {
                // Check if this guard's condition is currently true
                if let Ok(cond_val) = interp.eval_expr(condition) {
                    if cond_val == Value::Bool(true) {
                        // Check if any statement in the guard body is an escape
                        for s in statements {
                            if self.contains_escape(s) {
                                return true;
                            }
                        }
                    }
                }
                // Recursively check nested guards
                for s in statements {
                    if self.contains_escape_guard(s, interp) {
                        return true;
                    }
                }
                false
            }
            Statement::LocalTrigger { expr, .. } => {
                // Check if the trigger expression's result would trigger an escape
                // For now, just check the expression itself
                if let Some(e) = expr {
                    if self.contains_escape_in_expr(e, interp) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Check if a statement is an escape
    fn contains_escape(&self, stmt: &Statement) -> bool {
        matches!(stmt, Statement::Escape(_))
    }

    /// Check if an expression contains patterns that would lead to escape
    /// (e.g., error checks on FFI calls)
    fn contains_escape_in_expr(&self, _expr: &Expr, _interp: &mut Interpreter) -> bool {
        // TODO: Analyze expressions for error patterns that would trigger escapes
        // For now, conservative: don't skip
        false
    }

    pub fn run(&self, interp: &mut Interpreter) -> Result<bool, crate::interpreter::RuntimeError> {
        let mut any_executed = false;

        for &txn_idx in self.get_dirty_transactions().iter() {
            if let Some(txn) = self.transactions.get(txn_idx) {
                let pre_val = interp.eval_expr(&txn.contract.pre_condition)?;
                if pre_val == Value::Bool(true) {
                    // PRE-EVALUATION GUARD: Check if any escape conditions are provably true
                    // before running the transaction. If so, skip entirely to avoid FFI side effects.
                    if self.will_escape(txn, interp) {
                        continue;
                    }

                    interp.prior_state = interp.state.clone();

                    let mut term_executed = false;
                    let mut escape_triggered = false;

                    let max_iterations = 1000;
                    let mut iteration = 0;

                    while iteration < max_iterations {
                        iteration += 1;

                        let mut local_failed = false;
                        for stmt in &txn.body {
                            match self.execute_statement(interp, stmt) {
                                Ok(StmtResult::Continue) => {}
                                Ok(StmtResult::TermSuccess) => {
                                    let post_val =
                                        interp.eval_expr(&txn.contract.post_condition)?;
                                    if post_val == Value::Bool(true) {
                                        term_executed = true;
                                        any_executed = true;
                                        break;
                                    }
                                }
                                Ok(StmtResult::TermFailed) => {
                                    local_failed = true;
                                }
                                Ok(StmtResult::Escaped) => {
                                    escape_triggered = true;
                                    local_failed = true;
                                    break;
                                }
                                Err(_) => {
                                    local_failed = true;
                                    break;
                                }
                            }
                        }

                        if escape_triggered {
                            interp.state = interp.prior_state.clone();
                            break;
                        }

                        if term_executed {
                            break;
                        }

                        if local_failed && !term_executed {
                            interp.state = interp.prior_state.clone();
                            break;
                        }
                    }

                    if iteration >= max_iterations && !term_executed {
                        interp.state = interp.prior_state.clone();
                    }
                }
            }
        }

        Ok(any_executed)
    }

    fn execute_statement(
        &self,
        interp: &mut Interpreter,
        stmt: &Statement,
    ) -> Result<StmtResult, crate::interpreter::RuntimeError> {
        match stmt {
            Statement::Assignment { .. } => {
                interp.exec_stmt(stmt)?;
                Ok(StmtResult::Continue)
            }
            Statement::Let { name, expr, .. } => {
                if let Some(e) = expr {
                    let value = interp.eval_expr(e)?;
                    interp.state.insert(name.clone(), value);
                }
                Ok(StmtResult::Continue)
            }
            Statement::InlineAsm { .. } => {
                Ok(StmtResult::Continue)
            }
            Statement::Expression(expr) => {
                interp.eval_expr(expr)?;
                Ok(StmtResult::Continue)
            }
            Statement::Term { values: outputs, .. } | Statement::TermBang { values: outputs, .. } => {
                if let Some(first) = outputs.first() {
                    if let Some(expr) = first {
                        let value = interp.eval_expr(expr)?;
                        if value == Value::Bool(true) {
                            Ok(StmtResult::TermSuccess)
                        } else {
                            Ok(StmtResult::TermFailed)
                        }
                    } else {
                        Ok(StmtResult::TermSuccess)
                    }
                } else {
                    Ok(StmtResult::TermSuccess)
                }
            }
            Statement::Escape(_) => Ok(StmtResult::Escaped),
            Statement::Guarded {
                condition,
                statements,
            } => {
                let cond_val = interp.eval_expr(condition)?;
                if cond_val == Value::Bool(true) {
                    for stmt in statements {
                        let result = self.execute_statement(interp, stmt)?;
                        match result {
                            StmtResult::Continue => {}
                            _ => return Ok(result),
                        }
                    }
                    Ok(StmtResult::Continue)
                } else {
                    Ok(StmtResult::Continue)
                }
            }
            Statement::Unification { .. } => Ok(StmtResult::Continue),
            Statement::LocalTrigger { name, expr, .. } => {
                // Local trigger: evaluate expression if present, store result
                if let Some(e) = expr {
                    let value = interp.eval_expr(e)?;
                    interp.state.insert(name.clone(), value);
                }
                // TODO: Full async yield/await semantics with rollback support
                Ok(StmtResult::Continue)
            }
            Statement::SyncBlock { .. } => {
                interp.exec_stmt(stmt)?;
                Ok(StmtResult::Continue)
            }
            Statement::Alka(_) | Statement::OnExit { .. } => Ok(StmtResult::Continue),
        }
    }
}

enum StmtResult {
    Continue,
    TermSuccess,
    TermFailed,
    Escaped,
}

pub fn run_reactor(
    program: &Program,
    interp: &mut Interpreter,
) -> Result<(), crate::interpreter::RuntimeError> {
    let mut reactor = Reactor::new();
    reactor.build_from_program(program);

    loop {
        reactor.clear_dirty();
        let executed = reactor.run(interp)?;

        if !executed {
            let dirty = reactor.get_dirty_transactions();
            if dirty.is_empty() {
                break;
            }
        }

        let dirty = reactor.get_dirty_transactions();
        if dirty.is_empty() {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use crate::interpreter::{Interpreter, Value};

    fn make_rct_txn(name: &str, pre: Expr, post: Expr, body: Vec<Statement>) -> TopLevel {
        TopLevel::Transaction(Transaction {
            name: name.to_string(),
            is_reactive: true,
            is_async: false,
            parameters: vec![],
            contract: Contract { pre_condition: pre, post_condition: post, watchdog: None, span: None },
            body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        })
    }

    fn make_rct_txn_with_deps(name: &str, pre: Expr, post: Expr, body: Vec<Statement>, deps: Vec<String>) -> TopLevel {
        TopLevel::Transaction(Transaction {
            name: name.to_string(),
            is_reactive: true,
            is_async: false,
            parameters: vec![],
            contract: Contract { pre_condition: pre, post_condition: post, watchdog: None, span: None },
            body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: deps,
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        })
    }

    fn simple_program(items: Vec<TopLevel>) -> Program {
        Program { items, comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: StrictMode::Off, dispatch_mode: Default::default(), exit_condition: None, out_pragmas: vec![], default_sig_modifier: None }
    }

    fn build_reactor(prog: &Program) -> Reactor {
        let mut r = Reactor::new();
        r.build_from_program(prog);
        r
    }

    #[test]
    fn test_build_from_program_empty() {
        let reactor = build_reactor(&simple_program(vec![]));
        assert!(reactor.transactions.is_empty());
    }

    #[test]
    fn test_build_from_program_with_reactive_txn() {
        let txn = make_rct_txn("test", Expr::Bool(true), Expr::Bool(true), vec![]);
        let reactor = build_reactor(&simple_program(vec![txn]));
        assert_eq!(reactor.transactions.len(), 1);
        assert_eq!(reactor.transactions[0].name, "test");
    }

    #[test]
    fn test_build_from_program_skips_non_reactive() {
        let prog = simple_program(vec![
            TopLevel::Definition(Definition {
                name: "foo".into(), type_params: vec![], parameters: vec![], outputs: vec![],
                output_type: None, output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![], is_lambda: false, modifiers: vec![], variant_bodies: vec![],
            }),
            TopLevel::Transaction(Transaction {
                name: "bar".into(), is_reactive: false, is_async: false, parameters: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)), body: vec![],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
                attrs: vec![], modifiers: vec![], variant_bodies: vec![], outputs: vec![],
                output_type: None,
            }),
        ]);
        let reactor = build_reactor(&prog);
        assert_eq!(reactor.transactions.len(), 0);
    }

    #[test]
    fn test_dependency_map_populated() {
        let txn = make_rct_txn_with_deps("dep_test", Expr::Bool(true), Expr::Bool(true), vec![], vec!["x".into()]);
        let reactor = build_reactor(&simple_program(vec![txn]));
        assert!(reactor.dependency_map.contains_key("x"));
    }

    #[test]
    fn test_mark_dirty_propagates() {
        let txn = make_rct_txn_with_deps("a", Expr::Bool(true), Expr::Bool(true), vec![], vec!["y".into()]);
        let mut reactor = build_reactor(&simple_program(vec![txn]));
        assert!(reactor.dirty_preconditions.contains(&0usize));
        reactor.clear_dirty();
        assert!(reactor.dirty_preconditions.is_empty());
        reactor.mark_dirty("y");
        assert!(reactor.dirty_preconditions.contains(&0usize));
    }

    #[test]
    fn test_get_dirty_transactions() {
        let txn = make_rct_txn("a", Expr::Identifier("z".into()), Expr::Bool(true), vec![]);
        let reactor = build_reactor(&simple_program(vec![txn]));
        assert!(!reactor.get_dirty_transactions().is_empty());
    }

    #[test]
    fn test_run_executes_txn() {
        let body = vec![
            Statement::Assignment { lhs: Expr::Identifier("x".into()), expr: Expr::Integer(42), timeout: None, modifiers: vec![] },
            Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
        ];
        let txn = make_rct_txn("set_x",
            Expr::Bool(true),
            Expr::Eq(Box::new(Expr::Identifier("x".into())), Box::new(Expr::Integer(42))),
            body);
        let mut interp = Interpreter::new();
        interp.state.insert("x".into(), Value::Int(0));
        let reactor = build_reactor(&simple_program(vec![txn]));
        let result = reactor.run(&mut interp).unwrap();
        assert!(result);
        assert_eq!(interp.state.get("x"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_run_skips_txn_pre_false() {
        let body = vec![Statement::Assignment { lhs: Expr::Identifier("x".into()), expr: Expr::Integer(99), timeout: None, modifiers: vec![] }];
        let txn = make_rct_txn("skip", Expr::Bool(false), Expr::Bool(true), body);
        let mut interp = Interpreter::new();
        interp.state.insert("x".into(), Value::Int(0));
        let reactor = build_reactor(&simple_program(vec![txn]));
        let result = reactor.run(&mut interp).unwrap();
        assert!(!result);
        assert_eq!(interp.state.get("x"), Some(&Value::Int(0)));
    }

    #[test]
    fn test_escape_guard_detection() {
        let body = vec![
            Statement::Guarded {
                condition: Expr::Bool(true),
                statements: vec![Statement::Escape(None)],
            },
        ];
        let txn = make_rct_txn("escape_test", Expr::Bool(true), Expr::Bool(true), body);
        let mut interp = Interpreter::new();
        let reactor = build_reactor(&simple_program(vec![txn]));
        assert!(reactor.will_escape(&reactor.transactions[0], &mut interp));
    }

    #[test]
    fn test_run_escape_triggers_rollback() {
        let body = vec![
            Statement::Assignment { lhs: Expr::Identifier("x".into()), expr: Expr::Integer(10), timeout: None, modifiers: vec![] },
            Statement::Guarded { condition: Expr::Bool(true), statements: vec![Statement::Escape(None)] },
        ];
        let txn = make_rct_txn("rollback", Expr::Bool(true), Expr::Bool(true), body);
        let mut interp = Interpreter::new();
        interp.state.insert("x".into(), Value::Int(5));
        interp.prior_state = interp.state.clone();
        let reactor = build_reactor(&simple_program(vec![txn]));
        let result = reactor.run(&mut interp).unwrap();
        // Pre-evaluation guard skips the transaction entirely (escape would fire)
        assert!(!result);
        // State unchanged since transaction wasn't executed
        assert_eq!(interp.state.get("x"), Some(&Value::Int(5)));
    }
}
