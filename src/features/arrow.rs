use crate::ast::{ArrowDir, Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowMutExpr {
    pub dir: ArrowDir,
    pub target: Box<Expr>,
    pub index: Box<Expr>,
    pub value: Option<Box<Expr>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowDiscardExpr {
    pub target: Box<Expr>,
    pub index: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowTransferExpr {
    pub dest: Box<Expr>,
    pub source: Box<Expr>,
    pub filter: Option<Box<Expr>>,
}

impl ExprTypecheck for ArrowMutExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) } }
impl ExprTypecheck for ArrowDiscardExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) } }
impl ExprTypecheck for ArrowTransferExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) } }

    impl ExprEval for ArrowMutExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let (root_name, field_path) = ctx.extract_arrow_root(&self.target)?;
        let mut collection = ctx.resolve_arrow_value(&root_name, &field_path)?;
        match self.dir {
            ArrowDir::Push => {
                let v = ctx.eval_expr(self.value.as_ref().ok_or_else(|| RuntimeError::TypeMismatch("Push requires a value".into()))?)?;
                let vc = v.clone();
                match &mut collection {
                    Value::List(list) => {
                        let pos = ctx.eval_arrow_pos(list, &self.index)?;
                        match pos { Some(p) if p < list.len() => list.insert(p, v), _ => list.push(v), }
                        let c = list.clone();
                        ctx.store_arrow_value(&root_name, &field_path, Value::List(c.clone()));
                        Ok(Value::List(c))
                    }
                    Value::HashMap(map) => {
                        // &map <- (key, value) — index is Term, value is a 2-element pair
                        if matches!(self.index.as_ref(), Expr::Term) {
                            let pair = match v.clone() {
                                Value::List(p) | Value::Tuple(p) => p,
                                _ => return Err(RuntimeError::TypeMismatch("HashMap insert requires a 2-element tuple or list (key, value)".into())),
                            };
                            if pair.len() != 2 { return Err(RuntimeError::TypeMismatch("HashMap insert requires exactly 2 elements (key, value)".into())); }
                            let mut iter = pair.into_iter();
                            let key = ctx.value_to_string(&iter.next().unwrap())?;
                            let val = iter.next().unwrap();
                            map.insert(key, val);
                        } else {
                            // &map[key] <- value — use index as key expression
                            let key_val = ctx.eval_expr(&self.index)?;
                            let key = ctx.value_to_string(&key_val)?;
                            map.insert(key, v.clone());
                        }
                        let c = map.clone();
                        ctx.store_arrow_value(&root_name, &field_path, Value::HashMap(c.clone()));
                        Ok(Value::HashMap(c))
                    }
                    Value::HashSet(set) => {
                        let elem = ctx.value_to_string(&v)?;
                        set.insert(elem);
                        let c = set.clone();
                        ctx.store_arrow_value(&root_name, &field_path, Value::HashSet(c.clone()));
                        Ok(Value::HashSet(c))
                    }
                    Value::Stack(stack) => { stack.push(v); let c = stack.clone(); ctx.store_arrow_value(&root_name, &field_path, Value::Stack(c.clone())); Ok(Value::Stack(c)) }
                    Value::Queue(queue) => { queue.push_back(v); let c = queue.clone(); ctx.store_arrow_value(&root_name, &field_path, Value::Queue(c.clone())); Ok(Value::Queue(c)) }
                    _ => Err(RuntimeError::TypeMismatch("ArrowMut Push requires a compatible collection type".into())),
                }
            }
            ArrowDir::Pop => {
                match &mut collection {
                    Value::List(list) => {
                        let pos = ctx.eval_arrow_pos(list, &self.index)?;
                        let removed = match pos { Some(p) if p < list.len() => list.remove(p), _ => list.pop().ok_or_else(|| RuntimeError::TypeMismatch("Cannot pop from empty list".into()))? };
                        ctx.store_arrow_value(&root_name, &field_path, Value::List(list.clone()));
                        Ok(removed)
                    }
                    Value::HashMap(map) => {
                        let key_val = ctx.eval_expr(&self.index)?;
                        let key = ctx.value_to_string(&key_val)?;
                        let removed = map.remove(&key).ok_or_else(|| RuntimeError::TypeMismatch(format!("Key '{}' not found", key)))?;
                        ctx.store_arrow_value(&root_name, &field_path, Value::HashMap(map.clone()));
                        Ok(removed)
                    }
                    Value::HashSet(set) => {
                        if let Expr::Term = self.index.as_ref() {
                            let elem = set.iter().next().cloned().ok_or_else(|| RuntimeError::TypeMismatch("Cannot pop from empty HashSet".into()))?;
                            set.remove(&elem);
                            ctx.store_arrow_value(&root_name, &field_path, Value::HashSet(set.clone()));
                            Ok(Value::String(elem))
                        } else {
                            let key_val = ctx.eval_expr(&self.index)?;
                            let elem = ctx.value_to_string(&key_val)?;
                            if set.remove(&elem) { ctx.store_arrow_value(&root_name, &field_path, Value::HashSet(set.clone())); Ok(Value::String(elem)) }
                            else { Err(RuntimeError::TypeMismatch(format!("Element '{}' not found", elem))) }
                        }
                    }
                    Value::Stack(stack) => {
                        let removed = stack.pop().ok_or_else(|| RuntimeError::TypeMismatch("Cannot pop from empty Stack".into()))?;
                        ctx.store_arrow_value(&root_name, &field_path, Value::Stack(stack.clone()));
                        Ok(removed)
                    }
                    Value::Queue(queue) => {
                        let removed = queue.pop_front().ok_or_else(|| RuntimeError::TypeMismatch("Cannot dequeue from empty Queue".into()))?;
                        ctx.store_arrow_value(&root_name, &field_path, Value::Queue(queue.clone()));
                        Ok(removed)
                    }
                    _ => Err(RuntimeError::TypeMismatch("ArrowMut Pop requires a compatible collection type".into())),
                }
            }
        }
    }
}

impl ExprEval for ArrowDiscardExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let (root_name, field_path) = ctx.extract_arrow_root(&self.target)?;
        let mut collection = ctx.resolve_arrow_value(&root_name, &field_path)?;
        match &mut collection {
            Value::List(list) => {
                let pos = ctx.eval_arrow_pos(list, &self.index)?;
                match pos { Some(p) if p < list.len() => { list.remove(p); } _ => { list.pop(); } }
                ctx.store_arrow_value(&root_name, &field_path, Value::List(list.clone()));
            }
            Value::HashMap(map) => {
                let key_val = ctx.eval_expr(&self.index)?;
                let key = ctx.value_to_string(&key_val)?;
                map.remove(&key);
                ctx.store_arrow_value(&root_name, &field_path, Value::HashMap(map.clone()));
            }
            Value::HashSet(set) => {
                if let Expr::Term = self.index.as_ref() {
                    let elem = set.iter().next().cloned();
                    if let Some(e) = elem { set.remove(&e); }
                } else {
                    let key_val = ctx.eval_expr(&self.index)?;
                    let elem = ctx.value_to_string(&key_val)?;
                    if !set.remove(&elem) { return Err(RuntimeError::TypeMismatch(format!("Element '{}' not found", elem))); }
                }
                ctx.store_arrow_value(&root_name, &field_path, Value::HashSet(set.clone()));
            }
            Value::Stack(stack) => { stack.pop(); ctx.store_arrow_value(&root_name, &field_path, Value::Stack(stack.clone())); }
            Value::Queue(queue) => { queue.pop_front(); ctx.store_arrow_value(&root_name, &field_path, Value::Queue(queue.clone())); }
            _ => return Err(RuntimeError::TypeMismatch("ArrowDiscard requires compatible collection".into())),
        }
        Ok(Value::Void)
    }
}

impl ExprEval for ArrowTransferExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let (dest_root, dest_path) = ctx.extract_arrow_root(&self.dest)?;
        let (source_root, source_path) = ctx.extract_arrow_root(&self.source)?;
        let mut src_val = ctx.resolve_arrow_value(&source_root, &source_path)?;
        let mut dest_val = ctx.resolve_arrow_value(&dest_root, &dest_path)?;
        match (&mut src_val, &mut dest_val) {
            (Value::List(src), Value::List(dest)) => {
                if let Some(f) = &self.filter {
                    let mut remaining = std::mem::take(src);
                    let mut i = 0;
                    while i < remaining.len() {
                        let prev = ctx.state.insert("_".into(), remaining[i].clone());
                        let cond = ctx.eval_expr(f)?;
                        if prev.is_some() { ctx.state.insert("_".into(), prev.unwrap()); } else { ctx.state.remove("_"); }
                        if cond == Value::Bool(true) { dest.push(remaining.remove(i)); } else { i += 1; }
                    }
                    *src = remaining;
                } else { dest.extend(src.drain(..)); }
            }
            (Value::HashMap(src), Value::HashMap(dest)) => {
                if let Some(f) = &self.filter {
                    let mut remaining = std::mem::take(src);
                    let keys: Vec<String> = remaining.keys().cloned().collect();
                    for key in keys {
                        if let Some(val) = remaining.remove(&key) {
                            let prev = ctx.state.insert("_".into(), val.clone());
                            let cond = ctx.eval_expr(f)?;
                            if prev.is_some() { ctx.state.insert("_".into(), prev.unwrap()); } else { ctx.state.remove("_"); }
                            if cond == Value::Bool(true) { dest.insert(key, val); } else { src.insert(key, val); }
                        }
                    }
                } else { dest.extend(src.drain()); }
            }
            _ => return Err(RuntimeError::TypeMismatch("ArrowTransfer requires matching collection types".into())),
        }
        ctx.store_arrow_value(&dest_root, &dest_path, dest_val);
        ctx.store_arrow_value(&source_root, &source_path, src_val);
        Ok(Value::Void)
    }
}

impl ExprCodegenLLVM for ArrowMutExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%arr".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for ArrowDiscardExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%arr".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for ArrowTransferExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%arr".into(), ty: Type::Void } } }
impl ExprCodegenWebstack for ArrowDiscardExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for ArrowTransferExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
