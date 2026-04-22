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
use crate::ffi::orchestrator::Orchestrator;
use crate::ffi::FFI_REGISTRY;
use serde_json::Value as JsonValue;
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
    Instance {
        typename: String,
        fields: HashMap<String, Value>,
    },
    Enum(String, String, HashMap<String, Value>), // (enum_name, variant_name, fields)
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
            Value::Instance { typename, fields } => {
                write!(f, "<{} {{}}>", typename)
            }
            Value::Enum(name, variant, _) => {
                write!(f, "<{}::{}>", name, variant)
            }
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

// Helper functions for JSON serialization stdlib
fn value_to_json_value(v: &Value) -> JsonValue {
    match v {
        Value::Int(i) => JsonValue::Number((*i).into()),
        Value::Float(f) => serde_json::json!(*f),
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::String(s) => JsonValue::String(s.clone()),
        Value::List(items) => JsonValue::Array(items.iter().map(value_to_json_value).collect()),
        Value::Instance { fields, .. } => {
            let map: serde_json::Map<String, JsonValue> = fields
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json_value(v)))
                .collect();
            JsonValue::Object(map)
        }
        Value::Enum(name, variant, fields) => {
            let mut map = serde_json::Map::new();
            map.insert("_enum".to_string(), JsonValue::String(name.clone()));
            map.insert("_variant".to_string(), JsonValue::String(variant.clone()));
            for (k, v) in fields {
                map.insert(k.clone(), value_to_json_value(v));
            }
            JsonValue::Object(map)
        }
        Value::Data(_) => JsonValue::Null,
        Value::Defn(_) => JsonValue::Null,
        Value::Void => JsonValue::Null,
    }
}

fn json_value_to_value(v: JsonValue) -> Value {
    match v {
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Int(0)
            }
        }
        JsonValue::String(s) => Value::String(s),
        JsonValue::Bool(b) => Value::Bool(b),
        JsonValue::Array(arr) => Value::List(arr.into_iter().map(json_value_to_value).collect()),
        JsonValue::Object(map) => {
            let fields: HashMap<String, Value> = map
                .into_iter()
                .map(|(k, v)| (k, json_value_to_value(v)))
                .collect();
            Value::Instance {
                typename: "Object".to_string(),
                fields,
            }
        }
        JsonValue::Null => Value::Void,
    }
}

pub type ForeignFn = fn(Vec<Value>) -> Result<Value, RuntimeError>;

pub struct Interpreter {
    pub state: HashMap<String, Value>,
    pub prior_state: HashMap<String, Value>,
    pub foreign_functions: HashMap<String, ForeignFn>,
    pub definitions: HashMap<String, Definition>,
    pub ffi_bindings: HashMap<String, ForeignSignature>,
    pub ffi_name_to_location: HashMap<String, String>,
    pub orchestrator: Orchestrator,
}

impl Interpreter {
    pub fn new() -> Self {
        let foreign_functions = Self::load_ffi_functions();
        Self {
            state: HashMap::new(),
            prior_state: HashMap::new(),
            foreign_functions,
            definitions: HashMap::new(),
            ffi_bindings: HashMap::new(),
            ffi_name_to_location: HashMap::new(),
            orchestrator: Orchestrator::new(),
        }
    }

    pub fn load_program(&mut self, program: &Program) {
        self.ffi_bindings.clear();
        self.ffi_name_to_location.clear();

        for item in &program.items {
            if let TopLevel::ForeignBinding {
                name,
                signature,
                toml_path,
                ..
            } = item
            {
                self.ffi_bindings.insert(name.clone(), signature.clone());

                let location = if !signature.location.is_empty() {
                    signature.location.clone()
                } else {
                    Self::lookup_location_from_toml(&name, toml_path)
                        .unwrap_or_else(|_| signature.location.clone())
                };
                self.ffi_name_to_location.insert(name.clone(), location);
            }
        }

        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                self.definitions.insert(defn.name.clone(), defn.clone());
            }
        }

        eprintln!(
            "[DEBUG] Loaded {} FFI bindings, {} definitions",
            self.ffi_bindings.len(),
            self.definitions.len()
        );
    }

    fn lookup_location_from_toml(name: &str, toml_path: &str) -> Result<String, String> {
        use crate::ffi::loader;
        use std::path::Path;

        let path = Path::new(toml_path);
        let bindings =
            loader::load_binding(path).map_err(|e| format!("Failed to load TOML: {}", e))?;

        for binding in bindings {
            if binding.name == name {
                return Ok(binding.location);
            }
        }

        Err(format!("Binding '{}' not found in '{}'", name, toml_path))
    }

    fn load_ffi_functions() -> HashMap<String, ForeignFn> {
        let mut functions = HashMap::new();
        let registry = &*FFI_REGISTRY;

        for (location, func) in registry.iter() {
            functions.insert(location.clone(), *func);
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

    pub fn exec_stmt(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        match stmt {
            Statement::Assignment {
                lhs,
                expr,
                timeout: _,
            } => {
                let value = self.eval_expr(expr)?;
                match lhs {
                    Expr::Identifier(name) | Expr::OwnedRef(name) => {
                        self.state.insert(name.clone(), value);
                    }
                    Expr::ListIndex(list_expr, index_expr) => {
                        let list_name = match &**list_expr {
                            Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                            _ => {
                                return Err(RuntimeError::TypeMismatch(
                                    "Expected identifier".to_string(),
                                ))
                            }
                        };
                        let idx_val = self.eval_expr(index_expr)?;
                        if let Value::Int(idx) = idx_val {
                            if let Some(target) = self.state.get_mut(&list_name) {
                                if let Value::List(items) = target {
                                    if idx >= 0 && (idx as usize) < items.len() {
                                        items[idx as usize] = value;
                                    } else {
                                        return Err(RuntimeError::TypeMismatch(
                                            "Index out of bounds".to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    _ => return Err(RuntimeError::TypeMismatch("Invalid LHS".to_string())),
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

    fn handle_ffi_result(&self, fn_name: &str, mut result: Value) -> Result<Value, RuntimeError> {
        let sig = match self.ffi_bindings.get(fn_name) {
            Some(s) => s,
            None => return Ok(result),
        };

        let success_output = &sig.success_output;
        let error_fields = &sig.error_fields;
        let error_type_name = &sig.error_type_name;
        let ffi_kind = sig.ffi_kind.unwrap_or(FfiKind::Frgn);

        match (success_output.is_empty(), ffi_kind) {
            (true, _) | (false, FfiKind::FrgnBang) | (false, FfiKind::SyscallBang) => {
                // Void paths: frgn! and syscall! (always return void)
                if !success_output.is_empty() {
                    return Ok(result);
                }
                if let Value::Instance { fields, .. } = &result {
                    for (field_name, _) in error_fields {
                        if let Some(val) = fields.get(field_name) {
                            if !Self::is_empty_value(val) {
                                return Err(RuntimeError::ContractViolation(format!(
                                    "FFI Error: {}",
                                    error_type_name
                                )));
                            }
                        }
                    }
                }
                Ok(Value::Void)
            }
            (false, FfiKind::Frgn) | (false, FfiKind::Syscall) => {
                // Result paths: frgn and syscall
                if let Value::Instance {
                    typename,
                    mut fields,
                } = result
                {
                    let mut err_fields_map = HashMap::new();
                    let mut has_error = false;

                    for (field_name, _) in error_fields {
                        if let Some(val) = fields.get(field_name) {
                            if !Self::is_empty_value(val) {
                                err_fields_map.insert(field_name.clone(), val.clone());
                                has_error = true;
                            }
                        }
                    }

                    if has_error {
                        return Err(RuntimeError::ContractViolation(format!(
                            "FFI Error({}): {:?}",
                            error_type_name, err_fields_map
                        )));
                    }

                    if let Some((first_field, _)) = success_output.first() {
                        if let Some(value) = fields.remove(first_field) {
                            return Ok(value);
                        }
                    }

                    Ok(Value::Instance {
                        typename: "Success".to_string(),
                        fields,
                    })
                } else {
                    Ok(result)
                }
            }
        }
    }

    fn is_empty_value(value: &Value) -> bool {
        match value {
            Value::Int(0) => true,
            Value::Float(0.0) => true,
            Value::String(s) => s.is_empty(),
            Value::Bool(false) => true,
            Value::List(l) => l.is_empty(),
            Value::Instance {
                typename: _,
                fields,
            } => fields.is_empty(),
            Value::Void => true,
            Value::Data(d) => d.is_empty(),
            _ => false,
        }
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
            Expr::BitAnd(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l & r)),
                    _ => Err(RuntimeError::TypeMismatch("Bitwise AND".to_string())),
                }
            }
            Expr::BitOr(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l | r)),
                    _ => Err(RuntimeError::TypeMismatch("Bitwise OR".to_string())),
                }
            }
            Expr::BitXor(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l ^ r)),
                    _ => Err(RuntimeError::TypeMismatch("Bitwise XOR".to_string())),
                }
            }
            Expr::Shl(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l << r)),
                    _ => Err(RuntimeError::TypeMismatch("Shift left".to_string())),
                }
            }
            Expr::Shr(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l >> r)),
                    _ => Err(RuntimeError::TypeMismatch("Shift right".to_string())),
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

                if fn_name == "clone" && !arg_values.is_empty() {
                    return Ok(arg_values[0].clone());
                }

                // Built-in stdlib functions: to_json and from_json
                if fn_name == "to_json" {
                    let arg = arg_values.first().ok_or_else(|| {
                        RuntimeError::TypeMismatch("to_json requires 1 argument".into())
                    })?;

                    let json_str = match arg {
                        Value::Instance { fields, .. } => {
                            let mut map = serde_json::Map::new();
                            for (k, v) in fields {
                                map.insert(k.clone(), value_to_json_value(v));
                            }
                            JsonValue::Object(map).to_string()
                        }
                        Value::String(s) => s.clone(),
                        Value::List(items) => {
                            let arr: Vec<_> = items.iter().map(value_to_json_value).collect();
                            JsonValue::Array(arr).to_string()
                        }
                        Value::Int(i) => JsonValue::Number((*i).into()).to_string(),
                        Value::Float(f) => serde_json::json!(*f).to_string(),
                        Value::Bool(b) => JsonValue::Bool(*b).to_string(),
                        _ => "{}".to_string(),
                    };
                    return Ok(Value::String(json_str));
                }

                if fn_name == "from_json" {
                    let arg = arg_values.first().ok_or_else(|| {
                        RuntimeError::TypeMismatch("from_json requires 1 argument".into())
                    })?;

                    let json_str = match arg {
                        Value::String(s) => s.clone(),
                        _ => {
                            // Return Err("Invalid JSON string")
                            return Ok(Value::Enum(
                                "Result".to_string(),
                                "Err".to_string(),
                                HashMap::from([(
                                    "error".to_string(),
                                    Value::String("Invalid JSON string".to_string()),
                                )]),
                            ));
                        }
                    };

                    let result = match serde_json::from_str::<JsonValue>(&json_str) {
                        Ok(v) => {
                            let parsed_value = json_value_to_value(v);
                            // Return Ok(parsed_value)
                            Ok(Value::Enum(
                                "Result".to_string(),
                                "Ok".to_string(),
                                HashMap::from([("value".to_string(), parsed_value)]),
                            ))
                        }
                        Err(e) => {
                            // Return Err(error_message)
                            Ok(Value::Enum(
                                "Result".to_string(),
                                "Err".to_string(),
                                HashMap::from([(
                                    "error".to_string(),
                                    Value::String(e.to_string()),
                                )]),
                            ))
                        }
                    };
                    return result;
                }

                if let Some(first_arg) = arg_values.first() {
                    if let Value::Instance { typename, fields } = first_arg {
                        let method_name = format!("{}.{}", typename, fn_name);
                        if self.definitions.contains_key(&method_name) {
                            let mut method_args = args[1..].to_vec();
                            method_args.insert(0, args[0].clone());
                            return self.call_defn(&method_name, &method_args);
                        }
                    }
                }

                if let Some(location) = self.ffi_name_to_location.get(&fn_name) {
                    if let Some(frgn_fn) = self.foreign_functions.get(location) {
                        if let Some(sig) = self.ffi_bindings.get(&fn_name) {
                            // Only use orchestrator if layouts are defined (v2)
                            if sig.input_layout.is_some() || sig.output_layout.is_some() {
                                let binding = ForeignBinding::from_signature(sig);
                                return self.orchestrator.call(&binding, arg_values, *frgn_fn);
                            }
                        }
                        let result = frgn_fn(arg_values)?;
                        return self.handle_ffi_result(&fn_name, result);
                    }
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
                    Value::Instance {
                        typename: _,
                        fields,
                    } => fields.get(field_name).cloned().ok_or_else(|| {
                        RuntimeError::UndefinedVariable(format!("field '{}'", field_name))
                    }),
                    _ => Err(RuntimeError::TypeMismatch(
                        "field access requires Instance".to_string(),
                    )),
                }
            }
            Expr::StructInstance(typename, fields) => {
                let mut instance_fields = HashMap::new();
                for (field_name, field_expr) in fields {
                    instance_fields.insert(field_name.clone(), self.eval_expr(field_expr)?);
                }
                Ok(Value::Instance {
                    typename: typename.clone(),
                    fields: instance_fields,
                })
            }
            Expr::ObjectLiteral(fields) => {
                let mut instance_fields = HashMap::new();
                for (field_name, field_expr) in fields {
                    instance_fields.insert(field_name.clone(), self.eval_expr(field_expr)?);
                }
                Ok(Value::Instance {
                    typename: String::from("ObjectLiteral"),
                    fields: instance_fields,
                })
            }
            Expr::PatternMatch {
                value,
                variant,
                fields,
            } => {
                // Evaluate the value being matched
                let matched_value = self.eval_expr(value)?;

                // Check if it's an Enum with the matching variant
                match matched_value {
                    Value::Enum(enum_name, matched_variant, enum_fields) => {
                        // Check if the variant matches
                        if matched_variant == *variant {
                            // Bind the pattern variables to the enum fields
                            // For a pattern like Ok(h), we bind the first field to h
                            // For a pattern like Ok(a, b), we bind first two fields to a and b
                            let enum_field_values: Vec<_> = enum_fields.values().cloned().collect();

                            for (i, pattern_var) in fields.iter().enumerate() {
                                if let Some(field_value) = enum_field_values.get(i) {
                                    self.state.insert(pattern_var.clone(), field_value.clone());
                                }
                            }

                            // Return true - the pattern matched
                            Ok(Value::Bool(true))
                        } else {
                            // Variant doesn't match
                            Ok(Value::Bool(false))
                        }
                    }
                    _ => Ok(Value::Bool(false)),
                }
            }
            Expr::Slice { .. } | Expr::ForAll { .. } | Expr::Exists { .. } => {
                Err(RuntimeError::TypeMismatch(
                    "Slice/quantifier expressions not supported in interpreter".to_string(),
                ))
            }
        }
    }
}

pub(crate) fn print_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        print!("{}", s);
        Ok(Value::Bool(true))
    } else {
        Err(RuntimeError::TypeMismatch(
            "print expects String".to_string(),
        ))
    }
}

pub(crate) fn println_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        println!("{}", s);
        Ok(Value::Bool(true))
    } else {
        Err(RuntimeError::TypeMismatch(
            "println expects String".to_string(),
        ))
    }
}

pub(crate) fn input_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
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

pub(crate) fn abs_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Int(n) = &args[0] {
        Ok(Value::Int(n.abs()))
    } else {
        Err(RuntimeError::TypeMismatch("abs expects Int".to_string()))
    }
}

pub(crate) fn sqrt_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
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

pub(crate) fn pow_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
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

pub(crate) fn sin_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.sin()))
    } else {
        Err(RuntimeError::TypeMismatch("sin expects Float".to_string()))
    }
}

pub(crate) fn cos_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.cos()))
    } else {
        Err(RuntimeError::TypeMismatch("cos expects Float".to_string()))
    }
}

pub(crate) fn floor_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.floor()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "floor expects Float".to_string(),
        ))
    }
}

pub(crate) fn ceil_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.ceil()))
    } else {
        Err(RuntimeError::TypeMismatch("ceil expects Float".to_string()))
    }
}

pub(crate) fn round_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Float(n) = &args[0] {
        Ok(Value::Float(n.round()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "round expects Float".to_string(),
        ))
    }
}

pub(crate) fn random_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    Ok(Value::Float((nanos as f64) / (u32::MAX as f64)))
}

pub(crate) fn len_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::Int(s.len() as i64))
    } else {
        Err(RuntimeError::TypeMismatch("len expects String".to_string()))
    }
}

pub(crate) fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
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

pub(crate) fn to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::Int(n) => Ok(Value::String(n.to_string())),
        Value::Float(n) => Ok(Value::String(n.to_string())),
        _ => Err(RuntimeError::TypeMismatch(
            "to_string expects Int or Float".to_string(),
        )),
    }
}

pub(crate) fn to_float_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
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

pub(crate) fn to_int_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
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

pub(crate) fn trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.trim().to_string()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "trim expects String".to_string(),
        ))
    }
}

pub(crate) fn contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
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

pub(crate) fn to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.to_lowercase()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "to_lowercase expects String".to_string(),
        ))
    }
}

pub(crate) fn to_upper_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.to_uppercase()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "to_uppercase expects String".to_string(),
        ))
    }
}

pub(crate) fn replace_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        if let Value::String(from) = &args[1] {
            if let Value::String(to) = &args[2] {
                Ok(Value::String(s.replace(from, to)))
            } else {
                Err(RuntimeError::TypeMismatch(
                    "replace expects String".to_string(),
                ))
            }
        } else {
            Err(RuntimeError::TypeMismatch(
                "replace expects String".to_string(),
            ))
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "replace expects String".to_string(),
        ))
    }
}

pub(crate) fn chars_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.chars().take(1).collect()))
    } else {
        Err(RuntimeError::TypeMismatch(
            "chars expects String".to_string(),
        ))
    }
}

pub(crate) fn starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        if let Value::String(prefix) = &args[1] {
            Ok(Value::Bool(s.starts_with(prefix)))
        } else {
            Err(RuntimeError::TypeMismatch(
                "starts_with expects String".to_string(),
            ))
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "starts_with expects String".to_string(),
        ))
    }
}

pub(crate) fn ends_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        if let Value::String(suffix) = &args[1] {
            Ok(Value::Bool(s.ends_with(suffix)))
        } else {
            Err(RuntimeError::TypeMismatch(
                "ends_with expects String".to_string(),
            ))
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "ends_with expects String".to_string(),
        ))
    }
}

pub(crate) fn from_str_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        match s.parse::<i64>() {
            Ok(n) => Ok(Value::Int(n)),
            Err(_) => Ok(Value::Int(0)),
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "from_str expects String".to_string(),
        ))
    }
}

pub(crate) fn now_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => Ok(Value::Int(d.as_millis() as i64)),
        Err(_) => Ok(Value::Int(0)),
    }
}

pub(crate) fn read_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(path) = &args[0] {
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(Value::String(content)),
            Err(e) => Ok(Value::String(format!("Error: {}", e))),
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "read_file expects String".to_string(),
        ))
    }
}

pub(crate) fn write_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(path) = &args[0] {
        if let Value::String(content) = &args[1] {
            match std::fs::write(path, content) {
                Ok(_) => Ok(Value::String("OK".to_string())),
                Err(e) => Ok(Value::String(format!("Error: {}", e))),
            }
        } else {
            Err(RuntimeError::TypeMismatch(
                "write_file expects String".to_string(),
            ))
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "write_file expects String".to_string(),
        ))
    }
}

pub(crate) fn delete_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(path) = &args[0] {
        match std::fs::remove_file(path) {
            Ok(_) => Ok(Value::String("OK".to_string())),
            Err(e) => Ok(Value::String(format!("Error: {}", e))),
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "delete_file expects String".to_string(),
        ))
    }
}

pub(crate) fn create_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(path) = &args[0] {
        match std::fs::create_dir(path) {
            Ok(_) => Ok(Value::String("OK".to_string())),
            Err(e) => Ok(Value::String(format!("Error: {}", e))),
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "create_dir expects String".to_string(),
        ))
    }
}

pub(crate) fn delete_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(path) = &args[0] {
        match std::fs::remove_dir(path) {
            Ok(_) => Ok(Value::String("OK".to_string())),
            Err(e) => Ok(Value::String(format!("Error: {}", e))),
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "delete_dir expects String".to_string(),
        ))
    }
}
