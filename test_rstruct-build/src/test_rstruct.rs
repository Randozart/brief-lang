use wasm_bindgen::prelude::*;

const SIGNALS: usize = 1;

#[wasm_bindgen]
pub struct State {
    signals: Vec<JsValue>,
    dirty_signals: Vec<bool>,
    each_templates: Vec<(String, String, String)>,
    show_bindings: Vec<(String, String, bool)>,
}

#[wasm_bindgen]
impl State {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let signals = vec![
            JsValue::from(0).into(), // signal 0
        ];
        let dirty_signals = vec![false; SIGNALS];
        let mut each_templates: Vec<(String, String, String)> = Vec::new();
        let mut show_bindings: Vec<(String, String, bool)> = Vec::new();
        State { signals, dirty_signals, each_templates, show_bindings }
    }

    pub fn get_signal(&self, id: usize) -> JsValue {
        self.signals[id].clone()
    }

    fn html_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\"', "&quot;")
    }

    pub fn render_each(&self, iterable: &str) -> String {
        for (iter_name, item_name, template) in &self.each_templates {
            if iter_name == iterable {
                if iterable == "count" {
                    let list = &self.signals[0];
                    if list.is_array() {
                        let arr = js_sys::Array::from(list);
                        let mut result = String::new();
                        for i in 0..arr.length() {
                            let item = arr.get(i);
                            let item_str = if item.is_string() {
                                item.as_string().unwrap_or_default()
                            } else {
                                format!("{:?}", item)
                            };
                            let escaped = Self::html_escape(&item_str);
                            let mut html = template.clone();
                            let search = format!("b-text=\"{}\">", item_name);
                            if let Some(pos) = html.find(&search) {
                                let after = &html[pos + search.len()..];
                                if let Some(end) = after.find('<') {
                                    let before = &html[..pos];
                                    let rest = &after[end..];
                                    html = format!("{}>{}{}", before, escaped, rest);
                                }
                            }
                            result.push_str(&html);
                        }
                        return result;
                    }
                }
            }
        }
        String::new()
    }

    fn signal_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        map.insert("count".to_string(), 0);
        map
    }

    fn mark_dirty(&mut self, id: usize) {
        if id < SIGNALS {
            self.dirty_signals[id] = true;
        }
    }

    fn list_concat(&self, signal_id: usize, other: JsValue) -> JsValue {
        let current = self.signals[signal_id].clone();
        let arr = js_sys::Array::new();
        if current.is_array() {
            let curr_arr = js_sys::Array::from(&current);
            for i in 0..curr_arr.length() {
                arr.push(&curr_arr.get(i));
            }
        }
        if other.is_array() {
            let other_arr = js_sys::Array::from(&other);
            for i in 0..other_arr.length() {
                arr.push(&other_arr.get(i));
            }
        }
        arr.into()
    }

    pub fn get_count(&self) -> i32 {
        self.signals[0].as_f64().unwrap_or(0.0) as i32
    }

    pub fn set_count(&mut self, value: i32) {
        self.signals[0] = JsValue::from(value);
        self.mark_dirty(0);
    }

    pub fn poll_dispatch(&mut self) -> JsValue {
        let mut parts: Vec<String> = vec![];
        fn json_text(el: &str, val: JsValue) -> String {
            if let Some(n) = val.as_f64() {
                format!("{{\"op\":\"text\",\"el\":\"{}\",\"value\":{}}}", el, n as i32)
            } else {
                format!("{{\"op\":\"text\",\"el\":\"{}\",\"value\":0}}", el)
            }
        }
        let eval_show = |signals: &[JsValue], signal_map: &std::collections::HashMap<&str, usize>, expr: &str| -> bool {
            // Simple expression evaluator for show conditions
            // Handles: signal == value, signal != value, signal > value, etc.
            let parts: Vec<&str> = expr.split_whitespace().collect();
            if parts.len() >= 3 {
                let signal_name = parts[0];
                let op = parts[1];
                let value_str = parts[2];
                if let Some(&sig_id) = signal_map.get(signal_name) {
                    let sig_val = &signals[sig_id];
                    if let Some(sig_num) = sig_val.as_f64() {
                        let compare_val: f64 = value_str.parse().unwrap_or(0.0);
                        match op {
                            "==" => return sig_num == compare_val,
                            "!=" => return sig_num != compare_val,
                            ">" => return sig_num > compare_val,
                            "<" => return sig_num < compare_val,
                            ">=" => return sig_num >= compare_val,
                            "<=" => return sig_num <= compare_val,
                            _ => {}
                        }
                    }
                }
            }
            true // default to visible if can't evaluate
        };
        let mut signal_map = std::collections::HashMap::new();
        signal_map.insert("count", 0);
        for (el_id, expr, prev_visible) in &mut self.show_bindings {
            let visible = eval_show(&self.signals, &signal_map, expr);
            if visible != *prev_visible {
                *prev_visible = visible;
                parts.push(format!("{{\"op\":\"show\",\"el\":\"{}\",\"visible\":{}}}", el_id, visible));
            }
        }
        self.dirty_signals.fill(false);
        let result = format!("[{}]", parts.join(","));
        result.into()
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}
