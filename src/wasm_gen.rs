use crate::ast::{Program, TopLevel, Statement, Expr};
use crate::view_compiler::{ViewCompiler, Binding, Directive};
use std::collections::HashMap;

pub struct WasmGenerator {
    signal_counter: usize,
    txn_counter: usize,
    signal_map: HashMap<String, usize>,
    txn_map: HashMap<String, usize>,
}

impl WasmGenerator {
    pub fn new() -> Self {
        WasmGenerator {
            signal_counter: 0,
            txn_counter: 0,
            signal_map: HashMap::new(),
            txn_map: HashMap::new(),
        }
    }

    pub fn generate(&mut self, program: &Program, bindings: &[Binding]) -> WasmOutput {
        self.collect_signals_and_transactions(program);
        
        let rust_code = self.generate_rust_code(program, bindings);
        let js_glue = self.generate_js_glue(program, bindings);
        
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
                    self.signal_map.insert(decl.name.clone(), self.signal_counter);
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
        output.push_str("#[wasm_bindgen]\n");
        output.push_str("pub struct State {\n");
        output.push_str("    signals: Vec<JsValue>,\n");
        output.push_str("    dirty_signals: Vec<bool>,\n");
        output.push_str("}\n\n");
        
        output.push_str("#[wasm_bindgen]\n");
        output.push_str("impl State {\n");
        output.push_str("    #[wasm_bindgen(constructor)]\n");
        output.push_str("    pub fn new() -> Self {\n");
        output.push_str("        let signals = vec![JsValue::NULL; SIGNALS];\n");
        output.push_str("        let dirty_signals = vec![false; SIGNALS];\n");
        output.push_str("        State { signals, dirty_signals }\n");
        output.push_str("    }\n\n");
        
        output.push_str(&format!(
            "    pub fn get_signal(&mut self, id: usize) -> JsValue {{\n",
        ));
        output.push_str("        self.signals[id].clone()\n");
        output.push_str("    }\n\n");
        
        output.push_str(&format!(
            "    pub fn set_signal(&mut self, id: usize, value: JsValue) {{\n",
        ));
        output.push_str("        self.signals[id] = value;\n");
        output.push_str("        self.dirty_signals[id] = true;\n");
        output.push_str("    }\n\n");
        
        for (name, &id) in &self.signal_map {
            output.push_str(&format!(
                "    pub fn set_{}(&mut self, value: JsValue) {{\n",
                name
            ));
            output.push_str(&format!("        self.set_signal({}, value);\n", id));
            output.push_str("    }\n\n");
        }
        
        for (name, &id) in &self.txn_map {
            output.push_str(&format!(
                "    pub fn invoke_{}(&mut self) {{\n",
                name
            ));
            output.push_str(&format!("        // TODO: Execute transaction {}\n", id));
            output.push_str("        // Check preconditions, execute body, set postconditions\n");
            output.push_str("    }\n\n");
        }
        
        output.push_str("    pub fn poll_dispatch(&mut self) -> JsValue {\n");
        output.push_str("        let mut instructions = vec![];\n");
        for binding in bindings {
            if let Directive::Text { signal } = &binding.directive {
                if let Some(&sig_id) = self.signal_map.get(signal) {
                    output.push_str(&format!(
                        "        if self.dirty_signals[{}] {{\n",
                        sig_id
                    ));
                    output.push_str(&format!(
                        "            instructions.push(serde_json::json!({{\n",
                    ));
                    output.push_str(&format!("                \"op\": \"text\",\n"));
                    output.push_str(&format!("                \"el\": \"{}\",\n", binding.element_id));
                    output.push_str(&format!("                \"value\": self.signals[{}]\n", sig_id));
                    output.push_str("            }}));\n");
                    output.push_str("        }\n");
                }
            }
        }
        output.push_str("        self.dirty_signals.fill(false);\n");
        output.push_str("        serde_json::to_string(&instructions).unwrap().into()\n");
        output.push_str("    }\n");
        output.push_str("}\n\n");
        
        output.push_str("impl Default for State {\n");
        output.push_str("    fn default() -> Self {\n");
        output.push_str("        Self::new()\n");
        output.push_str("    }\n");
        output.push_str("}\n");
        
        output
    }

    fn generate_js_glue(&self, _program: &Program, bindings: &[Binding]) -> String {
        let mut output = String::new();
        
        output.push_str("(function() {\n");
        output.push_str("    'use strict';\n\n");
        
        output.push_str("    const ELEMENT_MAP = {\n");
        for binding in bindings {
            output.push_str(&format!(
                "        '{}': '[{}] {}',\n",
                binding.element_id,
                binding.directive.directive_name(),
                self.escape_selector(&binding.element_id)
            ));
        }
        output.push_str("    };\n\n");
        
        output.push_str("    let wasm = null;\n\n");
        output.push_str("    async function init(wasmUrl) {\n");
        output.push_str("        const response = await fetch(wasmUrl);\n");
        output.push_str("        const bytes = await response.arrayBuffer();\n");
        output.push_str("        const { State } = await wasm_bindgen(bytes);\n");
        output.push_str("        wasm = new State();\n");
        output.push_str("        attachListeners();\n");
        output.push_str("        startPollLoop();\n");
        output.push_str("    }\n\n");
        
        output.push_str("    function attachListeners() {\n");
        for binding in bindings {
            if let Directive::Trigger { event, txn } = &binding.directive {
                output.push_str(&format!(
                    "        document.querySelector('{}').addEventListener('{}', () => {{\n",
                    self.escape_selector(&binding.element_id),
                    event
                ));
                output.push_str(&format!("            wasm.invoke_{}({});\n", txn, "{}"));
                output.push_str("        });\n\n");
            }
        }
        output.push_str("    }\n\n");
        
        output.push_str("    function startPollLoop() {\n");
        output.push_str("        function poll() {\n");
        output.push_str("            const dispatch = wasm.poll_dispatch();\n");
        output.push_str("            if (dispatch) {\n");
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
        
        output.push_str("    window.rbv = { init };\n");
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

impl Directive {
    pub fn directive_name(&self) -> &'static str {
        match self {
            Directive::Text { .. } => "text",
            Directive::Show { .. } => "show",
            Directive::Hide { .. } => "hide",
            Directive::Trigger { .. } => "trigger",
            Directive::Class { .. } => "class",
            Directive::Attr { .. } => "attr",
        }
    }
}
