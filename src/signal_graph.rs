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
