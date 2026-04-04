use crate::ast::{Contract, Expr, Program, Statement, TopLevel, Type};
use crate::view_compiler::{Binding, Directive};
use std::collections::HashMap;

#[derive(Clone)]
enum SignalType {
    Int,
    Float,
    Bool,
    String,
}

pub struct WasmGenerator {
    signal_counter: usize,
    txn_counter: usize,
    signal_map: HashMap<String, usize>,
    signal_types: HashMap<String, SignalType>,
    signal_initializers: HashMap<String, String>,
    txn_map: HashMap<String, usize>,
}

impl WasmGenerator {
    pub fn new() -> Self {
        WasmGenerator {
            signal_counter: 0,
            txn_counter: 0,
            signal_map: HashMap::new(),
            signal_types: HashMap::new(),
            signal_initializers: HashMap::new(),
            txn_map: HashMap::new(),
        }
    }

    pub fn generate(
        &mut self,
        program: &Program,
        bindings: &[Binding],
        program_name: &str,
    ) -> WasmOutput {
        self.collect_signals_and_transactions(program);

        let rust_code = self.generate_rust_code(program, bindings);
        let js_glue = self.generate_js_glue(program_name, bindings);

        WasmOutput {
            rust_code,
            js_glue,
            signal_count: self.signal_counter,
            txn_count: self.txn_counter,
        }
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
                        _ => SignalType::Int,
                    };
                    self.signal_types.insert(decl.name.clone(), signal_type);

                    let initializer = if let Some(expr) = &decl.expr {
                        self.expr_to_rust(expr)
                    } else {
                        "0".to_string()
                    };
                    self.signal_initializers
                        .insert(decl.name.clone(), initializer);

                    self.signal_map
                        .insert(decl.name.clone(), self.signal_counter);
                    self.signal_counter += 1;
                }
                TopLevel::Transaction(txn) => {
                    self.txn_map.insert(txn.name.clone(), self.txn_counter);
                    self.txn_counter += 1;
                }
                _ => {}
            }
        }
    }

    fn generate_rust_code(&self, program: &Program, bindings: &[Binding]) -> String {
        let mut output = String::new();

        output.push_str("use wasm_bindgen::prelude::*;\n\n");
        output.push_str(&format!(
            "const SIGNALS: usize = {};\n\n",
            self.signal_counter
        ));
        output.push_str("#[wasm_bindgen]\n");
        output.push_str("pub struct State {\n");
        output.push_str("    signals: Vec<i32>,\n");
        output.push_str("    dirty_signals: Vec<bool>,\n");
        output.push_str("}\n\n");

        output.push_str("#[wasm_bindgen]\n");
        output.push_str("impl State {\n");
        output.push_str("    #[wasm_bindgen(constructor)]\n");
        output.push_str("    pub fn new() -> Self {\n");
        output.push_str("        let signals = vec![\n");
        for (name, &id) in &self.signal_map {
            let init = self
                .signal_initializers
                .get(name)
                .cloned()
                .unwrap_or_else(|| "0".to_string());
            output.push_str(&format!("            {} as i32, // signal {}\n", init, id));
        }
        output.push_str("        ];\n");
        output.push_str("        let dirty_signals = vec![false; SIGNALS];\n");
        output.push_str("        State { signals, dirty_signals }\n");
        output.push_str("    }\n\n");

        output.push_str("    pub fn get_signal(&self, id: usize) -> i32 {\n");
        output.push_str("        self.signals[id]\n");
        output.push_str("    }\n\n");

        output.push_str("    fn mark_dirty(&mut self, id: usize) {\n");
        output.push_str("        if id < SIGNALS {\n");
        output.push_str("            self.dirty_signals[id] = true;\n");
        output.push_str("        }\n");
        output.push_str("    }\n\n");

        for (name, &id) in &self.signal_map {
            let getter = format!("    pub fn get_{}(&self) -> i32 {{\n", name);
            output.push_str(&getter);
            output.push_str(&format!("        self.signals[{}]\n", id));
            output.push_str("    }\n\n");

            let setter = format!("    pub fn set_{}(&mut self, value: i32) {{\n", name);
            output.push_str(&setter);
            output.push_str(&format!("        self.signals[{}] = value;\n", id));
            output.push_str(&format!("        self.mark_dirty({});\n", id));
            output.push_str("    }\n\n");
        }

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.generate_transaction(&mut output, txn);
            }
        }

        output.push_str("    pub fn poll_dispatch(&mut self) -> JsValue {\n");
        output.push_str("        let mut parts: Vec<String> = vec![];\n");

        output.push_str("        fn json_text(el: &str, val: i32) -> String {\n");
        output.push_str("            format!(\"{{\\\"op\\\":\\\"text\\\",\\\"el\\\":\\\"{}\\\",\\\"value\\\":{}}}\", el, val)\n");
        output.push_str("        }\n");
        for binding in bindings {
            if let Directive::Text { signal } = &binding.directive {
                if let Some(&sig_id) = self.signal_map.get(signal) {
                    output.push_str(&format!("        if self.dirty_signals[{}] {{\n", sig_id));
                    output.push_str(&format!(
                        "            let val = self.signals[{}];\n",
                        sig_id
                    ));
                    output.push_str("            let json = json_text(&format!(\"{}\", \"");
                    output.push_str(&binding.element_id);
                    output.push_str("\"), val);\n");
                    output.push_str("            parts.push(json);\n");
                    output.push_str("        }\n");
                }
            }
        }
        output.push_str("        self.dirty_signals.fill(false);\n");
        output.push_str("        let result = format!(\"[{}]\", parts.join(\",\"));\n");
        output.push_str("        result.into()\n");
        output.push_str("    }\n");
        output.push_str("}\n\n");

        output.push_str("impl Default for State {\n");
        output.push_str("    fn default() -> Self {\n");
        output.push_str("        Self::new()\n");
        output.push_str("    }\n");
        output.push_str("}\n");

        output
    }

    fn generate_transaction(&self, output: &mut String, txn: &crate::ast::Transaction) {
        let method_name = format!("    pub fn invoke_{}(&mut self) {{\n", txn.name);
        output.push_str(&method_name);

        output.push_str("        // Precondition\n");
        let pre_code = self.expr_to_rust_condition(&txn.contract.pre_condition);
        output.push_str(&format!("        if !({}) {{\n", pre_code));
        output.push_str("            return;\n");
        output.push_str("        }\n\n");

        output.push_str("        // Save prior state\n");
        for (name, &id) in &self.signal_map {
            output.push_str(&format!(
                "        let prior_{} = self.signals[{}];\n",
                name, id
            ));
        }
        output.push_str("\n");

        output.push_str("        // Execute body\n");
        for stmt in &txn.body {
            self.statement_to_rust(output, stmt);
        }

        output.push_str("\n");
        output.push_str("        // Postcondition\n");
        let post_code = self.expr_to_rust_postcondition(&txn.contract.post_condition);
        output.push_str(&format!("        if !({}) {{\n", post_code));
        output.push_str("            // Rollback\n");
        for (name, &id) in &self.signal_map {
            output.push_str(&format!(
                "            self.signals[{}] = prior_{};\n",
                id, name
            ));
        }
        output.push_str("            return;\n");
        output.push_str("        }\n");

        output.push_str("    }\n\n");
    }

    fn statement_to_rust(&self, output: &mut String, stmt: &Statement) {
        match stmt {
            Statement::Assignment {
                is_owned,
                name,
                expr,
            } => {
                if *is_owned {
                    let expr_code = self.expr_to_rust(expr);
                    if let Some(&id) = self.signal_map.get(name) {
                        output
                            .push_str(&format!("        self.signals[{}] = {};\n", id, expr_code));
                        output.push_str(&format!("        self.mark_dirty({});\n", id));
                    }
                }
            }
            Statement::Term(_) => {
                output.push_str("        // term - transaction settled\n");
            }
            _ => {}
        }
    }

    fn expr_to_rust(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => n.to_string(),
            Expr::Bool(true) => "true".to_string(),
            Expr::Bool(false) => "false".to_string(),
            Expr::Identifier(name) => {
                if let Some(&id) = self.signal_map.get(name) {
                    format!("self.signals[{}]", id)
                } else {
                    name.clone()
                }
            }
            Expr::PriorState(name) => {
                if let Some(&id) = self.signal_map.get(name) {
                    format!("prior_{}", name)
                } else {
                    name.clone()
                }
            }
            Expr::Add(a, b) => format!("({} + {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Sub(a, b) => format!("({} - {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Mul(a, b) => format!("({} * {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Div(a, b) => format!("({} / {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Eq(a, b) => format!("({} == {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Ne(a, b) => format!("({} != {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Lt(a, b) => format!("({} < {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Le(a, b) => format!("({} <= {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Gt(a, b) => format!("({} > {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Ge(a, b) => format!("({} >= {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::And(a, b) => format!("({} && {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Or(a, b) => format!("({} || {})", self.expr_to_rust(a), self.expr_to_rust(b)),
            Expr::Not(a) => format!("(!{})", self.expr_to_rust(a)),
            Expr::Neg(a) => format!("(-{})", self.expr_to_rust(a)),
            _ => "true".to_string(),
        }
    }

    fn expr_to_rust_condition(&self, expr: &Expr) -> String {
        self.expr_to_rust(expr)
    }

    fn expr_to_rust_postcondition(&self, expr: &Expr) -> String {
        self.expr_to_rust(expr)
    }

    fn generate_js_glue(&self, program_name: &str, bindings: &[Binding]) -> String {
        let mut output = String::new();

        output.push_str("(async function() {\n");
        output.push_str("    'use strict';\n\n");

        output.push_str("    const ELEMENT_MAP = {\n");
        for binding in bindings {
            output.push_str(&format!(
                "        '{}': '{}',\n",
                binding.element_id,
                self.escape_selector(&binding.element_id)
            ));
        }
        output.push_str("    };\n\n");

        output.push_str(&format!(
            "    const wasm_pkg = await import('./pkg/{}.js');\n",
            program_name
        ));
        output.push_str("    await wasm_pkg.default();\n");
        output.push_str("    const wasm = new wasm_pkg.State();\n\n");

        output.push_str("    const TRIGGER_MAP = {\n");
        for binding in bindings {
            if let Directive::Trigger { event, txn } = &binding.directive {
                output.push_str(&format!(
                    "        '{}': {{ event: '{}', txn: '{}' }},\n",
                    binding.element_id, event, txn
                ));
            }
        }
        output.push_str("    };\n\n");

        output.push_str("    function attachListeners() {\n");
        output.push_str("        for (const [elId, config] of Object.entries(TRIGGER_MAP)) {\n");
        output.push_str("            const el = document.querySelector(ELEMENT_MAP[elId]);\n");
        output.push_str("            if (!el) continue;\n");
        output.push_str("            el.addEventListener(config.event, () => {\n");
        output.push_str("                const txnName = `invoke_${config.txn}`;\n");
        output.push_str("                wasm[txnName]();\n");
        output.push_str("            });\n");
        output.push_str("        }\n");
        output.push_str("    }\n\n");

        output.push_str("    function startPollLoop() {\n");
        output.push_str("        function poll() {\n");
        output.push_str("            const dispatch = wasm.poll_dispatch();\n");
        output.push_str("            if (dispatch && dispatch !== '[]') {\n");
        output.push_str("                applyInstructions(JSON.parse(dispatch));\n");
        output.push_str("            }\n");
        output.push_str("            requestAnimationFrame(poll);\n");
        output.push_str("        }\n");
        output.push_str("        requestAnimationFrame(poll);\n");
        output.push_str("    }\n\n");

        output.push_str("    function applyInstructions(instructions) {\n");
        output.push_str("        for (const inst of instructions) {\n");
        output.push_str("            const el = document.querySelector(ELEMENT_MAP[inst.el]);\n");
        output.push_str("            if (!el) continue;\n");
        output.push_str("            switch (inst.op) {\n");
        output.push_str("                case 'text':\n");
        output.push_str("                    el.textContent = inst.value;\n");
        output.push_str("                    break;\n");
        output.push_str("                case 'show':\n");
        output.push_str("                    el.hidden = !inst.visible;\n");
        output.push_str("                    break;\n");
        output.push_str("                case 'class_add':\n");
        output.push_str("                    el.classList.add(inst.class);\n");
        output.push_str("                    break;\n");
        output.push_str("                case 'class_remove':\n");
        output.push_str("                    el.classList.remove(inst.class);\n");
        output.push_str("                    break;\n");
        output.push_str("            }\n");
        output.push_str("        }\n");
        output.push_str("    }\n\n");

        output.push_str("    attachListeners();\n");
        output.push_str("    startPollLoop();\n");
        output.push_str("})();\n");

        output
    }

    fn escape_selector(&self, id: &str) -> String {
        if id.starts_with("rbv-") {
            id.to_string()
        } else {
            format!("#{}", id)
        }
    }
}

impl Default for WasmGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct WasmOutput {
    pub rust_code: String,
    pub js_glue: String,
    pub signal_count: usize,
    pub txn_count: usize,
}
