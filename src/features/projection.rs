use crate::ast::{Expr, ProjectionTarget, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::interpreter::value_as_i64;
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionExpr {
    pub source: Box<Expr>,
    pub target: ProjectionTarget,
}

impl ProjectionExpr {
    pub fn new(source: Expr, target: ProjectionTarget) -> Self {
        ProjectionExpr { source: Box::new(source), target }
    }
}

impl ExprTypecheck for ProjectionExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        Ok(Type::int())
    }
}

impl ExprEval for ProjectionExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let source_val = match ctx.eval_expr(&self.source)? {
            Value::Ref(inner) => *inner,
            v => v,
        };
        match &self.target {
            ProjectionTarget::Size => match &source_val {
                Value::Int(_) | Value::Float(_) | Value::Bool(_) | Value::Char(_) => Ok(Value::Int(1)),
                // 2026-07-11: Bits from Expr::String carry UTF-8 bytes; return byte length.
                Value::Bits(d) => Ok(Value::Int(d.len() as i64)),
                Value::List(items) => Ok(Value::Int(items.len() as i64)),
                Value::Tuple(items) => Ok(Value::Int(items.len() as i64)),
                Value::String(s) => Ok(Value::Int(s.len() as i64)),
                Value::HashMap(m) => Ok(Value::Int(m.len() as i64)),
                Value::HashSet(s) => Ok(Value::Int(s.len() as i64)),
                Value::Stack(v) => Ok(Value::Int(v.len() as i64)),
                Value::Queue(q) => Ok(Value::Int(q.len() as i64)),
                Value::StringBuilder(sb) => Ok(Value::Int(sb.len() as i64)),
                _ => Err(RuntimeError::TypeMismatch("Size projection requires List, String, or collection type".into())),
            },
            ProjectionTarget::Bytes => {
                let size = match &source_val {
                    Value::Int(_) | Value::Float(_) => 8,
                    Value::Bool(_) => 1, Value::Char(_) => 4,
                    Value::String(s) => s.len() as i64,
                    Value::List(items) => items.len() as i64 * 8,
                    Value::Bits(d) => d.len() as i64,
                    Value::Instance { fields, .. } => fields.len() as i64 * 8,
                    Value::Tuple(items) => items.len() as i64 * 8,
                    Value::Stack(v) => v.len() as i64 * 8,
                    Value::Queue(q) => q.len() as i64 * 8,
                    Value::StringBuilder(sb) => sb.len() as i64,
                    _ => 0,
                };
                Ok(Value::Int(size))
            }
            ProjectionTarget::Ptr => Ok(Value::Int(0)),
            ProjectionTarget::Alignment => {
                let align = match &source_val {
                    Value::Int(_) | Value::Float(_) => 8,
                    Value::Bool(_) => 1,
                    Value::Char(_) => 4,
                    Value::Bits(_) | Value::String(_) | Value::List(_) | Value::Tuple(_)
                        | Value::HashMap(_) | Value::HashSet(_)
                        | Value::Stack(_) | Value::Queue(_) | Value::Enum(..)
                        | Value::Instance { .. } | Value::StringBuilder(_)
                        | Value::Defn(_) | Value::DbvlTable(_) | Value::Regex(_)
                        | Value::Ptr(_) | Value::Ref(_) => 8,
                    Value::Void => 0,
                    Value::Expr(..) | Value::Stmt(..) | Value::Block(..) | Value::Items(..) | Value::Type(..) => {
                        return Err(RuntimeError::TypeMismatch("Alignment on compile-time value".into()));
                    }
                };
                Ok(Value::Int(align))
            }
            ProjectionTarget::Range => {
                let range = match &source_val {
                    Value::Int(_) | Value::Bits(_) => vec![Value::Int(i64::MIN), Value::Int(i64::MAX)],
                    Value::Bool(_) => vec![Value::Int(0), Value::Int(1)],
                    Value::Char(_) => vec![Value::Int(0), Value::Int(0x10FFFF)],
                    Value::Float(_) => vec![Value::Int(i64::MIN), Value::Int(i64::MAX)],
                    _ => vec![Value::Int(i64::MIN), Value::Int(i64::MAX)],
                };
                Ok(Value::List(range))
            }
            ProjectionTarget::Popcount => match source_val {
                Value::Int(n) => Ok(Value::Int(n.count_ones() as i64)),
                Value::Bits(_) => {
                    let n = value_as_i64(&source_val).unwrap();
                    Ok(Value::Int(n.count_ones() as i64))
                }
                _ => Err(RuntimeError::TypeMismatch("Popcount requires Int".into())),
            },
            ProjectionTarget::LeadingZeros => match source_val {
                Value::Int(n) => Ok(Value::Int(n.leading_zeros() as i64)),
                Value::Bits(_) => {
                    let n = value_as_i64(&source_val).unwrap();
                    Ok(Value::Int(n.leading_zeros() as i64))
                }
                _ => Err(RuntimeError::TypeMismatch("LeadingZeros requires Int".into())),
            },
            ProjectionTarget::TrailingZeros => match source_val {
                Value::Int(n) => Ok(Value::Int(n.trailing_zeros() as i64)),
                Value::Bits(_) => {
                    let n = value_as_i64(&source_val).unwrap();
                    Ok(Value::Int(n.trailing_zeros() as i64))
                }
                _ => Err(RuntimeError::TypeMismatch("TrailingZeros requires Int".into())),
            },
            ProjectionTarget::Absolute => match source_val {
                Value::Int(n) => Ok(Value::Int(n.abs())),
                Value::Bits(_) => {
                    let n = value_as_i64(&source_val).unwrap();
                    Ok(Value::Int(n.abs()))
                }
                Value::Float(f) => Ok(Value::Float(f.abs())),
                _ => Err(RuntimeError::TypeMismatch("Absolute requires Int or Float".into())),
            },
            ProjectionTarget::BitReverse => match source_val {
                Value::Int(n) => Ok(Value::Int(n.reverse_bits())),
                Value::Bits(_) => {
                    let n = value_as_i64(&source_val).unwrap();
                    Ok(Value::Int(n.reverse_bits()))
                }
                _ => Err(RuntimeError::TypeMismatch("BitReverse requires Int".into())),
            },
            ProjectionTarget::Type => {
                let discriminant = match &source_val {
                    Value::Int(_) => 1, Value::Bits(_) => 8, Value::Float(_) => 2, Value::Bool(_) => 3,
                    Value::Char(_) => 4, Value::String(_) => 5, Value::List(_) => 6,
                    Value::Tuple(_) => 7, Value::HashMap(_) => 9,
                    Value::HashSet(_) => 10, Value::StringBuilder(_) => 11,
                    Value::Stack(_) => 12, Value::Queue(_) => 13,
                    Value::Instance { .. } => 14, Value::Enum(..) => 15,
                    Value::Defn(_) => 16, Value::DbvlTable(_) => 17, Value::Regex(_) => 18,
                    Value::Ptr(_) | Value::Ref(_) => 19, Value::Void => 0,
                    Value::Expr(..) | Value::Stmt(..) | Value::Block(..) | Value::Items(..) | Value::Type(..) => {
                        unreachable!("compile-time only value")
                    }
                };
                Ok(Value::Int(discriminant))
            }
            ProjectionTarget::PtrBang => Ok(Value::Int(0)),
            ProjectionTarget::Keys => match &source_val {
                Value::HashMap(m) => {
                    let mut keys: Vec<Value> = m.keys().cloned().map(Value::String).collect();
                    keys.sort_by(|a, b| { if let (Value::String(a), Value::String(b)) = (a, b) { a.cmp(b) } else { std::cmp::Ordering::Equal } });
                    Ok(Value::List(keys))
                }
                _ => Err(RuntimeError::TypeMismatch("Keys requires HashMap".into())),
            },
            ProjectionTarget::Values => match &source_val {
                Value::HashMap(m) => Ok(Value::List(m.values().cloned().collect())),
                _ => Err(RuntimeError::TypeMismatch("Values requires HashMap".into())),
            },
            ProjectionTarget::Contains(key_expr) => {
                let key_val = ctx.eval_expr(key_expr)?;
                let key_str = ctx.value_to_string(&key_val)?;
                match &source_val {
                    Value::HashMap(m) => Ok(Value::Bool(m.contains_key(&key_str))),
                    Value::HashSet(s) => Ok(Value::Bool(s.contains(&key_str))),
                    _ => Err(RuntimeError::TypeMismatch("Contains requires HashMap or HashSet".into())),
                }
            }
            ProjectionTarget::IsEmpty => Ok(Value::Bool(match &source_val {
                Value::List(items) => items.is_empty(),
                Value::Tuple(items) => items.is_empty(),
                Value::HashMap(m) => m.is_empty(),
                Value::HashSet(s) => s.is_empty(),
                Value::String(s) => s.is_empty(),
                Value::Bits(d) => d.is_empty(),
                _ => return Err(RuntimeError::TypeMismatch("IsEmpty requires List, Tuple, HashMap, HashSet, String, or Bits".into())),
            })),
            ProjectionTarget::Get(key_expr) => {
                let key_val = ctx.eval_expr(key_expr)?;
                let key_str = ctx.value_to_string(&key_val)?;
                match &source_val {
                    Value::HashMap(m) => {
                        let mut fields = std::collections::HashMap::new();
                        match m.get(&key_str) {
                            Some(val) => { fields.insert("field_0".into(), val.clone()); Ok(Value::Enum("Option".into(), "Some".into(), fields)) }
                            None => Ok(Value::Enum("Option".into(), "None".into(), std::collections::HashMap::new())),
                        }
                    }
                    _ => Err(RuntimeError::TypeMismatch("Get requires HashMap".into())),
                }
            }
            ProjectionTarget::Top => match &source_val {
                Value::Stack(s) => {
                    let mut fields = std::collections::HashMap::new();
                    match s.last() {
                        Some(val) => { fields.insert("field_0".into(), val.clone()); Ok(Value::Enum("Option".into(), "Some".into(), fields)) }
                        None => Ok(Value::Enum("Option".into(), "None".into(), std::collections::HashMap::new())),
                    }
                }
                _ => Err(RuntimeError::TypeMismatch("Top requires Stack".into())),
            },
            ProjectionTarget::Front => match &source_val {
                Value::Queue(q) => {
                    let mut fields = std::collections::HashMap::new();
                    match q.front() {
                        Some(val) => { fields.insert("field_0".into(), val.clone()); Ok(Value::Enum("Option".into(), "Some".into(), fields)) }
                        None => Ok(Value::Enum("Option".into(), "None".into(), std::collections::HashMap::new())),
                    }
                }
                _ => Err(RuntimeError::TypeMismatch("Front requires Queue".into())),
            },
            ProjectionTarget::Elements => match &source_val {
                Value::HashSet(s) => {
                    let mut elems: Vec<Value> = s.iter().cloned().map(Value::String).collect();
                    elems.sort_by(|a, b| { if let (Value::String(a), Value::String(b)) = (a, b) { a.cmp(b) } else { std::cmp::Ordering::Equal } });
                    Ok(Value::List(elems))
                }
                _ => Err(RuntimeError::TypeMismatch("Elements requires HashSet".into())),
            },
            ProjectionTarget::AsStack => match &source_val {
                Value::List(items) => Ok(Value::Stack(items.clone())),
                _ => Err(RuntimeError::TypeMismatch("AsStack requires List".into())),
            },
            ProjectionTarget::AsQueue => match &source_val {
                Value::List(items) => Ok(Value::Queue(std::collections::VecDeque::from(items.clone()))),
                _ => Err(RuntimeError::TypeMismatch("AsQueue requires List".into())),
            },
            // Function metadata projections — handled by Interpreter::try_eval_fn_projection
            // before dispatch reaches ProjectionExpr. These are unreachable fallbacks.
            ProjectionTarget::Address
            | ProjectionTarget::Name
            | ProjectionTarget::Params
            | ProjectionTarget::Returns
            | ProjectionTarget::Arity
            | ProjectionTarget::Loc
            | ProjectionTarget::Doc
            | ProjectionTarget::Hash
            | ProjectionTarget::Contracts
            | ProjectionTarget::Module
            | ProjectionTarget::IsPure
            | ProjectionTarget::FnSpan => Err(RuntimeError::TypeMismatch(
                "Fn projection requires a function/transaction/inop name, not a runtime value".into()
            )),
            ProjectionTarget::BitRange(br) => {
                let n = match &source_val {
                    Value::Int(n) => *n,
                    Value::Bits(_) => value_as_i64(&source_val).unwrap(),
                    _ => return Err(RuntimeError::TypeMismatch("BitRange requires Int".into())),
                };
                let (lo, hi) = match br {
                    crate::ast::BitRange::Single(i) => (*i, *i),
                    crate::ast::BitRange::Range(l, h) => (*l, *h),
                    crate::ast::BitRange::Any(w) => (0, *w - 1),
                };
                if hi > 63 {
                    return Err(RuntimeError::TypeMismatch(
                        "BitRange exceeds 64-bit integer width".into()
                    ));
                }
                let width = hi - lo + 1;
                let shifted = (n as u64) >> lo;
                let result = if width >= 64 {
                    shifted as i64
                } else {
                    let mask = (1u64 << width) - 1;
                    (shifted & mask) as i64
                };
                Ok(Value::Int(result))
            },
            ProjectionTarget::UserDefined(name) => {
                let val = source_val.clone();
                match name.as_str() {
                    "Neg" => match &val {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        Value::Bits(_) => Ok(Value::Int(-value_as_i64(&val).unwrap())),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => Err(RuntimeError::TypeMismatch("Neg requires Int or Float".into())),
                    },
                    "Not" => match &val {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        _ => Err(RuntimeError::TypeMismatch("Not requires Bool".into())),
                    },
                    "BitNot" => match &val {
                        Value::Int(n) => Ok(Value::Int(!n)),
                        Value::Bits(_) => Ok(Value::Int(!value_as_i64(&val).unwrap())),
                        _ => Err(RuntimeError::TypeMismatch("BitNot requires Int".into())),
                    },
                    _ => Err(RuntimeError::UnsupportedProjection(format!(
                        "user-defined projection '{}' is not supported at runtime", name
                    ))),
                }
            }
            ProjectionTarget::UserDefinedWithArg(name, arg_expr) => {
                // Phase 3.5: Fast-path for well-known operator projections
                if let Ok(val) = eval_user_projection_fast_path(ctx, &source_val, name, arg_expr) {
                    return Ok(val);
                }
                Err(RuntimeError::UnsupportedProjection(format!(
                    "user-defined projection '{}' is not supported at runtime", name
                )))
            }
            // ── Phase 2F: Metadata projections ──────────────────
            ProjectionTarget::Width => {
                let w = match &source_val {
                    Value::Int(_) | Value::Bits(_) | Value::Float(_) => 64i64,
                    Value::Bool(_) => 1,
                    Value::Char(_) => 32,
                    _ => 64,
                };
                Ok(Value::Int(w))
            }
            ProjectionTarget::Endian => Ok(Value::String("little".to_string())),
            ProjectionTarget::Codec => Ok(Value::String("none".to_string())),
            ProjectionTarget::Ops => Ok(Value::Int(0)),
        }
    }
}

impl ExprCodegenLLVM for ProjectionExpr {
    fn emit_llvm(&self, 
        ctx: &mut crate::backend::llvm::LlvmBackend,
        out: &mut String,
        builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::Projection { source: self.source.clone(), target: self.target.clone() }, "")
    }
}


impl ExprCodegenWebstack for ProjectionExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        "JsValue::undefined".to_string()
    }
}

/// Phase 3.5: Fast-path for well-known UserDefinedWithArg operator projections
/// in the interpreter. Handles operator names (Add, Sub, Eq, etc.) on known
/// value types (Int, Float, Bool) directly without projection binding lookup.
fn eval_user_projection_fast_path(
    ctx: &mut Interpreter,
    source_val: &Value,
    name: &str,
    arg_expr: &Expr,
) -> Result<Value, RuntimeError> {
    let rhs = ctx.eval_expr(arg_expr)?;
    // Extract i64 from lhs and rhs, returning None if either is non-integer
    let (l_int, r_int) = (value_as_i64(source_val), value_as_i64(&rhs));
    match (source_val, &rhs, name) {
        // ── Int arithmetic ──
        _ if l_int.is_some() && r_int.is_some() => {
            let (l, r) = (l_int.unwrap(), r_int.unwrap());
            match name {
                "Add" => Ok(Value::Int(l + r)),
                "Sub" => Ok(Value::Int(l - r)),
                "Mul" => Ok(Value::Int(l * r)),
                "Div" => { if r == 0 { Err(RuntimeError::DivisionByZero) } else { Ok(Value::Int(l / r)) } }
                "Mod" => { if r == 0 { Err(RuntimeError::DivisionByZero) } else { Ok(Value::Int(l % r)) } }
                // ── Int comparisons ──
                "Eq" => Ok(Value::Bool(l == r)),
                "Ne" => Ok(Value::Bool(l != r)),
                "Lt" => Ok(Value::Bool(l < r)),
                "Le" => Ok(Value::Bool(l <= r)),
                "Gt" => Ok(Value::Bool(l > r)),
                "Ge" => Ok(Value::Bool(l >= r)),
                // ── Int bitwise ──
                "BitAnd" => Ok(Value::Int(l & r)),
                "BitOr" => Ok(Value::Int(l | r)),
                "BitXor" => Ok(Value::Int(l ^ r)),
                "Shl" => Ok(Value::Int(l << r)),
                "Shr" => Ok(Value::Int(l >> r)),
                // ── Int logical (treated as boolean in Brief) ──
                "And" => Ok(Value::Bool(l != 0 && r != 0)),
                "Or" => Ok(Value::Bool(l != 0 || r != 0)),
                _ => Err(RuntimeError::UnsupportedProjection(format!(
                    "projection '{}' not applicable to Int source type", name
                ))),
            }
        }
        // ── Float arithmetic ──
        (Value::Float(l), Value::Float(r), "Add") => Ok(Value::Float(l + r)),
        (Value::Float(l), Value::Float(r), "Sub") => Ok(Value::Float(l - r)),
        (Value::Float(l), Value::Float(r), "Mul") => Ok(Value::Float(l * r)),
        (Value::Float(l), Value::Float(r), "Div") => Ok(Value::Float(l / r)),
        // ── Float comparisons ──
        (Value::Float(l), Value::Float(r), "Eq") => Ok(Value::Bool((l - r).abs() < f64::EPSILON)),
        (Value::Float(l), Value::Float(r), "Ne") => Ok(Value::Bool((l - r).abs() >= f64::EPSILON)),
        (Value::Float(l), Value::Float(r), "Lt") => Ok(Value::Bool(l < r)),
        (Value::Float(l), Value::Float(r), "Le") => Ok(Value::Bool(l <= r)),
        (Value::Float(l), Value::Float(r), "Gt") => Ok(Value::Bool(l > r)),
        (Value::Float(l), Value::Float(r), "Ge") => Ok(Value::Bool(l >= r)),
        // ── Bool logical ──
        (Value::Bool(l), Value::Bool(r), "And") => Ok(Value::Bool(*l && *r)),
        (Value::Bool(l), Value::Bool(r), "Or") => Ok(Value::Bool(*l || *r)),
        (Value::Bool(l), Value::Bool(r), "Eq") => Ok(Value::Bool(l == r)),
        (Value::Bool(l), Value::Bool(r), "Ne") => Ok(Value::Bool(l != r)),
        // ── Unknown combination ──
        _ => Err(RuntimeError::UnsupportedProjection(format!(
            "projection '{}' not applicable to source type", name
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::Value;

    fn extract_bits(n: i64, lo: usize, hi: usize) -> i64 {
        if hi > 63 { return n; }
        let width = hi - lo + 1;
        let shifted = (n as u64) >> lo;
        if width >= 64 { shifted as i64 }
        else { (shifted & ((1u64 << width) - 1)) as i64 }
    }

    #[test]
    fn test_bit_range_single() {
        // Bit 2 of 0b1101 (13) = 1
        assert_eq!(extract_bits(0b1101, 2, 2), 1);
        // Bit 0 of 0b1101 (13) = 1
        assert_eq!(extract_bits(0b1101, 0, 0), 1);
        // Bit 3 of 0b1101 (13) = 1
        assert_eq!(extract_bits(0b1101, 3, 3), 1);
    }

    #[test]
    fn test_bit_range_range() {
        // Bits 0-1 of 0b1101 (13) = 0b01 = 1
        assert_eq!(extract_bits(0b1101, 0, 1), 0b01);
        // Bits 1-2 of 0b1101 (13) = 0b10 = 2
        assert_eq!(extract_bits(0b1101, 1, 2), 0b10);
        // Bits 0-3 of 0b1101 (13) = 0b1101 = 13
        assert_eq!(extract_bits(0b1101, 0, 3), 0b1101);
    }

    #[test]
    fn test_bit_range_wide() {
        // Bits 0-7 of 255 = 255
        assert_eq!(extract_bits(255, 0, 7), 255);
        // Bits 8-11 of 0xFF00 = 0xF = 15
        assert_eq!(extract_bits(0xFF00, 8, 11), 0xF);
        // Bits 8-15 of 0xFF00 = 0xFF = 255
        assert_eq!(extract_bits(0xFF00, 8, 15), 0xFF);
    }

    #[test]
    fn test_bit_range_exceeds_64() {
        // hi > 63 should return original
        assert_eq!(extract_bits(42, 0, 100), 42);
    }
}
