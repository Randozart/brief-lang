use crate::ast::*;
use crate::ffi::FFI_REGISTRY;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Data(Vec<u8>),
    List(Vec<Value>),
    Struct(HashMap<String, Value>),
    Defn(String),
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
            Value::List(items) => write!(f, "[{}]", items.len()),
            Value::Struct(fields) => write!(f, "{{{} fields}}", fields.len()),
            Value::Defn(name) => write!(f, "<defn {}>", name),
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
    pub definitions: HashMap<String, Definition>,
}

impl Interpreter {
    pub fn new() -> Self {
        let foreign_functions = Self::load_ffi_functions();

        Interpreter {
            state: HashMap::new(),
            prior_state: HashMap::new(),
            foreign_functions,
            definitions: HashMap::new(),
        }
    }

    fn load_ffi_functions() -> HashMap<String, ForeignFn> {
        let mut functions = HashMap::new();
        let registry = &*FFI_REGISTRY;

        for (name, func) in registry.functions.iter() {
            let short_name = name.replace("std::", "__");
            functions.insert(short_name, *func);
        }

        functions
    }

    fn call_defn(&mut self, name: &str, args: &[Expr]) -> Result<Value, RuntimeError> {
        let defn = match self.definitions.get(name) {
            Some(d) => d.clone(),
            None => return Err(RuntimeError::UndefinedForeignFunction(name.to_string())),
        };

        let mut local_scope = self.state.clone();
        for (i, (param_name, _)) in defn.parameters.iter().enumerate() {
            if i < args.len() {
                let arg_val = self.eval_expr(&args[i])?;
                local_scope.insert(param_name.clone(), arg_val);
            }
        }

        let old_state = std::mem::replace(&mut self.state, local_scope);

        let mut result = Value::Void;
        for stmt in &defn.body {
            match stmt {
                Statement::Term(outputs) => {
                    if let Some(Some(expr)) = outputs.first() {
                        result = self.eval_expr(expr)?;
                    }
                }
                _ => {
                    self.exec_stmt(stmt)?;
                }
            }
        }

        self.state = old_state;

        Ok(result)
    }

    pub fn run(&mut self, program: &Program) -> Result<(), RuntimeError> {
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                let value = if let Some(expr) = &decl.expr {
                    self.eval_expr(expr)?
                } else {
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
            } else if let TopLevel::Definition(defn) = item {
                self.definitions.insert(defn.name.clone(), defn.clone());
            }
        }

        let mut executed = true;
        let mut iterations = 0;
        let max_iterations = 100;

        while executed && iterations < max_iterations {
            iterations += 1;
            executed = false;
            for item in &program.items {
                if let TopLevel::Transaction(txn) = item {
                    if txn.is_reactive {
                        let pre_val = self.eval_expr(&txn.contract.pre_condition)?;
                        if pre_val == Value::Bool(true) {
                            self.prior_state = self.state.clone();

                            let mut transaction_failed = false;
                            for stmt in &txn.body {
                                if let Err(_e) = self.exec_stmt(stmt) {
                                    self.state = self.prior_state.clone();
                                    transaction_failed = true;
                                    break;
                                }
                            }

                            if !transaction_failed {
                                let post_val = self.eval_expr(&txn.contract.post_condition)?;
                                if post_val != Value::Bool(true) {
                                    self.state = self.prior_state.clone();
                                } else if self.state != self.prior_state {
                                    executed = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        if iterations >= max_iterations {
            eprintln!(
                "Warning: Reactor loop hit iteration limit ({})",
                max_iterations
            );
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
                    self.state.insert(name.clone(), value);
                } else {
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
                        if value != Value::Bool(true) {}
                    }
                }
            }
            Statement::Escape(_expr_opt) => {
                return Err(RuntimeError::ContractViolation(
                    "Transaction escaped".to_string(),
                ));
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                let cond_val = self.eval_expr(condition)?;
                if cond_val == Value::Bool(true) {
                    for stmt in statements {
                        self.exec_stmt(stmt)?;
                    }
                }
            }
            Statement::Unification { .. } => {}
        }
        Ok(())
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
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
                let fn_name = name.clone();

                if self.definitions.contains_key(&fn_name) {
                    return self.call_defn(&fn_name, args);
                }

                let defn_call = self.state.get(&fn_name).and_then(|v| {
                    if let Value::Defn(n) = v {
                        Some(n.clone())
                    } else {
                        None
                    }
                });

                if let Some(defn_name) = defn_call {
                    return self.call_defn(&defn_name, args);
                }

                let mut arg_values = Vec::new();
                for arg in args {
                    arg_values.push(self.eval_expr(arg)?);
                }

                if let Some(frgn_fn) = self.foreign_functions.get(&fn_name) {
                    return frgn_fn(arg_values);
                }

                Err(RuntimeError::UndefinedForeignFunction(fn_name))
            }
            Expr::ListLiteral(elements) => {
                let mut values = Vec::new();
                for elem in elements {
                    values.push(self.eval_expr(elem)?);
                }
                Ok(Value::List(values))
            }
            Expr::ListIndex(list_expr, index_expr) => {
                let list_val = self.eval_expr(list_expr)?;
                let index_val = self.eval_expr(index_expr)?;
                match (list_val, index_val) {
                    (Value::List(items), Value::Int(idx)) => {
                        if idx < 0 || idx as usize >= items.len() {
                            Err(RuntimeError::TypeMismatch(
                                "Index out of bounds".to_string(),
                            ))
                        } else {
                            Ok(items[idx as usize].clone())
                        }
                    }
                    _ => Err(RuntimeError::TypeMismatch(
                        "List indexing requires List and Int".to_string(),
                    )),
                }
            }
            Expr::ListLen(list_expr) => {
                let list_val = self.eval_expr(list_expr)?;
                match list_val {
                    Value::List(items) => Ok(Value::Int(items.len() as i64)),
                    _ => Err(RuntimeError::TypeMismatch("len requires List".to_string())),
                }
            }
            Expr::FieldAccess(obj_expr, field_name) => {
                let obj_val = self.eval_expr(obj_expr)?;
                match obj_val {
                    Value::Struct(fields) => fields.get(field_name).cloned().ok_or_else(|| {
                        RuntimeError::UndefinedVariable(format!("field '{}'", field_name))
                    }),
                    _ => Err(RuntimeError::TypeMismatch(
                        "field access requires Struct".to_string(),
                    )),
                }
            }
        }
    }
}

fn print_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        print!("{}", s);
        Ok(Value::Bool(true))
    } else {
        Err(RuntimeError::TypeMismatch(
            "print expects String".to_string(),
        ))
    }
}

fn println_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        println!("{}", s);
        Ok(Value::Bool(true))
    } else {
        Err(RuntimeError::TypeMismatch(
            "println expects String".to_string(),
        ))
    }
}

fn input_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    let mut line = String::new();
    if let Ok(_) = stdin.lock().read_line(&mut line) {
        line.pop();
        Ok(Value::String(line))
    } else {
        Ok(Value::String(String::new()))
    }
}

fn abs_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Int(n) = &args[0] {
        Ok(Value::Int(n.abs()))
    } else {
        Err(RuntimeError::TypeMismatch("abs expects Int".to_string()))
    }
}

fn sqrt_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.sqrt()))
    } else if let Value::Int(n) = &args[0] {
        Ok(Value::Float((*n as f64).sqrt()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "sqrt expects Float or Int".to_string(),
        ))
    }
}

fn pow_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(base) = &args[0] {
        if let Value::Float(exp) = &args[1] {
            Ok(Value::Float(base.powf(*exp)))
        } else {
            Err(RuntimeError::TypeMismatch("pow expects Float".to_string()))
        }
    } else {
        Err(RuntimeError::TypeMismatch("pow expects Float".to_string()))
    }
}

fn sin_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.sin()))
    } else {
        Err(RuntimeError::TypeMismatch("sin expects Float".to_string()))
    }
}

fn cos_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.cos()))
    } else {
        Err(RuntimeError::TypeMismatch("cos expects Float".to_string()))
    }
}

fn floor_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.floor()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "floor expects Float".to_string(),
        ))
    }
}

fn ceil_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.ceil()))
    } else {
        Err(RuntimeError::TypeMismatch("ceil expects Float".to_string()))
    }
}

fn round_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.round()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "round expects Float".to_string(),
        ))
    }
}

fn random_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    Ok(Value::Float((nanos as f64) / (u32::MAX as f64)))
}

fn len_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::Int(s.len() as i64))
    } else {
        Err(RuntimeError::TypeMismatch("len expects String".to_string()))
    }
}

fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(a) = &args[0] {
        if let Value::String(b) = &args[1] {
            Ok(Value::String(format!("{}{}", a, b)))
        } else {
            Err(RuntimeError::TypeMismatch(
                "concat expects String".to_string(),
            ))
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "concat expects String".to_string(),
        ))
    }
}

fn to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Int(n) => Ok(Value::String(n.to_string())),
        Value::Float(n) => Ok(Value::String(n.to_string())),
        _ => Err(RuntimeError::TypeMismatch(
            "to_string expects Int or Float".to_string(),
        )),
    }
}

fn to_float_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        match s.parse::<f64>() {
            Ok(n) => Ok(Value::Float(n)),
            Err(_) => Ok(Value::Float(0.0)),
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "to_float expects String".to_string(),
        ))
    }
}

fn to_int_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        match s.parse::<i64>() {
            Ok(n) => Ok(Value::Int(n)),
            Err(_) => Ok(Value::Int(0)),
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "to_int expects String".to_string(),
        ))
    }
}

fn trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.trim().to_string()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "trim expects String".to_string(),
        ))
    }
}

fn contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(haystack) = &args[0] {
        if let Value::String(needle) = &args[1] {
            Ok(Value::Bool(haystack.contains(needle)))
        } else {
            Err(RuntimeError::TypeMismatch(
                "contains expects String".to_string(),
            ))
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "contains expects String".to_string(),
        ))
    }
}
