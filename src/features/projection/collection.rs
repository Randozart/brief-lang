use crate::ast::Expr;
use crate::features::traits::*;
use crate::interpreter::{i64_to_bits, Interpreter, RuntimeError, Value};

/// Evaluate the Keys projection: return HashMap keys as a List of Bits strings.
pub fn eval_keys_projection(source_val: &Value) -> Result<Value, RuntimeError> {
    let Value::HashMap(m) = source_val else {
        return Err(RuntimeError::TypeMismatch("Keys requires HashMap".into()));
    };
    let keys: Vec<Value> = m.keys().map(|k| Value::Bits(k.to_string().into())).collect();
    Ok(Value::List(keys))
}

/// Evaluate the Contains projection: check if key exists in HashMap or HashSet.
pub fn eval_contains_projection(ctx: &mut Interpreter, source_val: &Value, key_expr: &Expr) -> Result<Value, RuntimeError> {
    let key_val = ctx.eval_expr(key_expr)?;
    let key_str = ctx.value_to_string(&key_val)?;
    let found = match source_val {
        Value::HashMap(m) => m.contains_key(&key_str),
        Value::HashSet(s) => s.contains(&key_str),
        _ => return Err(RuntimeError::TypeMismatch("Contains requires HashMap or HashSet".into())),
    };
    Ok(Value::Bits(vec![if found { 1u8 } else { 0u8 }]))
}

fn option_some(val: Value) -> Value {
    let mut fields = std::collections::HashMap::new();
    fields.insert("field_0".into(), val);
    Value::Enum("Option".into(), "Some".into(), fields)
}

fn option_none() -> Value {
    Value::Enum("Option".into(), "None".into(), std::collections::HashMap::new())
}

/// Evaluate the Get projection: look up key in HashMap.
pub fn eval_get_projection(ctx: &mut Interpreter, source_val: &Value, key_expr: &Expr) -> Result<Value, RuntimeError> {
    let Value::HashMap(m) = source_val else {
        return Err(RuntimeError::TypeMismatch("Get requires HashMap".into()));
    };
    let key_val = ctx.eval_expr(key_expr)?;
    let key_str = ctx.value_to_string(&key_val)?;
    match m.get(&key_str) {
        Some(val) => Ok(option_some(val.clone())),
        None => Ok(option_none()),
    }
}

/// Evaluate the Top projection: last element of a List.
pub fn eval_top_projection(source_val: &Value) -> Result<Value, RuntimeError> {
    let Value::List(items) = source_val else {
        return Err(RuntimeError::TypeMismatch("Top requires List".into()));
    };
    match items.last() {
        Some(val) => Ok(option_some(val.clone())),
        None => Ok(option_none()),
    }
}

/// Evaluate the Front projection: first element of a List.
pub fn eval_front_projection(source_val: &Value) -> Result<Value, RuntimeError> {
    let Value::List(items) = source_val else {
        return Err(RuntimeError::TypeMismatch("Front requires List".into()));
    };
    match items.first() {
        Some(val) => Ok(option_some(val.clone())),
        None => Ok(option_none()),
    }
}

/// Evaluate the Values projection: return a List of HashMap values.
pub fn eval_values_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Values not yet implemented".into()))
}

/// Evaluate the AsStack projection.
pub fn eval_asstack_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("AsStack not yet implemented".into()))
}

/// Evaluate the AsQueue projection.
pub fn eval_asqueue_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("AsQueue not yet implemented".into()))
}
