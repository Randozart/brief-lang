use crate::interpreter::{RuntimeError, Value};

pub fn json_parse_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(data)) => {
            let s = String::from_utf8_lossy(data);
            match serde_json::from_str::<serde_json::Value>(&s) {
                Ok(json) => Ok(crate::interpreter::json_value_to_value(json)),
                Err(e) => Err(RuntimeError::TypeMismatch(format!("json::parse failed: {}", e))),
            }
        }
        Some(other) => Ok(other.clone()),
        None => Err(RuntimeError::TypeMismatch("json::parse expects 1 argument (string)".to_string())),
    }
}

pub fn json_stringify_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(val) => {
            let json = crate::interpreter::value_to_json_value(val);
            Ok(Value::Bits(json.to_string().into_bytes()))
        }
        None => Err(RuntimeError::TypeMismatch("json::stringify expects 1 argument".to_string())),
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
        None => Err(RuntimeError::TypeMismatch("json::get expects at least 1 argument".to_string())),
    }
}

pub fn json_set_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(val) => Ok(val.clone()),
        None => Err(RuntimeError::TypeMismatch("json::set expects at least 1 argument".to_string())),
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
        None => Err(RuntimeError::TypeMismatch("json::keys expects 1 argument".to_string())),
    }
}

pub fn json_length_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::List(items)) => Ok(Value::Bits(crate::interpreter::i64_to_bits(items.len() as i64))),
        Some(Value::Instance { fields, .. }) => Ok(Value::Bits(crate::interpreter::i64_to_bits(fields.len() as i64))),
        Some(Value::HashMap(map)) => Ok(Value::Bits(crate::interpreter::i64_to_bits(map.len() as i64))),
        Some(Value::Bits(b)) => Ok(Value::Bits(crate::interpreter::i64_to_bits(b.len() as i64))),
        Some(_) => Ok(Value::Bits(crate::interpreter::i64_to_bits(0))),
        None => Err(RuntimeError::TypeMismatch("json::length expects 1 argument".to_string())),
    }
}

pub fn json_get_by_index_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    crate::interpreter::json_get_by_index_impl(args)
}
