use crate::ast::{Expr, ProjectionTarget, Type};
use crate::features::traits::*;
use crate::interpreter::{f64_to_bits, i64_to_bits, value_as_i64, Interpreter, RuntimeError, Value};
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
            ProjectionTarget::Size => {
                let n = match &source_val {
                    Value::Bits(d) => d.len() as i64,
                    Value::List(v) => v.len() as i64,
                    Value::Tuple(v) => v.len() as i64,
                    Value::HashMap(m) => m.len() as i64,
                    Value::HashSet(s) => s.len() as i64,
                    _ => return Err(RuntimeError::TypeMismatch("Size projection requires collection".into())),
                };
                Ok(Value::Bits(i64_to_bits(n)))
            }
            ProjectionTarget::Ptr => Ok(Value::Bits(i64_to_bits(0))),
            ProjectionTarget::IsEmpty => {
                let empty = match &source_val {
                    Value::List(items) => items.is_empty(),
                    Value::Tuple(items) => items.is_empty(),
                    Value::HashMap(m) => m.is_empty(),
                    Value::HashSet(s) => s.is_empty(),
                    Value::Bits(d) => d.is_empty(),
                    _ => return Err(RuntimeError::TypeMismatch("IsEmpty requires collection".into())),
                };
                Ok(Value::Bits(vec![if empty { 1u8 } else { 0u8 }]))
            }
            ProjectionTarget::Contains(key_expr) => {
                let key_val = ctx.eval_expr(key_expr)?;
                let key_str = ctx.value_to_string(&key_val)?;
                match &source_val {
                    Value::HashMap(m) => Ok(Value::Bits(vec![if m.contains_key(&key_str) { 1u8 } else { 0u8 }])),
                    Value::HashSet(s) => Ok(Value::Bits(vec![if s.contains(&key_str) { 1u8 } else { 0u8 }])),
                    _ => Err(RuntimeError::TypeMismatch("Contains requires HashMap or HashSet".into())),
                }
            }
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
                Value::List(items) => {
                    let mut fields = std::collections::HashMap::new();
                    match items.last() {
                        Some(val) => { fields.insert("field_0".into(), val.clone()); Ok(Value::Enum("Option".into(), "Some".into(), fields)) }
                        None => Ok(Value::Enum("Option".into(), "None".into(), std::collections::HashMap::new())),
                    }
                }
                _ => Err(RuntimeError::TypeMismatch("Top requires List".into())),
            },
            ProjectionTarget::Front => match &source_val {
                Value::List(items) => {
                    let mut fields = std::collections::HashMap::new();
                    match items.first() {
                        Some(val) => { fields.insert("field_0".into(), val.clone()); Ok(Value::Enum("Option".into(), "Some".into(), fields)) }
                        None => Ok(Value::Enum("Option".into(), "None".into(), std::collections::HashMap::new())),
                    }
                }
                _ => Err(RuntimeError::TypeMismatch("Front requires List".into())),
            },
            ProjectionTarget::BitRange(br) => {
                let n = value_as_i64(&source_val).ok_or_else(|| RuntimeError::TypeMismatch("BitRange requires Int".into()))?;
                let (lo, hi) = match br {
                    crate::ast::BitRange::Single(i) => (*i, *i),
                    crate::ast::BitRange::Range(l, h) => (*l, *h),
                    crate::ast::BitRange::Any(w) => (0, *w - 1),
                };
                if hi > 63 { return Err(RuntimeError::TypeMismatch("BitRange exceeds 64-bit integer width".into())); }
                let width = hi - lo + 1;
                let shifted = (n as u64) >> lo;
                let result = if width >= 64 { shifted as i64 }
                else { let mask = (1u64 << width) - 1; (shifted & mask) as i64 };
                Ok(Value::Bits(i64_to_bits(result)))
            }
            ProjectionTarget::Width => {
                let w = match &source_val {
                    Value::Bits(b) if b.len() == 1 => 1,
                    _ => 64,
                };
                Ok(Value::Bits(i64_to_bits(w)))
            }
            ProjectionTarget::Keys | ProjectionTarget::Values
            | ProjectionTarget::Bytes | ProjectionTarget::Alignment
            | ProjectionTarget::Range | ProjectionTarget::Popcount
            | ProjectionTarget::LeadingZeros | ProjectionTarget::TrailingZeros
            | ProjectionTarget::Absolute | ProjectionTarget::BitReverse
            | ProjectionTarget::Type | ProjectionTarget::PtrBang
            | ProjectionTarget::Endian | ProjectionTarget::Codec
            | ProjectionTarget::Ops | ProjectionTarget::Elements
            | ProjectionTarget::AsStack | ProjectionTarget::AsQueue
            | ProjectionTarget::Address | ProjectionTarget::Name
            | ProjectionTarget::Params | ProjectionTarget::Returns
            | ProjectionTarget::Arity | ProjectionTarget::Loc
            | ProjectionTarget::Doc | ProjectionTarget::Hash
            | ProjectionTarget::Contracts | ProjectionTarget::Module
            | ProjectionTarget::IsPure | ProjectionTarget::FnSpan
            | ProjectionTarget::UserDefined(_)
            | ProjectionTarget::UserDefinedWithArg(_, _) => {
                Err(RuntimeError::UnsupportedProjection("projection not yet implemented with Bits".into()))
            }
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
