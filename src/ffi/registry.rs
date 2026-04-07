//! FFI Function Registry
//!
//! Manages runtime registration of foreign function implementations.
//! Bridges the gap between TOML bindings and actual Rust function implementations.

use crate::interpreter::{ForeignFn, RuntimeError, Value};
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Global FFI function registry
pub static FFI_REGISTRY: Lazy<FunctionRegistry> = Lazy::new(|| {
    let mut registry = FunctionRegistry::new();
    registry.load_stdlib();
    registry
});

/// Function implementation registry
/// Maps function names (from TOML location field) to implementations
pub struct FunctionRegistry {
    functions: HashMap<String, ForeignFn>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        FunctionRegistry {
            functions: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: String, func: ForeignFn) {
        self.functions.insert(name, func);
    }

    pub fn get(&self, name: &str) -> Option<ForeignFn> {
        self.functions.get(name).copied()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    pub fn load_stdlib(&mut self) {
        self.register("std::io::print".to_string(), print_impl as ForeignFn);
        self.register("std::io::println".to_string(), println_impl as ForeignFn);
        self.register("std::io::input".to_string(), input_impl as ForeignFn);
        self.register("std::math::abs".to_string(), abs_impl as ForeignFn);
        self.register("std::math::sqrt".to_string(), sqrt_impl as ForeignFn);
        self.register("std::math::pow".to_string(), pow_impl as ForeignFn);
        self.register("std::math::sin".to_string(), sin_impl as ForeignFn);
        self.register("std::math::cos".to_string(), cos_impl as ForeignFn);
        self.register("std::math::floor".to_string(), floor_impl as ForeignFn);
        self.register("std::math::ceil".to_string(), ceil_impl as ForeignFn);
        self.register("std::math::round".to_string(), round_impl as ForeignFn);
        self.register("std::math::random".to_string(), random_impl as ForeignFn);
        self.register("std::string::len".to_string(), len_impl as ForeignFn);
        self.register("std::string::concat".to_string(), concat_impl as ForeignFn);
        self.register("std::string::to_string".to_string(), to_string_impl as ForeignFn);
        self.register("std::string::to_float".to_string(), to_float_impl as ForeignFn);
        self.register("std::string::to_int".to_string(), to_int_impl as ForeignFn);
        self.register("std::string::trim".to_string(), trim_impl as ForeignFn);
        self.register("std::string::contains".to_string(), contains_impl as ForeignFn);
    }

    pub fn register_from_binding(&mut self, location: &str, func: ForeignFn) {
        self.register(location.to_string(), func);
    }
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.load_stdlib();
        registry
    }
}

// Re-export implementations from interpreter
use crate::interpreter;

fn print_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::print_impl(args)
}
fn println_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::println_impl(args)
}
fn input_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::input_impl(args)
}
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
fn random_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::random_impl(args)
}
fn len_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::len_impl(args)
}
fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::concat_impl(args)
}
fn to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_string_impl(args)
}
fn to_float_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_float_impl(args)
}
fn to_int_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::to_int_impl(args)
}
fn trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::trim_impl(args)
}
fn contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    interpreter::contains_impl(args)
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
    fn test_load_stdlib() {
        let mut registry = FunctionRegistry::new();
        registry.load_stdlib();
        assert!(registry.contains("std::io::println"));
        assert!(registry.contains("std::math::sqrt"));
    }
}
