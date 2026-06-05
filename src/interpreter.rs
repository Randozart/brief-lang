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
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Data(Vec<u8>),
    List(Vec<Value>),
    HashMap(HashMap<String, Value>),  // HashMap (string keys for simplicity)
    HashSet(HashSet<String>),  // HashSet (string values for simplicity)
    StringBuilder(String),  // StringBuilder (internal buffer as String)
    Stack(Vec<Value>),  // Stack<T>
    Queue(VecDeque<Value>),  // Queue<T> (VecDeque for efficient pop_front)
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
            Value::Char(v) => write!(f, "'{}'", v),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Data(_) => write!(f, "<data>"),
            Value::List(items) => write!(f, "[{}]", items.len()),
            Value::HashMap(map) => write!(f, "<HashMap {}>", map.len()),
            Value::HashSet(set) => write!(f, "<HashSet {}>", set.len()),
            Value::StringBuilder(s) => write!(f, "<StringBuilder {}>", s.len()),
            Value::Stack(stack) => write!(f, "<Stack {}>", stack.len()),
            Value::Queue(queue) => write!(f, "<Queue {}>", queue.len()),
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
    /// Transaction escaped - this is not an error, but a valid cancellation path
    Escaped,
}

// Helper functions for JSON serialization stdlib
pub(crate) fn value_to_json_value(v: &Value) -> JsonValue {
    match v {
        Value::Int(i) => JsonValue::Number((*i).into()),
        Value::Float(f) => serde_json::json!(*f),
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::String(s) => JsonValue::String(s.clone()),
        Value::Char(c) => JsonValue::String(c.to_string()),
        Value::List(items) => JsonValue::Array(items.iter().map(value_to_json_value).collect()),
        Value::HashMap(map) => {
            let json_map: serde_json::Map<String, JsonValue> = map
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json_value(v)))
                .collect();
            JsonValue::Object(json_map)
        }
        Value::HashSet(set) => {
            let arr: Vec<JsonValue> = set.iter()
                .map(|s| JsonValue::String(s.clone()))
                .collect();
            JsonValue::Array(arr)
        }
        Value::StringBuilder(s) => JsonValue::String(s.clone()),
        Value::Stack(stack) => JsonValue::Array(stack.iter().map(value_to_json_value).collect()),
        Value::Queue(queue) => JsonValue::Array(queue.iter().map(value_to_json_value).collect()),
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

pub(crate) fn json_value_to_value(v: JsonValue) -> Value {
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
    pub metropolitan_hub: crate::ffi::metropolitan::MetropolitanHub,
    pub return_value: Option<Value>,
    pub frgn_registry: crate::ffi::dynamic::FrgnRegistry,
    pub profile_mode: bool,
    pub branch_counts: HashMap<String, (u64, u64)>,
    guard_counter: usize,
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
            metropolitan_hub: crate::ffi::metropolitan::MetropolitanHub::new(),
            return_value: None,
            frgn_registry: crate::ffi::dynamic::FrgnRegistry::new(),
            profile_mode: false,
            branch_counts: HashMap::new(),
            guard_counter: 0,
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
                    // Check if this is a profile-based location that needs registry fallback
                    let loc = signature.location.clone();
                    if loc.starts_with("<profile:") {
                        // Try the DBVS registry's name→location map first
                        match crate::ffi::registry::FFI_REGISTRY.get_location_by_name(&name) {
                            Some(actual_loc) => actual_loc.to_string(),
                            None => loc,
                        }
                    } else {
                        loc
                    }
                } else {
                    Self::lookup_location_from_toml(&name, toml_path)
                        .unwrap_or_else(|_| signature.location.clone())
                };
                self.ffi_name_to_location.insert(name.clone(), location);
            }

            // Register frgn declarations that use `from "lib.so"` (dynamic linking)
            if let TopLevel::ForeignBinding {
                name,
                signature,
                ..
            } = item
            {
                let loc = &signature.location;
                let is_dynamic = !loc.is_empty()
                    && (loc.contains(".so") || loc.starts_with('/') || loc == "libc.so.6"
                        || loc.ends_with(".dylib") || loc.ends_with(".dll"));
                if is_dynamic {
                    use crate::ffi::dynamic::{FrgnDecl, FrgnType};
                    let params: Vec<(String, FrgnType)> = signature.inputs.iter()
                        .filter_map(|(n, t)| {
                            let type_name = match t {
                                crate::ast::Type::Int => "Int",
                                crate::ast::Type::Float => "Float",
                                crate::ast::Type::Bool => "Bool",
                                crate::ast::Type::Char => "Char",
                                crate::ast::Type::String => "String",
                                crate::ast::Type::Void => "Void",
                                _ => return None,
                            };
                            FrgnType::from_name(type_name)
                                .map(|ft| (n.clone(), ft))
                        })
                        .collect();
                    let ret = if let Some((_, t)) = signature.success_output.first() {
                            match t {
                                crate::ast::Type::Int => FrgnType::Int,
                                crate::ast::Type::Float => FrgnType::Float,
                                crate::ast::Type::Bool => FrgnType::Bool,
                                crate::ast::Type::Char => FrgnType::Char,
                                crate::ast::Type::String => FrgnType::String,
                                crate::ast::Type::Void => FrgnType::Void,
                                _ => FrgnType::Int,
                            }
                        } else {
                            FrgnType::Void
                        };
                    let decl = FrgnDecl {
                        name: name.clone(),
                        params,
                        ret,
                        lib: loc.clone(),
                    };
                    self.frgn_registry.register(decl);
                }
            }
        }

        for item in &program.items {
            if let TopLevel::Definition(defn) = item {
                self.definitions.insert(defn.name.clone(), defn.clone());
            } else if let TopLevel::Enum(enum_def) = item {
                for variant in &enum_def.variants {
                    let variant_name = match variant {
                        EnumVariant::Unit(name) => name.clone(),
                        EnumVariant::Tuple(name, _) => name.clone(),
                        EnumVariant::Struct(name, _) => name.clone(),
                    };
                    self.state.insert(variant_name.clone(), Value::Enum(
                        enum_def.name.clone(),
                        variant_name,
                        std::collections::HashMap::new(),
                    ));
                }
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
        let old_return = self.return_value.take();

        let mut result = Value::Void;
        for stmt in &defn.body {
            match stmt {
                Statement::Term { values: outputs, .. } => {
                    if let Some(Some(expr)) = outputs.first() {
                        result = self.eval_expr(expr)?;
                        self.return_value = Some(result.clone());
                    }
                }
                _ => {
                    self.exec_stmt(stmt)?;
                }
            }
            if self.return_value.is_some() {
                result = self.return_value.take().unwrap();
                break;
            }
        }

        self.state = old_state;
        self.return_value = old_return;
        Ok(result)
    }

    /// Extract (root_variable_name, field_path) from an arrow mutation target.
    /// Supports: `&name`, `&name[i]`, `&name.field`, `&name.field[i]`
    fn extract_arrow_root(&self, target: &Expr) -> Result<(String, Vec<String>), RuntimeError> {
        match target {
            Expr::OwnedRef(n) => Ok((n.clone(), vec![])),
            Expr::FieldAccess(inner, field) => {
                let (root, mut path) = self.extract_arrow_root(inner)?;
                path.push(field.clone());
                Ok((root, path))
            }
            Expr::ListIndex(inner, _) => {
                // For indexed targets, the inner expr determines the root
                self.extract_arrow_root(inner)
            }
            _ => Err(RuntimeError::TypeMismatch(
                "Arrow target must be &name, &name.field, or &name[...]".to_string()
            )),
        }
    }

    /// Resolve the list Value from state using root name and optional field path.
    fn resolve_arrow_list(&self, root: &str, field_path: &[String]) -> Result<Vec<Value>, RuntimeError> {
        let mut val = self.state.get(root)
            .ok_or_else(|| RuntimeError::UndefinedVariable(root.to_string()))?
            .clone();
        for field in field_path {
            val = match val {
                Value::Instance { typename: _, ref fields } => {
                    fields.get(field)
                        .ok_or_else(|| RuntimeError::TypeMismatch(
                            format!("Instance has no field '{}'", field)
                        ))?
                        .clone()
                }
                _ => return Err(RuntimeError::TypeMismatch(
                    format!("Cannot access field '{}' on non-instance value", field)
                )),
            };
        }
        match val {
            Value::List(l) => Ok(l),
            _ => Err(RuntimeError::TypeMismatch(
                "Arrow target must resolve to a List".to_string()
            )),
        }
    }

    /// Store a list value back into state through the root and field path.
    fn store_arrow_list(&mut self, root: &str, field_path: &[String], list_val: Value) {
        if field_path.is_empty() {
            self.state.insert(root.to_string(), list_val);
        } else {
            // Walk the field path to reconstruct nested instances
            let mut val = self.state.get(root)
                .expect("Root must exist")
                .clone();
            let mut stack: Vec<(String, Value)> = Vec::new();
            for field in field_path.iter().rev() {
                stack.push((field.clone(), val.clone()));
                val = match &val {
                    Value::Instance { fields, .. } => {
                        fields.get(field)
                            .expect("Field must exist")
                            .clone()
                    }
                    _ => return,
                };
            }
            // Now `val` is the innermost value (the old list). Replace with new one.
            let mut current = list_val;
            for (field, parent_val) in stack {
                let (typename, mut fields) = match parent_val {
                    Value::Instance { typename, fields } => {
                        (typename.clone(), fields.clone())
                    }
                    _ => return,
                };
                fields.insert(field, current);
                current = Value::Instance { typename, fields };
            }
            self.state.insert(root.to_string(), current);
        }
    }

    /// Evaluate the index expression for an arrow operation.
    /// Returns None for Term (end operations) or Some(position) for specific index.
    fn eval_arrow_pos(&mut self, list: &[Value], index: &Expr) -> Result<Option<usize>, RuntimeError> {
        match index {
            Expr::Term => Ok(None),
            idx_expr => {
                let idx_val = self.eval_expr(idx_expr)?;
                match idx_val {
                    Value::Int(i) => {
                        let p = if i < 0 { (list.len() as i64 + i).max(0) as usize } else { i as usize };
                        Ok(Some(p))
                    }
                    _ => Err(RuntimeError::TypeMismatch(
                        "Arrow index must be an integer".to_string()
                    )),
                }
            }
        }
    }

    fn list_nesting_depth(value: &Value) -> usize {
        match value {
            Value::List(items) => match items.first() {
                Some(inner) => 1 + Self::list_nesting_depth(inner),
                None => 1,
            },
            _ => 0,
        }
    }

    fn expand_coordinates(
        coords: &[SliceCoordinate],
        total_dims: usize,
    ) -> Result<Vec<SliceCoordinate>, RuntimeError> {
        let ellipsis_count = coords.iter().filter(|c| matches!(c, SliceCoordinate::Ellipsis)).count();
        if ellipsis_count > 1 {
            return Err(RuntimeError::TypeMismatch(
                "Multiple ellipsis (...) in a single slice is ambiguous".to_string()
            ));
        }
        let explicit_count = coords.len() - ellipsis_count;
        if explicit_count > total_dims {
            return Err(RuntimeError::TypeMismatch(format!(
                "Too many slice coordinates: {} dimensions but {} coordinates",
                total_dims, explicit_count
            )));
        }
        let fill_count = total_dims - explicit_count;
        let wildcard = SliceCoordinate::Range { start: None, end: None };
        let mut expanded = Vec::with_capacity(total_dims);
        for c in coords {
            match c {
                SliceCoordinate::Ellipsis => {
                    for _ in 0..fill_count {
                        expanded.push(wildcard.clone());
                    }
                }
                other => expanded.push(other.clone()),
            }
        }
        Ok(expanded)
    }

    fn apply_multi_slice_coords(
        &mut self,
        value: &Value,
        coords: &[SliceCoordinate],
    ) -> Result<Value, RuntimeError> {
        if coords.is_empty() {
            return Ok(value.clone());
        }
        let first = &coords[0];
        let rest = &coords[1..];
        match first {
            SliceCoordinate::Index(idx_expr) => {
                let list = match value {
                    Value::List(items) => items,
                    _ => return Err(RuntimeError::TypeMismatch("Cannot index non-list in multi-slice".to_string())),
                };
                let idx_val = self.eval_expr(idx_expr)?;
                let n = match idx_val {
                    Value::Int(i) => if i < 0 { (list.len() as i64 + i).max(0) as usize } else { i as usize },
                    _ => return Err(RuntimeError::TypeMismatch("Index must be integer".to_string())),
                };
                if n >= list.len() {
                    return Err(RuntimeError::TypeMismatch("Index out of bounds".to_string()));
                }
                let extracted = list[n].clone();
                if rest.is_empty() { Ok(extracted) } else { self.apply_multi_slice_coords(&extracted, rest) }
            }
            SliceCoordinate::Range { start, end } => {
                let list = match value {
                    Value::List(items) => items,
                    _ => return Err(RuntimeError::TypeMismatch("Cannot slice non-list in multi-slice".to_string())),
                };
                let len = list.len();
                let start_idx = match start {
                    Some(s) => {
                        let sv = self.eval_expr(s)?;
                        match sv {
                            Value::Int(i) => if i < 0 { (len as i64 + i).max(0) as usize } else { i as usize },
                            _ => return Err(RuntimeError::TypeMismatch("Range start must be integer".to_string())),
                        }
                    }
                    None => 0,
                };
                let end_idx = match end {
                    Some(e) => {
                        let ev = self.eval_expr(e)?;
                        match ev {
                            Value::Int(i) => if i < 0 { (len as i64 + i).max(0) as usize } else { i as usize },
                            _ => return Err(RuntimeError::TypeMismatch("Range end must be integer".to_string())),
                        }
                    }
                    None => len,
                };
                let lo = start_idx.min(len);
                let hi = end_idx.min(len);
                let sublist: Vec<Value> = if lo < hi { list[lo..hi].to_vec() } else { vec![] };
                if rest.is_empty() {
                    Ok(Value::List(sublist))
                } else {
                    let results: Result<Vec<Value>, RuntimeError> = sublist.iter()
                        .map(|item| self.apply_multi_slice_coords(item, rest))
                        .collect();
                    Ok(Value::List(results?))
                }
            }
            SliceCoordinate::Named { coord, .. } => self.apply_multi_slice_coords(value, &[coord.as_ref().clone()]),
            SliceCoordinate::AtDimension { coord, .. } => self.apply_multi_slice_coords(value, &[coord.as_ref().clone()]),
            SliceCoordinate::Ellipsis => Err(RuntimeError::TypeMismatch(
                "Ellipsis must be expanded before coordinate application".to_string()
            )),
        }
    }

    fn handle_result_method(&mut self, fn_name: &str, arg_values: &[Value]) -> Result<Value, RuntimeError> {
        if arg_values.is_empty() {
            return Err(RuntimeError::TypeMismatch("Result method requires a value".to_string()));
        }
        match &arg_values[0] {
            Value::Enum(_, variant, fields) => {
                match fn_name {
                    "is_ok" => Ok(Value::Bool(variant == "Ok")),
                    "is_err" => Ok(Value::Bool(variant == "Err")),
                    "unwrap" => {
                        if variant == "Ok" {
                            Ok(fields.get("result").cloned().unwrap_or(Value::Void))
                        } else {
                            Err(RuntimeError::TypeMismatch("unwrap on Err".to_string()))
                        }
                    }
                    "unwrap_err" => {
                        if variant == "Err" {
                            Ok(fields.get("error").cloned().unwrap_or(Value::Void))
                        } else {
                            Err(RuntimeError::TypeMismatch("unwrap_err on Ok".to_string()))
                        }
                    }
                    _ => Err(RuntimeError::TypeMismatch(format!("Unknown Result method: {}", fn_name))),
                }
            }
            _ => Err(RuntimeError::TypeMismatch(format!("Result method {} requires an enum value", fn_name))),
        }
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

                            let mut transaction_escaped = false;
                            let mut transaction_failed = false;
                            for stmt in &txn.body {
                                if let Err(e) = self.exec_stmt(stmt) {
                                    match e {
                                        RuntimeError::Escaped => {
                                            // Escape cancels the transaction - valid path
                                            transaction_escaped = true;
                                        }
                                        _ => {
                                            // Actual error - restore state
                                            self.state = self.prior_state.clone();
                                            transaction_failed = true;
                                        }
                                    }
                                    break;
                                }
                            }

                            if !transaction_failed && !transaction_escaped {
                                let post_val = self.eval_expr(&txn.contract.post_condition)?;
                                if post_val != Value::Bool(true) {
                                    self.state = self.prior_state.clone();
                                } else if self.state != self.prior_state {
                                    executed = true;
                                }
                            }
                            // If escaped, state is already restored and we continue
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
                modifiers: _,
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
            Statement::InlineAsm { .. } => {}
            Statement::Expression(expr) => {
                self.eval_expr(expr)?;
            }
            Statement::Term { values: outputs, .. } => {
                if let Some(first) = outputs.first() {
                    if let Some(expr) = first {
                        let value = self.eval_expr(expr)?;
                        self.return_value = Some(value);
                    }
                }
            }
            Statement::Escape(_expr_opt) => {
                // Escape cancels the transaction - not an error, just a cancellation
                return Err(RuntimeError::Escaped);
            }
            Statement::Guarded {
                condition,
                statements,
            } => {
                let cond_val = self.eval_expr(condition)?;
                if self.profile_mode {
                    let guard_id = format!("guard_{}", self.guard_counter);
                    self.guard_counter += 1;
                    let entry = self.branch_counts.entry(guard_id).or_insert((0, 0));
                    if cond_val == Value::Bool(true) {
                        entry.0 += 1;
                    } else {
                        entry.1 += 1;
                    }
                }
                if cond_val == Value::Bool(true) {
                    for stmt in statements {
                        self.exec_stmt(stmt)?;
                    }
                }
            }
            Statement::Unification {
                name,
                pattern,
                expr,
            } => {
                // Look up value from state (set by previous let) not by evaluating expr
                let value = self.state.get(name).cloned().unwrap_or(Value::Void);
                let variant_name = pattern.split_once('(').map(|(n, _)| n).unwrap_or(pattern);
                let field_names: Vec<&str> = pattern.split_once('(')
                    .and_then(|(_, rest)| rest.strip_suffix(')'))
                    .map(|inner| inner.split(',').map(|s| s.trim()).collect())
                    .unwrap_or_default();
                let matched = match &value {
                    Value::Enum(_, variant, fields) if variant == variant_name => {
                        let mut sorted_keys: Vec<&String> = fields.keys().collect();
                        sorted_keys.sort();
                        let vals: Vec<&Value> = sorted_keys.iter()
                            .filter_map(|k| fields.get(*k)).collect();
                        for (i, fname) in field_names.iter().enumerate() {
                            if i < vals.len() && !fname.is_empty() && *fname != "_" {
                                self.state.insert(fname.to_string(), vals[i].clone());
                            }
                        }
                        true
                    }
                    _ if variant_name == pattern => {
                        if let Some(first) = field_names.first() {
                            if !first.is_empty() && *first != "_" {
                                self.state.insert(first.to_string(), value);
                            }
                        }
                        true
                    }
                    _ => false,
                };
                if matched {
                    if let Expr::Block(stmts, _) = expr {
                        for stmt in stmts { self.exec_stmt(stmt)?; }
                    } else { self.eval_expr(expr)?; }
                }
            }
            Statement::LocalTrigger { name, expr, .. } => {
                // Local trigger: await async event inside transaction
                // For now, evaluate the expression if present and store the result
                if let Some(e) = expr {
                    let value = self.eval_expr(e)?;
                    self.state.insert(name.clone(), value);
                }
                // TODO: Full async yield/await semantics with rollback support
            }
            Statement::Alka(_) | Statement::OnExit { .. } => {}
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
            Expr::Char(v) => Ok(Value::Char(*v)),  // NEW
            Expr::Bool(v) => Ok(Value::Bool(*v)),
            Expr::Term => self
                .state
                .get("term")
                .cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable("term".to_string())),
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
                match (l_val.clone(), r_val.clone()) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Int(l + r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Float(l + r)),
                    (Value::String(l), Value::String(r)) => Ok(Value::String(l + &r)),
                    (Value::String(l), Value::Int(r)) => Ok(Value::String(l + &r.to_string())),
                    (Value::Int(l), Value::String(r)) => Ok(Value::String(l.to_string() + &r)),
                    (Value::String(l), Value::Char(r)) => { let mut s = l; s.push(r); Ok(Value::String(s)) },
                    (Value::Char(l), Value::String(r)) => Ok(Value::String(l.to_string() + &r)),
                    (Value::String(l), Value::Void) => Ok(Value::String(l)),
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
            Expr::Mod(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                match (l_val, r_val) {
                    (Value::Int(l), Value::Int(r)) => {
                        if r == 0 {
                            return Err(RuntimeError::DivisionByZero);
                        }
                        Ok(Value::Int(l % r))
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
                fn ord(v: &Value) -> String {
                    match v { Value::Enum(_, n, _) => n.clone(), _ => format!("{:?}", v) }
                }
                match (&l_val, &r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l < r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l < r)),
                    (Value::Char(l), Value::Char(r)) => Ok(Value::Bool(l < r)),
                    (Value::String(l), Value::String(r)) => Ok(Value::Bool(l < r)),
                    _ => Ok(Value::Bool(ord(&l_val) < ord(&r_val))),
                }
            }
            Expr::Le(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                fn ord(v: &Value) -> String {
                    match v { Value::Enum(_, n, _) => n.clone(), _ => format!("{:?}", v) }
                }
                match (&l_val, &r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l <= r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l <= r)),
                    (Value::Char(l), Value::Char(r)) => Ok(Value::Bool(l <= r)),
                    (Value::String(l), Value::String(r)) => Ok(Value::Bool(l <= r)),
                    _ => Ok(Value::Bool(ord(&l_val) <= ord(&r_val))),
                }
            }
            Expr::Gt(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                fn ord(v: &Value) -> String {
                    match v { Value::Enum(_, n, _) => n.clone(), _ => format!("{:?}", v) }
                }
                match (&l_val, &r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l > r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l > r)),
                    (Value::Char(l), Value::Char(r)) => Ok(Value::Bool(l > r)),
                    (Value::String(l), Value::String(r)) => Ok(Value::Bool(l > r)),
                    _ => Ok(Value::Bool(ord(&l_val) > ord(&r_val))),
                }
            }
Expr::Ge(l, r) => {
                let l_val = self.eval_expr(l)?;
                let r_val = self.eval_expr(r)?;
                fn ord(v: &Value) -> String {
                    match v { Value::Enum(_, n, _) => n.clone(), _ => format!("{:?}", v) }
                }
                match (&l_val, &r_val) {
                    (Value::Int(l), Value::Int(r)) => Ok(Value::Bool(l >= r)),
                    (Value::Float(l), Value::Float(r)) => Ok(Value::Bool(l >= r)),
                    (Value::Char(l), Value::Char(r)) => Ok(Value::Bool(l >= r)),
                    (Value::String(l), Value::String(r)) => Ok(Value::Bool(l >= r)),
                    _ => Ok(Value::Bool(ord(&l_val) >= ord(&r_val))),
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
            Expr::ArrowMut { dir, target, index, value } => {
                let (root_name, field_path) = self.extract_arrow_root(target)?;
                let mut list = self.resolve_arrow_list(&root_name, &field_path)?;
                let pos = self.eval_arrow_pos(&list, index)?;
                match dir {
                    ArrowDir::Push => {
                        let val = match value {
                            Some(v) => self.eval_expr(v)?,
                            None => return Err(RuntimeError::TypeMismatch(
                                "ArrowMut Push requires a value".to_string()
                            )),
                        };
                        match pos {
                            Some(p) if p < list.len() => list.insert(p, val),
                            _ => list.push(val),
                        }
                        self.store_arrow_list(&root_name, &field_path, Value::List(list.clone()));
                        Ok(Value::List(list))
                    }
                    ArrowDir::Pop => {
                        let removed = match pos {
                            Some(p) if p < list.len() => list.remove(p),
                            _ => list.pop().ok_or_else(|| RuntimeError::TypeMismatch(
                                "Cannot pop from empty list".to_string()
                            ))?,
                        };
                        self.store_arrow_list(&root_name, &field_path, Value::List(list));
                        Ok(removed)
                    }
                }
            }
            Expr::ArrowDiscard { target, index } => {
                let (root_name, field_path) = self.extract_arrow_root(target)?;
                let mut list = self.resolve_arrow_list(&root_name, &field_path)?;
                let pos = self.eval_arrow_pos(&list, index)?;
                match pos {
                    Some(p) if p < list.len() => { list.remove(p); }
                    _ => { list.pop(); }
                }
                self.store_arrow_list(&root_name, &field_path, Value::List(list));
                Ok(Value::Void)
            }
            Expr::Ellipsis => {
                Err(RuntimeError::TypeMismatch(
                    "Ellipsis (...) must be resolved at parse time in bracket context".to_string(),
                ))
            }
            Expr::Call(name, args) => {
                let fn_name = name.clone();

                // Evaluate arguments — used by clone, to_json, from_json, len, etc.
                let mut arg_values = Vec::new();
                for arg in args.iter() {
                    arg_values.push(self.eval_expr(arg)?);
                }

                // Check built-in Result methods BEFORE user definitions, so that
                // Result::Ok → unwrap doesn't get intercepted by Option::unwrap.
                if (fn_name == "is_ok" || fn_name == "is_err" || fn_name == "unwrap" || fn_name == "unwrap_err")
                    && !arg_values.is_empty()
                    && matches!(&arg_values[0], Value::Enum(en, _, _) if en == "Result")
                {
                    return self.handle_result_method(&fn_name, &arg_values);
                }

                // Built-in len for String and List — must run before user definitions
                // to prevent infinite recursion from std/string.bv's self-referential defn len.
                if fn_name == "len" && arg_values.len() == 1 {
                    match &arg_values[0] {
                        Value::String(s) => return Ok(Value::Int(s.len() as i64)),
                        Value::List(l) => return Ok(Value::Int(l.len() as i64)),
                        _ => {}
                    }
                }

                if self.definitions.contains_key(&fn_name) {
                    return self.call_defn(&fn_name, args);
                }

                // Check dynamically linked FFI (frgn declarations with from "lib.so")
                if self.frgn_registry.declarations.contains_key(&fn_name) {
                    return self.frgn_registry.call(&fn_name, &arg_values);
                }

                // Check if this is an enum constructor call (e.g. Ok(value), Err(error))
let enum_state = self.state.get(&fn_name).cloned();
                if let Some(Value::Enum(enum_name, variant, _)) = &enum_state {
                    if !arg_values.is_empty() {
                        let mut fields = HashMap::new();
                        fields.insert("value".to_string(), arg_values[0].clone());
                        return Ok(Value::Enum(enum_name.clone(), fn_name.clone(), fields));
                    }
                    return Ok(enum_state.unwrap());
                }
                // Known enum constructors for intrinsic types
                if (fn_name == "Ok" || fn_name == "Err") && arg_values.len() == 1 {
                    let field_key = if fn_name == "Ok" { "result" } else { "error" };
                    let mut fields = HashMap::new();
                    fields.insert(field_key.to_string(), arg_values[0].clone());
                    return Ok(Value::Enum("Result".to_string(), fn_name, fields));
                }
                // Result methods: is_ok, is_err, unwrap, unwrap_err
                if (fn_name == "is_ok" || fn_name == "is_err" || fn_name == "unwrap" || fn_name == "unwrap_err")
                    && !arg_values.is_empty()
                {
                    return self.handle_result_method(&fn_name, &arg_values);
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

if fn_name == "clone" && !arg_values.is_empty() {
                    return Ok(arg_values[0].clone());
                }
                if fn_name == "char_at" && arg_values.len() == 2 {
                    if let Value::String(s) = &arg_values[0] {
                        if let Value::Int(idx) = &arg_values[1] {
                            let i = *idx as usize;
                            return Ok(s.chars().nth(i).map(Value::Char).unwrap_or(Value::Char('\0')));
                        }
                    }
                }
                if fn_name == "HashMap::new" || fn_name == "new_map" || fn_name == "empty_map" {
                    return Ok(Value::HashMap(HashMap::new()));
                }

                if fn_name == "HashSet::new" || fn_name == "new_set" {
                    return Ok(Value::HashSet(HashSet::new()));
                }

                // HashMap methods (called as map.insert(key, value))
                if arg_values.len() >= 1 {
                    if let Value::HashMap(map) = &arg_values[0] {
                        let mut map = map.clone();
                        
                        if fn_name == "insert" && arg_values.len() == 3 {
                            if let Value::String(key) = &arg_values[1] {
                                map.insert(key.clone(), arg_values[2].clone());
                                return Ok(Value::HashMap(map));
                            }
                        }
                        
                        if fn_name == "get" && arg_values.len() == 2 {
                            if let Value::String(key) = &arg_values[1] {
                                return Ok(match map.get(key) {
                                    Some(v) => Value::Enum(
                                        "Option".to_string(),
                                        "Some".to_string(),
                                        HashMap::from([("value".to_string(), v.clone())]),
                                    ),
                                    None => Value::Enum(
                                        "Option".to_string(),
                                        "None".to_string(),
                                        HashMap::new(),
                                    ),
                                });
                            }
                        }
                        
                        if fn_name == "contains_key" && arg_values.len() == 2 {
                            if let Value::String(key) = &arg_values[1] {
                                return Ok(Value::Bool(map.contains_key(key)));
                            }
                        }
                        
                        if fn_name == "remove" && arg_values.len() == 2 {
                            if let Value::String(key) = &arg_values[1] {
                                map.remove(key);
                                return Ok(Value::HashMap(map));
                            }
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(map.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(map.is_empty()));
                        }
                        
                        if fn_name == "keys" {
                            let keys: Vec<Value> = map.keys()
                                .map(|k| Value::String(k.clone()))
                                .collect();
                            return Ok(Value::List(keys));
                        }
                        
                        if fn_name == "values" {
                            let values: Vec<Value> = map.values().cloned().collect();
                            return Ok(Value::List(values));
                        }
                    }
                    
                    // HashSet methods
                    if let Value::HashSet(set) = &arg_values[0] {
                        let mut set = set.clone();
                        
                        if fn_name == "insert" && arg_values.len() == 2 {
                            if let Value::String(item) = &arg_values[1] {
                                set.insert(item.clone());
                                return Ok(Value::HashSet(set));
                            }
                        }
                        
                        if fn_name == "contains" && arg_values.len() == 2 {
                            if let Value::String(item) = &arg_values[1] {
                                return Ok(Value::Bool(set.contains(item)));
                            }
                        }
                        
                        if fn_name == "remove" && arg_values.len() == 2 {
                            if let Value::String(item) = &arg_values[1] {
                                set.remove(item);
                                return Ok(Value::HashSet(set));
                            }
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(set.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(set.is_empty()));
                        }
                    }
                }

                // HashMap built-in methods
                if fn_name == "HashMap::new" || fn_name == "new_map" {
                    return Ok(Value::HashMap(HashMap::new()));
                }

                if fn_name == "HashSet::new" || fn_name == "new_set" {
                    return Ok(Value::HashSet(HashSet::new()));
                }

                // HashMap methods (called as map.insert(key, value))
                if arg_values.len() >= 1 {
                    if let Value::HashMap(map) = &arg_values[0] {
                        let mut map = map.clone();
                        
                        if fn_name == "insert" && arg_values.len() == 3 {
                            if let Value::String(key) = &arg_values[1] {
                                map.insert(key.clone(), arg_values[2].clone());
                                return Ok(Value::HashMap(map));
                            }
                        }
                        
                        if fn_name == "get" && arg_values.len() == 2 {
                            if let Value::String(key) = &arg_values[1] {
                                return Ok(match map.get(key) {
                                    Some(v) => Value::Enum(
                                        "Option".to_string(),
                                        "Some".to_string(),
                                        HashMap::from([("value".to_string(), v.clone())]),
                                    ),
                                    None => Value::Enum(
                                        "Option".to_string(),
                                        "None".to_string(),
                                        HashMap::new(),
                                    ),
                                });
                            }
                        }
                        
                        if fn_name == "contains_key" && arg_values.len() == 2 {
                            if let Value::String(key) = &arg_values[1] {
                                return Ok(Value::Bool(map.contains_key(key)));
                            }
                        }
                        
                        if fn_name == "remove" && arg_values.len() == 2 {
                            if let Value::String(key) = &arg_values[1] {
                                map.remove(key);
                                return Ok(Value::HashMap(map));
                            }
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(map.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(map.is_empty()));
                        }
                        
                        if fn_name == "keys" {
                            let keys: Vec<Value> = map.keys()
                                .map(|k| Value::String(k.clone()))
                                .collect();
                            return Ok(Value::List(keys));
                        }
                        
                        if fn_name == "values" {
                            let values: Vec<Value> = map.values().cloned().collect();
                            return Ok(Value::List(values));
                        }
                    }
                    
                    // HashSet methods
                    if let Value::HashSet(set) = &arg_values[0] {
                        let mut set = set.clone();
                        
                        if fn_name == "insert" && arg_values.len() == 2 {
                            if let Value::String(item) = &arg_values[1] {
                                set.insert(item.clone());
                                return Ok(Value::HashSet(set));
                            }
                        }
                        
                        if fn_name == "contains" && arg_values.len() == 2 {
                            if let Value::String(item) = &arg_values[1] {
                                return Ok(Value::Bool(set.contains(item)));
                            }
                        }
                        
                        if fn_name == "remove" && arg_values.len() == 2 {
                            if let Value::String(item) = &arg_values[1] {
                                set.remove(item);
                                return Ok(Value::HashSet(set));
                            }
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(set.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(set.is_empty()));
                        }
                    }
                }

                // StringBuilder built-in methods
                if fn_name == "StringBuilder::new" || fn_name == "new_builder" {
                    return Ok(Value::StringBuilder(String::new()));
                }

                if arg_values.len() >= 1 {
                    if let Value::StringBuilder(buffer) = &arg_values[0] {
                        let mut buffer = buffer.clone();
                        
                        if fn_name == "append_char" && arg_values.len() == 2 {
                            if let Value::Char(c) = &arg_values[1] {
                                buffer.push(*c);
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "append_str" && arg_values.len() == 2 {
                            if let Value::String(s) = &arg_values[1] {
                                buffer.push_str(s);
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "append_int" && arg_values.len() == 2 {
                            if let Value::Int(n) = &arg_values[1] {
                                buffer.push_str(&n.to_string());
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "append_bool" && arg_values.len() == 2 {
                            if let Value::Bool(b) = &arg_values[1] {
                                buffer.push_str(&b.to_string());
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "append_float" && arg_values.len() == 2 {
                            if let Value::Float(f) = &arg_values[1] {
                                buffer.push_str(&f.to_string());
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "to_string" {
                            return Ok(Value::String(buffer.clone()));
                        }
                        
                        if fn_name == "clear" {
                            buffer.clear();
                            return Ok(Value::StringBuilder(buffer));
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(buffer.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(buffer.is_empty()));
                        }
                        
                        if fn_name == "capacity" {
                            // For simplicity, capacity = len (no pre-allocation yet)
                            return Ok(Value::Int(buffer.len() as i64));
                        }
                    }
                }

                // StringBuilder built-in methods
                if fn_name == "StringBuilder::new" || fn_name == "new_builder" {
                    return Ok(Value::StringBuilder(String::new()));
                }

                if arg_values.len() >= 1 {
                    if let Value::StringBuilder(buffer) = &arg_values[0] {
                        let mut buffer = buffer.clone();
                        
                        if fn_name == "append_char" && arg_values.len() == 2 {
                            if let Value::Char(c) = &arg_values[1] {
                                buffer.push(*c);
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "append_str" && arg_values.len() == 2 {
                            if let Value::String(s) = &arg_values[1] {
                                buffer.push_str(s);
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "append_int" && arg_values.len() == 2 {
                            if let Value::Int(n) = &arg_values[1] {
                                buffer.push_str(&n.to_string());
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "append_bool" && arg_values.len() == 2 {
                            if let Value::Bool(b) = &arg_values[1] {
                                buffer.push_str(&b.to_string());
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "append_float" && arg_values.len() == 2 {
                            if let Value::Float(f) = &arg_values[1] {
                                buffer.push_str(&f.to_string());
                                return Ok(Value::StringBuilder(buffer));
                            }
                        }
                        
                        if fn_name == "to_string" {
                            return Ok(Value::String(buffer.clone()));
                        }
                        
                        if fn_name == "clear" {
                            buffer.clear();
                            return Ok(Value::StringBuilder(buffer));
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(buffer.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(buffer.is_empty()));
                        }
                        
                        if fn_name == "capacity" {
                            // For simplicity, capacity = len (no pre-allocation yet)
                            return Ok(Value::Int(buffer.len() as i64));
                        }
                    }
                }

                // Stack built-in methods
                if fn_name == "Stack::new" || fn_name == "new_stack" {
                    return Ok(Value::Stack(Vec::new()));
                }

                if arg_values.len() >= 1 {
                    if let Value::Stack(stack) = &arg_values[0] {
                        let mut stack = stack.clone();
                        
                        if fn_name == "push" && arg_values.len() == 2 {
                            stack.push(arg_values[1].clone());
                            return Ok(Value::Stack(stack));
                        }
                        
                        if fn_name == "pop" && !stack.is_empty() {
                            let item = stack.pop().unwrap();
                            return Ok(Value::Enum(
                                "Option".to_string(),
                                "Some".to_string(),
                                HashMap::from([
                                    ("0".to_string(), item.clone()),
                                    ("1".to_string(), Value::Stack(stack)),
                                ]),
                            ));
                        }
                        
                        if fn_name == "peek" && !stack.is_empty() {
                            let item = stack.last().unwrap().clone();
                            return Ok(Value::Enum(
                                "Option".to_string(),
                                "Some".to_string(),
                                HashMap::from([("value".to_string(), item)]),
                            ));
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(stack.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(stack.is_empty()));
                        }
                        
                        if fn_name == "clear" {
                            stack.clear();
                            return Ok(Value::Stack(stack));
                        }
                    }
                    
                    // Queue methods
                    if let Value::Queue(queue) = &arg_values[0] {
                        let mut queue = queue.clone();
                        
                        if fn_name == "enqueue" && arg_values.len() == 2 {
                            queue.push_back(arg_values[1].clone());
                            return Ok(Value::Queue(queue));
                        }
                        
                        if fn_name == "dequeue" && !queue.is_empty() {
                            let item = queue.pop_front().unwrap();
                            return Ok(Value::Enum(
                                "Option".to_string(),
                                "Some".to_string(),
                                HashMap::from([
                                    ("0".to_string(), item.clone()),
                                    ("1".to_string(), Value::Queue(queue)),
                                ]),
                            ));
                        }
                        
                        if fn_name == "front" && !queue.is_empty() {
                            let item = queue.front().unwrap().clone();
                            return Ok(Value::Enum(
                                "Option".to_string(),
                                "Some".to_string(),
                                HashMap::from([("value".to_string(), item)]),
                            ));
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(queue.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(queue.is_empty()));
                        }
                        
                        if fn_name == "clear" {
                            queue.clear();
                            return Ok(Value::Queue(queue));
                        }
                    }
                }

                // Stack built-in methods
                if fn_name == "Stack::new" || fn_name == "new_stack" {
                    return Ok(Value::Stack(Vec::new()));
                }

                if arg_values.len() >= 1 {
                    if let Value::Stack(stack) = &arg_values[0] {
                        let mut stack = stack.clone();
                        
                        if fn_name == "push" && arg_values.len() == 2 {
                            stack.push(arg_values[1].clone());
                            return Ok(Value::Stack(stack));
                        }
                        
                        if fn_name == "pop" && !stack.is_empty() {
                            let item = stack.pop().unwrap();
                            return Ok(Value::Enum(
                                "Option".to_string(),
                                "Some".to_string(),
                                HashMap::from([
                                    ("0".to_string(), item.clone()),
                                    ("1".to_string(), Value::Stack(stack)),
                                ]),
                            ));
                        }
                        
                        if fn_name == "peek" && !stack.is_empty() {
                            let item = stack.last().unwrap().clone();
                            return Ok(Value::Enum(
                                "Option".to_string(),
                                "Some".to_string(),
                                HashMap::from([("value".to_string(), item)]),
                            ));
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(stack.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(stack.is_empty()));
                        }
                        
                        if fn_name == "clear" {
                            stack.clear();
                            return Ok(Value::Stack(stack));
                        }
                    }
                    
                    // Queue methods
                    if let Value::Queue(queue) = &arg_values[0] {
                        let mut queue = queue.clone();
                        
                        if fn_name == "enqueue" && arg_values.len() == 2 {
                            queue.push_back(arg_values[1].clone());
                            return Ok(Value::Queue(queue));
                        }
                        
                        if fn_name == "dequeue" && !queue.is_empty() {
                            let item = queue.pop_front().unwrap();
                            return Ok(Value::Enum(
                                "Option".to_string(),
                                "Some".to_string(),
                                HashMap::from([
                                    ("0".to_string(), item.clone()),
                                    ("1".to_string(), Value::Queue(queue)),
                                ]),
                            ));
                        }
                        
                        if fn_name == "front" && !queue.is_empty() {
                            let item = queue.front().unwrap().clone();
                            return Ok(Value::Enum(
                                "Option".to_string(),
                                "Some".to_string(),
                                HashMap::from([("value".to_string(), item)]),
                            ));
                        }
                        
                        if fn_name == "len" {
                            return Ok(Value::Int(queue.len() as i64));
                        }
                        
                        if fn_name == "is_empty" {
                            return Ok(Value::Bool(queue.is_empty()));
                        }
                        
                        if fn_name == "clear" {
                            queue.clear();
                            return Ok(Value::Queue(queue));
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
                        Expr::Projection { source, target } => {
                let source_val = self.eval_expr(source)?;
                match target {
                    ProjectionTarget::Size => match source_val {
                        Value::List(items) => Ok(Value::Int(items.len() as i64)),
                        Value::String(s) => Ok(Value::Int(s.len() as i64)),
                        _ => Err(RuntimeError::TypeMismatch(
                            "Size projection requires List or String".to_string(),
                        )),
                    },
                    ProjectionTarget::Bytes => {
                        let size = match &source_val {
                            Value::Int(_) => 8,
                            Value::Float(_) => 8,
                            Value::Bool(_) => 1,
                            Value::Char(_) => 4,
                            Value::String(s) => s.len() as i64,
                            Value::List(items) => items.len() as i64 * 8,
                            Value::Instance { fields, .. } => fields.len() as i64 * 8,
                            _ => 0,
                        };
                        Ok(Value::Int(size))
                    }
                    ProjectionTarget::Ptr => {
                        // In the interpreter, pointer addresses are simulated
                        Ok(Value::Int(0))
                    }
                    ProjectionTarget::Alignment => {
                        // Default alignment is 8 bytes
                        Ok(Value::Int(8))
                    }
                    ProjectionTarget::Range => {
                        // Range projection requires compile-time analysis;
                        // in the interpreter, return full i64 range
                        Ok(Value::List(vec![
                            Value::Int(i64::MIN),
                            Value::Int(i64::MAX),
                        ]))
                    }
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
            Expr::Concat(l, r) => {
                let left = self.eval_expr(l)?;
                let right = self.eval_expr(r)?;
                match (left, right) {
                    (Value::List(mut a), Value::List(b)) => {
                        a.extend(b);
                        Ok(Value::List(a))
                    }
                    _ => Err(RuntimeError::TypeMismatch("list concat".to_string())),
                }
            }
            Expr::Slice { value, start, end, stride, mask: _ } => {
                let list_val = self.eval_expr(value)?;
                // String slicing
                if let Value::String(ref s) = list_val {
                    let len = s.len();
                    let start_idx = match start {
                        Some(s_expr) => match self.eval_expr(s_expr)? {
                            Value::Int(n) => if n < 0 { (len as i64 + n).max(0) as usize } else { n as usize },
                            _ => return Err(RuntimeError::TypeMismatch("Slice start must be integer".to_string())),
                        },
                        None => 0,
                    };
                    let end_idx = match end {
                        Some(e_expr) => match self.eval_expr(e_expr)? {
                            Value::Int(n) => if n < 0 { (len as i64 + n).max(0) as usize } else { n as usize },
                            _ => return Err(RuntimeError::TypeMismatch("Slice end must be integer".to_string())),
                        },
                        None => len,
                    };
                    let lo = start_idx.min(len);
                    let hi = end_idx.min(len);
                    return Ok(Value::String(if lo < hi { s[lo..hi].to_string() } else { String::new() }));
                }
                let list = match list_val {
                    Value::List(vec) => vec,
                    _ => return Err(RuntimeError::TypeMismatch("Cannot slice non-list".to_string())),
                };
                
                let len = list.len();
                
                let start_idx = match start {
                    Some(s) => {
let s_val = self.eval_expr(s)?;
                        match s_val {
                            Value::Int(n) => {
                                if n < 0 { (len as i64 + n) as usize } else { n as usize }
                            }
                            _ => return Err(RuntimeError::TypeMismatch("Slice start must be integer".to_string())),
                        }
                    }
                    None => 0,
                };
                
                let end_idx = match end {
                    Some(e) => {
                        let e_val = self.eval_expr(e)?;
                        match e_val {
                            Value::Int(n) => {
                                if n < 0 { (len as i64 + n) as usize } else { n as usize }
                            }
                            _ => return Err(RuntimeError::TypeMismatch("Slice end must be integer".to_string())),
                        }
                    }
                    None => len,
                };
                
                let stride_val = stride.as_ref().map(|s| {
                    let s_val = self.eval_expr(s)?;
                    match s_val {
                        Value::Int(n) => Ok(n as usize),
                        _ => Err(RuntimeError::TypeMismatch("Stride must be integer".to_string())),
                    }
                }).transpose()?;
                
                let stride = stride_val.unwrap_or(1);
                
                let mut result = Vec::new();
                let mut idx = start_idx;
                
                while idx < end_idx && idx < len {
                    result.push(list[idx].clone());
                    if stride == 0 {
                        break;
                    }
                    idx += stride;
                }
                
                Ok(Value::List(result))
            }
            Expr::Block(stmts, last) => {
                let old_state = self.state.clone();
                for stmt in stmts {
                    self.exec_stmt(stmt)?;
                }
                let result = self.eval_expr(last)?;
                self.state = old_state;
                Ok(result)
            }
            Expr::Tuple(exprs) => {
                let mut values = Vec::new();
                for e in exprs {
                    values.push(self.eval_expr(e)?);
                }
                Ok(Value::List(values))
            }
            Expr::TupleDestructure(names, expr) => {
                let value = self.eval_expr(expr)?;
                match value {
                    Value::List(items) => {
                        for (i, name) in names.iter().enumerate() {
                            if i < items.len() {
                                self.state.insert(name.clone(), items[i].clone());
                            }
                        }
                        Ok(Value::Void)
                    }
                    _ => Err(RuntimeError::TypeMismatch(
                        "Tuple destructure requires a list value".to_string(),
                    )),
                }
            }
            Expr::ForAll { .. } => Ok(Value::Bool(true)),
            Expr::Exists { var, expr } => {
                let list = self.eval_expr(expr)?;
                match list {
                    Value::List(items) => Ok(Value::Bool(!items.is_empty())),
                    _ => Err(RuntimeError::TypeMismatch(
                        "Exists requires a list expression".to_string(),
                    )),
                }
            }
            Expr::MultiSlice { value, coordinates, mask: _ } => {
                let base = self.eval_expr(value)?;
                if coordinates.is_empty() {
                    return Ok(base);
                }
                let has_ellipsis = coordinates.iter().any(|c| matches!(c, SliceCoordinate::Ellipsis));
                if has_ellipsis {
                    let dims = Self::list_nesting_depth(&base);
                    let expanded = Self::expand_coordinates(coordinates, dims)?;
                    self.apply_multi_slice_coords(&base, &expanded)
                } else {
                    self.apply_multi_slice_coords(&base, coordinates)
                }
            }
            Expr::Cast(inner, _) => self.eval_expr(inner),
            Expr::Match { value, arms } => {
                let target = self.eval_expr(value)?;
                for arm in arms {
                    let matched = match &arm.pattern {
                        MatchPattern::Wildcard => true,
                        MatchPattern::Variant { name, fields } => {
                            match &target {
                                Value::Enum(_, variant, enum_fields) if variant == name => {
                                    for (i, field_name) in fields.iter().enumerate() {
                                        let key = if i == 0 { "value" } else { "" };
                                        let val = if i == 0 {
                                            enum_fields.get("value").cloned()
                                        } else {
                                            enum_fields.get(&format!("field{}", i)).cloned()
                                        };
                                        if let Some(v) = val {
                                            self.state.insert(field_name.clone(), v);
                                        }
                                    }
                                    true
                                }
                                _ => false,
                            }
                        }
                    };
                    if matched {
                        if let Some(guard) = &arm.guard {
                            let guard_val = self.eval_expr(guard)?;
                            if guard_val != Value::Bool(true) {
                                continue;
                            }
                        }
                        return self.eval_expr(&arm.body);
                    }
                }
                Err(RuntimeError::TypeMismatch(
                    "Non-exhaustive match: no arm matched".to_string(),
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
            Ok(content) => Ok(Value::Enum(
                "Result".to_string(),
                "Ok".to_string(),
                HashMap::from([("result".to_string(), Value::String(content))]),
            )),
            Err(e) => Ok(Value::Enum(
                "Result".to_string(),
                "Err".to_string(),
                HashMap::from([("error".to_string(), Value::String(format!("{}", e)))]),
            )),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_interpreter_with_list() -> Interpreter {
        let mut interp = Interpreter::new();
        interp.state.insert("list".to_string(), Value::List(vec![]));
        interp.state.insert("x".to_string(), Value::Int(0));
        interp
    }

    #[test]
    fn test_arrow_push_append() {
        let mut i = make_interpreter_with_list();
        let expr = Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Integer(42))),
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![Value::Int(42)]));
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![Value::Int(42)])));
    }

    #[test]
    fn test_arrow_push_multiple() {
        let mut i = make_interpreter_with_list();
        let push = |v: i64| Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Integer(v))),
        };
        i.eval_expr(&push(1)).unwrap();
        i.eval_expr(&push(2)).unwrap();
        i.eval_expr(&push(3)).unwrap();
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![
            Value::Int(1), Value::Int(2), Value::Int(3)
        ])));
    }

    #[test]
    fn test_arrow_pop() {
        let mut i = make_interpreter_with_list();
        // First push a value
        i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Integer(99))),
        }).unwrap();
        // Then pop it
        let popped = i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Pop,
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Term),
            value: None,
        }).unwrap();
        assert_eq!(popped, Value::Int(99));
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![])));
    }

    #[test]
    fn test_arrow_discard() {
        let mut i = make_interpreter_with_list();
        // Push 2 values
        i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Integer(10))),
        }).unwrap();
        i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Integer(20))),
        }).unwrap();
        // Discard last
        let discard = Expr::ArrowDiscard {
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Term),
        };
        i.eval_expr(&discard).unwrap();
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![Value::Int(10)])));
    }

    #[test]
    fn test_arrow_push_indexed() {
        let mut i = make_interpreter_with_list();
        // Push 3 items: [10, 20, 30]
        for v in &[10, 20, 30] {
            i.eval_expr(&Expr::ArrowMut {
                dir: ArrowDir::Push,
                target: Box::new(Expr::OwnedRef("list".to_string())),
                index: Box::new(Expr::Term),
                value: Some(Box::new(Expr::Integer(*v))),
            }).unwrap();
        }
        // Insert 15 at index 1: [10, 15, 20, 30]
        i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Integer(1)),
            value: Some(Box::new(Expr::Integer(15))),
        }).unwrap();
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![
            Value::Int(10), Value::Int(15), Value::Int(20), Value::Int(30)
        ])));
    }

    #[test]
    fn test_arrow_pop_indexed() {
        let mut i = make_interpreter_with_list();
        for v in &[10, 20, 30, 40] {
            i.eval_expr(&Expr::ArrowMut {
                dir: ArrowDir::Push,
                target: Box::new(Expr::OwnedRef("list".to_string())),
                index: Box::new(Expr::Term),
                value: Some(Box::new(Expr::Integer(*v))),
            }).unwrap();
        }
        // Pop at index 1 → removes 20
        let popped = i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Pop,
            target: Box::new(Expr::OwnedRef("list".to_string())),
            index: Box::new(Expr::Integer(1)),
            value: None,
        }).unwrap();
        assert_eq!(popped, Value::Int(20));
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![
            Value::Int(10), Value::Int(30), Value::Int(40)
        ])));
    }

    #[test]
    fn test_arrow_discard_field_indexed() {
        use std::collections::HashMap;
        let mut i = Interpreter::new();
        let items = vec![Value::Int(100), Value::Int(200), Value::Int(300)];
        i.state.insert("queue".to_string(), Value::Instance {
            typename: "Queue".to_string(),
            fields: HashMap::from([("items".to_string(), Value::List(items))]),
        });
        // <- &queue.items[0] — discard first element (dequeue)
        let discard = Expr::ArrowDiscard {
            target: Box::new(Expr::FieldAccess(
                Box::new(Expr::OwnedRef("queue".to_string())),
                "items".to_string(),
            )),
            index: Box::new(Expr::Integer(0)),
        };
        i.eval_expr(&discard).unwrap();
        let queue = i.state.get("queue").unwrap();
        match queue {
            Value::Instance { fields, .. } => {
                match fields.get("items").unwrap() {
                    Value::List(items) => {
                        assert_eq!(items.len(), 2);
                        assert_eq!(items[0], Value::Int(200));
                        assert_eq!(items[1], Value::Int(300));
                    }
                    _ => panic!("items field is not a List"),
                }
            }
            _ => panic!("queue is not an Instance"),
        }
    }

    #[test]
    fn test_list_nesting_depth() {
        assert_eq!(Interpreter::list_nesting_depth(&Value::Int(5)), 0);
        assert_eq!(Interpreter::list_nesting_depth(&Value::List(vec![Value::Int(1)])), 1);
        let inner = Value::List(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(Interpreter::list_nesting_depth(&Value::List(vec![inner.clone(), inner.clone()])), 2);
        let innermost = Value::List(vec![Value::Int(1)]);
        let mid = Value::List(vec![innermost.clone(), innermost.clone()]);
        assert_eq!(Interpreter::list_nesting_depth(&Value::List(vec![mid.clone(), mid.clone()])), 3);
    }

    #[test]
    fn test_expand_coordinates() {
        let coords = vec![SliceCoordinate::Ellipsis, SliceCoordinate::Index(Box::new(Expr::Integer(0)))];
        let expanded = Interpreter::expand_coordinates(&coords, 2).unwrap();
        assert_eq!(expanded.len(), 2);
        assert!(matches!(expanded[0], SliceCoordinate::Range { start: None, end: None }));
        // [..., 0] on 3D → [:, :, 0]
        let expanded = Interpreter::expand_coordinates(&coords, 3).unwrap();
        assert_eq!(expanded.len(), 3);
        assert!(matches!(expanded[2], SliceCoordinate::Index(_)));
        // [0, ...] on 3D → [0, :, :]
        let coords2 = vec![SliceCoordinate::Index(Box::new(Expr::Integer(0))), SliceCoordinate::Ellipsis];
        let expanded = Interpreter::expand_coordinates(&coords2, 3).unwrap();
        assert_eq!(expanded.len(), 3);
        assert!(matches!(expanded[0], SliceCoordinate::Index(_)));
        // Multiple ellipses — error
        assert!(Interpreter::expand_coordinates(
            &vec![SliceCoordinate::Ellipsis, SliceCoordinate::Ellipsis], 3
        ).is_err());
        // Too many explicit coords — error
        let coords3 = vec![SliceCoordinate::Index(Box::new(Expr::Integer(0))), SliceCoordinate::Index(Box::new(Expr::Integer(1)))];
        assert!(Interpreter::expand_coordinates(&coords3, 1).is_err());
    }

    #[test]
    fn test_multislice_basic() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let inner2 = Value::List(vec![Value::Int(3), Value::Int(4)]);
        let matrix = Value::List(vec![inner1, inner2]);
        let coords = vec![
            SliceCoordinate::Index(Box::new(Expr::Integer(0))),
            SliceCoordinate::Index(Box::new(Expr::Integer(1))),
        ];
        assert_eq!(i.apply_multi_slice_coords(&matrix, &coords).unwrap(), Value::Int(2));
    }

    #[test]
    fn test_multislice_ellipsis_trailing() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let inner2 = Value::List(vec![Value::Int(3), Value::Int(4)]);
        let matrix = Value::List(vec![inner1, inner2]);
        let coords = vec![
            SliceCoordinate::Range { start: None, end: None },
            SliceCoordinate::Index(Box::new(Expr::Integer(0))),
        ];
        let result = i.apply_multi_slice_coords(&matrix, &coords).unwrap();
        match result {
            Value::List(items) => assert_eq!(items, vec![Value::Int(1), Value::Int(3)]),
            _ => panic!("Expected list result"),
        }
    }

    #[test]
    fn test_multislice_ellipsis_leading() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let inner2 = Value::List(vec![Value::Int(3), Value::Int(4)]);
        let matrix = Value::List(vec![inner1, inner2]);
        let coords = vec![
            SliceCoordinate::Index(Box::new(Expr::Integer(0))),
            SliceCoordinate::Range { start: None, end: None },
        ];
        let result = i.apply_multi_slice_coords(&matrix, &coords).unwrap();
        match result {
            Value::List(items) => assert_eq!(items, vec![Value::Int(1), Value::Int(2)]),
            _ => panic!("Expected list result"),
        }
    }

    #[test]
    fn test_multislice_range_chain() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let inner2 = Value::List(vec![Value::Int(4), Value::Int(5), Value::Int(6)]);
        let inner3 = Value::List(vec![Value::Int(7), Value::Int(8), Value::Int(9)]);
        let matrix = Value::List(vec![inner1, inner2, inner3]);
        let coords = vec![
            SliceCoordinate::Range { start: Some(Box::new(Expr::Integer(0))), end: Some(Box::new(Expr::Integer(2))) },
            SliceCoordinate::Range { start: Some(Box::new(Expr::Integer(1))), end: Some(Box::new(Expr::Integer(3))) },
        ];
        let result = i.apply_multi_slice_coords(&matrix, &coords).unwrap();
        match result {
            Value::List(rows) => {
                assert_eq!(rows.len(), 2);
                match &rows[0] { Value::List(r) => assert_eq!(*r, vec![Value::Int(2), Value::Int(3)]), _ => panic!() }
                match &rows[1] { Value::List(r) => assert_eq!(*r, vec![Value::Int(5), Value::Int(6)]), _ => panic!() }
            }
            _ => panic!("Expected nested list result"),
        }
    }
}
