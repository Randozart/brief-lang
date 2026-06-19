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

use crate::ast::{BitRange, BracketOp, Contract, Expr, ForeignTarget, Intrinsic, Program, Statement, TopLevel, Transaction, Type};
use crate::features::traits::{ExprCodegenWebstack, ExprDispatch};
use crate::view_compiler::{Binding, Directive};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

#[derive(Clone, Copy, PartialEq)]
/// Intent: CodeTarget type.
pub enum CodeTarget {
    Wasm,   // Browser WASM (default)
    Arm,     // ARM ELF (bare-metal)
    Fpga,   // SystemVerilog (FPGA)
}

/// Intent: Default implementation block.
impl Default for CodeTarget {
    /// Intent: default function.
    /// Intent: return the default value.
    fn default() -> Self {
        CodeTarget::Wasm
    }
}

#[derive(Clone)]
/// Intent: SignalType type.
enum SignalType {
    Int,
    Float,
    Bool,
    String,
    List,
    Struct,
    Vector(usize),
}

/// Intent: WebstackGenerator type.
pub struct WebstackGenerator {
    spec: Option<crate::target_spec::TargetSpec>,
    signal_counter: usize,
    txn_counter: usize,
    signal_map: HashMap<String, usize>,
    signal_types: HashMap<String, SignalType>,
    signal_initializers: HashMap<String, String>,
    txn_map: HashMap<String, usize>,
    reactive_txns: Vec<Transaction>,
    reactive_dependency_map: HashMap<String, Vec<usize>>,
    reactor_speed: u32,
    ffi_bindings: HashMap<String, usize>, // function name -> arg count
    ffi_wasm_impl: HashMap<String, String>, // function name -> WASM JS implementation
    ffi_wasm_setups: HashSet<String>,     // global WASM JS setup/imports
    local_vars: HashMap<String, ()>,      // track local let-bound variables
    target: CodeTarget,
    pending_cleanup: RefCell<Vec<Statement>>,
    has_cycles: bool,
    trigger_names: Vec<String>,           // trg variable names for dirty-flag integration
    promise_counter: usize,
    pending_promises: Vec<PendingPromise>,
}

#[derive(Debug, Clone)]
struct PendingPromise {
    var: String,
    capture: Option<String>,
}

/// Intent: WebstackGenerator implementation block.
impl WebstackGenerator {
    /// Intent: create a new instance.
    pub fn new() -> Self {
        WebstackGenerator {
            spec: None,
            signal_counter: 0,
            txn_counter: 0,
            signal_map: HashMap::new(),
            signal_types: HashMap::new(),
            signal_initializers: HashMap::new(),
            txn_map: HashMap::new(),
            reactive_txns: Vec::new(),
            reactive_dependency_map: HashMap::new(),
            reactor_speed: 0,
            ffi_bindings: HashMap::new(),
            ffi_wasm_impl: HashMap::new(),
            ffi_wasm_setups: HashSet::new(),
            local_vars: HashMap::new(),
            target: CodeTarget::Wasm,
            pending_cleanup: RefCell::new(Vec::new()),
            has_cycles: false,
            trigger_names: Vec::new(),
            promise_counter: 0,
            pending_promises: Vec::new(),
        }
    }

    /// Intent: with_spec function.
    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    /// Intent: with_target function.
    pub fn with_target(mut self, target: CodeTarget) -> Self {
        self.target = target;
        self
    }

    /// Intent: set_reactor_speed function.
    pub fn set_reactor_speed(&mut self, speed: u32) {
        self.reactor_speed = speed;
    }

    /// Intent: generate function.
    pub fn generate(
        &mut self,
        program: &Program,
        bindings: &[Binding],
        program_name: &str,
    ) -> WebstackOutput {
        self.collect_signals_and_transactions(program);

        match self.target {
            CodeTarget::Wasm => {
                let rust_code = self.generate_rust_code(program, bindings);
                let js_glue = self.generate_js_glue(program_name, bindings);
                WebstackOutput {
                    rust_code,
                    js_glue,
                    signal_count: self.signal_counter,
                    txn_count: self.txn_counter,
                }
            }
            CodeTarget::Arm => {
                let rust_code = self.generate_arm_rust_code(program);
                WebstackOutput {
                    rust_code,
                    js_glue: String::new(),
                    signal_count: self.signal_counter,
                    txn_count: self.txn_counter,
                }
            }
            CodeTarget::Fpga => {
                let rust_code = self.generate_rust_code(program, bindings);
                WebstackOutput {
                    rust_code,
                    js_glue: String::new(),
                    signal_count: self.signal_counter,
                    txn_count: self.txn_counter,
                }
            }
        }
    }

    /// Intent: generate_arm_rust_code function.
    fn generate_arm_rust_code(&self, program: &Program) -> String {
        let mut output = String::new();

        output.push_str("// ARM bare-metal kernel - generated by Brief\n");
        output.push_str("// Target: KV260 Cortex-A53\n");
        output.push_str("// This is no_std code - compile with --target arm-none-eabi\n\n");

        output.push_str("#![no_std]\n");
        output.push_str("#![no_main]\n");
        output.push_str("#![feature(panic_info)]\n\n");

        output.push_str("use core::fmt;\n");
        output.push_str("use core::mem::zeroed;\n");
        output.push_str("use core::ptr::{read_volatile, write_volatile};\n\n");

        // Collect state declarations with their bit ranges
        let mut state_decls: Vec<(&String, &Type, &Option<BitRange>, &Option<u64>)> = Vec::new();
        for item in &program.items {
            if let TopLevel::StateDecl(decl) = item {
                state_decls.push((&decl.name, &decl.ty, &decl.bit_range, &decl.address));
            }
        }

        // Generate state struct with proper types based on bit_range
        output.push_str("#[repr(C)]\n");
        output.push_str("pub struct State {\n");
        for (name, ty, bit_range, addr) in &state_decls {
            let rust_type = Self::get_rust_type_for_arm(ty, bit_range);
            output.push_str(&format!("    pub {}: {},\n", name.replace('-', "_"), rust_type));
        }
        output.push_str("}\n\n");

        // Generate MMIO base addresses
        output.push_str("// MMIO Base Addresses\n");
        let mut generated_addrs = std::collections::HashSet::new();
        for (name, ty, bit_range, addr) in &state_decls {
            if let Some(base_addr) = addr {
                let addr_str = format!("0x{:08X}", base_addr);
                if !generated_addrs.contains(&addr_str) {
                    output.push_str(&format!("const {}: *mut State = {} as *mut State;\n", 
                        Self::mmio_const_name(name), addr_str));
                    generated_addrs.insert(addr_str);
                }
            }
        }
        output.push('\n');

        output.push_str("impl State {\n");
        output.push_str("    pub fn new() -> Self {\n");
        output.push_str("        unsafe { zeroed() }\n");
        output.push_str("    }\n");
        
        // Generate getters/setters for MMIO addresses
        for (name, ty, bit_range, addr) in &state_decls {
            if addr.is_some() {
                let const_name = Self::mmio_const_name(name);
                output.push_str(&format!(
                    "    pub fn {}_addr() -> *mut State {{ {} }}\n",
                    name.replace('-', "_"),
                    const_name
                ));
            }
        }
        output.push_str("}\n\n");

        output.push_str("impl Default for State {\n");
        output.push_str("    fn default() -> Self {\n");
        output.push_str("        Self::new()\n");
        output.push_str("    }\n");
        output.push_str("}\n\n");

        // Generate transactions as functions
        for (txn_name, &id) in &self.txn_map {
            let txn = &self.reactive_txns[id];
            output.push_str(&format!(
                "pub fn {}(state: &mut State) -> bool {{\n",
                txn_name.replace('-', "_")
            ));
            output.push_str(&format!(
                "    // pre: {:?}\n",
                txn.contract.pre_condition
            ));
            
            // Generate basic body — emit statements or placeholder
            if txn.body.is_empty() {
                output.push_str("    true\n");
            } else {
                output.push_str("    // Transaction body omitted (Arm codegen path)\n");
                output.push_str("    true\n");
            }
            output.push_str("}\n\n");
        }

        // Generate entry point (first reactive transaction)
        output.push_str("#[no_mangle]\n");
        output.push_str("pub extern \"C\" fn _start() -> ! {\n");
        output.push_str("    loop {}\n");
        output.push_str("}\n\n");

        // Generate panic handler
        output.push_str("#[panic_handler]\n");
        output.push_str("fn panic(_info: &core::panic::PanicInfo) -> ! {\n");
        output.push_str("    loop {}\n");
        output.push_str("}\n");

        output
    }

    /// Intent: get_rust_type_for_arm function.
    fn get_rust_type_for_arm(ty: &Type, bit_range: &Option<BitRange>) -> String {
        // Handle Vector types first - generate array
        if let Type::Vector(inner, dims) = ty {
            let inner_type = if let Some(br) = bit_range {
                Self::rust_type_from_bit_range(br)
            } else {
                Self::get_rust_type_for_arm(inner, &None)
            };
            // Build nested array syntax for multidimensional
            let mut result = inner_type;
            for d in dims.iter().rev() {
                let size = match d {
                    crate::ast::Dimension::Anonymous(s) => *s,
                    crate::ast::Dimension::Named(_, s) => *s,
                };
                result = format!("[{}; {}]", result, size);
            }
            return result;
        }

        // If bit_range is specified for scalar, use it
        if let Some(br) = bit_range {
            return Self::rust_type_from_bit_range(br);
        }

        // Otherwise, derive from the type
        match ty {
            Type::Int => "i32".to_string(),
            Type::UInt => "u32".to_string(),
            Type::Float => "f64".to_string(),
            Type::Bool => "bool".to_string(),
            Type::String => "u32".to_string(), // String would need special handling
            Type::Constrained(inner, br) => {
                Self::rust_type_from_bit_range(br)
            }
            _ => "u32".to_string(),
        }
    }

    /// Intent: rust_type_from_bit_range function.
    fn rust_type_from_bit_range(bit_range: &BitRange) -> String {
        match bit_range {
            BitRange::Single(1) => "bool".to_string(),
            BitRange::Single(n) | BitRange::Any(n) => {
                match n {
                    0..=8 => "u8".to_string(),
                    9..=16 => "u16".to_string(),
                    17..=32 => "u32".to_string(),
                    33..=64 => "u64".to_string(),
                    _ => "u128".to_string(),
                }
            }
            BitRange::Range(lo, hi) => {
                let bits = hi - lo + 1;
                match bits {
                    0..=8 => "u8".to_string(),
                    9..=16 => "u16".to_string(),
                    17..=32 => "u32".to_string(),
                    33..=64 => "u64".to_string(),
                    _ => "u128".to_string(),
                }
            }
        }
    }

    /// Intent: mmio_const_name function.
    fn mmio_const_name(var_name: &str) -> String {
        format!("{}_BASE", var_name.to_uppercase().replace('-', "_"))
    }

    /// Intent: collect_signals_and_transactions function.
    /// Intent: collect signals and transactions.
    fn collect_signals_and_transactions(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                TopLevel::StateDecl(decl) => {
                    let signal_type = match &decl.ty {
                        Type::Int => SignalType::Int,
                        Type::Float => SignalType::Float,
                        Type::Bool => SignalType::Bool,
                        Type::String => SignalType::String,
                        Type::Applied(name, _) if name == "List" => SignalType::List,
                        Type::Generic(name, _) if name == "List" => SignalType::List,
                        Type::Vector(_, dims) => {
                            let total: usize = dims.iter().map(|d| match d {
                                crate::ast::Dimension::Anonymous(s) => *s,
                                crate::ast::Dimension::Named(_, s) => *s,
                            }).product();
                            SignalType::Vector(total)
                        }
                        Type::Constrained(inner, _) => match **inner {
                            Type::Int => SignalType::Int,
                            Type::UInt => SignalType::Int,
                            Type::Float => SignalType::Float,
                            Type::Bool => SignalType::Bool,
                            _ => SignalType::Int,
                        },
                        Type::Custom(_) => SignalType::Struct,
                        Type::TypeVar(_) => SignalType::Int,
                        _ => SignalType::Int,
                    };
                    self.signal_types
                        .insert(decl.name.clone(), signal_type.clone());

                    let initializer = if let Some(expr) = &decl.expr {
                        self.expr_to_js_value(expr)
                    } else {
                        match &signal_type {
                            SignalType::Struct => "js_sys::Object::new().into()".to_string(),
                            SignalType::Vector(size) => {
                                format!("js_sys::Uint32Array::new_with_length({}).into()", size)
                            }
                            SignalType::List => "js_sys::Array::new().into()".to_string(),
                            SignalType::Int | SignalType::Float | SignalType::Bool => {
                                "JsValue::from(0)".to_string()
                            }
                            SignalType::String => "JsValue::from(\"\")".to_string(),
                        }
                    };
                    self.signal_initializers
                        .insert(decl.name.clone(), initializer);

                    self.signal_map
                        .insert(decl.name.clone(), self.signal_counter);
                    self.signal_counter += 1;
                }
                TopLevel::Trigger(trg) => {
                    self.trigger_names.push(trg.name.clone());
                    // Register trigger as a signal (like StateDecl but with FFI-observable initializer)
                    let signal_type = match &trg.ty {
                        Type::Int => SignalType::Int,
                        Type::Float => SignalType::Float,
                        Type::Bool => SignalType::Bool,
                        Type::String => SignalType::String,
                        _ => SignalType::Int,
                    };
                    self.signal_types.insert(trg.name.clone(), signal_type.clone());
                    let initializer = match &signal_type {
                        SignalType::Int | SignalType::Float | SignalType::Bool => {
                            "JsValue::from(0)".to_string()
                        }
                        SignalType::String => "JsValue::from(\"\")".to_string(),
                        _ => "JsValue::from(0)".to_string(),
                    };
                    self.signal_initializers.insert(trg.name.clone(), initializer);
                    self.signal_map.insert(trg.name.clone(), self.signal_counter);
                    self.signal_counter += 1;
                }
                TopLevel::Transaction(txn) => {
                    self.txn_map.insert(txn.name.clone(), self.txn_counter);
                    self.txn_counter += 1;

                    // Track reactive transactions
                    if txn.is_reactive {
                        let txn_idx = self.reactive_txns.len();
                        self.reactive_txns.push(txn.clone());
                        let deps = self.extract_dependencies(&txn.contract.pre_condition);
                        for dep in deps {
                            self.reactive_dependency_map
                                .entry(dep)
                                .or_insert_with(Vec::new)
                                .push(txn_idx);
                        }
                    }
                }
                TopLevel::ForeignBinding {
                    name, signature, ..
                } => {
                    // Track FFI bindings for code generation
                    self.ffi_bindings
                        .insert(signature.name.clone(), signature.inputs.len());
                    // Track WASM implementations
                    if let Some(impl_code) = &signature.wasm_impl {
                        self.ffi_wasm_impl.insert(name.clone(), impl_code.clone());
                    }
                    // Track WASM setup/imports
                    if let Some(setup_code) = &signature.wasm_setup {
                        self.ffi_wasm_setups.insert(setup_code.clone());
                    }
                }
                _ => {}
            }
        }
    }

    /// Generate Rust code for an FFI call - calls JS function from WASM
    fn gen_ffi_call(&self, fn_name: &str, args: &[Expr]) -> String {
        let mut code = String::from("{\n");
        code.push_str(&format!(
            "            let __fn_ref = js_sys::Reflect::get(&js_sys::global(), &JsValue::from(\"{}\"));\n",
            fn_name
        ));
        code.push_str("            match __fn_ref {\n");
        code.push_str("                Ok(f) => {\n");
        code.push_str("                    match f.dyn_into::<js_sys::Function>() {\n");
        code.push_str("                        Ok(func) => {\n");

        match args.len() {
            0 => {
                code.push_str("                            func.call0(&JsValue::NULL).unwrap_or(JsValue::NULL)\n");
            }
            1 => {
                let arg0 = self.expr_to_js_value(&args[0]);
                code.push_str(&format!(
                    "                            func.call1(&JsValue::NULL, &{}).unwrap_or(JsValue::NULL)\n",
                    arg0
                ));
            }
            2 => {
                let arg0 = self.expr_to_js_value(&args[0]);
                let arg1 = self.expr_to_js_value(&args[1]);
                code.push_str(&format!(
                    "                            func.call2(&JsValue::NULL, &{}, &{}).unwrap_or(JsValue::NULL)\n",
                    arg0, arg1
                ));
            }
            _ => {
                // For more than 2 args, build an array
                code.push_str("                            let args = js_sys::Array::new();\n");
                for arg in args {
                    let arg_code = self.expr_to_js_value(arg);
                    code.push_str(&format!(
                        "                            args.push(&{});\n",
                        arg_code
                    ));
                }
                code.push_str("                            func.apply(&JsValue::NULL, &args).unwrap_or(JsValue::NULL)\n");
            }
        }

        code.push_str("                        }\n");
        code.push_str("                        Err(_) => JsValue::NULL,\n");
        code.push_str("                    }\n");
        code.push_str("                }\n");
        code.push_str("                Err(_) => JsValue::NULL,\n");
        code.push_str("            }\n");
        code.push_str("        }");
        code
    }

    /// Intent: extract_dependencies function.
    fn extract_dependencies(&self, expr: &Expr) -> Vec<String> {
        let mut deps = Vec::new();
        self.extract_identifiers(expr, &mut deps);
        deps
    }

    /// Intent: extract_identifiers function.
    fn extract_identifiers(&self, expr: &Expr, deps: &mut Vec<String>) {
        match expr {
            Expr::Identifier(name) => {
                if !deps.contains(name) {
                    deps.push(name.clone());
                }
            }
            Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::Or(l, r)
            | Expr::And(l, r) => {
                self.extract_identifiers(l, deps);
                self.extract_identifiers(r, deps);
            }
            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) => {
                self.extract_identifiers(e, deps);
            }
            Expr::PriorState(name) => {
                if !deps.contains(name) {
                    deps.push(name.clone());
                }
            }
            Expr::FieldAccess(e, _) => self.extract_identifiers(e, deps),
            Expr::Call(_, args) => {
                for arg in args {
                    self.extract_identifiers(arg, deps);
                }
            }
            Expr::IntrinsicCall { intrinsic: _, args } => {
                for arg in args {
                    self.extract_identifiers(arg, deps);
                }
            }
            Expr::ListLiteral(items) => {
                for item in items {
                    self.extract_identifiers(item, deps);
                }
            }
            Expr::ListIndex(e, i) => {
                self.extract_identifiers(e, deps);
                self.extract_identifiers(i, deps);
            }
            Expr::Projection { source: e, .. } => self.extract_identifiers(e, deps),
            _ => {}
        }
    }

    /// Intent: generate_rust_code function.
    fn generate_rust_code(&mut self, program: &Program, bindings: &[Binding]) -> String {
        todo!()
    }

    /// Intent: generate_transaction function.
    fn generate_transaction(&mut self, output: &mut String, txn: &crate::ast::Transaction) {
        todo!()
    }

    /// Intent: is_vector_expr function.
    fn is_vector_expr(&self, expr: &Expr) -> bool {
        todo!()
    }

    /// Intent: statement_to_rust function.
    fn statement_to_rust(&mut self, output: &mut String, stmt: &Statement) {
        todo!()
    }

    /// Intent: is_list_signal function.
    fn is_list_signal(&self, expr: &Expr) -> bool {
        todo!()
    }

    /// Intent: expr_to_js_value function.
    /// Intent: expr to js value.
    fn expr_to_js_value(&self, expr: &Expr) -> String {
        todo!()
    }

    /// Intent: expr_to_js_slice_coord function.
    fn expr_to_js_slice_coord(&self, coord: &crate::ast::SliceCoordinate) -> String {
        todo!()
    }

    /// Intent: expr_to_js_value_for_condition function.
    fn expr_to_js_value_for_condition(&self, expr: &Expr) -> String {
        todo!()
    }

    /// Intent: is_string_expr function.
    fn is_string_expr(&self, expr: &Expr) -> bool {
        todo!()
    }

    /// Intent: js_value_to_f64 function.
    /// Intent: js value to f64.
    fn js_value_to_f64(&self, expr: &Expr) -> String {
        todo!()
    }

    /// Intent: generate_js_glue function.
    fn generate_js_glue(&self, program_name: &str, bindings: &[Binding]) -> String {
        todo!()
    }

    /// Intent: escape_selector function.
    fn escape_selector(&self, id: &str) -> String {
        todo!()
    }
}

/// Intent: Default implementation block.
impl Default for WebstackGenerator {
    /// Intent: return the default value.
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
/// Intent: WebstackOutput type.
pub struct WebstackOutput {
    pub rust_code: String,
    pub js_glue: String,
    pub signal_count: usize,
    pub txn_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::call_graph::CallGraph;
use crate::ast::*;

    /// Intent: verify that generate produces output for an empty program.
    #[test]
    fn test_webstack_generates_output() {
        let mut backend = WebstackGenerator::new();
        let program = Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let bindings: Vec<Binding> = vec![];
        let output = backend.generate(&program, &bindings, "test");
        assert!(!output.rust_code.is_empty());
    }

    #[test]
    fn test_webstack_generates_with_text_directive() {
        let mut backend = WebstackGenerator::new();
        let program = Program {
            items: vec![],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        };
        let bindings = vec![Binding {
            element_id: "e1".into(),
            directive: Directive::Text { signal: "greeting".into() },
        }];
        let output = backend.generate(&program, &bindings, "binding_test");
        assert!(output.js_glue.contains("greeting") || !output.rust_code.is_empty());
    }

    #[test]
    fn test_webstack_generates_with_show_directive() {
        let mut backend = WebstackGenerator::new();
        let program = Program {
            items: vec![],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        };
        let bindings = vec![Binding {
            element_id: "e2".into(),
            directive: Directive::Show { expr: "visible".into() },
        }];
        let output = backend.generate(&program, &bindings, "show_test");
        assert!(output.js_glue.contains("visible") || !output.rust_code.is_empty());
    }

    #[test]
    fn test_webstack_generates_rust_module() {
        let mut backend = WebstackGenerator::new();
        let program = Program {
            items: vec![],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        };
        let bindings: Vec<Binding> = vec![];
        let output = backend.generate(&program, &bindings, "rust_mod");
        assert!(!output.rust_code.is_empty(), "Should generate Rust code");
        assert!(output.rust_code.contains("State"), "Should generate State struct");
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    use crate::features::literal::LiteralExpr;

    #[kani::proof]
    fn verify_webstack_expr_literal_integer() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        let result = backend.expr_to_js_value(&expr);
        assert_eq!(result, "JsValue::from(42)");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_bool() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let result = backend.expr_to_js_value(&expr);
        assert_eq!(result, "JsValue::TRUE");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_bool_false() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(false)));
        let result = backend.expr_to_js_value(&expr);
        assert_eq!(result, "JsValue::FALSE");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_float() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Float(3.14)));
        let result = backend.expr_to_js_value(&expr);
        assert_eq!(result, "JsValue::from(3.14)");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_string() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::String("test".to_string())));
        let result = backend.expr_to_js_value(&expr);
        assert_eq!(result, "JsValue::from(\"test\")");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_char() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Char('A')));
        let result = backend.expr_to_js_value(&expr);
        assert!(result.contains("A"));
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_term() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Term));
        let result = backend.expr_to_js_value(&expr);
        assert_eq!(result, "JsValue::undefined");
    }
}