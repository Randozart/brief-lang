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

    pub fn run(&self, interp: &mut Interpreter) -> Result<bool, crate::interpreter::RuntimeError> {
        let mut any_executed = false;

        for &txn_idx in self.get_dirty_transactions().iter() {
            if let Some(txn) = self.transactions.get(txn_idx) {
                let pre_val = interp.eval_expr(&txn.contract.pre_condition)?;
                if pre_val == Value::Bool(true) {
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
            Statement::Expression(expr) => {
                interp.eval_expr(expr)?;
                Ok(StmtResult::Continue)
            }
            Statement::Term(outputs) => {
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
