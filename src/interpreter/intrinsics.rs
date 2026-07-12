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

// ── Intrinsic Dispatch & Byte-Conversion Helpers ─────────────────────
//
// These are the interpreter fast-path functions that replace the old
// Intrinsic enum.  `execute_intrinsic` dispatches by string name;
// the byte-conversion helpers are shared across all interpreter submodules.
//
// 2026-07-12: Extracted from the monolithic interpreter/mod.rs into its
// own submodule during the Phase 4 monolith split.

use crate::interpreter::{RuntimeError, Value};

/// Dispatch an intrinsic by name.
///
/// Each intrinsic is a flat match arm that does one thing:
/// extract args, compute, return Bits.
pub fn execute_intrinsic(name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    match name {
        "__add_i64" => {
            let a = bits_to_i64(&args[0])?;
            let b = bits_to_i64(&args[1])?;
            Ok(Value::Bits(i64_to_bits(a.wrapping_add(b))))
        }
        "__sub_i64" => {
            let a = bits_to_i64(&args[0])?;
            let b = bits_to_i64(&args[1])?;
            Ok(Value::Bits(i64_to_bits(a.wrapping_sub(b))))
        }
        "__mul_i64" => {
            let a = bits_to_i64(&args[0])?;
            let b = bits_to_i64(&args[1])?;
            Ok(Value::Bits(i64_to_bits(a.wrapping_mul(b))))
        }
        "__eq_i64" => {
            let a = bits_to_i64(&args[0])?;
            let b = bits_to_i64(&args[1])?;
            Ok(Value::Bits(if a == b { vec![1] } else { vec![0] }))
        }
        "__fadd_f64" => {
            let a = bits_to_f64(&args[0])?;
            let b = bits_to_f64(&args[1])?;
            Ok(Value::Bits(f64_to_bits(a + b)))
        }
        _ => Err(RuntimeError::UnsupportedProjection(
            format!("unknown intrinsic: {}", name)))
    }
}

/// Extract i64 from Value::Bits (fallible).
pub(crate) fn bits_to_i64(v: &Value) -> Result<i64, RuntimeError> {
    let b = match v {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("expected Bits".into())),
    };
    let mut arr = [0u8; 8];
    let copy_len = b.len().min(8);
    arr[..copy_len].copy_from_slice(&b[..copy_len]);
    Ok(i64::from_le_bytes(arr))
}

/// Encode i64 as Vec<u8> (little-endian, 8 bytes).
pub(crate) fn i64_to_bits(i: i64) -> Vec<u8> {
    i.to_le_bytes().to_vec()
}

/// Extract i64 from Value::Bits (infallible, returns None on mismatch).
pub(crate) fn value_as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Bits(b) => {
            let mut arr = [0u8; 8];
            let copy_len = b.len().min(8);
            arr[..copy_len].copy_from_slice(&b[..copy_len]);
            Some(i64::from_le_bytes(arr))
        }
        _ => None,
    }
}

/// Extract bool from Value::Bits (first byte non-zero = true).
pub(crate) fn value_as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bits(b) => Some(b.first().copied().unwrap_or(0) != 0),
        _ => None,
    }
}

/// Extract f64 from Value::Bits (requires at least 8 bytes).
pub(crate) fn value_as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Bits(b) if b.len() >= 8 => {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&b[..8]);
            Some(f64::from_le_bytes(arr))
        }
        _ => None,
    }
}

/// Extract f64 from Value::Bits (fallible, errors if < 8 bytes).
pub(crate) fn bits_to_f64(v: &Value) -> Result<f64, RuntimeError> {
    let b = match v {
        Value::Bits(b) => b,
        _ => return Err(RuntimeError::TypeMismatch("expected Bits".into())),
    };
    if b.len() < 8 {
        return Err(RuntimeError::TypeMismatch(
            format!("expected 8 bytes for f64, got {}", b.len())));
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&b[..8]);
    Ok(f64::from_le_bytes(arr))
}

/// Encode f64 as Vec<u8> (little-endian, 8 bytes).
pub(crate) fn f64_to_bits(f: f64) -> Vec<u8> {
    f.to_le_bytes().to_vec()
}
