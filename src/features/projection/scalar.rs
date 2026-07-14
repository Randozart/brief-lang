use crate::ast::{BitRange, Expr};
use crate::features::traits::*;
use crate::interpreter::{f64_to_bits, i64_to_bits, value_as_bool, value_as_f64, value_as_i64, Interpreter, RuntimeError, Value};

/// Compute the byte size of any Value recursively.
pub fn byte_size_of(v: &Value) -> i64 {
    match v {
        Value::Bits(b) => b.len() as i64,
        Value::List(items) => items.iter().map(byte_size_of).sum(),
        Value::Tuple(items) => items.iter().map(byte_size_of).sum(),
        Value::Instance { fields, .. } => fields.values().map(byte_size_of).sum(),
        Value::HashMap(m) => m.values().map(byte_size_of).sum(),
        Value::HashSet(s) => s.iter().map(|k| k.len() as i64).sum(),
        Value::Enum(_, _, fields) => fields.values().map(byte_size_of).sum(),
        _ => 0,
    }
}

/// Evaluate the Bytes projection: total byte size of the value.
pub fn eval_bytes_projection(source_val: &Value) -> Result<Value, RuntimeError> {
    let n = byte_size_of(source_val);
    Ok(i64_to_bits(n))
}

/// Evaluate the Size projection: element count for collections, byte count for Bits.
pub fn eval_size_projection(source_val: &Value) -> Result<Value, RuntimeError> {
    let n = match source_val {
        Value::Bits(d) => d.len() as i64,
        Value::List(v) => v.len() as i64,
        Value::Tuple(v) => v.len() as i64,
        Value::HashMap(m) => m.len() as i64,
        Value::HashSet(s) => s.len() as i64,
        _ => return Err(RuntimeError::TypeMismatch("Size projection requires collection".into())),
    };
    Ok(i64_to_bits(n))
}

/// Evaluate the Ptr projection: return zero (no pointer semantics in interpreter).
pub fn eval_ptr_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Ok(i64_to_bits(0))
}

/// Evaluate the IsEmpty projection: check if collection is empty.
pub fn eval_isempty_projection(source_val: &Value) -> Result<Value, RuntimeError> {
    let empty = match source_val {
        Value::List(items) => items.is_empty(),
        Value::Tuple(items) => items.is_empty(),
        Value::HashMap(m) => m.is_empty(),
        Value::HashSet(s) => s.is_empty(),
        Value::Bits(d) => d.is_empty(),
        _ => return Err(RuntimeError::TypeMismatch("IsEmpty requires collection".into())),
    };
    Ok(Value::Bits(vec![if empty { 1u8 } else { 0u8 }]))
}

/// Evaluate the Width projection: bit width of the value.
pub fn eval_width_projection(source_val: &Value) -> Result<Value, RuntimeError> {
    let w = match source_val {
        Value::Bits(b) if b.len() == 1 => 1,
        _ => 64,
    };
    Ok(i64_to_bits(w))
}

/// Evaluate the BitRange projection: extract a range of bits from an integer.
pub fn eval_bitrange_projection(source_val: &Value, br: &BitRange) -> Result<Value, RuntimeError> {
    let n = value_as_i64(source_val).ok_or_else(|| RuntimeError::TypeMismatch("BitRange requires Int".into()))?;
    let (lo, hi) = match br {
        BitRange::Single(i) => (*i, *i),
        BitRange::Range(l, h) => (*l, *h),
        BitRange::Any(w) => (0, *w - 1),
    };
    if hi > 63 {
        return Err(RuntimeError::TypeMismatch("BitRange exceeds 64-bit integer width".into()));
    }
    let width = hi - lo + 1;
    let shifted = (n as u64) >> lo;
    let result = if width >= 64 { shifted as i64 }
    else {
        let mask = (1u64 << width) - 1;
        (shifted & mask) as i64
    };
    Ok(i64_to_bits(result))
}

/// Fast-path for well-known UserDefinedWithArg operator projections (Add, Sub, Eq, etc.).
pub fn eval_user_projection_fast_path(
    ctx: &mut Interpreter,
    source_val: &Value,
    name: &str,
    arg_expr: &Expr,
) -> Result<Value, RuntimeError> {
    let rhs = ctx.eval_expr(arg_expr)?;
    let (l_int, r_int) = (value_as_i64(source_val), value_as_i64(&rhs));
    let l_f = value_as_f64(source_val);
    let r_f = value_as_f64(&rhs);
    let l_b = value_as_bool(source_val);
    let r_b = value_as_bool(&rhs);
    if l_int.is_some() && r_int.is_some() {
        return eval_int_op(l_int.unwrap(), r_int.unwrap(), name);
    }
    if l_f.is_some() && r_f.is_some() {
        return eval_float_op(l_f.unwrap(), r_f.unwrap(), name);
    }
    if l_b.is_some() && r_b.is_some() {
        return eval_bool_op(l_b.unwrap(), r_b.unwrap(), name);
    }
    Err(RuntimeError::UnsupportedProjection(format!("projection '{}' not applicable to source type", name)))
}

fn eval_int_op(l: i64, r: i64, name: &str) -> Result<Value, RuntimeError> {
    match name {
        "Add" => Ok(i64_to_bits(l + r)),
        "Sub" => Ok(i64_to_bits(l - r)),
        "Mul" => Ok(i64_to_bits(l * r)),
        "Div" => eval_int_div(l, r),
        "Mod" => eval_int_mod(l, r),
        "Eq" => Ok(Value::Bits(vec![if l == r { 1u8 } else { 0u8 }])),
        "Ne" => Ok(Value::Bits(vec![if l != r { 1u8 } else { 0u8 }])),
        "Lt" => Ok(Value::Bits(vec![if l < r { 1u8 } else { 0u8 }])),
        "Le" => Ok(Value::Bits(vec![if l <= r { 1u8 } else { 0u8 }])),
        "Gt" => Ok(Value::Bits(vec![if l > r { 1u8 } else { 0u8 }])),
        "Ge" => Ok(Value::Bits(vec![if l >= r { 1u8 } else { 0u8 }])),
        "BitAnd" => Ok(i64_to_bits(l & r)),
        "BitOr" => Ok(i64_to_bits(l | r)),
        "BitXor" => Ok(i64_to_bits(l ^ r)),
        "Shl" => Ok(i64_to_bits(l << r)),
        "Shr" => Ok(i64_to_bits(l >> r)),
        "And" => Ok(Value::Bits(vec![if l != 0 && r != 0 { 1u8 } else { 0u8 }])),
        "Or" => Ok(Value::Bits(vec![if l != 0 || r != 0 { 1u8 } else { 0u8 }])),
        _ => Err(RuntimeError::UnsupportedProjection(format!("projection '{}' not applicable to Int", name))),
    }
}

fn eval_int_div(l: i64, r: i64) -> Result<Value, RuntimeError> {
    if r == 0 { Err(RuntimeError::DivisionByZero) }
    else { Ok(i64_to_bits(l / r)) }
}

fn eval_int_mod(l: i64, r: i64) -> Result<Value, RuntimeError> {
    if r == 0 { Err(RuntimeError::DivisionByZero) }
    else { Ok(i64_to_bits(l % r)) }
}

fn eval_float_op(l: f64, r: f64, name: &str) -> Result<Value, RuntimeError> {
    match name {
        "Add" => Ok(f64_to_bits(l + r)),
        "Sub" => Ok(f64_to_bits(l - r)),
        "Mul" => Ok(f64_to_bits(l * r)),
        "Div" => Ok(f64_to_bits(l / r)),
        "Eq" => Ok(Value::Bits(vec![if (l - r).abs() < f64::EPSILON { 1u8 } else { 0u8 }])),
        "Ne" => Ok(Value::Bits(vec![if (l - r).abs() >= f64::EPSILON { 1u8 } else { 0u8 }])),
        "Lt" => Ok(Value::Bits(vec![if l < r { 1u8 } else { 0u8 }])),
        "Le" => Ok(Value::Bits(vec![if l <= r { 1u8 } else { 0u8 }])),
        "Gt" => Ok(Value::Bits(vec![if l > r { 1u8 } else { 0u8 }])),
        "Ge" => Ok(Value::Bits(vec![if l >= r { 1u8 } else { 0u8 }])),
        _ => Err(RuntimeError::UnsupportedProjection(format!("projection '{}' not applicable to Float", name))),
    }
}

fn eval_bool_op(l: bool, r: bool, name: &str) -> Result<Value, RuntimeError> {
    match name {
        "And" => Ok(Value::Bits(vec![if l && r { 1u8 } else { 0u8 }])),
        "Or" => Ok(Value::Bits(vec![if l || r { 1u8 } else { 0u8 }])),
        "Eq" => Ok(Value::Bits(vec![if l == r { 1u8 } else { 0u8 }])),
        "Ne" => Ok(Value::Bits(vec![if l != r { 1u8 } else { 0u8 }])),
        _ => Err(RuntimeError::UnsupportedProjection(format!("projection '{}' not applicable to Bool", name))),
    }
}

/// Evaluate Alignment projection: memory alignment hint.
pub fn eval_alignment_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Alignment not yet implemented".into()))
}

/// Evaluate Popcount projection: count set bits.
pub fn eval_popcount_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Popcount not yet implemented".into()))
}

/// Evaluate LeadingZeros projection.
pub fn eval_leading_zeros_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("LeadingZeros not yet implemented".into()))
}

/// Evaluate TrailingZeros projection.
pub fn eval_trailing_zeros_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("TrailingZeros not yet implemented".into()))
}

/// Evaluate Absolute projection.
pub fn eval_absolute_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Absolute not yet implemented".into()))
}

/// Evaluate BitReverse projection.
pub fn eval_bitreverse_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("BitReverse not yet implemented".into()))
}

/// Evaluate Type projection: return type name as string.
pub fn eval_type_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Type not yet implemented".into()))
}

/// Evaluate PtrBang projection: dereference pointer.
pub fn eval_ptrbang_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("PtrBang not yet implemented".into()))
}

pub fn eval_endian_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Endian not yet implemented".into()))
}

pub fn eval_codec_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Codec not yet implemented".into()))
}

pub fn eval_ops_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Ops not yet implemented".into()))
}

pub fn eval_elements_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Elements not yet implemented".into()))
}

pub fn eval_address_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Address not yet implemented".into()))
}

pub fn eval_name_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Name not yet implemented".into()))
}

pub fn eval_params_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Params not yet implemented".into()))
}

pub fn eval_returns_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Returns not yet implemented".into()))
}

pub fn eval_arity_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Arity not yet implemented".into()))
}

pub fn eval_loc_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Loc not yet implemented".into()))
}

pub fn eval_doc_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Doc not yet implemented".into()))
}

pub fn eval_hash_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Hash not yet implemented".into()))
}

pub fn eval_contracts_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Contracts not yet implemented".into()))
}

pub fn eval_module_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("Module not yet implemented".into()))
}

pub fn eval_ispure_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("IsPure not yet implemented".into()))
}

pub fn eval_fnspan_projection(_source_val: &Value) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("FnSpan not yet implemented".into()))
}

pub fn eval_user_defined_projection(_source_val: &Value, _name: &str) -> Result<Value, RuntimeError> {
    Err(RuntimeError::UnsupportedProjection("UserDefined projection not yet implemented".into()))
}
