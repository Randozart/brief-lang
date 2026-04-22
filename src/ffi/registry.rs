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

//! FFI Function Registry
//!
//! Manages runtime registration of foreign function implementations.
//! TOML-driven: loads bindings from std/bindings/*.toml and maps locations to implementations.

use crate::ffi::loader;
use crate::interpreter::{ForeignFn, RuntimeError, Value};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;

/// Global FFI function registry
pub static FFI_REGISTRY: Lazy<FunctionRegistry> = Lazy::new(|| {
    let mut registry = FunctionRegistry::new();
    registry.load_from_bindings_dir();
    registry
});

/// Function implementation registry
/// Maps function locations (from TOML location field) to implementations
pub struct FunctionRegistry {
    functions: HashMap<String, ForeignFn>,
    syscall_numbers: HashMap<String, HashMap<String, i64>>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        FunctionRegistry {
            functions: HashMap::new(),
            syscall_numbers: HashMap::new(),
        }
    }

    pub fn register_syscall_numbers(&mut self, target: String, numbers: HashMap<String, i64>) {
        self.syscall_numbers.insert(target, numbers);
    }

    pub fn get_syscall_number(&self, target: &str, name: &str) -> Option<i64> {
        self.syscall_numbers.get(target).and_then(|m| m.get(name).copied())
    }

    pub fn load_syscall_bindings(&mut self) -> Result<(), String> {
        let binding_dir = std::env::var("BRIEF_STDLIB_PATH")
            .map(|p| PathBuf::from(p).join("syscalls"))
            .unwrap_or_else(|_| PathBuf::from("std/bindings/syscalls"));

        if binding_dir.exists() {
            for entry in std::fs::read_dir(binding_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "toml") {
                    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    let toml: toml::Value = toml::from_str(&content).map_err(|e| e.to_string())?;
                    if let Some(syscalls) = toml.get("syscalls").and_then(|v| v.as_array()) {
                        for syscall in syscalls {
                            if let Some(name) = syscall.get("name").and_then(|v| v.as_str()) {
                                let mut numbers = HashMap::new();
                                if let Some(num_map) = syscall.get("syscall_num").and_then(|v| v.as_table()) {
                                    for (target, num) in num_map {
                                        if let Some(n) = num.as_integer() {
                                            numbers.insert(target.clone(), n);
                                        }
                                    }
                                }
                                self.syscall_numbers.insert(name.to_string(), numbers);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn register(&mut self, location: String, func: ForeignFn) {
        self.functions.insert(location, func);
    }

    pub fn get(&self, location: &str) -> Option<ForeignFn> {
        self.functions.get(location).copied()
    }

    pub fn contains(&self, location: &str) -> bool {
        self.functions.contains_key(location)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ForeignFn)> {
        self.functions.iter()
    }

    /// Load all bindings from std/bindings/*.toml
    pub fn load_from_bindings_dir(&mut self) {
        let bindings_dir = Self::bindings_dir();

        if let Ok(entries) = std::fs::read_dir(&bindings_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    if let Err(e) = self.load_from_toml(&path) {
                        eprintln!("[WARN] Failed to load binding {}: {}", path.display(), e);
                    }
                }
            }
        }

        eprintln!(
            "[INFO] FFI Registry loaded {} functions from TOML",
            self.functions.len()
        );
    }

    /// Load bindings from a single TOML file
    fn load_from_toml(&mut self, path: &std::path::Path) -> Result<(), String> {
        let bindings =
            loader::load_binding(path).map_err(|e| format!("Failed to parse TOML: {}", e))?;

        for binding in bindings {
            if let Some(func) = resolve_location_to_impl(&binding.location) {
                self.register(binding.location, func);
            } else {
                eprintln!(
                    "[WARN] No implementation for location '{}' in {}",
                    binding.location,
                    path.display()
                );
            }
        }

        Ok(())
    }

    /// Get the bindings directory path
    fn bindings_dir() -> PathBuf {
        let exe_path = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe_path.parent().unwrap_or(std::path::Path::new("."));

        // Try relative to executable first, then crate root
        let relative_path = exe_dir.join("std/bindings");
        if relative_path.exists() {
            return relative_path;
        }

        // Fallback to crate root (for development)
        std::path::PathBuf::from("std/bindings")
    }

    pub fn register_from_binding(&mut self, location: &str, func: ForeignFn) {
        self.register(location.to_string(), func);
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.load_from_bindings_dir();
        registry
    }
}

/// Resolve a TOML location string to an actual function implementation
fn resolve_location_to_impl(location: &str) -> Option<ForeignFn> {
    let func: fn(Vec<Value>) -> Result<Value, RuntimeError> = match location {
        // IO functions
        "std::io::print" => print_impl,
        "std::io::println" => println_impl,
        "std::io::input" => input_impl,

        // Math functions
        "std::f64::sqrt" => sqrt_impl,
        "std::f64::powf" => pow_impl,
        "std::f64::sin" => sin_impl,
        "std::f64::cos" => cos_impl,
        "std::f64::abs" => abs_impl,
        "std::f64::floor" => floor_impl,
        "std::f64::ceil" => ceil_impl,
        "std::f64::round" => round_impl,

        // String functions
        "std::string::String::len" => len_impl,
        "std::string::String::push_str" => concat_impl,
        "std::string::String::trim" => trim_impl,
        "std::string::String::contains" => contains_impl,
        "std::string::String::to_lowercase" => to_lower_impl,
        "std::string::String::to_uppercase" => to_upper_impl,
        "std::string::String::replace" => replace_impl,
        "std::string::String::chars" => chars_impl,
        "std::string::String::starts_with" => starts_with_impl,
        "std::string::String::ends_with" => ends_with_impl,
        "std::str::FromStr::from_str" => from_str_impl,
        "std::string::ToString::to_string" => to_string_impl,

        // Time functions
        "std::time::SystemTime::now" => now_impl,

        // File system (simplified - these return void on success)
        "std::fs::read_to_string" => read_file_impl,
        "std::fs::write" => write_file_impl,
        "std::fs::remove_file" => delete_file_impl,
        "std::fs::create_dir" => create_dir_impl,
        "std::fs::remove_dir" => delete_dir_impl,

        _ => {
            eprintln!("[DEBUG] Unresolved location: {}", location);
            return None;
        }
    };
    Some(func)
}

// Re-export implementations from interpreter
use crate::interpreter;

// IO implementations
fn print_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::print_impl(args)
}
fn println_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::println_impl(args)
}
fn input_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::input_impl(args)
}

// Math implementations
fn abs_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::abs_impl(args)
}
fn sqrt_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::sqrt_impl(args)
}
fn pow_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::pow_impl(args)
}
fn sin_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::sin_impl(args)
}
fn cos_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::cos_impl(args)
}
fn floor_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::floor_impl(args)
}
fn ceil_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::ceil_impl(args)
}
fn round_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::round_impl(args)
}

// String implementations
fn len_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::len_impl(args)
}
fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::concat_impl(args)
}
fn trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::trim_impl(args)
}
fn contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::contains_impl(args)
}
fn to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_string_impl(args)
}
fn to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_lower_impl(args)
}
fn to_upper_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_upper_impl(args)
}
fn replace_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::replace_impl(args)
}
fn chars_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::chars_impl(args)
}
fn starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::starts_with_impl(args)
}
fn ends_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::ends_with_impl(args)
}
fn from_str_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::from_str_impl(args)
}

// Time implementation
fn now_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::now_impl(args)
}

// File system implementations
fn read_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::read_file_impl(args)
}
fn write_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::write_file_impl(args)
}
fn delete_file_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::delete_file_impl(args)
}
fn create_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::create_dir_impl(args)
}
fn delete_dir_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::delete_dir_impl(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basics() {
        let registry = FunctionRegistry::new();
        assert!(!registry.contains("test"));
    }

    #[test]
    fn test_toml_loading() {
        let mut registry = FunctionRegistry::new();
        registry.load_from_bindings_dir();

        // Should have loaded functions from TOML
        assert!(registry.contains("std::io::println"), "println not loaded");
        assert!(registry.contains("std::f64::sqrt"), "sqrt not loaded");
    }

    #[test]
    fn test_location_resolution() {
        assert!(resolve_location_to_impl("std::f64::sqrt").is_some());
        assert!(resolve_location_to_impl("std::io::println").is_some());
        assert!(resolve_location_to_impl("unknown::function").is_none());
    }
}
