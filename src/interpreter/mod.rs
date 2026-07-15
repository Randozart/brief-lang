// ── Interpreter — Value, VirtualHeap, Re-exports ───────────────────────
// 2026-07-12: Phase 3.0 — Bits-only Value, sandboxed VirtualHeap.
// Value::Bits(Vec<u8>) is the ONLY program-value variant for program data.
// Meta-objects (Defn, Expr, etc.) are compiler-internal and never
// reach user code.
//
// 2026-07-14: Re-added List/Enum/Instance/HashMap variants for FFI bridge
// code that marshals structured types between native libraries and the
// interpreter. These are FFI-only — no interpreter eval path produces them.

pub mod casts;
mod cells;
mod eval;
mod ffi;
mod intrinsics;

pub use cells::*;
pub use eval::*;
pub use ffi::*;
pub use intrinsics::*;

use crate::ast::{Expr, Statement};
use std::collections::HashMap;
use std::sync::Arc;

/// FFI files import RuntimeError from crate::interpreter.
pub use crate::errors::RuntimeError;

/// Compatibility struct bridging old Interpreter-based code to the new
/// function-based eval API. The old `Interpreter` had `state`, `prior_state`,
/// `heap`, `eval_expr()`, and `exec_stmt()`. This shim preserves that API
/// while delegating to the standalone `eval_expr`/`eval_statement` functions.
/// 2026-07-14: Added for reactor.rs and feature code compatibility.
#[derive(Debug, Clone)]
pub struct Interpreter {
    pub state: HashMap<String, Value>,
    pub prior_state: HashMap<String, Value>,
    pub heap: VirtualHeap,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            state: HashMap::new(),
            prior_state: HashMap::new(),
            heap: VirtualHeap::new(),
        }
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        eval_expr(expr, &mut self.heap, &mut self.state)
    }

    pub fn exec_stmt(&mut self, stmt: &Statement) -> Result<Value, RuntimeError> {
        eval_statement(stmt, &mut self.heap, &mut self.state)
    }

    /// Pattern match a value against a pattern (delegated from features).
    pub fn pattern_match(pat: &crate::ast::Pattern, val: &Value, bindings: &mut HashMap<String, Value>) -> bool {
        match pat {
            crate::ast::Pattern::Wildcard => true,
            crate::ast::Pattern::Literal(lit) => {
                let lit_val = match lit {
                    Expr::Decimal(n) => Value::Int(*n),
                    Expr::Bool(b) => Value::Bits(vec![if *b { 1 } else { 0 }]),
                    _ => return false,
                };
                *val == lit_val
            }
            crate::ast::Pattern::Binding(name) => {
                bindings.insert(name.clone(), val.clone());
                true
            }
            crate::ast::Pattern::EnumVariant(_, _) => false,
            crate::ast::Pattern::Tuple(patterns) => {
                if let Value::List(items) = val {
                    if items.len() != patterns.len() {
                        return false;
                    }
                    for (pat, item) in patterns.iter().zip(items.iter()) {
                        if !Self::pattern_match(pat, item, bindings) {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            }
            crate::ast::Pattern::Range(_, _) => false,
        }
    }

    /// Compute the nesting depth of a list value (for FFI).
    pub fn list_nesting_depth(val: &Value) -> usize {
        match val {
            Value::List(items) => {
                items.iter().map(|v| Self::list_nesting_depth(v)).max().unwrap_or(0) + 1
            }
            _ => 0,
        }
    }

    /// Expand coordinate dimensions into a flat list of indices (for FFI).
    pub fn expand_coordinates(coords: &[Expr], dims: usize) -> Result<Vec<Value>, RuntimeError> {
        let mut result = Vec::new();
        for coord in coords {
            match coord {
                Expr::Decimal(n) => result.push(Value::Int(*n)),
                _ => return Err(RuntimeError::TypeError { expected: "integer".to_string(), found: format!("{:?}", coord) }),
            }
        }
        Ok(result)
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

/// Signature for foreign function implementations registered in the FFI registry.
pub type ForeignFn = fn(Vec<Value>) -> Result<Value, RuntimeError>;

/// The only representational value in the Brief interpreter.
/// All program data — Int, Float, Bool, String, pointers — is Bits(Vec<u8>).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// The sole representational storage cell for program data.
    Bits(Vec<u8>),

    /// 2026-07-14: Typed value variants for generic op dispatch.
    /// These allow the interpreter to distinguish int from float values
    /// when both would be valid as raw Bits(Vec<u8>).
    Int(i64),
    Float(f64),

    // Compiler-internal meta-objects (never reach user code):
    Defn(String),
    Void,
    Ref(Box<Value>),

    // ── FFI bridge variants ─────────────────────────────────────────
    // 2026-07-14: These are produced/consumed only by the FFI layer
    // when marshalling structured data to/from native libraries.
    // No interpreter eval path should produce or match these.
    /// Heterogeneous list of values — used for JSON arrays, SHM lists.
    List(Vec<Value>),
    /// Enum value with type name, variant name, and named fields.
    Enum(String, String, HashMap<String, Value>),
    /// Struct-like instance with a type name and named fields.
    Instance {
        typename: String,
        fields: HashMap<String, Value>,
    },
    /// String-keyed map — used by JSON objects, Arrow IPC.
    HashMap(HashMap<String, Value>),
}

impl Value {
    pub fn bits(data: Vec<u8>) -> Self {
        Value::Bits(data)
    }

    pub fn void() -> Self {
        Value::Void
    }

    /// Extract first 8 bytes as little-endian i64.
    // 2026-07-14: Zero-pad buffers shorter than 8 bytes so small bit
    // values (zero_bits(4), zext of 1-byte bool) convert correctly.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::Bits(bytes) => {
                let mut arr = [0u8; 8];
                let copy_len = bytes.len().min(8);
                arr[..copy_len].copy_from_slice(&bytes[..copy_len]);
                Some(i64::from_le_bytes(arr))
            }
            _ => None,
        }
    }

    /// Extract first 8 bytes as f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Bits(bytes) if bytes.len() >= 8 => {
                let arr: [u8; 8] = bytes[..8].try_into().ok()?;
                Some(f64::from_le_bytes(arr))
            }
            _ => None,
        }
    }

    /// Extract first byte as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bits(bytes) => bytes.first().map(|b| *b != 0),
            _ => None,
        }
    }

    /// Check if the value is truthy (any non-zero byte).
    pub fn is_true(&self) -> bool {
        self.as_bool().unwrap_or(false)
    }
}

/// Convert i64 to 8-byte little-endian Bits value.
pub fn i64_to_bits(n: i64) -> Value {
    Value::Int(n)
}

/// Convert f64 to 8-byte little-endian Bits value.
pub fn f64_to_bits(f: f64) -> Value {
    Value::Float(f)
}

/// Convert bool to 1-byte Bits value.
pub fn bool_to_bits(b: bool) -> Value {
    Value::Bits(vec![if b { 1 } else { 0 }])
}

/// Create a zero-filled Bits value of the given byte size.
pub fn zero_bits(size: usize) -> Value {
    Value::Bits(vec![0u8; size])
}

/// Sandboxed compile-time heap for pointer arithmetic and allocation.
/// 2026-07-12: Phase 3.0 — Bounds-checked read/write.
#[derive(Debug, Clone)]
pub struct VirtualHeap {
    allocations: HashMap<u64, Vec<u8>>,
    next_address: u64,
}

impl VirtualHeap {
    pub fn new() -> Self {
        VirtualHeap {
            allocations: HashMap::new(),
            next_address: 0x1000, // start at page-aligned address
        }
    }

    /// Allocate a block of the given size. Returns virtual address.
    /// The allocation is zero-filled.
    pub fn allocate(&mut self, size: usize) -> u64 {
        let addr = self.next_address;
        self.allocations.insert(addr, vec![0u8; size]);
        self.next_address += size as u64 + 16; // small gap between allocations
        addr
    }

    /// Read bytes from the given address. Returns None if address not found
    /// or the read would go out of bounds.
    pub fn read(&self, addr: u64, size: usize) -> Option<&[u8]> {
        let (base, data) = self.find_block(addr)?;
        let offset = (addr - base) as usize;
        if offset + size > data.len() {
            return None;
        }
        Some(&data[offset..offset + size])
    }

    /// Write bytes at the given address. Returns error if address not found
    /// or the write would go out of bounds.
    pub fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), String> {
        let (base, block) = self
            .find_block_mut(addr)
            .ok_or("heap: address not allocated")?;
        let offset = (addr - base) as usize;
        if offset + data.len() > block.len() {
            return Err("heap: write out of bounds".into());
        }
        block[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Free a previously allocated block. Returns error if not found.
    pub fn free(&mut self, addr: u64) -> Result<(), String> {
        // Find the block that contains this address
        let base = self
            .allocations
            .keys()
            .filter(|k| **k <= addr)
            .max()
            .copied()
            .ok_or("heap: address not found")?;
        self.allocations
            .remove(&base)
            .ok_or("heap: address not found")?;
        Ok(())
    }

    /// Check if an address is allocated.
    pub fn contains(&self, addr: u64) -> bool {
        self.allocations.keys().any(|k| *k <= addr)
    }

    /// Find the block base and data for an address (read).
    fn find_block(&self, addr: u64) -> Option<(u64, &Vec<u8>)> {
        self.allocations
            .iter()
            .filter(|(k, v)| **k <= addr && addr < **k + v.len() as u64)
            .map(|(k, v)| (*k, v))
            .next()
    }

    /// Find the block base and data for an address (write).
    fn find_block_mut(&mut self, addr: u64) -> Option<(u64, &mut Vec<u8>)> {
        for (k, v) in &mut self.allocations {
            if *k <= addr && addr < *k + v.len() as u64 {
                return Some((*k, v));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_as_i64() {
        let v = i64_to_bits(42);
        assert_eq!(v.as_i64(), Some(42));
    }

    #[test]
    fn test_value_as_f64() {
        let v = f64_to_bits(3.14);
        let result = v.as_f64().unwrap();
        assert!((result - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_value_as_bool() {
        assert!(bool_to_bits(true).as_bool().unwrap());
        assert!(!bool_to_bits(false).as_bool().unwrap());
    }

    #[test]
    fn test_virtual_heap_alloc_read_write() {
        let mut heap = VirtualHeap::new();
        let addr = heap.allocate(16);
        assert!(heap.contains(addr));

        heap.write(addr, &[1, 2, 3, 4]).unwrap();
        let data = heap.read(addr, 4).unwrap();
        assert_eq!(data, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_virtual_heap_read_out_of_bounds() {
        let mut heap = VirtualHeap::new();
        let addr = heap.allocate(8);
        assert!(heap.read(addr, 16).is_none());
    }

    #[test]
    fn test_virtual_heap_write_out_of_bounds() {
        let mut heap = VirtualHeap::new();
        let addr = heap.allocate(4);
        assert!(heap.write(addr, &[0u8; 8]).is_err());
    }

    #[test]
    fn test_virtual_heap_free() {
        let mut heap = VirtualHeap::new();
        let addr = heap.allocate(8);
        heap.free(addr).unwrap();
        assert!(!heap.contains(addr));
    }

    #[test]
    fn test_virtual_heap_free_nonexistent() {
        let mut heap = VirtualHeap::new();
        assert!(heap.free(999).is_err());
    }

    #[test]
    fn test_virtual_heap_sequential_allocs_give_different_addresses() {
        let mut heap = VirtualHeap::new();
        let a1 = heap.allocate(8);
        let a2 = heap.allocate(8);
        assert_ne!(a1, a2);
    }

    #[test]
    fn test_virtual_heap_non_contained_address() {
        let heap = VirtualHeap::new();
        assert!(!heap.contains(0xDEAD));
    }

    #[test]
    fn test_zero_bits() {
        let v = zero_bits(4);
        assert_eq!(v.as_i64(), Some(0));
    }

    #[test]
    fn test_deref_expression_evaluates_inner() {
        // Deref in the interpreter just evaluates the inner expression.
        let mut heap = VirtualHeap::new();
        let mut bindings = std::collections::HashMap::new();
        bindings.insert("x".to_string(), i64_to_bits(42));
        let expr = Expr::Deref(Box::new(Expr::Identifier("x".to_string())));
        let result = eval_expr(&expr, &mut heap, &mut bindings).unwrap();
        assert_eq!(result.as_i64(), Some(42));
    }

    #[test]
    fn test_deref_nested() {
        // Deref of deref: *(*y) — inner is evaluated, then outer.
        let mut heap = VirtualHeap::new();
        let mut bindings = std::collections::HashMap::new();
        bindings.insert("y".to_string(), i64_to_bits(99));
        let inner = Expr::Deref(Box::new(Expr::Identifier("y".to_string())));
        let outer = Expr::Deref(Box::new(inner));
        let result = eval_expr(&outer, &mut heap, &mut bindings).unwrap();
        assert_eq!(result.as_i64(), Some(99));
    }
}
