use wasm_bindgen::prelude::*;

const SIGNALS: usize = 4;

#[wasm_bindgen]
pub struct State {
    signals: Vec<JsValue>,
    dirty_signals: Vec<bool>,
    each_templates: Vec<(String, String, String)>,
}

#[wasm_bindgen]
impl State {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let signals = vec![
            JsValue::from(0).into(), // signal 0
            JsValue::from(0).into(), // signal 1
            JsValue::from(0).into(), // signal 2
            JsValue::from(0).into(), // signal 3
        ];
        let dirty_signals = vec![false; SIGNALS];
        let mut each_templates: Vec<(String, String, String)> = Vec::new();
        State { signals, dirty_signals, each_templates }
    }

    pub fn get_signal(&self, id: usize) -> JsValue {
        self.signals[id].clone()
    }

    fn html_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    pub fn render_each(&self, iterable: &str) -> String {
        for (iter_name, item_name, template) in &self.each_templates {
            if iter_name == iterable {
                if iterable == "step" {
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
                if iterable == "total" {
                    let list = &self.signals[1];
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
                if iterable == "items" {
                    let list = &self.signals[2];
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
                if iterable == "product" {
                    let list = &self.signals[3];
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
        map.insert("step".to_string(), 0);
        map.insert("total".to_string(), 1);
        map.insert("items".to_string(), 2);
        map.insert("product".to_string(), 3);
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

    pub fn get_step(&self) -> i32 {
        self.signals[0].as_f64().unwrap_or(0.0) as i32
    }

    pub fn set_step(&mut self, value: i32) {
        self.signals[0] = JsValue::from(value);
        self.mark_dirty(0);
    }

    pub fn get_product(&self) -> i32 {
        self.signals[3].as_f64().unwrap_or(0.0) as i32
    }

    pub fn set_product(&mut self, value: i32) {
        self.signals[3] = JsValue::from(value);
        self.mark_dirty(3);
    }

    pub fn get_total(&self) -> i32 {
        self.signals[1].as_f64().unwrap_or(0.0) as i32
    }

    pub fn set_total(&mut self, value: i32) {
        self.signals[1] = JsValue::from(value);
        self.mark_dirty(1);
    }

    pub fn get_items(&self) -> i32 {
        self.signals[2].as_f64().unwrap_or(0.0) as i32
    }

    pub fn set_items(&mut self, value: i32) {
        self.signals[2] = JsValue::from(value);
        self.mark_dirty(2);
    }

    pub fn invoke_ShoppingCart_select_laptop(&mut self) {
        // Precondition
        if !(true) {
            return;
        }

        // Save prior state
        let prior_step = self.signals[0].clone();
        let prior_total = self.signals[1].clone();
        let prior_items = self.signals[2].clone();
        let prior_product = self.signals[3].clone();

        // Execute body
        self.signals[3] = JsValue::from(1).into();
        self.mark_dirty(3);
        // term - transaction settled

        // Postcondition
        if !((self.signals[3].clone().as_f64().unwrap_or(0.0) == JsValue::from(1).as_f64().unwrap_or(0.0))) {
            // Rollback
            self.signals[0] = prior_step;
            self.signals[1] = prior_total;
            self.signals[2] = prior_items;
            self.signals[3] = prior_product;
            return;
        }
    }

    pub fn invoke_select_laptop(&mut self) {
        self.invoke_ShoppingCart_select_laptop();
    }

    pub fn invoke_ShoppingCart_select_keyboard(&mut self) {
        // Precondition
        if !(true) {
            return;
        }

        // Save prior state
        let prior_step = self.signals[0].clone();
        let prior_total = self.signals[1].clone();
        let prior_items = self.signals[2].clone();
        let prior_product = self.signals[3].clone();

        // Execute body
        self.signals[3] = JsValue::from(2).into();
        self.mark_dirty(3);
        // term - transaction settled

        // Postcondition
        if !((self.signals[3].clone().as_f64().unwrap_or(0.0) == JsValue::from(2).as_f64().unwrap_or(0.0))) {
            // Rollback
            self.signals[0] = prior_step;
            self.signals[1] = prior_total;
            self.signals[2] = prior_items;
            self.signals[3] = prior_product;
            return;
        }
    }

    pub fn invoke_select_keyboard(&mut self) {
        self.invoke_ShoppingCart_select_keyboard();
    }

    pub fn invoke_ShoppingCart_select_mouse(&mut self) {
        // Precondition
        if !(true) {
            return;
        }

        // Save prior state
        let prior_step = self.signals[0].clone();
        let prior_total = self.signals[1].clone();
        let prior_items = self.signals[2].clone();
        let prior_product = self.signals[3].clone();

        // Execute body
        self.signals[3] = JsValue::from(3).into();
        self.mark_dirty(3);
        // term - transaction settled

        // Postcondition
        if !((self.signals[3].clone().as_f64().unwrap_or(0.0) == JsValue::from(3).as_f64().unwrap_or(0.0))) {
            // Rollback
            self.signals[0] = prior_step;
            self.signals[1] = prior_total;
            self.signals[2] = prior_items;
            self.signals[3] = prior_product;
            return;
        }
    }

    pub fn invoke_select_mouse(&mut self) {
        self.invoke_ShoppingCart_select_mouse();
    }

    pub fn invoke_ShoppingCart_select_monitor(&mut self) {
        // Precondition
        if !(true) {
            return;
        }

        // Save prior state
        let prior_step = self.signals[0].clone();
        let prior_total = self.signals[1].clone();
        let prior_items = self.signals[2].clone();
        let prior_product = self.signals[3].clone();

        // Execute body
        self.signals[3] = JsValue::from(4).into();
        self.mark_dirty(3);
        // term - transaction settled

        // Postcondition
        if !((self.signals[3].clone().as_f64().unwrap_or(0.0) == JsValue::from(4).as_f64().unwrap_or(0.0))) {
            // Rollback
            self.signals[0] = prior_step;
            self.signals[1] = prior_total;
            self.signals[2] = prior_items;
            self.signals[3] = prior_product;
            return;
        }
    }

    pub fn invoke_select_monitor(&mut self) {
        self.invoke_ShoppingCart_select_monitor();
    }

    pub fn invoke_ShoppingCart_add(&mut self) {
        // Precondition
        if !((self.signals[3].clone().as_f64().unwrap_or(0.0) > JsValue::from(0).as_f64().unwrap_or(0.0))) {
            return;
        }

        // Save prior state
        let prior_step = self.signals[0].clone();
        let prior_total = self.signals[1].clone();
        let prior_items = self.signals[2].clone();
        let prior_product = self.signals[3].clone();

        // Execute body
        self.signals[2] = JsValue::from(self.signals[2].clone().as_f64().unwrap_or(0.0) + JsValue::from(1).as_f64().unwrap_or(0.0)).into();
        self.mark_dirty(2);
        // term - transaction settled

        // Postcondition
        if !((self.signals[2].clone().as_f64().unwrap_or(0.0) == JsValue::from(prior_items.clone().as_f64().unwrap_or(0.0) + JsValue::from(1).as_f64().unwrap_or(0.0)).as_f64().unwrap_or(0.0))) {
            // Rollback
            self.signals[0] = prior_step;
            self.signals[1] = prior_total;
            self.signals[2] = prior_items;
            self.signals[3] = prior_product;
            return;
        }
    }

    pub fn invoke_add(&mut self) {
        self.invoke_ShoppingCart_add();
    }

    pub fn invoke_ShoppingCart_checkout(&mut self) {
        // Precondition
        if !(((self.signals[2].clone().as_f64().unwrap_or(0.0) > JsValue::from(0).as_f64().unwrap_or(0.0)) && (self.signals[0].clone().as_f64().unwrap_or(0.0) == JsValue::from(0).as_f64().unwrap_or(0.0)))) {
            return;
        }

        // Save prior state
        let prior_step = self.signals[0].clone();
        let prior_total = self.signals[1].clone();
        let prior_items = self.signals[2].clone();
        let prior_product = self.signals[3].clone();

        // Execute body
        self.signals[0] = JsValue::from(1).into();
        self.mark_dirty(0);
        // term - transaction settled

        // Postcondition
        if !((self.signals[0].clone().as_f64().unwrap_or(0.0) == JsValue::from(1).as_f64().unwrap_or(0.0))) {
            // Rollback
            self.signals[0] = prior_step;
            self.signals[1] = prior_total;
            self.signals[2] = prior_items;
            self.signals[3] = prior_product;
            return;
        }
    }

    pub fn invoke_checkout(&mut self) {
        self.invoke_ShoppingCart_checkout();
    }

    pub fn invoke_ShoppingCart_confirm(&mut self) {
        // Precondition
        if !((self.signals[0].clone().as_f64().unwrap_or(0.0) == JsValue::from(1).as_f64().unwrap_or(0.0))) {
            return;
        }

        // Save prior state
        let prior_step = self.signals[0].clone();
        let prior_total = self.signals[1].clone();
        let prior_items = self.signals[2].clone();
        let prior_product = self.signals[3].clone();

        // Execute body
        self.signals[0] = JsValue::from(2).into();
        self.mark_dirty(0);
        // term - transaction settled

        // Postcondition
        if !((self.signals[0].clone().as_f64().unwrap_or(0.0) == JsValue::from(2).as_f64().unwrap_or(0.0))) {
            // Rollback
            self.signals[0] = prior_step;
            self.signals[1] = prior_total;
            self.signals[2] = prior_items;
            self.signals[3] = prior_product;
            return;
        }
    }

    pub fn invoke_confirm(&mut self) {
        self.invoke_ShoppingCart_confirm();
    }

    pub fn invoke_ShoppingCart_reset(&mut self) {
        // Precondition
        if !((self.signals[0].clone().as_f64().unwrap_or(0.0) > JsValue::from(0).as_f64().unwrap_or(0.0))) {
            return;
        }

        // Save prior state
        let prior_step = self.signals[0].clone();
        let prior_total = self.signals[1].clone();
        let prior_items = self.signals[2].clone();
        let prior_product = self.signals[3].clone();

        // Execute body
        self.signals[0] = JsValue::from(0).into();
        self.mark_dirty(0);
        self.signals[2] = JsValue::from(0).into();
        self.mark_dirty(2);
        self.signals[1] = JsValue::from(0).into();
        self.mark_dirty(1);
        self.signals[3] = JsValue::from(0).into();
        self.mark_dirty(3);
        // term - transaction settled

        // Postcondition
        if !((self.signals[0].clone().as_f64().unwrap_or(0.0) == JsValue::from(0).as_f64().unwrap_or(0.0))) {
            // Rollback
            self.signals[0] = prior_step;
            self.signals[1] = prior_total;
            self.signals[2] = prior_items;
            self.signals[3] = prior_product;
            return;
        }
    }

    pub fn invoke_reset(&mut self) {
        self.invoke_ShoppingCart_reset();
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
        if self.dirty_signals[2] {
            let val = self.signals[2].clone();
            let json = json_text(&format!("{}", "rbv-span-0"), val);
            parts.push(json);
        }
        if self.dirty_signals[1] {
            let val = self.signals[1].clone();
            let json = json_text(&format!("{}", "rbv-span-1"), val);
            parts.push(json);
        }
        if self.dirty_signals[2] {
            let val = self.signals[2].clone();
            let json = json_text(&format!("{}", "rbv-span-18"), val);
            parts.push(json);
        }
        if self.dirty_signals[1] {
            let val = self.signals[1].clone();
            let json = json_text(&format!("{}", "rbv-span-19"), val);
            parts.push(json);
        }
        if self.dirty_signals[1] {
            let val = self.signals[1].clone();
            let json = json_text(&format!("{}", "rbv-span-22"), val);
            parts.push(json);
        }
        if self.dirty_signals[1] {
            let val = self.signals[1].clone();
            let json = json_text(&format!("{}", "rbv-span-26"), val);
            parts.push(json);
        }
        if self.dirty_signals[2] {
            let val = self.signals[2].clone();
            let json = json_text(&format!("{}", "rbv-span-27"), val);
            parts.push(json);
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
