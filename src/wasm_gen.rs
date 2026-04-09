use crate::ast::{Contract, Expr, Program, Statement, TopLevel, Transaction, Type};
use crate::view_compiler::{Binding, Directive};
use std::collections::HashMap;

#[derive(Clone)]
enum SignalType {
    Int,
    Float,
    Bool,
    String,
    List,
    Struct,
}

pub struct WasmGenerator {
    signal_counter: usize,
    txn_counter: usize,
    signal_map: HashMap<String, usize>,
    signal_types: HashMap<String, SignalType>,
    signal_initializers: HashMap<String, String>,
    txn_map: HashMap<String, usize>,
    reactive_txns: Vec<Transaction>,
    reactive_dependency_map: HashMap<String, Vec<usize>>,
    reactor_speed: u32,
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
            reactive_txns: Vec::new(),
            reactive_dependency_map: HashMap::new(),
            reactor_speed: 10, // Default 10Hz
        }
    }

    pub fn set_reactor_speed(&mut self, speed: u32) {
        self.reactor_speed = speed;
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
                        Type::Applied(name, _) if name == "List" => SignalType::List,
                        Type::Generic(name, _) if name == "List" => SignalType::List,
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
                            SignalType::Struct => "js_sys::Object::new()".to_string(),
                            _ => "js_sys::Array::new()".to_string(),
                        }
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
                _ => {}
            }
        }
    }

    fn extract_dependencies(&self, expr: &Expr) -> Vec<String> {
        let mut deps = Vec::new();
        self.extract_identifiers(expr, &mut deps);
        deps
    }

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
            Expr::ListLiteral(items) => {
                for item in items {
                    self.extract_identifiers(item, deps);
                }
            }
            Expr::ListIndex(e, i) => {
                self.extract_identifiers(e, deps);
                self.extract_identifiers(i, deps);
            }
            Expr::ListLen(e) => self.extract_identifiers(e, deps),
            _ => {}
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
        output.push_str("    signals: Vec<JsValue>,\n");
        output.push_str("    dirty_signals: Vec<bool>,\n");
        output.push_str("    each_templates: Vec<(String, String, String)>,\n");
        output.push_str("    show_bindings: Vec<(String, String, bool)>,\n");
        output.push_str("    last_reactive_run: u64,\n");
        output.push_str("}\n\n");

        output.push_str("#[wasm_bindgen]\n");
        output.push_str("impl State {\n");
        output.push_str("    #[wasm_bindgen(constructor)]\n");
        output.push_str("    pub fn new() -> Self {\n");
        output.push_str("        let signals = vec![\n");
        let mut sorted_signals: Vec<(&String, &usize)> = self.signal_map.iter().collect();
        sorted_signals.sort_by_key(|&(_, &id)| id);

        for (name, &id) in sorted_signals {
            let init = self
                .signal_initializers
                .get(name)
                .cloned()
                .unwrap_or_else(|| "js_sys::Array::new()".to_string());
            if let Some(sig_type) = self.signal_types.get(name) {
                if matches!(sig_type, SignalType::List) {
                    output.push_str(&format!(
                        "            JsValue::from({}), // signal {}\n",
                        init, id
                    ));
                } else {
                    output.push_str(&format!("            {}.into(), // signal {}\n", init, id));
                }
            } else {
                output.push_str(&format!("            {}.into(), // signal {}\n", init, id));
            }
        }
        output.push_str("        ];\n");
        output.push_str("        let dirty_signals = vec![false; SIGNALS];\n");

        output.push_str(
            "        let mut each_templates: Vec<(String, String, String)> = Vec::new();\n",
        );
        for binding in bindings {
            if let Directive::Each {
                iterable,
                item_name,
                template_html,
                ..
            } = &binding.directive
            {
                output.push_str(&format!(
                    "        each_templates.push((\"{}\".to_string(), \"{}\".to_string(), r#\"{}\"#.to_string()));\n",
                    iterable, item_name, template_html
                ));
            }
        }

        output
            .push_str("        let mut show_bindings: Vec<(String, String, bool)> = Vec::new();\n");
        for binding in bindings {
            if let Directive::Show { expr } = &binding.directive {
                output.push_str(&format!(
                    "        show_bindings.push((\"{}\".to_string(), \"{}\".to_string(), true));\n",
                    binding.element_id, expr
                ));
            }
        }

        output
            .push_str("        State { signals, dirty_signals, each_templates, show_bindings, last_reactive_run: 0 }\n");
        output.push_str("    }\n\n");

        output.push_str("    pub fn get_signal(&self, id: usize) -> JsValue {\n");
        output.push_str("        self.signals[id].clone()\n");
        output.push_str("    }\n\n");

        output.push_str("    fn html_escape(s: &str) -> String {\n");
        output.push_str("        s.replace('&', \"&amp;\").replace('<', \"&lt;\").replace('>', \"&gt;\").replace('\"', \"&quot;\")\n");
        output.push_str("    }\n\n");

        output.push_str("    pub fn render_each(&self, iterable: &str) -> String {\n");
        output.push_str("        for (iter_name, item_name, template) in &self.each_templates {\n");
        output.push_str("            if iter_name == iterable {\n");
        for (name, &id) in &self.signal_map {
            output.push_str(&format!("                if iterable == \"{}\" {{\n", name));
            output.push_str(&format!(
                "                    let list = &self.signals[{}];\n",
                id
            ));
            output.push_str("                    if list.is_array() {\n");
            output.push_str("                        let arr = js_sys::Array::from(list);\n");
            output.push_str("                        let mut result = String::new();\n");
            output.push_str("                        for i in 0..arr.length() {\n");
            output.push_str("                            let item = arr.get(i);\n");
            output.push_str("                            let item_str = if item.is_string() { item.as_string().unwrap_or_default() } else { format!(\"{:?}\", item) };\n");
            output.push_str(
                "                            let escaped = Self::html_escape(&item_str);\n",
            );
            output.push_str("                            let mut html = template.clone();\n");

            output.push_str("                            // Handle b-text=\"item\"\n");
            output.push_str("                            let search_simple = format!(\"b-text=\\\"{}\\\">\", item_name);
");
            output.push_str(
                "                            if let Some(pos) = html.find(&search_simple) {\n",
            );
            output.push_str(
                "                                let after = &html[pos + search_simple.len()..];\n",
            );
            output
                .push_str("                                if let Some(end) = after.find('<') {\n");
            output.push_str("                                    let before = &html[..pos];\n");
            output.push_str("                                    let rest = &after[end..];\n");
            output.push_str("                                    html = format!(\"{}>{}{}\", before, escaped, rest);\n");
            output.push_str("                                }\n");
            output.push_str("                            }\n");

            output.push_str(
                "                            // Handle b-text=\"item.prop\" (recursive)\n",
            );
            output.push_str("                            let prop_prefix = format!(\"b-text=\\\"{}.\", item_name);
");
            output.push_str("                            let mut search_pos = 0;\n");
            output.push_str("                            while let Some(pos) = html[search_pos..].find(&prop_prefix) {\n");
            output.push_str("                                let abs_pos = search_pos + pos;\n");
            output.push_str("                                let after = &html[abs_pos + prop_prefix.len()..];\n");
            output.push_str(
                "                                if let Some(end_quote) = after.find('\\\"') {\n",
            );
            output
                .push_str("                                    let path = &after[..end_quote];\n");
            output
                .push_str("                                    let mut current = item.clone();\n");
            output.push_str("                                    for part in path.split('.') {\n");
            output.push_str(
                "                                        let key = JsValue::from_str(part);\n",
            );
            output.push_str("                                        current = js_sys::Reflect::get(&current, &key).unwrap_or(JsValue::UNDEFINED);\n");
            output.push_str("                                    }\n");
            output.push_str("                                    let val_str = if current.is_string() { current.as_string().unwrap_or_default() } else if let Some(n) = current.as_f64() { n.to_string() } else { format!(\"{:?}\", current) };\n");
            output.push_str(
                "                                    let val_esc = Self::html_escape(&val_str);\n",
            );
            output.push_str("                                    if let Some(tag_end) = after[end_quote..].find('>') {\n");
            output.push_str("                                        let abs_tag_end = abs_pos + prop_prefix.len() + end_quote + tag_end;\n");
            output.push_str(
                "                                        let rest = &html[abs_tag_end + 1..];\n",
            );
            output.push_str("                                        if let Some(content_end) = rest.find('<') {\n");
            output.push_str("                                            html = format!(\"{}{}{}\", &html[..abs_tag_end + 1], val_esc, &rest[content_end..]);\n");
            output.push_str("                                        }\n");
            output.push_str("                                    }\n");
            output.push_str("                                }\n");
            output.push_str("                                search_pos = abs_pos + 1;\n");
            output.push_str("                            }\n");

            output.push_str("                            // Handle b-show=\"item.prop\"\n");
            output.push_str("                            let show_prefix = format!(\"b-show=\\\"{}.\", item_name);
");
            output.push_str("                            search_pos = 0;\n");
            output.push_str("                            while let Some(pos) = html[search_pos..].find(&show_prefix) {\n");
            output.push_str("                                let abs_pos = search_pos + pos;\n");
            output.push_str("                                let after = &html[abs_pos + show_prefix.len()..];\n");
            output.push_str(
                "                                if let Some(end_quote) = after.find('\\\"') {\n",
            );
            output
                .push_str("                                    let path = &after[..end_quote];\n");
            output
                .push_str("                                    let mut current = item.clone();\n");
            output.push_str("                                    for part in path.split('.') {\n");
            output.push_str(
                "                                        let key = JsValue::from_str(part);\n",
            );
            output.push_str("                                        current = js_sys::Reflect::get(&current, &key).unwrap_or(JsValue::UNDEFINED);\n");
            output.push_str("                                    }\n");
            output.push_str("                                    if !current.is_truthy() {\n");
            output.push_str("                                        if let Some(tag_start) = html[..abs_pos].rfind('<') {\n");
            output.push_str("                                            let tag_after = &html[tag_start+1..];\n");
            output.push_str("                                            if let Some(space_pos) = tag_after.find(|c: char| c.is_whitespace() || c == '>') {\n");
            output.push_str("                                                let ins = tag_start + 1 + space_pos;\n");
            output.push_str("                                                html = format!(\"{} style=\\\"display:none;\\\" {}\", &html[..ins], &html[ins..]);\n");
            output.push_str("                                            }\n");
            output.push_str("                                        }\n");
            output.push_str("                                    }\n");
            output.push_str("                                }\n");
            output.push_str("                                search_pos = abs_pos + 1;\n");
            output.push_str("                            }\n");

            output.push_str("                            // Handle b-hide=\"item.prop\"\n");
            output.push_str("                            let hide_prefix = format!(\"b-hide=\\\"{}.\", item_name);
");
            output.push_str("                            search_pos = 0;\n");
            output.push_str("                            while let Some(pos) = html[search_pos..].find(&hide_prefix) {\n");
            output.push_str("                                let abs_pos = search_pos + pos;\n");
            output.push_str("                                let after = &html[abs_pos + hide_prefix.len()..];\n");
            output.push_str(
                "                                if let Some(end_quote) = after.find('\\\"') {\n",
            );
            output
                .push_str("                                    let path = &after[..end_quote];\n");
            output
                .push_str("                                    let mut current = item.clone();\n");
            output.push_str("                                    for part in path.split('.') {\n");
            output.push_str(
                "                                        let key = JsValue::from_str(part);\n",
            );
            output.push_str("                                        current = js_sys::Reflect::get(&current, &key).unwrap_or(JsValue::UNDEFINED);\n");
            output.push_str("                                    }\n");
            output.push_str("                                    if current.is_truthy() {\n");
            output.push_str("                                        if let Some(tag_start) = html[..abs_pos].rfind('<') {\n");
            output.push_str("                                            let tag_after = &html[tag_start+1..];\n");
            output.push_str("                                            if let Some(space_pos) = tag_after.find(|c: char| c.is_whitespace() || c == '>') {\n");
            output.push_str("                                                let ins = tag_start + 1 + space_pos;\n");
            output.push_str("                                                html = format!(\"{} style=\\\"display:none;\\\" {}\", &html[..ins], &html[ins..]);\n");
            output.push_str("                                            }\n");
            output.push_str("                                        }\n");
            output.push_str("                                    }\n");
            output.push_str("                                }\n");
            output.push_str("                                search_pos = abs_pos + 1;\n");
            output.push_str("                            }\n");

            output.push_str("                            result.push_str(&html);\n");
            output.push_str("                        }\n");
            output.push_str("                        return result;\n");
            output.push_str("                    }\n");
            output.push_str("                }\n");
        }
        output.push_str("            }\n");
        output.push_str("        }\n");
        output.push_str("        String::new()\n");
        output.push_str("    }\n\n");
        output.push_str("    fn signal_map(&self) -> std::collections::HashMap<String, usize> {\n");
        output.push_str("        let mut map = std::collections::HashMap::new();\n");
        for (name, &id) in &self.signal_map {
            output.push_str(&format!(
                "        map.insert(\"{}\".to_string(), {});\n",
                name, id
            ));
        }
        output.push_str("        map\n");
        output.push_str("    }\n\n");

        output.push_str("    fn mark_dirty(&mut self, id: usize) {\n");
        output.push_str("        if id < SIGNALS {\n");
        output.push_str("            self.dirty_signals[id] = true;\n");
        output.push_str("        }\n");
        output.push_str("    }\n\n");

        output
            .push_str("    fn list_concat(&self, signal_id: usize, other: JsValue) -> JsValue {\n");
        output.push_str("        let current = self.signals[signal_id].clone();\n");
        output.push_str("        let arr = js_sys::Array::new();\n");
        output.push_str("        if current.is_array() {\n");
        output.push_str("            let curr_arr = js_sys::Array::from(&current);\n");
        output.push_str("            for i in 0..curr_arr.length() {\n");
        output.push_str("                arr.push(&curr_arr.get(i));\n");
        output.push_str("            }\n");
        output.push_str("        }\n");
        output.push_str("        if other.is_array() {\n");
        output.push_str("            let other_arr = js_sys::Array::from(&other);\n");
        output.push_str("            for i in 0..other_arr.length() {\n");
        output.push_str("                arr.push(&other_arr.get(i));\n");
        output.push_str("            }\n");
        output.push_str("        }\n");
        output.push_str("        arr.into()\n");
        output.push_str("    }\n\n");

        for (name, sig_type) in &self.signal_types {
            let &id = self.signal_map.get(name).unwrap();
            match sig_type {
                SignalType::List | SignalType::Struct => {
                    let getter = format!("    pub fn get_{}(&self) -> JsValue {{\n", name);
                    output.push_str(&getter);
                    output.push_str(&format!("        self.signals[{}].clone()\n", id));
                    output.push_str("    }\n\n");

                    let setter = format!("    pub fn set_{}(&mut self, value: JsValue) {{\n", name);
                    output.push_str(&setter);
                    output.push_str(&format!("        self.signals[{}] = value;\n", id));
                    output.push_str(&format!("        self.mark_dirty({});\n", id));
                    output.push_str("    }\n\n");
                }
                _ => {
                    let getter = format!("    pub fn get_{}(&self) -> i32 {{\n", name);
                    output.push_str(&getter);
                    output.push_str(&format!(
                        "        self.signals[{}].as_f64().unwrap_or(0.0) as i32\n",
                        id
                    ));
                    output.push_str("    }\n\n");

                    let setter = format!("    pub fn set_{}(&mut self, value: i32) {{\n", name);
                    output.push_str(&setter);
                    output.push_str(&format!(
                        "        self.signals[{}] = JsValue::from(value);\n",
                        id
                    ));
                    output.push_str(&format!("        self.mark_dirty({});\n", id));
                    output.push_str("    }\n\n");
                }
            }
        }

        for item in &program.items {
            if let TopLevel::Transaction(txn) = item {
                self.generate_transaction(&mut output, txn);
            }
        }

        output.push_str("    pub fn poll_dispatch(&mut self) -> JsValue {\n");

        // Add reactive transaction execution
        if !self.reactive_txns.is_empty() {
            let interval_ms = (1000.0 / self.reactor_speed as f64) as u64;
            output.push_str("        // Run reactive transactions with timing\n");
            output.push_str(&format!("        let interval_ms = {}u64;\n", interval_ms));
            output.push_str("        let now = js_sys::Date::now() as u64;\n");
            output.push_str("        if now - self.last_reactive_run >= interval_ms {\n");
            output.push_str("            self.last_reactive_run = now;\n");
            output.push_str("            let mut changed = false;\n");
            output.push_str("            for _ in 0..1000 {\n");
            output.push_str("                changed = false;\n");
            for txn in &self.reactive_txns {
                let method_name = txn.name.replace(".", "_");
                output.push_str(&format!("                self.invoke_{}();\n", method_name));
                output.push_str(
                    "                if self.dirty_signals.iter().any(|&d| d) { changed = true; }\n",
                );
            }
            output.push_str("                if !changed { break; }\n");
            output.push_str("            }\n");
            output.push_str("        }\n");
        }

        output.push_str("        let mut parts: Vec<String> = vec![];\n");

        output.push_str("        fn json_text(el: &str, val: JsValue) -> String {\n");
        output.push_str("            if let Some(n) = val.as_f64() {\n");
        output.push_str("                format!(\"{{\\\"op\\\":\\\"text\\\",\\\"el\\\":\\\"{}\\\",\\\"value\\\":{}}}\", el, n as i32)\n");
        output.push_str("            } else {\n");
        output.push_str("                format!(\"{{\\\"op\\\":\\\"text\\\",\\\"el\\\":\\\"{}\\\",\\\"value\\\":0}}\", el)\n");
        output.push_str("            }\n");
        output.push_str("        }\n");
        output.push_str("        let eval_show = |signals: &[JsValue], signal_map: &std::collections::HashMap<&str, usize>, expr: &str| -> bool {\n");
        output.push_str("            // Simple expression evaluator for show conditions\n");
        output.push_str(
            "            // Handles: signal == value, signal != value, signal > value, etc.\n",
        );
        output.push_str("            let parts: Vec<&str> = expr.split_whitespace().collect();\n");
        output.push_str("            if parts.len() >= 3 {\n");
        output.push_str("                let signal_name = parts[0];\n");
        output.push_str("                let op = parts[1];\n");
        output.push_str("                let value_str = parts[2];\n");
        output.push_str("                if let Some(&sig_id) = signal_map.get(signal_name) {\n");
        output.push_str("                    let sig_val = &signals[sig_id];\n");
        output.push_str("                    if let Some(sig_num) = sig_val.as_f64() {\n");
        output.push_str(
            "                        let compare_val: f64 = value_str.parse().unwrap_or(0.0);\n",
        );
        output.push_str("                        match op {\n");
        output.push_str("                            \"==\" => return sig_num == compare_val,\n");
        output.push_str("                            \"!=\" => return sig_num != compare_val,\n");
        output.push_str("                            \">\" => return sig_num > compare_val,\n");
        output.push_str("                            \"<\" => return sig_num < compare_val,\n");
        output.push_str("                            \">=\" => return sig_num >= compare_val,\n");
        output.push_str("                            \"<=\" => return sig_num <= compare_val,\n");
        output.push_str("                            _ => {}\n");
        output.push_str("                        }\n");
        output.push_str("                    }\n");
        output.push_str("                }\n");
        output.push_str("            }\n");
        output.push_str("            true // default to visible if can't evaluate\n");
        output.push_str("        };\n");
        output.push_str("        let mut signal_map = std::collections::HashMap::new();\n");
        for (name, &id) in &self.signal_map {
            output.push_str(&format!(
                "        signal_map.insert(\"{}\", {});\n",
                name, id
            ));
        }

        for binding in bindings {
            if let Directive::Text { signal } = &binding.directive {
                if let Some(&sig_id) = self.signal_map.get(signal) {
                    output.push_str(&format!("        if self.dirty_signals[{}] {{\n", sig_id));
                    output.push_str(&format!(
                        "            let val = self.signals[{}].clone();\n",
                        sig_id
                    ));
                    output.push_str("            let json = json_text(&format!(\"{}\", \"");
                    output.push_str(&binding.element_id);
                    output.push_str("\"), val);\n");
                    output.push_str("            parts.push(json);\n");
                    output.push_str("        }\n");
                }
            } else if let Directive::Each { iterable, .. } = &binding.directive {
                if let Some(&sig_id) = self.signal_map.get(iterable) {
                    output.push_str(&format!("        if self.dirty_signals[{}] {{\n", sig_id));
                    output.push_str(&format!(
                        "            parts.push(format!(\"{{{{\\\"op\\\":\\\"each\\\",\\\"iterable\\\":\\\"{}\\\",\\\"el\\\":\\\"{}\\\"}}}}\"));\n",
                        iterable, binding.element_id
                    ));
                    output.push_str("        }\n");
                }
            }
        }

        // Emit show instructions for all show bindings (check visibility)
        output.push_str("        for (el_id, expr, prev_visible) in &mut self.show_bindings {\n");
        output.push_str("            let visible = eval_show(&self.signals, &signal_map, expr);\n");
        output.push_str("            if visible != *prev_visible {\n");
        output.push_str("                *prev_visible = visible;\n");
        output.push_str("                parts.push(format!(\"{{\\\"op\\\":\\\"show\\\",\\\"el\\\":\\\"{}\\\",\\\"visible\\\":{}}}\", el_id, visible));\n");
        output.push_str("            }\n");
        output.push_str("        }\n");

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
        // Add wasm_bindgen attribute to export to JavaScript
        output.push_str("    #[wasm_bindgen]\n");

        let method_name = format!(
            "    pub fn invoke_{}(&mut self) {{\n",
            txn.name.replace(".", "_")
        );
        output.push_str(&method_name);

        output.push_str("        // Precondition\n");
        let pre_code = self.expr_to_js_value_for_condition(&txn.contract.pre_condition);
        output.push_str(&format!("        if !({}) {{\n", pre_code));
        output.push_str("            return;\n");
        output.push_str("        }\n\n");

        output.push_str("        // Save prior state\n");
        for (name, &id) in &self.signal_map {
            output.push_str(&format!(
                "        let prior_{} = self.signals[{}].clone();\n",
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
        let post_code = self.expr_to_js_value_for_condition(&txn.contract.post_condition);
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

        if txn.name.contains('.') {
            let short_name = txn.name.split('.').last().unwrap_or(&txn.name);
            output.push_str("    #[wasm_bindgen]\n");
            let alias = format!("    pub fn invoke_{}(&mut self) {{\n", short_name);
            output.push_str(&alias);
            output.push_str(&format!(
                "        self.invoke_{}();\n",
                txn.name.replace(".", "_")
            ));
            output.push_str("    }\n\n");
        }
    }

    fn statement_to_rust(&self, output: &mut String, stmt: &Statement) {
        match stmt {
            Statement::Assignment {
                is_owned,
                name,
                expr,
            } => {
                if *is_owned {
                    if let Expr::Add(a, b) = expr {
                        let a_is_list = self.is_list_signal(a);
                        let b_is_list = self.is_list_signal(b);
                        if a_is_list || b_is_list {
                            if let Expr::Identifier(name) = a.as_ref() {
                                if let Some(&id) = self.signal_map.get(name) {
                                    let other_code = self.expr_to_js_value(b);
                                    let other_arg = if matches!(b.as_ref(), Expr::ListLiteral(_)) {
                                        format!("{}.into()", other_code)
                                    } else {
                                        other_code
                                    };
                                    output.push_str(&format!(
                                        "        self.signals[{}] = self.list_concat({}, {});\n",
                                        id, id, other_arg
                                    ));
                                    output.push_str(&format!("        self.mark_dirty({});\n", id));
                                    return;
                                }
                            }
                        }
                    }
                    let expr_code = self.expr_to_js_value(expr);
                    if let Some(&id) = self.signal_map.get(name) {
                        output.push_str(&format!(
                            "        self.signals[{}] = {}.into();\n",
                            id, expr_code
                        ));
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

    fn is_list_signal(&self, expr: &Expr) -> bool {
        match expr {
            Expr::ListLiteral(_) => true,
            Expr::Identifier(name) => {
                if let Some(sig_type) = self.signal_types.get(name) {
                    matches!(sig_type, SignalType::List)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn expr_to_js_value(&self, expr: &Expr) -> String {
        match expr {
            Expr::Integer(n) => format!("JsValue::from({})", n),
            Expr::Bool(true) => "JsValue::TRUE".to_string(),
            Expr::Bool(false) => "JsValue::FALSE".to_string(),
            Expr::String(s) => format!("JsValue::from(\"{}\")", s),
            Expr::Identifier(name) => {
                if let Some(&id) = self.signal_map.get(name) {
                    format!("self.signals[{}].clone()", id)
                } else {
                    format!("JsValue::from(\"{}\")", name)
                }
            }
            Expr::PriorState(name) => {
                if let Some(&id) = self.signal_map.get(name) {
                    format!("prior_{}.clone()", name)
                } else {
                    format!("JsValue::from(\"{}\")", name)
                }
            }
            Expr::Add(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                let a_is_list = self.is_list_signal(a);
                let b_is_list = self.is_list_signal(b);
                if a_is_list || b_is_list {
                    let arr_a = if a_is_list {
                        format!("{}.clone()", a_val)
                    } else {
                        format!("js_sys::Array::new()")
                    };
                    let arr_b = if b_is_list {
                        format!("{}.clone()", b_val)
                    } else {
                        format!("js_sys::Array::new()")
                    };
                    format!("{{ let mut __arr = js_sys::Array::new(); for __i in 0..{}.length() {{ __arr.push(&{}.get(__i)); }} for __i in 0..{}.length() {{ __arr.push(&{}.get(__i)); }} __arr.into() }}", arr_a, arr_a, arr_b, arr_b)
                } else {
                    format!(
                        "JsValue::from({}.as_f64().unwrap_or(0.0) + {}.as_f64().unwrap_or(0.0))",
                        a_val, b_val
                    )
                }
            }
            Expr::Sub(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "JsValue::from({}.as_f64().unwrap_or(0.0) - {}.as_f64().unwrap_or(0.0))",
                    a_val, b_val
                )
            }
            Expr::Mul(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "JsValue::from({}.as_f64().unwrap_or(0.0) * {}.as_f64().unwrap_or(0.0))",
                    a_val, b_val
                )
            }
            Expr::Div(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "JsValue::from({}.as_f64().unwrap_or(0.0) / {}.as_f64().unwrap_or(0.0))",
                    a_val, b_val
                )
            }
            Expr::Eq(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "{}.as_f64().unwrap_or(0.0) == {}.as_f64().unwrap_or(0.0)",
                    a_val, b_val
                )
            }
            Expr::Ne(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "{}.as_f64().unwrap_or(0.0) != {}.as_f64().unwrap_or(0.0)",
                    a_val, b_val
                )
            }
            Expr::Lt(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "{}.as_f64().unwrap_or(0.0) < {}.as_f64().unwrap_or(0.0)",
                    a_val, b_val
                )
            }
            Expr::Le(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "{}.as_f64().unwrap_or(0.0) <= {}.as_f64().unwrap_or(0.0)",
                    a_val, b_val
                )
            }
            Expr::Gt(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "{}.as_f64().unwrap_or(0.0) > {}.as_f64().unwrap_or(0.0)",
                    a_val, b_val
                )
            }
            Expr::Ge(a, b) => {
                let a_val = self.expr_to_js_value(a);
                let b_val = self.expr_to_js_value(b);
                format!(
                    "{}.as_f64().unwrap_or(0.0) >= {}.as_f64().unwrap_or(0.0)",
                    a_val, b_val
                )
            }
            Expr::And(a, b) => {
                let a_val = self.expr_to_js_value_for_condition(a);
                let b_val = self.expr_to_js_value_for_condition(b);
                format!("({} && {})", a_val, b_val)
            }
            Expr::Or(a, b) => {
                let a_val = self.expr_to_js_value_for_condition(a);
                let b_val = self.expr_to_js_value_for_condition(b);
                format!("({} || {})", a_val, b_val)
            }
            Expr::Not(a) => {
                let a_val = self.expr_to_js_value_for_condition(a);
                format!("(!{})", a_val)
            }
            Expr::Neg(a) => {
                let a_val = self.expr_to_js_value(a);
                format!("-{}.as_f64().unwrap_or(0.0)", a_val)
            }
            Expr::ListLiteral(elements) => {
                let items: Vec<String> =
                    elements.iter().map(|e| self.expr_to_js_value(e)).collect();
                if items.is_empty() {
                    "js_sys::Array::new()".to_string()
                } else {
                    let mut arr = String::from("{ let __arr = js_sys::Array::new(); ");
                    for item in &items {
                        arr.push_str(&format!("__arr.push(&{}); ", item));
                    }
                    arr.push_str("__arr }");
                    arr
                }
            }
            Expr::ListIndex(list_expr, index_expr) => {
                let list_val = self.expr_to_js_value(list_expr);
                let index_val = self.expr_to_js_value(index_expr);
                format!("(if {}.is_array() {{ js_sys::Array::from(&{}).get({}) }} else {{ JsValue::NULL }})", list_val, list_val, index_val)
            }
            Expr::ListLen(list_expr) => {
                let list_val = self.expr_to_js_value(list_expr);
                format!(
                    "(if {}.is_array() {{ js_sys::Array::from(&{}).length() }} else {{ 0.0 }})",
                    list_val, list_val
                )
            }
            Expr::FieldAccess(obj_expr, field_name) => {
                let obj_val = self.expr_to_js_value(obj_expr);
                format!("{}.{}", obj_val, field_name)
            }
            Expr::Call(name, args) if name == "len" && args.len() == 1 => {
                let list_val = self.expr_to_js_value(&args[0]);
                format!(
                    "JsValue::from(if {}.is_array() {{ js_sys::Array::from(&{}).length() as f64 }} else {{ 0.0 }})",
                    list_val, list_val
                )
            }
            Expr::Call(name, args) => {
                let args_vals: Vec<String> =
                    args.iter().map(|a| self.expr_to_js_value(a)).collect();
                format!("{}({})", name, args_vals.join(", "))
            }
            Expr::StructInstance(typename, fields) => {
                let mut sets = String::new();
                for (field_name, field_value) in fields {
                    let value_js = self.expr_to_js_value(field_value);
                    sets.push_str(&format!(
                        r#"js_sys::Reflect::set(&__obj, &JsValue::from("{}"), &{}).ok(); "#,
                        field_name, value_js
                    ));
                }
                format!(
                    "JsValue::from({{ let mut __obj = js_sys::Object::new(); {} __obj }})",
                    sets
                )
            }
            Expr::ObjectLiteral(fields) => {
                let mut sets = String::new();
                for (field_name, field_value) in fields {
                    let value_js = self.expr_to_js_value(field_value);
                    sets.push_str(&format!(
                        r#"js_sys::Reflect::set(&__obj, &JsValue::from("{}"), &{}).ok(); "#,
                        field_name, value_js
                    ));
                }
                format!(
                    "JsValue::from({{ let mut __obj = js_sys::Object::new(); {} __obj }})",
                    sets
                )
            }
            _ => "JsValue::TRUE".to_string(),
        }
    }

    fn expr_to_js_value_for_condition(&self, expr: &Expr) -> String {
        match expr {
            Expr::Bool(true) => "true".to_string(),
            Expr::Bool(false) => "false".to_string(),
            Expr::Not(inner) => format!("!{}", self.expr_to_js_value_for_condition(inner)),
            Expr::And(a, b) => {
                let a_val = self.expr_to_js_value_for_condition(a);
                let b_val = self.expr_to_js_value_for_condition(b);
                format!("({} && {})", a_val, b_val)
            }
            Expr::Or(a, b) => {
                let a_val = self.expr_to_js_value_for_condition(a);
                let b_val = self.expr_to_js_value_for_condition(b);
                format!("({} || {})", a_val, b_val)
            }
            Expr::Eq(a, b) => {
                let a_val = self.js_value_to_f64(a);
                let b_val = self.js_value_to_f64(b);
                format!("({} == {})", a_val, b_val)
            }
            Expr::Ne(a, b) => {
                let a_val = self.js_value_to_f64(a);
                let b_val = self.js_value_to_f64(b);
                format!("({} != {})", a_val, b_val)
            }
            Expr::Lt(a, b) => {
                let a_val = self.js_value_to_f64(a);
                let b_val = self.js_value_to_f64(b);
                format!("({} < {})", a_val, b_val)
            }
            Expr::Le(a, b) => {
                let a_val = self.js_value_to_f64(a);
                let b_val = self.js_value_to_f64(b);
                format!("({} <= {})", a_val, b_val)
            }
            Expr::Gt(a, b) => {
                let a_val = self.js_value_to_f64(a);
                let b_val = self.js_value_to_f64(b);
                format!("({} > {})", a_val, b_val)
            }
            Expr::Ge(a, b) => {
                let a_val = self.js_value_to_f64(a);
                let b_val = self.js_value_to_f64(b);
                format!("({} >= {})", a_val, b_val)
            }
            Expr::ListLen(list_expr) => {
                let list_val = self.expr_to_js_value(list_expr);
                format!(
                    "JsValue::from(if {}.is_array() {{ js_sys::Array::from(&{}).length() as f64 }} else {{ 0.0 }})",
                    list_val, list_val
                )
            }
            _ => {
                let val = self.js_value_to_f64(expr);
                format!("({} != 0.0)", val)
            }
        }
    }

    fn js_value_to_f64(&self, expr: &Expr) -> String {
        match expr {
            Expr::ListLen(list_expr) => self.expr_to_js_value(expr),
            Expr::Call(name, args) if name == "len" && args.len() == 1 => {
                self.expr_to_js_value(expr)
            }
            _ => {
                let val = self.expr_to_js_value(expr);
                format!("{}.as_f64().unwrap_or(0.0)", val)
            }
        }
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

        output.push_str(&format!("    console.log('Loading WASM module...');\n"));
        output.push_str(&format!(
            "    const wasm_pkg = await import('./pkg/{}.js');\n",
            program_name
        ));
        output.push_str("    console.log('WASM module loaded, initializing...');\n");
        output.push_str("    await wasm_pkg.default();\n");
        output.push_str("    console.log('WASM initialized, creating State...');\n");
        output.push_str("    const wasm = new wasm_pkg.State();\n");
        output.push_str("    console.log('State created, methods available:', Object.keys(wasm).filter(k => k.startsWith('invoke')));\n\n");

        output.push_str("    const TRIGGER_MAP = {\n");
        for binding in bindings {
            if let Directive::Trigger { event, txn } = &binding.directive {
                // Transform transaction name to invoke method name
                // Use short name to match the generated alias
                // e.g., "Counter.tick" -> "invoke_tick" (alias to invoke_Counter_tick)
                // e.g., "add" -> "invoke_add"
                let short_name = if txn.contains('.') {
                    txn.split('.').last().unwrap_or(txn)
                } else {
                    txn
                };
                let invoke_method = format!("invoke_{}", short_name);
                output.push_str(&format!(
                    "        '{}': {{ event: '{}', txn: '{}' }},\n",
                    binding.element_id, event, invoke_method
                ));
            }
        }
        output.push_str("    };\n\n");

        let mut each_configs: Vec<(String, String, String)> = Vec::new();
        for binding in bindings {
            if let Directive::Each {
                iterable,
                container_id,
                ..
            } = &binding.directive
            {
                each_configs.push((
                    binding.element_id.clone(),
                    iterable.clone(),
                    container_id.clone(),
                ));
            }
        }

        if !each_configs.is_empty() {
            output.push_str("    function renderEach(iterable, containerSelector) {\n");
            output
                .push_str("        const container = document.querySelector(containerSelector);\n");
            output.push_str("        if (!container) return;\n");
            output.push_str("        const html = wasm.render_each(iterable);\n");
            output.push_str("        container.innerHTML = html;\n");
            output.push_str("    }\n\n");

            output.push_str("    function attachEachListeners() {\n");
            for (_elem_id, iterable, container_id) in &each_configs {
                output.push_str(&format!(
                    "        renderEach('{}', '#{}');\n",
                    iterable, container_id
                ));
            }
            output.push_str("    }\n\n");
        }

        output.push_str("    function attachListeners() {\n");
        output.push_str("        console.log('Attaching event listeners...');\n");
        output.push_str("        for (const [elId, config] of Object.entries(TRIGGER_MAP)) {\n");
        output.push_str("            const el = document.querySelector(ELEMENT_MAP[elId]);\n");
        output.push_str("            if (!el) {\n");
        output.push_str("                console.warn('Element not found:', elId);\n");
        output.push_str("                continue;\n");
        output.push_str("            }\n");
        output.push_str("            console.log('Attaching', config.event, 'handler to', elId, '->', config.txn);\n");
        output.push_str("            el.addEventListener(config.event, () => {\n");
        output.push_str("                console.log('Trigger clicked:', config.txn, 'typeof:', typeof wasm[config.txn]);\n");
        output.push_str("                try {\n");
        output.push_str("                    wasm[config.txn]();\n");
        output.push_str("                } catch(e) {\n");
        output
            .push_str("                    console.error('Error calling', config.txn, ':', e);\n");
        output.push_str("                }\n");
        output.push_str("            });\n");
        output.push_str("        }\n");
        output.push_str("        console.log('All listeners attached');\n");
        output.push_str("    }\n\n");

        output.push_str("    function startPollLoop() {\n");
        output.push_str("        function poll() {\n");
        output.push_str("            const dispatch = wasm.poll_dispatch();\n");
        output.push_str("            console.log('Poll loop, dispatch:', dispatch);\n");
        output.push_str("            if (dispatch && dispatch !== '[]') {\n");
        output.push_str("                console.log('Applying instructions:', dispatch);\n");
        output.push_str("                applyInstructions(JSON.parse(dispatch));\n");
        output.push_str("            }\n");
        output.push_str("            requestAnimationFrame(poll);\n");
        output.push_str("        }\n");
        output.push_str("        console.log('Starting poll loop');\n");
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
        output.push_str("                case 'each':\n");
        output.push_str("                    renderEach(inst.iterable, '#' + inst.el);\n");
        output.push_str("                    break;\n");
        output.push_str("            }\n");
        output.push_str("        }\n");
        output.push_str("    }\n\n");

        if !each_configs.is_empty() {
            output.push_str("    attachEachListeners();\n");
        }
        output.push_str("    attachListeners();\n");
        output.push_str("    startPollLoop();\n");
        output.push_str("})();\n");

        output
    }

    fn escape_selector(&self, id: &str) -> String {
        if id.starts_with("rbv-") || id.starts_with('#') {
            format!("#{}", id.trim_start_matches('#'))
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
