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
                    let deps = self.extract_dependencies(&txn.contract.pre_condition);
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

    fn extract_dependencies(&self, expr: &Expr) -> HashSet<String> {
        let mut deps = HashSet::new();
        self.collect_identifiers(expr, &mut deps);
        deps
    }

    fn collect_identifiers(&self, expr: &Expr, deps: &mut HashSet<String>) {
        match expr {
            Expr::Identifier(name) => {
                deps.insert(name.clone());
            }
            Expr::OwnedRef(name) => {
                deps.insert(name.clone());
            }
            Expr::PriorState(name) => {
                deps.insert(name.clone());
            }
            Expr::Add(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Sub(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Mul(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Div(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Eq(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Ne(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Lt(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Le(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Gt(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Ge(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Or(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::And(l, r) => {
                self.collect_identifiers(l, deps);
                self.collect_identifiers(r, deps);
            }
            Expr::Not(inner) => {
                self.collect_identifiers(inner, deps);
            }
            Expr::Neg(inner) => {
                self.collect_identifiers(inner, deps);
            }
            Expr::BitNot(inner) => {
                self.collect_identifiers(inner, deps);
            }
            Expr::Call(_, args) => {
                for arg in args {
                    self.collect_identifiers(arg, deps);
                }
            }
            Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::Bool(_) => {}
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
                                    let post_val = interp.eval_expr(&txn.contract.post_condition)?;
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
            Statement::Assignment { is_owned, name, expr } => {
                let value = interp.eval_expr(expr)?;
                if *is_owned {
                    interp.state.insert(name.clone(), value);
                } else {
                    interp.state.insert(name.clone(), value);
                }
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
            Statement::Escape(_) => {
                Ok(StmtResult::Escaped)
            }
            Statement::Guarded { condition, statement } => {
                let cond_val = interp.eval_expr(condition)?;
                if cond_val == Value::Bool(true) {
                    self.execute_statement(interp, statement)
                } else {
                    Ok(StmtResult::Continue)
                }
            }
            Statement::Unification { .. } => {
                Ok(StmtResult::Continue)
            }
        }
    }
}

enum StmtResult {
    Continue,
    TermSuccess,
    TermFailed,
    Escaped,
}

pub fn run_reactor(program: &Program, interp: &mut Interpreter) -> Result<(), crate::interpreter::RuntimeError> {
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
