use crate::errors::RuntimeError;
use crate::interpreter::{bool_to_bits, f64_to_bits, i64_to_bits, Value};
use std::collections::HashMap;

/// Convert a serde_json::Value to a Brief Value.
fn json_value_to_value(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Void,
        serde_json::Value::Bool(b) => bool_to_bits(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i64_to_bits(i)
            } else if let Some(f) = n.as_f64() {
                f64_to_bits(f)
            } else {
                Value::Void
            }
        }
        serde_json::Value::String(s) => Value::Bits(s.into_bytes()),
        serde_json::Value::Array(arr) => {
            let items: Vec<Value> = arr.into_iter().map(json_value_to_value).collect();
            Value::List(items)
        }
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k, json_value_to_value(v));
            }
            Value::HashMap(map)
        }
    }
}

/// Convert a Brief Value to a serde_json::Value.
fn value_to_json_value(val: &Value) -> serde_json::Value {
    match val {
        Value::Void => serde_json::Value::Null,
        Value::Int(n) => serde_json::Value::Number(serde_json::Number::from(*n)),
        Value::Float(f) => serde_json::Value::Number(serde_json::Number::from_f64(*f).unwrap_or(0.into())),
        Value::Bits(b) => {
            if b.len() == 8 {
                let n = i64::from_le_bytes(b[..8].try_into().unwrap_or([0u8; 8]));
                serde_json::Value::Number(serde_json::Number::from(n))
            } else {
                serde_json::Value::String(String::from_utf8_lossy(b).to_string())
            }
        }
        Value::List(items) => {
            let arr: Vec<serde_json::Value> = items.iter().map(value_to_json_value).collect();
            serde_json::Value::Array(arr)
        }
        Value::HashMap(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), value_to_json_value(v));
            }
            serde_json::Value::Object(obj)
        }
        Value::Instance { fields, .. } => {
            let mut obj = serde_json::Map::new();
            for (k, v) in fields {
                obj.insert(k.clone(), value_to_json_value(v));
            }
            serde_json::Value::Object(obj)
        }
        Value::Enum(_, variant, fields) => {
            let mut obj = serde_json::Map::new();
            obj.insert("variant".to_string(), serde_json::Value::String(variant.clone()));
            for (k, v) in fields {
                obj.insert(k.clone(), value_to_json_value(v));
            }
            serde_json::Value::Object(obj)
        }
        Value::Defn(name) => serde_json::Value::String(name.clone()),
        Value::Ref(inner) => value_to_json_value(inner),
    }
}

/// Simple JSON get-by-index: treats List values as arrays.
fn json_get_by_index_body(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::List(items)), Some(idx_val)) => {
            let idx = idx_val.as_i64().unwrap_or(0) as usize;
            if idx < items.len() {
                Ok(items[idx].clone())
            } else {
                Ok(Value::Void)
            }
        }
        _ => Err(RuntimeError::TypeError {
            expected: "List, index".into(),
            found: format!("{:?}, {:?}", args.first(), args.get(1)),
        }),
    }
}

pub fn json_parse_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let s = String::from_utf8_lossy(data);
            match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(json) => Ok(json_value_to_value(json)),
                Err(e) => Err(RuntimeError::HeapError(format!("json::parse failed: {}", e))),
            }
        }
        Some(other) => Ok(other.clone()),
        None => Err(RuntimeError::TypeError {
            expected: "String".into(),
            found: "nothing".into(),
        }),
    }
}

pub fn json_stringify_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(val) => {
            let json = value_to_json_value(val);
            Ok(Value::Bits(json.to_string().into_bytes()))
        }
        None => Err(RuntimeError::TypeError {
            expected: "value".into(),
            found: "nothing".into(),
        }),
    }
}

pub fn json_is_object_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Instance { .. }) | Some(Value::HashMap(_)) => Ok(Value::Bits(vec![1u8])),
        _ => Ok(Value::Bits(vec![0u8])),
    }
}

pub fn json_is_array_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::List(_)) => Ok(Value::Bits(vec![1u8])),
        _ => Ok(Value::Bits(vec![0u8])),
    }
}

pub fn json_is_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(_)) => Ok(Value::Bits(vec![1u8])),
        _ => Ok(Value::Bits(vec![0u8])),
    }
}

pub fn json_is_number_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(_)) => Ok(Value::Bits(vec![1u8])),
        _ => Ok(Value::Bits(vec![0u8])),
    }
}

pub fn json_is_bool_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(_)) => Ok(Value::Bits(vec![1u8])),
        _ => Ok(Value::Bits(vec![0u8])),
    }
}

pub fn json_is_null_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Void) => Ok(Value::Bits(vec![1u8])),
        _ => Ok(Value::Bits(vec![0u8])),
    }
}

pub fn json_get_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(val) => Ok(val.clone()),
        None => Err(RuntimeError::TypeError {
            expected: "value".into(),
            found: "nothing".into(),
        }),
    }
}

pub fn json_set_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(val) => Ok(val.clone()),
        None => Err(RuntimeError::TypeError {
            expected: "value".into(),
            found: "nothing".into(),
        }),
    }
}

pub fn json_keys_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Instance { fields, .. }) => {
            let keys: Vec<Value> = fields.keys().cloned().map(|s| Value::Bits(s.into_bytes())).collect();
            Ok(Value::List(keys))
        }
        Some(Value::HashMap(map)) => {
            let keys: Vec<Value> = map.keys().cloned().map(|s| Value::Bits(s.into_bytes())).collect();
            Ok(Value::List(keys))
        }
        Some(_) => Ok(Value::List(Vec::new())),
        None => Err(RuntimeError::TypeError {
            expected: "object".into(),
            found: "nothing".into(),
        }),
    }
}

pub fn json_length_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::List(items)) => Ok(i64_to_bits(items.len() as i64)),
        Some(Value::Instance { fields, .. }) => Ok(i64_to_bits(fields.len() as i64)),
        Some(Value::HashMap(map)) => Ok(i64_to_bits(map.len() as i64)),
        Some(Value::Bits(b)) => Ok(i64_to_bits(b.len() as i64)),
        Some(_) => Ok(i64_to_bits(0)),
        None => Err(RuntimeError::TypeError {
            expected: "value".into(),
            found: "nothing".into(),
        }),
    }
}

pub fn json_get_by_index_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    json_get_by_index_body(args)
}
