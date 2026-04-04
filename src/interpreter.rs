use crate::ast::*;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Data(Vec<u8>),
    Void,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::String(v) => write!(f, "\"{}\"", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Data(_) => write!(f, "<data>"),
            Value::Void => write!(f, "void"),
        }
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    UndefinedVariable(String),
    TypeMismatch(String),
    DivisionByZero,
    ContractViolation(String),
    UnhandledOutcome(String),
    UndefinedForeignFunction(String),
}

pub type ForeignFn = fn(Vec<Value>) -> Result<Value, RuntimeError>;

pub struct Interpreter {
    pub state: HashMap<String, Value>,
    pub prior_state: HashMap<String, Value>,
    pub foreign_functions: HashMap<String, ForeignFn>,
}

impl Interpreter {
    pub fn new() -> Self {
        let mut foreign_functions = HashMap::new();
        register_std_io(&mut foreign_functions);
        
        Interpreter {
            state: HashMap::new(),
            prior_state: HashMap::new(),
            foreign_functions,
        }
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        // Initialize state from declarations
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                let value = if let Some(expr) = &decl.expr {
                    self.eval_expr(expr)?
                } else {
                    // Default values based on type
                    match decl.ty {
                        Type::Int => Value::Int(0),
                        Type::Float => Value::Float(0.0),
                        Type::String => Value::String(String::new()),
                        Type::Bool => Value::Bool(false),
                        _ => Value::Void,
                    }
                };
                self.state.insert(decl.name.clone(), value);
            } else if let TopLevel::Constant(const_decl) = item {
                let value = self.eval_expr(&const_decl.expr)?;
                self.state.insert(const_decl.name.clone(), value);
            }
        }

        // Execute reactive transactions (simplified)
        // In a real implementation, we'd have a reactor loop
        // Here we just execute matching transactions once
        let mut executed = true;
        let mut iterations = 0;
        let max_iterations = 100;
        
        while executed && iterations < max_iterations {
            iterations += 1;
            executed = false;
            for item in &program.items {
                if let TopLevel::Transaction(txn) = item {
                    if txn.is_reactive {
                        // Check precondition
                        let pre_val = self.eval_expr(&txn.contract.pre_condition)?;
                        if pre_val == Value::Bool(true) {
                            // Save prior state
                            self.prior_state = self.state.clone();

                            // Execute body
                            let mut transaction_failed = false;
                            for stmt in &txn.body {
                                if let Err(_e) = self.exec_stmt(stmt) {
                                    // Rollback on error
                                    self.state = self.prior_state.clone();
                                    transaction_failed = true;
                                    break;
                                }
                            }

                            if !transaction_failed {
                                // Check postcondition
                                let post_val = self.eval_expr(&txn.contract.post_condition)?;
                                if post_val != Value::Bool(true) {
                                    // Rollback on failed postcondition
                                    self.state = self.prior_state.clone();
                                } else if self.state != self.prior_state {
                                    // Only mark as executed if state actually changed
                                    executed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if iterations >= max_iterations {
            eprintln!("Warning: Reactor loop hit iteration limit ({})", max_iterations);
        }

        Ok(())
    }

    fn exec_stmt(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        match stmt {
            Statement::Assignment {
                is_owned,
                name,
                expr,
            } => {
                let value = self.eval_expr(expr)?;
                if *is_owned {
                    // In Brief, &var = value claims write access
                    // For interpreter, just update the state
                    self.state.insert(name.clone(), value);
                } else {
                    // Local variable or reading
                    // In this simplified interpreter, we treat it as assignment to state
                    self.state.insert(name.clone(), value);
                }
            }
            Statement::Let { name, expr, .. } => {
                if let Some(expr) = expr {
                    let value = self.eval_expr(expr)?;
                    self.state.insert(name.clone(), value);
                }
            }
            Statement::Expression(expr) => {
                self.eval_expr(expr)?;
            }
            Statement::Term(outputs) => {
                if let Some(first) = outputs.first() {
                    if let Some(expr) = first {
                        let value = self.eval_expr(expr)?;
                        if value != Value::Bool(true) {
                            // In Brief, this would loop the transaction
                        }
                    }
                }
            }
            Statement::Escape(_expr_opt) => {
                // Escape causes rollback
                // We signal this by returning an error
                return Err(RuntimeError::ContractViolation(
                    "Transaction escaped".to_string(),
                ));
            }
            Statement::Guarded {
                condition,
                statement,
            } => {
                let cond_val = self.eval_expr(condition)?;
                if cond_val == Value::Bool(true) {
                    self.exec_stmt(statement)?;
                }
            }
            Statement::Unification { .. } => {
                // Unification is complex; for now, skip
            }
        }
        Ok(())
    }

    pub fn eval_expr(&self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Integer(v) => Ok(Value::Int(*v)),
            Expr::Float(v) => Ok(Value::Float(*v)),
            Expr::String(v) => Ok(Value::String(v.clone())),
            Expr::Bool(v) => Ok(Value::Bool(*v)),
            Expr::Identifier(name) => self
                .state
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            Expr::OwnedRef(name) => self
                .state
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            Expr::PriorState(name) => self
                .prior_state
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            Expr::Add(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l + r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l + r)),
                    _ => Err(RuntimeError::TypeMismatch("Addition".to_string())),
                }
            }
            Expr::Sub(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l - r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l - r)),
                    _ => Err(RuntimeError::TypeMismatch("Subtraction".to_string())),
                }
            }
            Expr::Mul(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l * r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l * r)),
                    _ => Err(RuntimeError::TypeMismatch("Multiplication".to_string())),
                }
            }
            Expr::Div(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => {
                        if r == 0 {
                            return Err(RuntimeError::DivisionByZero);
                        }
                        Ok(Value::Int(l / r))
                    }
                    (Value::Float(l), Value::Float(r)) => {
                        if r == 0.0 {
                            return Err(RuntimeError::DivisionByZero);
                        }
                        Ok(Value::Float(l / r))
                    }
                    _ => Err(RuntimeError::TypeMismatch("Division".to_string())),
                }
            }
            Expr::Eq(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                Ok(Value::Bool(l_val == r_val))
            }
            Expr::Ne(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                Ok(Value::Bool(l_val != r_val))
            }
            Expr::Lt(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l < r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l < r)),
                    _ => Err(RuntimeError::TypeMismatch("Less than".to_string())),
                }
            }
            Expr::Le(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l <= r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l <= r)),
                    _ => Err(RuntimeError::TypeMismatch("Less or equal".to_string())),
                }
            }
            Expr::Gt(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l > r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l > r)),
                    _ => Err(RuntimeError::TypeMismatch("Greater than".to_string())),
                }
            }
            Expr::Ge(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l >= r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l >= r)),
                    _ => Err(RuntimeError::TypeMismatch("Greater or equal".to_string())),
                }
            }
            Expr::Or(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Bool(l), Value::Bool(r)) => Ok(Value::Bool(l || r)),
                    _ => Err(RuntimeError::TypeMismatch("Logical OR".to_string())),
                }
            }
            Expr::And(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Bool(l), Value::Bool(r)) => Ok(Value::Bool(l && r)),
                    _ => Err(RuntimeError::TypeMismatch("Logical AND".to_string())),
                }
            }
            Expr::Not(inner) => {
                let val = self.eval_expr(inner)?;
                match val {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    _ => Err(RuntimeError::TypeMismatch("Logical NOT".to_string())),
                }
            }
            Expr::Neg(inner) => {
                let val = self.eval_expr(inner)?;
                match val {
                    Value::Int(i) => Ok(Value::Int(-i)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    _ => Err(RuntimeError::TypeMismatch("Negation".to_string())),
                }
            }
            Expr::BitNot(inner) => {
                let val = self.eval_expr(inner)?;
                match val {
                    Value::Int(i) => Ok(Value::Int(!i)),
                    _ => Err(RuntimeError::TypeMismatch("Bitwise NOT".to_string())),
                }
            }
            Expr::Call(name, args) => {
                if let Some(frgn_fn) = self.foreign_functions.get(name) {
                    let mut arg_values = Vec::new();
                    for arg in args {
                        arg_values.push(self.eval_expr(arg)?);
                    }
                    frgn_fn(arg_values)
                } else {
                    Err(RuntimeError::UndefinedForeignFunction(name.clone()))
                }
            }
        }
    }
}

fn register_std_io(registry: &mut HashMap<String, ForeignFn>) {
    registry.insert("print".to_string(), |args| {
        if let Value::String(s) = &args[0] {
            print!("{}", s);
            Ok(Value::Bool(true))
        } else {
            Err(RuntimeError::TypeMismatch("print expects String".to_string()))
        }
    });
    
    registry.insert("println".to_string(), |args| {
        if let Value::String(s) = &args[0] {
            println!("{}", s);
            Ok(Value::Bool(true))
        } else {
            Err(RuntimeError::TypeMismatch("println expects String".to_string()))
        }
    });
    
    registry.insert("input".to_string(), |_args| {
        use std::io::{self, BufRead};
        let stdin = io::stdin();
        let mut line = String::new();
        if let Ok(_) = stdin.lock().read_line(&mut line) {
            line.pop(); // Remove trailing newline
            Ok(Value::String(line))
        } else {
            Ok(Value::String(String::new()))
        }
    });
}
