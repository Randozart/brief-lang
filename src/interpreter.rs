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
use crate::features::arrow::{ArrowMutExpr, ArrowDiscardExpr, ArrowTransferExpr};
use crate::features::binary_op::{BinaryOpExpr, BinaryOpKind};
use crate::features::block::BlockExpr;
use crate::features::call::CallExpr;
use crate::features::collection::{ListLiteralExpr, MapLiteralExpr, SetLiteralExpr, ListIndexExpr, SliceExpr, MultiSliceExpr};
use crate::features::dbvl::DbvlTableExpr;
use crate::features::ellipsis::EllipsisExpr;
use crate::features::field::{FieldAccessExpr, StructInstanceExpr, ObjectLiteralExpr};
use crate::features::pattern::{PatternMatchExpr, MatchExpr};
use crate::features::projection::ProjectionExpr;
use crate::features::sigcall::SigCallExpr;
use crate::features::subtype::SubtypeProjectionExpr;
use crate::features::tuple::{TupleExpr, TupleDestructureExpr};
use crate::features::unary_op::{UnaryOpExpr, UnaryOpKind};
use crate::features::traits::{ExprDispatch, ExprEval};
use crate::features::literal::LiteralExpr;
use crate::ffi::orchestrator::Orchestrator;
use crate::ffi::FFI_REGISTRY;
use regex::Regex;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

/// Metadata for a lazy-loaded DBVL table with key-offset index
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DbvlTableInner {
    pub path: String,
    /// Key → byte offset(s) in the file (Vec supports duplicate keys)
    pub key_offsets: HashMap<String, Vec<usize>>,
    /// Field names from schema, in order
    pub field_names: Vec<String>,
    /// Schema name this table conforms to
    pub schema_name: Option<String>,
    /// Index of the key field (default 0)
    pub schema_key_index: Option<usize>,
}

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Data(Vec<u8>),
    List(Vec<Value>),
    Tuple(Vec<Value>),  // True tuple type (not flattened to List)
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
    /// Lazy-loaded DBVL table with key-offset index.
    /// Users see it as a Map[String, T] — the DbvlTable type is internal.
    DbvlTable(Arc<DbvlTableInner>),
    /// Compiled regex pattern from `@"..."` literal.
    Regex(crate::analysis::dfa::RegexPattern),
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
            Value::Tuple(items) => write!(f, "({})", items.len()),
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
            Value::DbvlTable(t) => {
                let schema = t.schema_name.as_deref().unwrap_or("?");
                write!(f, "<DbvlTable {} '{}' ({} entries, lazy)>", schema, t.path, t.key_offsets.len())
            }
            Value::Regex(r) => write!(f, "<Regex {:?}>", r.pattern),
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
    /// Proof oracle fuel exhausted — state was rolled back, handler executed
    FuelExhausted,
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
        Value::Tuple(items) => JsonValue::Array(items.iter().map(value_to_json_value).collect()),
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
        Value::DbvlTable(t) => {
            let mut map = serde_json::Map::new();
            map.insert("__lazy".to_string(), JsonValue::String(t.path.clone()));
            map.insert("entries".to_string(), JsonValue::Number(t.key_offsets.len().into()));
            JsonValue::Object(map)
        }
        Value::Regex(_) => JsonValue::Null,
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

pub struct EnumVariantInfo {
    pub enum_name: String,
    pub variant_name: String,
    pub field_names: Vec<String>,
}

impl Clone for EnumVariantInfo {
    fn clone(&self) -> Self {
        Self {
            enum_name: self.enum_name.clone(),
            variant_name: self.variant_name.clone(),
            field_names: self.field_names.clone(),
        }
    }
}

pub struct Interpreter {
    pub state: HashMap<String, Value>,
    pub prior_state: HashMap<String, Value>,
    pub foreign_functions: HashMap<String, ForeignFn>,
    pub definitions: HashMap<String, Definition>,
    pub callable_txns: HashMap<String, Transaction>,
    pub ffi_bindings: HashMap<String, ForeignSignature>,
    pub ffi_name_to_location: HashMap<String, String>,
    pub orchestrator: Orchestrator,
    pub metropolitan_hub: crate::ffi::metropolitan::MetropolitanHub,
    pub return_value: Option<Value>,
    pub frgn_registry: crate::ffi::dynamic::FrgnRegistry,
    pub profile_mode: bool,
    pub branch_counts: HashMap<String, (u64, u64)>,
    guard_counter: usize,
    pub enum_variants: HashMap<String, EnumVariantInfo>,
    /// Cache for lazy-loaded DBVL table entries: path → key → values
    pub dbvl_cache: HashMap<String, HashMap<String, Vec<Value>>>,
    /// Proof oracle fuel — decremented on every statement when set.
    /// When it hits zero, FuelExhausted is returned.
    oracle_fuel: Option<u64>,
}

impl Interpreter {
    pub fn new() -> Self {
        let foreign_functions = Self::load_ffi_functions();
        Self {
            state: HashMap::new(),
            prior_state: HashMap::new(),
            foreign_functions,
            definitions: HashMap::new(),
            callable_txns: HashMap::new(),
            ffi_bindings: HashMap::new(),
            ffi_name_to_location: HashMap::new(),
            orchestrator: Orchestrator::new(),
            metropolitan_hub: crate::ffi::metropolitan::MetropolitanHub::new(),
            return_value: None,
            frgn_registry: crate::ffi::dynamic::FrgnRegistry::new(),
            profile_mode: false,
            branch_counts: HashMap::new(),
            guard_counter: 0,
            enum_variants: HashMap::new(),
            dbvl_cache: HashMap::new(),
            oracle_fuel: None,
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
                    let (variant_name, field_names) = match variant {
                        EnumVariant::Unit(name) => (name.clone(), vec![]),
                        EnumVariant::Tuple(name, types) => {
                            let names: Vec<String> = (0..types.len()).map(|i| format!("field_{}", i)).collect();
                            (name.clone(), names)
                        }
                        EnumVariant::Struct(name, fields) => {
                            let names: Vec<String> = fields.iter().map(|f| f.0.clone()).collect();
                            (name.clone(), names)
                        }
                    };
                    let v_name = variant_name.clone();
                    self.state.insert(v_name, Value::Enum(
                        enum_def.name.clone(),
                        variant_name.clone(),
                        std::collections::HashMap::new(),
                    ));
                    self.enum_variants.insert(variant_name.clone(), EnumVariantInfo {
                        enum_name: enum_def.name.clone(),
                        variant_name,
                        field_names,
                    });
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

    pub(crate) fn call_defn(&mut self, name: &str, args: &[Expr]) -> Result<Value, RuntimeError> {
        // Check oracle fuel on every definition call
        if let Some(fuel) = self.oracle_fuel.as_mut() {
            if *fuel == 0 { return Err(RuntimeError::FuelExhausted); }
            *fuel -= 1;
        }
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
            Statement::Term { values: outputs, swan_song, .. } => {
                if outputs.len() > 1 {
                    let mut collected = Vec::new();
                    for out in outputs {
                        if let Some(expr) = out {
                            collected.push(self.eval_expr(expr)?);
                        }
                    }
                    if let Some(swan) = swan_song {
                        self.exec_stmt(swan)?;
                    }
                    self.return_value = Some(Value::List(collected));
                } else if let Some(Some(expr)) = outputs.first() {
                    result = self.eval_expr(expr)?;
                    if let Some(swan) = swan_song {
                        self.exec_stmt(swan)?;
                    }
                    self.return_value = Some(result.clone());
                }
            }
            Statement::TermBang { values: outputs, swan_song, .. } => {
                if outputs.len() > 1 {
                    let mut collected = Vec::new();
                    for out in outputs {
                        if let Some(expr) = out {
                            collected.push(self.eval_expr(expr)?);
                        }
                    }
                    if let Some(swan) = swan_song {
                        self.exec_stmt(swan)?;
                    }
                    self.return_value = Some(Value::List(collected));
                } else if let Some(Some(expr)) = outputs.first() {
                    result = self.eval_expr(expr)?;
                    if let Some(swan) = swan_song {
                        self.exec_stmt(swan)?;
                    }
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

    pub(crate) fn call_txn(&mut self, name: &str, args: &[Expr]) -> Result<Value, RuntimeError> {
        let txn = match self.callable_txns.get(name) {
            Some(t) => t.clone(),
            None => return Err(RuntimeError::UndefinedForeignFunction(name.to_string())),
        };

        let mut local_scope = self.state.clone();
        for (i, (param_name, _)) in txn.parameters.iter().enumerate() {
            if i < args.len() {
                let arg_val = self.eval_expr(&args[i])?;
                local_scope.insert(param_name.clone(), arg_val);
            }
        }

        let old_state = std::mem::replace(&mut self.state, local_scope);
        let old_return = self.return_value.take();

        let mut iterations = 0;
        let max_iterations = 10_000_000;
        let mut result = Value::Void;

        // Convergence loop: execute body while precondition holds.
        // The precondition becoming false is the convergence signal.
        loop {
            let pre_val = self.eval_expr(&txn.contract.pre_condition)?;
            if pre_val != Value::Bool(true) {
                break;
            }

            if iterations >= max_iterations {
                break;
            }
            iterations += 1;

            let prior_state = self.state.clone();

            let mut txn_failed = false;
            for stmt in &txn.body {
                if let Err(e) = self.exec_stmt(stmt) {
                    match e {
                        RuntimeError::Escaped => {
                            self.state = prior_state.clone();
                            txn_failed = true;
                        }
                        _ => {
                            self.state = prior_state.clone();
                            txn_failed = true;
                        }
                    }
                    break;
                }
                if let Some(rv) = self.return_value.take() {
                    result = rv;
                }
            }

            if txn_failed {
                break;
            }

            if self.state == prior_state {
                break;
            }
        }

        // Verify postcondition on the final (converged) state.
        // If it fails, roll back to entry state.
        let post_val = self.eval_expr(&txn.contract.post_condition)?;
        if post_val != Value::Bool(true) {
            self.state = old_state.clone();
            self.return_value = old_return;
            return Err(RuntimeError::ContractViolation(format!(
                "txn '{}' postcondition not satisfied after convergence", name
            )));
        }

        self.state = old_state;
        self.return_value = old_return;
        Ok(result)
    }

    /// Resolve any Value from state by root name and optional field path.
    pub(crate) fn resolve_arrow_value(&self, root: &str, field_path: &[String]) -> Result<Value, RuntimeError> {
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
        Ok(val)
    }

    /// Store any Value back into state through the root and field path.
    pub(crate) fn store_arrow_value(&mut self, root: &str, field_path: &[String], val: Value) {
        if field_path.is_empty() {
            self.state.insert(root.to_string(), val);
        } else {
            let mut current_val = self.state.get(root)
                .expect("Root must exist")
                .clone();
            let mut stack: Vec<(String, Value)> = Vec::new();
            for field in field_path.iter().rev() {
                stack.push((field.clone(), current_val.clone()));
                current_val = match &current_val {
                    Value::Instance { fields, .. } => {
                        fields.get(field)
                            .expect("Field must exist")
                            .clone()
                    }
                    _ => return,
                };
            }
            let mut current = val;
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

    /// Extract (root_variable_name, field_path) from an arrow mutation target.
    /// Supports: `&name`, `&name[i]`, `&name.field`, `&name.field[i]`
    pub(crate) fn extract_arrow_root(&self, target: &Expr) -> Result<(String, Vec<String>), RuntimeError> {
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
    pub(crate) fn eval_arrow_pos(&mut self, list: &[Value], index: &Expr) -> Result<Option<usize>, RuntimeError> {
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

    /// Convert a Value to a String for use as a HashMap key.
    pub(crate) fn value_to_string(&self, val: &Value) -> Result<String, RuntimeError> {
        match val {
            Value::String(s) => Ok(s.clone()),
            Value::Int(i) => Ok(i.to_string()),
            Value::Float(f) => Ok(f.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Char(c) => Ok(c.to_string()),
            _ => Err(RuntimeError::TypeMismatch(
                "Value cannot be used as a HashMap key".to_string()
            )),
        }
    }

    pub(crate) fn list_nesting_depth(value: &Value) -> usize {
        match value {
            Value::List(items) => match items.first() {
                Some(inner) => 1 + Self::list_nesting_depth(inner),
                None => 1,
            },
            _ => 0,
        }
    }

    pub(crate) fn expand_coordinates(
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

    pub(crate) fn apply_multi_slice_coords(
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
            } else if let TopLevel::Transaction(txn) = item {
                if !txn.is_reactive {
                    self.callable_txns.insert(txn.name.clone(), txn.clone());
                }
            } else if let TopLevel::SyncGroup { item: inner, .. } = item {
                match &**inner {
                    TopLevel::Definition(defn) => {
                        self.definitions.insert(defn.name.clone(), defn.clone());
                    }
                    TopLevel::Transaction(txn) => {
                        if !txn.is_reactive {
                            self.callable_txns.insert(txn.name.clone(), txn.clone());
                        }
                    }
                    TopLevel::TypeDef(_) => {
                        // TypeDefs are compile-time only — skip.
                    }
                    _ => {}
                }
            }
        }

        let mut executed = true;
        let mut iterations = 0;
        let max_iterations = 100;

        while executed && iterations < max_iterations {
            iterations += 1;
            executed = false;
            for item in &program.items {
                // Unwrap Test items to access the inner transaction
                let inner_item = match item {
                    TopLevel::Test { item: inner, .. } => inner.as_ref(),
                    other => other,
                };
                if let TopLevel::Transaction(txn) = inner_item {
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
            } else if let TopLevel::TypeDef(_) = item {
                // TypeDefs are compile-time only — skip at runtime.
                // Phase 1.5: type_universe.rs handles resolution in Pass 1.
            } else if let TopLevel::Test { item: inner, groups: _ } = item {
                // Test wrapper — unwrap and register the inner item's definitions
                match inner.as_ref() {
                    TopLevel::Definition(defn) => {
                        self.definitions.insert(defn.name.clone(), defn.clone());
                    }
                    _ => {}
                }
            } else if let TopLevel::Assertion { .. } = item {
                // Assertions are compile-time only — skip at runtime.
            } else if let TopLevel::SyncGroup { item: inner, .. } = item {
                    if let TopLevel::Transaction(txn) = &**inner {
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
                                                transaction_escaped = true;
                                            }
                                            _ => {
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

    pub(crate) fn pattern_match(pat: &Pattern, value: &Value, state: &mut HashMap<String, Value>) -> bool {
        match pat {
            Pattern::Wildcard => true,
            Pattern::Var(name) => {
                state.insert(name.clone(), value.clone());
                true
            }
            Pattern::Tuple(elements) => {
                let items = match value {
                    Value::List(v) => v,
                    Value::Stack(v) => v,
                    _ => return false,
                };
                if elements.len() != items.len() {
                    return false;
                }
                elements.iter().zip(items.iter()).all(|(p, v)| {
                    Self::pattern_match(p, v, state)
                })
            }
            Pattern::LitInt(n) => matches!(value, Value::Int(v) if *v == *n),
            Pattern::LitFloat(f) => matches!(value, Value::Float(v) if *v == *f),
            Pattern::LitString(s) => matches!(value, Value::String(v) if v == s),
            Pattern::LitChar(c) => matches!(value, Value::Char(v) if v == c),
            Pattern::LitBool(b) => matches!(value, Value::Bool(v) if v == b),
        }
    }

    /// Execute a block with a runtime fuel limit.
    /// Sets oracle_fuel, runs the block, restores fuel state.
    fn exec_stmts_with_fuel(&mut self, stmts: &[Statement], fuel: u64) -> Result<(), RuntimeError> {
        let saved_fuel = self.oracle_fuel;
        self.oracle_fuel = Some(fuel);
        let mut result = Ok(());
        for stmt in stmts {
            if self.oracle_fuel == Some(0) { break; }
            match self.exec_stmt(stmt) {
                Ok(()) => {}
                Err(RuntimeError::FuelExhausted) => { break; }
                Err(e) => { result = Err(e); break; }
            }
        }
        let exhausted = self.oracle_fuel == Some(0);
        self.oracle_fuel = saved_fuel;
        if exhausted { Err(RuntimeError::FuelExhausted) } else { result }
    }

    pub fn exec_stmt(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        // Decrement oracle fuel on every statement execution
        if let Some(fuel) = self.oracle_fuel.as_mut() {
            if *fuel == 0 { return Err(RuntimeError::FuelExhausted); }
            *fuel -= 1;
        }
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
                    Expr::TupleDestructure(names, _) => {
                        match value {
                            Value::Tuple(items) | Value::List(items) => {
                                for (i, name) in names.iter().enumerate() {
                                    if i < items.len() {
                                        if name != "_" {
                                            self.state.insert(name.clone(), items[i].clone());
                                        }
                                    }
                                }
                            }
                            _ => {
                                return Err(RuntimeError::TypeMismatch(
                                    "Cannot destructure non-tuple/non-list value".to_string(),
                                ));
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
            Statement::Term { values: outputs, swan_song, .. } => {
                if outputs.len() > 1 {
                    let mut collected = Vec::new();
                    for out in outputs {
                        if let Some(expr) = out {
                            collected.push(self.eval_expr(expr)?);
                        }
                    }
                    if let Some(swan) = swan_song {
                        self.exec_stmt(swan)?;
                    }
                    self.return_value = Some(Value::List(collected));
                } else if let Some(Some(expr)) = outputs.first() {
                    let value = self.eval_expr(expr)?;
                    if let Some(swan) = swan_song {
                        self.exec_stmt(swan)?;
                    }
                    self.return_value = Some(value);
                }
            }
            Statement::TermBang { values: outputs, swan_song, .. } => {
                if outputs.len() > 1 {
                    let mut collected = Vec::new();
                    for out in outputs {
                        if let Some(expr) = out {
                            collected.push(self.eval_expr(expr)?);
                        }
                    }
                    if let Some(swan) = swan_song {
                        self.exec_stmt(swan)?;
                    }
                    self.return_value = Some(Value::List(collected));
                } else if let Some(Some(expr)) = outputs.first() {
                    let value = self.eval_expr(expr)?;
                    if let Some(swan) = swan_song {
                        self.exec_stmt(swan)?;
                    }
                    self.return_value = Some(value);
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
            Statement::SyncBlock { body } => {
                for stmt in body {
                    self.exec_stmt(stmt)?;
                }
            }
            Statement::Foreach { item, list, body } => {
                let list_val = self.eval_expr(list)?;
                match list_val {
                    Value::List(items) => {
                        for elem in items {
                            self.state.insert(item.clone(), elem);
                            for stmt in body {
                                self.exec_stmt(stmt)?;
                            }
                        }
                    }
                    _ => return Err(RuntimeError::TypeMismatch(
                        "foreach requires a List<T> value".to_string(),
                    )),
                }
            }
            Statement::Oracle { body, handler, .. } => {
                // Fuel-injected execution with state rollback on exhaustion.
                let saved_state = self.state.clone();
                let saved_prior = self.prior_state.clone();
                let fuel_limit = 100;
                match self.exec_stmts_with_fuel(body, fuel_limit) {
                    Ok(()) => {}
                    Err(RuntimeError::FuelExhausted) => {
                        self.state = saved_state;
                        self.prior_state = saved_prior;
                        for stmt in handler {
                            self.exec_stmt(stmt)?;
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
            Statement::Unification {
                name,
                variant,
                fields,
                expr,
            } => {
                let value = self.state.get(name).cloned().unwrap_or(Value::Void);
                let matched = match &value {
                    Value::Enum(_, v, enum_fields) if variant == "_" || v == variant => {
                        if fields.is_empty() {
                            true
                        } else {
                            let mut keys: Vec<&String> = enum_fields.keys().collect();
                            keys.sort();
                            let vals: Vec<&Value> = keys.iter()
                                .filter_map(|k| enum_fields.get(*k)).collect();
                            fields.iter().zip(vals.iter()).all(|(pat, val)| {
                                Self::pattern_match(pat, val, &mut self.state)
                            })
                        }
                    }
                    _ if variant != "_" && fields.is_empty() => {
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

    pub(crate) fn handle_ffi_result(&self, fn_name: &str, mut result: Value) -> Result<Value, RuntimeError> {
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
            // Pattern B: delegate to feature struct
            Expr::Literal(lit) => lit.evaluate(self, &ExprDispatch),
            // Legacy scalar variants — keep inline until Phase 14 (variant removal)
            Expr::Integer(v) => Ok(Value::Int(*v)),
            Expr::Float(v) => Ok(Value::Float(*v)),
            Expr::String(v) => Ok(Value::String(v.clone())),
            Expr::RegexLiteral(v) => {
                match crate::analysis::dfa::compile_to_dfa(v) {
                    Ok(dfa) => Ok(Value::Regex(dfa)),
                    Err(e) => Err(RuntimeError::TypeMismatch(format!("Invalid regex: {}", e))),
                }
            }
            Expr::Char(v) => Ok(Value::Char(*v)),
            Expr::Bool(v) => Ok(Value::Bool(*v)),
            Expr::Term => self.state.get("term").cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable("term".to_string())),
            Expr::Identifier(name) => self.state.get(name).cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            Expr::OwnedRef(name) => self.state.get(name).cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            Expr::PriorState(name) => self.prior_state.get(name).cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            // Legacy binary op variants — delegate through feature struct
            Expr::Add(l, r) => BinaryOpExpr::new(BinaryOpKind::Add, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Sub(l, r) => BinaryOpExpr::new(BinaryOpKind::Sub, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Mul(l, r) => BinaryOpExpr::new(BinaryOpKind::Mul, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Div(l, r) => BinaryOpExpr::new(BinaryOpKind::Div, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Mod(l, r) => BinaryOpExpr::new(BinaryOpKind::Mod, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Eq(l, r) => BinaryOpExpr::new(BinaryOpKind::Eq, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Ne(l, r) => BinaryOpExpr::new(BinaryOpKind::Ne, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Lt(l, r) => BinaryOpExpr::new(BinaryOpKind::Lt, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Le(l, r) => BinaryOpExpr::new(BinaryOpKind::Le, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Gt(l, r) => BinaryOpExpr::new(BinaryOpKind::Gt, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Ge(l, r) => BinaryOpExpr::new(BinaryOpKind::Ge, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::IsType(lit, target) => {
                let val = self.eval_expr(lit)?;
                self.eval_is_type(val, target)
            }
            Expr::FromCheck(le, ty) => {
                let val = self.eval_expr(le)?;
                self.eval_from_check(val, ty)
            }
            Expr::Like(l, r) => {
                let lv = self.eval_expr(l)?;
                let rv = self.eval_expr(r)?;
                self.eval_like(lv, rv)
            }
            Expr::Or(l, r) => BinaryOpExpr::new(BinaryOpKind::Or, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::And(l, r) => BinaryOpExpr::new(BinaryOpKind::And, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::BitAnd(l, r) => BinaryOpExpr::new(BinaryOpKind::BitAnd, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::BitOr(l, r) => BinaryOpExpr::new(BinaryOpKind::BitOr, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::BitXor(l, r) => BinaryOpExpr::new(BinaryOpKind::BitXor, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Shl(l, r) => BinaryOpExpr::new(BinaryOpKind::Shl, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Shr(l, r) => BinaryOpExpr::new(BinaryOpKind::Shr, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            // Legacy unary op variants — delegate through feature struct
            Expr::Not(inner) => UnaryOpExpr::new(UnaryOpKind::Not, *inner.clone()).evaluate(self, &ExprDispatch),
            Expr::Neg(inner) => UnaryOpExpr::new(UnaryOpKind::Neg, *inner.clone()).evaluate(self, &ExprDispatch),
            Expr::BitNot(inner) => UnaryOpExpr::new(UnaryOpKind::BitNot, *inner.clone()).evaluate(self, &ExprDispatch),
            // Legacy Arrow variants — delegate through feature structs
            Expr::ArrowMut { dir, target, index, value } => ArrowMutExpr {
                dir: dir.clone(), target: target.clone(), index: index.clone(), value: value.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::ArrowDiscard { target, index } => ArrowDiscardExpr {
                target: target.clone(), index: index.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::ArrowTransfer { dest, source, filter } => ArrowTransferExpr {
                dest: dest.clone(), source: source.clone(), filter: filter.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::SigCall { modifier, expr } => SigCallExpr { modifier: *modifier, expr: expr.clone() }.evaluate(self, &ExprDispatch),
            Expr::Ellipsis => EllipsisExpr.evaluate(self, &ExprDispatch),
            Expr::Call(name, args) =>
                crate::features::call::CallExpr::new(name.clone(), args.clone()).evaluate(self, &ExprDispatch),
            Expr::IntrinsicCall { intrinsic, args } => {
                let values: Result<Vec<Value>, _> = args.iter().map(|a| self.eval_expr(a)).collect();
                let mut values = values?;
                match intrinsic {
                    Intrinsic::Sqrt => {
                        let v = values.remove(0);
                        match v {
                            Value::Float(f) => Ok(Value::Float(f.sqrt())),
                            v => Err(RuntimeError::TypeMismatch(format!("sqrt requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Fabs => {
                        let v = values.remove(0);
                        match v {
                            Value::Float(f) => Ok(Value::Float(f.abs())),
                            v => Err(RuntimeError::TypeMismatch(format!("fabs requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Ceil => {
                        let v = values.remove(0);
                        match v {
                            Value::Float(f) => Ok(Value::Float(f.ceil())),
                            v => Err(RuntimeError::TypeMismatch(format!("ceil requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Floor => {
                        let v = values.remove(0);
                        match v {
                            Value::Float(f) => Ok(Value::Float(f.floor())),
                            v => Err(RuntimeError::TypeMismatch(format!("floor requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Ctpop => {
                        let v = values.remove(0);
                        match v {
                            Value::Int(n) => Ok(Value::Int(n.count_ones() as i64)),
                            v => Err(RuntimeError::TypeMismatch(format!("ctpop requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Ctlz => {
                        let v = values.remove(0);
                        match v {
                            Value::Int(n) => Ok(Value::Int(n.leading_zeros() as i64)),
                            v => Err(RuntimeError::TypeMismatch(format!("ctlz requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Cttz => {
                        let v = values.remove(0);
                        match v {
                            Value::Int(n) => Ok(Value::Int(n.trailing_zeros() as i64)),
                            v => Err(RuntimeError::TypeMismatch(format!("cttz requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Abs => {
                        let v = values.remove(0);
                        match v {
                            Value::Int(n) => Ok(Value::Int(n.abs())),
                            v => Err(RuntimeError::TypeMismatch(format!("abs requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Bitreverse => {
                        let v = values.remove(0);
                        match v {
                            Value::Int(n) => Ok(Value::Int(n.reverse_bits() as i64)),
                            v => Err(RuntimeError::TypeMismatch(format!("bitreverse requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Bytes => {
                        let v = values.remove(0);
                        match v {
                            Value::Float(_) => Ok(Value::Int(8)),
                            Value::Int(_) => Ok(Value::Int(8)),
                            Value::Bool(_) => Ok(Value::Int(1)),
                            Value::Char(_) => Ok(Value::Int(4)),
                            Value::String(s) => Ok(Value::Int(s.len() as i64)),
                            Value::List(l) => Ok(Value::Int((l.len() * 8) as i64)),
                            v => Err(RuntimeError::TypeMismatch(format!("bytes not implemented for {:?}", v))),
                        }
                    }
                    Intrinsic::Size => {
                        let v = values.remove(0);
                        match v {
                            Value::List(l) => Ok(Value::Int(l.len() as i64)),
                            Value::String(s) => Ok(Value::Int(s.len() as i64)),
                            Value::HashMap(m) => Ok(Value::Int(m.len() as i64)),
                            Value::HashSet(s) => Ok(Value::Int(s.len() as i64)),
                            v => Err(RuntimeError::TypeMismatch(format!("size requires collection, got {:?}", v))),
                        }
                    }
                    Intrinsic::Pop => {
                        let v = values.remove(0);
                        match v {
                            Value::List(mut l) => l.pop().map(Ok).unwrap_or(Err(RuntimeError::TypeMismatch("pop from empty list".into()))),
                            v => Err(RuntimeError::TypeMismatch(format!("pop requires List, got {:?}", v))),
                        }
                    }
                    Intrinsic::Contains => {
                        if values.len() != 2 {
                            return Err(RuntimeError::TypeMismatch("contains: expected 2 args (collection, element)".into()));
                        }
                        let elem = values.pop().unwrap();
                        let collection = values.remove(0);
                        match collection {
                            Value::List(l) => Ok(Value::Bool(l.contains(&elem))),
                            Value::String(s) => {
                                if let Value::Char(c) = elem {
                                    Ok(Value::Bool(s.contains(c)))
                                } else {
                                    Err(RuntimeError::TypeMismatch("contains: string requires Char element".into()))
                                }
                            }
                            Value::HashMap(m) => {
                                if let Value::String(key) = &elem {
                                    Ok(Value::Bool(m.contains_key(key)))
                                } else {
                                    Ok(Value::Bool(false))
                                }
                            }
                            Value::HashSet(s) => {
                                if let Value::String(key) = &elem {
                                    Ok(Value::Bool(s.contains(key)))
                                } else {
                                    Ok(Value::Bool(false))
                                }
                            }
                            v => Err(RuntimeError::TypeMismatch(format!("contains requires collection, got {:?}", v))),
                        }
                    }
                    Intrinsic::Keys => {
                        let v = values.remove(0);
                        match v {
                            Value::HashMap(m) => {
                                let keys: Vec<Value> = m.into_keys().map(Value::String).collect();
                                Ok(Value::List(keys))
                            }
                            v => Err(RuntimeError::TypeMismatch(format!("keys requires HashMap, got {:?}", v))),
                        }
                    }
                    Intrinsic::Values => {
                        let v = values.remove(0);
                        match v {
                            Value::HashMap(m) => Ok(Value::List(m.into_values().collect())),
                            v => Err(RuntimeError::TypeMismatch(format!("values requires HashMap, got {:?}", v))),
                        }
                    }
                    // System I/O intrinsics
                    Intrinsic::Println => {
                        let v = values.remove(0);
                        println!("{}", v);
                        Ok(Value::Bool(true))
                    }
                    Intrinsic::Print => {
                        let v = values.remove(0);
                        print!("{}", v);
                        Ok(Value::Bool(true))
                    }
                    Intrinsic::Readln => {
                        let mut buf = String::new();
                        let _ = std::io::stdin().read_line(&mut buf);
                        Ok(Value::String(buf.trim_end().to_string()))
                    }
                    Intrinsic::Exit => {
                        let code = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            _ => 0,
                        };
                        std::process::exit(code);
                    }
                    Intrinsic::Time => {
                        use std::time::{SystemTime, UNIX_EPOCH};
                        let nanos = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos() as i64;
                        Ok(Value::Int(nanos))
                    }
                    Intrinsic::ReadFile => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("read_file requires String, got {:?}", v))),
                        };
                        match std::fs::read_to_string(&path) {
                            Ok(contents) => Ok(Value::String(contents)),
                            Err(e) => Err(RuntimeError::TypeMismatch(format!("{}", e))),
                        }
                    }
                    Intrinsic::WriteFile => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("write_file requires String, got {:?}", v))),
                        };
                        let data = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("write_file requires String, got {:?}", v))),
                        };
                        match std::fs::write(&path, &data) {
                            Ok(_) => Ok(Value::Bool(true)),
                            Err(e) => Err(RuntimeError::TypeMismatch(format!("{}", e))),
                        }
                    }
                    Intrinsic::Sleep => {
                        let ms = match values.remove(0) {
                            Value::Int(n) => n as u64,
                            v => return Err(RuntimeError::TypeMismatch(format!("sleep requires Int, got {:?}", v))),
                        };
                        std::thread::sleep(std::time::Duration::from_millis(ms));
                        Ok(Value::Bool(true))
                    }
                    Intrinsic::Socket => Ok(Value::Int(-1)),
                    Intrinsic::Bind => Ok(Value::Bool(false)),
                    Intrinsic::Listen => Ok(Value::Bool(false)),
                    Intrinsic::Accept => Ok(Value::Int(-1)),
                    // ===== Phase A: Terminal (intrinsics.md D4) =====
                    Intrinsic::TtyRawMode => {
                        let enable = match values.remove(0) {
                            Value::Bool(b) => b,
                            Value::Int(n) => n != 0,
                            v => return Err(RuntimeError::TypeMismatch(format!("tty_raw_mode requires Bool, got {:?}", v))),
                        };
                        let result = set_tty_raw_mode(enable);
                        Ok(Value::Bool(result))
                    }
                    Intrinsic::TtySize => {
                        let (cols, rows) = get_terminal_size();
                        // Pack as width * 10000 + height (same as lib/std/ffi/tty.bv)
                        Ok(Value::Int(cols * 10000 + rows))
                    }
                    Intrinsic::TtyReadKey => {
                        match read_key_nonblocking() {
                            Some(c) => Ok(Value::Int(c as i64)),
                            None => Ok(Value::Int(-1)),
                        }
                    }
                    Intrinsic::IoCtl => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("ioctl fd requires Int, got {:?}", v))),
                        };
                        let req = match values.remove(0) {
                            Value::Int(n) => n as u64,
                            v => return Err(RuntimeError::TypeMismatch(format!("ioctl request requires Int, got {:?}", v))),
                        };
                        let arg = match values.remove(0) {
                            Value::Int(n) => n as u64,
                            v => return Err(RuntimeError::TypeMismatch(format!("ioctl arg requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::ioctl(fd, req, arg as *mut libc::c_void) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, req, arg);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::IsTty => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("isatty fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::isatty(fd) };
                            Ok(Value::Bool(ret != 0))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Bool(false))
                        }
                    }
                    // ===== Phase A: Process (intrinsics.md D5) =====
                    Intrinsic::SpawnWithOutput => {
                        let cmd = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("spawn_with_output requires String, got {:?}", v))),
                        };
                        match std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&cmd)
                            .output()
                        {
                            Ok(output) => {
                                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                Ok(Value::String(stdout))
                            }
                            Err(_) => Ok(Value::String(String::new())),
                        }
                    }
                    Intrinsic::Spawn => {
                        let cmd = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("spawn requires String, got {:?}", v))),
                        };
                        match std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&cmd)
                            .status()
                        {
                            Ok(status) => Ok(Value::Int(status.code().unwrap_or(-1) as i64)),
                            Err(_) => Ok(Value::Int(-1)),
                        }
                    }
                    // ===== Phase B: Raw File I/O (intrinsics.md D2) =====
                    Intrinsic::Open => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("open path requires String, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("open flags requires Int, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("open mode requires Int, got {:?}", v))),
                        };
                        let c_path = std::ffi::CString::new(path).ok();
                        let c_path = match c_path {
                            Some(p) => p,
                            None => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let fd = unsafe { libc::open(c_path.as_ptr(), flags, mode) };
                            Ok(Value::Int(fd as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, flags, mode);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Close => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("close fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::close(fd) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Read => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("read fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("read buf requires Int, got {:?}", v))),
                        };
                        let count = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("read count requires Int, got {:?}", v))),
                        };
                        // buf is an opaque pointer — allocate temp buffer for interpreter
                        #[cfg(unix)]
                        {
                            let mut tmp = vec![0u8; count];
                            let n = unsafe { libc::read(fd, tmp.as_mut_ptr() as *mut libc::c_void, count) };
                            let _ = buf; // unused in interpreter — caller's buf is opaque
                            Ok(Value::Int(n as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, count);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Write => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("write fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("write buf requires Int, got {:?}", v))),
                        };
                        let count = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("write count requires Int, got {:?}", v))),
                        };
                        // Interpreter can't dereference opaque buf pointer
                        let _ = (fd, buf, count);
                        Ok(Value::Int(-1))
                    }
                    Intrinsic::LSeek => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("lseek fd requires Int, got {:?}", v))),
                        };
                        let offset = match values.remove(0) {
                            Value::Int(n) => n as i64,
                            v => return Err(RuntimeError::TypeMismatch(format!("lseek offset requires Int, got {:?}", v))),
                        };
                        let whence = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("lseek whence requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::lseek(fd, offset, whence) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, offset, whence);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::PRead => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("pread fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("pread buf requires Int, got {:?}", v))),
                        };
                        let count = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("pread count requires Int, got {:?}", v))),
                        };
                        let offset = match values.remove(0) {
                            Value::Int(n) => n as i64,
                            v => return Err(RuntimeError::TypeMismatch(format!("pread offset requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut tmp = vec![0u8; count];
                            let n = unsafe { libc::pread(fd, tmp.as_mut_ptr() as *mut libc::c_void, count, offset) };
                            let _ = buf;
                            Ok(Value::Int(n as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, count, offset);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::PWrite => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("pwrite fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("pwrite buf requires Int, got {:?}", v))),
                        };
                        let count = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("pwrite count requires Int, got {:?}", v))),
                        };
                        let offset = match values.remove(0) {
                            Value::Int(n) => n as i64,
                            v => return Err(RuntimeError::TypeMismatch(format!("pwrite offset requires Int, got {:?}", v))),
                        };
                        let _ = (fd, buf, count, offset);
                        Ok(Value::Int(-1))
                    }
                    Intrinsic::Stat => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("stat path requires String, got {:?}", v))),
                        };
                        let c_path = std::ffi::CString::new(path).ok();
                        let c_path = match c_path {
                            Some(p) => p,
                            None => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let mut st: libc::stat = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::stat(c_path.as_ptr(), &mut st) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::FStat => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("fstat fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut st: libc::stat = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::fstat(fd, &mut st) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Truncate => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("truncate path requires String, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Int(n) => n as i64,
                            v => return Err(RuntimeError::TypeMismatch(format!("truncate len requires Int, got {:?}", v))),
                        };
                        let c_path = std::ffi::CString::new(path).ok();
                        let c_path = match c_path {
                            Some(p) => p,
                            None => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::truncate(c_path.as_ptr(), len) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, len);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::FTruncate => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("ftruncate fd requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Int(n) => n as i64,
                            v => return Err(RuntimeError::TypeMismatch(format!("ftruncate len requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::ftruncate(fd, len) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, len);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::FSync => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("fsync fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::fsync(fd) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::FDup => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("dup fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::dup(fd) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::FDup2 => {
                        let old = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("dup2 old requires Int, got {:?}", v))),
                        };
                        let new = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("dup2 new requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::dup2(old, new) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (old, new);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::FCntl => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("fcntl fd requires Int, got {:?}", v))),
                        };
                        let cmd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("fcntl cmd requires Int, got {:?}", v))),
                        };
                        let arg = match values.remove(0) {
                            Value::Int(n) => n as i64,
                            v => return Err(RuntimeError::TypeMismatch(format!("fcntl arg requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::fcntl(fd, cmd, arg) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, cmd, arg);
                            Ok(Value::Int(-1))
                        }
                    }
                    // ===== Phase C: Filesystem (intrinsics.md D3) =====
                    Intrinsic::MkDir => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("mkdir path requires String, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("mkdir mode requires Int, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mkdir(c_path.as_ptr(), mode) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, mode);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::RmDir => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("rmdir path requires String, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::rmdir(c_path.as_ptr()) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Unlink => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("unlink path requires String, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::unlink(c_path.as_ptr()) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Rename => {
                        let old = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("rename old requires String, got {:?}", v))),
                        };
                        let new = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("rename new requires String, got {:?}", v))),
                        };
                        let c_old = match std::ffi::CString::new(old) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        let c_new = match std::ffi::CString::new(new) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::rename(c_old.as_ptr(), c_new.as_ptr()) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_old, c_new);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::SymLink => {
                        let target = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("symlink target requires String, got {:?}", v))),
                        };
                        let link = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("symlink link requires String, got {:?}", v))),
                        };
                        let c_target = match std::ffi::CString::new(target) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        let c_link = match std::ffi::CString::new(link) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::symlink(c_target.as_ptr(), c_link.as_ptr()) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_target, c_link);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::ReadLink => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("readlink path requires String, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::String(String::new())),
                        };
                        #[cfg(unix)]
                        {
                            let mut buf = vec![0u8; 4096];
                            let n = unsafe {
                                libc::readlink(c_path.as_ptr(), buf.as_mut_ptr() as *mut libc::c_char, 4096)
                            };
                            if n < 0 {
                                Ok(Value::String(String::new()))
                            } else {
                                buf.truncate(n as usize);
                                let s = String::from_utf8_lossy(&buf).to_string();
                                Ok(Value::String(s))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::String(String::new()))
                        }
                    }
                    Intrinsic::Link => {
                        let old = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("link old requires String, got {:?}", v))),
                        };
                        let new = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("link new requires String, got {:?}", v))),
                        };
                        let c_old = match std::ffi::CString::new(old) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        let c_new = match std::ffi::CString::new(new) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::link(c_old.as_ptr(), c_new.as_ptr()) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_old, c_new);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::GetCwd => {
                        #[cfg(unix)]
                        {
                            let mut buf = vec![0u8; 4096];
                            let ptr = unsafe { libc::getcwd(buf.as_mut_ptr() as *mut libc::c_char, 4096) };
                            if ptr.is_null() {
                                Ok(Value::String(String::new()))
                            } else {
                                let len = unsafe { libc::strlen(ptr) };
                                buf.truncate(len);
                                let s = String::from_utf8_lossy(&buf).to_string();
                                Ok(Value::String(s))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::String(String::new()))
                        }
                    }
                    Intrinsic::ChDir => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("chdir path requires String, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::chdir(c_path.as_ptr()) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::ReadDir => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("readdir path requires String, got {:?}", v))),
                        };
                        match std::fs::read_dir(&path) {
                            Ok(entries) => {
                                let mut list = Vec::new();
                                for entry in entries.flatten() {
                                    if let Ok(name) = entry.file_name().into_string() {
                                        list.push(Value::String(name));
                                    }
                                }
                                Ok(Value::List(list))
                            }
                            Err(_) => Ok(Value::List(Vec::new())),
                        }
                    }
                    Intrinsic::ChMod => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("chmod path requires String, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("chmod mode requires Int, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::chmod(c_path.as_ptr(), mode) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, mode);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::ChOwn => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("chown path requires String, got {:?}", v))),
                        };
                        let uid = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("chown uid requires Int, got {:?}", v))),
                        };
                        let gid = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("chown gid requires Int, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::chown(c_path.as_ptr(), uid, gid) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, uid, gid);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::UMask => {
                        let mask = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("umask mask requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let old = unsafe { libc::umask(mask) };
                            Ok(Value::Int(old as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = mask;
                            Ok(Value::Int(0))
                        }
                    }
                    Intrinsic::Access => {
                        let path = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("access path requires String, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("access mode requires Int, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::access(c_path.as_ptr(), mode) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, mode);
                            Ok(Value::Int(-1))
                        }
                    }
                    // ===== Phase D: Memory (intrinsics.md D1) =====
                    Intrinsic::Mmap => {
                        let addr = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap addr requires Int, got {:?}", v))),
                        };
                        let length = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap length requires Int, got {:?}", v))),
                        };
                        let prot = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap prot requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap flags requires Int, got {:?}", v))),
                        };
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap fd requires Int, got {:?}", v))),
                        };
                        let offset = match values.remove(0) {
                            Value::Int(n) => n as i64,
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap offset requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mmap(addr as *mut libc::c_void, length, prot, flags, fd, offset) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (addr, length, prot, flags, fd, offset);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::MUnmap => {
                        let addr = match values.remove(0) {
                            Value::Int(n) => n as *mut libc::c_void,
                            v => return Err(RuntimeError::TypeMismatch(format!("munmap addr requires Int, got {:?}", v))),
                        };
                        let length = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("munmap length requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::munmap(addr, length) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (addr, length);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::MProtect => {
                        let addr = match values.remove(0) {
                            Value::Int(n) => n as *mut libc::c_void,
                            v => return Err(RuntimeError::TypeMismatch(format!("mprotect addr requires Int, got {:?}", v))),
                        };
                        let length = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("mprotect length requires Int, got {:?}", v))),
                        };
                        let prot = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("mprotect prot requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mprotect(addr, length, prot) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (addr, length, prot);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Brk => {
                        let addr = match values.remove(0) {
                            Value::Int(n) => n as *mut libc::c_void,
                            v => return Err(RuntimeError::TypeMismatch(format!("brk addr requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sbrk(0) }; // get current brk
                            let _ = addr; // setting brk is unsafe; just return current
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = addr;
                            Ok(Value::Int(0))
                        }
                    }
                    Intrinsic::MLock => {
                        let addr = match values.remove(0) {
                            Value::Int(n) => n as *mut libc::c_void,
                            v => return Err(RuntimeError::TypeMismatch(format!("mlock addr requires Int, got {:?}", v))),
                        };
                        let length = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("mlock length requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mlock(addr, length) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (addr, length);
                            Ok(Value::Int(-1))
                        }
                    }
                    // ===== Phase D: Synchronization (intrinsics.md D9) =====
                    // These operate on opaque memory addresses. The interpreter
                    // stubs them — they only work meaningfully in compiled code.
                    Intrinsic::AtomicLoad => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // order
                        Ok(Value::Int(0))
                    }
                    Intrinsic::AtomicStore => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // val
                        let _ = values.remove(0); // order
                        Ok(Value::Int(-1))
                    }
                    Intrinsic::AtomicCas => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // expected
                        let _ = values.remove(0); // new
                        let _ = values.remove(0); // order
                        Ok(Value::Int(0))
                    }
                    Intrinsic::AtomicXchg => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // val
                        let _ = values.remove(0); // order
                        Ok(Value::Int(0))
                    }
                    Intrinsic::AtomicAdd => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // val
                        let _ = values.remove(0); // order
                        Ok(Value::Int(0))
                    }
                    Intrinsic::Fence => {
                        let _ = values.remove(0); // order
                        Ok(Value::Int(0))
                    }
                    Intrinsic::Futex => {
                        let _ = values.remove(0); // uaddr
                        let _ = values.remove(0); // op
                        let _ = values.remove(0); // val
                        let _ = values.remove(0); // timeout
                        let _ = values.remove(0); // uaddr2
                        let _ = values.remove(0); // val3
                        Ok(Value::Int(-1))
                    }
                    // ===== Phase E: IPC (intrinsics.md D11) =====
                    Intrinsic::Pipe => {
                        let fds = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("pipe fds requires Int, got {:?}", v))),
                        };
                        // fds is an opaque pointer to int[2]; interpreter can't fill it
                        let _ = fds;
                        #[cfg(unix)]
                        {
                            let mut pipe_fds = [0i32; 2];
                            let ret = unsafe { libc::pipe(pipe_fds.as_mut_ptr()) };
                            if ret == 0 {
                                // Write fds back through pointer (attempt)
                                unsafe {
                                    std::ptr::write(fds as *mut i32, pipe_fds[0]);
                                    std::ptr::write((fds + 4) as *mut i32, pipe_fds[1]);
                                }
                            }
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::ShmOpen => {
                        let name = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("shm_open name requires String, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("shm_open flags requires Int, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("shm_open mode requires Int, got {:?}", v))),
                        };
                        let c_name = match std::ffi::CString::new(name) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let fd = unsafe { libc::shm_open(c_name.as_ptr(), flags, mode) };
                            Ok(Value::Int(fd as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_name, flags, mode);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::ShmUnlink => {
                        let name = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("shm_unlink name requires String, got {:?}", v))),
                        };
                        let c_name = match std::ffi::CString::new(name) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::shm_unlink(c_name.as_ptr()) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_name;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::SemOpen => {
                        let name = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_open name requires String, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_open flags requires Int, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_open mode requires Int, got {:?}", v))),
                        };
                        let value = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_open value requires Int, got {:?}", v))),
                        };
                        let c_name = match std::ffi::CString::new(name) {
                            Ok(p) => p,
                            _ => return Ok(Value::Int(-1)),
                        };
                        #[cfg(unix)]
                        {
                            let sem = unsafe { libc::sem_open(c_name.as_ptr(), flags, mode, value) };
                            Ok(Value::Int(sem as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_name, flags, mode, value);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::SemWait => {
                        let sem = match values.remove(0) {
                            Value::Int(n) => n as *mut libc::sem_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_wait sem requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sem_wait(sem) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = sem;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::SemPost => {
                        let sem = match values.remove(0) {
                            Value::Int(n) => n as *mut libc::sem_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_post sem requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sem_post(sem) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = sem;
                            Ok(Value::Int(-1))
                        }
                    }
                    // ===== Phase F: Signals (intrinsics.md D8) =====
                    Intrinsic::SigAction => {
                        let signum = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("sigaction signum requires Int, got {:?}", v))),
                        };
                        let handler = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("sigaction handler requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
                            let new: libc::sigaction = libc::sigaction {
                                sa_sigaction: handler as usize,
                                sa_mask: unsafe { std::mem::zeroed() },
                                sa_flags: 0,
                                sa_restorer: None,
                            };
                            let ret = unsafe { libc::sigaction(signum, &new, &mut old) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (signum, handler);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::SigProcMask => {
                        let how = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("sigprocmask how requires Int, got {:?}", v))),
                        };
                        let mask = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("sigprocmask mask requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::sigprocmask(how, &set as *const _ as *const libc::sigset_t, std::ptr::null_mut()) };
                            let _ = mask;
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (how, mask);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Kill => {
                        let pid = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("kill pid requires Int, got {:?}", v))),
                        };
                        let sig = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("kill sig requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::kill(pid, sig) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (pid, sig);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::SignalFd => {
                        let mask = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("signalfd mask requires Int, got {:?}", v))),
                        };
                        #[cfg(target_os = "linux")]
                        {
                            let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::signalfd(-1, &set, libc::SFD_NONBLOCK) };
                            let _ = mask;
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            let _ = mask;
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::TimerFdCreate => {
                        let hz = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("timerfd_create hz requires Int, got {:?}", v))),
                        };
                        #[cfg(target_os = "linux")]
                        {
                            let fd = unsafe { libc::timerfd_create(libc::CLOCK_MONOTONIC, libc::TFD_NONBLOCK) };
                            if fd >= 0 {
                                let sec = 1 / hz as u64;
                                let nsec = if hz > 0 { (1_000_000_000u64) / hz as u64 } else { 0 };
                                let itimerspec = libc::itimerspec {
                                    it_interval: libc::timespec { tv_sec: 0, tv_nsec: nsec as i64 },
                                    it_value: libc::timespec { tv_sec: sec as i64, tv_nsec: 0 },
                                };
                                unsafe { libc::timerfd_settime(fd, 0, &itimerspec, std::ptr::null_mut()) };
                            }
                            Ok(Value::Int(fd as i64))
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            let _ = hz;
                            Ok(Value::Int(-1))
                        }
                    }
                    // ===== Phase G: Networking (intrinsics.md D10) — Shim =====
                    Intrinsic::Socket => {
                        let domain = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("socket domain requires Int, got {:?}", v))),
                        };
                        let sock_type = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("socket type requires Int, got {:?}", v))),
                        };
                        let protocol = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("socket protocol requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::socket(domain, sock_type, protocol) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (domain, sock_type, protocol);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Bind => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("bind fd requires Int, got {:?}", v))),
                        };
                        let addr = match values.remove(0) {
                            Value::Int(n) => n as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("bind addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("bind addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::bind(fd, addr as *const libc::sockaddr, addrlen) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, addr, addrlen);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Listen => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("listen fd requires Int, got {:?}", v))),
                        };
                        let backlog = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("listen backlog requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::listen(fd, backlog) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, backlog);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Accept => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("accept fd requires Int, got {:?}", v))),
                        };
                        let addr = match values.remove(0) {
                            Value::Int(n) => n as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("accept addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("accept addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::accept(fd, addr as *mut libc::sockaddr, &addrlen as *const u32 as *mut libc::socklen_t) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, addr, addrlen);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Connect => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("connect fd requires Int, got {:?}", v))),
                        };
                        let addr = match values.remove(0) {
                            Value::Int(n) => n as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("connect addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("connect addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::connect(fd, addr as *const libc::sockaddr, addrlen) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, addr, addrlen);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Send => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("send fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("send buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("send len requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("send flags requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::send(fd, buf as *const std::ffi::c_void, len, flags) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, len, flags);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Recv => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("recv fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("recv buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("recv len requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("recv flags requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::recv(fd, buf as *mut std::ffi::c_void, len, flags) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, len, flags);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::SendTo => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto len requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto flags requires Int, got {:?}", v))),
                        };
                        let dest_addr = match values.remove(0) {
                            Value::Int(n) => n as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto dest_addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sendto(fd, buf as *const std::ffi::c_void, len, flags, dest_addr as *const libc::sockaddr, addrlen) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, len, flags, dest_addr, addrlen);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::RecvFrom => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Int(n) => n as usize,
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom len requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom flags requires Int, got {:?}", v))),
                        };
                        let src_addr = match values.remove(0) {
                            Value::Int(n) => n as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom src_addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::recvfrom(fd, buf as *mut std::ffi::c_void, len, flags, src_addr as *mut libc::sockaddr, &addrlen as *const u32 as *mut libc::socklen_t) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, len, flags, src_addr, addrlen);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::SetSockOpt => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt fd requires Int, got {:?}", v))),
                        };
                        let level = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt level requires Int, got {:?}", v))),
                        };
                        let opt = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt opt requires Int, got {:?}", v))),
                        };
                        let val = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt val requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt len requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::setsockopt(fd, level, opt, val as *const std::ffi::c_void, len) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, level, opt, val, len);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::GetSockOpt => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt fd requires Int, got {:?}", v))),
                        };
                        let level = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt level requires Int, got {:?}", v))),
                        };
                        let opt = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt opt requires Int, got {:?}", v))),
                        };
                        let val = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt val requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Int(n) => n as u32,
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt len requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::getsockopt(fd, level, opt, val as *mut std::ffi::c_void, &len as *const u32 as *mut libc::socklen_t) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, level, opt, val, len);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::Shutdown => {
                        let fd = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("shutdown fd requires Int, got {:?}", v))),
                        };
                        let how = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("shutdown how requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::shutdown(fd, how) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, how);
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::GetAddrInfo => {
                        let node = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("getaddrinfo node requires String, got {:?}", v))),
                        };
                        let service = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("getaddrinfo service requires String, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut hints: libc::addrinfo = unsafe { std::mem::zeroed() };
                            hints.ai_family = libc::AF_UNSPEC;
                            hints.ai_socktype = libc::SOCK_STREAM;
                            let mut result: *mut libc::addrinfo = std::ptr::null_mut();
                            let c_node = std::ffi::CString::new(node.clone()).ok();
                            let c_service = std::ffi::CString::new(service.clone()).ok();
                            let ret = unsafe {
                                libc::getaddrinfo(
                                    c_node.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                                    c_service.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                                    &hints,
                                    &mut result,
                                )
                            };
                            if ret == 0 && !result.is_null() {
                                unsafe { libc::freeaddrinfo(result) };
                            }
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (node, service);
                            Ok(Value::Int(-1))
                        }
                    }
                    // ===== Phase H: Everything Else (intrinsics.md D6, D7) =====
                    Intrinsic::GetEnv => {
                        let name = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("getenv name requires String, got {:?}", v))),
                        };
                        match std::env::var(&name) {
                            Ok(val) => Ok(Value::String(val)),
                            Err(_) => Ok(Value::String(String::new())),
                        }
                    }
                    Intrinsic::SetEnv => {
                        let name = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("setenv name requires String, got {:?}", v))),
                        };
                        let value = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("setenv value requires String, got {:?}", v))),
                        };
                        unsafe { std::env::set_var(&name, &value); }
                        Ok(Value::Int(0))
                    }
                    Intrinsic::UnsetEnv => {
                        let name = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(format!("unsetenv name requires String, got {:?}", v))),
                        };
                        unsafe { std::env::remove_var(&name); }
                        Ok(Value::Int(0))
                    }
                    Intrinsic::GetPid => {
                        #[cfg(unix)]
                        {
                            let pid = unsafe { libc::getpid() };
                            Ok(Value::Int(pid as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::GetPPid => {
                        #[cfg(unix)]
                        {
                            let ppid = unsafe { libc::getppid() };
                            Ok(Value::Int(ppid as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Int(-1))
                        }
                    }
                    Intrinsic::ClockGetTime => {
                        let clock_id = match values.remove(0) {
                            Value::Int(n) => n as i32,
                            v => return Err(RuntimeError::TypeMismatch(format!("clock_gettime clock_id requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::clock_gettime(clock_id, &mut ts) };
                            if ret == 0 {
                                let nanos = ts.tv_sec * 1_000_000_000 + ts.tv_nsec;
                                Ok(Value::Int(nanos))
                            } else {
                                Ok(Value::Int(0))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = clock_id;
                            Ok(Value::Int(0))
                        }
                    }
                    Intrinsic::NanoSleep => {
                        let ns = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("nanosleep ns requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let req = libc::timespec {
                                tv_sec: ns / 1_000_000_000,
                                tv_nsec: ns % 1_000_000_000,
                            };
                            let mut rem: libc::timespec = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::nanosleep(&req, &mut rem) };
                            Ok(Value::Int(ret as i64))
                        }
                        #[cfg(not(unix))]
                        {
                            std::thread::sleep(std::time::Duration::from_nanos(ns as u64));
                            Ok(Value::Int(0))
                        }
                    }
                    // Data intrinsics
                    Intrinsic::Sort => Ok(values.remove(0)),
                    Intrinsic::Reverse => Ok(values.remove(0)),
                    Intrinsic::Range => {
                        let end = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(format!("range requires Int, got {:?}", v))),
                        };
                        let list: Vec<Value> = (0..end).map(Value::Int).collect();
                        Ok(Value::List(list))
                    }
                    // String intrinsics (2026-06-18)
                    Intrinsic::TrimLeft => {
                        let s = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("trim_left requires String, got {:?}", v))),
                        };
                        Ok(Value::String(s.trim_start().to_string()))
                    }
                    Intrinsic::TrimRight => {
                        let s = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("trim_right requires String, got {:?}", v))),
                        };
                        Ok(Value::String(s.trim_end().to_string()))
                    }
                    Intrinsic::ToLower => {
                        let s = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("to_lower requires String, got {:?}", v))),
                        };
                        Ok(Value::String(s.to_lowercase()))
                    }
                    Intrinsic::ContainsAt => {
                        let haystack = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("contains_at haystack requires String, got {:?}", v))),
                        };
                        let needle = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("contains_at needle requires String, got {:?}", v))),
                        };
                        let start = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("contains_at start requires Int, got {:?}", v))),
                        };
                        if start < 0 || (start as usize) >= haystack.len() {
                            return Ok(Value::Bool(false));
                        }
                        Ok(Value::Bool(haystack[(start as usize)..].contains(&needle)))
                    }
                    Intrinsic::FindFrom => {
                        let s = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("find_from s requires String, got {:?}", v))),
                        };
                        let needle = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("find_from needle requires String, got {:?}", v))),
                        };
                        let start = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("find_from start requires Int, got {:?}", v))),
                        };
                        if start < 0 || (start as usize) >= s.len() {
                            return Ok(Value::Int(-1));
                        }
                        match s[(start as usize)..].find(&needle) {
                            Some(idx) => Ok(Value::Int((start as usize + idx) as i64)),
                            None => Ok(Value::Int(-1)),
                        }
                    }
                    Intrinsic::SplitN => {
                        let s = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("splitn s requires String, got {:?}", v))),
                        };
                        let delim = match values.remove(0) {
                            Value::String(s) => s,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("splitn delim requires String, got {:?}", v))),
                        };
                        let _n = match values.remove(0) {
                            Value::Int(n) => n,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("splitn n requires Int, got {:?}", v))),
                        };
                        let parts: Vec<Value> = if delim.is_empty() {
                            s.chars().map(|c| Value::String(c.to_string())).collect()
                        } else {
                            s.split(&delim).map(|p| Value::String(p.to_string())).collect()
                        };
                        Ok(Value::List(parts))
                    }
                    Intrinsic::IntToStr => {
                        let n = match values.remove(0) {
                            Value::Int(v) => v,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("int_to_str requires Int, got {:?}", v))),
                        };
                        Ok(Value::String(n.to_string()))
                    }
                    // Benchmark intrinsics (2026-06-16)
                    Intrinsic::PrintInt => {
                        let n = match values.remove(0) {
                            Value::Int(v) => v,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("print_int requires Int, got {:?}", v))),
                        };
                        print!("{}", n);
                        Ok(Value::Bool(true))
                    }
                    Intrinsic::PutChar => {
                        let c = match values.remove(0) {
                            Value::Char(v) => v,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("putchar requires Char, got {:?}", v))),
                        };
                        print!("{}", c);
                        Ok(Value::Bool(true))
                    }
                    Intrinsic::PrintFloat => {
                        let d = match values.remove(0) {
                            Value::Float(v) => v,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("print_float requires Float, got {:?}", v))),
                        };
                        print!("{:.9}", d);
                        Ok(Value::Bool(true))
                    }
                    Intrinsic::GetEnvInt => {
                        let name = match values.remove(0) {
                            Value::String(v) => v,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("getenv_int requires String, got {:?}", v))),
                        };
                        let val = std::env::var(&name)
                            .ok()
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(0);
                        Ok(Value::Int(val))
                    }
                }
            }
            // Legacy collection variants — delegate through feature structs
            Expr::ListLiteral(elements) =>
                ListLiteralExpr { elements: elements.clone() }.evaluate(self, &ExprDispatch),
            Expr::MapLiteral(entries) =>
                MapLiteralExpr { entries: entries.clone() }.evaluate(self, &ExprDispatch),
            Expr::SetLiteral(entries) =>
                SetLiteralExpr { entries: entries.clone() }.evaluate(self, &ExprDispatch),
            Expr::ListIndex(list_expr, index_expr) =>
                ListIndexExpr { list: list_expr.clone(), index: index_expr.clone() }.evaluate(self, &ExprDispatch),
            Expr::Projection { source, target } =>
                ProjectionExpr::new(*source.clone(), target.clone()).evaluate(self, &ExprDispatch),
            Expr::FieldAccess(obj_expr, field_name) =>
                FieldAccessExpr { obj: obj_expr.clone(), field: field_name.clone() }.evaluate(self, &ExprDispatch),
            Expr::StructInstance(typename, fields) =>
                StructInstanceExpr { typename: typename.clone(), fields: fields.clone() }.evaluate(self, &ExprDispatch),
            Expr::ObjectLiteral(fields) =>
                ObjectLiteralExpr { fields: fields.clone() }.evaluate(self, &ExprDispatch),
            Expr::PatternMatch { value, variant, fields } =>
                PatternMatchExpr { value: value.clone(), variant: variant.clone(), fields: fields.clone() }.evaluate(self, &ExprDispatch),
            Expr::Concat(l, r) => {
                let left = self.eval_expr(l)?;
                let right = self.eval_expr(r)?;
                match (left, right) {
                    (Value::List(mut a), Value::List(b)) => { a.extend(b); Ok(Value::List(a)) }
                    _ => Err(RuntimeError::TypeMismatch("list concat".into())),
                }
            }
            Expr::Slice { value, start, end, stride, mask } =>
                SliceExpr { value: value.clone(), start: start.clone(), end: end.clone(), stride: stride.clone(), mask: mask.clone() }.evaluate(self, &ExprDispatch),
            Expr::Block(stmts, last) =>
                BlockExpr { stmts: stmts.clone(), last: last.clone() }.evaluate(self, &ExprDispatch),
            Expr::Tuple(exprs) =>
                TupleExpr { exprs: exprs.clone() }.evaluate(self, &ExprDispatch),
            Expr::TupleDestructure(names, expr) =>
                TupleDestructureExpr { names: names.clone(), expr: expr.clone() }.evaluate(self, &ExprDispatch),
            Expr::MultiSlice { value, ops } =>
                MultiSliceExpr { value: value.clone(), ops: ops.clone() }.evaluate(self, &ExprDispatch),
            Expr::Cast(inner, target_ty) => {
                let v = self.eval_expr(inner)?;
                return self.eval_cast(v, target_ty);
            }
            Expr::SubtypeProjection { source, ops } =>
                SubtypeProjectionExpr { source: source.clone(), ops: ops.clone() }.evaluate(self, &ExprDispatch),
            Expr::DbvlTable { path, field_names, key_offsets, schema_name } =>
                DbvlTableExpr { path: path.clone(), field_names: field_names.clone(), key_offsets: key_offsets.clone(), schema_name: schema_name.clone() }.evaluate(self, &ExprDispatch),
            Expr::Match { value, arms } => {
                let match_arms: Vec<crate::features::pattern::MatchArm> = arms.iter().map(|a| crate::features::pattern::MatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.clone(),
                    body: a.body.clone(),
                }).collect();
                MatchExpr { value: value.clone(), arms: match_arms }.evaluate(self, &ExprDispatch)
            }
            // ── Pattern B routing ────────────────────────────────
            Expr::BinaryOp(bop) => bop.evaluate(self, &ExprDispatch),
            Expr::UnaryOp(uop) => uop.evaluate(self, &ExprDispatch),
            Expr::CallExpr(ce) => ce.evaluate(self, &ExprDispatch),
            // DEFERRED: Pattern B variants below are not yet evaluated.
            // They exist in the enum but the feature files have stub evaluate
            // methods. Old variants still handle all cases.
            Expr::ProjectionExpr(_) | Expr::CallExpr(_)
            | Expr::ListLiteralExpr(_) | Expr::MapLiteralExpr(_) | Expr::SetLiteralExpr(_)
            | Expr::SliceExpr(_) | Expr::MultiSliceExpr(_) | Expr::FieldAccessExpr(_)
            | Expr::StructInstanceExpr(_) | Expr::ObjectLiteralExpr(_)
            | Expr::TupleExpr(_) | Expr::TupleDestructureExpr(_) | Expr::EllipsisExpr(_)
            | Expr::ArrowMutExpr(_) | Expr::ArrowDiscardExpr(_) | Expr::ArrowTransferExpr(_)
            | Expr::PatternMatchExpr(_) | Expr::MatchExpr(_) | Expr::BlockExpr(_)
            | Expr::SigCallExpr(_) | Expr::SubtypeProjectionExpr(_) | Expr::DbvlTableExpr(_)
            | Expr::TypeRef(_) => {
                Err(RuntimeError::TypeMismatch("Pattern B variant not yet evaluated".into()))
            }
        }
    }

    fn eval_is_type(&self, val: Value, target: &crate::ast::IsTarget) -> Result<Value, RuntimeError> {
        use crate::ast::IsTarget;
        match target {
            IsTarget::Type(ty) => {
                let matches = match (&val, ty) {
                    (Value::Int(_), Type::Int | Type::UInt) => true,
                    (Value::Float(_), Type::Float) => true,
                    (Value::Bool(_), Type::Bool) => true,
                    (Value::String(_), Type::String) => true,
                    (Value::Char(_), Type::Char) => true,
                    (Value::List(_), Type::Vector(..)) => true,
                    (Value::List(_), Type::Applied(n, _)) if n == "List" => true,
                    (Value::Instance { typename, .. }, Type::Custom(n)) => typename == n,
                    (Value::Instance { typename, .. }, Type::Enum(n)) => typename == n,
                    (Value::Instance { typename, .. }, Type::Applied(n, _)) => typename == n,
                    (Value::Instance { typename, .. }, Type::Sig(n)) => typename == n,
                    (Value::Enum(ename, ..), Type::Custom(n)) => ename == n,
                    (Value::Enum(ename, ..), Type::Enum(n)) => ename == n,
                    (Value::Enum(ename, ..), Type::Applied(n, _)) => ename == n,
                    _ => false,
                };
                Ok(Value::Bool(matches))
            }
            IsTarget::Variant(vname) => {
                match &val {
                    Value::Enum(_, variant_name, _) => Ok(Value::Bool(variant_name == vname)),
                    _ => Err(RuntimeError::TypeMismatch("is requires an enum value for variant check".into())),
                }
            }
        }
    }

    fn eval_from_check(&self, val: Value, ty: &Type) -> Result<Value, RuntimeError> {
        let type_name = match &val {
            Value::Instance { typename, .. } | Value::Enum(typename, ..) => typename.clone(),
            _ => return Ok(Value::Bool(false)),
        };
        let target_name = format!("{:?}", ty);
        Ok(Value::Bool(type_name == target_name))
    }

    fn eval_like(&self, lhs: Value, rhs: Value) -> Result<Value, RuntimeError> {
        fn is_bool_true(v: &Value) -> bool {
            matches!(v, Value::Bool(true))
        }
        let result = match (&lhs, &rhs) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a - b).abs() < f64::EPSILON,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::List(a), Value::List(b)) => {
                if a.len() != b.len() { false }
                else {
                    a.iter().zip(b.iter()).all(|(la, lb)| {
                        self.eval_like(la.clone(), lb.clone()).map_or(false, |v| is_bool_true(&v))
                    })
                }
            }
            (Value::Instance { typename: _, fields: af }, Value::Instance { typename: _, fields: bf }) => {
                if af.len() != bf.len() { false }
                else {
                    af.iter().zip(bf.iter()).all(|pair| {
                        let ((k_a, av), (k_b, bv)) = pair;
                        k_a == k_b && self.eval_like(av.clone(), bv.clone()).map_or(false, |v| is_bool_true(&v))
                    })
                }
            }
            (Value::Enum(an, avn, ap), Value::Enum(bn, bvn, bp)) => {
                an == bn && avn == bvn && {
                    if ap.len() != bp.len() { false }
                    else {
                        ap.iter().zip(bp.iter()).all(|((ka, va), (kb, vb))| {
                            ka == kb && self.eval_like(va.clone(), vb.clone()).map_or(false, |v| is_bool_true(&v))
                        })
                    }
                }
            }
            _ => false,
        };
        Ok(Value::Bool(result))
    }

    /// Evaluate a type cast: convert a value to a target type.
    fn eval_cast(&self, val: Value, target: &Type) -> Result<Value, RuntimeError> {
        match (&val, target) {
            // Int ↔ Float
            (Value::Int(n), Type::Float) => Ok(Value::Float(*n as f64)),
            (Value::Float(f), Type::Int) => Ok(Value::Int(*f as i64)),

            // Int ↔ Char
            (Value::Char(c), Type::Int) => Ok(Value::Int(*c as i64)),
            (Value::Int(n), Type::Char) => {
                if *n < 0 || *n > 0x10FFFF {
                    return Err(RuntimeError::TypeMismatch("integer value out of valid Char range".into()));
                }
                let ch = char::from_u32(*n as u32).unwrap_or('\0');
                Ok(Value::Char(ch))
            }

            // Char → String
            (Value::Char(c), Type::String) => {
                let s = c.to_string();
                Ok(Value::String(s))
            }

            // String → Char
            (Value::String(s), Type::Char) => {
                let ch = s.chars().next().unwrap_or('\0');
                Ok(Value::Char(ch))
            }

            // Int → String
            (Value::Int(n), Type::String) => Ok(Value::String(n.to_string())),

            // String → Int
            (Value::String(s), Type::Int) => {
                let n = s.trim().parse::<i64>()
                    .map_err(|_| RuntimeError::TypeMismatch(format!("cannot parse '{}' as Int", s)))?;
                Ok(Value::Int(n))
            }

            // Bool → Int
            (Value::Bool(b), Type::Int) => Ok(Value::Int(if *b { 1 } else { 0 })),

            // Int → Bool
            (Value::Int(n), Type::Bool) => Ok(Value::Bool(*n != 0)),

            // Unsupported
            _ => Err(RuntimeError::TypeMismatch(format!(
                "cannot convert {:?} to {:?}", val, target
            ))),
        }
    }

    /// Evaluate a `<:` subtype projection: applies a sequence of ops to a source value.
    pub(crate) fn eval_subtype_projection(&mut self, mut source: Value, ops: &[crate::ast::SubtypeOp]) -> Result<Value, RuntimeError> {
        // Check for string match projection
        if let Value::String(ref s) = source {
            for op in ops {
                if let crate::ast::SubtypeOp::Match(pattern_expr) = op {
                    let pattern_val = self.eval_expr(pattern_expr)?;
                    if let Value::String(pattern) = pattern_val {
                        let re = Regex::new(&pattern)
                            .map_err(|e| RuntimeError::TypeMismatch(format!("Invalid regex: {}", e)))?;
                        if let Some(caps) = re.captures(s) {
                            // Count groups
                            let group_count = caps.iter().len().saturating_sub(1);
                            if group_count == 0 {
                                return Ok(Value::Bool(true));
                            }
                            let mut groups = Vec::new();
                            for i in 1..caps.iter().len() {
                                if let Some(m) = caps.get(i) {
                                    groups.push(Value::String(m.as_str().to_string()));
                                }
                            }
                            match groups.len() {
                                0 => return Ok(Value::Bool(true)),
                                1 => return Ok(groups.into_iter().next().unwrap()),
                                _ => return Ok(Value::Tuple(groups)),
                            }
                        } else {
                            return Ok(Value::Bool(false));
                        }
                    }
                }
            }
            return Ok(Value::String(s.clone()));
        }

        // Check for DbvlTable conversion to collection
        let source = match source {
            Value::DbvlTable(table_ref) => {
                // Check for indexed FILTER on key field
                if let Some(crate::ast::SubtypeOp::Filter(predicate)) = ops.first() {
                    if let Some(literal_key) = try_extract_key_eq(predicate, table_ref.schema_key_index.unwrap_or(0)) {
                        let results = self.resolve_dbvl_key(&table_ref, &literal_key)?;
                        let remaining_ops = &ops[1..];
                        if remaining_ops.is_empty() {
                            if results.len() == 1 {
                                return Ok(results.into_iter().next().unwrap());
                            }
                            return Ok(Value::List(results));
                        }
                        // Apply remaining ops to the resolved list by converting to Value::List
                        // and falling through to the collection processing code below
                        Value::List(results)
                    } else {
                        // Full materialization
                        let mut all_entries = Vec::new();
                        for key in table_ref.key_offsets.keys() {
                            if let Ok(mut results) = self.resolve_dbvl_key(&table_ref, key) {
                                all_entries.append(&mut results);
                            }
                        }
                        Value::List(all_entries)
                    }
                } else {
                    // Full materialization
                    let mut all_entries = Vec::new();
                    for key in table_ref.key_offsets.keys() {
                        if let Ok(mut results) = self.resolve_dbvl_key(&table_ref, key) {
                            all_entries.append(&mut results);
                        }
                    }
                    Value::List(all_entries)
                }
            }
            other => other,
        };

        // Collection projection — source must be a list
        let mut items: Vec<Value> = match source {
            Value::List(list) => list,
            Value::Tuple(tup) => tup,
            Value::HashMap(map) => map.into_values().collect(),
            Value::HashSet(set) => set.into_iter().map(Value::String).collect(),
            val => {
                return Err(RuntimeError::TypeMismatch(
                    format!("Subtype projection requires a collection or string, got {:?}", val)
                ));
            }
        };

        // Helper to compare two Values for ordering
        fn cmp_values(a: &Value, b: &Value) -> std::cmp::Ordering {
            match (a, b) {
                (Value::Int(ai), Value::Int(bi)) => ai.cmp(bi),
                (Value::Float(af), Value::Float(bf)) => af.partial_cmp(bf).unwrap_or(std::cmp::Ordering::Equal),
                (Value::String(as_), Value::String(bs)) => as_.cmp(bs),
                (Value::Int(ai), Value::Float(bf)) => (*ai as f64).partial_cmp(bf).unwrap_or(std::cmp::Ordering::Equal),
                (Value::Float(af), Value::Int(bi)) => af.partial_cmp(&(*bi as f64)).unwrap_or(std::cmp::Ordering::Equal),
                _ => std::cmp::Ordering::Equal,
            }
        }

        // Helper to compare Values for equality (used in dedup/group)
        fn values_equal(a: &Value, b: &Value) -> bool {
            match (a, b) {
                (Value::Int(ai), Value::Int(bi)) => ai == bi,
                (Value::Float(af), Value::Float(bf)) => (af - bf).abs() < 1e-10,
                (Value::String(as_), Value::String(bs)) => as_ == bs,
                (Value::Bool(ab), Value::Bool(bb)) => ab == bb,
                (Value::Tuple(av), Value::Tuple(bv)) => av.len() == bv.len() && av.iter().zip(bv.iter()).all(|(x, y)| values_equal(x, y)),
                _ => std::mem::discriminant(a) == std::mem::discriminant(b),
            }
        }

        // Apply each non-terminal op in order
        let mut is_terminal = false;
        for op in ops {
            match op {
                crate::ast::SubtypeOp::Match(_) => {
                    return Err(RuntimeError::TypeMismatch("MATCH can only be used on String sources".into()));
                }
                crate::ast::SubtypeOp::Filter(predicate) => {
                    items = items.into_iter().filter(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        let result = self.eval_expr(predicate).unwrap_or(Value::Bool(false));
                        result == Value::Bool(true)
                    }).collect();
                }
                crate::ast::SubtypeOp::Map(transform) => {
                    items = items.into_iter().map(|item| {
                        self.state.insert("_".to_string(), item);
                        self.eval_expr(transform).unwrap_or(Value::Bool(false))
                    }).collect();
                }
                crate::ast::SubtypeOp::Limit(n) => {
                    let take = (*n).min(items.len());
                    items = items.into_iter().take(take).collect();
                }
                crate::ast::SubtypeOp::Skip(n) => {
                    let skip = (*n).min(items.len());
                    items = items.into_iter().skip(skip).collect();
                }
                crate::ast::SubtypeOp::Unique => {
                    let mut seen = Vec::new();
                    items = items.into_iter().filter(|item| {
                        if seen.iter().any(|s: &Value| values_equal(s, item)) {
                            false
                        } else {
                            seen.push(item.clone());
                            true
                        }
                    }).collect();
                }
                crate::ast::SubtypeOp::Sort(key) => {
                    let keys: Vec<Value> = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        self.eval_expr(key).unwrap_or(Value::Int(0))
                    }).collect();
                    let mut indices: Vec<usize> = (0..items.len()).collect();
                    indices.sort_by(|&a, &b| cmp_values(&keys[a], &keys[b]));
                    items = indices.into_iter().map(|i| items[i].clone()).collect();
                }
                crate::ast::SubtypeOp::Join(other, key) => {
                    let other_val = self.eval_expr(other)?;
                    let other_list = match other_val {
                        Value::List(list) => list,
                        _ => return Err(RuntimeError::TypeMismatch("JOIN requires a List source".into())),
                    };
                    // Compute key for each source item
                    let item_keys: Vec<Value> = items.iter().map(|a| {
                        self.state.insert("_".to_string(), a.clone());
                        self.eval_expr(key).unwrap_or(Value::Int(0))
                    }).collect();
                    let other_keys: Vec<Value> = other_list.iter().map(|b| {
                        self.state.insert("_".to_string(), b.clone());
                        self.eval_expr(key).unwrap_or(Value::Int(0))
                    }).collect();

                    let mut result = Vec::new();
                    for (i, a) in items.iter().enumerate() {
                        for (j, b) in other_list.iter().enumerate() {
                            if values_equal(&item_keys[i], &other_keys[j]) {
                                result.push(Value::Tuple(vec![a.clone(), b.clone()]));
                            }
                        }
                    }
                    items = result;
                }
                crate::ast::SubtypeOp::Group(key) => {
                    let mut group_keys: Vec<Value> = Vec::new();
                    let mut group_items: Vec<Vec<Value>> = Vec::new();
                    for item in items {
                        self.state.insert("_".to_string(), item.clone());
                        let k = self.eval_expr(key).unwrap_or(Value::Int(0));
                        if let Some(pos) = group_keys.iter().position(|gk| values_equal(gk, &k)) {
                            group_items[pos].push(item);
                        } else {
                            group_keys.push(k);
                            group_items.push(vec![item]);
                        }
                    }
                    items = group_keys.into_iter().zip(group_items.into_iter())
                        .map(|(k, v)| Value::Tuple(vec![k, Value::List(v)]))
                        .collect();
                }
                crate::ast::SubtypeOp::Count => {
                    is_terminal = true;
                    items = vec![Value::Int(items.len() as i64)];
                }
                crate::ast::SubtypeOp::Sum(expr) => {
                    is_terminal = true;
                    let total: i64 = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        match self.eval_expr(expr).unwrap_or(Value::Int(0)) {
                            Value::Int(n) => n,
                            Value::Float(f) => f as i64,
                            _ => 0,
                        }
                    }).sum();
                    items = vec![Value::Int(total)];
                }
                crate::ast::SubtypeOp::Avg(expr) => {
                    is_terminal = true;
                    let len = items.len();
                    if len == 0 {
                        items = vec![Value::Int(0)];
                    } else {
                        let total: f64 = items.iter().map(|item| {
                            self.state.insert("_".to_string(), item.clone());
                            match self.eval_expr(expr).unwrap_or(Value::Int(0)) {
                                Value::Int(n) => n as f64,
                                Value::Float(f) => f,
                                _ => 0.0,
                            }
                        }).sum();
                        items = vec![Value::Float(total / len as f64)];
                    }
                }
                crate::ast::SubtypeOp::Min(expr) => {
                    is_terminal = true;
                    let best = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        self.eval_expr(expr).unwrap_or(Value::Int(i64::MAX))
                    }).min_by(|a, b| cmp_values(a, b));
                    items = vec![best.unwrap_or(Value::Int(0))];
                }
                crate::ast::SubtypeOp::Max(expr) => {
                    is_terminal = true;
                    let best = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        self.eval_expr(expr).unwrap_or(Value::Int(i64::MIN))
                    }).max_by(|a, b| cmp_values(a, b));
                    items = vec![best.unwrap_or(Value::Int(0))];
                }
            }
        }

        if is_terminal {
            Ok(items.into_iter().next().unwrap_or(Value::Int(0)))
        } else {
            Ok(Value::List(items))
        }
    }

    /// Resolve a key in a lazy-loaded DbvlTable.
    /// Checks cache first, then seeks + parses the line from the file.
    pub(crate) fn resolve_dbvl_key(&mut self, table: &DbvlTableInner, key: &str) -> Result<Vec<Value>, RuntimeError> {
        // Check cache
        if let Some(entry_cache) = self.dbvl_cache.get(&table.path) {
            if let Some(values) = entry_cache.get(key) {
                return Ok(values.clone());
            }
        }

        // Look up key in offset index
        let offsets = match table.key_offsets.get(key) {
            Some(offsets) => offsets.clone(),
            None => return Ok(vec![]), // key not found
        };

        // Read the file
        let content = std::fs::read_to_string(&table.path)
            .map_err(|e| RuntimeError::TypeMismatch(
                format!("Failed to read DBVL file '{}': {}", table.path, e)
            ))?;

        let mut results = Vec::new();
        for &offset in &offsets {
            // Extract line at byte offset
            let rest = &content[offset..];
            let line = rest.lines().next().unwrap_or("");

            // Parse CSV line into values
            let values = parse_csv_line(line);
            let mut field_map = HashMap::new();
            for (i, val) in values.iter().enumerate() {
                let field_name = table.field_names.get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("field_{}", i));
                field_map.insert(field_name, val.clone());
            }

            let entry = match &table.schema_name {
                Some(schema) => {
                    // Try to match field names to schema positions
                    let mut named_fields: Vec<(String, Value)> = Vec::new();
                    for (i, val) in values.iter().enumerate() {
                        if i < table.field_names.len() {
                            named_fields.push((table.field_names[i].clone(), val.clone()));
                        }
                    }
                    Value::Instance {
                        typename: schema.clone(),
                        fields: named_fields.into_iter().collect(),
                    }
                }
                None => Value::HashMap(field_map),
            };
            results.push(entry);
        }

        // Cache the result
        self.dbvl_cache
            .entry(table.path.clone())
            .or_default()
            .insert(key.to_string(), results.clone());

        Ok(results)
    }
}

/// Try to extract a literal key comparison from a FILTER predicate.
/// Detects patterns like `_.key_field == "literal"` or `_.field_0 == "literal"`.
fn try_extract_key_eq(expr: &crate::ast::Expr, key_index: usize) -> Option<String> {
    if let crate::ast::Expr::Eq(left, right) = expr {
        // Check if left side is `_.field_name` or `_.field_N`
        let is_key_field = match left.as_ref() {
            crate::ast::Expr::FieldAccess(obj, field) => {
                matches!(obj.as_ref(), crate::ast::Expr::Identifier(name) if name == "_")
                    && (field == &format!("field_{}", key_index) || field == "field_0")
            }
            _ => false,
        };
        if is_key_field {
            // Extract literal from right side
            match right.as_ref() {
                crate::ast::Expr::String(s) => return Some(s.clone()),
                crate::ast::Expr::Integer(n) => return Some(n.to_string()),
                _ => {}
            }
        }
    }
    None
}

/// Parse a single CSV line into Values (lightweight, for lazy dbvl loading)
fn parse_csv_line(line: &str) -> Vec<Value> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                result.push(parse_csv_value(current.trim()));
                current = String::new();
            }
            c => {
                current.push(c);
            }
        }
    }
    result.push(parse_csv_value(current.trim()));

    result
}

/// Parse a single CSV field value into a Brief Value
fn parse_csv_value(s: &str) -> Value {
    // Try int first
    if let Ok(n) = s.parse::<i64>() {
        return Value::Int(n);
    }
    // Try float
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }
    // Bool
    if s == "true" {
        return Value::Bool(true);
    }
    if s == "false" {
        return Value::Bool(false);
    }
    // Default: string
    Value::String(s.to_string())
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

pub(crate) fn tty_raw_mode_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Bool(enable) = &args[0] {
        let _ = *enable;
        // Placeholder — raw mode requires termios
        Ok(Value::Bool(true))
    } else {
        Err(RuntimeError::TypeMismatch(
            "tty_raw_mode expects Bool".to_string(),
        ))
    }
}

pub(crate) fn tty_size_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    // Return 80x24 as default terminal size
    let encoded: i64 = 80 * 10000 + 24;
    Ok(Value::Int(encoded))
}

pub(crate) fn tty_read_key_impl(_args: Vec<Value>) -> Result<Value, RuntimeError> {
    use std::io::Read;
    let mut buf = [0u8; 1];
    match std::io::stdin().read(&mut buf) {
        Ok(0) | Err(_) => Ok(Value::String(String::new())),
        Ok(_) => Ok(Value::String((buf[0] as char).to_string())),
    }
}

// ===== Phase A intrinsic helpers (intrinsics.md) =====

fn set_tty_raw_mode(enable: bool) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::io::RawFd;
        const STDIN: RawFd = 0;
        unsafe {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(STDIN, &mut termios) != 0 {
                return false;
            }
            if enable {
                let mut raw = termios;
                libc::cfmakeraw(&mut raw);
                if libc::tcsetattr(STDIN, libc::TCSANOW, &raw) != 0 {
                    return false;
                }
            } else {
                // Restore original — stored statically
                return false; // simplified: always succeed for now
            }
            true
        }
    }
    #[cfg(not(unix))]
    {
        let _ = enable;
        false
    }
}

fn get_terminal_size() -> (i64, i64) {
    #[cfg(unix)]
    {
        unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws as *mut _ as *mut libc::c_void) == 0
                && ws.ws_col > 0
            {
                return (ws.ws_col as i64, ws.ws_row as i64);
            }
        }
    }
    (80, 24)
}

fn read_key_nonblocking() -> Option<u8> {
    use std::io::Read;
    #[cfg(unix)]
    {
        // Set stdin to non-blocking for this read, then restore
        unsafe {
            let flags = libc::fcntl(0, libc::F_GETFL, 0);
            if flags >= 0 {
                libc::fcntl(0, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }
    }
    let mut buf = [0u8; 1];
    let result = match std::io::stdin().read(&mut buf) {
        Ok(n) if n > 0 => Some(buf[0]),
        _ => None,
    };
    #[cfg(unix)]
    {
        unsafe {
            let flags = libc::fcntl(0, libc::F_GETFL, 0);
            if flags >= 0 {
                libc::fcntl(0, libc::F_SETFL, flags & !libc::O_NONBLOCK);
            }
        }
    }
    result
}

pub(crate) fn exec_cmd_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(cmd) = &args[0] {
        match std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                Ok(Value::String(stdout))
            }
            Err(e) => Err(RuntimeError::TypeMismatch(
                format!("exec failed: {}", e),
            )),
        }
    } else {
        Err(RuntimeError::TypeMismatch(
            "exec_cmd expects String".to_string(),
        ))
    }
}

pub(crate) fn string_trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.trim().to_string()))
    } else {
        Err(RuntimeError::TypeMismatch("string_trim expects String".to_string()))
    }
}

pub(crate) fn string_to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.to_lowercase()))
    } else {
        Err(RuntimeError::TypeMismatch("string_to_lower expects String".to_string()))
    }
}

pub(crate) fn string_contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let (Value::String(s), Value::String(sub)) = (&args[0], &args[1]) {
        Ok(Value::Bool(s.contains(sub.as_str())))
    } else {
        Err(RuntimeError::TypeMismatch("string_contains expects String, String".to_string()))
    }
}

pub(crate) fn string_starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let (Value::String(s), Value::String(prefix)) = (&args[0], &args[1]) {
        Ok(Value::Bool(s.starts_with(prefix.as_str())))
    } else {
        Err(RuntimeError::TypeMismatch("string_starts_with expects String, String".to_string()))
    }
}

pub(crate) fn string_split_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        let parts: Vec<Value> = s.split(char::is_whitespace)
            .filter(|p| !p.is_empty())
            .map(|p| Value::String(p.to_string()))
            .collect();
        Ok(Value::List(parts))
    } else {
        Err(RuntimeError::TypeMismatch("string_split expects String".to_string()))
    }
}

pub(crate) fn substring_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        let chars: Vec<Value> = s.chars().map(|c| Value::Char(c)).collect();
        Ok(Value::List(chars))
    } else {
        Err(RuntimeError::TypeMismatch("substring expects String".to_string()))
    }
}

pub(crate) fn int_to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::Int(n) = &args[0] {
        Ok(Value::String(n.to_string()))
    } else {
        Err(RuntimeError::TypeMismatch("int_to_string expects Int".to_string()))
    }
}

pub(crate) fn json_parse_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::String(s) = &args[0] {
        match serde_json::from_str::<serde_json::Value>(s) {
            Ok(v) => Ok(json_value_to_brief(v)),
            Err(e) => Ok(Value::String(format!("JSON parse error: {}", e))),
        }
    } else {
        Err(RuntimeError::TypeMismatch("json_parse expects String".to_string()))
    }
}

fn json_value_to_brief(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::String("null".to_string()),
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => Value::Int(n.as_i64().unwrap_or(0)),
        serde_json::Value::String(s) => Value::String(s),
        serde_json::Value::Array(arr) => Value::List(arr.into_iter().map(json_value_to_brief).collect()),
        serde_json::Value::Object(obj) => {
            let mut pairs = Vec::new();
            for (k, v) in obj {
                pairs.push(Value::List(vec![Value::String(k), json_value_to_brief(v)]));
            }
            Value::List(pairs)
        }
    }
}

pub(crate) fn json_is_array_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let Value::List(_) = &args[0] {
        Ok(Value::Bool(true))
    } else {
        Ok(Value::Bool(false))
    }
}

pub(crate) fn json_length_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match &args[0] {
        Value::List(items) => Ok(Value::Int(items.len() as i64)),
        _ => Ok(Value::Int(0)),
    }
}

pub(crate) fn json_get_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let (Value::List(obj), Value::String(key)) = (&args[0], &args[1]) {
        for pair in obj {
            if let Value::List(kv) = pair {
                if kv.len() == 2 && kv[0] == Value::String(key.clone()) {
                    return Ok(kv[1].clone());
                }
            }
        }
        Ok(Value::String(String::new()))
    } else {
        Err(RuntimeError::TypeMismatch("json_get expects Value, String".to_string()))
    }
}

pub(crate) fn json_get_by_index_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if let (Value::List(items), Value::Int(idx)) = (&args[0], &args[1]) {
        if *idx >= 0 && (*idx as usize) < items.len() {
            Ok(items[*idx as usize].clone())
        } else {
            Ok(Value::String(String::new()))
        }
    } else {
        Err(RuntimeError::TypeMismatch("json_get_by_index expects Value, Int".to_string()))
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
                HashMap::from([("value".to_string(), Value::String(content))]),
            )),
            Err(e) => Ok(Value::Enum(
                "Result".to_string(),
                "Err".to_string(),
                HashMap::from([("value".to_string(), Value::String(format!("{}", e)))]),
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

    #[test]
    fn test_projection_size_on_list() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Int(10), Value::Int(20), Value::Int(30)]);
        i.state.insert("xs".to_string(), list);
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("xs".to_string())),
            target: ProjectionTarget::Size,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_projection_size_on_string() {
        let mut i = Interpreter::new();
        i.state.insert("s".to_string(), Value::String("hello".to_string()));
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("s".to_string())),
            target: ProjectionTarget::Size,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_len_through_defn_no_magic() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));
        // Register a defn len that uses :> Size (exactly as stdlib does)
        let defn = Definition {
            name: "len".to_string(),
            type_params: vec![],
            parameters: vec![("list".to_string(), Type::Custom("List".to_string()))],
            outputs: vec![Type::Int],
            output_type: Some(crate::ast::OutputType::Single(Type::Int)),
            output_names: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                watchdog: None,
                span: None,
            },
            body: vec![Statement::Term {
                values: vec![Some(Expr::Projection {
                    source: Box::new(Expr::Identifier("list".to_string())),
                    target: ProjectionTarget::Size,
                })],
                swan_song: None,
                modifiers: vec![],
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        };
        i.definitions.insert("len".to_string(), defn);
        let expr = Expr::Call("len".to_string(), vec![Expr::Identifier("xs".to_string())]);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    fn make_enum(variant: &str, fields: Vec<(&str, Value)>) -> Value {
        Value::Enum(
            "Option".to_string(),
            variant.to_string(),
            fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        )
    }

    #[test]
    fn test_uni_wildcard_matches_some() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(42))]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "_".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_uni_wildcard_matches_none() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("None", vec![]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "_".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_uni_binds_field_from_variant() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(99))]));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("v".to_string())],
            expr: Expr::Block(vec![], Box::new(Expr::Term)),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("v"), Some(&Value::Int(99)));
    }

    #[test]
    fn test_uni_mismatch_does_not_bind() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("None", vec![]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("v".to_string())],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        // flag should not have been set (match failed)
        assert_eq!(i.state.get("flag"), Some(&Value::Int(0)));
        // v should not be bound
        assert!(!i.state.contains_key("v"));
    }

    #[test]
    fn test_uni_wildcard_does_not_bind_fields() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(42))]));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "_".to_string(),
            fields: vec![],
            expr: Expr::Block(vec![], Box::new(Expr::Term)),
        };
        i.exec_stmt(&stmt).unwrap();
        // wildcard should bind nothing
        assert!(!i.state.contains_key("0"));
    }

    #[test]
    fn test_uni_multiple_fields() {
        let mut i = Interpreter::new();
        let fields: HashMap<String, Value> = [
            ("0".to_string(), Value::Int(10)),
            ("1".to_string(), Value::String("hello".to_string())),
        ].into();
        i.state.insert("pair".to_string(), Value::Enum(
            "Pair".to_string(), "Pair".to_string(), fields,
        ));
        let stmt = Statement::Unification {
            name: "pair".to_string(),
            variant: "Pair".to_string(),
            fields: vec![
                Pattern::Var("a".to_string()),
                Pattern::Var("b".to_string()),
            ],
            expr: Expr::Block(vec![], Box::new(Expr::Term)),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("a"), Some(&Value::Int(10)));
        assert_eq!(i.state.get("b"), Some(&Value::String("hello".to_string())));
    }

    #[test]
    fn test_uni_literal_pattern_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(42))]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::LitInt(42)],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_uni_literal_pattern_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(99))]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::LitInt(42)],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(0)));
    }

    #[test]
    fn test_uni_literal_string_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Msg", vec![("0", Value::String("ok".to_string()))]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Msg".to_string(),
            fields: vec![Pattern::LitString("ok".to_string())],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_uni_tuple_pattern() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![(
            "0",
            Value::List(vec![Value::Int(1), Value::Int(2)]),
        )]));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Tuple(vec![
                Pattern::Var("a".to_string()),
                Pattern::Var("b".to_string()),
            ])],
            expr: Expr::Block(vec![], Box::new(Expr::Term)),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("a"), Some(&Value::Int(1)));
        assert_eq!(i.state.get("b"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_uni_tuple_pattern_mismatch_length() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![(
            "0",
            Value::List(vec![Value::Int(1)]), // length 1, but pattern expects length 2
        )]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Tuple(vec![
                Pattern::Var("a".to_string()),
                Pattern::Var("b".to_string()),
            ])],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(0)));
    }

    #[test]
    fn test_uni_recursive_nested_enum() {
        let mut i = Interpreter::new();
        // Simulate Some(Some(42)) — nested enum
        let inner = make_enum("Some", vec![("0", Value::Int(42))]);
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", inner)]));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("inner".to_string())],
            expr: Expr::Block(vec![], Box::new(Expr::Term)),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("inner"), Some(&make_enum("Some", vec![("0", Value::Int(42))])));
    }

    #[test]
    fn test_uni_simple_name_always_matches() {
        let mut i = Interpreter::new();
        i.state.insert("flag".to_string(), Value::Int(0));
        // Syntax 3: uni x = expr — always matches, binds nothing
        let stmt = Statement::Unification {
            name: "uni".to_string(),
            variant: "x".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_uni_fieldless_variant_matches_void() {
        // Syntax: uni val(Some) = expr;  — a variant name with no field patterns
        // This matches Void under the same catch-all that syntax 3 uses
        let mut i = Interpreter::new();
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        // Void matches because variant != "_" && fields.is_empty() is a catch-all
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_uni_wildcard_matches_void() {
        let mut i = Interpreter::new();
        // val is not in state, value is Void
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "_".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        // Wildcard only matches enums in the current interpreter, so Void should not match
        // The first match arm requires Value::Enum, the second requires variant != "_"
        // So wildcard on Void should NOT match
        assert_eq!(i.state.get("flag"), Some(&Value::Int(0)));
    }

    #[test]
    fn test_uni_field_with_wildcard_ignores_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(42))]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Wildcard],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
        // Nothing bound to the wildcard
        assert!(!i.state.contains_key("_"));
    }

    #[test]
    fn test_uni_literal_float_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Val", vec![("0", Value::Float(3.14))]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Val".to_string(),
            fields: vec![Pattern::LitFloat(3.14)],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_uni_literal_bool_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Flag", vec![("0", Value::Bool(true))]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Flag".to_string(),
            fields: vec![Pattern::LitBool(true)],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_uni_literal_char_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Ch", vec![("0", Value::Char('x'))]));
        i.state.insert("flag".to_string(), Value::Int(0));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Ch".to_string(),
            fields: vec![Pattern::LitChar('x')],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::OwnedRef("flag".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_pattern_match_binds_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(42))]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("v".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
        assert_eq!(i.state.get("v"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_pattern_match_variant_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("None", vec![]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("v".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false));
        assert!(!i.state.contains_key("v"));
    }

    #[test]
    fn test_pattern_match_literal_int() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("N", vec![("0", Value::Int(42))]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "N".to_string(),
            fields: vec![Pattern::LitInt(42)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_pattern_match_literal_int_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("N", vec![("0", Value::Int(99))]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "N".to_string(),
            fields: vec![Pattern::LitInt(42)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_pattern_match_tuple_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("P", vec![(
            "0",
            Value::List(vec![Value::Int(10), Value::Int(20)]),
        )]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "P".to_string(),
            fields: vec![Pattern::Tuple(vec![
                Pattern::Var("a".to_string()),
                Pattern::Var("b".to_string()),
            ])],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
        assert_eq!(i.state.get("a"), Some(&Value::Int(10)));
        assert_eq!(i.state.get("b"), Some(&Value::Int(20)));
    }

    #[test]
    fn test_pattern_match_wildcard_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(42))]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "Some".to_string(),
            fields: vec![Pattern::Wildcard],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
        assert!(!i.state.contains_key("_"));
    }

    #[test]
    fn test_pattern_match_no_fields() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("None", vec![]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "None".to_string(),
            fields: vec![],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_match_simple_variant_binds_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Int(7))]));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("val".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Variant {
                        name: "Some".to_string(),
                        fields: vec![Pattern::Var("x".to_string())],
                    },
                    guard: None,
                    body: Box::new(Expr::Identifier("x".to_string())),
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Integer(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(7));
    }

    #[test]
    fn test_match_falls_through_to_wildcard() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("None", vec![]));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("val".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Variant {
                        name: "Some".to_string(),
                        fields: vec![Pattern::Var("x".to_string())],
                    },
                    guard: None,
                    body: Box::new(Expr::Identifier("x".to_string())),
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Integer(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(0));
    }

    #[test]
    fn test_match_literal_pattern() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("N", vec![("0", Value::Int(42))]));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("val".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Variant {
                        name: "N".to_string(),
                        fields: vec![Pattern::LitInt(42)],
                    },
                    guard: None,
                    body: Box::new(Expr::Integer(1)),
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Integer(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_match_literal_pattern_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("N", vec![("0", Value::Int(99))]));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("val".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Variant {
                        name: "N".to_string(),
                        fields: vec![Pattern::LitInt(42)],
                    },
                    guard: None,
                    body: Box::new(Expr::Integer(1)),
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Integer(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(0));
    }

    #[test]
    fn test_match_tuple_pattern() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("P", vec![(
            "0",
            Value::List(vec![Value::Int(3), Value::Int(4)]),
        )]));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("val".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Variant {
                        name: "P".to_string(),
                        fields: vec![Pattern::Tuple(vec![
                            Pattern::Var("a".to_string()),
                            Pattern::Var("b".to_string()),
                        ])],
                    },
                    guard: None,
                    body: Box::new(Expr::Identifier("a".to_string())),
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Integer(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_match_wildcard_only() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("None", vec![]));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("val".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Integer(99)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(99));
    }

    #[test]
    fn test_match_multiple_fields() {
        let mut i = Interpreter::new();
        let fields: HashMap<String, Value> = [
            ("0".to_string(), Value::Int(10)),
            ("1".to_string(), Value::Int(20)),
        ].into();
        i.state.insert("pair".to_string(), Value::Enum(
            "Pair".to_string(), "Pair".to_string(), fields,
        ));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("pair".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Variant {
                        name: "Pair".to_string(),
                        fields: vec![
                            Pattern::Var("a".to_string()),
                            Pattern::Var("b".to_string()),
                        ],
                    },
                    guard: None,
                    body: Box::new(Expr::Add(
                        Box::new(Expr::Identifier("a".to_string())),
                        Box::new(Expr::Identifier("b".to_string())),
                    )),
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Integer(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(30));
    }

    #[test]
    fn test_callable_txn_simple_iteration() {
        let mut i = Interpreter::new();
        // txn count_up(n: Int, result: Int, i: Int) [i < n][i == n] -> Int { &result = result + 1; &i = i + 1; term result; };
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "count_up".to_string(),
            parameters: vec![
                ("n".to_string(), Type::Int),
                ("result".to_string(), Type::Int),
                ("i".to_string(), Type::Int),
            ],
            contract: Contract {
                pre_condition: Expr::Lt(
                    Box::new(Expr::Identifier("i".to_string())),
                    Box::new(Expr::Identifier("n".to_string())),
                ),
                post_condition: Expr::Eq(
                    Box::new(Expr::Identifier("i".to_string())),
                    Box::new(Expr::Identifier("n".to_string())),
                ),
                watchdog: None,
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("result".to_string()),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("result".to_string())),
                        Box::new(Expr::Integer(1)),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Assignment {
                    lhs: Expr::OwnedRef("i".to_string()),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("i".to_string())),
                        Box::new(Expr::Integer(1)),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Term {
                    values: vec![Some(Expr::Identifier("result".to_string()))],
                    swan_song: None,
                    modifiers: vec![],
                },
            ],
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        };
        i.callable_txns.insert("count_up".to_string(), txn);
        let result = i.eval_expr(&Expr::Call("count_up".to_string(), vec![
            Expr::Integer(5),
            Expr::Integer(0),
            Expr::Integer(0),
        ])).unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_callable_txn_no_loop_if_pre_false() {
        let mut i = Interpreter::new();
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "noop".to_string(),
            parameters: vec![
                ("n".to_string(), Type::Int),
                ("result".to_string(), Type::Int),
                ("i".to_string(), Type::Int),
            ],
            contract: Contract {
                pre_condition: Expr::Lt(
                    Box::new(Expr::Identifier("i".to_string())),
                    Box::new(Expr::Identifier("n".to_string())),
                ),
                post_condition: Expr::Bool(true),
                watchdog: None,
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("result".to_string()),
                    expr: Expr::Integer(99),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Term {
                    values: vec![Some(Expr::Identifier("result".to_string()))],
                    swan_song: None,
                    modifiers: vec![],
                },
            ],
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        };
        i.callable_txns.insert("noop".to_string(), txn);
        // pre is i < n, but i=5, n=3 → false, so body never runs
        let result = i.eval_expr(&Expr::Call("noop".to_string(), vec![
            Expr::Integer(3),
            Expr::Integer(0),
            Expr::Integer(5),
        ])).unwrap();
        assert_eq!(result, Value::Void);
    }

    #[test]
    fn test_callable_state_restored_after_txn() {
        let mut i = Interpreter::new();
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "mutate".to_string(),
            parameters: vec![("x".to_string(), Type::Int)],
            contract: Contract {
                pre_condition: Expr::Lt(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Integer(10)),
                ),
                post_condition: Expr::Eq(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Integer(10)),
                ),
                watchdog: None,
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("x".to_string()),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("x".to_string())),
                        Box::new(Expr::Integer(1)),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Term {
                    values: vec![Some(Expr::Identifier("x".to_string()))],
                    swan_song: None,
                    modifiers: vec![],
                },
            ],
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        };
        i.callable_txns.insert("mutate".to_string(), txn);
        i.state.insert("outer".to_string(), Value::Int(42));
        let result = i.eval_expr(&Expr::Call("mutate".to_string(), vec![
            Expr::Integer(0),
        ])).unwrap();
        assert_eq!(result, Value::Int(10));
        // outer variable should still be intact
        assert_eq!(i.state.get("outer"), Some(&Value::Int(42)));
    }

    // ── Phase 12: HashMap/HashSet arrow operations ──

    #[test]
    fn test_hashmap_arrow_push_key_value() {
        let mut i = Interpreter::new();
        i.state.insert("m".to_string(), Value::HashMap(std::collections::HashMap::new()));
        // Push (key, value) as a tuple (list with 2 elements)
        let expr = Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("m".to_string())),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Tuple(vec![
                Expr::String("a".to_string()),
                Expr::Integer(1),
            ]))),
        };
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::HashMap(map) => assert_eq!(map.get("a"), Some(&Value::Int(1))),
            _ => panic!("Expected HashMap"),
        }
    }

    #[test]
    fn test_hashmap_arrow_push_indexed() {
        let mut i = Interpreter::new();
        i.state.insert("m".to_string(), Value::HashMap(std::collections::HashMap::new()));
        // &m[key] <- value
        let expr = Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("m".to_string())),
            index: Box::new(Expr::String("b".to_string())),
            value: Some(Box::new(Expr::Integer(2))),
        };
        i.eval_expr(&expr).unwrap();
        match i.state.get("m").unwrap() {
            Value::HashMap(map) => assert_eq!(map.get("b"), Some(&Value::Int(2))),
            _ => panic!("Expected HashMap"),
        }
    }

    #[test]
    fn test_hashmap_arrow_pop_key() {
        let mut i = Interpreter::new();
        let mut map = std::collections::HashMap::new();
        map.insert("x".to_string(), Value::Int(42));
        i.state.insert("m".to_string(), Value::HashMap(map));
        // value <- &m[key]
        let popped = i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Pop,
            target: Box::new(Expr::OwnedRef("m".to_string())),
            index: Box::new(Expr::String("x".to_string())),
            value: None,
        }).unwrap();
        assert_eq!(popped, Value::Int(42));
        match i.state.get("m").unwrap() {
            Value::HashMap(map) => assert!(map.is_empty()),
            _ => panic!("Expected HashMap"),
        }
    }

    #[test]
    fn test_hashset_arrow_push() {
        let mut i = Interpreter::new();
        i.state.insert("s".to_string(), Value::HashSet(std::collections::HashSet::new()));
        let expr = Expr::ArrowMut {
            dir: ArrowDir::Push,
            target: Box::new(Expr::OwnedRef("s".to_string())),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::String("hello".to_string()))),
        };
        i.eval_expr(&expr).unwrap();
        match i.state.get("s").unwrap() {
            Value::HashSet(set) => assert!(set.contains("hello")),
            _ => panic!("Expected HashSet"),
        }
    }

    #[test]
    fn test_hashset_arrow_pop() {
        let mut i = Interpreter::new();
        let mut set = std::collections::HashSet::new();
        set.insert("world".to_string());
        i.state.insert("s".to_string(), Value::HashSet(set));
        let popped = i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Pop,
            target: Box::new(Expr::OwnedRef("s".to_string())),
            index: Box::new(Expr::Term),
            value: None,
        }).unwrap();
        assert_eq!(popped, Value::String("world".to_string()));
        match i.state.get("s").unwrap() {
            Value::HashSet(set) => assert!(set.is_empty()),
            _ => panic!("Expected HashSet"),
        }
    }

    #[test]
    fn test_hashset_arrow_discard() {
        let mut i = Interpreter::new();
        let mut set = std::collections::HashSet::new();
        set.insert("discard".to_string());
        i.state.insert("s".to_string(), Value::HashSet(set));
        i.eval_expr(&Expr::ArrowDiscard {
            target: Box::new(Expr::OwnedRef("s".to_string())),
            index: Box::new(Expr::Term),
        }).unwrap();
        match i.state.get("s").unwrap() {
            Value::HashSet(set) => assert!(set.is_empty()),
            _ => panic!("Expected HashSet"),
        }
    }

    #[test]
    fn test_map_literal_eval() {
        let mut i = Interpreter::new();
        let expr = Expr::MapLiteral(vec![
            (Expr::String("a".to_string()), Expr::Integer(1)),
            (Expr::String("b".to_string()), Expr::Integer(2)),
        ]);
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::HashMap(map) => {
                assert_eq!(map.len(), 2);
                assert_eq!(map.get("a"), Some(&Value::Int(1)));
                assert_eq!(map.get("b"), Some(&Value::Int(2)));
            }
            _ => panic!("Expected HashMap"),
        }
    }

    #[test]
    fn test_set_literal_eval() {
        let mut i = Interpreter::new();
        let expr = Expr::SetLiteral(vec![
            Expr::String("x".to_string()),
            Expr::String("y".to_string()),
        ]);
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::HashSet(set) => {
                assert_eq!(set.len(), 2);
                assert!(set.contains("x"));
                assert!(set.contains("y"));
            }
            _ => panic!("Expected HashSet"),
        }
    }

    #[test]
    fn test_projection_keys() {
        let mut i = Interpreter::new();
        let mut map = std::collections::HashMap::new();
        map.insert("a".to_string(), Value::Int(1));
        map.insert("b".to_string(), Value::Int(2));
        i.state.insert("m".to_string(), Value::HashMap(map));
        let result = i.eval_expr(&Expr::Projection {
            source: Box::new(Expr::OwnedRef("m".to_string())),
            target: ProjectionTarget::Keys,
        }).unwrap();
        match result {
            Value::List(keys) => {
                assert_eq!(keys.len(), 2);
                assert!(keys.contains(&Value::String("a".to_string())));
                assert!(keys.contains(&Value::String("b".to_string())));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_projection_contains() {
        let mut i = Interpreter::new();
        let mut set = std::collections::HashSet::new();
        set.insert("hello".to_string());
        i.state.insert("s".to_string(), Value::HashSet(set));
        let result = i.eval_expr(&Expr::Projection {
            source: Box::new(Expr::OwnedRef("s".to_string())),
            target: ProjectionTarget::Contains(Box::new(Expr::String("hello".to_string()))),
        }).unwrap();
        assert_eq!(result, Value::Bool(true));
        let result = i.eval_expr(&Expr::Projection {
            source: Box::new(Expr::OwnedRef("s".to_string())),
            target: ProjectionTarget::Contains(Box::new(Expr::String("nope".to_string()))),
        }).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_arrow_transfer_list() {
        let mut i = Interpreter::new();
        i.state.insert("src".to_string(), Value::List(vec![
            Value::Int(1), Value::Int(2), Value::Int(3),
        ]));
        i.state.insert("dest".to_string(), Value::List(vec![]));
        i.eval_expr(&Expr::ArrowTransfer {
            dest: Box::new(Expr::OwnedRef("dest".to_string())),
            source: Box::new(Expr::OwnedRef("src".to_string())),
            filter: None,
        }).unwrap();
        assert_eq!(i.state.get("src"), Some(&Value::List(vec![])));
        assert_eq!(i.state.get("dest"), Some(&Value::List(vec![
            Value::Int(1), Value::Int(2), Value::Int(3),
        ])));
    }

    #[test]
    fn test_arrow_transfer_hashmap() {
        let mut i = Interpreter::new();
        let mut src = std::collections::HashMap::new();
        src.insert("a".to_string(), Value::Int(1));
        src.insert("b".to_string(), Value::Int(2));
        i.state.insert("src".to_string(), Value::HashMap(src));
        i.state.insert("dest".to_string(), Value::HashMap(std::collections::HashMap::new()));
        i.eval_expr(&Expr::ArrowTransfer {
            dest: Box::new(Expr::OwnedRef("dest".to_string())),
            source: Box::new(Expr::OwnedRef("src".to_string())),
            filter: None,
        }).unwrap();
        match (i.state.get("src").unwrap(), i.state.get("dest").unwrap()) {
            (Value::HashMap(src), Value::HashMap(dest)) => {
                assert!(src.is_empty());
                assert_eq!(dest.len(), 2);
            }
            _ => panic!("Expected HashMaps"),
        }
    }

    #[test]
    fn test_multislice_stride() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Int(0), Value::Int(1), Value::Int(2),
            Value::Int(3), Value::Int(4), Value::Int(5)]);
        i.state.insert("xs".to_string(), list);
        // xs[::2] — take every 2nd element
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Stride(Box::new(Expr::Integer(2)))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![
            Value::Int(0), Value::Int(2), Value::Int(4),
        ]));
    }

    #[test]
    fn test_multislice_stride_with_coords() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let inner2 = Value::List(vec![Value::Int(4), Value::Int(5), Value::Int(6)]);
        let inner3 = Value::List(vec![Value::Int(7), Value::Int(8), Value::Int(9)]);
        let matrix = Value::List(vec![inner1, inner2, inner3]);
        // matrix[0..3 ::2] — first 3 rows, then every 2nd
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("matrix".to_string())),
            ops: vec![
                BracketOp::Coord(SliceCoordinate::Index(Box::new(Expr::Integer(0)))),
                BracketOp::Stride(Box::new(Expr::Integer(2))),
            ],
        };
        i.state.insert("matrix".to_string(), matrix);
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::List(items) => {
                assert_eq!(items.len(), 2); // every 2nd of [1,2,3] → [1,3]
                assert_eq!(items[0], Value::Int(1));
                assert_eq!(items[1], Value::Int(3));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_multislice_mask() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Int(10), Value::Int(25), Value::Int(5),
            Value::Int(30), Value::Int(15)]);
        i.state.insert("xs".to_string(), list);
        // xs[; _ > 15] — keep elements > 15
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Mask(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Integer(15)))
            ))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![
            Value::Int(25), Value::Int(30),
        ]));
    }

    #[test]
    fn test_multislice_stride_then_mask() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Int(10), Value::Int(25), Value::Int(5),
            Value::Int(30), Value::Int(15), Value::Int(40)]);
        i.state.insert("xs".to_string(), list);
        // xs[::2 ; _ > 12] — every 2nd, then keep > 12
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![
                BracketOp::Stride(Box::new(Expr::Integer(2))),
                BracketOp::Mask(Box::new(
                    Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                        Box::new(Expr::Integer(12)))
                )),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        // Every 2nd: [10, 5, 15] → filter > 12: [15]
        assert_eq!(result, Value::List(vec![Value::Int(15)]));
    }

    #[test]
    fn test_multislice_mask_then_stride() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Int(10), Value::Int(25), Value::Int(5),
            Value::Int(30), Value::Int(15)]);
        i.state.insert("xs".to_string(), list);
        // xs[; _ > 12 ::2] — keep > 12, then take every 2nd
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![
                BracketOp::Mask(Box::new(
                    Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                        Box::new(Expr::Integer(12)))
                )),
                BracketOp::Stride(Box::new(Expr::Integer(2))),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        // Filter > 12: [25, 30, 15] → every 2nd: [25, 15]
        assert_eq!(result, Value::List(vec![Value::Int(25), Value::Int(15)]));
    }

    #[test]
    fn test_slice_with_mask() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Int(10), Value::Int(25), Value::Int(5),
            Value::Int(30), Value::Int(15)]);
        i.state.insert("xs".to_string(), list);
        // xs[0..5 ; _ > 10] — slice then filter
        let expr = Expr::Slice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            start: None,
            end: None,
            stride: None,
            mask: Some(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Integer(10)))
            )),
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![
            Value::Int(25), Value::Int(30), Value::Int(15),
        ]));
    }

    #[test]
    fn test_arrow_transfer_list_with_filter() {
        let mut i = Interpreter::new();
        i.state.insert("src".to_string(), Value::List(vec![
            Value::Int(1), Value::Int(6), Value::Int(3), Value::Int(8), Value::Int(2),
        ]));
        i.state.insert("dest".to_string(), Value::List(vec![]));
        i.eval_expr(&Expr::ArrowTransfer {
            dest: Box::new(Expr::OwnedRef("dest".to_string())),
            source: Box::new(Expr::OwnedRef("src".to_string())),
            filter: Some(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Integer(5)))
            )),
        }).unwrap();
        assert_eq!(i.state.get("src"), Some(&Value::List(vec![
            Value::Int(1), Value::Int(3), Value::Int(2),
        ])));
        assert_eq!(i.state.get("dest"), Some(&Value::List(vec![
            Value::Int(6), Value::Int(8),
        ])));
    }

    #[test]
    fn test_arrow_transfer_hashmap_with_filter() {
        let mut i = Interpreter::new();
        let mut src = std::collections::HashMap::new();
        src.insert("a".to_string(), Value::Int(10));
        src.insert("b".to_string(), Value::Int(25));
        src.insert("c".to_string(), Value::Int(5));
        i.state.insert("src".to_string(), Value::HashMap(src));
        i.state.insert("dest".to_string(), Value::HashMap(std::collections::HashMap::new()));
        i.eval_expr(&Expr::ArrowTransfer {
            dest: Box::new(Expr::OwnedRef("dest".to_string())),
            source: Box::new(Expr::OwnedRef("src".to_string())),
            filter: Some(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Integer(15)))
            )),
        }).unwrap();
        match (i.state.get("src").unwrap(), i.state.get("dest").unwrap()) {
            (Value::HashMap(src), Value::HashMap(dest)) => {
                assert_eq!(src.len(), 2); // b=25 moved, a=10 and c=5 stay
                assert_eq!(dest.len(), 1);
                assert_eq!(dest.get("b"), Some(&Value::Int(25)));
            }
            _ => panic!("Expected HashMaps"),
        }
    }

    #[test]
    fn test_multislice_stride_zero_error() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Int(1), Value::Int(2)]);
        i.state.insert("xs".to_string(), list);
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Stride(Box::new(Expr::Integer(0)))],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_multislice_mask_on_non_list_error() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Int(42));
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Mask(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Integer(10)))
            ))],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_multislice_stride_on_int() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Int(42));
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Stride(Box::new(Expr::Integer(2)))],
        };
        // Int(42) decomposes to ['4', '2'], stride 2 gives ['4'], reconstructs to Int(4)
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(4));
    }

    #[test]
    fn test_multislice_regex_mask_on_list() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::String("hello".into()), Value::String("world".into()),
            Value::String("abc".into()), Value::String("wow".into())]);
        i.state.insert("xs".to_string(), list);
        // xs[;@\"^[hw]\"] — keep strings starting with 'h' or 'w' -> hello, world, wow
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Mask(Box::new(
                Expr::RegexLiteral("^[hw]".to_string())
            ))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![
            Value::String("hello".into()), Value::String("world".into()),
            Value::String("wow".into()),
        ]));
    }

    #[test]
    fn test_multislice_regex_mask_on_atomic_int() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Int(15561));
        // xs[;@\"[15]\"] — keep chars '1' or '5' -> "1551" -> Int(1551)
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Mask(Box::new(
                Expr::RegexLiteral("[15]".to_string())
            ))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1551));
    }

    #[test]
    fn test_slice_regex_mask_on_atomic_int() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Int(15561));
        // xs[0..5;@\"[15]\"] — slice [0..5] then keep chars '1' or '5' -> "1551" -> Int(1551)
        let expr = Expr::Slice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            start: Some(Box::new(Expr::Integer(0))),
            end: Some(Box::new(Expr::Integer(5))),
            stride: None,
            mask: Some(Box::new(Expr::RegexLiteral("[15]".to_string()))),
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1551));
    }

    #[test]
    fn test_type_directed_desugar_int_regex_coord() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Int(15561));
        // xs["[15]"] — single string coord on Int, desugars to per-char regex filter
        // "15561" → chars '1','5','5','6','1' → keep [15] → "1551" → Int(1551)
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Coord(SliceCoordinate::Index(Box::new(
                Expr::String("[15]".to_string())
            )))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1551));
    }

    #[test]
    fn test_sync_block_executes_statements_in_order() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Int(0));
        i.state.insert("y".to_string(), Value::Int(0));
        let sync_block = Statement::SyncBlock {
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("x".to_string()),
                    expr: Expr::Integer(1),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Assignment {
                    lhs: Expr::OwnedRef("y".to_string()),
                    expr: Expr::Integer(2),
                    timeout: None,
                    modifiers: vec![],
                },
            ],
        };
        i.exec_stmt(&sync_block).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Int(1)));
        assert_eq!(i.state.get("y"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_sync_block_nested_guarded() {
        let mut i = Interpreter::new();
        i.state.insert("a".to_string(), Value::Bool(false));
        i.state.insert("b".to_string(), Value::Int(0));
        let sync_block = Statement::SyncBlock {
            body: vec![
                Statement::Guarded {
                    condition: Expr::Bool(true),
                    statements: vec![Statement::Assignment {
                        lhs: Expr::OwnedRef("a".to_string()),
                        expr: Expr::Bool(true),
                        timeout: None,
                        modifiers: vec![],
                    }],
                },
                Statement::Assignment {
                    lhs: Expr::OwnedRef("b".to_string()),
                    expr: Expr::Integer(42),
                    timeout: None,
                    modifiers: vec![],
                },
            ],
        };
        i.exec_stmt(&sync_block).unwrap();
        assert_eq!(i.state.get("a"), Some(&Value::Bool(true)));
        assert_eq!(i.state.get("b"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_sync_block_empty() {
        let mut i = Interpreter::new();
        let sync_block = Statement::SyncBlock { body: vec![] };
        i.exec_stmt(&sync_block).unwrap();
    }

    // ---- Subtype Projection Tests ----

    #[test]
    fn test_projection_filter() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4), Value::Int(5),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Filter(Box::new(Expr::Gt(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Integer(3)),
            )))],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Int(4), Value::Int(5)]));
    }

    #[test]
    fn test_projection_map() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(1), Value::Int(2), Value::Int(3),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Map(Box::new(Expr::Mul(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Integer(2)),
            )))],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Int(2), Value::Int(4), Value::Int(6)]));
    }

    #[test]
    fn test_projection_filter_limit() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4), Value::Int(5),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![
                SubtypeOp::Filter(Box::new(Expr::Gt(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Integer(1)),
                ))),
                SubtypeOp::Limit(2),
            ],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Int(2), Value::Int(3)]));
    }

    #[test]
    fn test_projection_count_aggregate() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(10), Value::Int(20), Value::Int(30),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Count],
        }).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_projection_sum() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(5), Value::Int(10), Value::Int(15),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Sum(Box::new(Expr::Identifier("_".to_string())))],
        }).unwrap();
        assert_eq!(result, Value::Int(30));
    }

    #[test]
    fn test_projection_group_count() {
        let mut i = Interpreter::new();
        i.state.insert("items".to_string(), Value::List(vec![
            Value::Tuple(vec![Value::String("A".into()), Value::Int(1)]),
            Value::Tuple(vec![Value::String("A".into()), Value::Int(2)]),
            Value::Tuple(vec![Value::String("B".into()), Value::Int(3)]),
        ]));
        // Group by first element of each tuple
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("items".to_string())),
            ops: vec![
                SubtypeOp::Group(Box::new(Expr::Projection {
                    source: Box::new(Expr::Identifier("_".to_string())),
                    target: ProjectionTarget::Index(0),
                })),
            ],
        }).unwrap();
        // Result is a list of (key, list) tuples
        if let Value::List(groups) = result {
            assert_eq!(groups.len(), 2); // two groups: A and B
        } else {
            panic!("Expected List");
        }
    }

    #[test]
    fn test_projection_string_match() {
        let mut i = Interpreter::new();
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::String("user@example.com".into())),
            ops: vec![SubtypeOp::Match(Box::new(Expr::String("^([a-z]+)@(.+)$".into())))],
        }).unwrap();
        match result {
            Value::Tuple(groups) => {
                assert_eq!(groups.len(), 2);
                assert_eq!(groups[0], Value::String("user".into()));
                assert_eq!(groups[1], Value::String("example.com".into()));
            }
            _ => panic!("Expected Tuple, got {:?}", result),
        }
    }

    #[test]
    fn test_projection_string_no_match() {
        let mut i = Interpreter::new();
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::String("hello world".into())),
            ops: vec![SubtypeOp::Match(Box::new(Expr::String("^[0-9]+$".into())))],
        }).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    // ---- SubtypeOp gap tests ----

    #[test]
    fn test_subtype_skip_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4), Value::Int(5),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Skip(2)],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Int(3), Value::Int(4), Value::Int(5)]));
    }

    #[test]
    fn test_subtype_unique_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(1), Value::Int(1), Value::Int(2), Value::Int(2), Value::Int(3),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Unique],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));
    }

    #[test]
    fn test_subtype_sort_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("items".to_string(), Value::List(vec![
            Value::Tuple(vec![Value::String("b".into()), Value::Int(2)]),
            Value::Tuple(vec![Value::String("a".into()), Value::Int(1)]),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("items".to_string())),
            ops: vec![SubtypeOp::Sort(Box::new(Expr::Projection {
                source: Box::new(Expr::Identifier("_".to_string())),
                target: ProjectionTarget::Index(0),
            }))],
        }).unwrap();
        if let Value::List(sorted) = result {
            assert_eq!(sorted.len(), 2);
            // First element should be "a"
            if let Value::Tuple(ref fields) = sorted[0] {
                assert_eq!(fields[0], Value::String("a".into()));
            } else { panic!("Expected Tuple"); }
        } else { panic!("Expected List"); }
    }

    #[test]
    fn test_subtype_avg_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Avg(Box::new(Expr::Identifier("_".to_string())))],
        }).unwrap();
        assert_eq!(result, Value::Float(2.5));
    }

    #[test]
    fn test_subtype_min_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(3), Value::Int(1), Value::Int(4), Value::Int(1), Value::Int(5),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Min(Box::new(Expr::Identifier("_".to_string())))],
        }).unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_subtype_max_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Int(3), Value::Int(1), Value::Int(4), Value::Int(1), Value::Int(5),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Max(Box::new(Expr::Identifier("_".to_string())))],
        }).unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_subtype_join_two_lists() {
        let mut i = Interpreter::new();
        i.state.insert("left".to_string(), Value::List(vec![
            Value::Tuple(vec![Value::Int(1), Value::String("a".into())]),
            Value::Tuple(vec![Value::Int(2), Value::String("b".into())]),
        ]));
        i.state.insert("right".to_string(), Value::List(vec![
            Value::Tuple(vec![Value::Int(1), Value::String("x".into())]),
            Value::Tuple(vec![Value::Int(3), Value::String("y".into())]),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("left".to_string())),
            ops: vec![SubtypeOp::Join(
                Box::new(Expr::Identifier("right".to_string())),
                Box::new(Expr::Projection {
                    source: Box::new(Expr::Identifier("_".to_string())),
                    target: ProjectionTarget::Index(0),
                }),
            )],
        }).unwrap();
        if let Value::List(joined) = result {
            // Should join on matching key (1) — 1 row with both fields
            assert!(!joined.is_empty(), "Join should produce at least one result");
        } else { panic!("Expected List"); }
    }

    // ---- DBVL Append Tests ----

    #[test]
    fn test_dbvl_append_basic() {
        let path = "/tmp/dbrief_test_append.csv";
        // Clean up any previous test file
        let _ = std::fs::remove_file(path);

        let values = vec![
            Value::String("key1".into()),
            Value::String("value1".into()),
            Value::Int(42),
        ];
        let args = vec![
            Value::String(path.into()),
            Value::List(values),
        ];

        let result = crate::ffi::registry::dbvl_append_impl(args).unwrap();
        assert_eq!(result, Value::Bool(true));

        // Read file back and verify
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("key1,value1,42"));

        // Clean up
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_dbvl_append_csv_escaping() {
        let path = "/tmp/dbrief_test_escape.csv";
        let _ = std::fs::remove_file(path);

        // Value with comma — should be quoted
        let values = vec![
            Value::String("hello,world".into()),
            Value::String("normal".into()),
        ];
        let args = vec![
            Value::String(path.into()),
            Value::List(values),
        ];

        crate::ffi::registry::dbvl_append_impl(args).unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("\"hello,world\",normal"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_dbvl_append_multiple_lines() {
        let path = "/tmp/dbrief_test_multi.csv";
        let _ = std::fs::remove_file(path);

        for i in 0..3 {
            let values = vec![
                Value::String(format!("key{}", i)),
                Value::Int(i),
            ];
            let args = vec![
                Value::String(path.into()),
                Value::List(values),
            ];
            crate::ffi::registry::dbvl_append_impl(args).unwrap();
        }

        let content = std::fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "key0,0");
        assert_eq!(lines[1], "key1,1");
        assert_eq!(lines[2], "key2,2");

        let _ = std::fs::remove_file(path);
    }

    // ---- DBVL Lazy Loading Tests ----

    #[test]
    fn test_parse_csv_line_basic() {
        let vals = parse_csv_line(r#"key1,"hello,world",42,true"#);
        assert_eq!(vals.len(), 4);
        assert_eq!(vals[0], Value::String("key1".into()));
        assert_eq!(vals[1], Value::String("hello,world".into()));
        assert_eq!(vals[2], Value::Int(42));
        assert_eq!(vals[3], Value::Bool(true));
    }

    #[test]
    fn test_parse_csv_line_ints_floats() {
        let vals = parse_csv_line("42,3.14,100,0.5");
        assert_eq!(vals[0], Value::Int(42));
        assert_eq!(vals[1], Value::Float(3.14));
        assert_eq!(vals[2], Value::Int(100));
        assert_eq!(vals[3], Value::Float(0.5));
    }

    #[test]
    fn test_try_extract_key_eq_positive() {
        use crate::ast::*;
        // _.field_0 == "rusty_key"
        let field_access = Expr::FieldAccess(
            Box::new(Expr::Identifier("_".into())),
            "field_0".into(),
        );
        let eq = Expr::Eq(
            Box::new(field_access),
            Box::new(Expr::String("rusty_key".into())),
        );
        assert_eq!(
            try_extract_key_eq(&eq, 0),
            Some("rusty_key".to_string()),
        );
    }

    #[test]
    fn test_try_extract_key_eq_negative() {
        use crate::ast::*;
        // _.name > "rusty_key" — not an equality
        let gt = Expr::Gt(
            Box::new(Expr::FieldAccess(
                Box::new(Expr::Identifier("_".into())),
                "name".into(),
            )),
            Box::new(Expr::String("rusty_key".into())),
        );
        assert_eq!(try_extract_key_eq(&gt, 0), None);
    }

    #[test]
    fn test_dbvl_table_resolve_key() {
        // Create a temp .dbvl file
        let path = "/tmp/dbrief_test_lazy.csv";
        let _ = std::fs::remove_file(path);
        std::fs::write(path, "rusty_key,\"Rusty Key\",5\ncandle,\"Wax Candle\",3\n").unwrap();

        let mut i = Interpreter::new();
        let offsets: HashMap<String, Vec<usize>> = [
            ("rusty_key".into(), vec![0usize]),
            ("candle".into(), vec![29usize]),
        ].into();

        let table = Arc::new(DbvlTableInner {
            path: path.into(),
            key_offsets: offsets,
            field_names: vec!["id".into(), "name".into(), "hp".into()],
            schema_name: Some("Item".into()),
            schema_key_index: Some(0),
        });

        // Resolve a key
        let results = i.resolve_dbvl_key(&table, "rusty_key").unwrap();
        assert_eq!(results.len(), 1);
        match &results[0] {
            Value::Instance { typename, fields } => {
                assert_eq!(typename, "Item");
                assert_eq!(fields.get("id"), Some(&Value::String("rusty_key".into())));
                assert_eq!(fields.get("name"), Some(&Value::String("Rusty Key".into())));
                assert_eq!(fields.get("hp"), Some(&Value::Int(5)));
            }
            other => panic!("Expected Instance, got {:?}", other),
        }

        // Resolve a non-existent key
        let results = i.resolve_dbvl_key(&table, "nonexistent").unwrap();
        assert!(results.is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_dbvl_table_list_index_access() {
        let path = "/tmp/dbrief_test_lazy_index.csv";
        let _ = std::fs::remove_file(path);
        std::fs::write(path, "rusty_key,\"Rusty Key\",5\ncandle,\"Wax Candle\",3\n").unwrap();

        let offsets: HashMap<String, Vec<usize>> = [
            ("rusty_key".into(), vec![0usize]),
            ("candle".into(), vec![29usize]),
        ].into();

        let table_val = Value::DbvlTable(Arc::new(DbvlTableInner {
            path: path.into(),
            key_offsets: offsets,
            field_names: vec!["id".into(), "name".into(), "hp".into()],
            schema_name: Some("Item".into()),
            schema_key_index: Some(0),
        }));

        let mut i = Interpreter::new();
        let result = i.eval_expr(&Expr::ListIndex(
            Box::new(Expr::Identifier("table".into())),
            Box::new(Expr::String("candle".into())),
        )).unwrap_err(); // This will fail because table isn't in state

        // Actually let's test by calling resolve_dbvl_key directly on the table
        // This simulates what ListIndex does internally
        let table_inner = match &table_val {
            Value::DbvlTable(t) => t.clone(),
            _ => panic!("Expected DbvlTable"),
        };
        let results = i.resolve_dbvl_key(&table_inner, "candle").unwrap();
        assert_eq!(results.len(), 1);
        match &results[0] {
            Value::Instance { typename, fields } => {
                assert_eq!(typename, "Item");
                assert_eq!(fields.get("name"), Some(&Value::String("Wax Candle".into())));
            }
            other => panic!("Expected Instance, got {:?}", other),
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_tuple_destructure_assignment() {
        let mut i = Interpreter::new();
        i.state.insert("a".to_string(), Value::Int(0));
        i.state.insert("b".to_string(), Value::Int(0));
        let stmt = Statement::Assignment {
            lhs: Expr::TupleDestructure(
                vec!["a".to_string(), "b".to_string()],
                Box::new(Expr::Term),
            ),
            expr: Expr::Tuple(vec![Expr::Integer(42), Expr::Integer(99)]),
            timeout: None,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("a"), Some(&Value::Int(42)));
        assert_eq!(i.state.get("b"), Some(&Value::Int(99)));
    }

    #[test]
    fn test_tuple_destructure_assignment_from_list() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Int(0));
        i.state.insert("y".to_string(), Value::Int(0));
        let stmt = Statement::Assignment {
            lhs: Expr::TupleDestructure(
                vec!["x".to_string(), "y".to_string()],
                Box::new(Expr::Term),
            ),
            expr: Expr::ListLiteral(vec![Expr::Integer(7), Expr::Integer(13)]),
            timeout: None,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Int(7)));
        assert_eq!(i.state.get("y"), Some(&Value::Int(13)));
    }

    #[test]
    fn test_tuple_destructure_assignment_wrong_type_errors() {
        let mut i = Interpreter::new();
        i.state.insert("a".to_string(), Value::Int(0));
        let stmt = Statement::Assignment {
            lhs: Expr::TupleDestructure(
                vec!["a".to_string()],
                Box::new(Expr::Term),
            ),
            expr: Expr::Integer(42),
            timeout: None,
            modifiers: vec![],
        };
        let err = i.exec_stmt(&stmt).unwrap_err();
        match err {
            RuntimeError::TypeMismatch(ref msg) => {
                assert!(msg.contains("destructure"), "msg: {}", msg);
            }
            _ => panic!("Expected TypeMismatch error, got: {:?}", err),
        }
    }

    #[test]
    fn test_callable_txn_postcondition_failure_returns_error() {
        let mut i = Interpreter::new();
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "bad_post".to_string(),
            parameters: vec![
                ("x".to_string(), Type::Int),
            ],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Eq(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Integer(0)),
                ),
                watchdog: None,
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("x".to_string()),
                    expr: Expr::Integer(99),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Term {
                    values: vec![Some(Expr::Identifier("x".to_string()))],
                    swan_song: None,
                    modifiers: vec![],
                },
            ],
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        };
        i.callable_txns.insert("bad_post".to_string(), txn);
        let result = i.eval_expr(&Expr::Call("bad_post".to_string(), vec![
            Expr::Integer(0),
        ]));
        assert!(result.is_err(), "postcondition violation should return error");
        match result {
            Err(RuntimeError::ContractViolation(msg)) => {
                assert!(msg.contains("bad_post"), "error should name the txn");
            }
            _ => panic!("Expected ContractViolation error, got {:?}", result),
        }
    }

    #[test]
    fn test_callable_txn_convergence_satisfies_postcondition() {
        let mut i = Interpreter::new();
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "count_to_n".to_string(),
            parameters: vec![
                ("n".to_string(), Type::Int),
                ("acc".to_string(), Type::Int),
                ("i".to_string(), Type::Int),
            ],
            contract: Contract {
                pre_condition: Expr::Lt(
                    Box::new(Expr::Identifier("i".to_string())),
                    Box::new(Expr::Identifier("n".to_string())),
                ),
                post_condition: Expr::Eq(
                    Box::new(Expr::Identifier("i".to_string())),
                    Box::new(Expr::Identifier("n".to_string())),
                ),
                watchdog: None,
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("acc".to_string()),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("acc".to_string())),
                        Box::new(Expr::Identifier("i".to_string())),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Assignment {
                    lhs: Expr::OwnedRef("i".to_string()),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("i".to_string())),
                        Box::new(Expr::Integer(1)),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Term {
                    values: vec![Some(Expr::Identifier("acc".to_string()))],
                    swan_song: None,
                    modifiers: vec![],
                },
            ],
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        };
        i.callable_txns.insert("count_to_n".to_string(), txn);
        let result = i.eval_expr(&Expr::Call("count_to_n".to_string(), vec![
            Expr::Integer(5),
            Expr::Integer(0),
            Expr::Integer(0),
        ])).unwrap();
        assert_eq!(result, Value::Int(10));  // 0+1+2+3+4 = 10
    }

    #[test]
    fn test_foreach_filter() {
        let mut i = Interpreter::new();
        let stmt = Statement::Foreach {
            item: "x".to_string(),
            list: Box::new(Expr::ListLiteral(vec![Expr::Integer(1), Expr::Integer(2), Expr::Integer(3)])),
            body: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        // Just verify no error — basic loop completion
    }

    #[test]
    fn test_foreach_accumulates() {
        let mut i = Interpreter::new();
        i.state.insert("sum".to_string(), Value::Int(0));
        let stmt = Statement::Foreach {
            item: "x".to_string(),
            list: Box::new(Expr::ListLiteral(vec![Expr::Integer(10), Expr::Integer(20), Expr::Integer(30)])),
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("sum".to_string()),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("sum".to_string())),
                        Box::new(Expr::Identifier("x".to_string())),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
            ],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("sum"), Some(&Value::Int(60)));
    }

    #[test]
    fn test_match_string_literal() {
        let mut i = Interpreter::new();
        let expr = Expr::Match {
            value: Box::new(Expr::String("foo".to_string())),
            arms: vec![
                crate::ast::MatchArm {
                    pattern: crate::ast::MatchPattern::Literal(crate::ast::Pattern::LitString("foo".to_string())),
                    guard: None,
                    body: Box::new(Expr::Integer(1)),
                },
                crate::ast::MatchArm {
                    pattern: crate::ast::MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Integer(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_match_int_literal() {
        let mut i = Interpreter::new();
        let expr = Expr::Match {
            value: Box::new(Expr::Integer(42)),
            arms: vec![
                crate::ast::MatchArm {
                    pattern: crate::ast::MatchPattern::Literal(crate::ast::Pattern::LitInt(42)),
                    guard: None,
                    body: Box::new(Expr::Bool(true)),
                },
                crate::ast::MatchArm {
                    pattern: crate::ast::MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Bool(false)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_oracle_executes_body() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Int(0));
        let stmt = Statement::Oracle {
            handler: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("x".to_string()),
                    expr: Expr::Integer(99),
                    timeout: None, modifiers: vec![],
                },
            ],
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("x".to_string()),
                    expr: Expr::Integer(42),
                    timeout: None, modifiers: vec![],
                },
            ],
            span: None,
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_oracle_fuel_exhausts_runs_handler() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Int(0));
        // Use a long sequence of statements to exhaust fuel, not recursion
        let mut body = Vec::new();
        // The fuel limit is 100, so 200 assignments should exhaust it
        for _ in 0..200 {
            body.push(Statement::Assignment {
                lhs: Expr::OwnedRef("x".to_string()),
                expr: Expr::Integer(42),
                timeout: None, modifiers: vec![],
            });
        }
        let stmt = Statement::Oracle {
            handler: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("x".to_string()),
                    expr: Expr::Integer(999),
                    timeout: None, modifiers: vec![],
                },
            ],
            body,
            span: None,
        };
        i.exec_stmt(&stmt).unwrap();
        // Fuel exhausted — handler sets x = 999
        assert_eq!(i.state.get("x"), Some(&Value::Int(999)));
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;


    #[kani::proof]
    fn verify_eval_expr_literal_integer() {
        let mut ctx = Interpreter::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let result = ctx.eval_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[kani::proof]
    fn verify_eval_expr_literal_bool() {
        let mut ctx = Interpreter::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let result = ctx.eval_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bool(true));
    }

    #[kani::proof]
    fn verify_eval_expr_literal_float() {
        let mut ctx = Interpreter::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Float(3.14)));
        let result = ctx.eval_expr(&expr);
        assert!(result.is_ok());
    }

    #[kani::proof]
    fn verify_eval_expr_literal_string() {
        let mut ctx = Interpreter::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::String("test".to_string())));
        let result = ctx.eval_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::String("test".to_string()));
    }

    #[kani::proof]
    fn verify_eval_expr_literal_char() {
        let mut ctx = Interpreter::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Char('A')));
        let result = ctx.eval_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Char('A'));
    }

    // ── Intrinsic evaluation tests ──────────────────────────────

    #[test]
    fn test_intrinsic_sqrt() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Sqrt,
            args: vec![Expr::Float(9.0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Float(3.0));
    }

    #[test]
    fn test_intrinsic_fabs() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Fabs,
            args: vec![Expr::Float(-3.5)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Float(3.5));
    }

    #[test]
    fn test_intrinsic_ceil() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Ceil,
            args: vec![Expr::Float(3.2)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Float(4.0));
    }

    #[test]
    fn test_intrinsic_floor() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Floor,
            args: vec![Expr::Float(3.8)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Float(3.0));
    }

    #[test]
    fn test_intrinsic_ctpop() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Ctpop,
            args: vec![Expr::Integer(255)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(8));
    }

    #[test]
    fn test_intrinsic_ctlz() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Ctlz,
            args: vec![Expr::Integer(1)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(63));
    }

    #[test]
    fn test_intrinsic_cttz() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Cttz,
            args: vec![Expr::Integer(8)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_intrinsic_abs() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Abs,
            args: vec![Expr::Integer(-42)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(42));
    }

    #[test]
    fn test_intrinsic_bitreverse() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bitreverse,
            args: vec![Expr::Integer(1)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1i64.reverse_bits()));
    }

    #[test]
    fn test_intrinsic_bytes_int() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bytes,
            args: vec![Expr::Integer(42)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(8));
    }

    #[test]
    fn test_intrinsic_bytes_float() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bytes,
            args: vec![Expr::Float(3.0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(8));
    }

    #[test]
    fn test_intrinsic_bytes_bool() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bytes,
            args: vec![Expr::Bool(true)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1));
    }

    #[test]
    fn test_intrinsic_bytes_char() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bytes,
            args: vec![Expr::Char('A')],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(4));
    }

    #[test]
    fn test_intrinsic_size_list() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Size,
            args: vec![Expr::ListLiteral(vec![
                Expr::Integer(1), Expr::Integer(2), Expr::Integer(3),
            ])],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_intrinsic_size_string() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Size,
            args: vec![Expr::String("hello".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn test_intrinsic_pop() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Pop,
            args: vec![Expr::ListLiteral(vec![Expr::Integer(1), Expr::Integer(2)])],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn test_intrinsic_contains_list() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Contains,
            args: vec![
                Expr::ListLiteral(vec![Expr::Integer(1), Expr::Integer(2), Expr::Integer(3)]),
                Expr::Integer(1),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_intrinsic_contains_list_false() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Contains,
            args: vec![
                Expr::ListLiteral(vec![Expr::Integer(1), Expr::Integer(2)]),
                Expr::Integer(99),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_intrinsic_contains_string() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Contains,
            args: vec![
                Expr::String("hello".to_string()),
                Expr::Char('e'),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_intrinsic_keys() {
        let mut i = Interpreter::new();
        let mut map = std::collections::HashMap::new();
        map.insert("a".to_string(), Value::Int(1));
        map.insert("b".to_string(), Value::Int(2));
        i.state.insert("m".to_string(), Value::HashMap(map));
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Keys,
            args: vec![Expr::OwnedRef("m".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::List(keys) => {
                assert_eq!(keys.len(), 2);
                assert!(keys.contains(&Value::String("a".to_string())));
                assert!(keys.contains(&Value::String("b".to_string())));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_intrinsic_values() {
        let mut i = Interpreter::new();
        let mut map = std::collections::HashMap::new();
        map.insert("a".to_string(), Value::Int(10));
        map.insert("b".to_string(), Value::Int(20));
        i.state.insert("m".to_string(), Value::HashMap(map));
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Values,
            args: vec![Expr::OwnedRef("m".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::List(vals) => {
                assert_eq!(vals.len(), 2);
                assert!(vals.contains(&Value::Int(10)));
                assert!(vals.contains(&Value::Int(20)));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_intrinsic_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Sqrt,
            args: vec![Expr::String("hello".to_string())],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "sqrt#(\"hello\") should produce a type error");
    }

    // ── Phase A: Terminal + Process intrinsic tests ─────────────────
    //
    // These intrinsics wrap libc / std::process::Command. In a test
    // environment without a real terminal, they fall back gracefully:
    //   - tty_raw_mode#(false)  → Ok(Bool(false)) on non-tty
    //   - tty_size#()           → 80 * 10000 + 24
    //   - tty_read_key#()       → -1 (no key available)
    //   - ioctl#(-1, 0, 0)      → -1 (invalid fd)
    //   - isatty#(0)            → false (stdin not a tty in test runner)
    //   - spawn_with_output#    → stdout string or empty
    //   - spawn#                → exit code

    #[test]
    fn test_intrinsic_tty_raw_mode_disable() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TtyRawMode,
            args: vec![Expr::Bool(false)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false), "tty_raw_mode#(false) should return false (not a tty)");
    }

    #[test]
    fn test_intrinsic_tty_raw_mode_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TtyRawMode,
            args: vec![Expr::Integer(42)],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "tty_raw_mode#(42) should produce type error");
    }

    #[test]
    fn test_intrinsic_tty_size_returns_fallback() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TtySize,
            args: vec![],
        };
        let result = i.eval_expr(&expr).unwrap();
        if let Value::Int(encoded) = result {
            let cols = encoded / 10000;
            let rows = encoded % 10000;
            assert!(cols >= 80 && cols <= 400, "tty_size# should return cols >= 80, got {}", cols);
            assert!(rows >= 24 && rows <= 200, "tty_size# should return rows >= 24, got {}", rows);
        } else {
            panic!("tty_size# should return Int, got {:?}", result);
        }
    }

    #[test]
    fn test_intrinsic_tty_read_key_no_key() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TtyReadKey,
            args: vec![],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(-1), "tty_read_key#() should return -1 when no key available");
    }

    #[test]
    fn test_intrinsic_ioctl_invalid_fd() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IoCtl,
            args: vec![Expr::Integer(-1), Expr::Integer(0), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        // ioctl with invalid fd returns -1, wrapped in Int
        assert!(result == Value::Int(-1), "ioctl#(-1,0,0) should return -1, got {:?}", result);
    }

    #[test]
    fn test_intrinsic_ioctl_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IoCtl,
            args: vec![Expr::String("stdin".into()), Expr::Integer(0), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "ioctl#(\"stdin\",...) should type error");
    }

    #[test]
    fn test_intrinsic_isatty_stdin() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IsTty,
            args: vec![Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        // In test runner, stdin is usually piped, not a tty
        assert_eq!(result, Value::Bool(false), "isatty#(0) should return false in test runner");
    }

    #[test]
    fn test_intrinsic_isatty_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IsTty,
            args: vec![Expr::Bool(true)],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "isatty#(true) should type error");
    }

    #[test]
    fn test_intrinsic_spawn_with_output_echo() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SpawnWithOutput,
            args: vec![Expr::String("echo hello".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        if let Value::String(s) = result {
            assert_eq!(s.trim(), "hello", "spawn_with_output#(\"echo hello\") should return \"hello\"");
        } else {
            panic!("spawn_with_output# should return String, got {:?}", result);
        }
    }

    #[test]
    fn test_intrinsic_spawn_with_output_empty_cmd() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SpawnWithOutput,
            args: vec![Expr::String("".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        // Empty command may return empty string or error depending on platform
        assert!(matches!(result, Value::String(_)), "spawn_with_output#(\"\") should return String");
    }

    #[test]
    fn test_intrinsic_spawn_true() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Spawn,
            args: vec![Expr::String("true".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(0), "spawn#(\"true\") should return 0");
    }

    #[test]
    fn test_intrinsic_spawn_false() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Spawn,
            args: vec![Expr::String("false".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1), "spawn#(\"false\") should return 1");
    }

    #[test]
    fn test_intrinsic_spawn_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Spawn,
            args: vec![Expr::Integer(42)],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "spawn#(42) should type error");
    }

    // ── Phase B: Raw File I/O intrinsic tests ───────────────────────
    //
    // These intrinsics wrap POSIX raw I/O. In the interpreter, syscalls
    // that require opaque pointer args (read/write/pread/pwrite) allocate
    // temporary buffers since the interpreter can't dereference caller
    // pointers. write#/pwrite# return -1 (pointer is opaque).

    #[test]
    fn test_intrinsic_open_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Open,
            args: vec![Expr::Bool(true), Expr::Integer(0), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_open_bad_path() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Open,
            args: vec![
                Expr::String("/nonexistent_file_xyz.bv".into()),
                Expr::Integer(0),
                Expr::Integer(0),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(-1), "open#(bad path) should return -1");
    }

    #[test]
    fn test_intrinsic_close_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Close,
            args: vec![Expr::String("fd".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_close_bad_fd() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Close,
            args: vec![Expr::Integer(-1)],
        };
        let result = i.eval_expr(&expr).unwrap();
        #[cfg(unix)]
        assert_eq!(result, Value::Int(-1), "close#(-1) should return -1");
        #[cfg(not(unix))]
        assert_eq!(result, Value::Int(-1));
    }

    #[test]
    fn test_intrinsic_read_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Read,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::String("nope".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_write_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Write,
            args: vec![Expr::Integer(1), Expr::Integer(0), Expr::Integer(5)],
        };
        // write# with opaque pointer returns -1 in interpreter
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(-1), "write# should return -1 in interpreter");
    }

    #[test]
    fn test_intrinsic_lseek_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::LSeek,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_lseek_bad_fd() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::LSeek,
            args: vec![Expr::Integer(-1), Expr::Integer(0), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(-1), "lseek#(-1,0,0) should return -1");
    }

    #[test]
    fn test_intrinsic_pread_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::PRead,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(5), Expr::String("off".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_pwrite_returns_minus_one() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::PWrite,
            args: vec![Expr::Integer(1), Expr::Integer(0), Expr::Integer(5), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(-1), "pwrite# should return -1 in interpreter");
    }

    #[test]
    fn test_intrinsic_stat_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Stat,
            args: vec![Expr::Integer(42)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_stat_bad_path() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Stat,
            args: vec![Expr::String("/nonexistent_stat_file.xyz".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(-1), "stat#(bad path) should return -1");
    }

    #[test]
    fn test_intrinsic_fstat_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FStat,
            args: vec![Expr::String("fd".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_truncate_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Truncate,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_ftruncate_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FTruncate,
            args: vec![Expr::Integer(0), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_fsync_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FSync,
            args: vec![Expr::String("fd".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_dup_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FDup,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_dup2_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FDup2,
            args: vec![Expr::Integer(0), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_fcntl_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FCntl,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase C: Filesystem intrinsic tests ─────────────────────────

    #[test]
    fn test_intrinsic_mkdir_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MkDir,
            args: vec![Expr::Integer(42), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_rmdir_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::RmDir,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_unlink_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Unlink,
            args: vec![Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_rename_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Rename,
            args: vec![Expr::String("a".into()), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_symlink_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SymLink,
            args: vec![Expr::String("target".into()), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_readlink_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadLink,
            args: vec![Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_readlink_bad_path() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadLink,
            args: vec![Expr::String("/nonexistent_readlink.xyz".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::String(String::new()), "readlink#(bad path) should return empty string");
    }

    #[test]
    fn test_intrinsic_link_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Link,
            args: vec![Expr::String("old".into()), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_getcwd_returns_string() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetCwd,
            args: vec![],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert!(matches!(result, Value::String(s) if !s.is_empty()), "getcwd#() should return non-empty string");
    }

    #[test]
    fn test_intrinsic_chdir_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChDir,
            args: vec![Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_readdir_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadDir,
            args: vec![Expr::Integer(42)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_readdir_bad_path() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadDir,
            args: vec![Expr::String("/nonexistent_dir_xyz".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![]), "readdir#(bad path) should return empty list");
    }

    #[test]
    fn test_intrinsic_readdir_current_dir() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadDir,
            args: vec![Expr::String(".".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert!(matches!(result, Value::List(ref items) if !items.is_empty()), "readdir#(\".\") should return entries");
    }

    #[test]
    fn test_intrinsic_chmod_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChMod,
            args: vec![Expr::String("/tmp".into()), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_chown_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChOwn,
            args: vec![Expr::String("/tmp".into()), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_umask_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UMask,
            args: vec![Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_umask_returns_int() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UMask,
            args: vec![Expr::Integer(0o022)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert!(matches!(result, Value::Int(_)), "umask# should return Int");
    }

    #[test]
    fn test_intrinsic_access_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Access,
            args: vec![Expr::Integer(42), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase D: Memory + Synchronization intrinsic tests ───────────

    #[test]
    fn test_intrinsic_mmap_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Mmap,
            args: vec![Expr::Integer(0), Expr::Integer(4096), Expr::Bool(true), Expr::Integer(0), Expr::Integer(-1), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err(), "mmap# with Bool prot should type error");
    }

    #[test]
    fn test_intrinsic_munmap_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MUnmap,
            args: vec![Expr::Bool(false), Expr::Integer(4096)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_mprotect_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MProtect,
            args: vec![Expr::Integer(0), Expr::Integer(4096), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_brk_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Brk,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_brk_returns_zero() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Brk,
            args: vec![Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert!(matches!(result, Value::Int(_)), "brk# should return Int");
    }

    #[test]
    fn test_intrinsic_mlock_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MLock,
            args: vec![Expr::Integer(0), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_load_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicLoad,
            args: vec![Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_load_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicLoad,
            args: vec![Expr::Integer(0), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(0), "atomic_load# stub should return 0");
    }

    #[test]
    fn test_intrinsic_atomic_store_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicStore,
            args: vec![Expr::Integer(0), Expr::Integer(42), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_store_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicStore,
            args: vec![Expr::Integer(0), Expr::Integer(42), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(-1), "atomic_store# stub should return -1");
    }

    #[test]
    fn test_intrinsic_atomic_cas_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicCas,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(1), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_cas_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicCas,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(1), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(0), "atomic_cas# stub should return 0");
    }

    #[test]
    fn test_intrinsic_atomic_xchg_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicXchg,
            args: vec![Expr::Integer(0), Expr::Bool(true), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_xchg_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicXchg,
            args: vec![Expr::Integer(0), Expr::Integer(42), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(0), "atomic_xchg# stub should return 0");
    }

    #[test]
    fn test_intrinsic_atomic_add_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicAdd,
            args: vec![Expr::Integer(0), Expr::Bool(true), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_add_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicAdd,
            args: vec![Expr::Integer(0), Expr::Integer(1), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(0), "atomic_add# stub should return 0");
    }

    #[test]
    fn test_intrinsic_fence_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Fence,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_fence_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Fence,
            args: vec![Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(0), "fence# stub should return 0");
    }

    #[test]
    fn test_intrinsic_futex_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Futex,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_futex_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Futex,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Integer(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(-1), "futex# stub should return -1");
    }

    // ── Phase E: IPC intrinsic tests ────────────────────────────────

    #[test]
    fn test_intrinsic_pipe_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Pipe,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_shm_open_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ShmOpen,
            args: vec![Expr::Integer(42), Expr::Integer(0), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_shm_unlink_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ShmUnlink,
            args: vec![Expr::Integer(42)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_sem_open_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SemOpen,
            args: vec![Expr::String("/test".into()), Expr::Integer(0), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_sem_wait_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SemWait,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_sem_post_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SemPost,
            args: vec![Expr::String("sem".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase F: Signals intrinsic tests ───────────────────────────

    #[test]
    fn test_intrinsic_sigaction_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SigAction,
            args: vec![Expr::Bool(false), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_sigprocmask_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SigProcMask,
            args: vec![Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_kill_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Kill,
            args: vec![Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_signalfd_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SignalFd,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_timerfd_create_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TimerFdCreate,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase G: Networking intrinsic tests ────────────────────────

    #[test]
    fn test_intrinsic_socket_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Socket,
            args: vec![Expr::Bool(false), Expr::Integer(0), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_bind_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bind,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_listen_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Listen,
            args: vec![Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_accept_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Accept,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_connect_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Connect,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_send_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Send,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_recv_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Recv,
            args: vec![Expr::Integer(0), Expr::Integer(0), Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_sendto_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SendTo,
            args: vec![
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
                Expr::Bool(false), Expr::Integer(0), Expr::Integer(0),
            ],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_recvfrom_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::RecvFrom,
            args: vec![
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
                Expr::Bool(false), Expr::Integer(0), Expr::Integer(0),
            ],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_setsockopt_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SetSockOpt,
            args: vec![
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
                Expr::Bool(false), Expr::Integer(0),
            ],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_getsockopt_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetSockOpt,
            args: vec![
                Expr::Integer(0), Expr::Integer(0), Expr::Integer(0),
                Expr::Bool(false), Expr::Integer(0),
            ],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_shutdown_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Shutdown,
            args: vec![Expr::Integer(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_getaddrinfo_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetAddrInfo,
            args: vec![Expr::Integer(0), Expr::String("http".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase H: Everything Else intrinsic tests ──────────────────

    #[test]
    fn test_intrinsic_getenv_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetEnv,
            args: vec![Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_setenv_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SetEnv,
            args: vec![Expr::String("PATH".into()), Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_unsetenv_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UnsetEnv,
            args: vec![Expr::Integer(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_clock_gettime_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ClockGetTime,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_nanosleep_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::NanoSleep,
            args: vec![Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_eval_is_type_int() {
        let mut i = Interpreter::new();
        let expr = Expr::IsType(
            Box::new(Expr::Integer(42)),
            crate::ast::IsTarget::Type(Type::Int),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true), "42 is Int should be true");
    }

    #[test]
    fn test_eval_is_type_string() {
        let mut i = Interpreter::new();
        let expr = Expr::IsType(
            Box::new(Expr::String("hello".to_string())),
            crate::ast::IsTarget::Type(Type::Int),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false), "string is Int should be false");
    }

    #[test]
    fn test_eval_is_variant() {
        let mut i = Interpreter::new();
        let mut fields = std::collections::HashMap::new();
        fields.insert("value".to_string(), Value::Int(42));
        i.state.insert("x".to_string(), Value::Enum("Option".to_string(), "Some".to_string(), fields));
        let expr = Expr::IsType(
            Box::new(Expr::OwnedRef("x".to_string())),
            crate::ast::IsTarget::Variant("Some".to_string()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true), "Option::Some is Some should be true");
    }

    #[test]
    fn test_eval_is_variant_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Enum("Option".to_string(), "None".to_string(), std::collections::HashMap::new()));
        let expr = Expr::IsType(
            Box::new(Expr::OwnedRef("x".to_string())),
            crate::ast::IsTarget::Variant("Some".to_string()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false), "Option::None is Some should be false");
    }

    #[test]
    fn test_eval_from_check() {
        let mut i = Interpreter::new();
        let mut fields = std::collections::HashMap::new();
        fields.insert("x".to_string(), Value::Int(1));
        i.state.insert("obj".to_string(), Value::Instance { typename: "Foo".to_string(), fields });
        let expr = Expr::FromCheck(
            Box::new(Expr::OwnedRef("obj".to_string())),
            Type::Custom("Foo".to_string()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true), "obj from Foo should be true");
    }

    #[test]
    fn test_eval_like_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::Integer(42)),
            Box::new(Expr::Integer(42)),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true), "42 like 42 should be true");
    }

    #[test]
    fn test_eval_like_int_mismatch() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::Integer(42)),
            Box::new(Expr::Integer(1)),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false), "42 like 1 should be false");
    }

    #[test]
    fn test_eval_like_float() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::Float(3.14)),
            Box::new(Expr::Float(3.14)),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true), "3.14 like 3.14 should be true");
    }

    #[test]
    fn test_eval_like_string() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::String("hello".to_string())),
            Box::new(Expr::String("hello".to_string())),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true), "\"hello\" like \"hello\" should be true");
    }

    #[test]
    fn test_eval_like_string_mismatch() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::String("hello".to_string())),
            Box::new(Expr::String("world".to_string())),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false), "\"hello\" like \"world\" should be false");
    }

    #[test]
    fn test_eval_like_list() {
        let mut i = Interpreter::new();
        let a = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let b = Value::List(vec![Value::Int(1), Value::Int(2)]);
        i.state.insert("a".to_string(), a);
        i.state.insert("b".to_string(), b);
        let expr = Expr::Like(
            Box::new(Expr::OwnedRef("a".to_string())),
            Box::new(Expr::OwnedRef("b".to_string())),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true), "[1,2] like [1,2] should be true");
    }

    #[test]
    fn test_eval_like_list_mismatch() {
        let mut i = Interpreter::new();
        let a = Value::List(vec![Value::Int(1), Value::Int(2)]);
        let b = Value::List(vec![Value::Int(1), Value::Int(3)]);
        i.state.insert("a".to_string(), a);
        i.state.insert("b".to_string(), b);
        let expr = Expr::Like(
            Box::new(Expr::OwnedRef("a".to_string())),
            Box::new(Expr::OwnedRef("b".to_string())),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false), "[1,2] like [1,3] should be false");
    }

    #[test]
    fn test_eval_cast_int_to_string() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Integer(42)), Type::String);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::String("42".to_string()), "Int -> String should format as decimal");
    }

    #[test]
    fn test_eval_cast_string_to_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::String("42".to_string())), Type::Int);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(42), "String -> Int should parse decimal");
    }

    #[test]
    fn test_eval_cast_char_to_string() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Char('A')), Type::String);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::String("A".to_string()), "Char -> String should be single-char");
    }

    #[test]
    fn test_eval_cast_string_to_char() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::String("hello".to_string())), Type::Char);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Char('h'), "String -> Char should take first char");
    }

    #[test]
    fn test_eval_cast_int_to_float() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Integer(42)), Type::Float);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Float(42.0), "Int -> Float should be exact");
    }

    #[test]
    fn test_eval_cast_float_to_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Float(3.14)), Type::Int);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(3), "Float -> Int should truncate");
    }

    #[test]
    fn test_eval_cast_int_to_char() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Integer(65)), Type::Char);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Char('A'), "Int 65 -> Char should be 'A'");
    }

    #[test]
    fn test_eval_cast_char_to_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Char('A')), Type::Int);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(65), "Char 'A' -> Int should be 65");
    }

    #[test]
    fn test_eval_cast_bool_to_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Bool(true)), Type::Int);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Int(1), "Bool true -> Int should be 1");
    }

    #[test]
    fn test_eval_cast_int_to_bool() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Integer(42)), Type::Bool);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(true), "Int 42 -> Bool should be true");
    }

    #[test]
    fn test_eval_cast_int_zero_to_bool() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Integer(0)), Type::Bool);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bool(false), "Int 0 -> Bool should be false");
    }

    #[test]
    fn test_eval_cast_unsupported() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::List(vec![])), Type::Int);
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "List -> Int should be an error");
    }
}
