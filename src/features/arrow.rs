use crate::ast::{ArrowDir, Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowMutExpr {
    pub dir: ArrowDir,
    pub consume: bool,
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
    pub consume: bool,
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
                // D-3: Check InsertAt strategy before default Value dispatch
                if let Some(strategy) = ctx.lookup_insert_strategy(&root_name) {
                    match (&mut collection, strategy) {
                        (Value::List(list), crate::type_universe::InsertStrategy::Prepend) => {
                            list.insert(0, v);
                            let c = list.clone();
                            ctx.store_arrow_value(&root_name, &field_path, Value::List(c.clone()));
                            return Ok(Value::List(c));
                        }
                        (Value::List(list), crate::type_universe::InsertStrategy::Sorted) => {
                            // Sorted insert: find position via element comparison
                            // Value doesn't impl PartialOrd at runtime, so fall through
                            // to append (sorted insert is a compile-time optimization
                            // when the type system can prove ordering).
                            list.push(v);
                            let c = list.clone();
                            ctx.store_arrow_value(&root_name, &field_path, Value::List(c.clone()));
                            return Ok(Value::List(c));
                        }
                        (_, crate::type_universe::InsertStrategy::Custom(fn_name)) => {
                            // Custom insert: call fn_name(collection, value) -> new_collection
                            let result = ctx.call_custom_fn(&fn_name, vec![collection.clone(), v.clone()])?;
                            ctx.store_arrow_value(&root_name, &field_path, result.clone());
                            return Ok(result);
                        }
                        _ => {} // Fall through to default dispatch
                    }
                }
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
                    _ => Err(RuntimeError::TypeMismatch("ArrowMut Push requires a compatible collection type".into())),
                }
            }
            ArrowDir::Pop => {
                // D-3: Check ExtractFrom strategy before default Value dispatch
                if let Some(strategy) = ctx.lookup_extract_strategy(&root_name) {
                    match (&mut collection, strategy) {
                        (Value::List(list), crate::type_universe::ExtractStrategy::Pop) => {
                            let removed = list.pop().ok_or_else(|| RuntimeError::TypeMismatch("Cannot pop from empty list".into()))?;
                            ctx.store_arrow_value(&root_name, &field_path, Value::List(list.clone()));
                            return Ok(removed);
                        }
                        (Value::List(list), crate::type_universe::ExtractStrategy::Shift) => {
                            if list.is_empty() { return Err(RuntimeError::TypeMismatch("Cannot shift from empty list".into())); }
                            let removed = list.remove(0);
                            ctx.store_arrow_value(&root_name, &field_path, Value::List(list.clone()));
                            return Ok(removed);
                        }
                        (_, crate::type_universe::ExtractStrategy::Custom(fn_name)) => {
                            // Custom extract: fn(collection) -> (popped, new_collection)
                            let result = ctx.call_custom_fn(&fn_name, vec![collection.clone()])?;
                            match result {
                                Value::List(pair) if pair.len() == 2 => {
                                    ctx.store_arrow_value(&root_name, &field_path, pair[1].clone());
                                    return Ok(pair[0].clone());
                                }
                                _ => return Err(RuntimeError::TypeMismatch(
                                    "Custom extract function must return (value, new_collection)".into()
                                )),
                            }
                        }
                        _ => {} // Fall through to default dispatch
                    }
                }
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
                            Ok(Value::Bits(elem.as_bytes().to_vec()))
                        } else {
                            let key_val = ctx.eval_expr(&self.index)?;
                            let elem = ctx.value_to_string(&key_val)?;
                            if set.remove(&elem) { ctx.store_arrow_value(&root_name, &field_path, Value::HashSet(set.clone())); Ok(Value::Bits(elem.as_bytes().to_vec())) }
                            else { Err(RuntimeError::TypeMismatch(format!("Element '{}' not found", elem))) }
                        }
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
                        if cond == Value::Bits(vec![1u8]) { dest.push(remaining.remove(i)); } else { i += 1; }
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
                            if cond == Value::Bits(vec![1u8]) { dest.insert(key, val); } else { src.insert(key, val); }
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

impl ExprCodegenLLVM for ArrowMutExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%arr".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for ArrowDiscardExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%arr".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for ArrowTransferExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%arr".into(), ty: Type::Void } } }
impl ExprCodegenWebstack for ArrowDiscardExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for ArrowTransferExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
