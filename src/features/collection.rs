use crate::ast::{BracketOp, Expr, SliceCoordinate, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct ListLiteralExpr {
    pub elements: Vec<Expr>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct MapLiteralExpr {
    pub entries: Vec<(Expr, Expr)>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SetLiteralExpr {
    pub entries: Vec<Expr>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ListIndexExpr {
    pub list: Box<Expr>,
    pub index: Box<Expr>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SliceExpr {
    pub value: Box<Expr>,
    pub start: Option<Box<Expr>>,
    pub end: Option<Box<Expr>>,
    pub stride: Option<Box<Expr>>,
    pub mask: Option<Box<Expr>>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct MultiSliceExpr {
    pub value: Box<Expr>,
    pub ops: Vec<BracketOp>,
}

impl ExprTypecheck for ListLiteralExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) } }
impl ExprTypecheck for MapLiteralExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) } }
impl ExprTypecheck for SetLiteralExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) } }
impl ExprTypecheck for ListIndexExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) } }
impl ExprTypecheck for SliceExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) } }
impl ExprTypecheck for MultiSliceExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) } }

impl ExprEval for ListLiteralExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let mut values = Vec::new();
        for elem in &self.elements { values.push(ctx.eval_expr(elem)?); }
        Ok(Value::List(values))
    }
}

impl ExprEval for MapLiteralExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let mut map = std::collections::HashMap::new();
        for (key_expr, val_expr) in &self.entries {
            let key_val = ctx.eval_expr(key_expr)?;
            let key_str = ctx.value_to_string(&key_val)?;
            map.insert(key_str, ctx.eval_expr(val_expr)?);
        }
        Ok(Value::HashMap(map))
    }
}

impl ExprEval for SetLiteralExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let mut set = std::collections::HashSet::new();
        for elem in &self.entries {
            let val = ctx.eval_expr(elem)?;
            let s = ctx.value_to_string(&val)?;
            set.insert(s);
        }
        Ok(Value::HashSet(set))
    }
}

impl ExprEval for ListIndexExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let list_val = ctx.eval_expr(&self.list)?;
        let index_val = ctx.eval_expr(&self.index)?;
        match (list_val, index_val) {
            (Value::List(items), Value::Int(idx)) => {
                if idx < 0 || idx as usize >= items.len() {
                    Err(RuntimeError::TypeMismatch("Index out of bounds".into()))
                } else { Ok(items[idx as usize].clone()) }
            }
            (Value::DbvlTable(table), Value::String(key)) => {
                let results = ctx.resolve_dbvl_key(&table, &key)?;
                if results.len() == 1 { Ok(results.into_iter().next().unwrap()) }
                else if results.is_empty() { Err(RuntimeError::TypeMismatch(format!("Key '{}' not found", key))) }
                else { Ok(Value::List(results)) }
            }
            _ => Err(RuntimeError::TypeMismatch("List indexing requires List and Int".into())),
        }
    }
}

impl ExprEval for SliceExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let list_val = ctx.eval_expr(&self.value)?;
        if let Value::String(ref s) = list_val {
            let len = s.len();
            let start_idx = self.start.as_ref().map(|s| match ctx.eval_expr(s) { Ok(Value::Int(n)) => if n < 0 { (len as i64 + n).max(0) as usize } else { n as usize }, _ => len }).unwrap_or(0);
            let end_idx = self.end.as_ref().map(|e| match ctx.eval_expr(e) { Ok(Value::Int(n)) => if n < 0 { (len as i64 + n).max(0) as usize } else { n as usize }, _ => len }).unwrap_or(len);
            return Ok(Value::String(if start_idx < end_idx { s[start_idx..end_idx.min(len)].to_string() } else { String::new() }));
        }
        let list = match list_val { Value::List(vec) => vec, _ => return Err(RuntimeError::TypeMismatch("Cannot slice non-list".into())) };
        let len = list.len();
        let start_idx = self.start.as_ref().map(|s| match ctx.eval_expr(s) { Ok(Value::Int(n)) => if n < 0 { (len as i64 + n) as usize } else { n as usize }, _ => 0 }).unwrap_or(0);
        let end_idx = self.end.as_ref().map(|e| match ctx.eval_expr(e) { Ok(Value::Int(n)) => if n < 0 { (len as i64 + n) as usize } else { n as usize }, _ => len }).unwrap_or(len);
        let stride = self.stride.as_ref().map(|s| match ctx.eval_expr(s) { Ok(Value::Int(n)) => n as usize, _ => 1 }).unwrap_or(1);
        let mut result = Vec::new();
        let mut idx = start_idx;
        while idx < end_idx && idx < len { result.push(list[idx].clone()); if stride == 0 { break; } idx += stride; }
        if let Some(mask_expr) = &self.mask {
            let mut filtered = Vec::new();
            for item in result {
                let prev = ctx.state.insert("_".into(), item.clone());
                let cond = ctx.eval_expr(mask_expr)?;
                if prev.is_some() { ctx.state.insert("_".into(), prev.unwrap()); } else { ctx.state.remove("_"); }
                if cond == Value::Bool(true) { filtered.push(item); }
            }
            result = filtered;
        }
        Ok(Value::List(result))
    }
}

impl ExprEval for MultiSliceExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let base = ctx.eval_expr(&self.value)?;
        let coords: Vec<SliceCoordinate> = self.ops.iter().filter_map(|op| { if let BracketOp::Coord(c) = op { Some(c.clone()) } else { None } }).collect();
        let mut current = if coords.is_empty() { base } else {
            let has_ellipsis = coords.iter().any(|c| matches!(c, SliceCoordinate::Ellipsis));
            if has_ellipsis {
                let dims = Interpreter::list_nesting_depth(&base);
                let expanded = Interpreter::expand_coordinates(&coords, dims)?;
                ctx.apply_multi_slice_coords(&base, &expanded)?
            } else { ctx.apply_multi_slice_coords(&base, &coords)? }
        };
        for op in &self.ops {
            match op {
                BracketOp::Coord(_) => {}
                BracketOp::Stride(stride_expr) => {
                    let list = match current { Value::List(ref items) => items.clone(), _ => return Err(RuntimeError::TypeMismatch("Stride requires list".into())) };
                    match ctx.eval_expr(stride_expr)? { Value::Int(n) if n > 0 => current = Value::List(list.into_iter().step_by(n as usize).collect()), _ => return Err(RuntimeError::TypeMismatch("Stride must be positive Int".into())) }
                }
                BracketOp::Mask(mask_expr) => {
                    let list = match current { Value::List(ref items) => items.clone(), _ => return Err(RuntimeError::TypeMismatch("Mask requires list".into())) };
                    let mut filtered = Vec::new();
                    for item in list {
                        let prev = ctx.state.insert("_".into(), item.clone());
                        let cond = ctx.eval_expr(mask_expr)?;
                        if prev.is_some() { ctx.state.insert("_".into(), prev.unwrap()); } else { ctx.state.remove("_"); }
                        if cond == Value::Bool(true) { filtered.push(item); }
                    }
                    current = Value::List(filtered);
                }
            }
        }
        Ok(current)
    }
}

impl ExprCodegenLLVM for ListLiteralExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::ListLiteral(self.elements.clone()), "")
    }
}
impl ExprCodegenLLVM for MapLiteralExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%map".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for SetLiteralExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%set".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for ListIndexExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::ListIndex(self.list.clone(), self.index.clone()), "")
    }
}
impl ExprCodegenLLVM for SliceExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%slc".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for MultiSliceExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%msl".into(), ty: Type::Void } } }
impl ExprCodegenVHDL for ListLiteralExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenVHDL for MapLiteralExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenVHDL for SetLiteralExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenVHDL for ListIndexExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenVHDL for SliceExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenVHDL for MultiSliceExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenWebstack for ListLiteralExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for MapLiteralExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for SetLiteralExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for ListIndexExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for SliceExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for MultiSliceExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
