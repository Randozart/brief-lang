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

// ── Type Cast / Is / Like / FromCheck ────────────────────────────────
//
// These are the interpreter methods for type-level queries and
// conversions: `is` (type check), `from` (variant check), `like`
// (structural equality), and `cast` (type conversion).
//
// Extracted from the monolithic interpreter/mod.rs during Phase 4.
// All functions follow max 2 nesting with guard clauses.

use super::intrinsics::{f64_to_bits, i64_to_bits, value_as_f64, value_as_i64};
use super::{Interpreter, RuntimeError, Value};
use crate::ast::*;
use std::collections::HashMap;

impl Interpreter {
    /// Check whether a value matches a type or variant target.
    ///
    /// `is Int`, `is String`, `is Foo`, `is Some` (variant check).
    /// Returns Bool (Bits([1]) or Bits([0])).
    pub(crate) fn eval_is_type(
        &self,
        val: Value,
        target: &crate::ast::IsTarget,
    ) -> Result<Value, RuntimeError> {
        use crate::ast::IsTarget;
        match target {
            IsTarget::Type(ty) => {
                let matches = self.type_match(&val, ty);
                Ok(Value::Bits(vec![if matches { 1u8 } else { 0u8 }]))
            }
            IsTarget::Variant(vname) => match &val {
                Value::Enum(_, variant_name, _) => {
                    Ok(Value::Bits(vec![if variant_name == vname { 1u8 } else { 0u8 }]))
                }
                _ => Err(RuntimeError::TypeMismatch(
                    "is requires an enum value for variant check".into(),
                )),
            },
        }
    }

    /// Check whether a value structurally belongs to a type (from check).
    ///
    /// Returns `true` if the value's typename matches the target's Debug
    /// representation. Used for runtime type validation.
    pub(crate) fn eval_from_check(
        &self,
        val: Value,
        ty: &Type,
    ) -> Result<Value, RuntimeError> {
        let type_name = match &val {
            Value::Instance { typename, .. } | Value::Enum(typename, ..) => typename.clone(),
            _ => return Ok(Value::Bits(vec![0u8])),
        };
        let target_name = format!("{:?}", ty);
        Ok(Value::Bits(vec![if type_name == target_name {
            1u8
        } else {
            0u8
        }]))
    }

    /// Structural equality check (`like` operator).
    ///
    /// Recursively compares two values. Bits compare by byte equality,
    /// Lists compare element-by-element, Instances compare field-by-field,
    /// Enums compare name+variant+fields.
    pub(crate) fn eval_like(&self, lhs: Value, rhs: Value) -> Result<Value, RuntimeError> {
        fn is_bool_true(v: &Value) -> bool {
            matches!(v, Value::Bits(b) if b.first() == Some(&1u8))
        }

        let result = match (&lhs, &rhs) {
            (Value::Bits(la), Value::Bits(lb)) => la == lb,
            (Value::List(a), Value::List(b)) => self.like_lists(a, b, is_bool_true),
            (Value::Instance { fields: af, .. }, Value::Instance { fields: bf, .. }) => {
                self.like_instances(af, bf, is_bool_true)
            }
            (
                Value::Enum(an, avn, ap),
                Value::Enum(bn, bvn, bp),
            ) => self.like_enums(an, avn, ap, bn, bvn, bp, is_bool_true),
            _ => false,
        };

        Ok(Value::Bits(vec![if result { 1u8 } else { 0u8 }]))
    }

    /// Compare two lists element-by-element using `like` semantics.
    fn like_lists(
        &self,
        a: &[Value],
        b: &[Value],
        is_true: fn(&Value) -> bool,
    ) -> bool {
        if a.len() != b.len() {
            return false;
        }
        a.iter()
            .zip(b.iter())
            .all(|(la, lb)| self.eval_like(la.clone(), lb.clone()).map_or(false, |v| is_true(&v)))
    }

    /// Compare two instance field maps using `like` semantics.
    fn like_instances(
        &self,
        af: &HashMap<String, Value>,
        bf: &HashMap<String, Value>,
        is_true: fn(&Value) -> bool,
    ) -> bool {
        if af.len() != bf.len() {
            return false;
        }
        af.iter().zip(bf.iter()).all(|pair| {
            let ((k_a, av), (k_b, bv)) = pair;
            k_a == k_b
                && self
                    .eval_like(av.clone(), bv.clone())
                    .map_or(false, |v| is_true(&v))
        })
    }

    /// Compare two enums using `like` semantics (name + variant + fields).
    fn like_enums(
        &self,
        an: &str,
        avn: &str,
        ap: &HashMap<String, Value>,
        bn: &str,
        bvn: &str,
        bp: &HashMap<String, Value>,
        is_true: fn(&Value) -> bool,
    ) -> bool {
        if an != bn || avn != bvn {
            return false;
        }
        if ap.len() != bp.len() {
            return false;
        }
        ap.iter().zip(bp.iter()).all(|((ka, va), (kb, vb))| {
            ka == kb
                && self
                    .eval_like(va.clone(), vb.clone())
                    .map_or(false, |v| is_true(&v))
        })
    }

    /// Check whether a Value::Bits matches a target Type for type-checking.
    ///
    /// Used by `eval_is_type` to determine type membership. This is a
    /// flat dispatch over known type names — no nesting beyond 1 level.
    fn type_match(&self, val: &Value, ty: &Type) -> bool {
        match (val, ty) {
            (Value::Bits(_), Type::Custom(t)) => {
                matches!(t.as_str(), "Int" | "UInt" | "Float" | "Bool" | "String" | "Char")
            }
            (Value::List(_), Type::Vector(..)) => true,
            (Value::List(_), Type::Applied(n, _)) if n == "List" => true,
            (Value::Instance { typename, .. }, Type::Custom(n)) => typename == n,
            (Value::Instance { typename, .. }, Type::Enum(n)) => typename == n,
            (Value::Instance { typename, .. }, Type::Applied(n, _)) => typename == n,
            (Value::Instance { typename, .. }, Type::Sig(n)) => typename == n,
            (Value::Enum(ename, ..), Type::Custom(n)) => ename == n,
            (Value::Enum(ename, ..), Type::Enum(n)) => ename == n,
            (Value::Enum(ename, ..), Type::Applied(n, _)) => ename == n,
            // 2026-07-03: Ptr value matches Ptr<T> or LayoutPtr type
            (Value::Bits(_), Type::Applied(n, _)) if n == "Ptr" => true,
            (Value::Bits(_), Type::LayoutPtr(_)) => true,
            _ => false,
        }
    }

    /// Evaluate a type cast: convert a value to a target type.
    ///
    /// Handles conversions between Int, Float, Char, Bool, String, Ptr.
    /// Any Bits value matches any scalar type (identity reinterpretation).
    pub(crate) fn eval_cast(&self, val: Value, target: &Type) -> Result<Value, RuntimeError> {
        match (&val, target) {
            // Int ↔ Float: extract i64, convert, re-encode as Bits
            (Value::Bits(_), Type::Custom(t)) if t == "Float" => {
                let n = value_as_i64(&val).unwrap_or(0);
                Ok(Value::Bits(f64_to_bits(n as f64)))
            }
            (Value::Bits(_), Type::Custom(t)) if t == "Int" => {
                // Extend to canonical i64 (8 bytes)
                let n = value_as_i64(&val).unwrap_or(0);
                Ok(Value::Bits(i64_to_bits(n)))
            }

            // Char ↔ Int (via u32 interpretation of first 4 bytes)
            (Value::Bits(b), Type::Custom(t)) if t == "Char" && b.len() <= 4 => {
                let code = u32::from_le_bytes([b.first().copied().unwrap_or(0), 0, 0, 0]);
                if code > 0x10FFFF {
                    return Err(RuntimeError::TypeMismatch(
                        "value out of valid Char range".into(),
                    ));
                }
                let ch = char::from_u32(code).unwrap_or('\0');
                Ok(Value::Bits((ch as u32).to_le_bytes().to_vec()))
            }

            // String: decode/encode as UTF-8 Bits
            // 2026-07-11: 4-byte values with trailing zeros → Char u32 encoding
            // 8-byte values → interpret as i64, format as decimal string
            // All others → treat as raw UTF-8 bytes
            (Value::Bits(b), Type::Custom(t)) if t == "String" => {
                let s = Self::bits_to_string(b, &val);
                Ok(Value::Bits(s.into_bytes()))
            }

            // Bool: check first byte
            (Value::Bits(b), Type::Custom(t)) if t == "Bool" => {
                let bval = b.first().copied().unwrap_or(0) != 0;
                Ok(Value::Bits(vec![if bval { 1u8 } else { 0u8 }]))
            }

            // Identity for Bits (all scalar values)
            (Value::Bits(_), Type::Custom(t))
                if matches!(t.as_str(), "Int" | "Float" | "Bool" | "Char" | "String" | "UInt" | "Data") =>
            {
                Ok(val.clone())
            }

            // Ptr ↔ Int identity (Ptr is just Bits containing the address)
            (Value::Bits(_), Type::Applied(t, _)) if t == "Ptr" => Ok(val.clone()),

            // Meld-backed custom type cast: identity (reinterpretation)
            (_, Type::Custom(_)) => Ok(val),

            // Unsupported
            _ => Err(RuntimeError::TypeMismatch(format!(
                "cannot convert {:?} to {:?}",
                val, target
            ))),
        }
    }

    /// Convert a byte slice to a string for casting purposes.
    ///
    /// 4-byte values with trailing zeros → Char u32, render as UTF-8.
    /// 8-byte values → interpret as i64, format as decimal.
    /// All others → treat as raw UTF-8 bytes.
    fn bits_to_string(b: &[u8], val: &Value) -> String {
        if b.len() == 4 && b[1] == 0 && b[2] == 0 && b[3] == 0 {
            char::from_u32(b[0] as u32).unwrap_or('\0').to_string()
        } else if b.len() == 8 {
            value_as_i64(val)
                .map(|n| n.to_string())
                .unwrap_or_else(|| String::from_utf8_lossy(b).to_string())
        } else {
            String::from_utf8_lossy(b).to_string()
        }
    }
}
