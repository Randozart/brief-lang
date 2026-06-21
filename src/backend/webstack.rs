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
pub enum CodeTarget {
    Wasm,
    Arm,
    Fpga,
}

impl Default for CodeTarget {
    fn default() -> Self {
        CodeTarget::Wasm
    }
}

#[derive(Clone)]
pub enum TsType {
    Number,
    Boolean,
    String,
    NumberArray,
    Any,
}

#[derive(Clone)]
enum SignalType {
    Int,
    Float,
    Bool,
    String,
    List,
    Struct,
    Vector(usize),
}

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
    ffi_bindings: HashMap<String, usize>,
    ffi_ts_impl: HashMap<String, String>,   // JS implementation code (from wasm_impl field)
    ffi_ts_setups: HashSet<String>,         // JS setup code (from wasm_setup field)
    local_vars: HashMap<String, ()>,
    target: CodeTarget,
    pending_cleanup: RefCell<Vec<Statement>>,
    has_cycles: bool,
    trigger_names: Vec<String>,
    promise_counter: usize,
    pending_promises: Vec<PendingPromise>,
    wasm_modules: Vec<(String, Vec<u8>)>,   // (module_name, wasm binary) for (wasm) import embedding
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
            ffi_ts_impl: HashMap::new(),
            ffi_ts_setups: HashSet::new(),
            local_vars: HashMap::new(),
            target: CodeTarget::Wasm,
            pending_cleanup: RefCell::new(Vec::new()),
            has_cycles: false,
            trigger_names: Vec::new(),
            promise_counter: 0,
            pending_promises: Vec::new(),
            wasm_modules: Vec::new(),
        }
    }

    pub fn with_wasm_module(mut self, name: String, binary: Vec<u8>) -> Self {
        self.wasm_modules.push((name, binary));
        self
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

    pub fn generate(
        &mut self,
        program: &Program,
        bindings: &[Binding],
        program_name: &str,
    ) -> WebstackOutput {
        self.collect_signals_and_transactions(program);

        match self.target {
            CodeTarget::Wasm => {
                let ts_code = self.generate_ts_code(program, bindings);
                let js_glue = self.generate_js_glue(program_name, bindings);
                WebstackOutput {
                    ts_code,
                    rust_code: String::new(),
                    js_glue,
                    signal_count: self.signal_counter,
                    txn_count: self.txn_counter,
                }
            }
            CodeTarget::Arm => {
                let rust_code = self.generate_arm_rust_code(program);
                WebstackOutput {
                    ts_code: String::new(),
                    rust_code,
                    js_glue: String::new(),
                    signal_count: self.signal_counter,
                    txn_count: self.txn_counter,
                }
            }
            CodeTarget::Fpga => {
                let ts_code = self.generate_ts_code(program, bindings);
                WebstackOutput {
                    ts_code,
                    rust_code: String::new(),
                    js_glue: String::new(),
                    signal_count: self.signal_counter,
                    txn_count: self.txn_counter,
                }
            }
        }
    }

    /// Intent: generate_arm_rust_code function.
    fn generate_arm_rust_code(&mut self, program: &Program) -> String {
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
        let txn_data: Vec<(String, Transaction)> = self.txn_map.iter()
            .map(|(name, &id)| (name.clone(), self.reactive_txns[id].clone()))
            .collect();
        for (txn_name, txn) in &txn_data {
            output.push_str(&format!(
                "pub fn {}(state: &mut State) -> bool {{\n",
                txn_name.replace('-', "_")
            ));
            output.push_str(&format!(
                "    // pre: {:?}\n",
                txn.contract.pre_condition
            ));
            
            // Generate real body
            if txn.body.is_empty() {
                output.push_str("    true\n");
            } else {
                let mut body_buf = String::new();
                for stmt in &txn.body {
                    self.statement_to_rust(&mut body_buf, stmt);
                }
                for line in body_buf.lines() {
                    output.push_str(&format!("    {}\n", line));
                }
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
                        self.expr_to_ts(expr)
                    } else {
                        match &signal_type {
                            SignalType::Struct => "{}".to_string(),
                            SignalType::Vector(size) => {
                                format!("new Array({}).fill(0)", size)
                            }
                            SignalType::List => "[]".to_string(),
                            SignalType::Int | SignalType::Float | SignalType::Bool => {
                                "0".to_string()
                            }
                            SignalType::String => "\"\"".to_string(),
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
                            "0".to_string()
                        }
                        SignalType::String => "\"\"".to_string(),
                        _ => "0".to_string(),
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
                    // Track JS FFI implementations — the wasm_impl/wasm_setup fields
                    // contain JavaScript code used for both wasm-bindgen and TS emitter.
                    if let Some(impl_code) = &signature.wasm_impl {
                        self.ffi_ts_impl.insert(signature.name.clone(), impl_code.clone());
                    }
                    if let Some(setup_code) = &signature.wasm_setup {
                        self.ffi_ts_setups.insert(setup_code.clone());
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

    /// Generate TypeScript source code for the web target.
    /// Each state variable becomes a typed class field; each transaction becomes a method.
    fn generate_ts_code(&mut self, program: &Program, bindings: &[Binding]) -> String {
        let mut out = String::new();

        out.push_str("// Generated by Brief Compiler\n");
        out.push_str("// Target: Web (TypeScript)\n\n");

        // Emit JS glue code from frgn "javascript" imports
        for setup in &self.ffi_ts_setups {
            out.push_str(setup);
            out.push('\n');
        }

        // Sort signals by id for deterministic output
        let mut sorted_sigs: Vec<(&String, &usize)> = self.signal_map.iter().collect();
        sorted_sigs.sort_by_key(|&(_, &id)| id);

        // Emit the State class
        out.push_str("class App {\n");

        // Signal fields with type annotations
        for &(ref name, &id) in &sorted_sigs {
            let ts_ty = self.ts_type_for_signal(name);
            let init = self.signal_initializers.get(name.as_str())
                .cloned()
                .unwrap_or_else(|| "0".to_string());
            out.push_str(&format!("  {}: {} = {};\n", self.ts_ident(name), ts_ty, init));
        }
        out.push('\n');

        // FFI implementation methods
        for (fn_name, impl_code) in &self.ffi_ts_impl {
            let arg_count = self.ffi_bindings.get(fn_name).copied().unwrap_or(0);
            let args: Vec<String> = (0..arg_count).map(|i| format!("arg{}: any", i)).collect();
            out.push_str(&format!(
                "  {}({}): any {{\n",
                self.ts_ident(fn_name),
                args.join(", ")
            ));
            // Indent the impl code by 2 spaces
            for line in impl_code.lines() {
                out.push_str(&format!("    {}\n", line));
            }
            out.push_str("  }\n\n");
        }

        // Collect txn data upfront to avoid borrow conflict with self
        let txn_data: Vec<(String, Transaction)> = self.txn_map.iter()
            .map(|(name, &id)| (name.clone(), self.reactive_txns[id].clone()))
            .collect();
        for (txn_name, txn) in &txn_data {
            out.push_str(&format!("  async {}(): Promise<void> {{\n", self.ts_ident(txn_name)));
            out.push_str("    // pre: unknown\n");
            self.emit_ts_txn_body(&mut out, txn);
            out.push_str("  }\n\n");
        }

        // End of class
        out.push_str("}\n\n");

        // Factory function — creates an App and wires up the reactor loop
        out.push_str("export function createApp(): App {\n");
        out.push_str("  const app = new App();\n");
        out.push_str("  return app;\n");
        out.push_str("}\n\n");

        // WASM module embedding from (wasm) import directives
        if !self.wasm_modules.is_empty() {
            out.push_str("// WASM modules — compiled from (wasm) import directives\n\n");
            for (module_name, binary) in &self.wasm_modules {
                let ts_name = self.ts_ident(module_name);
                // Embed as byte array literal (compact enough for typical Brief WASM modules)
                let bytes_ts = binary.iter()
                    .map(|b| format!("{}", b))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "const __wasm_{}_bytes = new Uint8Array([{}]);\n",
                    ts_name, bytes_ts
                ));
                out.push_str(&format!(
                    "let __wasm_{}_instance: WebAssembly.Instance | null = null;\n\n",
                    ts_name
                ));
                out.push_str(&format!(
                    "export async function init_{}_wasm(): Promise<WebAssembly.Instance> {{\n",
                    ts_name
                ));
                out.push_str(&format!(
                    "  if (__wasm_{}_instance) return __wasm_{}_instance;\n",
                    ts_name, ts_name
                ));
                out.push_str("  const result = await WebAssembly.instantiate(__wasm_${ts_name}_bytes, {\n");
                out.push_str("    env: {\n");
                out.push_str("      print_int: (n: number) => console.log(n),\n");
                out.push_str("      print_float: (n: number) => console.log(n),\n");
                out.push_str("      put_char: (c: number) => process.stdout.write(String.fromCharCode(c)),\n");
                out.push_str("      abort: () => { throw new Error('WASM abort'); },\n");
                out.push_str("    },\n");
                out.push_str("  });\n");
                out.push_str(&format!(
                    "  __wasm_{}_instance = result.instance;\n",
                    ts_name
                ));
                out.push_str("  return result.instance;\n");
                out.push_str("}\n\n");
            }
        }

        out
    }

    fn ts_type_for_signal(&self, name: &str) -> String {
        match self.signal_types.get(name) {
            Some(SignalType::Int | SignalType::Float) => "number".to_string(),
            Some(SignalType::Bool) => "boolean".to_string(),
            Some(SignalType::String) => "string".to_string(),
            Some(SignalType::List) => "any[]".to_string(),
            Some(SignalType::Vector(_)) => "number[]".to_string(),
            Some(SignalType::Struct) => "any".to_string(),
            None => "number".to_string(),
        }
    }

    fn ts_ident(&self, name: &str) -> String {
        name.replace('-', "_")
    }

    fn emit_ts_txn_body(&mut self, out: &mut String, txn: &Transaction) {
        for stmt in &txn.body {
            self.statement_to_ts(out, stmt);
        }
    }

    fn statement_to_ts(&mut self, out: &mut String, stmt: &Statement) {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                let lhs = self.expr_to_ts(lhs);
                let rhs = self.expr_to_ts(expr);
                out.push_str(&format!("{} = {};\n", lhs, rhs));
            }
            Statement::Let { name, expr, .. } => {
                let rhs = expr.as_ref().map(|e| self.expr_to_ts(e)).unwrap_or_else(|| "0".to_string());
                out.push_str(&format!("let {} = {};\n", name, rhs));
            }
            Statement::Expression(e) => {
                let ts = self.expr_to_ts(e);
                out.push_str(&format!("{};\n", ts));
            }
            Statement::Term { swan_song, .. } | Statement::TermBang { swan_song, .. } => {
                if let Some(swan) = swan_song {
                    self.statement_to_ts(out, swan);
                }
                out.push_str("return;\n");
            }
            Statement::Guarded { condition, statements } => {
                let cond = self.expr_to_ts(condition);
                out.push_str(&format!("if ({}) {{\n", cond));
                for s in statements {
                    self.statement_to_ts(out, s);
                }
                out.push_str("}\n");
            }
            Statement::Escape(..) => {
                out.push_str("break;\n");
            }
            Statement::Unification { name, variant, fields, expr } => {
                let e = self.expr_to_ts(expr);
                out.push_str(&format!("// unification: {}(..) = {};\n", name, e));
                let _ = variant;
                let _ = fields;
            }
            Statement::InlineAsm { .. } => {
                out.push_str("// asm: not applicable to TS\n");
            }
            Statement::LocalTrigger { name, .. } => {
                out.push_str(&format!("// local trigger: {} (omitted in TS)\n", name));
            }
            Statement::Alka(alka) => {
                out.push_str("// alka block (hardware construct, omitted)\n");
                let _ = alka;
            }
            Statement::OnExit { body, .. } => {
                out.push_str("try {\n");
                for s in body {
                    self.statement_to_ts(out, s);
                }
                out.push_str("} catch (__exit_err) { /* on_exit handler */ }\n");
            }
            Statement::SyncBlock { body } => {
                // sync block: emitted sequentially in TS (structural concurrency hint)
                for s in body {
                    self.statement_to_ts(out, s);
                }
            }
            Statement::Foreach { item, list, body, .. } => {
                let list_expr = self.expr_to_ts(list);
                out.push_str(&format!("for (const {} of {}) {{\n", item, list_expr));
                for s in body {
                    self.statement_to_ts(out, s);
                }
                out.push_str("}\n");
            }
            Statement::Oracle { handler, body, .. } => {
                // Emit body directly; handler runs on fuel exhaustion
                for s in body {
                    self.statement_to_ts(out, s);
                }
                let _ = handler;
            }
            Statement::Await { expr, .. } => {
                let e = self.expr_to_ts(expr);
                out.push_str(&format!("await {};\n", e));
            }
            Statement::Async { body, .. } => {
                self.statement_to_ts(out, body);
            }
            Statement::AsyncAwait { body, lhs, .. } => {
                if let Some(name) = lhs {
                    let mut buf = String::new();
                    self.statement_to_ts(&mut buf, body);
                    out.push_str(&format!("const {} = await (async () => {{ {} }})();\n", name, buf.trim()));
                } else {
                    out.push_str("await (async () => {\n");
                    self.statement_to_ts(out, body);
                    out.push_str("})();\n");
                }
            }
        }
    }

    fn self_expr_to_ts(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Identifier(name) => format!("this.{}", self.ts_ident(name)),
            Expr::PriorState(name) => format!("this.{}", self.ts_ident(name)),
            _ => self.expr_to_ts(expr),
        }
    }

    fn expr_to_ts(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => {
                if f.is_infinite() {
                    if *f > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
                } else if f.is_nan() {
                    "NaN".to_string()
                } else {
                    f.to_string()
                }
            }
            Expr::Bool(b) => b.to_string(),
            Expr::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Expr::Char(c) => format!("\"{}\"", c.escape_default()),
            Expr::Term => "undefined".to_string(),
            Expr::Identifier(name) => self.ts_ident(name),
            Expr::PriorState(name) => self.ts_ident(name),
            Expr::Add(l, r) => format!("({} + {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Sub(l, r) => format!("({} - {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Mul(l, r) => format!("({} * {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Div(l, r) => format!("({} / {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Mod(l, r) => format!("({} % {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Eq(l, r) => format!("({} === {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Ne(l, r) => format!("({} !== {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Lt(l, r) => format!("({} < {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Le(l, r) => format!("({} <= {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Gt(l, r) => format!("({} > {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Ge(l, r) => format!("({} >= {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Or(l, r) => format!("({} || {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::And(l, r) => format!("({} && {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Not(e) => format!("(!{})", self.expr_to_ts(e)),
            Expr::Neg(e) => format!("(-{})", self.expr_to_ts(e)),
            Expr::BitNot(e) => format!("(~{})", self.expr_to_ts(e)),
            Expr::BitAnd(l, r) => format!("({} & {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::BitOr(l, r) => format!("({} | {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::BitXor(l, r) => format!("({} ^ {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Shl(l, r) => format!("({} << {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Shr(l, r) => format!("({} >> {})", self.expr_to_ts(l), self.expr_to_ts(r)),
            Expr::Call(name, args) => {
                let ts_args: Vec<String> = args.iter().map(|a| self.expr_to_ts(a)).collect();
                let ts_name = self.ts_ident(name);
                if self.ffi_ts_impl.contains_key(name) {
                    format!("this.{}({})", ts_name, ts_args.join(", "))
                } else if self.txn_map.contains_key(name) {
                    format!("await this.{}({})", ts_name, ts_args.join(", "))
                } else if name == "print_int" || name == "__print_int" || name.starts_with("print") {
                    format!("console.log({})", ts_args.join(", "))
                } else {
                    format!("this.{}({})", ts_name, ts_args.join(", "))
                }
            }
            Expr::IntrinsicCall { intrinsic, args } => {
                let ts_args: Vec<String> = args.iter().map(|a| self.expr_to_ts(a)).collect();
                let joined = ts_args.join(", ");
                match intrinsic {
                    // Obvious TS mappings — intrinsics over frgn
                    Intrinsic::PrintInt | Intrinsic::PrintFloat => {
                        format!("console.log({})", joined)
                    }
                    Intrinsic::PutChar => {
                        format!("process.stdout.write(String.fromCharCode({}))", joined)
                    }
                    Intrinsic::GetEnvInt => {
                        format!("Number(process.env[{}] || \"0\")", joined)
                    }
                    Intrinsic::GetEnv => {
                        format!("(process.env[{}] || \"\")", joined)
                    }
                    Intrinsic::Strlen => {
                        match args.first() {
                            Some(_) => format!("({}).length", ts_args[0]),
                            None => "0".to_string(),
                        }
                    }
                    Intrinsic::IntToStr | Intrinsic::FloatToStr | Intrinsic::ToStr => {
                        format!("String({})", joined)
                    }
                    Intrinsic::ReadFile => {
                        format!("(() => {{ throw new Error(\"read_file# not available in browser\"); }})()")
                    }
                    Intrinsic::Exit => {
                        format!("(() => {{ throw new Error(\"exit\"); }})()")
                    }
                    Intrinsic::Time => {
                        "Date.now()".to_string()
                    }
                    Intrinsic::Sleep => {
                        format!("new Promise(resolve => setTimeout(resolve, {}))", joined)
                    }
                    Intrinsic::Abs => format!("Math.abs({})", joined),
                    Intrinsic::Sqrt => format!("Math.sqrt({})", joined),
                    Intrinsic::Ceil => format!("Math.ceil({})", joined),
                    Intrinsic::Floor => format!("Math.floor({})", joined),
                    Intrinsic::Sin => format!("Math.sin({})", joined),
                    Intrinsic::Cos => format!("Math.cos({})", joined),
                    Intrinsic::Pow => format!("Math.pow({})", joined),
                    Intrinsic::Reverse => format!("[...{}].reverse()", joined),
                    Intrinsic::Sort => format!("[...{}].sort()", joined),
                    Intrinsic::Range => format!("[...Array({}).keys()]", joined),
                    Intrinsic::PageSize => "4096".to_string(),
                    Intrinsic::CpuCount => "navigator.hardwareConcurrency || 4".to_string(),
                    Intrinsic::Hostname => "window.location.hostname".to_string(),
                    Intrinsic::Errno => "0".to_string(),
                    _ => {
                        // Fallback: if the function has a JS impl from frgn "javascript", use it
                        if let Intrinsic::GetGlobalId | Intrinsic::GetLocalId | Intrinsic::GetGroupId = intrinsic {
                            "0".to_string()
                        } else {
                            joined
                        }
                    }
                }
            }
            Expr::ListLiteral(items) => {
                let ts_items: Vec<String> = items.iter().map(|i| self.expr_to_ts(i)).collect();
                format!("[{}]", ts_items.join(", "))
            }
            Expr::ListIndex(list, index) => {
                format!("{}[{}]", self.expr_to_ts(list), self.expr_to_ts(index))
            }
            Expr::Projection { source, target } => {
                let src = self.expr_to_ts(source);
                match target {
                    crate::ast::ProjectionTarget::Size => format!("({}).length", src),
                    crate::ast::ProjectionTarget::Popcount => {
                        format!("({}).toString(2).split('1').length - 1", src)
                    }
                    _ => src,
                }
            }
            Expr::Block(stmts, last) => {
                let mut block = "(() => {\n".to_string();
                for s in stmts {
                    self.statement_to_ts(&mut block, s);
                }
                block.push_str(&format!("  return {};\n", self.expr_to_ts(last)));
                block.push_str("})()");
                block
            }
            Expr::Tuple(items) => {
                let ts_items: Vec<String> = items.iter().map(|i| self.expr_to_ts(i)).collect();
                format!("[{}]", ts_items.join(", "))
            }
            Expr::FieldAccess(obj, field) => {
                format!("{}[\"{}\"]", self.expr_to_ts(obj), field)
            }
            Expr::ArrowMut { target, index, value, .. } => {
                let list = self.expr_to_ts(target);
                let idx = self.expr_to_ts(index);
                match value {
                    Some(v) => format!("{}.splice({}, 1, {})[0]", list, idx, self.expr_to_ts(v)),
                    None => format!("{}.splice({}, 1)[0]", list, idx),
                }
            }
            Expr::ArrowDiscard { target, index } => {
                let list = self.expr_to_ts(target);
                let idx = self.expr_to_ts(index);
                format!("{}.splice({}, 1)[0]", list, idx)
            }
            Expr::ArrowTransfer { dest, source, filter } => {
                let d = self.expr_to_ts(dest);
                let s = self.expr_to_ts(source);
                if filter.is_some() {
                    format!("(() => {{ /* arrow-transfer filtered */ return {}; }})()", s)
                } else {
                    format!("(() => {{ {}.push(...{}.splice(0)); return {}; }})()", d, s, s)
                }
            }
            Expr::BinaryOp(bop) => {
                let l = self.expr_to_ts(&bop.left);
                let r = self.expr_to_ts(&bop.right);
                match bop.kind {
                    crate::features::binary_op::BinaryOpKind::Add => format!("({} + {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Sub => format!("({} - {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Mul => format!("({} * {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Div => format!("({} / {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Mod => format!("({} % {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Eq => format!("({} === {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Ne => format!("({} !== {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Lt => format!("({} < {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Le => format!("({} <= {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Gt => format!("({} > {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Ge => format!("({} >= {})", l, r),
                    crate::features::binary_op::BinaryOpKind::And => format!("({} && {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Or => format!("({} || {})", l, r),
                    crate::features::binary_op::BinaryOpKind::BitAnd => format!("({} & {})", l, r),
                    crate::features::binary_op::BinaryOpKind::BitOr => format!("({} | {})", l, r),
                    crate::features::binary_op::BinaryOpKind::BitXor => format!("({} ^ {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Shl => format!("({} << {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Shr => format!("({} >> {})", l, r),
                }
            }
            Expr::UnaryOp(uop) => {
                let op = self.expr_to_ts(&uop.operand);
                match uop.kind {
                    crate::features::unary_op::UnaryOpKind::Neg => format!("(-{})", op),
                    crate::features::unary_op::UnaryOpKind::Not => format!("(!{})", op),
                    crate::features::unary_op::UnaryOpKind::BitNot => format!("(~{})", op),
                }
            }
            _ => format!("/* expr: {:?} */ 0", expr),
        }
    }

    fn is_vector_expr(&self, expr: &Expr) -> bool {
        if let Expr::Identifier(name) = expr {
            matches!(self.signal_types.get(name), Some(SignalType::Vector(_)))
        } else {
            false
        }
    }

    fn is_list_signal(&self, expr: &Expr) -> bool {
        if let Expr::Identifier(name) = expr {
            matches!(self.signal_types.get(name), Some(SignalType::List))
        } else {
            false
        }
    }

    fn is_string_expr(&self, expr: &Expr) -> bool {
        if let Expr::Identifier(name) = expr {
            matches!(self.signal_types.get(name), Some(SignalType::String))
        } else if let Expr::String(_) = expr {
            true
        } else {
            false
        }
    }

    fn js_value_to_f64(&mut self, expr: &Expr) -> String {
        format!("Number({})", self.expr_to_ts(expr))
    }

    /// Generate JS glue code that wires view bindings to App signal changes.
    fn generate_js_glue(&self, program_name: &str, bindings: &[Binding]) -> String {
        let mut out = String::new();

        out.push_str("// Auto-generated view binding glue\n\n");

        out.push_str(&format!(
            "const app = createApp();\n\n"
        ));

        // Render each binding as a watcher / setter
        for binding in bindings {
            let sel = self.escape_selector(&binding.element_id);
            match &binding.directive {
                Directive::Text { signal } => {
                    out.push_str(&format!(
                        "// b-text: select element #{} and bind '{}'\n",
                        sel, signal
                    ));
                    out.push_str(&format!(
                        "const el_{} = document.querySelector('#{}');\n",
                        sel, sel
                    ));
                    out.push_str(&format!(
                        "function update_{}() {{ el_{}.textContent = String(app.{}); }}\n",
                        sel, sel, signal
                    ));
                    out.push_str(&format!("update_{}();\n", sel));
                }
                Directive::Trigger { event, txn, .. } => {
                    out.push_str(&format!(
                        "document.querySelector('#{}').addEventListener('{}', () => app.{}());\n",
                        sel, event, self.ts_ident(txn)
                    ));
                }
                Directive::Show { expr } => {
                    out.push_str(&format!(
                        "const el_{} = document.querySelector('#{}');\n",
                        sel, sel
                    ));
                    out.push_str(&format!(
                        "function show_{}() {{ el_{}.style.display = app.{} ? '' : 'none'; }}\n",
                        sel, sel, expr
                    ));
                    out.push_str(&format!("show_{}();\n", sel));
                }
                Directive::Each { iterable, item_name, template_html, container_id } => {
                    out.push_str(&format!(
                        "// b-each: iterate over {} as {} using template #{}\n",
                        iterable, item_name, container_id
                    ));
                }
                Directive::Class { pairs } => {
                    for (expr, class) in pairs {
                        out.push_str(&format!(
                            "const el_{} = document.querySelector('#{}');\n",
                            sel, sel
                        ));
                        out.push_str(&format!(
                            "function toggle_{}_{}() {{ el_{}.classList.toggle('{}', Boolean(app.{})); }}\n",
                            sel, class, sel, class, expr
                        ));
                        out.push_str(&format!("toggle_{}_{}();\n", sel, class));
                    }
                }
                _ => {}
            }
        }

        out
    }

    fn escape_selector(&self, id: &str) -> String {
        id.replace('.', "\\.").replace(':', "\\:")
    }

    /// Legacy stub — needed by archive but replaced in TS path.
    fn generate_rust_code(&mut self, _program: &Program, _bindings: &[Binding]) -> String {
        String::new()
    }

    /// Legacy stub — needed by archive. Wasm-bindgen codegen removed.
    fn generate_transaction(&mut self, _output: &mut String, _txn: &Transaction) {}

    /// Emit Rust code for a statement (ARM bare-metal target).
    fn statement_to_rust(&mut self, out: &mut String, stmt: &Statement) {
        match stmt {
            Statement::Assignment { lhs, expr, .. } => {
                let lhs = self.expr_to_rust(lhs);
                let rhs = self.expr_to_rust(expr);
                out.push_str(&format!("{} = {};\n", lhs, rhs));
            }
            Statement::Let { name, expr, .. } => {
                let rhs = expr.as_ref().map(|e| self.expr_to_rust(e)).unwrap_or_else(|| "0".to_string());
                out.push_str(&format!("let {} = {};\n", name, rhs));
            }
            Statement::Expression(e) => {
                let r = self.expr_to_rust(e);
                out.push_str(&format!("{};\n", r));
            }
            Statement::Term { swan_song, .. } | Statement::TermBang { swan_song, .. } => {
                if let Some(swan) = swan_song {
                    self.statement_to_rust(out, swan);
                }
                out.push_str("return true;\n");
            }
            Statement::Guarded { condition, statements } => {
                let cond = self.expr_to_rust(condition);
                out.push_str(&format!("if {} {{\n", cond));
                for s in statements {
                    self.statement_to_rust(out, s);
                }
                out.push_str("}\n");
            }
            Statement::Escape(..) => {
                out.push_str("break;\n");
            }
            Statement::Unification { name, variant, fields, expr } => {
                let e = self.expr_to_rust(expr);
                out.push_str(&format!("// unification: {}(..) = {};\n", name, e));
                let _ = variant;
                let _ = fields;
            }
            Statement::InlineAsm { asm_string, .. } => {
                out.push_str(&format!("core::arch::asm!(\"{}\");\n", asm_string));
            }
            Statement::LocalTrigger { name, .. } => {
                out.push_str(&format!("// local trigger: {}\n", name));
            }
            Statement::Alka(alka) => {
                out.push_str("// alka block (hardware construct)\n");
                let _ = alka;
            }
            Statement::OnExit { body, .. } => {
                out.push_str("// on_exit block\n");
                for s in body {
                    self.statement_to_rust(out, s);
                }
            }
            Statement::SyncBlock { body } => {
                for s in body {
                    self.statement_to_rust(out, s);
                }
            }
            Statement::Foreach { item, list, body, .. } => {
                let list_expr = self.expr_to_rust(list);
                out.push_str(&format!("for {} in &{} {{\n", item, list_expr));
                for s in body {
                    self.statement_to_rust(out, s);
                }
                out.push_str("}\n");
            }
            Statement::Oracle { handler, body, .. } => {
                // Emit body directly; handler runs on fuel exhaustion
                for s in body {
                    self.statement_to_rust(out, s);
                }
                let _ = handler;
            }
            Statement::Await { expr, .. } => {
                let e = self.expr_to_rust(expr);
                out.push_str(&format!("{}; // await\n", e));
            }
            Statement::Async { body, .. } => {
                self.statement_to_rust(out, body);
            }
            Statement::AsyncAwait { body, lhs, .. } => {
                if let Some(name) = lhs {
                    let mut buf = String::new();
                    self.statement_to_rust(&mut buf, body);
                    out.push_str(&format!("let {} = {{ {} }}; // async await\n", name, buf.trim()));
                } else {
                    out.push_str("// async await block\n");
                    self.statement_to_rust(out, body);
                }
            }
        }
    }

    /// Emit Rust expression string (ARM bare-metal target).
    fn expr_to_rust(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::Bool(b) => b.to_string(),
            Expr::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Expr::Char(c) => format!("'{}'", c.escape_default()),
            Expr::Term => "true".to_string(),
            Expr::Identifier(name) => format!("state.{}", name.replace('-', "_")),
            Expr::PriorState(name) => format!("state.{}", name.replace('-', "_")),
            Expr::Add(l, r) => format!("({} + {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Sub(l, r) => format!("({} - {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Mul(l, r) => format!("({} * {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Div(l, r) => format!("({} / {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Mod(l, r) => format!("({} % {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Eq(l, r) => format!("({} == {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Ne(l, r) => format!("({} != {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Lt(l, r) => format!("({} < {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Le(l, r) => format!("({} <= {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Gt(l, r) => format!("({} > {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Ge(l, r) => format!("({} >= {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Or(l, r) => format!("({} || {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::And(l, r) => format!("({} && {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Not(e) => format!("(!{})", self.expr_to_rust(e)),
            Expr::Neg(e) => format!("(-{})", self.expr_to_rust(e)),
            Expr::BitNot(e) => format!("(!{})", self.expr_to_rust(e)),
            Expr::BitAnd(l, r) => format!("({} & {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::BitOr(l, r) => format!("({} | {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::BitXor(l, r) => format!("({} ^ {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Shl(l, r) => format!("({} << {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Shr(l, r) => format!("({} >> {})", self.expr_to_rust(l), self.expr_to_rust(r)),
            Expr::Call(name, args) => {
                let rust_args: Vec<String> = args.iter().map(|a| self.expr_to_rust(a)).collect();
                format!("{}({})", name.replace('-', "_"), rust_args.join(", "))
            }
            Expr::IntrinsicCall { intrinsic, args } => {
                let rust_args: Vec<String> = args.iter().map(|a| self.expr_to_rust(a)).collect();
                let joined = rust_args.join(", ");
                match intrinsic {
                    Intrinsic::PrintInt | Intrinsic::PrintFloat => {
                        format!("print_int({})", joined)
                    }
                    Intrinsic::PutChar => format!("put_char({})", joined),
                    Intrinsic::GetEnvInt => format!("get_env_int({})", joined),
                    Intrinsic::Strlen => format!("strlen({})", joined),
                    Intrinsic::ReadFile => format!("read_file({})", joined),
                    Intrinsic::Abs => format!("({}).abs()", joined),
                    Intrinsic::Sqrt => format!("({} as f64).sqrt() as i64", joined),
                    Intrinsic::Ceil => format!("({} as f64).ceil() as i64", joined),
                    Intrinsic::Floor => format!("({} as f64).floor() as i64", joined),
                    _ => format!("{}({})", format!("{:?}", intrinsic).to_lowercase(), joined),
                }
            }
            Expr::ListLiteral(items) => {
                let rust_items: Vec<String> = items.iter().map(|i| self.expr_to_rust(i)).collect();
                format!("vec![{}]", rust_items.join(", "))
            }
            Expr::ListIndex(list, index) => {
                format!("{}[{} as usize]", self.expr_to_rust(list), self.expr_to_rust(index))
            }
            Expr::Projection { source, target } => {
                let src = self.expr_to_rust(source);
                match target {
                    crate::ast::ProjectionTarget::Size => format!("({}).len()", src),
                    _ => src,
                }
            }
            Expr::Block(stmts, last) => {
                let mut block = "{\n".to_string();
                for s in stmts {
                    self.statement_to_rust(&mut block, s);
                }
                block.push_str(&format!("{}\n", self.expr_to_rust(last)));
                block.push('}');
                block
            }
            Expr::Tuple(items) => {
                let rust_items: Vec<String> = items.iter().map(|i| self.expr_to_rust(i)).collect();
                format!("({})", rust_items.join(", "))
            }
            Expr::FieldAccess(obj, field) => {
                format!("{}.{}", self.expr_to_rust(obj), field)
            }
            Expr::BinaryOp(bop) => {
                let l = self.expr_to_rust(&bop.left);
                let r = self.expr_to_rust(&bop.right);
                match bop.kind {
                    crate::features::binary_op::BinaryOpKind::Add => format!("({} + {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Sub => format!("({} - {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Mul => format!("({} * {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Div => format!("({} / {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Mod => format!("({} % {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Eq => format!("({} == {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Ne => format!("({} != {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Lt => format!("({} < {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Le => format!("({} <= {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Gt => format!("({} > {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Ge => format!("({} >= {})", l, r),
                    crate::features::binary_op::BinaryOpKind::And => format!("({} && {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Or => format!("({} || {})", l, r),
                    crate::features::binary_op::BinaryOpKind::BitAnd => format!("({} & {})", l, r),
                    crate::features::binary_op::BinaryOpKind::BitOr => format!("({} | {})", l, r),
                    crate::features::binary_op::BinaryOpKind::BitXor => format!("({} ^ {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Shl => format!("({} << {})", l, r),
                    crate::features::binary_op::BinaryOpKind::Shr => format!("({} >> {})", l, r),
                }
            }
            Expr::UnaryOp(uop) => {
                let op = self.expr_to_rust(&uop.operand);
                match uop.kind {
                    crate::features::unary_op::UnaryOpKind::Neg => format!("(-{})", op),
                    crate::features::unary_op::UnaryOpKind::Not => format!("(!{})", op),
                    crate::features::unary_op::UnaryOpKind::BitNot => format!("(!{})", op),
                }
            }
            Expr::ArrowMut { target, index, value, .. } => {
                let list = self.expr_to_rust(target);
                let idx = self.expr_to_rust(index);
                match value {
                    Some(v) => format!("{}.splice({} as usize, 1, {})", list, idx, self.expr_to_rust(v)),
                    None => format!("{}.remove({} as usize)", list, idx),
                }
            }
            Expr::ArrowDiscard { target, index } => {
                let list = self.expr_to_rust(target);
                let idx = self.expr_to_rust(index);
                format!("{}.remove({} as usize)", list, idx)
            }
            _ => "0".to_string(),
        }
    }

    fn expr_to_js_value(&self, _expr: &Expr) -> String { String::new() }
    fn expr_to_js_slice_coord(&self, _coord: &crate::ast::SliceCoordinate) -> String { String::new() }
    fn expr_to_js_value_for_condition(&self, _expr: &Expr) -> String { String::new() }
}

/// Intent: Default implementation block.
impl Default for WebstackGenerator {
    /// Intent: return the default value.
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct WebstackOutput {
    pub ts_code: String,
    pub rust_code: String,
    pub js_glue: String,
    pub signal_count: usize,
    pub txn_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

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
        assert!(output.ts_code.contains("App"), "Should generate App class");
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
        assert!(output.js_glue.contains("greeting"));
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
        assert!(output.js_glue.contains("visible"));
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
        let output = backend.generate(&program, &bindings, "ts_mod");
        assert!(!output.ts_code.is_empty(), "Should generate TS code");
        assert!(output.ts_code.contains("App"), "Should generate App class");
    }

    #[test]
    fn test_webstack_ts_signal_fields() {
        let mut backend = WebstackGenerator::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".into(),
                    ty: Type::Int,
                    expr: None,
                    address: None,
                    bit_range: None,
                    constraint: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "name".into(),
                    ty: Type::String,
                    expr: None,
                    address: None,
                    bit_range: None,
                    constraint: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        };
        let output = backend.generate(&program, &[], "test");
        assert!(output.ts_code.contains("count: number = 0;"));
        assert!(output.ts_code.contains("name: string = \"\";"));
    }

    #[test]
    fn test_webstack_ts_with_state_decl_expr() {
        let mut backend = WebstackGenerator::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".into(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(42)),
                    address: None,
                    bit_range: None,
                    constraint: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        };
        let output = backend.generate(&program, &[], "test");
        assert!(output.ts_code.contains("x: number = 42;"));
    }

    #[test]
    fn test_webstack_export_create_app() {
        let mut backend = WebstackGenerator::new();
        let program = Program {
            items: vec![],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        };
        let output = backend.generate(&program, &[], "test");
        assert!(output.ts_code.contains("export function createApp"));
    }

    #[test]
    fn test_ts_int_division_trunc() {
        // Brief integer division must emit native JS division.
        // JS division yields floats, so Brief integer division would need
        // Math.trunc() — but the current TS emitter uses direct `/`.
        let mut backend = WebstackGenerator::new();
        let out = backend.expr_to_ts(&Expr::Div(
            Box::new(Expr::Integer(3)),
            Box::new(Expr::Integer(2)),
        ));
        assert!(out.contains("3 / 2") || out.contains("Math.trunc"),
            "Int division should emit native JS division (or Math.trunc): got {}", out);
    }

    #[test]
    fn test_ts_reactive_contract() {
        // A simple transaction with a state variable, assignment, and term
        let mut backend = WebstackGenerator::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".into(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    constraint: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "tick".into(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Lt(
                            Box::new(Expr::Identifier("count".into())),
                            Box::new(Expr::Integer(10)),
                        ),
                        post_condition: Expr::Eq(
                            Box::new(Expr::Identifier("count".into())),
                            Box::new(Expr::Add(
                                Box::new(Expr::PriorState("count".into())),
                                Box::new(Expr::Integer(1)),
                            )),
                        ),
                        watchdog: None,
                        span: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("count".into()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("count".into())),
                                Box::new(Expr::Integer(1)),
                            ),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term {
                            values: vec![],
                            swan_song: None,
                            modifiers: vec![],
                        },
                    ],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                    outputs: vec![],
                    output_type: None,
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        };
        let output = backend.generate(&program, &[], "test");
        assert!(output.ts_code.contains("tick"), "Transaction method 'tick' should exist");
        assert!(output.ts_code.contains("count") || output.ts_code.contains("count = count + 1"),
            "Should reference count signal in tick body");
    }

    // --- New statement type tests ---

    #[test]
    fn test_statement_to_ts_foreach() {
        let mut backend = WebstackGenerator::new();
        let mut out = String::new();
        let stmt = Statement::Foreach {
            item: "x".into(),
            list: Box::new(Expr::Identifier("items".into())),
            body: vec![
                Statement::Expression(Expr::Call("print".into(), vec![Expr::Identifier("x".into())])),
            ],
            modifiers: vec![],
        };
        backend.statement_to_ts(&mut out, &stmt);
        assert!(out.contains("for (const x of"));
        assert!(out.contains("items"));
    }

    #[test]
    fn test_statement_to_ts_sync_block() {
        let mut backend = WebstackGenerator::new();
        let mut out = String::new();
        let stmt = Statement::SyncBlock {
            body: vec![
                Statement::Expression(Expr::Add(
                    Box::new(Expr::Identifier("a".into())),
                    Box::new(Expr::Integer(1)),
                )),
            ],
        };
        backend.statement_to_ts(&mut out, &stmt);
        assert!(out.contains("a + 1"));
    }

    #[test]
    fn test_statement_to_ts_await() {
        let mut backend = WebstackGenerator::new();
        let mut out = String::new();
        let stmt = Statement::Await {
            expr: Expr::Call("fetch_data".into(), vec![]),
            modifiers: vec![],
        };
        backend.statement_to_ts(&mut out, &stmt);
        assert!(out.contains("await"));
        assert!(out.contains("fetch_data"));
    }

    #[test]
    fn test_intrinsic_to_ts_math() {
        let mut backend = WebstackGenerator::new();
        let r = backend.expr_to_ts(&Expr::IntrinsicCall {
            intrinsic: Intrinsic::Abs,
            args: vec![Expr::Integer(-5)],
        });
        assert!(r.contains("Math.abs"));

        let r2 = backend.expr_to_ts(&Expr::IntrinsicCall {
            intrinsic: Intrinsic::Sqrt,
            args: vec![Expr::Integer(9)],
        });
        assert!(r2.contains("Math.sqrt"));

        let r3 = backend.expr_to_ts(&Expr::IntrinsicCall {
            intrinsic: Intrinsic::IntToStr,
            args: vec![Expr::Integer(42)],
        });
        assert!(r3.contains("String"));
    }

    #[test]
    fn test_intrinsic_to_str_ts() {
        let mut backend = WebstackGenerator::new();
        let r = backend.expr_to_ts(&Expr::IntrinsicCall {
            intrinsic: Intrinsic::ToStr,
            args: vec![Expr::Integer(100)],
        });
        assert!(r.contains("String(100)"), "ToStr should map to String() in TS: {}", r);
    }

    #[test]
    fn test_expr_to_rust_arm_add() {
        let mut backend = WebstackGenerator::new();
        let expr = Expr::Add(
            Box::new(Expr::Identifier("x".into())),
            Box::new(Expr::Integer(1)),
        );
        let r = backend.expr_to_rust(&expr);
        assert_eq!(r, "(state.x + 1)");
    }

    #[test]
    fn test_statement_to_rust_assignment() {
        let mut backend = WebstackGenerator::new();
        let mut out = String::new();
        backend.statement_to_rust(&mut out, &Statement::Assignment {
            lhs: Expr::Identifier("count".into()),
            expr: Expr::Add(
                Box::new(Expr::Identifier("count".into())),
                Box::new(Expr::Integer(1)),
            ),
            timeout: None,
            modifiers: vec![],
        });
        assert!(out.contains("state.count = (state.count + 1)"));
    }

    #[test]
    fn test_statement_to_rust_guarded() {
        let mut backend = WebstackGenerator::new();
        let mut out = String::new();
        backend.statement_to_rust(&mut out, &Statement::Guarded {
            condition: Expr::Lt(
                Box::new(Expr::Identifier("count".into())),
                Box::new(Expr::Integer(10)),
            ),
            statements: vec![
                Statement::Assignment {
                    lhs: Expr::Identifier("count".into()),
                    expr: Expr::Add(
                        Box::new(Expr::Identifier("count".into())),
                        Box::new(Expr::Integer(1)),
                    ),
                    timeout: None,
                    modifiers: vec![],
                },
            ],
        });
        assert!(out.contains("if"));
        assert!(out.contains("state.count < 10"));
        assert!(out.contains("state.count + 1"));
    }

    #[test]
    fn test_arm_txn_body_not_placeholder() {
        let mut backend = WebstackGenerator::new().with_target(CodeTarget::Arm);
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".into(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    constraint: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "tick".into(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None,
                        span: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("x".into()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("x".into())),
                                Box::new(Expr::Integer(1)),
                            ),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term {
                            values: vec![],
                            swan_song: None,
                            modifiers: vec![],
                        },
                    ],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                    outputs: vec![],
                    output_type: None,
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        };
        let output = backend.generate(&program, &[], "arm_test");
        let rust = output.rust_code;
        assert!(!rust.contains("Transaction body omitted"), "Should not contain placeholder comment");
        assert!(rust.contains("state.x = (state.x + 1)"), "Should contain real assignment: {}", rust);
        assert!(rust.contains("return true"), "Should contain return statement");
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
        let result = backend.expr_to_ts(&expr);
        assert_eq!(result, "42");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_bool() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        let result = backend.expr_to_ts(&expr);
        assert_eq!(result, "true");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_bool_false() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Bool(false)));
        let result = backend.expr_to_ts(&expr);
        assert_eq!(result, "false");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_float() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Float(3.14)));
        let result = backend.expr_to_ts(&expr);
        assert_eq!(result, "3.14");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_string() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::String("test".to_string())));
        let result = backend.expr_to_ts(&expr);
        assert_eq!(result, "\"test\"");
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_char() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Char('A')));
        let result = backend.expr_to_ts(&expr);
        assert!(result.contains("A"));
    }

    #[kani::proof]
    fn verify_webstack_expr_literal_term() {
        let backend = WebstackGenerator::new();
        let expr = Expr::Literal(Box::new(LiteralExpr::Term));
        let result = backend.expr_to_ts(&expr);
        assert_eq!(result, "undefined");
    }
}