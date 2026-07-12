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
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;

mod cells;
pub(crate) use cells::{
    cell_convergence_pass, cell_tick, CellChannel, CellWire,
    PersistentCellInstance, TrgBindingReg, VirtualHeap,
};
mod ffi;
pub(crate) use ffi::{
    abs_impl, ceil_impl, chars_impl, concat_impl, contains_impl, cos_impl, create_dir_impl,
    delete_dir_impl, delete_file_impl, ends_with_impl, exec_cmd_impl, floor_impl, from_str_impl,
    input_impl, int_to_string_impl, json_get_by_index_impl, json_get_impl, json_is_array_impl,
    json_length_impl, json_parse_impl, len_impl, now_impl, pow_impl, print_impl, println_impl,
    random_impl, read_file_impl, replace_impl, round_impl, sin_impl, sqrt_impl,
    starts_with_impl, string_contains_impl, string_split_impl, string_starts_with_impl,
    string_to_lower_impl, string_trim_impl, substring_impl, to_float_impl, to_int_impl,
    to_lower_impl, to_string_impl, to_upper_impl, trim_impl, tty_raw_mode_impl, tty_read_key_impl,
    tty_size_impl, write_file_impl,
};
mod intrinsics;
pub(crate) use intrinsics::{
    bits_to_f64, bits_to_i64, execute_intrinsic, f64_to_bits, i64_to_bits,
    value_as_bool, value_as_f64, value_as_i64,
};

/// Metadata for a lazy-loaded DBVL table with key-offset index
#[derive(Debug, Clone, PartialEq)]
pub struct DbvlTableInner {
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

#[derive(Debug, Clone)]
pub enum Value {
    Bits(Vec<u8>),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    HashMap(HashMap<String, Value>),
    HashSet(HashSet<String>),
    Instance {
        typename: String,
        fields: HashMap<String, Value>,
    },
    Enum(String, String, HashMap<String, Value>),
    Defn(String),
    Void,
    DbvlTable(Arc<DbvlTableInner>),
    Regex(crate::analysis::dfa::RegexPattern),

    Ref(Box<Value>),
    Expr(Box<crate::ast::Expr>),
    Stmt(Box<crate::ast::Statement>),
    Block(Vec<crate::ast::Statement>),
    Items(Vec<crate::ast::TopLevel>),
    Type(crate::ast::Type),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Bits(a), Value::Bits(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::HashMap(a), Value::HashMap(b)) => a == b,
            (Value::HashSet(a), Value::HashSet(b)) => a == b,
            (Value::Instance { typename: a1, fields: a2 }, Value::Instance { typename: b1, fields: b2 }) => a1 == b1 && a2 == b2,
            (Value::Enum(a1, a2, a3), Value::Enum(b1, b2, b3)) => a1 == b1 && a2 == b2 && a3 == b3,
            (Value::Defn(a), Value::Defn(b)) => a == b,
            (Value::Void, Value::Void) => true,
            (Value::DbvlTable(a), Value::DbvlTable(b)) => a == b,
            (Value::Regex(a), Value::Regex(b)) => a == b,
            (Value::Ref(a), Value::Ref(b)) => a == b,
            (Value::Expr(a), Value::Expr(b)) => a == b,
            (Value::Stmt(a), Value::Stmt(b)) => a == b,
            (Value::Block(a), Value::Block(b)) => a == b,
            (Value::Items(_), Value::Items(_)) => {
                true
            }
            (Value::Type(a), Value::Type(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bits(b) => write!(f, "<Bits {}>", b.len()),
            Value::List(items) => write!(f, "[{}]", items.len()),
            Value::Tuple(items) => write!(f, "({})", items.len()),
            Value::HashMap(map) => write!(f, "<HashMap {}>", map.len()),
            Value::HashSet(set) => write!(f, "<HashSet {}>", set.len()),
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
            Value::Ref(v) => write!(f, "&{}", v),
            Value::Expr(_) => write!(f, "<Expr>"),
            Value::Stmt(_) => write!(f, "<Stmt>"),
            Value::Block(_) => write!(f, "<Block>"),
            Value::Items(_) => write!(f, "<Items>"),
            Value::Type(t) => write!(f, "<Type {:?}>", t),
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
    /// User-defined projection not found in type bindings
    UnsupportedProjection(String),
    /// Watchdog timing bound exceeded — cycle counter hit the transaction's budget
    Timeout(String),
}

// Helper functions for JSON serialization stdlib
pub(crate) fn value_to_json_value(v: &Value) -> JsonValue {
    match v {
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
        Value::Ref(v) => value_to_json_value(v),
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
        Value::Defn(_) => JsonValue::Null,
        Value::Void => JsonValue::Null,
        Value::DbvlTable(t) => {
            let mut map = serde_json::Map::new();
            map.insert("__lazy".to_string(), JsonValue::String(t.path.clone()));
            map.insert("entries".to_string(), JsonValue::Number(t.key_offsets.len().into()));
            JsonValue::Object(map)
        }
        Value::Regex(_) => JsonValue::Null,
        Value::Bits(b) => JsonValue::Array(b.iter().map(|x| JsonValue::Number((*x).into())).collect()),
        Value::Expr(..) | Value::Stmt(..) | Value::Block(..) | Value::Items(..) | Value::Type(..) => {
            unreachable!("compile-time only value")
        }
        _ => JsonValue::Null,
    }
}

pub(crate) fn json_value_to_value(v: JsonValue) -> Value {
    match v {
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Bits(crate::interpreter::i64_to_bits(i))
            } else if let Some(f) = n.as_f64() {
                Value::Bits(crate::interpreter::f64_to_bits(f))
            } else {
                Value::Bits(crate::interpreter::i64_to_bits(0))
            }
        }
        JsonValue::String(s) => Value::Bits(s.to_string().into()),
        JsonValue::Bool(b) => Value::Bits(vec![if b { 1u8 } else { 0u8 }]),
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

/// Execute a named intrinsic on raw byte-slice arguments.
/// This is the pure-Bits dispatch engine. It operates on Value::Bits
/// byte arrays and returns Value::Bits results, with no knowledge of
/// Int/Float/Bool/etc. variants.
/// 2026-07-11: Phase 8A.2 — property-based operator dispatch.
/// Metadata for a callable declaration (defn/inop/txn).
/// Used by function metadata projections (Address, Name, etc.).
#[derive(Debug, Clone)]
struct FnMeta {
    params: Vec<Type>,
    outputs: Vec<Type>,
    span: Option<crate::errors::Span>,
    has_side_effects: bool,
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
    /// Watchdog cycle counter — incremented on every statement.
    /// When it exceeds cycle_budget, Timeout is returned.
    cycle_counter: u64,
    /// Watchdog cycle budget — the maximum number of statements before timeout.
    cycle_budget: u64,
    // 2026-07-01: Max iterations for callable txn convergence loop.
    // When exceeded, the loop returns an error instead of hanging.
    // Default 10_000 — a non-converging txn should be caught in milliseconds.
    pub txn_convergence_max_iterations: u64,
    pub type_universe: Option<crate::type_universe::TypeUniverse>,
    /// Maps variable names to their declared type annotations.
    /// Used by lookup_insert_strategy / lookup_extract_strategy to resolve
    /// the declared type name for strategy dispatch (vs the variable name).
    pub let_types: HashMap<String, crate::ast::Type>,
    /// Default watchdogs for frgn functions — wraps calls automatically.
    pub frgn_watchdogs: std::collections::HashMap<String, (u64, TimeUnit, u64, Option<Expr>)>,
    pub inop_decls: HashMap<String, InopDeclaration>,
    pub cell_defs: HashMap<String, CellDef>,
    pub next_cell_uid: usize,
    /// Registered persistent cell instances — keyed by cell name.
    /// Each instance has its own private state and ticks independently.
    pub persistent_cells: HashMap<String, PersistentCellInstance>,
    /// Trigger binding registry — maps trigger names to (cell_name, port_name)
    /// for output synchronization from persistent cells to parent state.
    pub trg_bindings: Vec<TrgBindingReg>,
    pub cell_wires: Vec<CellWire>,
    /// Handle for cell thread — joined on drop or program exit.
    pub cell_thread_handle: Option<thread::JoinHandle<()>>,
    /// Virtual heap for compile-time memory allocation.
    /// 2026-07-11: Phase 7.5 — Bits thesis.
    pub virtual_heap: VirtualHeap,
    /// Expected type for the current expression being evaluated.
    /// Set by the caller before eval_expr when the type context is known.
    /// Used by property-based operator dispatch (Phase 8B).
    /// 2026-07-11: Phase 8A.3.
    pub current_expected_type: Option<crate::ast::Type>,
}

impl Clone for Interpreter {
    fn clone(&self) -> Self {
        Interpreter {
            state: HashMap::new(),
            prior_state: HashMap::new(),
            foreign_functions: self.foreign_functions.clone(),
            definitions: self.definitions.clone(),
            callable_txns: self.callable_txns.clone(),
            ffi_bindings: self.ffi_bindings.clone(),
            ffi_name_to_location: self.ffi_name_to_location.clone(),
            orchestrator: Orchestrator::new(),
            metropolitan_hub: crate::ffi::metropolitan::MetropolitanHub::new(),
            return_value: None,
            frgn_registry: crate::ffi::dynamic::FrgnRegistry::new(),
            profile_mode: false,
            branch_counts: HashMap::new(),
            guard_counter: 0,
            enum_variants: self.enum_variants.clone(),
            dbvl_cache: HashMap::new(),
            oracle_fuel: None,
            cycle_counter: 0,
            cycle_budget: u64::MAX,
            txn_convergence_max_iterations: self.txn_convergence_max_iterations,
            type_universe: self.type_universe.clone(),
            let_types: HashMap::new(),
            frgn_watchdogs: self.frgn_watchdogs.clone(),
            inop_decls: self.inop_decls.clone(),
            cell_defs: self.cell_defs.clone(),
            next_cell_uid: self.next_cell_uid,
            persistent_cells: HashMap::new(),
            trg_bindings: Vec::new(),
            cell_wires: Vec::new(),
            cell_thread_handle: None,
            virtual_heap: self.virtual_heap.clone(),
            current_expected_type: None,
        }
    }
}


impl Interpreter {
    pub fn new() -> Self {
        let foreign_functions = ffi::load_ffi_functions();
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
            cycle_counter: 0,
            cycle_budget: u64::MAX,
            txn_convergence_max_iterations: 10_000,
            type_universe: None,
            let_types: HashMap::new(),
            frgn_watchdogs: std::collections::HashMap::new(),
            inop_decls: HashMap::new(),
            cell_defs: HashMap::new(),
            next_cell_uid: 0,
            persistent_cells: HashMap::new(),
            trg_bindings: Vec::new(),
            cell_wires: Vec::new(),
            cell_thread_handle: None,
            virtual_heap: VirtualHeap::new(),
            current_expected_type: None,
        }
    }

    // 2026-07-01: Builder method for txn convergence max iterations.
    // Allows the CLI flag --txn-convergence-max-iterations to tune this at
    // compile time without modifying the struct default.
    pub fn with_txn_convergence_max_iterations(mut self, n: u64) -> Self {
        self.txn_convergence_max_iterations = n;
        self
    }

    pub fn load_program(&mut self, program: &Program) {
        self.ffi_bindings.clear();
        self.ffi_bindings.clear();
        self.ffi_name_to_location.clear();
        self.inop_decls.clear();

        for item in &program.items {
            let inner = match item {
                TopLevel::Fuzzed { item: inner, .. }
                | TopLevel::Test { item: inner, .. } => inner.as_ref(),
                other => other,
            };
            if let TopLevel::Inop(inop) = inner {
                self.inop_decls.insert(inop.name.clone(), inop.clone());
            }
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
                    ffi::lookup_location_from_toml(&name, toml_path)
                        .unwrap_or_else(|_| signature.location.clone())
                };
                self.ffi_name_to_location.insert(name.clone(), location);
                // Store default watchdog if present
                if let Some((bound, unit, retries, fallback)) = &signature.default_watchdog {
                    self.frgn_watchdogs.insert(name.clone(), (*bound, unit.clone(), *retries, Some(fallback.as_ref().clone())));
                }
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
                                crate::ast::Type::Custom(__t) if __t == "Int" => "Int",
                                crate::ast::Type::Custom(__t) if __t == "Float" => "Float",
                                crate::ast::Type::Custom(__t) if __t == "Bool" => "Bool",
                                crate::ast::Type::Custom(__t) if __t == "Char" => "Char",
                                crate::ast::Type::Custom(__t) if __t == "String" => "String",
                                crate::ast::Type::Void => "Void",
                                _ => return None,
                            };
                            FrgnType::from_name(type_name)
                                .map(|ft| (n.clone(), ft))
                        })
                        .collect();
                    let ret = if let Some((_, t)) = signature.success_output.first() {
                            match t {
                                crate::ast::Type::Custom(__t) if __t == "Int" => FrgnType::Int,
                                crate::ast::Type::Custom(__t) if __t == "Float" => FrgnType::Float,
                                crate::ast::Type::Custom(__t) if __t == "Bool" => FrgnType::Bool,
                                crate::ast::Type::Custom(__t) if __t == "Char" => FrgnType::Char,
                                crate::ast::Type::Custom(__t) if __t == "String" => FrgnType::String,
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
            let inner = match item {
                TopLevel::Fuzzed { item: inner, .. } => inner.as_ref(),
                other => other,
            };
            if let TopLevel::Definition(defn) = inner {
                self.definitions.insert(defn.name.clone(), defn.clone());
            } else if let TopLevel::Transaction(txn) = inner {
                if !txn.is_reactive {
                    self.callable_txns.insert(txn.name.clone(), txn.clone());
                }
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

    /// Call a function by name with pre-evaluated Value arguments.
    /// Tries inop first (uses fallback expression), then regular defn.
    /// Used by Custom insert/extract strategy dispatch in arrow.rs.
    pub(crate) fn call_custom_fn(&mut self, fn_name: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
        if let Some(inop) = self.inop_decls.get(fn_name).cloned() {
            if let Some(fallback) = inop.fallback {
                let mut local_state = self.state.clone();
                for (i, (param_name, _)) in inop.params.iter().enumerate() {
                    if i < args.len() {
                        local_state.insert(param_name.clone(), args[i].clone());
                    }
                }
                let prev_state = std::mem::replace(&mut self.state, local_state);
                let result = self.eval_expr(&fallback);
                self.state = prev_state;
                return result;
            }
        }
        if let Some(defn) = self.definitions.get(fn_name).cloned() {
            let mut local_scope = self.state.clone();
            for (i, (param_name, _)) in defn.parameters.iter().enumerate() {
                if i < args.len() {
                    local_scope.insert(param_name.clone(), args[i].clone());
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
            return Ok(result);
        }
        Err(RuntimeError::TypeMismatch(format!(
            "unknown custom insert/extract function: `{}` — no inop or defn found with that name",
            fn_name
        )))
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
        let max_iterations = self.txn_convergence_max_iterations;
        let mut result = Value::Void;

        // Convergence loop: execute body while precondition holds.
        // The precondition becoming false is the convergence signal.
        // 2026-07-01: When max_iterations is exceeded, return an error instead
        // of silently breaking — non-convergence is a compile-time error, not
        // something to accept silently.
        loop {
            let pre_val = self.eval_expr(&txn.contract.pre_condition)?;
            if pre_val != Value::Bits(vec![1u8]) {
                break;
            }

            if iterations >= max_iterations {
                return Err(RuntimeError::ContractViolation(format!(
                    "txn '{}' did not converge after {} iterations \
                     (precondition remained true, state kept changing). \
                     Increase --txn-convergence-max-iterations or fix the txn.",
                    name, max_iterations
                )));
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
        if post_val != Value::Bits(vec![1u8]) {
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
            expr @ Expr::AddrOf(_) => Ok((expr.as_var_name().unwrap().to_string(), vec![])),
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
                let i = value_as_i64(&idx_val).ok_or_else(|| RuntimeError::TypeMismatch(
                    "Arrow index must be an integer".to_string()
                ))?;
                let p = if i < 0 { (list.len() as i64 + i).max(0) as usize } else { i as usize };
                Ok(Some(p))
            }
        }
    }

    /// Try to apply an InsertAt strategy from TypeUniverse for the given variable.
    /// Resolves the variable's declared type (from `let_types`) to look up
    /// the strategy in the type universe, rather than using the variable name.
    /// Falls back to None (caller uses default behavior).
    pub(crate) fn lookup_insert_strategy(&self, root_name: &str) -> Option<crate::type_universe::InsertStrategy> {
        let tu = self.type_universe.as_ref()?;
        // Resolve declared type from the type annotation, then look up strategy
        let type_name = self.let_types.get(root_name).and_then(|t| match t {
            crate::ast::Type::Custom(n) => Some(n.as_str()),
            crate::ast::Type::Applied(n, _) => Some(n.as_str()),
            _ => None,
        })?;
        tu.insert_strategy(type_name)
    }

    /// Try to apply an ExtractFrom strategy from TypeUniverse.
    pub(crate) fn lookup_extract_strategy(&self, root_name: &str) -> Option<crate::type_universe::ExtractStrategy> {
        let tu = self.type_universe.as_ref()?;
        let type_name = self.let_types.get(root_name).and_then(|t| match t {
            crate::ast::Type::Custom(n) => Some(n.as_str()),
            crate::ast::Type::Applied(n, _) => Some(n.as_str()),
            _ => None,
        })?;
        tu.extract_strategy(type_name)
    }

    /// Convert a Value to a String for use as a HashMap key.
    pub(crate) fn value_to_string(&self, val: &Value) -> Result<String, RuntimeError> {
        match val {
            Value::Bits(b) => {
                if b.len() != 8 {
                    String::from_utf8(b.clone()).map_err(|_| RuntimeError::TypeMismatch(
                        "Value cannot be used as a HashMap key".to_string()
                    ))
                } else {
                    value_as_i64(val).map(|i| i.to_string()).ok_or_else(|| RuntimeError::TypeMismatch(
                        "Value cannot be used as a HashMap key".to_string()
                    ))
                }
            }
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
                let list: &Vec<Value> = match value {
                    Value::List(items) => items,
                    Value::Tuple(items) => items,
                    _ => return Err(RuntimeError::TypeMismatch("Cannot index non-list/non-tuple in multi-slice".to_string())),
                };
                let idx_val = self.eval_expr(idx_expr)?;
                let i = value_as_i64(&idx_val).ok_or_else(|| RuntimeError::TypeMismatch("Index must be integer".to_string()))?;
                let n = if i < 0 { (list.len() as i64 + i).max(0) as usize } else { i as usize };
                if n >= list.len() {
                    return Err(RuntimeError::TypeMismatch("Index out of bounds".to_string()));
                }
                let extracted = list[n].clone();
                if rest.is_empty() { Ok(extracted) } else { self.apply_multi_slice_coords(&extracted, rest) }
            }
            SliceCoordinate::Range { start, end } => {
                let (list, is_tuple): (&Vec<Value>, bool) = match value {
                    Value::List(items) => (items, false),
                    Value::Tuple(items) => (items, true),
                    _ => return Err(RuntimeError::TypeMismatch("Cannot slice non-list/non-tuple in multi-slice".to_string())),
                };
                let len = list.len();
                let start_idx = match start {
                    Some(s) => {
                        let sv = self.eval_expr(s)?;
                        let i = value_as_i64(&sv).ok_or_else(|| RuntimeError::TypeMismatch("Range start must be integer".to_string()))?;
                        if i < 0 { (len as i64 + i).max(0) as usize } else { i as usize }
                    }
                    None => 0,
                };
                let end_idx = match end {
                    Some(e) => {
                        let ev = self.eval_expr(e)?;
                        let i = value_as_i64(&ev).ok_or_else(|| RuntimeError::TypeMismatch("Range end must be integer".to_string()))?;
                        if i < 0 { (len as i64 + i).max(0) as usize } else { i as usize }
                    }
                    None => len,
                };
                let lo = start_idx.min(len);
                let hi = end_idx.min(len);
                let sublist: Vec<Value> = if lo < hi { list[lo..hi].to_vec() } else { vec![] };
                if is_tuple {
                    if rest.is_empty() {
                        Ok(Value::Tuple(sublist))
                    } else {
                        let results: Result<Vec<Value>, RuntimeError> = sublist.iter()
                            .map(|item| self.apply_multi_slice_coords(item, rest))
                            .collect();
                        Ok(Value::Tuple(results?))
                    }
                } else {
                    if rest.is_empty() {
                        Ok(Value::List(sublist))
                    } else {
                        let results: Result<Vec<Value>, RuntimeError> = sublist.iter()
                            .map(|item| self.apply_multi_slice_coords(item, rest))
                            .collect();
                        Ok(Value::List(results?))
                    }
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
                    match &decl.ty {
                        Type::Custom(__t) if __t == "Int" => Value::Bits(i64_to_bits(0)),
                        Type::Custom(__t) if __t == "Float" => Value::Bits(f64_to_bits(0.0)),
                        Type::Custom(__t) if __t == "String" => Value::Bits(Vec::new()),
                        Type::Custom(__t) if __t == "Bool" => Value::Bits(vec![0u8]),
                        _ => Value::Void,
                    }
                };
                self.state.insert(decl.name.clone(), value);
            } else if let TopLevel::Constant(const_decl) = item {
                let value = self.eval_expr(&const_decl.expr)?;
                self.state.insert(const_decl.name.clone(), value);
            } else if let TopLevel::Fuzzed { item: inner, .. } = item {
                // Unwrap Fuzzed items — register the inner item
                match inner.as_ref() {
                    TopLevel::Definition(defn) => {
                        self.definitions.insert(defn.name.clone(), defn.clone());
                    }
                    TopLevel::Transaction(txn) => {
                        if !txn.is_reactive {
                            self.callable_txns.insert(txn.name.clone(), txn.clone());
                        }
                    }
                    TopLevel::Cell(cell) => {
                        self.cell_defs.insert(cell.name.clone(), cell.as_ref().clone());
                    }
                    _ => {}
                }
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
            } else if let TopLevel::Cell(cell) = item {
                self.cell_defs.insert(cell.name.clone(), cell.as_ref().clone());
            } else if let TopLevel::TriggerBinding { name, ty, instance, port, modifiers: _ } = item {
                // Handle trg name: Type @ CellName!.port — register cell + bind trigger
                if let Expr::Identifier(cell_name) = instance {
                    let cell_name = cell_name.clone();
                    let cell_def_opt = self.cell_defs.get(&cell_name).cloned();
                    if let Some(cell_def) = cell_def_opt {
                        let port_name = if port.is_empty() {
                            // Shorthand: auto-detect single output port
                            if let Some(ref ot) = cell_def.output_type {
                                let names = self.extract_output_names(ot);
                                names.first().cloned().unwrap_or_default()
                            } else { String::new() }
                        } else { port.clone() };
                        // Register as persistent cell if not already registered
                        if !self.persistent_cells.contains_key(&cell_name) {
                            self.register_persistent_cell(&cell_def, &[], None).unwrap_or_else(|_| {
                                String::new()
                            });
                        }
                        // Create trigger binding entry
                        self.trg_bindings.push(TrgBindingReg {
                            trigger_name: name.clone(),
                            cell_name,
                            port_name,
                            ty: ty.clone(),
                        });
                    }
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
                // Unwrap Fuzzed/Test items to access the inner transaction
                let inner_item = match item {
                    TopLevel::Fuzzed { item: inner, .. }
                    | TopLevel::Test { item: inner, .. } => inner.as_ref(),
                    other => other,
                };
                if let TopLevel::Transaction(txn) = inner_item {
                    if txn.is_reactive {
                        let pre_val = self.eval_expr(&txn.contract.pre_condition)?;
                        if pre_val == Value::Bits(vec![1u8]) {
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
                                // Commit state if postcondition is met (convergence reached).
                                // If post is not yet met, state still advances (convergent loop
                                // makes progress each tick). Only revert on error/escape.
                                if post_val == Value::Bits(vec![1u8]) {
                                    if self.state != self.prior_state {
                                        executed = true;
                                    }
                                } else {
                                    executed = true;
                                }
                            }
                            // If escaped or failed, state is already restored and we continue
                        }
                    }
            } else if let TopLevel::TypeDef(_) = item {
                // TypeDefs are compile-time only — skip at runtime.
                // Phase 1.5: type_universe.rs handles resolution in Pass 1.
            } else if let TopLevel::Fuzzed { item: inner, .. }
                     | TopLevel::Test { item: inner, groups: _ } = item {
                // Wrapper — unwrap and register the inner item's definitions
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
                            if pre_val == Value::Bits(vec![1u8]) {
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
                                    if post_val != Value::Bits(vec![1u8]) {
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

            // Tick persistent cells — each cell! gets one convergence pass per iteration
            if self.tick_persistent_cells()? {
                executed = true;
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
                    _ => return false,
                };
                if elements.len() != items.len() {
                    return false;
                }
                elements.iter().zip(items.iter()).all(|(p, v)| {
                    Self::pattern_match(p, v, state)
                })
            }
            Pattern::LitInt(n) => value_as_i64(value) == Some(*n),
            Pattern::LitFloat(f) => value_as_f64(value).map_or(false, |v| v == *f),
            Pattern::LitString(s) => matches!(value, Value::Bits(v) if v == s.as_bytes()),
            Pattern::LitChar(c) => value_as_i64(value).map_or(false, |v| v == *c as i64),
            Pattern::LitBool(b) => matches!(value, Value::Bits(v) if v.first() == Some(&(if *b { 1u8 } else { 0u8 }))),
        }
    }

    pub fn exec_stmt(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        // Decrement oracle fuel on every statement execution
        if let Some(fuel) = self.oracle_fuel.as_mut() {
            if *fuel == 0 { return Err(RuntimeError::FuelExhausted); }
            *fuel -= 1;
        }
        // Increment watchdog cycle counter and check budget
        self.cycle_counter += 1;
        if self.cycle_counter > self.cycle_budget {
            return Err(RuntimeError::Timeout(
                format!("cycle budget exceeded ({} > {})", self.cycle_counter - 1, self.cycle_budget)
            ));
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
                    Expr::Identifier(name) => {
                        self.state.insert(name.clone(), value);
                    }
                    Expr::ListIndex(list_expr, index_expr) => {
                        let list_name = match &**list_expr {
                            Expr::Identifier(n) => n.clone(),
                            _ => {
                                return Err(RuntimeError::TypeMismatch(
                                    "Expected identifier".to_string(),
                                ))
                            }
                        };
                        let idx_val = self.eval_expr(index_expr)?;
                        if let Some(idx) = value_as_i64(&idx_val) {
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
                    expr @ Expr::AddrOf(_) => {
                        let name = expr.as_var_name().ok_or_else(|| RuntimeError::TypeMismatch("AddrOf target must be an identifier".to_string()))?.to_string();
                        self.state.insert(name, value);
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
            Statement::Let { name, expr, constraint, ty, .. } => {
                if let Some(expr) = expr {
                    let value = self.eval_expr(expr)?;
                    if let Some(constraint_expr) = constraint {
                        self.eval_constraint(&value, constraint_expr)?;
                    }
                    if let Some(ann_ty) = ty {
                        self.check_type_guards(ann_ty, &value)?;
                        self.let_types.insert(name.clone(), ann_ty.clone());
                    } else {
                        self.let_types.remove(name);
                    }
                    if name != "_" {
                        self.state.insert(name.clone(), value);
                    }
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
                ..
            } => {
                let cond_val = self.eval_expr(condition)?;
                if self.profile_mode {
                    let guard_id = format!("guard_{}", self.guard_counter);
                    self.guard_counter += 1;
                    let entry = self.branch_counts.entry(guard_id).or_insert((0, 0));
                    if cond_val == Value::Bits(vec![1u8]) {
                        entry.0 += 1;
                    } else {
                        entry.1 += 1;
                    }
                }
                if cond_val == Value::Bits(vec![1u8]) {
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
            Statement::Foreach { item, list, body, .. } => {
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
            Statement::TrgBinding { name, ty, instance, port, modifiers, .. } => {
                // Extract @Hz tick rate from modifiers (e.g., modifiers=[Hashtag{name:"hz", args:["1000"]}])
                let tick_hz: Option<u64> = modifiers.iter()
                    .find(|m| m.name == "hz")
                    .and_then(|m| m.string_value())
                    .and_then(|s| s.parse().ok());
                let value = match instance {
                    Expr::Call(callee, args) if self.cell_defs.contains_key(callee) => {
                        let cell = self.cell_defs.get(callee).unwrap().clone();
                        // Phase 4: detect cell-to-cell wires in arguments
                        for (i, arg) in args.iter().enumerate() {
                            if let Expr::FieldAccess(inner, port_name) = arg {
                                if let Expr::Identifier(src_cell) = inner.as_ref() {
                                    if self.persistent_cells.contains_key(src_cell) {
                                        if let Some(param_name) = cell.parameters.get(i) {
                                            self.cell_wires.push(CellWire {
                                                from_cell: src_cell.clone(),
                                                from_port: port_name.clone(),
                                                to_cell: callee.clone(),
                                                to_param: param_name.0.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        let arg_values: Result<Vec<Value>, _> = args.iter().map(|a| {
                            // Resolve cell-to-cell references: if arg is FieldAccess(Identifier(src_cell), port),
                            // read from the source cell's cached state instead of evaluating normally.
                            if let Expr::FieldAccess(inner, port_name) = a {
                                if let Expr::Identifier(src_cell) = inner.as_ref() {
                                    if let Some(instance) = self.persistent_cells.get(src_cell) {
                                        let key = format!("{}${}.{}", src_cell, 0, port_name);
                                        return Ok(instance.state.get(&key).cloned().unwrap_or(Value::Void));
                                    }
                                }
                            }
                            self.eval_expr(a)
                        }).collect();
                        let args = arg_values?;
                        if cell.is_persistent {
                            self.register_persistent_cell(&cell, &args, tick_hz)?;
                            self.trg_bindings.push(TrgBindingReg {
                                trigger_name: name.clone(),
                                cell_name: cell.name.clone(),
                                port_name: if port.is_empty() && cell.output_type.is_some() {
                                    if let Some(ref ot) = cell.output_type {
                                        let names = self.extract_output_names(ot);
                                        names.first().cloned().unwrap_or_default()
                                    } else { String::new() }
                                } else { port.clone() },
                                ty: ty.clone(),
                            });
                            self.call_cell(&cell, &args)?
                        } else {
                            self.call_cell(&cell, &args)?
                        }
                    }
                    _ => self.eval_expr(instance)?,
                };
                self.state.insert(name.clone(), value);
            }
            Statement::Oracle { body, handler, .. } => {
                // Fuel-injected execution with state rollback on exhaustion.
                let saved_state = self.state.clone();
                let saved_prior = self.prior_state.clone();
                let saved_cycle = self.cycle_counter;
                let fuel_limit = 100;
                match self.exec_stmts_with_fuel(body, fuel_limit) {
                    Ok(()) => {}
                    Err(RuntimeError::FuelExhausted) | Err(RuntimeError::Timeout(_)) => {
                        self.state = saved_state;
                        self.prior_state = saved_prior;
                        self.cycle_counter = saved_cycle;
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
            Statement::Await { expr, .. } => {
                let value = self.eval_expr(expr)?;
                self.return_value = Some(value);
            }
            Statement::Async { body, .. } => {
                self.exec_stmt(body)?;
            }
            Statement::AsyncAwait { body, lhs, .. } => {
                if let Statement::Expression(expr) = body.as_ref() {
                    let value = self.eval_expr(expr)?;
                    if let Some(name) = lhs {
                        self.state.insert(name.clone(), value);
                    }
                } else {
                    self.exec_stmt(body)?;
                }
            }
        }
        Ok(())
    }

    /// Evaluate an expression against a specific state HashMap instead of self.state.
    pub fn eval_expr_in_state(&mut self, expr: &Expr, state: &HashMap<String, Value>) -> Result<Value, RuntimeError> {
        let saved = std::mem::replace(&mut self.state, state.clone());
        let result = self.eval_expr(expr);
        self.state = saved;
        result
    }

    /// Execute a statement against a specific state HashMap.
    pub fn exec_stmt_in_state(&mut self, stmt: &Statement, state: &mut HashMap<String, Value>, return_val: &mut Option<Value>) -> Result<(), RuntimeError> {
        let saved_state = std::mem::replace(&mut self.state, state.clone());
        let saved_return = std::mem::replace(&mut self.return_value, return_val.take());
        let result = self.exec_stmt(stmt);
        *state = std::mem::replace(&mut self.state, saved_state);
        *return_val = std::mem::replace(&mut self.return_value, saved_return);
        result
    }

    /// Check TypeUniverse guards for a given type annotation on a value.
    /// TypeDef body constraints (`type Foo <: Int { [> 0]; }`) are stored as
    /// guards on the resolved type. This evaluates each guard with `_` bound
    /// to the value.
    pub(crate) fn check_type_guards(&mut self, ty: &Type, value: &Value) -> Result<(), RuntimeError> {
        let guards: Vec<Expr> = match self.type_universe.as_ref() {
            Some(tu) => match ty {
                Type::Custom(name) => match tu.get(name) {
                    Some(r) => r.guards.clone(),
                    None => return Ok(()),
                },
                _ => return Ok(()),
            },
            None => return Ok(()),
        };
        for guard in &guards {
            self.eval_constraint(value, guard)?;
        }
        Ok(())
    }

    /// Evaluate a constraint expression with `_` bound to the given value.
    /// Returns Ok(()) if the constraint passes, Err(RuntimeError::TypeMismatch) if violated.
    pub fn eval_constraint(&mut self, value: &Value, constraint: &Expr) -> Result<(), RuntimeError> {
        let prior = self.state.insert("_".to_string(), value.clone());
        let result = self.eval_expr(constraint)?;
        match prior {
            Some(v) => { self.state.insert("_".to_string(), v); }
            None => { self.state.remove("_"); }
        }
        let ok = match result { Value::Bits(ref b) => b == &vec![1u8], _ => false };
        if ok { Ok(()) } else { Err(RuntimeError::TypeMismatch("constraint violated".into())) }
    }

    /// Dispatch a pipe-syntax frgn call.
    /// Validates the raw FFI return against the expected type T.
    /// Returns `Ok(value)` if valid, `Err(fallback_value)` if not.
    pub(crate) fn call_pipe_frgn(&mut self, fn_name: &str, raw: Value) -> Result<Value, RuntimeError> {
        let sig = match self.ffi_bindings.get(fn_name) {
            Some(s) => s.clone(),
            None => return Ok(raw),
        };
        let success_type = sig.success_output.first()
            .map(|(_, t)| t)
            .cloned()
            .unwrap_or(Type::Void);

        let is_valid = Self::is_valid_ffi_return(&raw, &success_type);

        if is_valid {
            // Ok(raw)
            let mut fields = std::collections::HashMap::new();
            fields.insert("value".to_string(), raw);
            Ok(Value::Enum("Result".to_string(), "Ok".to_string(), fields))
        } else {
            // Err(fallback_value)
            let fallback_val = match &sig.fallback {
                Some(expr) => self.eval_expr(expr)?,
                None => Value::Void,
            };
            let mut fields = std::collections::HashMap::new();
            fields.insert("value".to_string(), fallback_val);
            Ok(Value::Enum("Result".to_string(), "Err".to_string(), fields))
        }
    }

    /// Check whether a raw FFI return value constitutes a valid value of the
    /// expected type T. Used by pipe-syntax frgn declarations for sentinel-based
    /// error detection without TOML bindings.
    ///
    /// Sentinel detection applies to primitive types:
    ///   - Float: NaN/Inf → invalid
    ///   - String/Data: at the interpreter level, null pointers never reach here
    ///     (the FFI layer converts them before constructing Value). Real null
    ///     checks belong in the LLVM backend at the C ABI level.
    ///   - Int/UInt/Bool/Char: any valid value is accepted
    ///
    /// Complex types (List, Instance, Enum, etc.) returned by FFI handlers
    /// are always considered valid — the handler constructed them correctly.
    fn is_valid_ffi_return(value: &Value, expected: &Type) -> bool {
        match (value, expected) {
            // All scalar types are Bits now — accept any Bits for primitive types
            (Value::Bits(b), Type::Custom(__t))
                if __t == "Int" || __t == "UInt" || __t == "Bool"
                    || __t == "Char" || __t == "String" || __t == "Data" => true,
            // Float sentinel: NaN/Inf → invalid
            (Value::Bits(_), Type::Custom(__t)) if __t == "Float" => {
                value_as_f64(value).map_or(true, |f| f.is_finite())
            },
            // Ptr sentinel: non-zero address is valid, null (0) is invalid
            (Value::Bits(_), Type::Applied(__t, _)) if __t == "Ptr" => {
                value_as_i64(value).map_or(false, |addr| addr != 0)
            }
            (Value::Void, Type::Void) => true,
            // Complex types from FFI handlers: always valid
            (Value::List(_) | Value::Tuple(_), _) => true,
            (Value::HashMap(_) | Value::HashSet(_), _) => true,
            (Value::Instance { .. }, _) => true,
            (Value::Enum(..), _) => true,
            (Value::DbvlTable(_) | Value::Regex(_), _) => true,
            // Primitive type mismatch
            _ => false,
        }
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
            Value::Bits(d) if d.len() == 8 => {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&d[..8]);
                i64::from_le_bytes(arr) == 0
            }
            Value::Bits(d) => d.is_empty() || (d.len() == 1 && d[0] == 0),
            Value::List(l) => l.is_empty(),
            Value::Instance {
                typename: _,
                fields,
            } => fields.is_empty(),
            Value::Void => true,
            _ => false,
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            // Pattern B: delegate to feature struct
            Expr::Literal(lit) => lit.evaluate(self, &ExprDispatch),
            // Legacy scalar variants — keep inline until Phase 14 (variant removal)
            Expr::Decimal(v) => Ok(Value::Bits(i64_to_bits(*v))),
            Expr::IntegerSuffixed(v, _) => Ok(Value::Bits(i64_to_bits(*v))),
            Expr::Float(v) => Ok(Value::Bits(f64_to_bits(*v))),
            Expr::Float64(v) => Ok(Value::Bits(f64_to_bits(*v))),
            Expr::Quoted(v) => Ok(Value::Bits(v.clone())),
            Expr::RegexLiteral(v) => {
                match crate::analysis::dfa::compile_to_dfa(v) {
                    Ok(dfa) => Ok(Value::Regex(dfa)),
                    Err(e) => Err(RuntimeError::TypeMismatch(format!("Invalid regex: {}", e))),
                }
            }
            Expr::Char(v) => Ok(Value::Bits((*v as u32).to_le_bytes().to_vec())),
            Expr::Bool(v) => Ok(Value::Bits(vec![if *v { 1u8 } else { 0u8 }])),
            Expr::Term => self.state.get("term").cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable("term".to_string())),
            Expr::Identifier(name) => self.state.get(name).cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            Expr::AddrOf(inner) => {
                let val = self.eval_expr(inner)?;
                Ok(Value::Ref(Box::new(val)))
            },
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
            Expr::ArrowMut { dir, target, index, value, consume } => ArrowMutExpr { consume: *consume, 
                dir: dir.clone(), target: target.clone(), index: index.clone(), value: value.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::ArrowDiscard { target, index } => ArrowDiscardExpr {
                target: target.clone(), index: index.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::ArrowTransfer { dest, source, filter, consume } => ArrowTransferExpr { consume: *consume, 
                dest: dest.clone(), source: source.clone(), filter: filter.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::SigCall { modifier, expr } => SigCallExpr { modifier: modifier.clone(), expr: expr.clone() }.evaluate(self, &ExprDispatch),
            Expr::Ellipsis => EllipsisExpr.evaluate(self, &ExprDispatch),
            Expr::Call(name, args) => {
                // Check if this function has a default watchdog from frgn import
                if let Some((bound, unit, retries, fallback_opt)) = self.frgn_watchdogs.get(name) {
                    let fallback = fallback_opt.as_ref().cloned().unwrap_or(Expr::Decimal(0));
                    let within = Expr::Within {
                        body: Box::new(Expr::Call(name.clone(), args.clone())),
                        bound: *bound,
                        unit: unit.clone(),
                        retries: *retries,
                        fallback: Box::new(fallback),
                    };
                    self.eval_expr(&within)
                } else {
                    crate::features::call::CallExpr::new(name.clone(), args.clone()).evaluate(self, &ExprDispatch)
                }
            }
            Expr::CellCall(callee, args) => {
                let callee_name = match callee.as_ref() {
                    Expr::Identifier(name) => name.clone(),
                    other => return Err(RuntimeError::TypeMismatch(
                        format!("CellCall callee must be an identifier, got {:?}", other)
                    )),
                };
                let cell_def = self.cell_defs.get(&callee_name).ok_or_else(|| {
                    RuntimeError::TypeMismatch(format!("Cell '{}' not found", callee_name))
                })?.clone();
                let arg_values: Result<Vec<Value>, _> = args.iter().map(|a| self.eval_expr(a)).collect();
                self.call_cell(&cell_def, &arg_values?)
            }
            Expr::IntrinsicCall { intrinsic, args } => {
                let values: Result<Vec<Value>, _> = args.iter().map(|a| self.eval_expr(a)).collect();
                let mut values = values?;
                match intrinsic {
                    Intrinsic::Sqrt => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let f = bits_to_f64(&Value::Bits(b))?;
                                Ok(Value::Bits(f64_to_bits(f.sqrt())))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("sqrt requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Fabs => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let f = bits_to_f64(&Value::Bits(b))?;
                                Ok(Value::Bits(f64_to_bits(f.abs())))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("fabs requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Ceil => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let f = bits_to_f64(&Value::Bits(b))?;
                                Ok(Value::Bits(f64_to_bits(f.ceil())))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("ceil requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Floor => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let f = bits_to_f64(&Value::Bits(b))?;
                                Ok(Value::Bits(f64_to_bits(f.floor())))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("floor requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Ctpop => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let n = bits_to_i64(&Value::Bits(b))?;
                                Ok(Value::Bits(i64_to_bits(n.count_ones() as i64)))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("ctpop requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Ctlz => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let n = bits_to_i64(&Value::Bits(b))?;
                                Ok(Value::Bits(i64_to_bits(n.leading_zeros() as i64)))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("ctlz requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Cttz => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let n = bits_to_i64(&Value::Bits(b))?;
                                Ok(Value::Bits(i64_to_bits(n.trailing_zeros() as i64)))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("cttz requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Abs => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let n = bits_to_i64(&Value::Bits(b))?;
                                Ok(Value::Bits(i64_to_bits(n.abs())))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("abs requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Bitreverse => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let n = bits_to_i64(&Value::Bits(b))?;
                                Ok(Value::Bits(i64_to_bits(n.reverse_bits() as i64)))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("bitreverse requires Int, got {:?}", v))),
                        }
                    }
                    Intrinsic::Sin => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let f = bits_to_f64(&Value::Bits(b))?;
                                Ok(Value::Bits(f64_to_bits(f.sin())))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("sin# requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Cos => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let f = bits_to_f64(&Value::Bits(b))?;
                                Ok(Value::Bits(f64_to_bits(f.cos())))
                            },
                            v => Err(RuntimeError::TypeMismatch(format!("cos# requires Float, got {:?}", v))),
                        }
                    }
                    Intrinsic::Pow => {
                        let base = values.remove(0);
                        let exp = values.remove(0);
                        let b = bits_to_f64(&base)?;
                        let e = bits_to_f64(&exp)?;
                        Ok(Value::Bits(f64_to_bits(b.powf(e))))
                    }
                    Intrinsic::ByteCount => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(_) => Ok(Value::Bits(i64_to_bits(8))),
                            Value::Bits(_) => Ok(Value::Bits(i64_to_bits(1))),
                            Value::Bits(_) => Ok(Value::Bits(i64_to_bits(4))),
                            Value::Bits(_) => Ok(Value::Bits(i64_to_bits(8))),
                            Value::Bits(b) => {
                                let s = String::from_utf8_lossy(&b).to_string();
                                Ok(Value::Bits(i64_to_bits(s.len() as i64)))
                            },
                            Value::List(l) => Ok(Value::Bits(i64_to_bits((l.len() * 8) as i64))),
                            Value::Bits(d) => Ok(Value::Bits(i64_to_bits(d.len() as i64))),
                            Value::Instance { fields, .. } => Ok(Value::Bits(i64_to_bits((fields.len() * 8) as i64))),
                            Value::Tuple(t) => Ok(Value::Bits(i64_to_bits((t.len() * 8) as i64))),
                            v => Err(RuntimeError::TypeMismatch(format!("bytes not implemented for {:?}", v))),
                        }
                    }
                    Intrinsic::StrBytes => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {

                                let s = String::from_utf8_lossy(&b).to_string();
                                let bytes: Vec<Value> = s.bytes().map(|b| Value::Bits(i64_to_bits(b as i64))).collect();
                                Ok(Value::List(bytes))
                            }
                            v => Err(RuntimeError::TypeMismatch(format!("str_bytes requires String, got {:?}", v))),
                        }
                    }
                    Intrinsic::Strlen => {
                        // strlen# returns the length of a C string.
                        // In the interpreter, operates on Value::String.
                        // Used by the CString lazy lens pattern.
                        let v = values.remove(0);
                        match v {
                            Value::Bits(b) => {
                                let s = String::from_utf8_lossy(&b).to_string();
                                Ok(Value::Bits(i64_to_bits(s.len() as i64)))
                            },
                            _ => Err(RuntimeError::TypeMismatch(
                                "strlen# requires a string".into()
                            )),
                        }
                    }
                    Intrinsic::Size => {
                        let v = values.remove(0);
                        match v {
                            Value::Bits(_) => Ok(Value::Bits(i64_to_bits(1))),
                            Value::List(l) => Ok(Value::Bits(i64_to_bits(l.len() as i64))),
                            Value::Bits(b) => {
                                let s = String::from_utf8_lossy(&b).to_string();
                                Ok(Value::Bits(i64_to_bits(s.len() as i64)))
                            },
                            Value::HashMap(m) => Ok(Value::Bits(i64_to_bits(m.len() as i64))),
                            Value::HashSet(s) => Ok(Value::Bits(i64_to_bits(s.len() as i64))),
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
                            Value::List(l) => Ok(Value::Bits(vec![if (l.contains(&elem)) { 1u8 } else { 0u8 }])),
                            Value::Bits(b) => {

                                let s = String::from_utf8_lossy(&b).to_string();
                                let contains = match &elem {
                                    Value::Bits(cb) => {
                                        let c = char::from_u32(bits_to_i64(&Value::Bits(cb.clone())).unwrap_or(0) as u32);
                                        c.map(|ch| s.contains(ch)).unwrap_or(false)
                                    }
                                    _ => false,
                                };
                                if contains {
                                    Ok(Value::Bits(vec![1u8]))
                                } else {
                                    Err(RuntimeError::TypeMismatch("contains: string requires Char element".into()))
                                }
                            }
                            Value::HashMap(m) => {
                                let contains = match &elem {
                                    Value::Bits(key_bytes) => {
                                        let key = String::from_utf8_lossy(&key_bytes).to_string();
                                        m.contains_key(&key)
                                    }
                                    _ => false,
                                };
                                Ok(Value::Bits(vec![if contains { 1u8 } else { 0u8 }]))
                            }
                            Value::HashSet(s) => {
                                let contains = match &elem {
                                    Value::Bits(key_bytes) => {
                                        let key = String::from_utf8_lossy(&key_bytes).to_string();
                                        s.contains(&key)
                                    }
                                    _ => false,
                                };
                                Ok(Value::Bits(vec![if contains { 1u8 } else { 0u8 }]))
                            }
                            v => Err(RuntimeError::TypeMismatch(format!("contains requires collection, got {:?}", v))),
                        }
                    }
                    Intrinsic::Keys => {
                        let v = values.remove(0);
                        match v {
                            Value::HashMap(m) => {
                                let keys: Vec<Value> = m.into_keys().map(|s| Value::Bits(s.into_bytes())).collect();
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
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::Print => {
                        let v = values.remove(0);
                        print!("{}", v);
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::Readln => {
                        let mut buf = String::new();
                        let _ = std::io::stdin().read_line(&mut buf);
                        Ok(Value::Bits(buf.trim_end().to_string().into_bytes()))
                    }
                    Intrinsic::Exit => {
                        let code = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            _ => 0,
                        };
                        std::process::exit(code);
                    }
                    Intrinsic::VolatileLoad => {
                        let ptr = values.remove(0);
                        match ptr {
                            Value::Bits(_addr) => {
                                // Interpreter cannot read real hardware registers.
                                // Return a zero value of the appropriate type.
                                // The type is determined at compile time from the Ptr<T> type arg.
                                Ok(Value::Bits(i64_to_bits(0)))
                            }
                            v => Err(RuntimeError::TypeMismatch(
                                format!("volatile_load requires Ptr, got {:?}", v))),
                        }
                    }
                    Intrinsic::VolatileStore => {
                        let ptr = values.remove(0);
                        let _val = values.remove(0);
                        match ptr {
                            Value::Bits(_addr) => {
                                // Interpreter cannot write to real hardware registers.
                                // No-op: just return success.
                                Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                            }
                            v => Err(RuntimeError::TypeMismatch(
                                format!("volatile_store requires Ptr as first arg, got {:?}", v))),
                        }
                    }
                    Intrinsic::Halt => {
                        // No-op in interpreter — can't halt the host CPU
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::Time => {
                        use std::time::{SystemTime, UNIX_EPOCH};
                        let nanos = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos() as i64;
                        Ok(Value::Bits(i64_to_bits(nanos)))
                    }
                    Intrinsic::ReadFile => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("read_file requires String, got {:?}", v))),
                        };
                        match std::fs::read_to_string(&path) {
                            Ok(contents) => Ok(Value::Bits(contents.into_bytes())),
                            Err(e) => Err(RuntimeError::TypeMismatch(format!("{}", e))),
                        }
                    }
                    Intrinsic::WriteFile => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("write_file requires String, got {:?}", v))),
                        };
                        let data = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("write_file requires String, got {:?}", v))),
                        };
                        match std::fs::write(&path, &data) {
                            Ok(_) => Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }])),
                            Err(e) => Err(RuntimeError::TypeMismatch(format!("{}", e))),
                        }
                    }
                    Intrinsic::RingPush => {
                        // RingPush(handle, value) -> handle (unchanged for fixed-capacity ring)
                        // Interpreter stub: returns handle unchanged.
                        // LLVM codegen does actual head/tail pointer arithmetic.
                        // Full interpreter support requires VecDeque in Value system.
                        let handle = values.get(0).cloned().unwrap_or(Value::Bits(i64_to_bits(0)));
                        Ok(handle)
                    }
                    Intrinsic::RingPop => {
                        // RingPop(handle) -> value (0 if empty)
                        // Interpreter stub: always returns 0 (empty ring).
                        // LLVM codegen does actual head/tail arithmetic.
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::Sleep => {
                        let ms = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sleep requires Int, got {:?}", v))),
                        };
                        std::thread::sleep(std::time::Duration::from_millis(ms));
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::Socket => Ok(Value::Bits(i64_to_bits(-1))),
                    Intrinsic::Bind => Ok(Value::Bits(vec![0u8])),
                    Intrinsic::Listen => Ok(Value::Bits(vec![0u8])),
                    Intrinsic::Accept => Ok(Value::Bits(i64_to_bits(-1))),
                    // ===== Phase A: Terminal (intrinsics.md D4) =====
                    Intrinsic::TtyRawMode => {
                        let enable = match values.remove(0) {
                            Value::Bits(b) => value_as_bool(&Value::Bits(b)).unwrap_or(false),
                            Value::Bits(b) => value_as_bool(&Value::Bits(b)).unwrap_or(false),
                            v => return Err(RuntimeError::TypeMismatch(format!("tty_raw_mode requires Bool, got {:?}", v))),
                        };
                        let result = ffi::set_tty_raw_mode(enable);
                        Ok(Value::Bits(vec![if result {{ 1u8 }} else {{ 0u8 }}]))
                    }
                    Intrinsic::TtySize => {
                        let (cols, rows) = ffi::get_terminal_size();
                        // Pack as width * 10000 + height (same as lib/std/ffi/tty.bv)
                        Ok(Value::Bits(i64_to_bits(cols * 10000 + rows)))
                    }
                    Intrinsic::TtyReadKey => {
                        match ffi::read_key_nonblocking() {
                            Some(c) => Ok(Value::Bits(i64_to_bits(c as i64))),
                            None => Ok(Value::Bits(i64_to_bits(-1))),
                        }
                    }
                    Intrinsic::IoCtl => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("ioctl fd requires Int, got {:?}", v))),
                        };
                        let req = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("ioctl request requires Int, got {:?}", v))),
                        };
                        let arg = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("ioctl arg requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::ioctl(fd, req, arg as *mut libc::c_void) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, req, arg);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::IsTty => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("isatty fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::isatty(fd) };
                            Ok(Value::Bits(vec![if (ret != 0) { 1u8 } else { 0u8 }]))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Bits(vec![0u8]))
                        }
                    }
                    // ===== Phase A: Process (intrinsics.md D5) =====
                    Intrinsic::SpawnWithOutput => {
                        let cmd = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("spawn_with_output requires String, got {:?}", v))),
                        };
                        match std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&cmd)
                            .output()
                        {
                            Ok(output) => {
                                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                Ok(Value::Bits(stdout.into_bytes()))
                            }
                            Err(_) => Ok(Value::Bits(Vec::new())),
                        }
                    }
                    Intrinsic::Spawn => {
                        let cmd = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("spawn requires String, got {:?}", v))),
                        };
                        match std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&cmd)
                            .status()
                        {
                            Ok(status) => Ok(Value::Bits(i64_to_bits(status.code().unwrap_or(-1) as i64))),
                            Err(_) => Ok(Value::Bits(i64_to_bits(-1))),
                        }
                    }
                    Intrinsic::Argv => {
                        // argv#() returns command-line arguments as List<String>
                        // Skips argv[0] (program name) — matches argv convention
                        let args: Vec<Value> = std::env::args().skip(1)
                            .map(|a| Value::Bits(a.into_bytes()))
                            .collect();
                        Ok(Value::List(args))
                    }
                    // ===== Phase B: Raw File I/O (intrinsics.md D2) =====
                    Intrinsic::Open => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("open path requires String, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("open flags requires Int, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("open mode requires Int, got {:?}", v))),
                        };
                        let c_path = std::ffi::CString::new(path).ok();
                        let c_path = match c_path {
                            Some(p) => p,
                            None => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let fd = unsafe { libc::open(c_path.as_ptr(), flags, mode) };
                            Ok(Value::Bits(i64_to_bits(fd as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, flags, mode);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Close => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("close fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::close(fd) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Read => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("read fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("read buf requires Int, got {:?}", v))),
                        };
                        let count = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("read count requires Int, got {:?}", v))),
                        };
                        // buf is an opaque pointer — allocate temp buffer for interpreter
                        #[cfg(unix)]
                        {
                            let mut tmp = vec![0u8; count];
                            let n = unsafe { libc::read(fd, tmp.as_mut_ptr() as *mut libc::c_void, count) };
                            let _ = buf; // unused in interpreter — caller's buf is opaque
                            Ok(Value::Bits(i64_to_bits(n as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, count);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Write => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("write fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("write buf requires Int, got {:?}", v))),
                        };
                        let count = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("write count requires Int, got {:?}", v))),
                        };
                        // Interpreter can't dereference opaque buf pointer
                        let _ = (fd, buf, count);
                        Ok(Value::Bits(i64_to_bits(-1)))
                    }
                    Intrinsic::LSeek => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("lseek fd requires Int, got {:?}", v))),
                        };
                        let offset = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("lseek offset requires Int, got {:?}", v))),
                        };
                        let whence = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("lseek whence requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::lseek(fd, offset, whence) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, offset, whence);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::PRead => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("pread fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("pread buf requires Int, got {:?}", v))),
                        };
                        let count = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("pread count requires Int, got {:?}", v))),
                        };
                        let offset = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("pread offset requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut tmp = vec![0u8; count];
                            let n = unsafe { libc::pread(fd, tmp.as_mut_ptr() as *mut libc::c_void, count, offset) };
                            let _ = buf;
                            Ok(Value::Bits(i64_to_bits(n as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, count, offset);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::PWrite => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("pwrite fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("pwrite buf requires Int, got {:?}", v))),
                        };
                        let count = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("pwrite count requires Int, got {:?}", v))),
                        };
                        let offset = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("pwrite offset requires Int, got {:?}", v))),
                        };
                        let _ = (fd, buf, count, offset);
                        Ok(Value::Bits(i64_to_bits(-1)))
                    }
                    Intrinsic::Stat => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("stat path requires String, got {:?}", v))),
                        };
                        let c_path = std::ffi::CString::new(path).ok();
                        let c_path = match c_path {
                            Some(p) => p,
                            None => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let mut st: libc::stat = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::stat(c_path.as_ptr(), &mut st) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::FStat => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("fstat fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut st: libc::stat = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::fstat(fd, &mut st) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::FTruncate => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("truncate path requires String, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("truncate len requires Int, got {:?}", v))),
                        };
                        let c_path = std::ffi::CString::new(path).ok();
                        let c_path = match c_path {
                            Some(p) => p,
                            None => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::truncate(c_path.as_ptr(), len) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, len);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::FTruncate => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("ftruncate fd requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("ftruncate len requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::ftruncate(fd, len) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, len);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::FSync => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("fsync fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::fsync(fd) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::FDup => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("dup fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::dup(fd) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::FDup2 => {
                        let old = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("dup2 old requires Int, got {:?}", v))),
                        };
                        let new = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("dup2 new requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::dup2(old, new) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (old, new);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::FCntl => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("fcntl fd requires Int, got {:?}", v))),
                        };
                        let cmd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("fcntl cmd requires Int, got {:?}", v))),
                        };
                        let arg = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("fcntl arg requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::fcntl(fd, cmd, arg) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, cmd, arg);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    // ===== Phase C: Filesystem (intrinsics.md D3) =====
                    Intrinsic::MkDir => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("mkdir path requires String, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mkdir mode requires Int, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mkdir(c_path.as_ptr(), mode) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, mode);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::RmDir => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("rmdir path requires String, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::rmdir(c_path.as_ptr()) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Unlink => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("unlink path requires String, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::unlink(c_path.as_ptr()) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Rename => {
                        let old = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("rename old requires String, got {:?}", v))),
                        };
                        let new = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("rename new requires String, got {:?}", v))),
                        };
                        let c_old = match std::ffi::CString::new(old) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        let c_new = match std::ffi::CString::new(new) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::rename(c_old.as_ptr(), c_new.as_ptr()) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_old, c_new);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SymLink => {
                        let target = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("symlink target requires String, got {:?}", v))),
                        };
                        let link = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("symlink link requires String, got {:?}", v))),
                        };
                        let c_target = match std::ffi::CString::new(target) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        let c_link = match std::ffi::CString::new(link) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::symlink(c_target.as_ptr(), c_link.as_ptr()) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_target, c_link);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::ReadLink => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("readlink path requires String, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(Vec::new())),
                        };
                        #[cfg(unix)]
                        {
                            let mut buf = vec![0u8; 4096];
                            let n = unsafe {
                                libc::readlink(c_path.as_ptr(), buf.as_mut_ptr() as *mut libc::c_char, 4096)
                            };
                            if n < 0 {
                                Ok(Value::Bits(Vec::new()))
                            } else {
                                buf.truncate(n as usize);
                                let s = String::from_utf8_lossy(&buf).to_string();
                                Ok(Value::Bits(s.into_bytes()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Bits(Vec::new()))
                        }
                    }
                    Intrinsic::Link => {
                        let old = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("link old requires String, got {:?}", v))),
                        };
                        let new = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("link new requires String, got {:?}", v))),
                        };
                        let c_old = match std::ffi::CString::new(old) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        let c_new = match std::ffi::CString::new(new) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::link(c_old.as_ptr(), c_new.as_ptr()) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_old, c_new);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::GetCwd => {
                        #[cfg(unix)]
                        {
                            let mut buf = vec![0u8; 4096];
                            let ptr = unsafe { libc::getcwd(buf.as_mut_ptr() as *mut libc::c_char, 4096) };
                            if ptr.is_null() {
                                Ok(Value::Bits(Vec::new()))
                            } else {
                                let len = unsafe { libc::strlen(ptr) };
                                buf.truncate(len);
                                let s = String::from_utf8_lossy(&buf).to_string();
                                Ok(Value::Bits(s.into_bytes()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(Vec::new()))
                        }
                    }
                    Intrinsic::ChDir => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("chdir path requires String, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::chdir(c_path.as_ptr()) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_path;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::ReadDir => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("readdir path requires String, got {:?}", v))),
                        };
                        match std::fs::read_dir(&path) {
                            Ok(entries) => {
                                let mut list = Vec::new();
                                for entry in entries.flatten() {
                                    if let Ok(name) = entry.file_name().into_string() {
                                        list.push(Value::Bits(name.into_bytes()));
                                    }
                                }
                                Ok(Value::List(list))
                            }
                            Err(_) => Ok(Value::List(Vec::new())),
                        }
                    }
                    Intrinsic::ChMod => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("chmod path requires String, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("chmod mode requires Int, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::chmod(c_path.as_ptr(), mode) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, mode);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::ChOwn => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("chown path requires String, got {:?}", v))),
                        };
                        let uid = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("chown uid requires Int, got {:?}", v))),
                        };
                        let gid = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("chown gid requires Int, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::chown(c_path.as_ptr(), uid, gid) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, uid, gid);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::UMask => {
                        let mask = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("umask mask requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let old = unsafe { libc::umask(mask) };
                            Ok(Value::Bits(i64_to_bits(old as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = mask;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::Access => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("access path requires String, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("access mode requires Int, got {:?}", v))),
                        };
                        let c_path = match std::ffi::CString::new(path) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::access(c_path.as_ptr(), mode) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_path, mode);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    // ===== Phase D: Memory (intrinsics.md D1) =====
                    Intrinsic::Mmap => {
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap addr requires Int, got {:?}", v))),
                        };
                        let length = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap length requires Int, got {:?}", v))),
                        };
                        let prot = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap prot requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap flags requires Int, got {:?}", v))),
                        };
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap fd requires Int, got {:?}", v))),
                        };
                        let offset = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mmap offset requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mmap(addr as *mut libc::c_void, length, prot, flags, fd, offset) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (addr, length, prot, flags, fd, offset);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::MUnmap => {
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as *mut libc::c_void,
                            v => return Err(RuntimeError::TypeMismatch(format!("munmap addr requires Int, got {:?}", v))),
                        };
                        let length = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("munmap length requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::munmap(addr, length) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (addr, length);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::MProtect => {
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as *mut libc::c_void,
                            v => return Err(RuntimeError::TypeMismatch(format!("mprotect addr requires Int, got {:?}", v))),
                        };
                        let length = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mprotect length requires Int, got {:?}", v))),
                        };
                        let prot = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mprotect prot requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mprotect(addr, length, prot) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (addr, length, prot);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Brk => {
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as *mut libc::c_void,
                            v => return Err(RuntimeError::TypeMismatch(format!("brk addr requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sbrk(0) }; // get current brk
                            let _ = addr; // setting brk is unsafe; just return current
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = addr;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::MLock => {
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as *mut libc::c_void,
                            v => return Err(RuntimeError::TypeMismatch(format!("mlock addr requires Int, got {:?}", v))),
                        };
                        let length = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("mlock length requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mlock(addr, length) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (addr, length);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    // ===== Phase D: Synchronization (intrinsics.md D9) =====
                    // These operate on opaque memory addresses. The interpreter
                    // stubs them — they only work meaningfully in compiled code.
                    Intrinsic::AtomicLoad => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // order
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::AtomicStore => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // val
                        let _ = values.remove(0); // order
                        Ok(Value::Bits(i64_to_bits(-1)))
                    }
                    Intrinsic::AtomicCas => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // expected
                        let _ = values.remove(0); // new
                        let _ = values.remove(0); // order
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::AtomicXchg => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // val
                        let _ = values.remove(0); // order
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::AtomicAdd => {
                        let _ = values.remove(0); // addr
                        let _ = values.remove(0); // val
                        let _ = values.remove(0); // order
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::Fence => {
                        let _ = values.remove(0); // order
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::Futex => {
                        let _ = values.remove(0); // uaddr
                        let _ = values.remove(0); // op
                        let _ = values.remove(0); // val
                        let _ = values.remove(0); // timeout
                        let _ = values.remove(0); // uaddr2
                        let _ = values.remove(0); // val3
                        Ok(Value::Bits(i64_to_bits(-1)))
                    }
                    // ===== Phase E: IPC (intrinsics.md D11) =====
                    Intrinsic::Pipe => {
                        let fds = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
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
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::ShmOpen => {
                        let name = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("shm_open name requires String, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("shm_open flags requires Int, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("shm_open mode requires Int, got {:?}", v))),
                        };
                        let c_name = match std::ffi::CString::new(name) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let fd = unsafe { libc::shm_open(c_name.as_ptr(), flags, mode) };
                            Ok(Value::Bits(i64_to_bits(fd as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_name, flags, mode);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::ShmUnlink => {
                        let name = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("shm_unlink name requires String, got {:?}", v))),
                        };
                        let c_name = match std::ffi::CString::new(name) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::shm_unlink(c_name.as_ptr()) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = c_name;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SemOpen => {
                        let name = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_open name requires String, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_open flags requires Int, got {:?}", v))),
                        };
                        let mode = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_open mode requires Int, got {:?}", v))),
                        };
                        let value = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_open value requires Int, got {:?}", v))),
                        };
                        let c_name = match std::ffi::CString::new(name) {
                            Ok(p) => p,
                            _ => return Ok(Value::Bits(i64_to_bits(-1))),
                        };
                        #[cfg(unix)]
                        {
                            let sem = unsafe { libc::sem_open(c_name.as_ptr(), flags, mode, value) };
                            Ok(Value::Bits(i64_to_bits(sem as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (c_name, flags, mode, value);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SemWait => {
                        let sem = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as *mut libc::sem_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_wait sem requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sem_wait(sem) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = sem;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SemPost => {
                        let sem = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as *mut libc::sem_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("sem_post sem requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sem_post(sem) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = sem;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    // ===== Phase F: Signals (intrinsics.md D8) =====
                    Intrinsic::SigAction => {
                        let signum = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sigaction signum requires Int, got {:?}", v))),
                        };
                        let handler = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
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
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (signum, handler);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SigProcMask => {
                        let how = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sigprocmask how requires Int, got {:?}", v))),
                        };
                        let mask = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("sigprocmask mask requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::sigprocmask(how, &set as *const _ as *const libc::sigset_t, std::ptr::null_mut()) };
                            let _ = mask;
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (how, mask);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Kill => {
                        let pid = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("kill pid requires Int, got {:?}", v))),
                        };
                        let sig = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("kill sig requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::kill(pid, sig) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (pid, sig);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SignalFd => {
                        let mask = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("signalfd mask requires Int, got {:?}", v))),
                        };
                        #[cfg(target_os = "linux")]
                        {
                            let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::signalfd(-1, &set, libc::SFD_NONBLOCK) };
                            let _ = mask;
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            let _ = mask;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::TimerFdCreate => {
                        let hz = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
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
                            Ok(Value::Bits(i64_to_bits(fd as i64)))
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            let _ = hz;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    // ===== Phase G: Networking (intrinsics.md D10) — Shim =====
                    Intrinsic::Socket => {
                        let domain = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("socket domain requires Int, got {:?}", v))),
                        };
                        let sock_type = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("socket type requires Int, got {:?}", v))),
                        };
                        let protocol = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("socket protocol requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::socket(domain, sock_type, protocol) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (domain, sock_type, protocol);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Bind => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("bind fd requires Int, got {:?}", v))),
                        };
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("bind addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("bind addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::bind(fd, addr as *const libc::sockaddr, addrlen) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, addr, addrlen);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Listen => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("listen fd requires Int, got {:?}", v))),
                        };
                        let backlog = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("listen backlog requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::listen(fd, backlog) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, backlog);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Accept => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("accept fd requires Int, got {:?}", v))),
                        };
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("accept addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("accept addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::accept(fd, addr as *mut libc::sockaddr, &addrlen as *const u32 as *mut libc::socklen_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, addr, addrlen);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Connect => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("connect fd requires Int, got {:?}", v))),
                        };
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("connect addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("connect addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::connect(fd, addr as *const libc::sockaddr, addrlen) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, addr, addrlen);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Send => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("send fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("send buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("send len requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("send flags requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::send(fd, buf as *const std::ffi::c_void, len, flags) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, len, flags);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Recv => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("recv fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("recv buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("recv len requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("recv flags requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::recv(fd, buf as *mut std::ffi::c_void, len, flags) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, len, flags);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SendTo => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto len requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto flags requires Int, got {:?}", v))),
                        };
                        let dest_addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto dest_addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("sendto addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sendto(fd, buf as *const std::ffi::c_void, len, flags, dest_addr as *const libc::sockaddr, addrlen) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, len, flags, dest_addr, addrlen);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::RecvFrom => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom fd requires Int, got {:?}", v))),
                        };
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom len requires Int, got {:?}", v))),
                        };
                        let flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom flags requires Int, got {:?}", v))),
                        };
                        let src_addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as libc::uintptr_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom src_addr requires Int, got {:?}", v))),
                        };
                        let addrlen = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("recvfrom addrlen requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::recvfrom(fd, buf as *mut std::ffi::c_void, len, flags, src_addr as *mut libc::sockaddr, &addrlen as *const u32 as *mut libc::socklen_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, buf, len, flags, src_addr, addrlen);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SetSockOpt => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt fd requires Int, got {:?}", v))),
                        };
                        let level = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt level requires Int, got {:?}", v))),
                        };
                        let opt = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt opt requires Int, got {:?}", v))),
                        };
                        let val = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt val requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("setsockopt len requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::setsockopt(fd, level, opt, val as *const std::ffi::c_void, len) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, level, opt, val, len);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::GetSockOpt => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt fd requires Int, got {:?}", v))),
                        };
                        let level = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt level requires Int, got {:?}", v))),
                        };
                        let opt = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt opt requires Int, got {:?}", v))),
                        };
                        let val = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt val requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getsockopt len requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::getsockopt(fd, level, opt, val as *mut std::ffi::c_void, &len as *const u32 as *mut libc::socklen_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, level, opt, val, len);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::Shutdown => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("shutdown fd requires Int, got {:?}", v))),
                        };
                        let how = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("shutdown how requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::shutdown(fd, how) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (fd, how);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::GetAddrInfo => {
                        let node = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("getaddrinfo node requires String, got {:?}", v))),
                        };
                        let service = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
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
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = (node, service);
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    // ===== Phase H: Everything Else (intrinsics.md D6, D7) =====
                    Intrinsic::GetEnv => {
                        let name = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("getenv name requires String, got {:?}", v))),
                        };
                        match std::env::var(&name) {
                            Ok(val) => Ok(Value::Bits(val.into_bytes())),
                            Err(_) => Ok(Value::Bits(Vec::new())),
                        }
                    }
                    Intrinsic::SetEnv => {
                        let name = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("setenv name requires String, got {:?}", v))),
                        };
                        let value = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("setenv value requires String, got {:?}", v))),
                        };
                        unsafe { std::env::set_var(&name, &value); }
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::UnsetEnv => {
                        let name = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("unsetenv name requires String, got {:?}", v))),
                        };
                        unsafe { std::env::remove_var(&name); }
                        Ok(Value::Bits(i64_to_bits(0)))
                    }
                    Intrinsic::GetPid => {
                        #[cfg(unix)]
                        {
                            let pid = unsafe { libc::getpid() };
                            Ok(Value::Bits(i64_to_bits(pid as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::GetPPid => {
                        #[cfg(unix)]
                        {
                            let ppid = unsafe { libc::getppid() };
                            Ok(Value::Bits(i64_to_bits(ppid as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::ClockGetTime => {
                        let clock_id = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("clock_gettime clock_id requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::clock_gettime(clock_id, &mut ts) };
                            if ret == 0 {
                                let nanos = ts.tv_sec * 1_000_000_000 + ts.tv_nsec;
                                Ok(Value::Bits(i64_to_bits(nanos)))
                            } else {
                                Ok(Value::Bits(i64_to_bits(0)))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = clock_id;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::NanoSleep => {
                        let ns = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
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
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            std::thread::sleep(std::time::Duration::from_nanos(ns as u64));
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    // Data intrinsics
                    Intrinsic::Sort => Ok(values.remove(0)),
                    Intrinsic::Reverse => Ok(values.remove(0)),
                    Intrinsic::Range => {
                        let end = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("range requires Int, got {:?}", v))),
                        };
                        let list: Vec<Value> = (0..end).map(|i| Value::Bits(i64_to_bits(i))).collect();
                        Ok(Value::List(list))
                    }
                    // String intrinsics (2026-06-18)
                    Intrinsic::TrimLeft => {
                        let s = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("trim_left requires String, got {:?}", v))),
                        };
                        Ok(Value::Bits(s.trim_start().to_string().into_bytes()))
                    }
                    Intrinsic::TrimRight => {
                        let s = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("trim_right requires String, got {:?}", v))),
                        };
                        Ok(Value::Bits(s.trim_end().to_string().into_bytes()))
                    }
                    Intrinsic::ToLower => {
                        let s = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("to_lower requires String, got {:?}", v))),
                        };
                        Ok(Value::Bits(s.to_lowercase().into_bytes()))
                    }
                    Intrinsic::ContainsAt => {
                        let haystack = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("contains_at haystack requires String, got {:?}", v))),
                        };
                        let needle = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("contains_at needle requires String, got {:?}", v))),
                        };
                        let start = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("contains_at start requires Int, got {:?}", v))),
                        };
                        if start < 0 || (start as usize) >= haystack.len() {
                            return Ok(Value::Bits(vec![0u8]));
                        }
                        Ok(Value::Bits(vec![if (haystack[(start as usize)..].contains(&needle)) { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::FindFrom => {
                        let s = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("find_from s requires String, got {:?}", v))),
                        };
                        let needle = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("find_from needle requires String, got {:?}", v))),
                        };
                        let start = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("find_from start requires Int, got {:?}", v))),
                        };
                        if start < 0 || (start as usize) >= s.len() {
                            return Ok(Value::Bits(i64_to_bits(-1)));
                        }
                        match s[(start as usize)..].find(&needle) {
                            Some(idx) => Ok(Value::Bits(i64_to_bits((start as usize + idx) as i64))),
                            None => Ok(Value::Bits(i64_to_bits(-1))),
                        }
                    }
                    Intrinsic::SplitN => {
                        let s = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("splitn s requires String, got {:?}", v))),
                        };
                        let delim = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("splitn delim requires String, got {:?}", v))),
                        };
                        let _n = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("splitn n requires Int, got {:?}", v))),
                        };
                        let parts: Vec<Value> = if delim.is_empty() {
                            s.chars().map(|c| Value::Bits(c.to_string().into_bytes())).collect()
                        } else {
                            s.split(&delim).map(|p| Value::Bits(p.to_string().into())).collect()
                        };
                        Ok(Value::List(parts))
                    }
                    Intrinsic::IntToStr => {
                        let n = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("int_to_str requires Int, got {:?}", v))),
                        };
                        Ok(Value::Bits(n.to_string().into_bytes()))
                    }
                    Intrinsic::FloatToStr => {
                        let f = match values.remove(0) {
                            Value::Bits(b) => bits_to_f64(&Value::Bits(b)).unwrap_or(0.0),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("float_to_str requires Float, got {:?}", v))),
                        };
                        Ok(Value::Bits(format!("[{:.9}]", f).into_bytes()))
                    }
                    Intrinsic::ToStr => {
                        let v = values.remove(0);
                        let s = match &v {
                            Value::Bits(b) if b.len() == 8 => {
                                let n = bits_to_i64(&v);
                                match n {
                                    Ok(n) => n.to_string(),
                                    Err(_) => {
                                        let f = bits_to_f64(&v).unwrap_or(0.0);
                                        format!("{:.9}", f)
                                    }
                                }
                            }
                            Value::Bits(b) => String::from_utf8_lossy(b).to_string(),
                            _ => return Err(RuntimeError::TypeMismatch(
                                format!("to_str requires Int|Float|Char|Bool|String, got {:?}", v))),
                        };
                        Ok(Value::Bits(s.into_bytes()))
                    }
                    // Benchmark intrinsics (2026-06-16)
                    Intrinsic::PrintInt => {
                        let n = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("print_int requires Int, got {:?}", v))),
                        };
                        print!("{}", n);
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::PutChar => {
                        let val = values.remove(0);
                        let c = bits_to_i64(&val).unwrap_or(0) as u8 as char;
                        print!("{}", c);
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::PrintFloat => {
                        let d = match values.remove(0) {
                            Value::Bits(b) => bits_to_f64(&Value::Bits(b)).unwrap_or(0.0),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("print_float requires Float, got {:?}", v))),
                        };
                        print!("{:.9}", d);
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::GetEnvInt => {
                        let name = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("getenv_int requires String, got {:?}", v))),
                        };
                        let val = std::env::var(&name)
                            .ok()
                            .and_then(|s| s.parse::<i64>().ok())
                            .unwrap_or(0);
                        Ok(Value::Bits(i64_to_bits(val)))
                    }
                    Intrinsic::SetStdoutBuf => {
                        let mode = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("set_stdout_buf requires Int, got {:?}", v))),
                        };
                        // mode: 0=_IOFBF, 1=_IOLBF, 2=_IONBF
                        // Rust's stdio doesn't expose setvbuf — inform user
                        eprintln!("note: set_stdout_buf#({}) is a no-op in interpreter mode", mode);
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::Compile => {
                        let code = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("compile# requires String, got {:?}", v))),
                        };
                        // Parse the code string as Brief source
                        let mut parser = crate::parser::Parser::new(&code);
                        match parser.parse() {
                            Ok(prog) => {
                                // 2026-06-28: Return all items as Value::Items so the
                                // macro expander can inject them at the top level.
                                Ok(Value::Items(prog.items))
                            }
                            Err(e) => Err(RuntimeError::TypeMismatch(
                                format!("compile#: parse error: {}", e))),
                        }
                    }
                    Intrinsic::MacroError => {
                        let msg = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("error# requires String, got {:?}", v))),
                        };
                        return Err(RuntimeError::TypeMismatch(
                            format!("compile-time macro error: {}", msg)));
                    }
                    Intrinsic::MacroWarn => {
                        let msg = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("warn# requires String, got {:?}", v))),
                        };
                        eprintln!("warning: {}", msg);
                        Ok(Value::Void)
                    }
                    Intrinsic::MacroGenSym => {
                        // gensym#() without a macro context returns a runtime fallback
                        let sym = format!("__gensym_rt_{}", std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos());
                        Ok(Value::Bits(sym.into_bytes()))
                    }
                    //
                    // emit_file#(filename: String, content: String) — write file at compile time.
                    // Used by GLUE adapter macros to emit native language source files into the
                    // compiler's output directory during `$!macro` expansion.
                    // Takes (filename, content) as two String arguments. Returns Void on success,
                    // propagates I/O errors as RuntimeError.
                    //
                    // Architecture rationale: GLUE adapters are Brief `$!` macros, not Rust template
                    // engines. Adding a language = writing one `.bv` file. The emit_file# intrinsic
                    // is the bridge between the macro system and the filesystem — without it, adapters
                    // would need a Rust-side template engine, which will be thrown away during
                    // self-hosting. By keeping file emission as a compile-time intrinsic, the entire
                    // adapter pipeline stays in Brief code.
                    //
                    Intrinsic::EmitFile => {
                        let filename = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("emit_file# requires String filename, got {:?}", v))),
                        };
                        let content = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("emit_file# requires String content, got {:?}", v))),
                        };
                        // Determine output directory: use --out if provided, else cwd
                        let out_dir = std::env::var("BRIEF_OUTPUT_DIR")
                            .unwrap_or_else(|_| ".".to_string());
                        let path = std::path::Path::new(&out_dir).join(&filename);
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        match std::fs::write(&path, &content) {
                            Ok(_) => Ok(Value::Void),
                            Err(e) => Err(RuntimeError::TypeMismatch(
                                format!("emit_file#: failed to write {}: {}", filename, e))),
                        }
                    }
                    // GPU compute intrinsics — interpreter stubs (no GPU simulation)
                    // These accept their standard dimension arguments for validation
                    // but return constant values since the interpreter cannot simulate a GPU.
                    Intrinsic::GetGlobalId => {
                        let dim = values.remove(0);
                        let d = value_as_i64(&dim).ok_or_else(|| RuntimeError::TypeMismatch(
                            format!("get_global_id expects Int dimension, got {:?}", dim)))?;
                        if d >= 0 && d < 3 { Ok(Value::Bits(i64_to_bits(0))) }
                        else { Err(RuntimeError::TypeMismatch(
                            format!("get_global_id: dimension {} out of range [0,2]", d))) }
                    }
                    Intrinsic::GetLocalId => {
                        let dim = values.remove(0);
                        let d = value_as_i64(&dim).ok_or_else(|| RuntimeError::TypeMismatch(
                            format!("get_local_id expects Int dimension, got {:?}", dim)))?;
                        if d >= 0 && d < 3 { Ok(Value::Bits(i64_to_bits(0))) }
                        else { Err(RuntimeError::TypeMismatch(
                            format!("get_local_id: dimension {} out of range [0,2]", d))) }
                    }
                    Intrinsic::GetGroupId => {
                        let dim = values.remove(0);
                        let d = value_as_i64(&dim).ok_or_else(|| RuntimeError::TypeMismatch(
                            format!("get_group_id expects Int dimension, got {:?}", dim)))?;
                        if d >= 0 && d < 3 { Ok(Value::Bits(i64_to_bits(0))) }
                        else { Err(RuntimeError::TypeMismatch(
                            format!("get_group_id: dimension {} out of range [0,2]", d))) }
                    }
                    Intrinsic::GetNumGroups => {
                        let dim = values.remove(0);
                        let d = value_as_i64(&dim).ok_or_else(|| RuntimeError::TypeMismatch(
                            format!("get_num_groups expects Int dimension, got {:?}", dim)))?;
                        if d >= 0 && d < 3 { Ok(Value::Bits(i64_to_bits(1))) }
                        else { Err(RuntimeError::TypeMismatch(
                            format!("get_num_groups: dimension {} out of range [0,2]", d))) }
                    }
                    Intrinsic::SubGroupBarrier => {
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    // ===== D12: Random / Entropy (2026-06-19) =====
                    Intrinsic::Errno => {
                        #[cfg(unix)]
                        {
                            let err = unsafe { *libc::__errno_location() };
                            Ok(Value::Bits(i64_to_bits(err as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::GetRandom => {
                        let buf = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("getrandom buf requires Int, got {:?}", v))),
                        };
                        let len = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as usize).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getrandom len requires Int, got {:?}", v))),
                        };
                        let _flags = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getrandom flags requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::getrandom(buf as *mut libc::c_void, len, _flags) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = buf; let _ = len;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    // ===== D13: System Info (2026-06-19) =====
                    Intrinsic::Uname => {
                        #[cfg(unix)]
                        {
                            let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::uname(&mut uts) };
                            if ret == 0 {
                                let sysname = unsafe { std::ffi::CStr::from_ptr(uts.sysname.as_ptr()).to_string_lossy().to_string() };
                                let release = unsafe { std::ffi::CStr::from_ptr(uts.release.as_ptr()).to_string_lossy().to_string() };
                                let version = unsafe { std::ffi::CStr::from_ptr(uts.version.as_ptr()).to_string_lossy().to_string() };
                                let machine = unsafe { std::ffi::CStr::from_ptr(uts.machine.as_ptr()).to_string_lossy().to_string() };
                                Ok(Value::Bits(format!("[{}:{}:{}:{}]", sysname, release, version, machine).into_bytes()))
                            } else {
                                Ok(Value::Bits(Vec::new()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(Vec::new()))
                        }
                    }
                    Intrinsic::PageSize => {
                        #[cfg(unix)]
                        {
                            let ps = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
                            Ok(Value::Bits(i64_to_bits(ps as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(4096)))
                        }
                    }
                    Intrinsic::CpuCount => {
                        #[cfg(unix)]
                        {
                            let ncpu = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
                            Ok(Value::Bits(i64_to_bits(ncpu as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(1)))
                        }
                    }
                    Intrinsic::Hostname => {
                        #[cfg(unix)]
                        {
                            let mut buf = vec![0u8; 256];
                            let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, 256) };
                            if ret == 0 {
                                let name = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const libc::c_char).to_string_lossy().to_string() };
                                Ok(Value::Bits(name.into_bytes()))
                            } else {
                                Ok(Value::Bits(Vec::new()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits("localhost".to_string().into_bytes()))
                        }
                    }
                    Intrinsic::StrError => {
                        let errnum = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("strerror errnum requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut buf = vec![0u8; 1024];
                            let ret = unsafe { libc::strerror_r(errnum, buf.as_mut_ptr() as *mut libc::c_char, 1024) };
                            let msg = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const libc::c_char).to_string_lossy().to_string() };
                            if ret == 0 { Ok(Value::Bits(msg.into_bytes())) } else { Ok(Value::Bits(format!("[Unknown error {}]", errnum).into_bytes())) }
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(format!("[error {}]", errnum).into_bytes()))
                        }
                    }
                    Intrinsic::StrSignal => {
                        let signum = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("strsignal signum requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let msg = unsafe { std::ffi::CStr::from_ptr(libc::strsignal(signum)).to_string_lossy().to_string() };
                            Ok(Value::Bits(msg.into_bytes()))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(format!("[signal {}]", signum).into_bytes()))
                        }
                    }
                    Intrinsic::RealPath => {
                        let path = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("realpath path requires String, got {:?}", v))),
                        };
                        let cpath = std::ffi::CString::new(path.as_bytes()).unwrap_or_default();
                        #[cfg(unix)]
                        {
                            let resolved = unsafe { libc::realpath(cpath.as_ptr(), std::ptr::null_mut()) };
                            if !resolved.is_null() {
                                let result = unsafe { std::ffi::CStr::from_ptr(resolved).to_string_lossy().to_string() };
                                unsafe { libc::free(resolved as *mut libc::c_void); }
                                Ok(Value::Bits(result.into_bytes()))
                            } else {
                                Ok(Value::Bits(Vec::new()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(path.into_bytes()))
                        }
                    }
                    // ===== D14: Debugging (2026-06-19) =====
                    Intrinsic::Abort => {
                        #[cfg(unix)]
                        { unsafe { libc::abort(); } }
                        #[cfg(not(unix))]
                        { std::process::abort(); }
                        // unreachable
                        #[allow(unreachable_code)]
                        Ok(Value::Void)
                    }
                    Intrinsic::Backtrace => {
                        #[cfg(all(unix, not(target_os = "linux")))]
                        {
                            Ok(Value::List(Vec::new()))
                        }
                        #[cfg(target_os = "linux")]
                        {
                            let mut addrs: Vec<*mut libc::c_void> = vec![std::ptr::null_mut(); 128];
                            let count = unsafe { libc::backtrace(addrs.as_mut_ptr(), 128) };
                            if count > 0 {
                                let frames: Vec<Value> = addrs[..count as usize].iter().map(|&addr| Value::Bits(i64_to_bits(addr as i64))).collect();
                                Ok(Value::List(frames))
                            } else {
                                Ok(Value::List(Vec::new()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::List(Vec::new()))
                        }
                    }
                    // ===== D15: Scheduling (2026-06-19) =====
                    Intrinsic::SchedYield => {
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::sched_yield() };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::GetPriority => {
                        let which = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getpriority which requires Int, got {:?}", v))),
                        };
                        let who = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getpriority who requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::getpriority(which as u32, who as libc::id_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = which; let _ = who;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::SetPriority => {
                        let which = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("setpriority which requires Int, got {:?}", v))),
                        };
                        let who = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("setpriority who requires Int, got {:?}", v))),
                        };
                        let prio = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("setpriority prio requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::setpriority(which as u32, who as libc::id_t, prio) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = which; let _ = who; let _ = prio;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    // ===== D16: User / Group (2026-06-19) =====
                    Intrinsic::GetUid => {
                        #[cfg(unix)]
                        {
                            let uid = unsafe { libc::getuid() };
                            Ok(Value::Bits(i64_to_bits(uid as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::GetEUid => {
                        #[cfg(unix)]
                        {
                            let euid = unsafe { libc::geteuid() };
                            Ok(Value::Bits(i64_to_bits(euid as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::GetGid => {
                        #[cfg(unix)]
                        {
                            let gid = unsafe { libc::getgid() };
                            Ok(Value::Bits(i64_to_bits(gid as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::GetEGid => {
                        #[cfg(unix)]
                        {
                            let egid = unsafe { libc::getegid() };
                            Ok(Value::Bits(i64_to_bits(egid as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::GetPwUid => {
                        let uid = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as libc::uid_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("getpwuid uid requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
                            let mut buf = vec![0u8; 4096];
                            let mut result: *mut libc::passwd = std::ptr::null_mut();
                            let ret = unsafe { libc::getpwuid_r(uid, &mut pw, buf.as_mut_ptr() as *mut libc::c_char, 4096, &mut result) };
                            if ret == 0 && !result.is_null() {
                                let name = unsafe { std::ffi::CStr::from_ptr(pw.pw_name).to_string_lossy().to_string() };
                                let dir = unsafe { std::ffi::CStr::from_ptr(pw.pw_dir).to_string_lossy().to_string() };
                                let shell = unsafe { std::ffi::CStr::from_ptr(pw.pw_shell).to_string_lossy().to_string() };
                                Ok(Value::Bits(format!("[{}:{}:{}]", name, dir, shell).into_bytes()))
                            } else {
                                Ok(Value::Bits(Vec::new()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = pw; let _ = buf; let _ = result;
                            Ok(Value::Bits(Vec::new()))
                        }
                    }
                    Intrinsic::GetGrGid => {
                        let gid = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0) as libc::gid_t,
                            v => return Err(RuntimeError::TypeMismatch(format!("getgrgid gid requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut gr: libc::group = unsafe { std::mem::zeroed() };
                            let mut buf = vec![0u8; 4096];
                            let mut result: *mut libc::group = std::ptr::null_mut();
                            let ret = unsafe { libc::getgrgid_r(gid, &mut gr, buf.as_mut_ptr() as *mut libc::c_char, 4096, &mut result) };
                            if ret == 0 && !result.is_null() {
                                let name = unsafe { std::ffi::CStr::from_ptr(gr.gr_name).to_string_lossy().to_string() };
                                Ok(Value::Bits(name.into_bytes()))
                            } else {
                                Ok(Value::Bits(Vec::new()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = gr; let _ = buf; let _ = result;
                            Ok(Value::Bits(Vec::new()))
                        }
                    }
                    // ===== D17: Threading (2026-06-19) =====
                    Intrinsic::ThreadCreate => {
                        let fn_ptr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("thread_create fn_ptr requires Int, got {:?}", v))),
                        };
                        let arg = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("thread_create arg requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut thread: libc::pthread_t = unsafe { std::mem::zeroed() };
                            let fn_ptr_fn: extern "C" fn(*mut libc::c_void) -> *mut libc::c_void = unsafe { std::mem::transmute(fn_ptr) };
                            let ret = unsafe { libc::pthread_create(&mut thread, std::ptr::null(), fn_ptr_fn, arg as *mut libc::c_void) };
                            if ret == 0 {
                                Ok(Value::Bits(i64_to_bits(thread as i64)))
                            } else {
                                Ok(Value::Bits(i64_to_bits(-1)))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fn_ptr; let _ = arg;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::ThreadJoin => {
                        let thread = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("thread_join thread requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::pthread_join(thread as libc::pthread_t, std::ptr::null_mut()) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = thread;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::ThreadExit => {
                        let code = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("thread_exit code requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            unsafe { libc::pthread_exit(code as *mut libc::c_void); }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = code;
                        }
                        #[allow(unreachable_code)]
                        Ok(Value::Void)
                    }
                    Intrinsic::MutexLock => {
                        let mptr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("mutex_lock mptr requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::pthread_mutex_lock(mptr as *mut libc::pthread_mutex_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = mptr;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::MutexUnlock => {
                        let mptr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("mutex_unlock mptr requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::pthread_mutex_unlock(mptr as *mut libc::pthread_mutex_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = mptr;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::CondvarWait => {
                        let cptr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("condvar_wait cptr requires Int, got {:?}", v))),
                        };
                        let mptr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("condvar_wait mptr requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::pthread_cond_wait(cptr as *mut libc::pthread_cond_t, mptr as *mut libc::pthread_mutex_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = cptr; let _ = mptr;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::CondvarSignal => {
                        let cptr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("condvar_signal cptr requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::pthread_cond_signal(cptr as *mut libc::pthread_cond_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = cptr;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::CondvarBroadcast => {
                        let cptr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("condvar_broadcast cptr requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::pthread_cond_broadcast(cptr as *mut libc::pthread_cond_t) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = cptr;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    // ===== D18: Resource Limits (2026-06-19) =====
                    Intrinsic::GetRlimit => {
                        let resource = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("getrlimit resource requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut rlim: libc::rlimit = unsafe { std::mem::zeroed() };
                            let ret = unsafe { libc::getrlimit(resource as u32, &mut rlim) };
                            if ret == 0 {
                                let packed = (rlim.rlim_cur as i64) << 32 | (rlim.rlim_max as i64 & 0xFFFF_FFFF);
                                Ok(Value::Bits(i64_to_bits(packed)))
                            } else {
                                Ok(Value::Bits(i64_to_bits(-1)))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = resource;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::SetRlimit => {
                        let resource = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("setrlimit resource requires Int, got {:?}", v))),
                        };
                        let packed = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("setrlimit packed requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let rlim = libc::rlimit {
                                rlim_cur: (packed >> 32) as libc::rlim_t,
                                rlim_max: (packed & 0xFFFF_FFFF) as libc::rlim_t,
                            };
                            let ret = unsafe { libc::setrlimit(resource as u32, &rlim) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = resource; let _ = packed;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    // ===== Extra intrinsics (2026-06-19) =====
                    Intrinsic::MkStemp => {
                        let template = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("mkstemp template requires String, got {:?}", v))),
                        };
                        let ctemplate = std::ffi::CString::new(template.as_bytes()).unwrap_or_default();
                        let mut buf = ctemplate.into_bytes_with_nul();
                        #[cfg(unix)]
                        {
                            let fd = unsafe { libc::mkstemp(buf.as_mut_ptr() as *mut libc::c_char) };
                            Ok(Value::Bits(i64_to_bits(fd as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = buf;
                            Ok(Value::Bits(i64_to_bits(-1)))
                        }
                    }
                    Intrinsic::MkDtemp => {
                        let template = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("mkdtemp template requires String, got {:?}", v))),
                        };
                        let ctemplate = std::ffi::CString::new(template.as_bytes()).unwrap_or_default();
                        let mut buf = ctemplate.into_bytes_with_nul();
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::mkdtemp(buf.as_mut_ptr() as *mut libc::c_char) };
                            if !ret.is_null() {
                                let path = unsafe { std::ffi::CStr::from_ptr(ret).to_string_lossy().to_string() };
                                Ok(Value::Bits(path.into_bytes()))
                            } else {
                                Ok(Value::Bits(Vec::new()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = buf;
                            Ok(Value::Bits(Vec::new()))
                        }
                    }
                    Intrinsic::DlOpen => {
                        let filename = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("dlopen filename requires String, got {:?}", v))),
                        };
                        let cfilename = std::ffi::CString::new(filename.as_bytes()).unwrap_or_default();
                        #[cfg(unix)]
                        {
                            let handle = unsafe { libc::dlopen(cfilename.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
                            Ok(Value::Bits(i64_to_bits(handle as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = cfilename;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::DlSym => {
                        let handle = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("dlsym handle requires Int, got {:?}", v))),
                        };
                        let symbol = match values.remove(0) {
                            Value::Bits(b) => String::from_utf8_lossy(&b).to_string(),
                            v => return Err(RuntimeError::TypeMismatch(format!("dlsym symbol requires String, got {:?}", v))),
                        };
                        let csymbol = std::ffi::CString::new(symbol.as_bytes()).unwrap_or_default();
                        #[cfg(unix)]
                        {
                            let addr = unsafe { libc::dlsym(handle as *mut libc::c_void, csymbol.as_ptr()) };
                            Ok(Value::Bits(i64_to_bits(addr as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = handle; let _ = csymbol;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::DlClose => {
                        let handle = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b))?,
                            v => return Err(RuntimeError::TypeMismatch(format!("dlclose handle requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::dlclose(handle as *mut libc::c_void) };
                            Ok(Value::Bits(i64_to_bits(ret as i64)))
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = handle;
                            Ok(Value::Bits(i64_to_bits(0)))
                        }
                    }
                    Intrinsic::TtyName => {
                        let fd = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as i32).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(format!("ttyname fd requires Int, got {:?}", v))),
                        };
                        #[cfg(unix)]
                        {
                            let mut buf = vec![0u8; 256];
                            let ret = unsafe { libc::ttyname_r(fd, buf.as_mut_ptr() as *mut libc::c_char, 256) };
                            if ret == 0 {
                                let name = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const libc::c_char).to_string_lossy().to_string() };
                                Ok(Value::Bits(name.into_bytes()))
                            } else {
                                Ok(Value::Bits(Vec::new()))
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = fd;
                            Ok(Value::Bits(Vec::new()))
                        }
                    }
                    // 2026-07-03: Spatial memory intrinsics (interpreter uses libc)
                    Intrinsic::Memcpy => {
                        let dst = values.remove(0);
                        let src = values.remove(0);
                        let n = values.remove(0);
                        let dst_addr = bits_to_i64(&dst).unwrap_or(0) as usize as *mut u8;
                        let src_addr = bits_to_i64(&src).unwrap_or(0) as usize as *const u8;
                        let count = bits_to_i64(&n).unwrap_or(0) as usize;
                        if count == 0 { return Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }])); }
                        unsafe {
                            let src_slice = std::slice::from_raw_parts(src_addr, count);
                            let dst_slice = std::slice::from_raw_parts_mut(dst_addr, count);
                            dst_slice.copy_from_slice(src_slice);
                        }
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::Memcmp => {
                        let a = values.remove(0);
                        let b = values.remove(0);
                        let n = values.remove(0);
                        let a_addr = bits_to_i64(&a).unwrap_or(0) as usize as *const u8;
                        let b_addr = bits_to_i64(&b).unwrap_or(0) as usize as *const u8;
                        let count = bits_to_i64(&n).unwrap_or(0) as usize;
                        if count == 0 { return Ok(Value::Bits(i64_to_bits(0))); }
                        let result = unsafe {
                            let a_slice = std::slice::from_raw_parts(a_addr, count);
                            let b_slice = std::slice::from_raw_parts(b_addr, count);
                            a_slice.iter().zip(b_slice.iter())
                                .map(|(x, y)| (*x as i64) - (*y as i64))
                                .find(|&d| d != 0)
                                .unwrap_or(0)
                        };
                        Ok(Value::Bits(i64_to_bits(result)))
                    }
                    Intrinsic::Memset => {
                        let ptr = values.remove(0);
                        let val = values.remove(0);
                        let n = values.remove(0);
                        let addr = bits_to_i64(&ptr).unwrap_or(0) as usize as *mut u8;
                        let byte_val = bits_to_i64(&val).unwrap_or(0) as u8;
                        let count = bits_to_i64(&n).unwrap_or(0) as usize;
                        if count == 0 { return Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }])); }
                        unsafe {
                            std::ptr::write_bytes(addr, byte_val, count);
                        }
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::Hash => {
                        let ptr = values.remove(0);
                        let n = values.remove(0);
                        let addr = bits_to_i64(&ptr).unwrap_or(0) as usize as *const u8;
                        let count = bits_to_i64(&n).unwrap_or(0) as usize;
                        if count == 0 { return Ok(Value::Bits(i64_to_bits(0))); }
                        let mut hash: u64 = 0xcbf29ce484222325;
                        let bytes = unsafe {
                            std::slice::from_raw_parts(addr, count)
                        };
                        for &b in bytes {
                            hash ^= b as u64;
                            hash = hash.wrapping_mul(0x100000001b3);
                        }
                        Ok(Value::Bits(i64_to_bits(hash as i64)))
                    }
                    Intrinsic::Malloc => {
                        let size_val = values.remove(0);
                        let size = bits_to_i64(&size_val).unwrap_or(0) as usize;
                        let addr = self.virtual_heap.alloc(&vec![0u8; size]);
                        Ok(Value::Bits(i64_to_bits(addr as i64)))
                    }
                    Intrinsic::Free => {
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("free# requires Int address, got {:?}", v))),
                        };
                        self.virtual_heap.free(addr);
                        Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]))
                    }
                    Intrinsic::Realloc => {
                        let addr = match values.remove(0) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).map(|v| v as u64).unwrap_or(0),
                            v => return Err(RuntimeError::TypeMismatch(
                                format!("realloc# requires Int address, got {:?}", v))),
                        };
                        let new_size_val = values.remove(0);
                        let new_size = bits_to_i64(&new_size_val).unwrap_or(0) as usize;
                        // Read existing block, create new one
                        let existing = self.virtual_heap.read(addr, new_size as u64).unwrap_or(&[]);
                        let mut new_block = existing.to_vec();
                        new_block.resize(new_size, 0);
                        self.virtual_heap.write(addr, &new_block).ok();
                        Ok(Value::Bits(i64_to_bits(addr as i64)))
                    }
                    Intrinsic::UserDefined(name) => {
                        let inop = self.inop_decls.get(name).cloned();
                        if let Some(inop) = inop {
                            if let Some(fallback) = inop.fallback {
                                let mut local_state = self.state.clone();
                                for (i, (param_name, _)) in inop.params.iter().enumerate() {
                                    if i < values.len() {
                                        local_state.insert(param_name.clone(), values[i].clone());
                                    }
                                }
                                let prev_state = self.state.clone();
                                self.state = local_state;
                                let result = self.eval_expr(&fallback);
                                self.state = prev_state;
                                result
                            } else {
                                Err(RuntimeError::TypeMismatch(format!(
                                    "inop# `{}` has no fallback expression and cannot be evaluated in the interpreter",
                                    name
                                )))
                            }
                        } else {
                            Err(RuntimeError::TypeMismatch(format!(
                                "unknown user-defined intrinsic: {}#", name
                            )))
                        }
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
            Expr::Projection { source, target } => {
                // Function metadata projections don't evaluate the source expression
                // (the source is a defn/inop/txn name, not a state variable)
                if let Some(result) = self.try_eval_fn_projection(source, target) {
                    result
                } else {
                    ProjectionExpr::new(*source.clone(), target.clone()).evaluate(self, &ExprDispatch)
                }
            }
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
            // Template/macro nodes
            Expr::TemplateCall { name, .. } => {
                Err(RuntimeError::TypeMismatch(format!("macro not expanded: {}", name)))
            }
            Expr::MacroCall { name, .. } => {
                Err(RuntimeError::TypeMismatch(format!("macro not expanded: {}", name)))
            }
            Expr::Interpolate(_) | Expr::InterpolateExpr(_) => {
                unreachable!("should have been substituted")
            }
            // GPU shared memory — interpreter returns 0 (no GPU simulation)
            Expr::SharedMem(_) => Ok(Value::Bits(i64_to_bits(0))),
            Expr::QuoteBlock { statements, .. } => {
                Ok(Value::Block(statements.clone()))
            }
            // Pipe chains — desugared before this pass
            Expr::PipeChain(_) => unreachable!("PipeChain should have been desugared"),
            Expr::Deref(inner) => {
                let val = self.eval_expr(inner)?;
                match val {
                    Value::Ref(v) => Ok(*v),
                    _ => Err(RuntimeError::TypeMismatch("cannot dereference non-pointer".into())),
                }
            },
            Expr::Within { body, bound, unit: _, retries, fallback } => {
                let saved_counter = self.cycle_counter;
                let saved_budget = self.cycle_budget;
                let max_cycles = saved_counter + bound;
                let mut attempt = 0u64;
                let saved_state = self.state.clone();
                loop {
                    self.cycle_counter = saved_counter;
                    self.cycle_budget = max_cycles;
                    match self.eval_expr(body) {
                        Ok(val) => {
                            self.cycle_budget = saved_budget;
                            break Ok(val);
                        }
                        Err(RuntimeError::Timeout(_)) => {
                            attempt += 1;
                            if attempt > *retries {
                                self.state = saved_state;
                                self.cycle_budget = saved_budget;
                                self.cycle_counter = saved_counter;
                                break self.eval_expr(fallback);
                            }
                            self.state = saved_state.clone();
                        }
                        Err(e) => {
                            self.cycle_budget = saved_budget;
                            break Err(e);
                        }
                    }
                }
            }
            // 2026-07-11: Phase 5 — deferred literal not evaluable in interpreter
            Expr::DeferredLiteral { .. } => Ok(Value::Bits(i64_to_bits(0))),
        }
    }

    /// Try to evaluate a function metadata projection (Address, Name, etc.).
    /// Returns None if the source is not an identifier or the target is not Fn*.
    fn try_eval_fn_projection(&mut self, source: &Expr, target: &ProjectionTarget) -> Option<Result<Value, RuntimeError>> {
        let name = match source {
            Expr::Identifier(n) => n.clone(),
            _ => return None,
        };
        match target {
            ProjectionTarget::Address
            | ProjectionTarget::Name
            | ProjectionTarget::Params
            | ProjectionTarget::Returns
            | ProjectionTarget::Arity
            | ProjectionTarget::Loc
            | ProjectionTarget::Doc
            | ProjectionTarget::Hash
            | ProjectionTarget::Contracts
            | ProjectionTarget::Module
            | ProjectionTarget::IsPure
            | ProjectionTarget::FnSpan => {}
            _ => return None,
        }

        // Look up the declaration
        let meta = if let Some(defn) = self.definitions.get(&name) {
            Some(FnMeta {
                params: defn.parameters.iter().map(|(_, t)| t.clone()).collect(),
                outputs: defn.outputs.clone(),
                span: None, // Definition does not store span
                has_side_effects: false,
            })
        } else if let Some(inop) = self.inop_decls.get(&name) {
            Some(FnMeta {
                params: inop.params.iter().map(|(_, t)| t.clone()).collect(),
                outputs: inop.outputs.clone(),
                span: inop.span,
                has_side_effects: inop.has_side_effects,
            })
        } else if let Some(txn) = self.callable_txns.get(&name) {
            Some(FnMeta {
                params: txn.parameters.iter().map(|(_, t)| t.clone()).collect(),
                outputs: txn.outputs.clone(),
                span: txn.span,
                has_side_effects: !txn.is_reactive,
            })
        } else {
            None
        };

        let meta = match meta {
            Some(m) => m,
            None => return Some(Err(RuntimeError::UndefinedVariable(
                format!("cannot apply Fn projection to '{}': not a defined function, inop, or transaction", name)
            ))),
        };

        Some(match target {
            ProjectionTarget::Address => Ok(Value::Bits(i64_to_bits(0))),  // sentinel; real addr only in codegen
            ProjectionTarget::Name => Ok(Value::Bits(name.into_bytes())),
            ProjectionTarget::Params => {
                let s = meta.params.iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(Value::Bits(s.into_bytes()))
            }
            ProjectionTarget::Returns => {
                let s = meta.outputs.iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(Value::Bits(s.into_bytes()))
            }
            ProjectionTarget::Arity => Ok(Value::Bits(i64_to_bits(meta.params.len() as i64))),
            ProjectionTarget::Loc => {
                let loc = match meta.span {
                    Some(s) => format!("{}:{}", s.line, s.column),
                    None => String::new(),
                };
                Ok(Value::Bits(loc.into_bytes()))
            }
            ProjectionTarget::Doc => Ok(Value::Bits(Vec::new())), // doc comments not stored yet
            ProjectionTarget::Hash => {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                name.hash(&mut hasher);
                Ok(Value::Bits(i64_to_bits(hasher.finish() as i64)))
            }
            ProjectionTarget::Contracts => Ok(Value::Bits(Vec::new())), // contracts not serialized yet
            ProjectionTarget::Module => Ok(Value::Bits(Vec::new())), // module tracking not implemented yet
            ProjectionTarget::IsPure => Ok(Value::Bits(vec![if (!meta.has_side_effects) { 1u8 } else { 0u8 }])),
            ProjectionTarget::FnSpan => {
                let (start, end) = match meta.span {
                    Some(s) => (s.start as i64, s.end as i64),
                    None => (0, 0),
                };
                Ok(Value::Tuple(vec![Value::Bits(i64_to_bits(start)), Value::Bits(i64_to_bits(end))]))
            }
            _ => unreachable!(),
        })
    }

    fn eval_is_type(&self, val: Value, target: &crate::ast::IsTarget) -> Result<Value, RuntimeError> {
        use crate::ast::IsTarget;
        match target {
            IsTarget::Type(ty) => {
                let matches = match (&val, ty) {
                    (Value::Bits(_), Type::Custom(__t)) if __t == "Int" || __t == "UInt" => true,
                    (Value::Bits(_), Type::Custom(__t)) if __t == "Float" => true,
                    (Value::Bits(_), Type::Custom(__t)) if __t == "Bool" => true,
                    (Value::Bits(_), Type::Custom(__t)) if __t == "String" => true,
                    (Value::Bits(_), Type::Custom(__t)) if __t == "Char" => true,
                    (Value::List(_), Type::Vector(..)) => true,
                    (Value::List(_), Type::Applied(n, _)) if n == "List" => true,
                    (Value::Instance { typename, .. }, Type::Custom(n)) => typename == n,
                    (Value::Instance { typename, .. }, Type::Enum(n)) => typename == n,
                    (Value::Instance { typename, .. }, Type::Applied(n, _)) => typename == n,
                    (Value::Instance { typename, .. }, Type::Sig(n)) => typename == n,
                    (Value::Enum(ename, ..), Type::Custom(n)) => ename == n,
                    (Value::Enum(ename, ..), Type::Enum(n)) => ename == n,
                    (Value::Enum(ename, ..), Type::Applied(n, _)) => ename == n,
                    // 2026-07-03: Ptr value matches Ptr<T> or LayoutPtr type
                    (Value::Bits(_), Type::Applied(n, _)) if n == "Ptr" => true,
                    (Value::Bits(_), Type::LayoutPtr(_)) => true,
                    _ => false,
                };
                Ok(Value::Bits(vec![if matches {{ 1u8 }} else {{ 0u8 }}]))
            }
            IsTarget::Variant(vname) => {
                match &val {
                    Value::Enum(_, variant_name, _) => Ok(Value::Bits(vec![if (variant_name == vname) { 1u8 } else { 0u8 }])),
                    _ => Err(RuntimeError::TypeMismatch("is requires an enum value for variant check".into())),
                }
            }
        }
    }

    fn eval_from_check(&self, val: Value, ty: &Type) -> Result<Value, RuntimeError> {
        let type_name = match &val {
            Value::Instance { typename, .. } | Value::Enum(typename, ..) => typename.clone(),
            _ => return Ok(Value::Bits(vec![0u8])),
        };
        let target_name = format!("{:?}", ty);
        Ok(Value::Bits(vec![if (type_name == target_name) { 1u8 } else { 0u8 }]))
    }

    fn eval_like(&self, lhs: Value, rhs: Value) -> Result<Value, RuntimeError> {
        fn is_bool_true(v: &Value) -> bool {
            matches!(v, Value::Bits(b) if b.first() == Some(&1u8))
        }
        let result = match (&lhs, &rhs) {
            (Value::Bits(la), Value::Bits(lb)) => la == lb,
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
        Ok(Value::Bits(vec![if result {{ 1u8 }} else {{ 0u8 }}]))
    }

    /// Evaluate a type cast: convert a value to a target type.
    fn eval_cast(&self, val: Value, target: &Type) -> Result<Value, RuntimeError> {
        match (&val, target) {
            // Int ↔ Float: extract i64, convert, re-encode as Bits
            (Value::Bits(_), Type::Custom(__t)) if __t == "Float" => {
                let n = value_as_i64(&val).unwrap_or(0);
                Ok(Value::Bits(f64_to_bits(n as f64)))
            }
            (Value::Bits(_), Type::Custom(__t)) if __t == "Int" => {
                // Extend to canonical i64 (8 bytes)
                let n = value_as_i64(&val).unwrap_or(0);
                Ok(Value::Bits(i64_to_bits(n)))
            }

            // Char ↔ Int (via u32 interpretation of first 4 bytes)
            (Value::Bits(b), Type::Custom(__t)) if __t == "Char" && b.len() <= 4 => {
                let code = u32::from_le_bytes([b.first().copied().unwrap_or(0), 0, 0, 0]);
                if code > 0x10FFFF {
                    return Err(RuntimeError::TypeMismatch("value out of valid Char range".into()));
                }
                let ch = char::from_u32(code).unwrap_or('\0');
                Ok(Value::Bits((ch as u32).to_le_bytes().to_vec()))
            }

            // String: decode/encode as UTF-8 Bits
            // 2026-07-11: 4-byte values with trailing zeros → Char u32 encoding, convert to UTF-8 char
            // 8-byte values → interpret as i64, format as decimal string
            // All others → treat as raw UTF-8 bytes
            (Value::Bits(b), Type::Custom(__t)) if __t == "String" => {
                let s = if b.len() == 4 && b[1] == 0 && b[2] == 0 && b[3] == 0 {
                    char::from_u32(b[0] as u32).unwrap_or('\0').to_string()
                } else if b.len() == 8 {
                    value_as_i64(&val).map(|n| n.to_string()).unwrap_or_else(|| String::from_utf8_lossy(&b).to_string())
                } else {
                    String::from_utf8_lossy(&b).to_string()
                };
                Ok(Value::Bits(s.into_bytes()))
            }

            // Bool: check first byte
            (Value::Bits(b), Type::Custom(__t)) if __t == "Bool" => {
                let bval = b.first().copied().unwrap_or(0) != 0;
                Ok(Value::Bits(vec![if bval { 1u8 } else { 0u8 }]))
            }

            // Identity for Bits (all scalar values)
            (Value::Bits(_), Type::Custom(__t))
                if __t == "Int" || __t == "Float" || __t == "Bool" || __t == "Char"
                    || __t == "String" || __t == "UInt" || __t == "Data" => Ok(val.clone()),

            // Ptr ↔ Int identity (Ptr is just Bits containing the address)
            (Value::Bits(_), Type::Applied(__t, _)) if __t == "Ptr" => Ok(val.clone()),

            // Meld-backed custom type cast: identity (reinterpretation, not conversion)
            (_, Type::Custom(_)) => Ok(val),

            // Unsupported
            _ => Err(RuntimeError::TypeMismatch(format!(
                "cannot convert {:?} to {:?}", val, target
            ))),
        }
    }

    /// Evaluate a `<:` subtype projection: applies a sequence of ops to a source value.
    pub(crate) fn eval_subtype_projection(&mut self, mut source: Value, ops: &[crate::ast::SubtypeOp]) -> Result<Value, RuntimeError> {
        // 2026-07-11: Expr::String produces Value::Bits; check both representations.
        let source_str = match &source {
            Value::Bits(b) => String::from_utf8(b.clone()).ok(),
            _ => None,
        };
        if let Some(ref s) = source_str {
            for op in ops {
                if let crate::ast::SubtypeOp::Match(pattern_expr) = op {
                    let pattern_val = self.eval_expr(pattern_expr)?;
                    // 2026-07-11: Expr::String produces Value::Bits; convert both representations.
                    let pattern = match pattern_val {
                        Value::Bits(b) => String::from_utf8(b).map_err(|_| RuntimeError::TypeMismatch(
                            "Regex pattern must be a valid UTF-8 string".to_string()
                        ))?,
                        _ => return Err(RuntimeError::TypeMismatch(
                            "Regex pattern must be a string".to_string()
                        )),
                    };
                    let re = Regex::new(&pattern)
                        .map_err(|e| RuntimeError::TypeMismatch(format!("Invalid regex: {}", e)))?;
                    if let Some(caps) = re.captures(s) {
                        let group_count = caps.iter().len().saturating_sub(1);
                        if group_count == 0 {
                            return Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]));
                        }
                        let mut groups = Vec::new();
                        for i in 1..caps.iter().len() {
                            if let Some(m) = caps.get(i) {
                                groups.push(Value::Bits(m.as_str().to_string().into_bytes()));
                            }
                        }
                        match groups.len() {
                            0 => return Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }])),
                            1 => return Ok(groups.into_iter().next().unwrap()),
                            _ => return Ok(Value::Tuple(groups)),
                        }
                    } else {
                        return Ok(Value::Bits(vec![0u8]));
                    }
                }
            }
            return Ok(Value::Bits(s.to_string().into()));
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
            Value::HashSet(set) => set.into_iter().map(|s| Value::Bits(s.into_bytes())).collect(),
            val => {
                return Err(RuntimeError::TypeMismatch(
                    format!("Subtype projection requires a collection or string, got {:?}", val)
                ));
            }
        };

        // Helper to compare two Values for ordering
        fn cmp_values(a: &Value, b: &Value) -> std::cmp::Ordering {
            match (a, b) {
                (Value::Bits(a_bits), Value::Bits(b_bits)) => {
                    let a_int = value_as_i64(a);
                    let b_int = value_as_i64(b);
                    match (a_int, b_int) {
                        (Some(ai), Some(bi)) => ai.cmp(&bi),
                        _ => {
                            let af = value_as_f64(a);
                            let bf = value_as_f64(b);
                            match (af, bf) {
                                (Some(af), Some(bf)) => af.partial_cmp(&bf).unwrap_or(std::cmp::Ordering::Equal),
                                _ => a_bits.cmp(b_bits),
                            }
                        }
                    }
                }
                _ => std::cmp::Ordering::Equal,
            }
        }

        // Helper to compare Values for equality (used in dedup/group)
        fn values_equal(a: &Value, b: &Value) -> bool {
            match (a, b) {
                (Value::Tuple(av), Value::Tuple(bv)) => av.len() == bv.len() && av.iter().zip(bv.iter()).all(|(x, y)| values_equal(x, y)),
                (a, b) => a == b,
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
                        let result = self.eval_expr(predicate).unwrap_or(Value::Bits(vec![0u8]));
                        result == Value::Bits(vec![1u8])
                    }).collect();
                }
                crate::ast::SubtypeOp::Map(transform) => {
                    items = items.into_iter().map(|item| {
                        self.state.insert("_".to_string(), item);
                        self.eval_expr(transform).unwrap_or(Value::Bits(vec![0u8]))
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
                        self.eval_expr(key).unwrap_or(Value::Bits(i64_to_bits(0)))
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
                        self.eval_expr(key).unwrap_or(Value::Bits(i64_to_bits(0)))
                    }).collect();
                    let other_keys: Vec<Value> = other_list.iter().map(|b| {
                        self.state.insert("_".to_string(), b.clone());
                        self.eval_expr(key).unwrap_or(Value::Bits(i64_to_bits(0)))
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
                        let k = self.eval_expr(key).unwrap_or(Value::Bits(i64_to_bits(0)));
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
                    items = vec![Value::Bits(i64_to_bits(items.len() as i64))];
                }
                crate::ast::SubtypeOp::Sum(expr) => {
                    is_terminal = true;
                    let total: i64 = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        match self.eval_expr(expr).unwrap_or(Value::Bits(i64_to_bits(0))) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0),
                            _ => 0,
                        }
                    }).sum();
                    items = vec![Value::Bits(i64_to_bits(total))];
                }
                crate::ast::SubtypeOp::Avg(expr) => {
                    is_terminal = true;
                    let len = items.len();
                    if len == 0 {
                        items = vec![Value::Bits(i64_to_bits(0))];
                    } else {
                        let total: f64 = items.iter().map(|item| {
                            self.state.insert("_".to_string(), item.clone());
                            let v = self.eval_expr(expr).unwrap_or(Value::Bits(i64_to_bits(0)));
                            match &v {
                                Value::Bits(b) => {
                                    bits_to_i64(&v).map(|n| n as f64)
                                        .or_else(|_| bits_to_f64(&v))
                                        .unwrap_or(0.0)
                                }
                                _ => 0.0,
                            }
                        }).sum();
                        items = vec![Value::Bits(f64_to_bits(total / len as f64))];
                    }
                }
                crate::ast::SubtypeOp::Min(expr) => {
                    is_terminal = true;
                    let best = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        self.eval_expr(expr).unwrap_or(Value::Bits(i64_to_bits(i64::MAX)))
                    }).min_by(|a, b| cmp_values(a, b));
                    items = vec![best.unwrap_or(Value::Bits(i64_to_bits(0)))];
                }
                crate::ast::SubtypeOp::Max(expr) => {
                    is_terminal = true;
                    let best = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        self.eval_expr(expr).unwrap_or(Value::Bits(i64_to_bits(i64::MIN)))
                    }).max_by(|a, b| cmp_values(a, b));
                    items = vec![best.unwrap_or(Value::Bits(i64_to_bits(0)))];
                }
            }
        }

        if is_terminal {
            Ok(items.into_iter().next().unwrap_or(Value::Bits(i64_to_bits(0))))
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
                crate::ast::Expr::Quoted(s) => return Some(String::from_utf8_lossy(s).to_string()),
                crate::ast::Expr::Decimal(n) => return Some(n.to_string()),
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
        return Value::Bits(i64_to_bits(n));
    }
    // Try float
    if let Ok(f) = s.parse::<f64>() {
        return Value::Bits(f64_to_bits(f));
    }
    // Bool
    if s == "true" {
        return Value::Bits(vec![1u8]);
    }
    if s == "false" {
        return Value::Bits(vec![0u8]);
    }
    // Default: string
    Value::Bits(s.to_string().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_interpreter_with_list() -> Interpreter {
        let mut interp = Interpreter::new();
        interp.state.insert("list".to_string(), Value::List(vec![]));
        interp.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        interp
    }

    #[test]
    fn test_arrow_push_append() {
        let mut i = make_interpreter_with_list();
        let expr = Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Decimal(42))),
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![Value::Bits(i64_to_bits(42))]));
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![Value::Bits(i64_to_bits(42))])));
    }

    #[test]
    fn test_arrow_push_multiple() {
        let mut i = make_interpreter_with_list();
        let push = |v: i64| Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Decimal(v))),
        };
        i.eval_expr(&push(1)).unwrap();
        i.eval_expr(&push(2)).unwrap();
        i.eval_expr(&push(3)).unwrap();
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3))
        ])));
    }

    #[test]
    fn test_arrow_pop() {
        let mut i = make_interpreter_with_list();
        // First push a value
        i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Decimal(99))),
        }).unwrap();
        // Then pop it
        let popped = i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Pop, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Term),
            value: None,
        }).unwrap();
        assert_eq!(popped, Value::Bits(i64_to_bits(99)));
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![])));
    }

    #[test]
    fn test_arrow_discard() {
        let mut i = make_interpreter_with_list();
        // Push 2 values
        i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Decimal(10))),
        }).unwrap();
        i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Decimal(20))),
        }).unwrap();
        // Discard last
        let discard = Expr::ArrowDiscard {
            target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Term),
        };
        i.eval_expr(&discard).unwrap();
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![Value::Bits(i64_to_bits(10))])));
    }

    #[test]
    fn test_arrow_push_indexed() {
        let mut i = make_interpreter_with_list();
        // Push 3 items: [10, 20, 30]
        for v in &[10, 20, 30] {
            i.eval_expr(&Expr::ArrowMut {
                dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
                index: Box::new(Expr::Term),
                value: Some(Box::new(Expr::Decimal(*v))),
            }).unwrap();
        }
        // Insert 15 at index 1: [10, 15, 20, 30]
        i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Decimal(1)),
            value: Some(Box::new(Expr::Decimal(15))),
        }).unwrap();
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![
            Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(15)), Value::Bits(i64_to_bits(20)), Value::Bits(i64_to_bits(30))
        ])));
    }

    #[test]
    fn test_arrow_pop_indexed() {
        let mut i = make_interpreter_with_list();
        for v in &[10, 20, 30, 40] {
            i.eval_expr(&Expr::ArrowMut {
                dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
                index: Box::new(Expr::Term),
                value: Some(Box::new(Expr::Decimal(*v))),
            }).unwrap();
        }
        // Pop at index 1 → removes 20
        let popped = i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Pop, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("list".to_string())))),
            index: Box::new(Expr::Decimal(1)),
            value: None,
        }).unwrap();
        assert_eq!(popped, Value::Bits(i64_to_bits(20)));
        assert_eq!(i.state.get("list"), Some(&Value::List(vec![
            Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(30)), Value::Bits(i64_to_bits(40))
        ])));
    }

    #[test]
    fn test_arrow_discard_field_indexed() {
        use std::collections::HashMap;
        let mut i = Interpreter::new();
        let items = vec![Value::Bits(i64_to_bits(100)), Value::Bits(i64_to_bits(200)), Value::Bits(i64_to_bits(300))];
        i.state.insert("queue".to_string(), Value::Instance {
            typename: "Queue".to_string(),
            fields: HashMap::from([("items".to_string(), Value::List(items))]),
        });
        // <- &queue.items[0] — discard first element (dequeue)
        let discard = Expr::ArrowDiscard {
            target: Box::new(Expr::FieldAccess(
                Box::new(Expr::AddrOf(Box::new(Expr::Identifier("queue".to_string())))),
                "items".to_string(),
            )),
            index: Box::new(Expr::Decimal(0)),
        };
        i.eval_expr(&discard).unwrap();
        let queue = i.state.get("queue").unwrap();
        match queue {
            Value::Instance { fields, .. } => {
                match fields.get("items").unwrap() {
                    Value::List(items) => {
                        assert_eq!(items.len(), 2);
                        assert_eq!(items[0], Value::Bits(i64_to_bits(200)));
                        assert_eq!(items[1], Value::Bits(i64_to_bits(300)));
                    }
                    _ => panic!("items field is not a List"),
                }
            }
            _ => panic!("queue is not an Instance"),
        }
    }

    #[test]
    fn test_list_nesting_depth() {
        assert_eq!(Interpreter::list_nesting_depth(&Value::Bits(i64_to_bits(5))), 0);
        assert_eq!(Interpreter::list_nesting_depth(&Value::List(vec![Value::Bits(i64_to_bits(1))])), 1);
        let inner = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]);
        assert_eq!(Interpreter::list_nesting_depth(&Value::List(vec![inner.clone(), inner.clone()])), 2);
        let innermost = Value::List(vec![Value::Bits(i64_to_bits(1))]);
        let mid = Value::List(vec![innermost.clone(), innermost.clone()]);
        assert_eq!(Interpreter::list_nesting_depth(&Value::List(vec![mid.clone(), mid.clone()])), 3);
    }

    #[test]
    fn test_expand_coordinates() {
        let coords = vec![SliceCoordinate::Ellipsis, SliceCoordinate::Index(Box::new(Expr::Decimal(0)))];
        let expanded = Interpreter::expand_coordinates(&coords, 2).unwrap();
        assert_eq!(expanded.len(), 2);
        assert!(matches!(expanded[0], SliceCoordinate::Range { start: None, end: None }));
        // [..., 0] on 3D → [:, :, 0]
        let expanded = Interpreter::expand_coordinates(&coords, 3).unwrap();
        assert_eq!(expanded.len(), 3);
        assert!(matches!(expanded[2], SliceCoordinate::Index(_)));
        // [0, ...] on 3D → [0, :, :]
        let coords2 = vec![SliceCoordinate::Index(Box::new(Expr::Decimal(0))), SliceCoordinate::Ellipsis];
        let expanded = Interpreter::expand_coordinates(&coords2, 3).unwrap();
        assert_eq!(expanded.len(), 3);
        assert!(matches!(expanded[0], SliceCoordinate::Index(_)));
        // Multiple ellipses — error
        assert!(Interpreter::expand_coordinates(
            &vec![SliceCoordinate::Ellipsis, SliceCoordinate::Ellipsis], 3
        ).is_err());
        // Too many explicit coords — error
        let coords3 = vec![SliceCoordinate::Index(Box::new(Expr::Decimal(0))), SliceCoordinate::Index(Box::new(Expr::Decimal(1)))];
        assert!(Interpreter::expand_coordinates(&coords3, 1).is_err());
    }

    #[test]
    fn test_multislice_basic() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]);
        let inner2 = Value::List(vec![Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4))]);
        let matrix = Value::List(vec![inner1, inner2]);
        let coords = vec![
            SliceCoordinate::Index(Box::new(Expr::Decimal(0))),
            SliceCoordinate::Index(Box::new(Expr::Decimal(1))),
        ];
        assert_eq!(i.apply_multi_slice_coords(&matrix, &coords).unwrap(), Value::Bits(i64_to_bits(2)));
    }

    #[test]
    fn test_multislice_ellipsis_trailing() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]);
        let inner2 = Value::List(vec![Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4))]);
        let matrix = Value::List(vec![inner1, inner2]);
        let coords = vec![
            SliceCoordinate::Range { start: None, end: None },
            SliceCoordinate::Index(Box::new(Expr::Decimal(0))),
        ];
        let result = i.apply_multi_slice_coords(&matrix, &coords).unwrap();
        match result {
            Value::List(items) => assert_eq!(items, vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(3))]),
            _ => panic!("Expected list result"),
        }
    }

    #[test]
    fn test_multislice_ellipsis_leading() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]);
        let inner2 = Value::List(vec![Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4))]);
        let matrix = Value::List(vec![inner1, inner2]);
        let coords = vec![
            SliceCoordinate::Index(Box::new(Expr::Decimal(0))),
            SliceCoordinate::Range { start: None, end: None },
        ];
        let result = i.apply_multi_slice_coords(&matrix, &coords).unwrap();
        match result {
            Value::List(items) => assert_eq!(items, vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]),
            _ => panic!("Expected list result"),
        }
    }

    #[test]
    fn test_multislice_range_chain() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3))]);
        let inner2 = Value::List(vec![Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(5)), Value::Bits(i64_to_bits(6))]);
        let inner3 = Value::List(vec![Value::Bits(i64_to_bits(7)), Value::Bits(i64_to_bits(8)), Value::Bits(i64_to_bits(9))]);
        let matrix = Value::List(vec![inner1, inner2, inner3]);
        let coords = vec![
            SliceCoordinate::Range { start: Some(Box::new(Expr::Decimal(0))), end: Some(Box::new(Expr::Decimal(2))) },
            SliceCoordinate::Range { start: Some(Box::new(Expr::Decimal(1))), end: Some(Box::new(Expr::Decimal(3))) },
        ];
        let result = i.apply_multi_slice_coords(&matrix, &coords).unwrap();
        match result {
            Value::List(rows) => {
                assert_eq!(rows.len(), 2);
                match &rows[0] { Value::List(r) => assert_eq!(*r, vec![Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3))]), _ => panic!() }
                match &rows[1] { Value::List(r) => assert_eq!(*r, vec![Value::Bits(i64_to_bits(5)), Value::Bits(i64_to_bits(6))]), _ => panic!() }
            }
            _ => panic!("Expected nested list result"),
        }
    }

    #[test]
    fn test_projection_size_on_list() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(20)), Value::Bits(i64_to_bits(30))]);
        i.state.insert("xs".to_string(), list);
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("xs".to_string())),
            target: ProjectionTarget::Size,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(3)));
    }

    #[test]
    fn test_projection_size_on_string() {
        let mut i = Interpreter::new();
        i.state.insert("s".to_string(), Value::Bits("hello".to_string().into_bytes()));
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("s".to_string())),
            target: ProjectionTarget::Size,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(5)));
    }

    #[test]
    fn test_projection_size_on_int() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(42)));
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("x".to_string())),
            target: ProjectionTarget::Size,
        };
        let result = i.eval_expr(&expr).unwrap();
        // Size on Bits returns byte count (8 for i64)
        assert_eq!(result, Value::Bits(i64_to_bits(8)));
    }

    #[test]
    fn test_projection_size_on_float() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(f64_to_bits(3.14)));
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("x".to_string())),
            target: ProjectionTarget::Size,
        };
        let result = i.eval_expr(&expr).unwrap();
        // Size on Bits returns byte count (8 for f64)
        assert_eq!(result, Value::Bits(i64_to_bits(8)));
    }

    #[test]
    fn test_projection_size_on_bool() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(vec![1u8]));
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("x".to_string())),
            target: ProjectionTarget::Size,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1)));
    }

    #[test]
    fn test_projection_size_on_char() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits('a' as i64)));
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("x".to_string())),
            target: ProjectionTarget::Size,
        };
        let result = i.eval_expr(&expr).unwrap();
        // Size on Bits returns byte count (8 for i64-encoded char)
        assert_eq!(result, Value::Bits(i64_to_bits(8)));
    }

    #[test]
    fn test_projection_bytes_on_instance() {
        let mut i = Interpreter::new();
        let mut fields = std::collections::HashMap::new();
        fields.insert("x".to_string(), Value::Bits(i64_to_bits(1)));
        fields.insert("y".to_string(), Value::Bits(i64_to_bits(2)));
        let val = Value::Instance {
            typename: "Point".to_string(),
            fields,
        };
        i.state.insert("p".to_string(), val);
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("p".to_string())),
            target: ProjectionTarget::Bytes,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(16))); // 2 fields * 8 bytes
    }

    #[test]
    fn test_projection_bytes_on_data() {
        let mut i = Interpreter::new();
        i.state.insert("d".to_string(), Value::Bits(vec![1, 2, 3, 4]));
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("d".to_string())),
            target: ProjectionTarget::Bytes,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(4)));
    }

    #[test]
    fn test_projection_bytes_on_tuple() {
        let mut i = Interpreter::new();
        i.state.insert("t".to_string(), Value::Tuple(vec![Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(20)), Value::Bits(i64_to_bits(30))]));
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("t".to_string())),
            target: ProjectionTarget::Bytes,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(24))); // 3 elements * 8 bytes
    }

    #[test]
    fn test_bytes_intrinsic_on_instance() {
        let mut i = Interpreter::new();
        let mut fields = std::collections::HashMap::new();
        fields.insert("x".to_string(), Value::Bits(i64_to_bits(1)));
        let val = Value::Instance {
            typename: "Point".to_string(),
            fields,
        };
        i.state.insert("p".to_string(), val);
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ByteCount,
            args: vec![Expr::Identifier("p".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(8))); // 1 field * 8 bytes
    }

    #[test]
    fn test_bytes_intrinsic_on_unsupported_type_errors() {
        let mut i = Interpreter::new();
        i.state.insert("m".to_string(), Value::HashMap(std::collections::HashMap::new()));
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ByteCount,
            args: vec![Expr::Identifier("m".to_string())],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("not implemented"), "Expected error about not implemented, got: {}", err);
    }

    #[test]
    fn test_len_through_defn_no_magic() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3))]));
        // Register a defn len that uses :> Size (exactly as stdlib does)
        let defn = Definition {
            name: "len".to_string(),
            type_params: vec![],
            parameters: vec![("list".to_string(), Type::Custom("List".to_string()))],
            outputs: vec![Type::int()],
            output_type: Some(crate::ast::OutputType::Single(Type::int())),
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
            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
            derivation: None,
        };
        i.definitions.insert("len".to_string(), defn);
        let expr = Expr::Call("len".to_string(), vec![Expr::Identifier("xs".to_string())]);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(3)));
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
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(42)))]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "_".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_uni_wildcard_matches_none() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("None", vec![]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "_".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_uni_binds_field_from_variant() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(99)))]));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("v".to_string())],
            expr: Expr::Block(vec![], Box::new(Expr::Term)),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("v"), Some(&Value::Bits(i64_to_bits(99))));
    }

    #[test]
    fn test_uni_mismatch_does_not_bind() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("None", vec![]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("v".to_string())],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        // flag should not have been set (match failed)
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(0))));
        // v should not be bound
        assert!(!i.state.contains_key("v"));
    }

    #[test]
    fn test_uni_wildcard_does_not_bind_fields() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(42)))]));
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
            ("0".to_string(), Value::Bits(i64_to_bits(10))),
            ("1".to_string(), Value::Bits("hello".to_string().into_bytes())),
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
        assert_eq!(i.state.get("a"), Some(&Value::Bits(i64_to_bits(10))));
        assert_eq!(i.state.get("b"), Some(&Value::Bits("hello".to_string().into_bytes())));
    }

    #[test]
    fn test_uni_literal_pattern_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(42)))]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::LitInt(42)],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_uni_literal_pattern_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(99)))]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::LitInt(42)],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(0))));
    }

    #[test]
    fn test_uni_literal_string_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Msg", vec![("0", Value::Bits("ok".to_string().into_bytes()))]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Msg".to_string(),
            fields: vec![Pattern::LitString("ok".to_string())],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_uni_tuple_pattern() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![(
            "0",
            Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]),
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
        assert_eq!(i.state.get("a"), Some(&Value::Bits(i64_to_bits(1))));
        assert_eq!(i.state.get("b"), Some(&Value::Bits(i64_to_bits(2))));
    }

    #[test]
    fn test_uni_tuple_pattern_mismatch_length() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![(
            "0",
            Value::List(vec![Value::Bits(i64_to_bits(1))]), // length 1, but pattern expects length 2
        )]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Tuple(vec![
                Pattern::Var("a".to_string()),
                Pattern::Var("b".to_string()),
            ])],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(0))));
    }

    #[test]
    fn test_uni_recursive_nested_enum() {
        let mut i = Interpreter::new();
        // Simulate Some(Some(42)) — nested enum
        let inner = make_enum("Some", vec![("0", Value::Bits(i64_to_bits(42)))]);
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", inner)]));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("inner".to_string())],
            expr: Expr::Block(vec![], Box::new(Expr::Term)),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("inner"), Some(&make_enum("Some", vec![("0", Value::Bits(i64_to_bits(42)))])));
    }

    #[test]
    fn test_uni_simple_name_always_matches() {
        let mut i = Interpreter::new();
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        // Syntax 3: uni x = expr — always matches, binds nothing
        let stmt = Statement::Unification {
            name: "uni".to_string(),
            variant: "x".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_uni_fieldless_variant_matches_void() {
        // Syntax: uni val(Some) = expr;  — a variant name with no field patterns
        // This matches Void under the same catch-all that syntax 3 uses
        let mut i = Interpreter::new();
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        // Void matches because variant != "_" && fields.is_empty() is a catch-all
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_uni_wildcard_matches_void() {
        let mut i = Interpreter::new();
        // val is not in state, value is Void
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "_".to_string(),
            fields: vec![],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
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
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(0))));
    }

    #[test]
    fn test_uni_field_with_wildcard_ignores_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(42)))]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Some".to_string(),
            fields: vec![Pattern::Wildcard],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
        // Nothing bound to the wildcard
        assert!(!i.state.contains_key("_"));
    }

    #[test]
    fn test_uni_literal_float_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Val", vec![("0", Value::Bits(f64_to_bits(3.14)))]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Val".to_string(),
            fields: vec![Pattern::LitFloat(3.14)],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_uni_literal_bool_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Flag", vec![("0", Value::Bits(vec![1u8]))]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Flag".to_string(),
            fields: vec![Pattern::LitBool(true)],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_uni_literal_char_matches() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Ch", vec![("0", Value::Bits(i64_to_bits('x' as i64)))]));
        i.state.insert("flag".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Unification {
            name: "val".to_string(),
            variant: "Ch".to_string(),
            fields: vec![Pattern::LitChar('x')],
            expr: Expr::Block(
                vec![Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("flag".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                }],
                Box::new(Expr::Term),
            ),
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("flag"), Some(&Value::Bits(i64_to_bits(1))));
    }

    #[test]
    fn test_pattern_match_binds_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(42)))]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "Some".to_string(),
            fields: vec![Pattern::Var("v".to_string())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));
        assert_eq!(i.state.get("v"), Some(&Value::Bits(i64_to_bits(42))));
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
        assert_eq!(result, Value::Bits(vec![0u8]));
        assert!(!i.state.contains_key("v"));
    }

    #[test]
    fn test_pattern_match_literal_int() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("N", vec![("0", Value::Bits(i64_to_bits(42)))]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "N".to_string(),
            fields: vec![Pattern::LitInt(42)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));
    }

    #[test]
    fn test_pattern_match_literal_int_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("N", vec![("0", Value::Bits(i64_to_bits(99)))]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "N".to_string(),
            fields: vec![Pattern::LitInt(42)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]));
    }

    #[test]
    fn test_pattern_match_tuple_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("P", vec![(
            "0",
            Value::List(vec![Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(20))]),
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
        assert_eq!(result, Value::Bits(vec![1u8]));
        assert_eq!(i.state.get("a"), Some(&Value::Bits(i64_to_bits(10))));
        assert_eq!(i.state.get("b"), Some(&Value::Bits(i64_to_bits(20))));
    }

    #[test]
    fn test_pattern_match_wildcard_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(42)))]));
        let expr = Expr::PatternMatch {
            value: Box::new(Expr::Identifier("val".to_string())),
            variant: "Some".to_string(),
            fields: vec![Pattern::Wildcard],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));
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
        assert_eq!(result, Value::Bits(vec![1u8]));
    }

    #[test]
    fn test_match_simple_variant_binds_field() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("Some", vec![("0", Value::Bits(i64_to_bits(7)))]));
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
                    body: Box::new(Expr::Decimal(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(7)));
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
                    body: Box::new(Expr::Decimal(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)));
    }

    #[test]
    fn test_match_literal_pattern() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("N", vec![("0", Value::Bits(i64_to_bits(42)))]));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("val".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Variant {
                        name: "N".to_string(),
                        fields: vec![Pattern::LitInt(42)],
                    },
                    guard: None,
                    body: Box::new(Expr::Decimal(1)),
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Decimal(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1)));
    }

    #[test]
    fn test_match_literal_pattern_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("N", vec![("0", Value::Bits(i64_to_bits(99)))]));
        let expr = Expr::Match {
            value: Box::new(Expr::Identifier("val".to_string())),
            arms: vec![
                MatchArm {
                    pattern: MatchPattern::Variant {
                        name: "N".to_string(),
                        fields: vec![Pattern::LitInt(42)],
                    },
                    guard: None,
                    body: Box::new(Expr::Decimal(1)),
                },
                MatchArm {
                    pattern: MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Decimal(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)));
    }

    #[test]
    fn test_match_tuple_pattern() {
        let mut i = Interpreter::new();
        i.state.insert("val".to_string(), make_enum("P", vec![(
            "0",
            Value::List(vec![Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4))]),
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
                    body: Box::new(Expr::Decimal(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(3)));
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
                    body: Box::new(Expr::Decimal(99)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(99)));
    }

    #[test]
    fn test_match_multiple_fields() {
        let mut i = Interpreter::new();
        let fields: HashMap<String, Value> = [
            ("0".to_string(), Value::Bits(i64_to_bits(10))),
            ("1".to_string(), Value::Bits(i64_to_bits(20))),
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
                    body: Box::new(Expr::Decimal(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(30)));
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
                ("n".to_string(), Type::int()),
                ("result".to_string(), Type::int()),
                ("i".to_string(), Type::int()),
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
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("result".to_string()))),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("result".to_string())),
                        Box::new(Expr::Decimal(1)),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("i".to_string()))),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("i".to_string())),
                        Box::new(Expr::Decimal(1)),
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

            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
            derivation: None,
        };
        i.callable_txns.insert("count_up".to_string(), txn);
        let result = i.eval_expr(&Expr::Call("count_up".to_string(), vec![
            Expr::Decimal(5),
            Expr::Decimal(0),
            Expr::Decimal(0),
        ])).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(5)));
    }

    #[test]
    fn test_callable_txn_no_loop_if_pre_false() {
        let mut i = Interpreter::new();
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "noop".to_string(),
            parameters: vec![
                ("n".to_string(), Type::int()),
                ("result".to_string(), Type::int()),
                ("i".to_string(), Type::int()),
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
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("result".to_string()))),
                    expr: Expr::Decimal(99),
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

            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
            derivation: None,
        };
        i.callable_txns.insert("noop".to_string(), txn);
        // pre is i < n, but i=5, n=3 → false, so body never runs
        let result = i.eval_expr(&Expr::Call("noop".to_string(), vec![
            Expr::Decimal(3),
            Expr::Decimal(0),
            Expr::Decimal(5),
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
            parameters: vec![("x".to_string(), Type::int())],
            contract: Contract {
                pre_condition: Expr::Lt(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(10)),
                ),
                post_condition: Expr::Eq(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(10)),
                ),
                watchdog: None,
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("x".to_string())),
                        Box::new(Expr::Decimal(1)),
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

            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
            derivation: None,
        };
        i.callable_txns.insert("mutate".to_string(), txn);
        i.state.insert("outer".to_string(), Value::Bits(i64_to_bits(42)));
        let result = i.eval_expr(&Expr::Call("mutate".to_string(), vec![
            Expr::Decimal(0),
        ])).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(10)));
        // outer variable should still be intact
        assert_eq!(i.state.get("outer"), Some(&Value::Bits(i64_to_bits(42))));
    }

    // ── Phase 12: HashMap/HashSet arrow operations ──

    #[test]
    fn test_hashmap_arrow_push_key_value() {
        let mut i = Interpreter::new();
        i.state.insert("m".to_string(), Value::HashMap(std::collections::HashMap::new()));
        // Push (key, value) as a tuple (list with 2 elements)
        let expr = Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("m".to_string())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Tuple(vec![
                Expr::Quoted("a".into()),
                Expr::Decimal(1),
            ]))),
        };
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::HashMap(map) => assert_eq!(map.get("a"), Some(&Value::Bits(i64_to_bits(1)))),
            _ => panic!("Expected HashMap"),
        }
    }

    #[test]
    fn test_hashmap_arrow_push_indexed() {
        let mut i = Interpreter::new();
        i.state.insert("m".to_string(), Value::HashMap(std::collections::HashMap::new()));
        // &m[key] <- value
        let expr = Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("m".to_string())))),
            index: Box::new(Expr::Quoted("b".into())),
            value: Some(Box::new(Expr::Decimal(2))),
        };
        i.eval_expr(&expr).unwrap();
        match i.state.get("m").unwrap() {
            Value::HashMap(map) => assert_eq!(map.get("b"), Some(&Value::Bits(i64_to_bits(2)))),
            _ => panic!("Expected HashMap"),
        }
    }

    #[test]
    fn test_hashmap_arrow_pop_key() {
        let mut i = Interpreter::new();
        let mut map = std::collections::HashMap::new();
        map.insert("x".to_string(), Value::Bits(i64_to_bits(42)));
        i.state.insert("m".to_string(), Value::HashMap(map));
        // value <- &m[key]
        let popped = i.eval_expr(&Expr::ArrowMut {
            dir: ArrowDir::Pop, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("m".to_string())))),
            index: Box::new(Expr::Quoted("x".into())),
            value: None,
        }).unwrap();
        assert_eq!(popped, Value::Bits(i64_to_bits(42)));
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
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("s".to_string())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Quoted("hello".into()))),
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
            dir: ArrowDir::Pop, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("s".to_string())))),
            index: Box::new(Expr::Term),
            value: None,
        }).unwrap();
        assert_eq!(popped, Value::Bits("world".to_string().into_bytes()));
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
            target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("s".to_string())))),
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
            (Expr::Quoted("a".into()), Expr::Decimal(1)),
            (Expr::Quoted("b".into()), Expr::Decimal(2)),
        ]);
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::HashMap(map) => {
                assert_eq!(map.len(), 2);
                assert_eq!(map.get("a"), Some(&Value::Bits(i64_to_bits(1))));
                assert_eq!(map.get("b"), Some(&Value::Bits(i64_to_bits(2))));
            }
            _ => panic!("Expected HashMap"),
        }
    }

    #[test]
    fn test_set_literal_eval() {
        let mut i = Interpreter::new();
        let expr = Expr::SetLiteral(vec![
            Expr::Quoted("x".into()),
            Expr::Quoted("y".into()),
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
        map.insert("a".to_string(), Value::Bits(i64_to_bits(1)));
        map.insert("b".to_string(), Value::Bits(i64_to_bits(2)));
        i.state.insert("m".to_string(), Value::HashMap(map));
        let result = i.eval_expr(&Expr::Projection {
            source: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("m".to_string())))),
            target: ProjectionTarget::Keys,
        }).unwrap();
        match result {
            Value::List(keys) => {
                assert_eq!(keys.len(), 2);
                assert!(keys.contains(&Value::Bits("a".to_string().into_bytes())));
                assert!(keys.contains(&Value::Bits("b".to_string().into_bytes())));
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
            source: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("s".to_string())))),
            target: ProjectionTarget::Contains(Box::new(Expr::Quoted("hello".into()))),
        }).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));
        let result = i.eval_expr(&Expr::Projection {
            source: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("s".to_string())))),
            target: ProjectionTarget::Contains(Box::new(Expr::Quoted("nope".into()))),
        }).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]));
    }

    #[test]
    fn test_arrow_transfer_list() {
        let mut i = Interpreter::new();
        i.state.insert("src".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3)),
        ]));
        i.state.insert("dest".to_string(), Value::List(vec![]));
        i.eval_expr(&Expr::ArrowTransfer { consume: false, dest: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("dest".to_string())))),
            source: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("src".to_string())))),
            filter: None,
        }).unwrap();
        assert_eq!(i.state.get("src"), Some(&Value::List(vec![])));
        assert_eq!(i.state.get("dest"), Some(&Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3)),
        ])));
    }

    #[test]
    fn test_arrow_transfer_hashmap() {
        let mut i = Interpreter::new();
        let mut src = std::collections::HashMap::new();
        src.insert("a".to_string(), Value::Bits(i64_to_bits(1)));
        src.insert("b".to_string(), Value::Bits(i64_to_bits(2)));
        i.state.insert("src".to_string(), Value::HashMap(src));
        i.state.insert("dest".to_string(), Value::HashMap(std::collections::HashMap::new()));
        i.eval_expr(&Expr::ArrowTransfer { consume: false, dest: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("dest".to_string())))),
            source: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("src".to_string())))),
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
        let list = Value::List(vec![Value::Bits(i64_to_bits(0)), Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)),
            Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(5))]);
        i.state.insert("xs".to_string(), list);
        // xs[::2] — take every 2nd element
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Stride(Box::new(Expr::Decimal(2)))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![
            Value::Bits(i64_to_bits(0)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(4)),
        ]));
    }

    #[test]
    fn test_multislice_stride_with_coords() {
        let mut i = Interpreter::new();
        let inner1 = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3))]);
        let inner2 = Value::List(vec![Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(5)), Value::Bits(i64_to_bits(6))]);
        let inner3 = Value::List(vec![Value::Bits(i64_to_bits(7)), Value::Bits(i64_to_bits(8)), Value::Bits(i64_to_bits(9))]);
        let matrix = Value::List(vec![inner1, inner2, inner3]);
        // matrix[0..3 ::2] — first 3 rows, then every 2nd
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("matrix".to_string())),
            ops: vec![
                BracketOp::Coord(SliceCoordinate::Index(Box::new(Expr::Decimal(0)))),
                BracketOp::Stride(Box::new(Expr::Decimal(2))),
            ],
        };
        i.state.insert("matrix".to_string(), matrix);
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::List(items) => {
                assert_eq!(items.len(), 2); // every 2nd of [1,2,3] → [1,3]
                assert_eq!(items[0], Value::Bits(i64_to_bits(1)));
                assert_eq!(items[1], Value::Bits(i64_to_bits(3)));
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_multislice_mask() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(25)), Value::Bits(i64_to_bits(5)),
            Value::Bits(i64_to_bits(30)), Value::Bits(i64_to_bits(15))]);
        i.state.insert("xs".to_string(), list);
        // xs[; _ > 15] — keep elements > 15
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Mask(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(15)))
            ))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![
            Value::Bits(i64_to_bits(25)), Value::Bits(i64_to_bits(30)),
        ]));
    }

    #[test]
    fn test_multislice_stride_then_mask() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(25)), Value::Bits(i64_to_bits(5)),
            Value::Bits(i64_to_bits(30)), Value::Bits(i64_to_bits(15)), Value::Bits(i64_to_bits(40))]);
        i.state.insert("xs".to_string(), list);
        // xs[::2 ; _ > 12] — every 2nd, then keep > 12
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![
                BracketOp::Stride(Box::new(Expr::Decimal(2))),
                BracketOp::Mask(Box::new(
                    Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                        Box::new(Expr::Decimal(12)))
                )),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        // Every 2nd: [10, 5, 15] → filter > 12: [15]
        assert_eq!(result, Value::List(vec![Value::Bits(i64_to_bits(15))]));
    }

    #[test]
    fn test_multislice_mask_then_stride() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(25)), Value::Bits(i64_to_bits(5)),
            Value::Bits(i64_to_bits(30)), Value::Bits(i64_to_bits(15))]);
        i.state.insert("xs".to_string(), list);
        // xs[; _ > 12 ::2] — keep > 12, then take every 2nd
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![
                BracketOp::Mask(Box::new(
                    Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                        Box::new(Expr::Decimal(12)))
                )),
                BracketOp::Stride(Box::new(Expr::Decimal(2))),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        // Filter > 12: [25, 30, 15] → every 2nd: [25, 15]
        assert_eq!(result, Value::List(vec![Value::Bits(i64_to_bits(25)), Value::Bits(i64_to_bits(15))]));
    }

    #[test]
    fn test_slice_with_mask() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(25)), Value::Bits(i64_to_bits(5)),
            Value::Bits(i64_to_bits(30)), Value::Bits(i64_to_bits(15))]);
        i.state.insert("xs".to_string(), list);
        // xs[0..5 ; _ > 10] — slice then filter
        let expr = Expr::Slice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            start: None,
            end: None,
            stride: None,
            mask: Some(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(10)))
            )),
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![
            Value::Bits(i64_to_bits(25)), Value::Bits(i64_to_bits(30)), Value::Bits(i64_to_bits(15)),
        ]));
    }

    #[test]
    fn test_arrow_transfer_list_with_filter() {
        let mut i = Interpreter::new();
        i.state.insert("src".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(6)), Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(8)), Value::Bits(i64_to_bits(2)),
        ]));
        i.state.insert("dest".to_string(), Value::List(vec![]));
        i.eval_expr(&Expr::ArrowTransfer { consume: false, dest: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("dest".to_string())))),
            source: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("src".to_string())))),
            filter: Some(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(5)))
            )),
        }).unwrap();
        assert_eq!(i.state.get("src"), Some(&Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(2)),
        ])));
        assert_eq!(i.state.get("dest"), Some(&Value::List(vec![
            Value::Bits(i64_to_bits(6)), Value::Bits(i64_to_bits(8)),
        ])));
    }

    #[test]
    fn test_arrow_transfer_hashmap_with_filter() {
        let mut i = Interpreter::new();
        let mut src = std::collections::HashMap::new();
        src.insert("a".to_string(), Value::Bits(i64_to_bits(10)));
        src.insert("b".to_string(), Value::Bits(i64_to_bits(25)));
        src.insert("c".to_string(), Value::Bits(i64_to_bits(5)));
        i.state.insert("src".to_string(), Value::HashMap(src));
        i.state.insert("dest".to_string(), Value::HashMap(std::collections::HashMap::new()));
        i.eval_expr(&Expr::ArrowTransfer { consume: false, dest: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("dest".to_string())))),
            source: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("src".to_string())))),
            filter: Some(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(15)))
            )),
        }).unwrap();
        match (i.state.get("src").unwrap(), i.state.get("dest").unwrap()) {
            (Value::HashMap(src), Value::HashMap(dest)) => {
                assert_eq!(src.len(), 2); // b=25 moved, a=10 and c=5 stay
                assert_eq!(dest.len(), 1);
                assert_eq!(dest.get("b"), Some(&Value::Bits(i64_to_bits(25))));
            }
            _ => panic!("Expected HashMaps"),
        }
    }

    #[test]
    fn test_multislice_stride_zero_error() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]);
        i.state.insert("xs".to_string(), list);
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Stride(Box::new(Expr::Decimal(0)))],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_multislice_mask_on_non_list_error() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Bits(i64_to_bits(42)));
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Mask(Box::new(
                Expr::Gt(Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(10)))
            ))],
        };
        // Int is atomic — decomposes to chars, applies mask, reconstructs.
        // '4'=52 > 10, '2'=50 > 10 → both kept → Int(42)
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(42)));
    }

    #[test]
    fn test_multislice_stride_on_int() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Bits(i64_to_bits(42)));
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Stride(Box::new(Expr::Decimal(2)))],
        };
        // Bits(42) with b.len()==8 is detected as i64: 42→"42"→['4','2']→stride 2→['4']→"4"→4
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(4)));
    }

    #[test]
    fn test_multislice_regex_mask_on_list() {
        let mut i = Interpreter::new();
        let list = Value::List(vec![Value::Bits("hello".to_string().into_bytes()), Value::Bits("world".to_string().into_bytes()),
            Value::Bits("abc".to_string().into_bytes()), Value::Bits("wow".to_string().into_bytes())]);
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
            Value::Bits("hello".to_string().into_bytes()), Value::Bits("world".to_string().into_bytes()),
            Value::Bits("wow".to_string().into_bytes()),
        ]));
    }

    #[test]
    fn test_multislice_regex_mask_on_atomic_int() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Bits(i64_to_bits(15561)));
        // Bits-only: regex mask operates on raw bytes, not string representation.
        // This test verifies the operation doesn't crash and returns Bit values.
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Mask(Box::new(
                Expr::RegexLiteral("[15]".to_string())
            ))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert!(matches!(result, Value::Bits(_)), "Expected Bits result");
    }

    #[test]
    fn test_type_directed_desugar_int_regex_coord() {
        let mut i = Interpreter::new();
        i.state.insert("xs".to_string(), Value::Bits(i64_to_bits(15561)));
        // xs["[15]"] — single string coord on Int, desugars to per-char regex filter
        // "15561" → chars '1','5','5','6','1' → keep [15] → "1551" → Int(1551)
        let expr = Expr::MultiSlice {
            value: Box::new(Expr::Identifier("xs".to_string())),
            ops: vec![BracketOp::Coord(SliceCoordinate::Index(Box::new(
                Expr::Quoted("[15]".into())
            )))],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1551)));
    }

    #[test]
    fn test_sync_block_executes_statements_in_order() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        i.state.insert("y".to_string(), Value::Bits(i64_to_bits(0)));
        let sync_block = Statement::SyncBlock {
            body: vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                    expr: Expr::Decimal(1),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("y".to_string()))),
                    expr: Expr::Decimal(2),
                    timeout: None,
                    modifiers: vec![],
                },
            ],
        };
        i.exec_stmt(&sync_block).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(1))));
        assert_eq!(i.state.get("y"), Some(&Value::Bits(i64_to_bits(2))));
    }

    #[test]
    fn test_sync_block_nested_guarded() {
        let mut i = Interpreter::new();
        i.state.insert("a".to_string(), Value::Bits(vec![0u8]));
        i.state.insert("b".to_string(), Value::Bits(i64_to_bits(0)));
        let sync_block = Statement::SyncBlock {
            body: vec![
                Statement::Guarded {
                    condition: Expr::Bool(true),
                    statements: vec![Statement::Assignment {
                        lhs: Expr::AddrOf(Box::new(Expr::Identifier("a".to_string()))),
                        expr: Expr::Bool(true),
                        timeout: None,
                        modifiers: vec![],
                    }],
                    metadata: HashMap::new(),
                },
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("b".to_string()))),
                    expr: Expr::Decimal(42),
                    timeout: None,
                    modifiers: vec![],
                },
            ],
        };
        i.exec_stmt(&sync_block).unwrap();
        assert_eq!(i.state.get("a"), Some(&Value::Bits(vec![if true { 1u8 } else { 0u8 }])));
        assert_eq!(i.state.get("b"), Some(&Value::Bits(i64_to_bits(42))));
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
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(5)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Filter(Box::new(Expr::Gt(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(3)),
            )))],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(5))]));
    }

    #[test]
    fn test_projection_map() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Map(Box::new(Expr::Mul(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(2)),
            )))],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(6))]));
    }

    #[test]
    fn test_projection_filter_limit() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(5)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![
                SubtypeOp::Filter(Box::new(Expr::Gt(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(1)),
                ))),
                SubtypeOp::Limit(2),
            ],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3))]));
    }

    #[test]
    fn test_projection_count_aggregate() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(20)), Value::Bits(i64_to_bits(30)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Count],
        }).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(3)));
    }

    #[test]
    fn test_projection_sum() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(5)), Value::Bits(i64_to_bits(10)), Value::Bits(i64_to_bits(15)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Sum(Box::new(Expr::Identifier("_".to_string())))],
        }).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(30)));
    }

    #[test]
    fn test_projection_group_count() {
        let mut i = Interpreter::new();
        i.state.insert("items".to_string(), Value::List(vec![
            Value::Tuple(vec![Value::Bits("A".to_string().into_bytes()), Value::Bits(i64_to_bits(1))]),
            Value::Tuple(vec![Value::Bits("A".to_string().into_bytes()), Value::Bits(i64_to_bits(2))]),
            Value::Tuple(vec![Value::Bits("B".to_string().into_bytes()), Value::Bits(i64_to_bits(3))]),
        ]));
        // Group by first element of each tuple
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("items".to_string())),
            ops: vec![
                SubtypeOp::Group(Box::new(Expr::ListIndex(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(0)),
                ))),
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
            source: Box::new(Expr::Quoted("user@example.com".into())),
            ops: vec![SubtypeOp::Match(Box::new(Expr::Quoted("^([a-z]+)@(.+)$".into())))],
        }).unwrap();
        match result {
            Value::Tuple(groups) => {
                assert_eq!(groups.len(), 2);
                assert_eq!(groups[0], Value::Bits("user".to_string().into_bytes()));
                assert_eq!(groups[1], Value::Bits("example.com".to_string().into_bytes()));
            }
            _ => panic!("Expected Tuple, got {:?}", result),
        }
    }

    #[test]
    fn test_projection_string_no_match() {
        let mut i = Interpreter::new();
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Quoted("hello world".into())),
            ops: vec![SubtypeOp::Match(Box::new(Expr::Quoted("^[0-9]+$".into())))],
        }).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]));
    }

    // ---- SubtypeOp gap tests ----

    #[test]
    fn test_subtype_skip_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(5)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Skip(2)],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(5))]));
    }

    #[test]
    fn test_subtype_unique_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Unique],
        }).unwrap();
        assert_eq!(result, Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3))]));
    }

    #[test]
    fn test_subtype_sort_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("items".to_string(), Value::List(vec![
            Value::Tuple(vec![Value::Bits("b".to_string().into_bytes()), Value::Bits(i64_to_bits(2))]),
            Value::Tuple(vec![Value::Bits("a".to_string().into_bytes()), Value::Bits(i64_to_bits(1))]),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("items".to_string())),
            ops: vec![SubtypeOp::Sort(Box::new(Expr::ListIndex(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(0)),
            )))],
        }).unwrap();
        if let Value::List(sorted) = result {
            assert_eq!(sorted.len(), 2);
            // First element should be "a"
            if let Value::Tuple(ref fields) = sorted[0] {
                assert_eq!(fields[0], Value::Bits("a".to_string().into_bytes()));
            } else { panic!("Expected Tuple"); }
        } else { panic!("Expected List"); }
    }

    #[test]
    fn test_subtype_avg_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2)), Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(4)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Avg(Box::new(Expr::Identifier("_".to_string())))],
        }).unwrap();
        assert_eq!(result, Value::Bits(f64_to_bits(2.5)));
    }

    #[test]
    fn test_subtype_min_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(5)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Min(Box::new(Expr::Identifier("_".to_string())))],
        }).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1)));
    }

    #[test]
    fn test_subtype_max_on_list() {
        let mut i = Interpreter::new();
        i.state.insert("list".to_string(), Value::List(vec![
            Value::Bits(i64_to_bits(3)), Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(4)), Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(5)),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("list".to_string())),
            ops: vec![SubtypeOp::Max(Box::new(Expr::Identifier("_".to_string())))],
        }).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(5)));
    }

    #[test]
    fn test_subtype_join_two_lists() {
        let mut i = Interpreter::new();
        i.state.insert("left".to_string(), Value::List(vec![
            Value::Tuple(vec![Value::Bits(i64_to_bits(1)), Value::Bits("a".to_string().into_bytes())]),
            Value::Tuple(vec![Value::Bits(i64_to_bits(2)), Value::Bits("b".to_string().into_bytes())]),
        ]));
        i.state.insert("right".to_string(), Value::List(vec![
            Value::Tuple(vec![Value::Bits(i64_to_bits(1)), Value::Bits("x".to_string().into_bytes())]),
            Value::Tuple(vec![Value::Bits(i64_to_bits(3)), Value::Bits("y".to_string().into_bytes())]),
        ]));
        let result = i.eval_expr(&Expr::SubtypeProjection {
            source: Box::new(Expr::Identifier("left".to_string())),
            ops: vec![SubtypeOp::Join(
                Box::new(Expr::Identifier("right".to_string())),
                Box::new(Expr::ListIndex(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(0)),
                )),
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
            Value::Bits("key1".to_string().into_bytes()),
            Value::Bits("value1".to_string().into_bytes()),
            Value::Bits(i64_to_bits(42)),
        ];
        let args = vec![
            Value::Bits(path.into()),
            Value::List(values),
        ];

        let result = crate::ffi::registry::dbvl_append_impl(args).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));

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
            Value::Bits("hello,world".to_string().into_bytes()),
            Value::Bits("normal".to_string().into_bytes()),
        ];
        let args = vec![
            Value::Bits(path.into()),
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
                Value::Bits(format!("key{}", i).into_bytes()),
                Value::Bits(i64_to_bits(i)),
            ];
            let args = vec![
                Value::Bits(path.into()),
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
        assert_eq!(vals[0], Value::Bits("key1".to_string().into_bytes()));
        assert_eq!(vals[1], Value::Bits("hello,world".to_string().into_bytes()));
        assert_eq!(vals[2], Value::Bits(i64_to_bits(42)));
        assert_eq!(vals[3], Value::Bits(vec![1u8]));
    }

    #[test]
    fn test_parse_csv_line_ints_floats() {
        let vals = parse_csv_line("42,3.14,100,0.5");
        assert_eq!(vals[0], Value::Bits(i64_to_bits(42)));
        assert_eq!(vals[1], Value::Bits(f64_to_bits(3.14)));
        assert_eq!(vals[2], Value::Bits(i64_to_bits(100)));
        assert_eq!(vals[3], Value::Bits(f64_to_bits(0.5)));
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
            Box::new(Expr::Quoted("rusty_key".into())),
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
            Box::new(Expr::Quoted("rusty_key".into())),
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
                assert_eq!(fields.get("id"), Some(&Value::Bits("rusty_key".to_string().into_bytes())));
                assert_eq!(fields.get("name"), Some(&Value::Bits("Rusty Key".to_string().into_bytes())));
                assert_eq!(fields.get("hp"), Some(&Value::Bits(i64_to_bits(5))));
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
            Box::new(Expr::Quoted("candle".into())),
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
                assert_eq!(fields.get("name"), Some(&Value::Bits("Wax Candle".to_string().into_bytes())));
            }
            other => panic!("Expected Instance, got {:?}", other),
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_tuple_destructure_assignment() {
        let mut i = Interpreter::new();
        i.state.insert("a".to_string(), Value::Bits(i64_to_bits(0)));
        i.state.insert("b".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Assignment {
            lhs: Expr::TupleDestructure(
                vec!["a".to_string(), "b".to_string()],
                Box::new(Expr::Term),
            ),
            expr: Expr::Tuple(vec![Expr::Decimal(42), Expr::Decimal(99)]),
            timeout: None,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("a"), Some(&Value::Bits(i64_to_bits(42))));
        assert_eq!(i.state.get("b"), Some(&Value::Bits(i64_to_bits(99))));
    }

    #[test]
    fn test_tuple_destructure_assignment_from_list() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        i.state.insert("y".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Assignment {
            lhs: Expr::TupleDestructure(
                vec!["x".to_string(), "y".to_string()],
                Box::new(Expr::Term),
            ),
            expr: Expr::ListLiteral(vec![Expr::Decimal(7), Expr::Decimal(13)]),
            timeout: None,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(7))));
        assert_eq!(i.state.get("y"), Some(&Value::Bits(i64_to_bits(13))));
    }

    #[test]
    fn test_tuple_destructure_assignment_wrong_type_errors() {
        let mut i = Interpreter::new();
        i.state.insert("a".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Assignment {
            lhs: Expr::TupleDestructure(
                vec!["a".to_string()],
                Box::new(Expr::Term),
            ),
            expr: Expr::Decimal(42),
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
    fn test_tuple_bracket_index() {
        let mut i = Interpreter::new();
        i.state.insert("result".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Assignment {
            lhs: Expr::AddrOf(Box::new(Expr::Identifier("result".to_string()))),
            expr: Expr::ListIndex(
                Box::new(Expr::Tuple(vec![Expr::Decimal(10), Expr::Decimal(20), Expr::Decimal(30)])),
                Box::new(Expr::Decimal(1)),
            ),
            timeout: None,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("result"), Some(&Value::Bits(i64_to_bits(20))));
    }

    #[test]
    fn test_callable_txn_postcondition_failure_returns_error() {
        let mut i = Interpreter::new();
        let txn = Transaction {
            is_async: false,
            is_reactive: false,
            name: "bad_post".to_string(),
            parameters: vec![
                ("x".to_string(), Type::int()),
            ],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Eq(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(0)),
                ),
                watchdog: None,
                span: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                    expr: Expr::Decimal(99),
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

            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
            derivation: None,
        };
        i.callable_txns.insert("bad_post".to_string(), txn);
        let result = i.eval_expr(&Expr::Call("bad_post".to_string(), vec![
            Expr::Decimal(0),
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
                ("n".to_string(), Type::int()),
                ("acc".to_string(), Type::int()),
                ("i".to_string(), Type::int()),
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
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("acc".to_string()))),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("acc".to_string())),
                        Box::new(Expr::Identifier("i".to_string())),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("i".to_string()))),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("i".to_string())),
                        Box::new(Expr::Decimal(1)),
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

            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
            derivation: None,
        };
        i.callable_txns.insert("count_to_n".to_string(), txn);
        let result = i.eval_expr(&Expr::Call("count_to_n".to_string(), vec![
            Expr::Decimal(5),
            Expr::Decimal(0),
            Expr::Decimal(0),
        ])).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(10)));  // 0+1+2+3+4 = 10
    }

    #[test]
    fn test_foreach_filter() {
        let mut i = Interpreter::new();
        let stmt = Statement::Foreach {
            item: "x".to_string(),
            list: Box::new(Expr::ListLiteral(vec![Expr::Decimal(1), Expr::Decimal(2), Expr::Decimal(3)])),
            body: vec![],
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        // Just verify no error — basic loop completion
    }

    #[test]
    fn test_foreach_accumulates() {
        let mut i = Interpreter::new();
        i.state.insert("sum".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Foreach {
            item: "x".to_string(),
            list: Box::new(Expr::ListLiteral(vec![Expr::Decimal(10), Expr::Decimal(20), Expr::Decimal(30)])),
            modifiers: vec![],
            body: vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("sum".to_string()))),
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
        assert_eq!(i.state.get("sum"), Some(&Value::Bits(i64_to_bits(60))));
    }

    #[test]
    fn test_match_string_literal() {
        let mut i = Interpreter::new();
        let expr = Expr::Match {
            value: Box::new(Expr::Quoted("foo".into())),
            arms: vec![
                crate::ast::MatchArm {
                    pattern: crate::ast::MatchPattern::Literal(crate::ast::Pattern::LitString("foo".to_string())),
                    guard: None,
                    body: Box::new(Expr::Decimal(1)),
                },
                crate::ast::MatchArm {
                    pattern: crate::ast::MatchPattern::Wildcard,
                    guard: None,
                    body: Box::new(Expr::Decimal(0)),
                },
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1)));
    }

    #[test]
    fn test_match_int_literal() {
        let mut i = Interpreter::new();
        let expr = Expr::Match {
            value: Box::new(Expr::Decimal(42)),
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
        assert_eq!(result, Value::Bits(vec![1u8]));
    }

    #[test]
    fn test_oracle_executes_body() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Oracle {
            handler: vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                    expr: Expr::Decimal(99),
                    timeout: None, modifiers: vec![],
                },
            ],
            body: vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                    expr: Expr::Decimal(42),
                    timeout: None, modifiers: vec![],
                },
            ],
            span: None,
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(42))));
    }

    #[test]
    fn test_oracle_fuel_exhausts_runs_handler() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        // Use a long sequence of statements to exhaust fuel, not recursion
        let mut body = Vec::new();
        // The fuel limit is 100, so 200 assignments should exhaust it
        for _ in 0..200 {
            body.push(Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                expr: Expr::Decimal(42),
                timeout: None, modifiers: vec![],
            });
        }
        let stmt = Statement::Oracle {
            handler: vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                    expr: Expr::Decimal(999),
                    timeout: None, modifiers: vec![],
                },
            ],
            body,
            span: None,
        };
        i.exec_stmt(&stmt).unwrap();
        // Fuel exhausted — handler sets x = 999
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(999))));
    }

    #[test]
    fn test_watchdog_cycle_counter_tracks_statements() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        i.cycle_budget = 100;
        // Run 10 assignments — should stay under budget
        for _ in 0..10 {
            let stmt = Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Decimal(1),
                timeout: None, modifiers: vec![],
            };
            i.exec_stmt(&stmt).unwrap();
        }
        assert_eq!(i.cycle_counter, 10);
        // Budget = 100, so we should still be good
        assert!(i.cycle_counter <= i.cycle_budget);
    }

    #[test]
    fn test_watchdog_timeout_on_budget_exceeded() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        i.cycle_budget = 5;
        // Run 5 statements — should stay under budget
        for _ in 0..5 {
            let stmt = Statement::Assignment {
                lhs: Expr::Identifier("x".to_string()),
                expr: Expr::Decimal(1),
                timeout: None, modifiers: vec![],
            };
            i.exec_stmt(&stmt).unwrap();
        }
        assert_eq!(i.cycle_counter, 5);
        // 6th statement should timeout
        let stmt = Statement::Assignment {
            lhs: Expr::Identifier("x".to_string()),
            expr: Expr::Decimal(1),
            timeout: None, modifiers: vec![],
        };
        let err = i.exec_stmt(&stmt).unwrap_err();
        match err {
            RuntimeError::Timeout(msg) => {
                assert!(msg.contains("budget exceeded"));
            }
            other => panic!("Expected Timeout, got {:?}", other),
        }
    }

    #[test]
    fn test_watchdog_timeout_in_oracle_triggers_handler() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        i.cycle_budget = 3;
        let body = vec![
            Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                expr: Expr::Decimal(1),
                timeout: None, modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                expr: Expr::Decimal(2),
                timeout: None, modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                expr: Expr::Decimal(3),
                timeout: None, modifiers: vec![],
            },
            // 4th assignment — exceeds budget of 3
            Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                expr: Expr::Decimal(4),
                timeout: None, modifiers: vec![],
            },
        ];
        let stmt = Statement::Oracle {
            handler: vec![
                Statement::Assignment {
                    lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
                    expr: Expr::Decimal(999),
                    timeout: None, modifiers: vec![],
                },
            ],
            body,
            span: None,
        };
        i.exec_stmt(&stmt).unwrap();
        // Cycle budget exceeded → handler runs and sets x = 999
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(999))));
    }

    fn mock_pipe_fn_int_42(_args: Vec<Value>) -> Result<Value, RuntimeError> {
        Ok(Value::Bits(i64_to_bits(42)))
    }

    fn mock_pipe_fn_float_nan(_args: Vec<Value>) -> Result<Value, RuntimeError> {
        Ok(Value::Bits(f64_to_bits(f64::NAN)))
    }

    #[test]
    fn test_pipe_frgn_integration_through_call_dispatch() {
        let mut i = Interpreter::new();
        // Register a pipe frgn with a mock function that returns an int
        let sig = ForeignSignature {
            name: "integration_pipe".into(),
            location: "mock:integration_pipe".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::int())],
            result_type: ResultType::Projection(vec![Type::int()]),
            error_type_name: "".into(), error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true, fallback: Some(Expr::Decimal(-1)),
            default_watchdog: None,
            span: None,
        };
        i.ffi_bindings.insert("integration_pipe".into(), sig.clone());
        i.ffi_name_to_location.insert("integration_pipe".into(), "mock:integration_pipe".into());
        i.foreign_functions.insert("mock:integration_pipe".into(), mock_pipe_fn_int_42);

        let expr = Expr::Call("integration_pipe".into(), vec![]);
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::Enum(e, v, fields) if e == "Result" && v == "Ok" => {
                assert_eq!(fields.get("value"), Some(&Value::Bits(i64_to_bits(42))));
            }
            other => panic!("Expected Ok(42) from integration dispatch, got {:?}", other),
        }
    }

    #[test]
    fn test_pipe_frgn_integration_nan_float() {
        let mut i = Interpreter::new();
        let sig = ForeignSignature {
            name: "integration_pipe_nan".into(),
            location: "mock:integration_pipe_nan".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::float())],
            result_type: ResultType::Projection(vec![Type::float()]),
            error_type_name: "".into(), error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true, fallback: Some(Expr::Float(0.0)),
            default_watchdog: None,
            span: None,
        };
        i.ffi_bindings.insert("integration_pipe_nan".into(), sig.clone());
        i.ffi_name_to_location.insert("integration_pipe_nan".into(), "mock:integration_pipe_nan".into());
        i.foreign_functions.insert("mock:integration_pipe_nan".into(), mock_pipe_fn_float_nan);

        let expr = Expr::Call("integration_pipe_nan".into(), vec![]);
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::Enum(e, v, fields) if e == "Result" && v == "Err" => {
                match fields.get("value") {
                    Some(Value::Bits(b)) => {
                        if b.len() >= 8 {
                            let mut arr = [0u8; 8];
                            arr.copy_from_slice(&b[..8]);
                            let f = f64::from_le_bytes(arr);
                            assert_eq!(f, 0.0, "Float fallback should be 0.0");
                        } else {
                            panic!("Expected 8 bytes for float, got {}", b.len());
                        }
                    }
                    other => panic!("Expected fallback Float(0.0), got {:?}", other),
                }
            }
            other => panic!("Expected Err(0.0) from NaN dispatch, got {:?}", other),
        }
    }

    #[test]
    fn test_pipe_frgn_dynamic_ffi_getpid() {
        // Tests the dynamic FFI pipe interceptor path by registering a real
        // libc function (getpid) through the FrgnRegistry with a pipe binding.
        use crate::ffi::dynamic::{FrgnDecl, FrgnType};

        let mut i = Interpreter::new();

        // Register the dynamic FFI declaration for getpid (0 args → Int)
        let decl = FrgnDecl {
            name: "getpid".into(),
            params: vec![],
            ret: FrgnType::Int,
            lib: "libc.so.6".into(),
        };
        i.frgn_registry.register(decl);

        // Register the pipe frgn binding
        let sig = ForeignSignature {
            name: "getpid".into(),
            location: "libc.so.6".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::int())],
            result_type: ResultType::Projection(vec![Type::int()]),
            error_type_name: "".into(), error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true,
            fallback: Some(Expr::Decimal(-1)),
            default_watchdog: None,
            span: None,
        };
        i.ffi_bindings.insert("getpid".into(), sig);

        // Call via Expr::Call — hits dynamic FFI path first
        let expr = Expr::Call("getpid".into(), vec![]);
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::Enum(e, v, fields) if e == "Result" && v == "Ok" => {
                match fields.get("value") {
                    Some(val @ Value::Bits(_)) => {
                        let pid = crate::interpreter::value_as_i64(val).unwrap_or(0);
                        assert!(pid > 0, "PID should be positive, got {}", pid);
                    }
                    other => panic!("Expected Int pid in Ok, got {:?}", other),
                }
            }
            other => panic!("Expected Ok(pid) from dynamic FFI pipe, got {:?}", other),
        }
    }

    #[test]
    fn test_pipe_frgn_dynamic_ffi_unwraps_ok() {
        // Tests that the pipe interceptor correctly unwraps the registry's
        // Ok(value) and re-wraps through call_pipe_frgn.
        // Uses a String-returning libc function via dlopen.
        use crate::ffi::dynamic::{FrgnDecl, FrgnType};

        let mut i = Interpreter::new();

        // getenv("HOME") returns a string — dynamic FFI supports 1-arg String→Int
        // but NOT String→String. So we use getpid which is 0-args→Int.
        // For the pipe path we just need to verify the wrapping is correct.
        let decl = FrgnDecl {
            name: "getpid".into(),
            params: vec![],
            ret: FrgnType::Int,
            lib: "libc.so.6".into(),
        };
        i.frgn_registry.register(decl);

        let sig = ForeignSignature {
            name: "getpid".into(),
            location: "libc.so.6".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::int())],
            result_type: ResultType::Projection(vec![Type::int()]),
            error_type_name: "".into(), error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true,
            fallback: Some(Expr::Decimal(-1)),
            default_watchdog: None,
            span: None,
        };
        i.ffi_bindings.insert("getpid".into(), sig);

        let expr = Expr::Call("getpid".into(), vec![]);
        let result = i.eval_expr(&expr).unwrap();
        // The pipe interceptor unwraps the registry's Ok, validates the int
        // (always valid), and re-wraps in Ok. We should get Ok(pid).
        assert!(matches!(&result,
            Value::Enum(_, v, _) if v == "Ok"),
            "Expected Ok variant, got {:?}", result);
    }

    #[test]
    fn test_constraint_expression_violated() {
        let mut i = Interpreter::new();
        // let x: Int <: [_ > 0] = -5; — should fail
        let stmt = Statement::Let {
            name: "x".to_string(),
            ty: Some(Type::int()),
            expr: Some(Expr::Decimal(-5)),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: Some(Box::new(Expr::Gt(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(0)),
            ))),
            is_override: false,
            modifiers: vec![],
        };
        let result = i.exec_stmt(&stmt);
        assert!(result.is_err(), "Expected constraint violation for -5");
    }

    #[test]
    fn test_type_def_guard_enforced() {
        let mut i = Interpreter::new();
        // Build TypeUniverse with a Positive type
        let td = crate::ast::TypeDef {
            name: "Positive".to_string(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Int".into())),
            body: crate::ast::TypeDefBody {
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![],
                operators: vec![],
            constraints: vec![Expr::Gt(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(0)),
                )],
                span: None,
            },
            span: None,
        };
        use crate::ast::{TopLevel, Program};
        let program = Program {
            items: vec![TopLevel::TypeDef(Box::new(td))],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        };
        i.type_universe = Some(crate::type_universe::TypeUniverse::build(&program));
        let stmt = Statement::Let {
            name: "x".to_string(),
            ty: Some(Type::Custom("Positive".to_string())),
            expr: Some(Expr::Decimal(-5)),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: None,
            is_override: false,
            modifiers: vec![],
        };
        let result = i.exec_stmt(&stmt);
        assert!(result.is_err(), "Expected TypeDef guard violation for -5");
    }

    #[test]
    fn test_type_def_guard_passes() {
        let mut i = Interpreter::new();
        let td = crate::ast::TypeDef {
            name: "Positive".to_string(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Int".into())),
            body: crate::ast::TypeDefBody {
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![],
                operators: vec![],
            constraints: vec![Expr::Gt(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(0)),
                )],
                span: None,
            },
            span: None,
        };
        use crate::ast::{TopLevel, Program};
        let program = Program {
            items: vec![TopLevel::TypeDef(Box::new(td))],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        };
        i.type_universe = Some(crate::type_universe::TypeUniverse::build(&program));
        let stmt = Statement::Let {
            name: "x".to_string(),
            ty: Some(Type::Custom("Positive".to_string())),
            expr: Some(Expr::Decimal(42)),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: None,
            is_override: false,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(42))));
    }

    #[test]
    fn test_constraint_regex_evaluates_to_non_bool() {
        // Regex literals evaluate to Value::Regex(dfa), not Bool(true).
        // Using @"pattern" alone as a constraint always violates because
        // eval_constraint requires Value::Bits(vec![1u8]). A future enhancement
        // could auto-apply regex against _ in constraint context.
        let mut i = Interpreter::new();
        let val = Value::Bits("hello".to_string().into_bytes());
        let constraint = Expr::RegexLiteral("^hello".to_string());
        let result = i.eval_constraint(&val, &constraint);
        assert!(result.is_err(), "Regex literal alone is not a valid constraint expression");
    }

    // ── GPU compute intrinsic stubs ──────────────────────────────

    #[test]
    fn test_gpu_get_global_id() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetGlobalId,
            args: vec![Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)));
    }

    #[test]
    fn test_gpu_get_local_id() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetLocalId,
            args: vec![Expr::Decimal(1)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)));
    }

    #[test]
    fn test_gpu_get_group_id() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetGroupId,
            args: vec![Expr::Decimal(2)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)));
    }

    #[test]
    fn test_gpu_get_num_groups() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetNumGroups,
            args: vec![Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1)));
    }

    #[test]
    fn test_gpu_barrier() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SubGroupBarrier,
            args: vec![],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));
    }

    #[test]
    fn test_gpu_intrinsic_invalid_dimension_errors() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetGlobalId,
            args: vec![Expr::Decimal(5)],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("out of range"), "Expected out-of-range error, got: {}", err);
    }

    #[test]
    fn test_gpu_intrinsic_wrong_arg_type_errors() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetGlobalId,
            args: vec![Expr::Quoted("x".into())],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_inop_fallback_evaluation() {
        let mut i = Interpreter::new();
        let inop = InopDeclaration {
            name: "test_add".into(),
            type_params: vec![],
            params: vec![("x".into(), Type::int()), ("y".into(), Type::int())],
            outputs: vec![Type::int()],
            contract: crate::ast::Contract::new(Expr::Bool(true), Expr::Bool(true)),
            llvm_body: vec!["%res = add i64 %x, %y;".into(), "term %res;".into()],
            fallback: Some(Expr::Add(
                Box::new(Expr::Identifier("x".into())),
                Box::new(Expr::Identifier("y".into())),
            )),
            has_side_effects: false,
            has_state_access: false,
            section: None,
            llvm_body_spans: vec![],
            span: None,
        };
        i.inop_decls.insert("sadd".to_string(), inop);
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UserDefined("sadd".to_string()),
            args: vec![Expr::Decimal(3), Expr::Decimal(7)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(10)), "inop# fallback should compute 3 + 7 = 10");
    }

    #[test]
    fn test_inop_fallback_missing_decl_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UserDefined("nonexistent".to_string()),
            args: vec![],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "missing inop# declaration should error");
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("unknown"), "error should mention 'unknown', got: {}", err);
    }

    #[test]
    fn test_inop_fallback_missing_body_error() {
        let mut i = Interpreter::new();
        let inop = InopDeclaration {
            name: "no_fallback".into(),
            type_params: vec![],
            params: vec![("x".into(), Type::int())],
            outputs: vec![Type::int()],
            contract: crate::ast::Contract::new(Expr::Bool(true), Expr::Bool(true)),
            llvm_body: vec!["%res = add i64 %x, %x;".into(), "term %res;".into()],
            fallback: None,
            has_side_effects: false,
            has_state_access: false,
            section: None,
            llvm_body_spans: vec![],
            span: None,
        };
        i.inop_decls.insert("void_inop".to_string(), inop);
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UserDefined("void_inop".to_string()),
            args: vec![],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "inop# without fallback should error in interpreter");
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("no fallback"), "error should mention 'no fallback', got: {}", err);
    }

    #[test]
    fn test_custom_insert_strategy_dispatch() {
        let mut i = Interpreter::new();
        use crate::ast::{TopLevel, Program, TypeDef, TypeDefBody, TypeBinding, InopDeclaration, Contract, Statement, Expr, Type, ArrowDir};
        use crate::interpreter::Value;
        let td = TypeDef {
            name: "MyList".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("List".into())),
            body: TypeDefBody {
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![TypeBinding {
                    name: "InsertAt".into(),
                    params: vec![],
                    value: Box::new(Expr::Identifier("my_insert".into())),
                    span: None,
                }],
                operators: vec![], constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = Program {
            items: vec![TopLevel::TypeDef(Box::new(td))],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::interpreter::StrictMode::Off,
            dispatch_mode: crate::interpreter::DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
            watchdog_defaults: (None, None),
        };
        i.type_universe = Some(crate::type_universe::TypeUniverse::build(&program));
        // Set up an inop "my_insert" whose fallback returns a known value
        i.inop_decls.insert("my_insert".into(), InopDeclaration {
            name: "my_insert".into(),
            type_params: vec![],
            params: vec![("list".into(), Type::Void), ("val".into(), Type::Void)],
            outputs: vec![Type::int()],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            llvm_body: vec![],
            fallback: Some(Expr::Decimal(999)),
            has_side_effects: false,
            has_state_access: false,
            section: None,
            llvm_body_spans: vec![],
            span: None,
        });
        // Declare variable with MyList type
        let let_stmt = Statement::Let {
            name: "x".into(),
            ty: Some(Type::Custom("MyList".into())),
            expr: Some(Expr::ListLiteral(vec![Expr::Decimal(10)])),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: None,
            is_override: false,
            modifiers: vec![],
        };
        i.exec_stmt(&let_stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::List(vec![Value::Bits(i64_to_bits(10))])));
        // Execute &x <- 42 — should dispatch through Custom("my_insert")
        let push_stmt = Statement::Expression(Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("x".into())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Decimal(42))),
        });
        let result = i.exec_stmt(&push_stmt);
        assert!(result.is_ok(), "ArrowMut Push should succeed: {:?}", result);
        // The custom inop fallback returns Int(999), which replaces the collection value
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(999))),
            "Custom strategy should have dispatched to my_insert# which returns 999");
    }

    #[test]
    fn test_custom_extract_strategy_dispatch() {
        let mut i = Interpreter::new();
        use crate::ast::{TopLevel, Program, TypeDef, TypeDefBody, TypeBinding, InopDeclaration, Contract, Statement, Expr, Type, ArrowDir};
        use crate::interpreter::Value;
        // Set up type "MyQueue" with ExtractFrom = "my_extract"
        let td = TypeDef {
            name: "MyQueue".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("List".into())),
            body: TypeDefBody {
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![TypeBinding {
                    name: "ExtractFrom".into(),
                    params: vec![],
                    value: Box::new(Expr::Identifier("my_extract".into())),
                    span: None,
                }],
                operators: vec![], constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = Program {
            items: vec![TopLevel::TypeDef(Box::new(td))],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::interpreter::StrictMode::Off,
            dispatch_mode: crate::interpreter::DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
            watchdog_defaults: (None, None),
        };
        i.type_universe = Some(crate::type_universe::TypeUniverse::build(&program));
        // Inop "my_extract" fallback returns (pushed_value, new_collection) as a 2-element tuple
        i.inop_decls.insert("my_extract".into(), InopDeclaration {
            name: "my_extract".into(),
            type_params: vec![],
            params: vec![("list".into(), Type::Void)],
            outputs: vec![Type::int(), Type::Void],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            llvm_body: vec![],
            fallback: Some(Expr::ListLiteral(vec![
                Expr::Decimal(777),
                Expr::ListLiteral(vec![]),
            ])),
            has_side_effects: false,
            has_state_access: false,
            section: None,
            llvm_body_spans: vec![],
            span: None,
        });
        // Declare variable with MyQueue type
        let let_stmt = Statement::Let {
            name: "q".into(),
            ty: Some(Type::Custom("MyQueue".into())),
            expr: Some(Expr::ListLiteral(vec![Expr::Decimal(1), Expr::Decimal(2)])),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: None,
            is_override: false,
            modifiers: vec![],
        };
        i.exec_stmt(&let_stmt).unwrap();
        // Execute val <- &q — should dispatch through Custom("my_extract")
        let pop_stmt = Statement::Expression(Expr::ArrowMut {
            dir: ArrowDir::Pop, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("q".into())))),
            index: Box::new(Expr::Term),
            value: None,
        });
        let result = i.exec_stmt(&pop_stmt);
        assert!(result.is_ok(), "ArrowMut Pop should succeed: {:?}", result);
        // The custom inop fallback returns ([777, []], []). The first element (777) is
        // the popped value. The second element ([]) is the new collection.
        assert_eq!(i.state.get("q"), Some(&Value::List(vec![])),
            "Custom extract should have updated collection to the second return element");
    }

    #[test]
    fn test_custom_insert_strategy_with_defn() {
        let mut i = Interpreter::new();
        use crate::ast::{TopLevel, Program, TypeDef, TypeDefBody, TypeBinding, Definition, Statement, Expr, Type, ArrowDir};
        use crate::interpreter::Value;
        // Set up type "SList" with InsertAt = "sl_insert_fn"
        let td = TypeDef {
            name: "SList".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("List".into())),
            body: TypeDefBody {
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![TypeBinding {
                    name: "InsertAt".into(),
                    params: vec![],
                    value: Box::new(Expr::Identifier("sl_insert_fn".into())),
                    span: None,
                }],
                operators: vec![], constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = Program {
            items: vec![TopLevel::TypeDef(Box::new(td))],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::interpreter::StrictMode::Off,
            dispatch_mode: crate::interpreter::DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
            watchdog_defaults: (None, None),
        };
        i.type_universe = Some(crate::type_universe::TypeUniverse::build(&program));
        // Define a defn that appends and adds a sentinel value
        let defn = Definition {
            name: "sl_insert_fn".into(),
            type_params: vec![],
            parameters: vec![("l".into(), Type::Void), ("v".into(), Type::Void)],
            outputs: vec![Type::Void],
            output_type: None,
            output_names: vec![],
            contract: crate::ast::Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![
                Statement::Term {
                    values: vec![Some(Expr::ArrowMut {
                        dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("l".into())))),
                        index: Box::new(Expr::Term),
                        value: Some(Box::new(Expr::Identifier("v".into()))),
                    })],
                    swan_song: None,
                    modifiers: vec![],
                },
            ],
            is_lambda: false,
            annotations: vec![],
            metadata: HashMap::new(),
            modifiers: vec![],
            variant_bodies: vec![],
            derivation: None,
        };
        i.definitions.insert("sl_insert_fn".into(), defn);
        // Declare variable with SList type
        let let_stmt = Statement::Let {
            name: "s".into(),
            ty: Some(Type::Custom("SList".into())),
            expr: Some(Expr::ListLiteral(vec![Expr::Decimal(1)])),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: None,
            is_override: false,
            modifiers: vec![],
        };
        i.exec_stmt(&let_stmt).unwrap();
        // Execute &s <- 42 — should dispatch through Custom("sl_insert_fn")
        let push_stmt = Statement::Expression(Expr::ArrowMut {
            dir: ArrowDir::Push, consume: false, target: Box::new(Expr::AddrOf(Box::new(Expr::Identifier("s".into())))),
            index: Box::new(Expr::Term),
            value: Some(Box::new(Expr::Decimal(42))),
        });
        let result = i.exec_stmt(&push_stmt);
        assert!(result.is_ok(), "ArrowMut Push via defn should succeed: {:?}", result);
        // The defn appends 42 to the list
        assert_eq!(i.state.get("s"), Some(&Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(42))])),
            "Custom strategy via defn should have appended 42");
    }

    #[test]
    fn test_eval_meld_cast_identity() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(
            Box::new(Expr::Decimal(42)),
            Type::Custom("CString".to_string()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(42)),
            "meld-backed cast should return identity value");
    }

    #[test]
    fn test_eval_meld_cast_string_identity() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(
            Box::new(Expr::Quoted("hello".into())),
            Type::Custom("CString".to_string()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits("hello".to_string().into_bytes()),
            "meld-backed cast of String should return identity");
    }

    // ── Cell primitive tests ──────────────────────────────────────

    #[test]
    fn test_cell_simple() {
        let mut interp = Interpreter::new();
        let cell_def = CellDef {
            is_persistent: false,
            name: "add_one".to_string(), type_params: vec![],
            parameters: vec![("x".to_string(), Type::int())],
            output_type: Some(OutputType::Named("val".to_string(), Box::new(OutputType::Single(Type::int())))),
            fields: vec![
                StructField { name: "val".to_string(), ty: Type::int(), default: Some(Expr::Decimal(0)), visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "compute".to_string(), is_async: false, is_reactive: true,
                parameters: vec![], contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![
                    Statement::Assignment { lhs: Expr::Identifier("val".to_string()), expr: Expr::Add(Box::new(Expr::Identifier("x".to_string())), Box::new(Expr::Decimal(1))), timeout: None, modifiers: vec![] },
                    Statement::Term { values: vec![None], swan_song: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![Type::int()], output_type: None, derivation: None,
            }],
            definitions: vec![], internal_triggers: vec![],
            span: None, modifiers: vec![],
        };
        interp.cell_defs.insert("add_one".to_string(), cell_def.clone());
        let call = Expr::CellCall(Box::new(Expr::Identifier("add_one".to_string())), vec![Expr::Decimal(41)]);
        let result = interp.eval_expr(&call).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(42)));
    }

    #[test]
    fn test_cell_loop_convergence() {
        let mut interp = Interpreter::new();
        let cell_def = CellDef {
            is_persistent: false,
            name: "countdown".to_string(), type_params: vec![],
            parameters: vec![("start".to_string(), Type::int())],
            output_type: Some(OutputType::Named("counter".to_string(), Box::new(OutputType::Single(Type::int())))),
            fields: vec![
                StructField { name: "counter".to_string(), ty: Type::int(), default: None, visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "dec".to_string(), is_async: false, is_reactive: true,
                parameters: vec![],
                contract: Contract::new(Expr::Gt(Box::new(Expr::Identifier("counter".to_string())), Box::new(Expr::Decimal(0))), Expr::Bool(true)),
                body: vec![
                    Statement::Assignment { lhs: Expr::Identifier("counter".to_string()), expr: Expr::Sub(Box::new(Expr::Identifier("counter".to_string())), Box::new(Expr::Decimal(1))), timeout: None, modifiers: vec![] },
                    Statement::Term { values: vec![None], swan_song: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![], output_type: None, derivation: None,
            }],
            definitions: vec![], internal_triggers: vec![],
            span: None, modifiers: vec![],
        };
        interp.cell_defs.insert("countdown".to_string(), cell_def.clone());
        let call = Expr::CellCall(Box::new(Expr::Identifier("countdown".to_string())), vec![Expr::Decimal(3)]);
        let result = interp.eval_expr(&call).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)));
    }

    #[test]
    fn test_cell_no_output() {
        let mut interp = Interpreter::new();
        let cell_def = CellDef {
            is_persistent: false,
            name: "noop".to_string(), type_params: vec![],
            parameters: vec![],
            output_type: None,
            fields: vec![
                StructField { name: "ran".to_string(), ty: Type::bool_(), default: Some(Expr::Bool(false)), visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "do_it".to_string(), is_async: false, is_reactive: true,
                parameters: vec![],
                contract: Contract::new(Expr::Not(Box::new(Expr::Identifier("ran".to_string()))), Expr::Bool(true)),
                body: vec![
                    Statement::Assignment { lhs: Expr::Identifier("ran".to_string()), expr: Expr::Bool(true), timeout: None, modifiers: vec![] },
                    Statement::Term { values: vec![None], swan_song: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![], output_type: None, derivation: None,
            }],
            definitions: vec![], internal_triggers: vec![],
            span: None, modifiers: vec![],
        };
        interp.cell_defs.insert("noop".to_string(), cell_def.clone());
        let call = Expr::CellCall(Box::new(Expr::Identifier("noop".to_string())), vec![]);
        let result = interp.eval_expr(&call).unwrap();
        assert_eq!(result, Value::Void);
    }

    #[test]
    fn test_cell_term_bang_exits_early() {
        let mut interp = Interpreter::new();
        let cell_def = CellDef {
            is_persistent: false,
            name: "early_exit".to_string(), type_params: vec![],
            parameters: vec![],
            output_type: Some(OutputType::Named("counter".to_string(), Box::new(OutputType::Single(Type::int())))),
            fields: vec![
                StructField { name: "counter".to_string(), ty: Type::int(), default: None, visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "stop_early".to_string(), is_async: false, is_reactive: true,
                parameters: vec![],
                contract: Contract::new(Expr::Eq(Box::new(Expr::Identifier("counter".to_string())), Box::new(Expr::Decimal(0))), Expr::Bool(true)),
                body: vec![
                    Statement::TermBang { values: vec![Some(Expr::Decimal(99))], swan_song: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![], output_type: None, derivation: None,
            }],
            definitions: vec![], internal_triggers: vec![],
            span: None, modifiers: vec![],
        };
        interp.cell_defs.insert("early_exit".to_string(), cell_def.clone());
        let call = Expr::CellCall(Box::new(Expr::Identifier("early_exit".to_string())), vec![]);
        let result = interp.eval_expr(&call).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(99)));
    }

    #[test]
    fn test_cell_persistent() {
        let mut interp = Interpreter::new();
        let cell_def = CellDef {
            is_persistent: true,
            name: "counter".to_string(), type_params: vec![],
            parameters: vec![],
            output_type: Some(OutputType::Named("val".to_string(), Box::new(OutputType::Single(Type::int())))),
            fields: vec![
                StructField { name: "val".to_string(), ty: Type::int(), default: Some(Expr::Decimal(0)), visibility: Visibility::Private },
                StructField { name: "fired".to_string(), ty: Type::bool_(), default: Some(Expr::Bool(false)), visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "inc".to_string(), is_async: false, is_reactive: true,
                parameters: vec![],
                contract: Contract::new(Expr::Not(Box::new(Expr::Identifier("fired".to_string()))), Expr::Bool(true)),
                body: vec![
                    Statement::Assignment { lhs: Expr::Identifier("val".to_string()), expr: Expr::Add(Box::new(Expr::Identifier("val".to_string())), Box::new(Expr::Decimal(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: Expr::Identifier("fired".to_string()), expr: Expr::Bool(true), timeout: None, modifiers: vec![] },
                    Statement::Term { values: vec![None], swan_song: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![Type::int()], output_type: None, derivation: None,
            }],
            definitions: vec![], internal_triggers: vec![], span: None, modifiers: vec![],
        };
        interp.cell_defs.insert("counter".to_string(), cell_def.clone());

        // Register the persistent cell
        let counter_def = interp.cell_defs["counter"].clone();
        interp.register_persistent_cell(&counter_def, &[], None).unwrap();

        // Initial output (before any tick): val = 0 (default)
        let call = Expr::CellCall(Box::new(Expr::Identifier("counter".to_string())), vec![]);
        let r0 = interp.eval_expr(&call).unwrap();
        assert_eq!(r0, Value::Bits(i64_to_bits(0)), "before first tick: val = 0 (default)");

        // Tick the cell: fired=false → !fired=true → fires → val=0+1=1, fired=true
        interp.tick_persistent_cells().unwrap();

        // Now call_cell returns current output: val = 1
        let r1 = interp.eval_expr(&call).unwrap();
        assert_eq!(r1, Value::Bits(i64_to_bits(1)), "after first tick: val = 0 + 1 = 1");

        // Second tick: fired=true → !fired=false → doesn't fire → val stays 1
        interp.tick_persistent_cells().unwrap();
        let r2 = interp.eval_expr(&call).unwrap();
        assert_eq!(r2, Value::Bits(i64_to_bits(1)), "second tick: precondition !fired is false, val stays 1");

        // Reset fired in saved state — demonstrate persistence
        let saved = interp.persistent_cells.get_mut("counter").unwrap();
        saved.state.insert("counter$0.fired".to_string(), Value::Bits(vec![0u8]));

        // Tick again: fired=false → !fired=true → fires → val=1+1=2
        interp.tick_persistent_cells().unwrap();
        let r3 = interp.eval_expr(&call).unwrap();
        assert_eq!(r3, Value::Bits(i64_to_bits(2)), "after resetting fired and ticking: val = 1 + 1 = 2");
    }

    #[test]
    fn test_cell_to_cell_wire() {
        let mut interp = Interpreter::new();

        // Producer cell: persistent, has output port `val`, counts up each tick
        let producer = CellDef {
            is_persistent: true,
            name: "producer".to_string(), type_params: vec![],
            parameters: vec![],
            output_type: Some(OutputType::Named("val".to_string(), Box::new(OutputType::Single(Type::int())))),
            fields: vec![
                StructField { name: "val".to_string(), ty: Type::int(), default: Some(Expr::Decimal(0)), visibility: Visibility::Private },
                StructField { name: "fired".to_string(), ty: Type::bool_(), default: Some(Expr::Bool(false)), visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "inc".to_string(), is_async: false, is_reactive: true,
                parameters: vec![],
                contract: Contract::new(Expr::Not(Box::new(Expr::Identifier("fired".to_string()))), Expr::Bool(true)),
                body: vec![
                    Statement::Assignment { lhs: Expr::Identifier("val".to_string()), expr: Expr::Add(Box::new(Expr::Identifier("val".to_string())), Box::new(Expr::Decimal(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: Expr::Identifier("fired".to_string()), expr: Expr::Bool(true), timeout: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![], output_type: None, derivation: None,
            }],
            definitions: vec![], internal_triggers: vec![], span: None, modifiers: vec![],
        };

        // Consumer cell: persistent, takes `input` param, echoes it as `out`
        let consumer = CellDef {
            is_persistent: true,
            name: "consumer".to_string(), type_params: vec![],
            parameters: vec![("input".to_string(), Type::int())],
            output_type: Some(OutputType::Named("out".to_string(), Box::new(OutputType::Single(Type::int())))),
            fields: vec![
                StructField { name: "out".to_string(), ty: Type::int(), default: Some(Expr::Decimal(0)), visibility: Visibility::Private },
                StructField { name: "input".to_string(), ty: Type::int(), default: None, visibility: Visibility::Private },
                StructField { name: "fired".to_string(), ty: Type::bool_(), default: Some(Expr::Bool(false)), visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "forward".to_string(), is_async: false, is_reactive: true,
                parameters: vec![],
                contract: Contract::new(Expr::Not(Box::new(Expr::Identifier("fired".to_string()))), Expr::Bool(true)),
                body: vec![
                    Statement::Assignment { lhs: Expr::Identifier("out".to_string()), expr: Expr::Identifier("input".to_string()), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: Expr::Identifier("fired".to_string()), expr: Expr::Bool(true), timeout: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![], output_type: None, derivation: None,
            }],
            definitions: vec![], internal_triggers: vec![], span: None, modifiers: vec![],
        };

        interp.cell_defs.insert("producer".to_string(), producer.clone());
        interp.cell_defs.insert("consumer".to_string(), consumer.clone());

        // Register both cells as persistent
        interp.register_persistent_cell(&producer, &[], None).unwrap();
        interp.register_persistent_cell(&consumer, &[Value::Bits(i64_to_bits(0))], None).unwrap();

        // Add a wire: producer.val → consumer.input
        interp.cell_wires.push(CellWire {
            from_cell: "producer".to_string(),
            from_port: "val".to_string(),
            to_cell: "consumer".to_string(),
            to_param: "input".to_string(),
        });

        // Initial state: producer.val=0, consumer.input=0, consumer.out=0
        let prod_val = interp.call_cell(&producer, &[]).unwrap();
        assert_eq!(prod_val, Value::Bits(i64_to_bits(0)), "producer initial val");
        let cons_val = interp.call_cell(&consumer, &[]).unwrap();
        assert_eq!(cons_val, Value::Bits(i64_to_bits(0)), "consumer initial out");

        // Tick persistent cells: producer fires (fired=false, val→1), wire should propagate
        interp.tick_persistent_cells().unwrap();

        // After tick: producer.val=1, consumer should have received it via wire
        let prod_val = interp.call_cell(&producer, &[]).unwrap();
        assert_eq!(prod_val, Value::Bits(i64_to_bits(1)), "producer.val after one tick");

        // Wire should have propagated producer.val (1) to consumer.input
        // Consumer should have ticked too, setting consumer.out = consumer.input = 1
        let cons_val = interp.call_cell(&consumer, &[]).unwrap();
        assert_eq!(cons_val, Value::Bits(i64_to_bits(1)), "consumer.out after wire propagation");
    }

    #[test]
    fn test_cell_internal_trigger_stdin() {
        // Verify that a cell with an internal trg @ stdin# evaluates the trigger
        // before running transactions. tty_read_key# returns -1 when no key is
        // available, which should become Char('\0') for a Char-typed trigger.
        let mut interp = Interpreter::new();
        let cell_def = CellDef {
            is_persistent: false,
            name: "reader".to_string(),
            type_params: vec![],
            parameters: vec![],
            output_type: Some(OutputType::Named("captured".to_string(), Box::new(OutputType::Single(Type::char_())))),
            fields: vec![
                StructField { name: "captured".to_string(), ty: Type::char_(), default: Some(Expr::Char('\0')), visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "capture".to_string(), is_async: false, is_reactive: true,
                parameters: vec![],
                // Fire when trigger 'raw' differs from saved 'prev'
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![
                    // Store the trigger value into the output field so we can inspect it
                    Statement::Assignment {
                        lhs: Expr::Identifier("captured".to_string()),
                        expr: Expr::Identifier("raw".to_string()),
                        timeout: None, modifiers: vec![],
                    },
                    Statement::Term { values: vec![None], swan_song: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![], output_type: None, derivation: None,
            }],
            definitions: vec![],
            internal_triggers: vec![TriggerDeclaration {
                name: "raw".to_string(),
                ty: Type::char_(),
                address: LinkRef::Stdin,
                bit_range: None,
                stages: vec![],
                condition: None,
                is_wake: false,
                is_const: false,
                span: None,
                annotations: vec![],
                modifiers: vec![],
            }],
            span: None, modifiers: vec![],
        };
        interp.cell_defs.insert("reader".to_string(), cell_def.clone());

        // Call the sync cell — internal trigger should be evaluated before convergence
        let call = Expr::CellCall(Box::new(Expr::Identifier("reader".to_string())), vec![]);
        let result = interp.eval_expr(&call).unwrap();

        // tty_read_key# returns -1 when no key is available, which we convert to Char('\0')
        // Char is encoded as u32 (4 bytes), not i64 (8 bytes)
        assert_eq!(result, Value::Bits(('\0' as u32).to_le_bytes().to_vec()),
            "cell with internal stdin trigger should capture tty_read_key result as Char");
    }

    #[test]
    fn test_console_cell_accumulate_and_emit() {
        // Test that a Console-like cell with internal stdin trigger correctly
        // accumulates characters, emits a line on Enter with line_id, and
        // handles backspace. Uses a persistent cell with direct state
        // manipulation to simulate trigger values (since tty_read_key returns
        // -1 in the test environment).
        let mut interp = Interpreter::new();

        let console_def = CellDef {
            is_persistent: true,
            name: "Console".to_string(), type_params: vec![],
            parameters: vec![],
            output_type: Some(OutputType::Tuple(vec![
                OutputType::Named("line".to_string(), Box::new(OutputType::Single(Type::string()))),
                OutputType::Named("line_id".to_string(), Box::new(OutputType::Single(Type::int()))),
            ])),
            fields: vec![
                StructField { name: "buffer".to_string(), ty: Type::string(), default: Some(Expr::Quoted(Vec::new())), visibility: Visibility::Private },
                StructField { name: "prev_key".to_string(), ty: Type::char_(), default: Some(Expr::Char('\0')), visibility: Visibility::Private },
                StructField { name: "seq".to_string(), ty: Type::int(), default: Some(Expr::Decimal(0)), visibility: Visibility::Private },
                // line and line_id are output ports — registered as fields with defaults
                StructField { name: "line".to_string(), ty: Type::string(), default: Some(Expr::Quoted(Vec::new())), visibility: Visibility::Private },
                StructField { name: "line_id".to_string(), ty: Type::int(), default: Some(Expr::Decimal(0)), visibility: Visibility::Private },
                // raw is the trigger input — managed manually in this test
                StructField { name: "raw".to_string(), ty: Type::char_(), default: Some(Expr::Char('\0')), visibility: Visibility::Private },
            ],
            transactions: vec![Transaction {
                name: "process".to_string(), is_async: false, is_reactive: true,
                parameters: vec![],
                // Pre: raw has a new value (different from prev_key)
                contract: Contract::new(
                    Expr::Ne(Box::new(Expr::Identifier("raw".to_string())), Box::new(Expr::Identifier("prev_key".to_string()))),
                    Expr::Bool(true),
                ),
                body: vec![
                    Statement::Guarded {
                        condition: Expr::Eq(Box::new(Expr::Identifier("raw".to_string())), Box::new(Expr::Char('\n'))),
                        statements: vec![
                            Statement::Assignment { lhs: Expr::Identifier("line".to_string()), expr: Expr::Identifier("buffer".to_string()), timeout: None, modifiers: vec![] },
                            Statement::Assignment { lhs: Expr::Identifier("seq".to_string()), expr: Expr::Add(Box::new(Expr::Identifier("seq".to_string())), Box::new(Expr::Decimal(1))), timeout: None, modifiers: vec![] },
                            Statement::Assignment { lhs: Expr::Identifier("line_id".to_string()), expr: Expr::Identifier("seq".to_string()), timeout: None, modifiers: vec![] },
                            Statement::Assignment { lhs: Expr::Identifier("buffer".to_string()), expr: Expr::Quoted(Vec::new()), timeout: None, modifiers: vec![] },
                        ],
                        metadata: HashMap::new(),
                    },
                    Statement::Guarded {
                        condition: Expr::And(
                            Box::new(Expr::Eq(Box::new(Expr::Identifier("raw".to_string())), Box::new(Expr::Char('\x7f')))),
                            Box::new(Expr::Gt(Box::new(Expr::Projection { source: Box::new(Expr::Identifier("buffer".to_string())), target: ProjectionTarget::Size }), Box::new(Expr::Decimal(0)))),
                        ),
                        statements: vec![
                            Statement::Assignment {
                                lhs: Expr::Identifier("buffer".to_string()),
                                expr: Expr::Slice {
                                    value: Box::new(Expr::Identifier("buffer".to_string())),
                                    start: Some(Box::new(Expr::Decimal(0))),
                                    end: Some(Box::new(Expr::Sub(
                                        Box::new(Expr::Projection { source: Box::new(Expr::Identifier("buffer".to_string())), target: ProjectionTarget::Size }),
                                        Box::new(Expr::Decimal(1)),
                                    ))),
                                    stride: None, mask: None,
                                },
                                timeout: None, modifiers: vec![],
                            },
                        ],
                        metadata: HashMap::new(),
                    },
                    Statement::Guarded {
                        condition: Expr::And(
                            Box::new(Expr::Ge(Box::new(Expr::Identifier("raw".to_string())), Box::new(Expr::Char(' ')))),
                            Box::new(Expr::Ne(Box::new(Expr::Identifier("raw".to_string())), Box::new(Expr::Char('\x7f')))),
                        ),
                        statements: vec![
                            Statement::Assignment {
                                lhs: Expr::Identifier("buffer".to_string()),
                                expr: Expr::Add(
                                    Box::new(Expr::Identifier("buffer".to_string())),
                                    Box::new(Expr::Cast(Box::new(Expr::Identifier("raw".to_string())), Type::string())),
                                ),
                                timeout: None, modifiers: vec![],
                            },
                        ],
                        metadata: HashMap::new(),
                    },
                    Statement::Assignment { lhs: Expr::Identifier("prev_key".to_string()), expr: Expr::Identifier("raw".to_string()), timeout: None, modifiers: vec![] },
                    Statement::Term { values: vec![None], swan_song: None, modifiers: vec![] },
                ],
                reactor_speed: None, span: None, is_lambda: false, dependencies: vec![],
 modifiers: vec![], variant_bodies: vec![],
                annotations: vec![],
                metadata: HashMap::new(),
                outputs: vec![], output_type: None, derivation: None,
            }],
            definitions: vec![],
            // Note: Internal trigger is NOT declared here — we manually manage
            // the `raw` field to simulate keypresses in the test.
            // Internal trigger evaluation is tested separately in
            // test_cell_internal_trigger_stdin.
            internal_triggers: vec![],
            span: None, modifiers: vec![],
        };
        interp.cell_defs.insert("Console".to_string(), console_def.clone());

        // Register the persistent Console cell
        interp.register_persistent_cell(&console_def, &[], None).unwrap();

        // Initial tick: no key available (tty_read_key returns -1 → '\0')
        interp.tick_persistent_cells().unwrap();
        let line_key = "Console$0.line".to_string();
        let line_id_key = "Console$0.line_id".to_string();
        let buffer_key = "Console$0.buffer".to_string();
        let prev_key = "Console$0.prev_key".to_string();

        // Initial state: line = "", line_id = 0, buffer = ""
        assert_eq!(interp.persistent_cells["Console"].state.get(&line_key), Some(&Value::Bits("".to_string().into_bytes())));
        assert_eq!(interp.persistent_cells["Console"].state.get(&line_id_key), Some(&Value::Bits(i64_to_bits(0))));

        // Simulate typing 'h' by setting trigger value directly
        // Char encoding is u32 (4 bytes), not i64 (8 bytes)
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('h' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        // After tick: buffer = "h", prev_key = 'h'
        assert_eq!(interp.persistent_cells["Console"].state.get(&buffer_key), Some(&Value::Bits("h".to_string().into_bytes())),
            "buffer should be 'h' after typing 'h'");

        // Type 'e'
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('e' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        assert_eq!(interp.persistent_cells["Console"].state.get(&buffer_key), Some(&Value::Bits("he".to_string().into_bytes())),
            "buffer should be 'he' after typing 'e'");

        // Type 'y'
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('y' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        assert_eq!(interp.persistent_cells["Console"].state.get(&buffer_key), Some(&Value::Bits("hey".to_string().into_bytes())),
            "buffer should be 'hey' after typing 'y'");

        // Press Enter: emit line
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('\n' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        assert_eq!(interp.persistent_cells["Console"].state.get(&buffer_key), Some(&Value::Bits("".to_string().into_bytes())),
            "buffer should be empty after Enter");
        assert_eq!(interp.persistent_cells["Console"].state.get(&line_key), Some(&Value::Bits("hey".to_string().into_bytes())),
            "line should be 'hey' after Enter");
        assert_eq!(interp.persistent_cells["Console"].state.get(&line_id_key), Some(&Value::Bits(i64_to_bits(1))),
            "line_id should be 1 after first Enter");

        // Press Enter again with same input (duplicate): line_id must increment
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('h' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('i' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('\n' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        assert_eq!(interp.persistent_cells["Console"].state.get(&line_key), Some(&Value::Bits("hi".to_string().into_bytes())),
            "second line should be 'hi'");
        assert_eq!(interp.persistent_cells["Console"].state.get(&line_id_key), Some(&Value::Bits(i64_to_bits(2))),
            "line_id should be 2 after second Enter");

        // Now test duplicate: type "hi" again and Enter
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('h' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('i' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('\n' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(i64_to_bits('\0' as i64)));
        }
        interp.tick_persistent_cells().unwrap();
        // Same line value but line_id incremented
        assert_eq!(interp.persistent_cells["Console"].state.get(&line_key), Some(&Value::Bits("hi".to_string().into_bytes())),
            "duplicate input: line should still be 'hi'");
        assert_eq!(interp.persistent_cells["Console"].state.get(&line_id_key), Some(&Value::Bits(i64_to_bits(3))),
            "duplicate input: line_id must increment to 3");

        // Test backspace
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('a' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('b' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        assert_eq!(interp.persistent_cells["Console"].state.get(&buffer_key), Some(&Value::Bits("ab".to_string().into_bytes())),
            "buffer should be 'ab' after typing 'ab'");
        // Backspace
        {
            let inst = interp.persistent_cells.get_mut("Console").unwrap();
            inst.state.insert("Console$0.raw".to_string(), Value::Bits(('\x7f' as u32).to_le_bytes().to_vec()));
            inst.state.insert(prev_key.clone(), Value::Bits(('\0' as u32).to_le_bytes().to_vec()));
        }
        interp.tick_persistent_cells().unwrap();
        assert_eq!(interp.persistent_cells["Console"].state.get(&buffer_key), Some(&Value::Bits("a".to_string().into_bytes())),
            "buffer should be 'a' after backspace");
    }

    // --- Ptr<T> tests ---

    #[test]
    fn test_eval_cast_int_to_ptr() {
        let mut i = Interpreter::new();
        let ptr_ty = Type::Applied("Ptr".to_string(), vec![Type::int()]);
        let expr = Expr::Cast(Box::new(Expr::Decimal(0x40011000)), ptr_ty);
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0x40011000)), "Int -> Ptr should wrap address");
    }

    #[test]
    fn test_eval_cast_ptr_to_int() {
        let mut i = Interpreter::new();
        let ptr_ty = Type::Applied("Ptr".to_string(), vec![Type::int()]);
        let ptr_expr = Expr::Cast(Box::new(Expr::Decimal(0x40011004)), ptr_ty);
        let cast_back = Expr::Cast(ptr_expr.into(), Type::int());
        let result = i.eval_expr(&cast_back).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0x40011004)), "Ptr -> Int should extract address");
    }

    #[test]
    fn test_ptr_display() {
        let p = Value::Bits(i64_to_bits(0x40011000));
        // Ptr variant removed — Bits displays as byte count
        assert_eq!(format!("{}", p), "<Bits 8>");
    }

    #[test]
    fn test_is_valid_ffi_return_ptr() {
        let ptr_ty = Type::Applied("Ptr".to_string(), vec![Type::int()]);
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits(i64_to_bits(0x1000)), &ptr_ty));
        assert!(!Interpreter::is_valid_ffi_return(&Value::Bits(i64_to_bits(0)), &ptr_ty));
    }

    // --- Volatile load/store tests ---

    #[test]
    fn test_volatile_load_returns_zero() {
        let mut i = Interpreter::new();
        let ptr_ty = Type::Applied("Ptr".to_string(), vec![Type::int()]);
        let ptr_expr = Expr::Cast(Box::new(Expr::Decimal(0x40011000)), ptr_ty);
        let vl = Expr::IntrinsicCall {
            intrinsic: Intrinsic::VolatileLoad,
            args: vec![ptr_expr],
        };
        let result = i.eval_expr(&vl).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)), "volatile_load returns 0 in interpreter");
    }

    #[test]
    fn test_volatile_load_requires_ptr() {
        let mut i = Interpreter::new();
        // With Bits-only: all scalar values are Bits. volatile_load accepts any Bits.
        // Type-level checking catches non-Ptr arguments at compile time.
        // At runtime, any Bits value is a valid "address" for the interpreter.
        let ptr_ty = Type::Applied("Ptr".to_string(), vec![Type::int()]);
        let ptr_expr = Expr::Cast(Box::new(Expr::Decimal(42)), ptr_ty);
        let vl = Expr::IntrinsicCall {
            intrinsic: Intrinsic::VolatileLoad,
            args: vec![ptr_expr],
        };
        let result = i.eval_expr(&vl).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)), "volatile_load returns 0 in interpreter");
    }

    #[test]
    fn test_volatile_store_returns_true() {
        let mut i = Interpreter::new();
        let ptr_ty = Type::Applied("Ptr".to_string(), vec![Type::int()]);
        let ptr_expr = Expr::Cast(Box::new(Expr::Decimal(0x40011000)), ptr_ty);
        let vs = Expr::IntrinsicCall {
            intrinsic: Intrinsic::VolatileStore,
            args: vec![ptr_expr, Expr::Decimal(42)],
        };
        let result = i.eval_expr(&vs).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "volatile_store returns true");
    }

    #[test]
    fn test_volatile_store_requires_ptr() {
        let mut i = Interpreter::new();
        // With Bits-only: volatile_store accepts any Bits as Ptr.
        // Type-level checking catches non-Ptr arguments at compile time.
        let ptr_ty = Type::Applied("Ptr".to_string(), vec![Type::int()]);
        let ptr_expr = Expr::Cast(Box::new(Expr::Decimal(0)), ptr_ty);
        let vs = Expr::IntrinsicCall {
            intrinsic: Intrinsic::VolatileStore,
            args: vec![ptr_expr, Expr::Decimal(1)],
        };
        let result = i.eval_expr(&vs).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "volatile_store with Ptr should succeed");
    }

    // ── AddrOf / Deref regression tests ─────────────────────────
    #[test]
    fn test_addr_of_identifier_reads_from_state() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(42)));
        let expr = Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())));
        let result = i.eval_expr(&expr).unwrap();
        // AddrOf wraps the value in Value::Ref
        assert_eq!(result, Value::Ref(Box::new(Value::Bits(i64_to_bits(42)))));
    }

    #[test]
    fn test_addr_of_undefined_var_errors() {
        let mut i = Interpreter::new();
        let expr = Expr::AddrOf(Box::new(Expr::Identifier("undefined".to_string())));
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "AddrOf of undefined variable should error");
    }

    #[test]
    fn test_addr_of_assignment_writes_to_state() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Assignment {
            lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
            expr: Expr::Decimal(99),
            timeout: None,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(99))));
    }

    #[test]
    fn test_addr_of_assignment_creates_new_state_entry() {
        let mut i = Interpreter::new();
        let stmt = Statement::Assignment {
            lhs: Expr::AddrOf(Box::new(Expr::Identifier("new_var".to_string()))),
            expr: Expr::Bool(true),
            timeout: None,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("new_var"), Some(&Value::Bits(vec![if true { 1u8 } else { 0u8 }])));
    }

    #[test]
    fn test_addr_of_assignment_in_txn_body() {
        let code = r#"
            let count: Int = 10;
            rct txn dec [count > 0][count == 0] {
                &count = count - 1;
                term;
            };
        "#;
        let mut parser = crate::parser::Parser::new(code);
        let program = parser.parse().expect("Failed to parse");
        let mut i = Interpreter::new();
        let _ = i.run(&program);
        assert_eq!(i.state.get("count"), Some(&Value::Bits(i64_to_bits(0))));
    }

    #[test]
    fn test_deref_identifier_evaluates_inner() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(42)));
        // Deref expects a Value::Ref, dereferences to inner value
        let inner = Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())));
        let expr = Expr::Deref(Box::new(inner));
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(42)));
    }

    #[test]
    fn test_deref_non_pointer_errors() {
        let mut i = Interpreter::new();
        // Dereferencing a plain Int (not a Ref) should error
        let expr = Expr::Deref(Box::new(Expr::Decimal(42)));
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "Deref of non-pointer should error");
    }

    #[test]
    fn test_addr_of_lhs_via_feature_assignment() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Bits(i64_to_bits(0)));
        let stmt = Statement::Assignment {
            lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".to_string()))),
            expr: Expr::Decimal(77),
            timeout: None,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(77))));
    }
}

#[cfg(all(feature = "kani", feature = "kani_full"))]
mod kani_full_tests {
    use super::*;


    #[kani::proof]
    fn verify_eval_expr_literal_integer() {
        let mut ctx = Interpreter::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let result = ctx.eval_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(i64_to_bits(42)));
    }

    #[kani::proof]
    fn verify_eval_expr_literal_bool() {
        let mut ctx = Interpreter::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let result = ctx.eval_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(vec![1u8]));
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
        assert_eq!(result.unwrap(), Value::Bits("test".to_string().into_bytes()));
    }

    #[kani::proof]
    fn verify_eval_expr_literal_char() {
        let mut ctx = Interpreter::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Char('A')));
        let result = ctx.eval_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Bits(i64_to_bits('A' as i64)));
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
        assert_eq!(result, Value::Bits(f64_to_bits(3.0)));
    }

    #[test]
    fn test_intrinsic_fabs() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Fabs,
            args: vec![Expr::Float(-3.5)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(f64_to_bits(3.5)));
    }

    #[test]
    fn test_intrinsic_ceil() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Ceil,
            args: vec![Expr::Float(3.2)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(f64_to_bits(4.0)));
    }

    #[test]
    fn test_intrinsic_floor() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Floor,
            args: vec![Expr::Float(3.8)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(f64_to_bits(3.0)));
    }

    #[test]
    fn test_intrinsic_ctpop() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Ctpop,
            args: vec![Expr::Decimal(255)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(8)));
    }

    #[test]
    fn test_intrinsic_ctlz() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Ctlz,
            args: vec![Expr::Decimal(1)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(63)));
    }

    #[test]
    fn test_intrinsic_cttz() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Cttz,
            args: vec![Expr::Decimal(8)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(3)));
    }

    #[test]
    fn test_intrinsic_abs() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Abs,
            args: vec![Expr::Decimal(-42)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(42)));
    }

    #[test]
    fn test_intrinsic_bitreverse() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bitreverse,
            args: vec![Expr::Decimal(1)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1i64.reverse_bits())));
    }

    #[test]
    fn test_intrinsic_bytes_int() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ByteCount,
            args: vec![Expr::Decimal(42)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(8)));
    }

    #[test]
    fn test_intrinsic_bytes_float() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ByteCount,
            args: vec![Expr::Float(3.0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(8)));
    }

    #[test]
    fn test_intrinsic_bytes_bool() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ByteCount,
            args: vec![Expr::Bool(true)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1)));
    }

    #[test]
    fn test_intrinsic_bytes_char() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ByteCount,
            args: vec![Expr::Char('A')],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(4)));
    }

    #[test]
    fn test_intrinsic_size_list() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Size,
            args: vec![Expr::ListLiteral(vec![
                Expr::Decimal(1), Expr::Decimal(2), Expr::Decimal(3),
            ])],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(3)));
    }

    #[test]
    fn test_intrinsic_size_string() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Size,
            args: vec![Expr::Quoted("hello".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(5)));
    }

    #[test]
    fn test_intrinsic_pop() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Pop,
            args: vec![Expr::ListLiteral(vec![Expr::Decimal(1), Expr::Decimal(2)])],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(2)));
    }

    #[test]
    fn test_intrinsic_contains_list() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Contains,
            args: vec![
                Expr::ListLiteral(vec![Expr::Decimal(1), Expr::Decimal(2), Expr::Decimal(3)]),
                Expr::Decimal(1),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));
    }

    #[test]
    fn test_intrinsic_contains_list_false() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Contains,
            args: vec![
                Expr::ListLiteral(vec![Expr::Decimal(1), Expr::Decimal(2)]),
                Expr::Decimal(99),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]));
    }

    #[test]
    fn test_intrinsic_contains_string() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Contains,
            args: vec![
                Expr::Quoted("hello".into()),
                Expr::Char('e'),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));
    }

    #[test]
    fn test_intrinsic_keys() {
        let mut i = Interpreter::new();
        let mut map = std::collections::HashMap::new();
        map.insert("a".to_string(), Value::Bits(i64_to_bits(1)));
        map.insert("b".to_string(), Value::Bits(i64_to_bits(2)));
        i.state.insert("m".to_string(), Value::HashMap(map));
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Keys,
            args: vec![Expr::AddrOf(Box::new(Expr::Identifier("m".to_string())))],
        };
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::List(keys) => {
                assert_eq!(keys.len(), 2);
                assert!(keys.contains(&Value::Bits("a".to_string().into_bytes())));
                assert!(keys.contains(&Value::Bits("b".to_string().into_bytes())));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_intrinsic_values() {
        let mut i = Interpreter::new();
        let mut map = std::collections::HashMap::new();
        map.insert("a".to_string(), Value::Bits(i64_to_bits(10)));
        map.insert("b".to_string(), Value::Bits(i64_to_bits(20)));
        i.state.insert("m".to_string(), Value::HashMap(map));
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Values,
            args: vec![Expr::AddrOf(Box::new(Expr::Identifier("m".to_string())))],
        };
        let result = i.eval_expr(&expr).unwrap();
        match result {
            Value::List(vals) => {
                assert_eq!(vals.len(), 2);
                assert!(vals.contains(&Value::Bits(i64_to_bits(10))));
                assert!(vals.contains(&Value::Bits(i64_to_bits(20))));
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_intrinsic_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Sqrt,
            args: vec![Expr::Quoted("hello".into())],
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
        assert_eq!(result, Value::Bits(vec![0u8]), "tty_raw_mode#(false) should return false (not a tty)");
    }

    #[test]
    fn test_intrinsic_tty_raw_mode_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::TtyRawMode,
            args: vec![Expr::Decimal(42)],
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
        if let Value::Bits(i64_to_bits(encoded)) = result {
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
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "tty_read_key#() should return -1 when no key available");
    }

    #[test]
    fn test_intrinsic_ioctl_invalid_fd() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IoCtl,
            args: vec![Expr::Decimal(-1), Expr::Decimal(0), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        // ioctl with invalid fd returns -1, wrapped in Int
        assert!(result == Value::Bits(i64_to_bits(-1)), "ioctl#(-1,0,0) should return -1, got {:?}", result);
    }

    #[test]
    fn test_intrinsic_ioctl_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IoCtl,
            args: vec![Expr::Quoted("stdin".into()), Expr::Decimal(0), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "ioctl#(\"stdin\",...) should type error");
    }

    #[test]
    fn test_intrinsic_isatty_stdin() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::IsTty,
            args: vec![Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        // In test runner, stdin is usually piped, not a tty
        assert_eq!(result, Value::Bits(vec![0u8]), "isatty#(0) should return false in test runner");
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
            args: vec![Expr::Quoted("echo hello".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        if let Value::Bits(s_bytes) = result {
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
            args: vec![Expr::Quoted("".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        // Empty command may return empty string or error depending on platform
        assert!(matches!(result, Value::Bits(_)), "spawn_with_output#(\"\") should return String");
    }

    #[test]
    fn test_intrinsic_spawn_true() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Spawn,
            args: vec![Expr::Quoted("true".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)), "spawn#(\"true\") should return 0");
    }

    #[test]
    fn test_intrinsic_spawn_false() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Spawn,
            args: vec![Expr::Quoted("false".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1)), "spawn#(\"false\") should return 1");
    }

    #[test]
    fn test_intrinsic_spawn_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Spawn,
            args: vec![Expr::Decimal(42)],
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
            args: vec![Expr::Bool(true), Expr::Decimal(0), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_open_bad_path() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Open,
            args: vec![
                Expr::Quoted("/nonexistent_file_xyz.bv".into()),
                Expr::Decimal(0),
                Expr::Decimal(0),
            ],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "open#(bad path) should return -1");
    }

    #[test]
    fn test_intrinsic_close_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Close,
            args: vec![Expr::Quoted("fd".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_close_bad_fd() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Close,
            args: vec![Expr::Decimal(-1)],
        };
        let result = i.eval_expr(&expr).unwrap();
        #[cfg(unix)]
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "close#(-1) should return -1");
        #[cfg(not(unix))]
        assert_eq!(result, Value::Bits(i64_to_bits(-1)));
    }

    #[test]
    fn test_intrinsic_read_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Read,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Quoted("nope".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_write_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Write,
            args: vec![Expr::Decimal(1), Expr::Decimal(0), Expr::Decimal(5)],
        };
        // write# with opaque pointer returns -1 in interpreter
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "write# should return -1 in interpreter");
    }

    #[test]
    fn test_intrinsic_lseek_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::LSeek,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_lseek_bad_fd() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::LSeek,
            args: vec![Expr::Decimal(-1), Expr::Decimal(0), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "lseek#(-1,0,0) should return -1");
    }

    #[test]
    fn test_intrinsic_pread_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::PRead,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(5), Expr::Quoted("off".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_pwrite_returns_minus_one() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::PWrite,
            args: vec![Expr::Decimal(1), Expr::Decimal(0), Expr::Decimal(5), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "pwrite# should return -1 in interpreter");
    }

    #[test]
    fn test_intrinsic_stat_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Stat,
            args: vec![Expr::Decimal(42)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_stat_bad_path() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Stat,
            args: vec![Expr::Quoted("/nonexistent_stat_file.xyz".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "stat#(bad path) should return -1");
    }

    #[test]
    fn test_intrinsic_fstat_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FStat,
            args: vec![Expr::Quoted("fd".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_truncate_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FTruncate,
            args: vec![Expr::Decimal(0), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_ftruncate_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FTruncate,
            args: vec![Expr::Decimal(0), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_fsync_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FSync,
            args: vec![Expr::Quoted("fd".into())],
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
            args: vec![Expr::Decimal(0), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_fcntl_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::FCntl,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase C: Filesystem intrinsic tests ─────────────────────────

    #[test]
    fn test_intrinsic_mkdir_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MkDir,
            args: vec![Expr::Decimal(42), Expr::Decimal(0)],
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
            args: vec![Expr::Quoted("a".into()), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_symlink_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SymLink,
            args: vec![Expr::Quoted("target".into()), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_readlink_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadLink,
            args: vec![Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_readlink_bad_path() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadLink,
            args: vec![Expr::Quoted("/nonexistent_readlink.xyz".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(Vec::new()), "readlink#(bad path) should return empty string");
    }

    #[test]
    fn test_intrinsic_link_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Link,
            args: vec![Expr::Quoted("old".into()), Expr::Bool(false)],
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
        assert!(matches!(result, Value::Bits(s.into_bytes()) if !s.is_empty()), "getcwd#() should return non-empty string");
    }

    #[test]
    fn test_intrinsic_chdir_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChDir,
            args: vec![Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_readdir_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadDir,
            args: vec![Expr::Decimal(42)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_readdir_bad_path() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadDir,
            args: vec![Expr::Quoted("/nonexistent_dir_xyz".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::List(vec![]), "readdir#(bad path) should return empty list");
    }

    #[test]
    fn test_intrinsic_readdir_current_dir() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ReadDir,
            args: vec![Expr::Quoted(".".into())],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert!(matches!(result, Value::List(ref items) if !items.is_empty()), "readdir#(\".\") should return entries");
    }

    #[test]
    fn test_intrinsic_chmod_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChMod,
            args: vec![Expr::Quoted("/tmp".into()), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_chown_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ChOwn,
            args: vec![Expr::Quoted("/tmp".into()), Expr::Decimal(0), Expr::Bool(false)],
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
            args: vec![Expr::Decimal(0o022)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert!(matches!(result, Value::Bits(i64_to_bits(_))), "umask# should return Int");
    }

    #[test]
    fn test_intrinsic_access_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Access,
            args: vec![Expr::Decimal(42), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase D: Memory + Synchronization intrinsic tests ───────────

    #[test]
    fn test_intrinsic_mmap_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Mmap,
            args: vec![Expr::Decimal(0), Expr::Decimal(4096), Expr::Bool(true), Expr::Decimal(0), Expr::Decimal(-1), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err(), "mmap# with Bool prot should type error");
    }

    #[test]
    fn test_intrinsic_munmap_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MUnmap,
            args: vec![Expr::Bool(false), Expr::Decimal(4096)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_mprotect_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MProtect,
            args: vec![Expr::Decimal(0), Expr::Decimal(4096), Expr::Bool(true)],
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
            args: vec![Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert!(matches!(result, Value::Bits(i64_to_bits(_))), "brk# should return Int");
    }

    #[test]
    fn test_intrinsic_mlock_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::MLock,
            args: vec![Expr::Decimal(0), Expr::Bool(true)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_load_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicLoad,
            args: vec![Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_load_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicLoad,
            args: vec![Expr::Decimal(0), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)), "atomic_load# stub should return 0");
    }

    #[test]
    fn test_intrinsic_atomic_store_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicStore,
            args: vec![Expr::Decimal(0), Expr::Decimal(42), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_store_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicStore,
            args: vec![Expr::Decimal(0), Expr::Decimal(42), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "atomic_store# stub should return -1");
    }

    #[test]
    fn test_intrinsic_atomic_cas_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicCas,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(1), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_cas_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicCas,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(1), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)), "atomic_cas# stub should return 0");
    }

    #[test]
    fn test_intrinsic_atomic_xchg_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicXchg,
            args: vec![Expr::Decimal(0), Expr::Bool(true), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_xchg_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicXchg,
            args: vec![Expr::Decimal(0), Expr::Decimal(42), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)), "atomic_xchg# stub should return 0");
    }

    #[test]
    fn test_intrinsic_atomic_add_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicAdd,
            args: vec![Expr::Decimal(0), Expr::Bool(true), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_atomic_add_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::AtomicAdd,
            args: vec![Expr::Decimal(0), Expr::Decimal(1), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)), "atomic_add# stub should return 0");
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
            args: vec![Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(0)), "fence# stub should return 0");
    }

    #[test]
    fn test_intrinsic_futex_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Futex,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_futex_stub() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Futex,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0)],
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(-1)), "futex# stub should return -1");
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
            args: vec![Expr::Decimal(42), Expr::Decimal(0), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_shm_unlink_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::ShmUnlink,
            args: vec![Expr::Decimal(42)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_sem_open_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SemOpen,
            args: vec![Expr::Quoted("/test".into()), Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(false)],
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
            args: vec![Expr::Quoted("sem".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase F: Signals intrinsic tests ───────────────────────────

    #[test]
    fn test_intrinsic_sigaction_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SigAction,
            args: vec![Expr::Bool(false), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_sigprocmask_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SigProcMask,
            args: vec![Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_kill_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Kill,
            args: vec![Expr::Decimal(0), Expr::Bool(false)],
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
            args: vec![Expr::Bool(false), Expr::Decimal(0), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_bind_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Bind,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_listen_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Listen,
            args: vec![Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_accept_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Accept,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_connect_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Connect,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_send_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Send,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_recv_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Recv,
            args: vec![Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_sendto_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SendTo,
            args: vec![
                Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0),
                Expr::Bool(false), Expr::Decimal(0), Expr::Decimal(0),
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
                Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0),
                Expr::Bool(false), Expr::Decimal(0), Expr::Decimal(0),
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
                Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0),
                Expr::Bool(false), Expr::Decimal(0),
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
                Expr::Decimal(0), Expr::Decimal(0), Expr::Decimal(0),
                Expr::Bool(false), Expr::Decimal(0),
            ],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_shutdown_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::Shutdown,
            args: vec![Expr::Decimal(0), Expr::Bool(false)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_getaddrinfo_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetAddrInfo,
            args: vec![Expr::Decimal(0), Expr::Quoted("http".into())],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    // ── Phase H: Everything Else intrinsic tests ──────────────────

    #[test]
    fn test_intrinsic_getenv_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::GetEnv,
            args: vec![Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_setenv_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::SetEnv,
            args: vec![Expr::Quoted("PATH".into()), Expr::Decimal(0)],
        };
        assert!(i.eval_expr(&expr).is_err());
    }

    #[test]
    fn test_intrinsic_unsetenv_type_error() {
        let mut i = Interpreter::new();
        let expr = Expr::IntrinsicCall {
            intrinsic: Intrinsic::UnsetEnv,
            args: vec![Expr::Decimal(0)],
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
            Box::new(Expr::Decimal(42)),
            crate::ast::IsTarget::Type(Type::int()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "42 is Int should be true");
    }

    #[test]
    fn test_eval_is_type_string() {
        let mut i = Interpreter::new();
        let expr = Expr::IsType(
            Box::new(Expr::Quoted("hello".into())),
            crate::ast::IsTarget::Type(Type::int()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]), "string is Int should be false");
    }

    #[test]
    fn test_eval_is_variant() {
        let mut i = Interpreter::new();
        let mut fields = std::collections::HashMap::new();
        fields.insert("value".to_string(), Value::Bits(i64_to_bits(42)));
        i.state.insert("x".to_string(), Value::Enum("Option".to_string(), "Some".to_string(), fields));
        let expr = Expr::IsType(
            Box::new(Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())))),
            crate::ast::IsTarget::Variant("Some".to_string()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "Option::Some is Some should be true");
    }

    #[test]
    fn test_eval_is_variant_mismatch() {
        let mut i = Interpreter::new();
        i.state.insert("x".to_string(), Value::Enum("Option".to_string(), "None".to_string(), std::collections::HashMap::new()));
        let expr = Expr::IsType(
            Box::new(Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())))),
            crate::ast::IsTarget::Variant("Some".to_string()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]), "Option::None is Some should be false");
    }

    #[test]
    fn test_eval_from_check() {
        let mut i = Interpreter::new();
        let mut fields = std::collections::HashMap::new();
        fields.insert("x".to_string(), Value::Bits(i64_to_bits(1)));
        i.state.insert("obj".to_string(), Value::Instance { typename: "Foo".to_string(), fields });
        let expr = Expr::FromCheck(
            Box::new(Expr::AddrOf(Box::new(Expr::Identifier("obj".to_string())))),
            Type::Custom("Foo".to_string()),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "obj from Foo should be true");
    }

    #[test]
    fn test_eval_like_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::Decimal(42)),
            Box::new(Expr::Decimal(42)),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "42 like 42 should be true");
    }

    #[test]
    fn test_eval_like_int_mismatch() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::Decimal(42)),
            Box::new(Expr::Decimal(1)),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]), "42 like 1 should be false");
    }

    #[test]
    fn test_eval_like_float() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::Float(3.14)),
            Box::new(Expr::Float(3.14)),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "3.14 like 3.14 should be true");
    }

    #[test]
    fn test_eval_like_string() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::Quoted("hello".into())),
            Box::new(Expr::Quoted("hello".into())),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "\"hello\" like \"hello\" should be true");
    }

    #[test]
    fn test_eval_like_string_mismatch() {
        let mut i = Interpreter::new();
        let expr = Expr::Like(
            Box::new(Expr::Quoted("hello".into())),
            Box::new(Expr::Quoted("world".into())),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]), "\"hello\" like \"world\" should be false");
    }

    #[test]
    fn test_eval_like_list() {
        let mut i = Interpreter::new();
        let a = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]);
        let b = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]);
        i.state.insert("a".to_string(), a);
        i.state.insert("b".to_string(), b);
        let expr = Expr::Like(
            Box::new(Expr::AddrOf(Box::new(Expr::Identifier("a".to_string())))),
            Box::new(Expr::AddrOf(Box::new(Expr::Identifier("b".to_string())))),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "[1,2] like [1,2] should be true");
    }

    #[test]
    fn test_eval_like_list_mismatch() {
        let mut i = Interpreter::new();
        let a = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(2))]);
        let b = Value::List(vec![Value::Bits(i64_to_bits(1)), Value::Bits(i64_to_bits(3))]);
        i.state.insert("a".to_string(), a);
        i.state.insert("b".to_string(), b);
        let expr = Expr::Like(
            Box::new(Expr::AddrOf(Box::new(Expr::Identifier("a".to_string())))),
            Box::new(Expr::AddrOf(Box::new(Expr::Identifier("b".to_string())))),
        );
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]), "[1,2] like [1,3] should be false");
    }

    #[test]
    fn test_eval_cast_int_to_string() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Decimal(42)), Type::string());
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits("42".to_string().into_bytes()), "Int -> String should format as decimal");
    }

    #[test]
    fn test_eval_cast_string_to_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Quoted("42".into())), Type::int());
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(42)), "String -> Int should parse decimal");
    }

    #[test]
    fn test_eval_cast_char_to_string() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Char('A')), Type::string());
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits("A".to_string().into_bytes()), "Char -> String should be single-char");
    }

    #[test]
    fn test_eval_cast_string_to_char() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Quoted("hello".into())), Type::char_());
        let result = i.eval_expr(&expr).unwrap();
        // Char encoding is u32 (4 bytes), not i64 (8 bytes)
        let expected = Value::Bits(('h' as u32).to_le_bytes().to_vec());
        assert_eq!(result, expected, "String -> Char should take first char");
    }

    #[test]
    fn test_eval_cast_int_to_float() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Decimal(42)), Type::float());
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(f64_to_bits(42.0)), "Int -> Float should be exact");
    }

    #[test]
    fn test_eval_cast_float_to_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Float(3.14)), Type::int());
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(3)), "Float -> Int should truncate");
    }

    #[test]
    fn test_eval_cast_int_to_char() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Decimal(65)), Type::char_());
        let result = i.eval_expr(&expr).unwrap();
        // Char encoding is u32 (4 bytes)
        assert_eq!(result, Value::Bits(('A' as u32).to_le_bytes().to_vec()), "Int 65 -> Char should be 'A'");
    }

    #[test]
    fn test_eval_cast_char_to_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Char('A')), Type::int());
        let result = i.eval_expr(&expr).unwrap();
        // Char encoding is u32 (4 bytes), Int is i64 (8 bytes) — extract u32 code
        assert_eq!(result, Value::Bits(i64_to_bits(65)), "Char 'A' -> Int should be 65");
    }

    #[test]
    fn test_eval_cast_bool_to_int() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Bool(true)), Type::int());
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(1)), "Bool true -> Int should be 1");
    }

    #[test]
    fn test_eval_cast_int_to_bool() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Decimal(42)), Type::bool_());
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]), "Int 42 -> Bool should be true");
    }

    #[test]
    fn test_eval_cast_int_zero_to_bool() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::Decimal(0)), Type::bool_());
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]), "Int 0 -> Bool should be false");
    }

    #[test]
    fn test_eval_cast_unsupported() {
        let mut i = Interpreter::new();
        let expr = Expr::Cast(Box::new(Expr::List(vec![])), Type::int());
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "List -> Int should be an error");
    }

    // --- Pipe frgn tests ---

    #[test]
    fn test_is_valid_ffi_return_string_valid() {
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits("hello".to_string().into_bytes()), &Type::string()));
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits("".to_string().into_bytes()), &Type::string()));
    }

    #[test]
    fn test_is_valid_ffi_return_int_valid() {
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits(i64_to_bits(42)), &Type::int()));
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits(i64_to_bits(0)), &Type::int()));
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits(i64_to_bits(-1)), &Type::int()));
    }

    #[test]
    fn test_is_valid_ffi_return_float_valid() {
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits(f64_to_bits(3.14)), &Type::float()));
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits(f64_to_bits(0.0)), &Type::float()));
    }

    #[test]
    fn test_is_valid_ffi_return_float_nan_invalid() {
        assert!(!Interpreter::is_valid_ffi_return(&Value::Bits(f64_to_bits(f64::NAN)), &Type::float()));
        assert!(!Interpreter::is_valid_ffi_return(&Value::Bits(f64_to_bits(f64::INFINITY)), &Type::float()));
        assert!(!Interpreter::is_valid_ffi_return(&Value::Bits(f64_to_bits(f64::NEG_INFINITY)), &Type::float()));
    }

    #[test]
    fn test_is_valid_ffi_return_bool_valid() {
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits(vec![if true { 1u8 } else { 0u8 }]), &Type::bool_()));
        assert!(Interpreter::is_valid_ffi_return(&Value::Bits(vec![0u8]), &Type::bool_()));
    }

    #[test]
    fn test_is_valid_ffi_return_type_mismatch() {
        // An Int value is not valid when String is expected
        assert!(!Interpreter::is_valid_ffi_return(&Value::Bits(i64_to_bits(42)), &Type::string()));
        // A float is not valid when Int is expected
        assert!(!Interpreter::is_valid_ffi_return(&Value::Bits(f64_to_bits(1.0)), &Type::int()));
    }

    #[test]
    fn test_call_pipe_frgn_ok_wraps_result() {
        let mut i = Interpreter::new();
        let sig = ForeignSignature {
            name: "test_pipe_ok".into(), location: "test".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::int())],
            result_type: ResultType::Projection(vec![Type::int()]),
            error_type_name: "".into(), error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true, fallback: Some(Expr::Decimal(0)),
            default_watchdog: None,
            span: None,
        };
        i.ffi_bindings.insert("test_pipe_ok".into(), sig);

        // Valid return: Int is always valid -> Ok(42)
        let result = i.call_pipe_frgn("test_pipe_ok", Value::Bits(i64_to_bits(42))).unwrap();
        match result {
            Value::Enum(e, v, fields) if e == "Result" && v == "Ok" => {
                assert_eq!(fields.get("value"), Some(&Value::Bits(i64_to_bits(42))));
            }
            other => panic!("Expected Ok(42), got {:?}", other),
        }
    }

    #[test]
    fn test_call_pipe_frgn_err_uses_fallback() {
        let mut i = Interpreter::new();
        // Set up a pipe frgn with float type and fallback to 0.0
        let sig = ForeignSignature {
            name: "test_pipe_err".into(), location: "test".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::float())],
            result_type: ResultType::Projection(vec![Type::float()]),
            error_type_name: "".into(), error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true, fallback: Some(Expr::Float(0.0)),
            default_watchdog: None,
            span: None,
        };
        i.ffi_bindings.insert("test_pipe_err".into(), sig);

        // NaN is invalid for Float -> Err(0.0)
        let result = i.call_pipe_frgn("test_pipe_err", Value::Bits(f64_to_bits(f64::NAN))).unwrap();
        match result {
            Value::Enum(e, v, fields) if e == "Result" && v == "Err" => {
                assert_eq!(fields.get("value"), Some(&Value::Bits(f64_to_bits(0.0))));
            }
            other => panic!("Expected Err(0.0), got {:?}", other),
        }
    }

    #[test]
    fn test_call_pipe_frgn_valid_float_ok() {
        let mut i = Interpreter::new();
        let sig = ForeignSignature {
            name: "test_pipe_f".into(), location: "test".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::float())],
            result_type: ResultType::Projection(vec![Type::float()]),
            error_type_name: "".into(), error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true, fallback: Some(Expr::Float(0.0)),
            default_watchdog: None,
            span: None,
        };
        i.ffi_bindings.insert("test_pipe_f".into(), sig);

        // Finite float is valid -> Ok(3.14)
        let result = i.call_pipe_frgn("test_pipe_f", Value::Bits(f64_to_bits(3.14))).unwrap();
        match result {
            Value::Enum(e, v, fields) if e == "Result" && v == "Ok" => {
                match fields.get("value") {
                    Some(Value::Bits(f64_to_bits(f))) => assert!((f - 3.14).abs() < 1e-10),
                    other => panic!("Expected Float(3.14), got {:?}", other),
                }
            }
            other => panic!("Expected Ok(3.14), got {:?}", other),
        }
    }

    #[test]
    fn test_call_pipe_frgn_string_null_fallback() {
        let mut i = Interpreter::new();
        let sig = ForeignSignature {
            name: "test_pipe_null_str".into(), location: "test".into(),
            wasm_impl: None, wasm_setup: None,
            inputs: vec![],
            success_output: vec![("result".into(), Type::string())],
            result_type: ResultType::Projection(vec![Type::string()]),
            error_type_name: "".into(), error_fields: vec![],
            input_layout: None, output_layout: None,
            precondition: None, postcondition: None,
            buffer_mode: None, ffi_kind: None, is_out: false,
            is_pipe: true, fallback: Some(Expr::Quoted("default".into())),
            default_watchdog: None,
            span: None,
        };
        i.ffi_bindings.insert("test_pipe_null_str".into(), sig);

        // Valid string returns Ok with the string
        let result = i.call_pipe_frgn("test_pipe_null_str", Value::Bits("hello".to_string().into_bytes())).unwrap();
        match result {
            Value::Enum(e, v, fields) if e == "Result" && v == "Ok" => {
                assert_eq!(fields.get("value"), Some(&Value::Bits("hello".to_string().into_bytes())));
            }
            other => panic!("Expected Ok(\"hello\"), got {:?}", other),
        }
    }

    #[test]
    fn test_interp_await() {
        let mut i = Interpreter::new();
        let stmt = Statement::Await {
            expr: Expr::Decimal(42),
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.return_value, Some(Value::Bits(i64_to_bits(42))));
    }

    #[test]
    fn test_interp_async() {
        let mut i = Interpreter::new();
        let inner = Statement::Expression(Expr::Decimal(42));
        let stmt = Statement::Async {
            body: Box::new(inner),
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        // Async is fire-and-forget — return_value should not be set
        assert!(i.return_value.is_none());
    }

    #[test]
    fn test_interp_async_await() {
        let mut i = Interpreter::new();
        let inner = Statement::Expression(Expr::Decimal(99));
        let stmt = Statement::AsyncAwait {
            body: Box::new(inner),
            lhs: Some("result".to_string()),
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("result"), Some(&Value::Bits(i64_to_bits(99))));
    }

    // --- Constraint evaluation tests (Phase B) ---

    #[test]
    fn test_constraint_passes() {
        let mut i = Interpreter::new();
        let val = Value::Bits(i64_to_bits(50));
        // Constraint: _ >= 0 && _ <= 100 (desugared from 0..100)
        let constraint = Expr::And(
            Box::new(Expr::Ge(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(0)),
            )),
            Box::new(Expr::Le(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(100)),
            )),
        );
        assert!(i.eval_constraint(&val, &constraint).is_ok());
    }

    #[test]
    fn test_constraint_violated_low() {
        let mut i = Interpreter::new();
        let val = Value::Bits(i64_to_bits(-1));
        let constraint = Expr::And(
            Box::new(Expr::Ge(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(0)),
            )),
            Box::new(Expr::Le(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(100)),
            )),
        );
        assert!(i.eval_constraint(&val, &constraint).is_err());
    }

    #[test]
    fn test_constraint_violated_high() {
        let mut i = Interpreter::new();
        let val = Value::Bits(i64_to_bits(200));
        let constraint = Expr::And(
            Box::new(Expr::Ge(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(0)),
            )),
            Box::new(Expr::Le(
                Box::new(Expr::Identifier("_".to_string())),
                Box::new(Expr::Decimal(100)),
            )),
        );
        assert!(i.eval_constraint(&val, &constraint).is_err());
    }

    #[test]
    fn test_constraint_let_statement_passes() {
        let mut i = Interpreter::new();
        // let x: Int <: [0..100] = 50;
        let stmt = Statement::Let {
            name: "x".to_string(),
            ty: Some(Type::int()),
            expr: Some(Expr::Decimal(50)),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: Some(Box::new(Expr::And(
                Box::new(Expr::Ge(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(0)),
                )),
                Box::new(Expr::Le(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(100)),
                )),
            ))),
            is_override: false,
            modifiers: vec![],
        };
        i.exec_stmt(&stmt).unwrap();
        assert_eq!(i.state.get("x"), Some(&Value::Bits(i64_to_bits(50))));
    }

    #[test]
    fn test_constraint_let_statement_violated() {
        let mut i = Interpreter::new();
        // let x: Int <: [0..100] = 200; — should fail
        let stmt = Statement::Let {
            name: "x".to_string(),
            ty: Some(Type::int()),
            expr: Some(Expr::Decimal(200)),
            address: None,
            address_expr: None,
            bit_range: None,
            constraint: Some(Box::new(Expr::And(
                Box::new(Expr::Ge(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(0)),
                )),
                Box::new(Expr::Le(
                    Box::new(Expr::Identifier("_".to_string())),
                    Box::new(Expr::Decimal(100)),
                )),
            ))),
            is_override: false,
            modifiers: vec![],
        };
        let result = i.exec_stmt(&stmt);
        assert!(result.is_err(), "Expected constraint violation error, got ok");
    }

    #[test]
    fn test_constraint_passes_with_prior_underscore() {
        let mut i = Interpreter::new();
        // Bind _ first, then constraint should shadow it temporarily
        i.state.insert("_".to_string(), Value::Bits(i64_to_bits(999)));
        let val = Value::Bits(i64_to_bits(50));
        let constraint = Expr::Ge(
            Box::new(Expr::Identifier("_".to_string())),
            Box::new(Expr::Decimal(0)),
        );
        assert!(i.eval_constraint(&val, &constraint).is_ok());
        // After eval_constraint, _ should be restored
        assert_eq!(i.state.get("_"), Some(&Value::Bits(i64_to_bits(999))));
    }

    // ── Pipe Chain E2E Tests ─────────────────────────────────────────

    #[test]
    fn test_pipe_chain_e2e_basic() {
        // Test desugared pipe chain through interpreter.
        // Pipeline: 5 |> add_one() |> double()  →  double(add_one(5)) = 12
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Decimal(5)),
            steps: vec![
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("add_one".to_string(), vec![])),
                    skip: 0,
                },
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("double".to_string(), vec![])),
                    skip: 0,
                },
            ],
        };

        // Desugar the pipe chain
        let mut desugarer = crate::desugarer::Desugarer::new();
        let desugared = desugarer.desugar_expr(Expr::PipeChain(pipe));

        // Evaluate in interpreter (must register function defs)
        let mut interp = Interpreter::new();
        interp.definitions.insert("add_one".to_string(), Definition {
            name: "add_one".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::int())],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Add(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(1)),
                ))],
                modifiers: vec![],
                swan_song: None,
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });
        interp.definitions.insert("double".to_string(), Definition {
            name: "double".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::int())],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Mul(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(2)),
                ))],
                modifiers: vec![],
                swan_song: None,
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });

        let result = interp.eval_expr(&desugared).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(12)), "5 |> add_one() |> double() should be 12");
    }

    #[test]
    fn test_pipe_chain_e2e_dot_skip() {
        // Pipeline: 10 |> add_one() .|> double()
        // add_one(10) = 11 (pos 1)
        // .|> skips pos 1, reads initial 10: double(10) = 20
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Decimal(10)),
            steps: vec![
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("add_one".to_string(), vec![])),
                    skip: 0,
                },
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("double".to_string(), vec![])),
                    skip: 1,
                },
            ],
        };

        let mut desugarer = crate::desugarer::Desugarer::new();
        let desugared = desugarer.desugar_expr(Expr::PipeChain(pipe));

        let mut interp = Interpreter::new();
        interp.definitions.insert("add_one".to_string(), Definition {
            name: "add_one".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::int())],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Add(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(1)),
                ))],
                modifiers: vec![],
                swan_song: None,
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });
        interp.definitions.insert("double".to_string(), Definition {
            name: "double".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::int())],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Mul(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(2)),
                ))],
                modifiers: vec![],
                swan_song: None,
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });

        let result = interp.eval_expr(&desugared).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(20)), "10 |> add_one() .|> double() should be 20");
    }

    #[test]
    fn test_pipe_chain_e2e_with_args() {
        // Pipeline: 7 |> sum(3) — sum(7, 3) = 10
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Decimal(7)),
            steps: vec![
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("sum".to_string(), vec![Expr::Decimal(3)])),
                    skip: 0,
                },
            ],
        };

        let mut desugarer = crate::desugarer::Desugarer::new();
        let desugared = desugarer.desugar_expr(Expr::PipeChain(pipe));

        let mut interp = Interpreter::new();
        interp.definitions.insert("sum".to_string(), Definition {
            name: "sum".to_string(),
            type_params: vec![],
            parameters: vec![("a".to_string(), Type::int()), ("b".to_string(), Type::int())],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Add(
                    Box::new(Expr::Identifier("a".to_string())),
                    Box::new(Expr::Identifier("b".to_string())),
                ))],
                modifiers: vec![],
                swan_song: None,
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });

        let result = interp.eval_expr(&desugared).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(10)), "7 |> sum(3) should be sum(7, 3) = 10");
    }

    #[test]
    fn test_pipe_chain_e2e_three_step() {
        // Pipeline: 2 |> square() |> add_one() .|> double()
        // square(2) = 4 (pos 1)
        // add_one(4) = 5 (pos 2)
        // .|> reads pos 2-1 = pos 1 = 4: double(4) = 8
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Decimal(2)),
            steps: vec![
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("square".to_string(), vec![])), skip: 0,
                },
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("add_one".to_string(), vec![])), skip: 0,
                },
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("double".to_string(), vec![])), skip: 1,
                },
            ],
        };

        let mut desugarer = crate::desugarer::Desugarer::new();
        let desugared = desugarer.desugar_expr(Expr::PipeChain(pipe));

        let mut interp = Interpreter::new();
        for (name, params, body_expr) in vec![
            ("square", vec![("x", Type::int())], Expr::Mul(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Identifier("x".to_string())),
            )),
            ("add_one", vec![("x", Type::int())], Expr::Add(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Decimal(1)),
            )),
            ("double", vec![("x", Type::int())], Expr::Mul(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Decimal(2)),
            )),
        ] {
            interp.definitions.insert(name.to_string(), Definition {
                name: name.to_string(),
                type_params: vec![],
                parameters: params.into_iter()
                    .map(|(n, t)| (n.to_string(), t))
                    .collect(),
                outputs: vec![],
                output_type: None,
                output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![Statement::Term {
                    values: vec![Some(body_expr)],
                    modifiers: vec![],
                    swan_song: None,
                }],
                is_lambda: false,
                modifiers: vec![],
                variant_bodies: vec![],
            });
        }

        let result = interp.eval_expr(&desugared).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(8)), "2 |> square() |> add_one() .|> double() should be 8");
    }

    #[test]
    fn test_pipe_chain_e2e_auto_wrap() {
        // Pipeline: 5 |> add_one — bare identifier auto-wrapped
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Decimal(5)),
            steps: vec![
                crate::ast::PipeStep {
                    target: Box::new(Expr::Identifier("add_one".to_string())), skip: 0,
                },
            ],
        };

        let mut desugarer = crate::desugarer::Desugarer::new();
        let desugared = desugarer.desugar_expr(Expr::PipeChain(pipe));

        let mut interp = Interpreter::new();
        interp.definitions.insert("add_one".to_string(), Definition {
            name: "add_one".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::int())],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Add(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(1)),
                ))],
                modifiers: vec![],
                swan_song: None,
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });

        let result = interp.eval_expr(&desugared).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(6)), "5 |> add_one should be 6");
    }

    #[test]
    fn test_pipe_chain_e2e_function_start() {
        // Pipeline: f() |> g() — starts with a function call
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Call("forty_two".to_string(), vec![])),
            steps: vec![
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("add_one".to_string(), vec![])), skip: 0,
                },
            ],
        };

        let mut desugarer = crate::desugarer::Desugarer::new();
        let desugared = desugarer.desugar_expr(Expr::PipeChain(pipe));

        let mut interp = Interpreter::new();
        interp.definitions.insert("forty_two".to_string(), Definition {
            name: "forty_two".to_string(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Decimal(42))],
                modifiers: vec![],
                swan_song: None,
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });
        interp.definitions.insert("add_one".to_string(), Definition {
            name: "add_one".to_string(),
            type_params: vec![],
            parameters: vec![("x".to_string(), Type::int())],
            outputs: vec![],
            output_type: None,
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Add(
                    Box::new(Expr::Identifier("x".to_string())),
                    Box::new(Expr::Decimal(1)),
                ))],
                modifiers: vec![],
                swan_song: None,
            }],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });

        let result = interp.eval_expr(&desugared).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(43)), "f() |> add_one() should be 43");
    }

    // ── .N|> E2E Tests ───────────────────────────────────────────────

    #[test]
    fn test_pipe_dot_2_e2e() {
        // 3 |> square() |> add_one() .2|> double()
        //   = double(initial) = double(3) = 6
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Decimal(3)),
            steps: vec![
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("square".to_string(), vec![])), skip: 0,
                },
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("add_one".to_string(), vec![])), skip: 0,
                },
                crate::ast::PipeStep {
                    target: Box::new(Expr::Call("double".to_string(), vec![])), skip: 2,
                },
            ],
        };

        let mut desugarer = crate::desugarer::Desugarer::new();
        let desugared = desugarer.desugar_expr(Expr::PipeChain(pipe));

        let mut interp = Interpreter::new();
        for (name, params, body_expr) in vec![
            ("square", vec![("x", Type::int())], Expr::Mul(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Identifier("x".to_string())),
            )),
            ("add_one", vec![("x", Type::int())], Expr::Add(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Decimal(1)),
            )),
            ("double", vec![("x", Type::int())], Expr::Mul(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Decimal(2)),
            )),
        ] {
            interp.definitions.insert(name.to_string(), Definition {
                name: name.to_string(),
                type_params: vec![],
                parameters: params.into_iter()
                    .map(|(n, t)| (n.to_string(), t))
                    .collect(),
                outputs: vec![],
                output_type: None,
                output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![Statement::Term {
                    values: vec![Some(body_expr)],
                    modifiers: vec![],
                    swan_song: None,
                }],
                is_lambda: false,
                modifiers: vec![],
                variant_bodies: vec![],
            });
        }

        let result = interp.eval_expr(&desugared).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(6)), "3 |> square() |> add_one() .2|> double() should be 6");
    }

    #[test]
    fn test_pipe_dot_3_e2e() {
        // 3 |> square() |> add_one() .3|> double() reads __pipe_{4-1-3}=__pipe_0 = 3
        // double(3) = 6
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Decimal(3)),
            steps: vec![
                crate::ast::PipeStep { target: Box::new(Expr::Call("square".to_string(), vec![])), skip: 0 },
                crate::ast::PipeStep { target: Box::new(Expr::Call("add_one".to_string(), vec![])), skip: 0 },
                crate::ast::PipeStep { target: Box::new(Expr::Call("double".to_string(), vec![])), skip: 3 },
            ],
        };

        let mut desugarer = crate::desugarer::Desugarer::new();
        let desugared = desugarer.desugar_expr(Expr::PipeChain(pipe));

        let mut interp = Interpreter::new();
        for (name, params, body_expr) in vec![
            ("square", vec![("x", Type::int())], Expr::Mul(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Identifier("x".to_string())),
            )),
            ("add_one", vec![("x", Type::int())], Expr::Add(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Decimal(1)),
            )),
            ("double", vec![("x", Type::int())], Expr::Mul(
                Box::new(Expr::Identifier("x".to_string())),
                Box::new(Expr::Decimal(2)),
            )),
        ] {
            interp.definitions.insert(name.to_string(), Definition {
                name: name.to_string(),
                type_params: vec![],
                parameters: params.into_iter()
                    .map(|(n, t)| (n.to_string(), t))
                    .collect(),
                outputs: vec![],
                output_type: None,
                output_names: vec![],
                contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                body: vec![Statement::Term {
                    values: vec![Some(body_expr)],
                    modifiers: vec![],
                    swan_song: None,
                }],
                is_lambda: false,
                modifiers: vec![],
                variant_bodies: vec![],
            });
        }

        let result = interp.eval_expr(&desugared).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(6)), "3 |> square() |> add_one() .3|> double() should be 6");
    }

    #[test]
    #[should_panic(expected = "exceeds pipeline position")]
    fn test_pipe_skip_overflow_panics() {
        // 3 |> square() .2|> double() has skip=2 but only 1 step before it
        let pipe = crate::ast::PipeChain {
            initial: Box::new(Expr::Decimal(3)),
            steps: vec![
                crate::ast::PipeStep { target: Box::new(Expr::Call("square".to_string(), vec![])), skip: 0 },
                crate::ast::PipeStep { target: Box::new(Expr::Call("double".to_string(), vec![])), skip: 2 },
            ],
        };

        let mut desugarer = crate::desugarer::Desugarer::new();
        // This should panic because skip=2 but only 1 step precedes the .2|>
        let _ = desugarer.desugar_expr(Expr::PipeChain(pipe));
    }

    #[test]
    fn test_fn_projection_name() {
        let mut i = Interpreter::new();
        i.definitions.insert("add".to_string(), Definition {
            name: "add".into(),
            type_params: vec![],
            parameters: vec![("x".into(), Type::int()), ("y".into(), Type::int())],
            outputs: vec![Type::int()],
            output_type: Some(OutputType::Single(Box::new(Type::int()))),
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("add".into())),
            target: ProjectionTarget::Name,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits("add".to_string().into_bytes()));
    }

    #[test]
    fn test_fn_projection_arity() {
        let mut i = Interpreter::new();
        i.definitions.insert("add".to_string(), Definition {
            name: "add".into(),
            type_params: vec![],
            parameters: vec![("x".into(), Type::int()), ("y".into(), Type::int())],
            outputs: vec![Type::int()],
            output_type: Some(OutputType::Single(Box::new(Type::int()))),
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("add".into())),
            target: ProjectionTarget::Arity,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(i64_to_bits(2)));
    }

    #[test]
    fn test_fn_projection_is_pure() {
        let mut i = Interpreter::new();
        i.definitions.insert("pure_fn".to_string(), Definition {
            name: "pure_fn".into(),
            type_params: vec![],
            parameters: vec![],
            outputs: vec![Type::int()],
            output_type: Some(OutputType::Single(Box::new(Type::int()))),
            output_names: vec![],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            body: vec![],
            is_lambda: false,
            modifiers: vec![],
            variant_bodies: vec![],
        });
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("pure_fn".into())),
            target: ProjectionTarget::IsPure,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![1u8]));
    }

    #[test]
    fn test_fn_projection_inop_not_pure() {
        let mut i = Interpreter::new();
        i.inop_decls.insert("write_buf".to_string(), InopDeclaration {
            name: "write_buf".into(),
            type_params: vec![],
            params: vec![("buf".into(), Type::int())],
            outputs: vec![Type::int()],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            llvm_body: vec![],
            llvm_body_spans: vec![],
            fallback: None,
            has_side_effects: true,
            has_state_access: false,
            section: None,
            span: None,
        });
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("write_buf".into())),
            target: ProjectionTarget::IsPure,
        };
        let result = i.eval_expr(&expr).unwrap();
        assert_eq!(result, Value::Bits(vec![0u8]));
    }

    #[test]
    fn test_fn_projection_unknown_name_errors() {
        let mut i = Interpreter::new();
        let expr = Expr::Projection {
            source: Box::new(Expr::Identifier("undefined_fn".into())),
            target: ProjectionTarget::Address,
        };
        let result = i.eval_expr(&expr);
        assert!(result.is_err(), "Undefined function should error on Fn projection");
    }
}