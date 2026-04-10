use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
pub struct SignalGraph {
    /// Maps signal name -> Set of transaction names that depend on it
    subscribers: HashMap<String, HashSet<String>>,
    /// Current state of signals
    values: HashMap<String, JsValue>,
}

impl SignalGraph {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            values: HashMap::new(),
        }
    }

    pub fn subscribe(&mut self, signal: &str, txn_name: &str) {
        self.subscribers
            .entry(signal.to_string())
            .or_default()
            .insert(txn_name.to_string());
    }

    pub fn update_signal(&mut self, signal: &str, value: JsValue) -> Vec<String> {
        self.values.insert(signal.to_string(), value);
        self.subscribers
            .get(signal)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get_value(&self, signal: &str) -> Option<&JsValue> {
        self.values.get(signal)
    }

    pub fn clear_subscribers(&mut self) {
        self.subscribers.clear();
    }
}

impl Default for SignalGraph {
    fn default() -> Self {
        Self::new()
    }
}
