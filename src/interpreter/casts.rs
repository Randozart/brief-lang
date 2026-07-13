// ── Type Cast / Is / Like ──────────────────────────────────────────────
// 2026-07-12: Phase 3.5 — Rewritten for Bits-only Value.
// Type casts reinterpret Value::Bits bytes between type interpretations.
// IsType checks the byte width for type compatibility.

use crate::ast::{Expr, Type};
use crate::errors::RuntimeError;
use crate::interpreter::{f64_to_bits, i64_to_bits, Value};

/// Check whether a value matches a type.
/// With Bits-only values, this checks byte width against the expected type.
pub fn eval_is_type(val: &Value, target: &Type) -> Result<Value, RuntimeError> {
    let matches = val.bits_len() == type_byte_width(target);
    Ok(Value::Bits(vec![if matches { 1u8 } else { 0u8 }]))
}

/// Evaluate a type cast: reinterpret Value::Bits bytes between types.
pub fn eval_cast(val: Value, target: &Type) -> Result<Value, RuntimeError> {
    match target {
        Type::Custom(name) if name == "Float" || name == "Float64" => {
            let n = val.as_i64().unwrap_or(0);
            Ok(f64_to_bits(n as f64))
        }
        Type::Custom(name) if name == "Int" || name == "Int64" || name == "Bool" => {
            let bytes = val.bits_to_vec(8);
            Ok(Value::Bits(bytes))
        }
        Type::Custom(name) if name == "String" => {
            let s = bits_to_string(val);
            Ok(Value::Bits(s.into_bytes()))
        }
        Type::Custom(name) if name == "Char" => {
            let code = val.as_i64().unwrap_or(0) as u32;
            let ch = char::from_u32(code).unwrap_or('\0');
            Ok(Value::Bits((ch as u32).to_le_bytes().to_vec()))
        }
        _ => Ok(val),
    }
}

/// Structural equality check.
pub fn eval_like(lhs: &Value, rhs: &Value) -> Result<Value, RuntimeError> {
    let result = match (lhs, rhs) {
        (Value::Bits(a), Value::Bits(b)) => a == b,
        _ => false,
    };
    Ok(Value::Bits(vec![if result { 1u8 } else { 0u8 }]))
}

/// Get the byte length of a Bits value.
fn value_bits_len(v: &Value) -> usize {
    match v {
        Value::Bits(bytes) => bytes.len(),
        _ => 0,
    }
}

/// Convert value to Vec<u8> of given minimum size (padding with zeros).
fn value_bits_to_vec(v: &Value, min_len: usize) -> Vec<u8> {
    match v {
        Value::Bits(bytes) => {
            let mut result = bytes.clone();
            while result.len() < min_len {
                result.push(0);
            }
            result
        }
        _ => vec![0u8; min_len],
    }
}

/// Determine the byte width of a type for compatibility checking.
fn type_byte_width(ty: &Type) -> usize {
    match ty {
        Type::Custom(name) => match name.as_str() {
            "Int" | "UInt" | "Float" | "Float64" | "Double" | "Int64" | "UInt64" => 8,
            "Float32" | "F32" | "Int32" | "UInt32" | "Char" => 4,
            "Int16" | "UInt16" => 2,
            "Bool" | "Int8" | "UInt8" => 1,
            "String" | "Data" => 8,
            _ => 8,
        },
        Type::Applied(name, _) => match name.as_str() {
            "Ptr" => 8,
            _ => 8,
        },
        Type::Bits(n) => *n as usize,
        _ => 8,
    }
}

/// Convert Bits to a human-readable string for casting.
fn bits_to_string(val: &Value) -> String {
    match val {
        Value::Bits(bytes) => {
            if bytes.len() == 8 {
                val.as_i64()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| String::from_utf8_lossy(bytes).to_string())
            } else {
                String::from_utf8_lossy(bytes).to_string()
            }
        }
        _ => String::new(),
    }
}

// Helper methods on Value for this module
trait ValueExt {
    fn bits_len(&self) -> usize;
    fn bits_to_vec(&self, min_len: usize) -> Vec<u8>;
}

impl ValueExt for Value {
    fn bits_len(&self) -> usize {
        value_bits_len(self)
    }
    fn bits_to_vec(&self, min_len: usize) -> Vec<u8> {
        value_bits_to_vec(self, min_len)
    }
}
