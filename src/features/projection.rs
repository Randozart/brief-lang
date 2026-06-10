use crate::ast::{Expr, ProjectionTarget, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
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
        Ok(Type::Int)
    }
}

impl ExprEval for ProjectionExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let source_val = ctx.eval_expr(&self.source)?;
        match &self.target {
            ProjectionTarget::Size => match &source_val {
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
                    Value::Int(_) => 8, Value::Float(_) => 8,
                    Value::Bool(_) => 1, Value::Char(_) => 4,
                    Value::String(s) => s.len() as i64,
                    Value::List(items) => items.len() as i64 * 8,
                    Value::Instance { fields, .. } => fields.len() as i64 * 8,
                    _ => 0,
                };
                Ok(Value::Int(size))
            }
            ProjectionTarget::Ptr => Ok(Value::Int(0)),
            ProjectionTarget::Alignment => Ok(Value::Int(8)),
            ProjectionTarget::Range => Ok(Value::List(vec![Value::Int(i64::MIN), Value::Int(i64::MAX)])),
            ProjectionTarget::Popcount => match source_val {
                Value::Int(n) => Ok(Value::Int(n.count_ones() as i64)),
                _ => Err(RuntimeError::TypeMismatch("Popcount requires Int".into())),
            },
            ProjectionTarget::LeadingZeros => match source_val {
                Value::Int(n) => Ok(Value::Int(n.leading_zeros() as i64)),
                _ => Err(RuntimeError::TypeMismatch("LeadingZeros requires Int".into())),
            },
            ProjectionTarget::TrailingZeros => match source_val {
                Value::Int(n) => Ok(Value::Int(n.trailing_zeros() as i64)),
                _ => Err(RuntimeError::TypeMismatch("TrailingZeros requires Int".into())),
            },
            ProjectionTarget::Absolute => match source_val {
                Value::Int(n) => Ok(Value::Int(n.abs())),
                Value::Float(f) => Ok(Value::Float(f.abs())),
                _ => Err(RuntimeError::TypeMismatch("Absolute requires Int or Float".into())),
            },
            ProjectionTarget::BitReverse => match source_val {
                Value::Int(n) => Ok(Value::Int(n.reverse_bits())),
                _ => Err(RuntimeError::TypeMismatch("BitReverse requires Int".into())),
            },
            ProjectionTarget::Type => {
                let discriminant = match &source_val {
                    Value::Int(_) => 1, Value::Float(_) => 2, Value::Bool(_) => 3,
                    Value::Char(_) => 4, Value::String(_) => 5, Value::List(_) => 6,
                    Value::Tuple(_) => 7, Value::Data(_) => 8, Value::HashMap(_) => 9,
                    Value::HashSet(_) => 10, Value::StringBuilder(_) => 11,
                    Value::Stack(_) => 12, Value::Queue(_) => 13,
                    Value::Instance { .. } => 14, Value::Enum(..) => 15,
                    Value::Defn(_) => 16, Value::DbvlTable(_) => 17, Value::Void => 0,
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
            ProjectionTarget::Pop => Err(RuntimeError::TypeMismatch(
                "Pop projection not supported — use '<- &set' instead".into())),
            ProjectionTarget::Index(n) => match &source_val {
                Value::Tuple(items) if *n < items.len() => Ok(items[*n].clone()),
                _ => Err(RuntimeError::TypeMismatch("Index requires Tuple".into())),
            },
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
        }
    }
}

impl ExprCodegenLLVM for ProjectionExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::Projection { source: self.source.clone(), target: self.target.clone() }, "")
    }
}

impl ExprCodegenVHDL for ProjectionExpr {
    fn emit_vhdl(&self, _ctx: &crate::backend::vhdl::VhdlGenerator, _dispatch: &ExprDispatch) -> String {
        "'0'".to_string()
    }
}

impl ExprCodegenWebstack for ProjectionExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        "JsValue::undefined".to_string()
    }
}
