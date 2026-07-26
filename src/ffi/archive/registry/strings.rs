use crate::errors::RuntimeError;
use crate::interpreter::{bool_to_bits, i64_to_bits, Value};

fn value_to_string(val: &Value) -> Result<String, RuntimeError> {
    match val {
        Value::Bits(b) => Ok(String::from_UTF8_lossy(b).to_string()),
        _ => Err(RuntimeError::TypeError {
            expected: "String".into(),
            found: format!("{:?}", val),
        }),
    }
}

pub fn len_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    }).and_then(value_to_string)?;
    Ok(i64_to_bits(s.len() as i64))
}
pub fn concat_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let a = args.get(0).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    }).and_then(value_to_string)?;
    let b = args.get(1).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    }).and_then(value_to_string)?;
    Ok(Value::Bits(format!("{}{}", a, b).into_bytes()))
}
pub fn trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    }).and_then(value_to_string)?;
    Ok(Value::Bits(s.trim().to_string().into_bytes()))
}
pub fn contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let pattern = value_to_string(args.get(1).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(bool_to_bits(s.contains(&pattern)))
}
pub fn to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(Value::Bits(s.to_lowercase().into_bytes()))
}
pub fn to_upper_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(Value::Bits(s.to_uppercase().into_bytes()))
}
pub fn replace_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let from = value_to_string(args.get(1).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let to = value_to_string(args.get(2).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(Value::Bits(s.replace(&from, &to).into_bytes()))
}
pub fn chars_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let chars: Vec<Value> = s.chars().map(|c| Value::Bits((c as u32).to_le_bytes().to_vec())).collect();
    Ok(Value::List(chars))
}
pub fn starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let pattern = value_to_string(args.get(1).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(bool_to_bits(s.starts_with(&pattern)))
}
pub fn ends_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let pattern = value_to_string(args.get(1).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(bool_to_bits(s.ends_with(&pattern)))
}
pub fn from_str_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(Value::Bits(s.into_bytes()))
}
pub fn to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Bits(b)) => Ok(Value::Bits(b.clone())),
        Some(other) => Ok(Value::Bits(format!("{:?}", other).into_bytes())),
        None => Err(RuntimeError::TypeError {
            expected: "value".into(),
            found: "nothing".into(),
        }),
    }
}
pub fn string_trim_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(Value::Bits(s.trim().to_string().into_bytes()))
}
pub fn string_to_lower_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(Value::Bits(s.to_lowercase().into_bytes()))
}
pub fn string_contains_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let pattern = value_to_string(args.get(1).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(bool_to_bits(s.contains(&pattern)))
}
pub fn string_starts_with_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let pattern = value_to_string(args.get(1).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    Ok(bool_to_bits(s.starts_with(&pattern)))
}
pub fn string_split_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let delimiter = value_to_string(args.get(1).ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let parts: Vec<Value> = s.split(&delimiter).map(|p| Value::Bits(p.to_string().into_bytes())).collect();
    Ok(Value::List(parts))
}
pub fn substring_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let s = value_to_string(args.first().ok_or_else(|| RuntimeError::TypeError {
        expected: "String".into(),
        found: "nothing".into(),
    })?)?;
    let start = args.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as usize;
    let end = args.get(2).and_then(|v| v.as_i64())
        .map(|n| n as usize)
        .unwrap_or(s.len());
    let end = end.min(s.len());
    if start >= s.len() {
        return Ok(Value::Bits(Vec::new()));
    }
    Ok(Value::Bits(s[start..end].to_string().into_bytes()))
}
pub fn int_to_string_impl(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let n = args.first()
        .and_then(|v| v.as_i64())
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "Int".into(),
            found: format!("{:?}", args.first()),
        })?;
    Ok(Value::Bits(n.to_string().into_bytes()))
}

