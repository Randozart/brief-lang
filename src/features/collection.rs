use crate::ast::{BracketOp, Expr, SliceCoordinate, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::interpreter::{value_as_i64, i64_to_bits, f64_to_bits};

/// Evaluate a mask condition against an item value.
/// Supports both Bool predicates and Regex matching.
fn eval_mask_condition(ctx: &mut Interpreter, mask_expr: &Expr, item_value: &Value) -> Result<bool, RuntimeError> {
    let prev = ctx.state.insert("_".into(), item_value.clone());
    let cond = ctx.eval_expr(mask_expr)?;
    if prev.is_some() { ctx.state.insert("_".into(), prev.unwrap()); } else { ctx.state.remove("_"); }
    match cond {
        Value::Bits(b) if b.len() == 1 => Ok(b[0] != 0),
        Value::Regex(ref dfa) => {
            let s = ctx.value_to_string(item_value)?;
            Ok(crate::analysis::dfa::execute_dfa(dfa, &s).is_some())
        }
        Value::Bits(ref bytes) => {
            let pattern = String::from_utf8_lossy(bytes);
            match crate::analysis::dfa::compile_to_dfa(&pattern) {
                Ok(dfa) => {
                    let s = ctx.value_to_string(item_value)?;
                    Ok(crate::analysis::dfa::execute_dfa(&dfa, &s).is_some())
                }
                Err(e) => Err(RuntimeError::TypeMismatch(format!("Invalid regex: {}", e))),
            }
        }
        _ => Err(RuntimeError::TypeMismatch("Mask expression must evaluate to Bool or Regex".into())),
    }
}

/// Decompose an atomic value to its visual `Char` fragments.
/// `Int(15561)` → `['1', '5', '5', '6', '1']`
fn decompose_atomic_to_chars(val: &Value) -> Option<Vec<char>> {
    match val {
        Value::Bits(b) if b.len() == 8 => {
            value_as_i64(val).map(|n| n.to_string().chars().collect())
        }
        Value::Bits(b) if b.len() == 1 => {
            Some(if b[0] != 0 { "true".to_string() } else { "false".to_string() }.chars().collect())
        }
        Value::Bits(b) if b.len() == 4 => {
            let mut arr = [0u8; 4];
            arr.copy_from_slice(b);
            char::from_u32(u32::from_le_bytes(arr)).map(|c| vec![c])
        }
        Value::Bits(b) => {
            let s = String::from_utf8_lossy(b);
            Some(s.chars().collect())
        }
        _ => None,
    }
}

/// Reconstruct an atomic value from `Char` fragments.
/// `['1', '6', '1']` from original `Int(15561)` → `Int(161)`
fn reconstruct_from_chars(chars: &[char], original: &Value) -> Option<Value> {
    match original {
        Value::Bits(b) if b.len() == 8 => {
            let s: String = chars.iter().collect();
            s.parse::<i64>().ok().map(|n| Value::Bits(i64_to_bits(n)))
        }
        Value::Bits(b) if b.len() == 1 => {
            let s: String = chars.iter().collect();
            match s.as_str() {
                "true" => Some(Value::Bits(vec![1u8])),
                "false" => Some(Value::Bits(vec![0u8])),
                _ => None,
            }
        }
        Value::Bits(b) if b.len() == 4 => {
            chars.first().copied().map(|c| Value::Bits((c as u32).to_le_bytes().to_vec()))
        }
        Value::Bits(b) => {
            let s: String = chars.iter().collect();
            s.parse::<i64>().ok().map(|n| Value::Bits(i64_to_bits(n)))
        }
        _ => None,
    }
}

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

impl ExprTypecheck for ListLiteralExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::int()) } }
impl ExprTypecheck for MapLiteralExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::int()) } }
impl ExprTypecheck for SetLiteralExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::int()) } }
impl ExprTypecheck for ListIndexExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::int()) } }
impl ExprTypecheck for SliceExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::int()) } }
impl ExprTypecheck for MultiSliceExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::int()) } }

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
        let idx = match value_as_i64(&index_val) {
            Some(i) if i >= 0 => i as usize,
            Some(i) => return Err(RuntimeError::TypeMismatch(format!("Negative index {}", i))),
            None => {
                // Non-integer index: try DbvlTable string key
                // 2026-07-11: Expr::String produces Value::Bits; check both.
                let dbvl_key = match &index_val {
                    Value::Bits(b) => String::from_utf8(b.clone()).ok(),
                    _ => None,
                };
                match (&list_val, dbvl_key) {
                    (Value::DbvlTable(table), Some(key)) => {
                        let results = ctx.resolve_dbvl_key(table, &key)?;
                        return if results.len() == 1 { Ok(results.into_iter().next().unwrap()) }
                        else if results.is_empty() { Err(RuntimeError::TypeMismatch(format!("Key '{}' not found", key))) }
                        else { Ok(Value::List(results)) };
                    }
                    _ => return Err(RuntimeError::TypeMismatch("List indexing requires List and Int".into())),
                }
            }
        };
        match list_val {
            Value::List(items) => {
                if idx >= items.len() {
                    Err(RuntimeError::TypeMismatch("Index out of bounds".into()))
                } else { Ok(items[idx].clone()) }
            }
            Value::Tuple(items) => {
                if idx >= items.len() {
                    Err(RuntimeError::TypeMismatch("Tuple index out of bounds".into()))
                } else { Ok(items[idx].clone()) }
            }
            _ => Err(RuntimeError::TypeMismatch("List indexing requires List and Int".into())),
        }
    }
}

impl ExprEval for SliceExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let list_val = ctx.eval_expr(&self.value)?;

        // Helper to extract index from option expression with negative-index support
        let mut eval_idx = |opt: &Option<Box<Expr>>, len: usize| -> Option<usize> {
            match opt {
                Some(s) => match ctx.eval_expr(s) {
                    Ok(v) => value_as_i64(&v).map(|n| if n < 0 { (len as i64 + n).max(0) as usize } else { n as usize }),
                    _ => None,
                },
                None => None,
            }
        };

        // Handle String atomically via Bits
        if let Value::Bits(ref s_bytes) = list_val {
            let s = String::from_utf8_lossy(s_bytes);
            let len = s.len();
            let start_idx = eval_idx(&self.start, len).unwrap_or(0);
            let end_idx = eval_idx(&self.end, len).unwrap_or(len);
            let result = if start_idx < end_idx && start_idx < len { s[start_idx..end_idx.min(len)].to_string() } else { String::new() };
            return Ok(Value::Bits(result.as_bytes().to_vec()));
        }

        // Handle atomic types (Int, Float, Bool, Char) via visual char decomposition
        if let Some(chars) = decompose_atomic_to_chars(&list_val) {
            let len = chars.len();
            let start_idx = eval_idx(&self.start, len).unwrap_or(0);
            let end_idx = eval_idx(&self.end, len).unwrap_or(len);
            let stride = self.stride.as_ref().and_then(|s| ctx.eval_expr(s).ok().and_then(|v| value_as_i64(&v)).map(|n| n as usize)).unwrap_or(1);
            let mut result: Vec<char> = Vec::new();
            let mut idx = start_idx;
            while idx < end_idx && idx < len { result.push(chars[idx]); if stride == 0 { break; } idx += stride; }
            if let Some(mask_expr) = &self.mask {
                let mut filtered = Vec::new();
                for item in result {
                    if eval_mask_condition(ctx, mask_expr, &Value::Bits((item as u32).to_le_bytes().to_vec()))? { filtered.push(item); }
                }
                result = filtered;
            }
            return reconstruct_from_chars(&result, &list_val)
                .ok_or_else(|| RuntimeError::TypeMismatch("Empty slice result".into()));
        }

        let list = match list_val { Value::List(vec) => vec, _ => return Err(RuntimeError::TypeMismatch("Cannot slice non-list".into())) };
        let len = list.len();
        let start_idx = eval_idx(&self.start, len).unwrap_or(0);
        let end_idx = eval_idx(&self.end, len).unwrap_or(len);
        let stride = self.stride.as_ref().and_then(|s| ctx.eval_expr(s).ok().and_then(|v| value_as_i64(&v)).map(|n| n as usize)).unwrap_or(1);
        let mut result = Vec::new();
        let mut idx = start_idx;
        while idx < end_idx && idx < len { result.push(list[idx].clone()); if stride == 0 { break; } idx += stride; }
        if let Some(mask_expr) = &self.mask {
            let mut filtered = Vec::new();
            for item in result {
                if eval_mask_condition(ctx, mask_expr, &item)? { filtered.push(item); }
            }
            result = filtered;
        }
        Ok(Value::List(result))
    }
}

impl ExprEval for MultiSliceExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let base = ctx.eval_expr(&self.value)?;

        // Atomic types (Int, Bits, Float, Bool, Char): decompose, apply ops, reconstruct
        if matches!(base, Value::Bits(_)) {
            // Type-directed desugar: single string coord -> treat as regex filter
            // e.g., 15561["[15]"] is equivalent to 15561[;@"[15]"]
            if self.ops.len() == 1 {
                if let BracketOp::Coord(crate::ast::SliceCoordinate::Index(coord_expr)) = &self.ops[0] {
                    // 2026-07-11: Expr::String produces Value::Bits; check both.
                    let coord_val = ctx.eval_expr(coord_expr)?;
                    let pattern = match &coord_val {
                        Value::Bits(b) => String::from_utf8(b.clone()).ok(),
                        _ => None,
                    };
                    if let Some(ref pattern) = pattern {
                        if let Ok(dfa) = crate::analysis::dfa::compile_to_dfa(pattern) {
                            // Desugar: single string coord on atomic = per-char regex filter
                            // e.g., 15561["[15]"] → decompose to ['1','5','5','6','1'],
                            // keep chars matching [15] → ['1','5','5','1'] → Int(1551)
                            let chars = decompose_atomic_to_chars(&base).unwrap_or_default();
                            let mut filtered = Vec::new();
                            for &c in &chars {
                                let item = Value::Bits((c as u32).to_le_bytes().to_vec());
                                let s = ctx.value_to_string(&item)?;
                                if crate::analysis::dfa::execute_dfa(&dfa, &s).is_some() {
                                    filtered.push(c);
                                }
                            }
                            return reconstruct_from_chars(&filtered, &base)
                                .ok_or_else(|| RuntimeError::TypeMismatch("Empty regex filter result".into()));
                        }
                    }
                }
            }

            let chars = decompose_atomic_to_chars(&base).ok_or_else(|| RuntimeError::TypeMismatch("Cannot decompose atomic value".into()))?;
            let mut current: Vec<Value> = chars.iter().map(|&c| Value::Bits((c as u32).to_le_bytes().to_vec())).collect();

            for op in &self.ops {
                match op {
                    BracketOp::Coord(_) => {}
                    BracketOp::Stride(stride_expr) => {
                        let n = ctx.eval_expr(stride_expr).ok().and_then(|v| value_as_i64(&v)).filter(|n| *n > 0).map(|n| n as usize)
                            .ok_or_else(|| RuntimeError::TypeMismatch("Stride must be positive Int".into()))?;
                        current = current.into_iter().step_by(n).collect();
                    }
                    BracketOp::Mask(mask_expr) => {
                        let mut filtered = Vec::new();
                        for item in current {
                            if eval_mask_condition(ctx, mask_expr, &item)? { filtered.push(item); }
                        }
                        current = filtered;
                    }
                }
            }

            let result_chars: Vec<char> = current.iter().filter_map(|v| {
                if let Value::Bits(b) = v {
                    if b.len() == 4 {
                        let mut arr = [0u8; 4];
                        arr.copy_from_slice(b);
                        char::from_u32(u32::from_le_bytes(arr))
                    } else { None }
                } else { None }
            }).collect();
            return reconstruct_from_chars(&result_chars, &base)
                .ok_or_else(|| RuntimeError::TypeMismatch("Empty bracket result on atomic value".into()));
        }

        // Non-atomic: existing behavior
        use crate::ast::SliceCoordinate;
        let coords: Vec<SliceCoordinate> = self.ops.iter().filter_map(|op| { if let BracketOp::Coord(c) = op { Some(c.clone()) } else { None } }).collect();
        let mut current = if coords.is_empty() { base } else {
            let has_ellipsis = coords.iter().any(|c| matches!(c, SliceCoordinate::Ellipsis));
            if has_ellipsis {
                let dims = Interpreter::list_nesting_depth(&base);
                let expanded = Interpreter::expand_coordinates(&coords, dims)?;
                ctx.apply_multi_slice_coords(&base, &expanded)?
            } else { ctx.apply_multi_slice_coords(&base, &coords)? }
        };

        let is_tuple_type = matches!(current, Value::Tuple(_));
        for op in &self.ops {
            match op {
                BracketOp::Coord(_) => {}
                BracketOp::Stride(stride_expr) => {
                    let items = match current { Value::List(ref items) => items.clone(), Value::Tuple(ref items) => items.clone(), _ => return Err(RuntimeError::TypeMismatch("Stride requires list or tuple".into())) };
                    let n = ctx.eval_expr(stride_expr).ok().and_then(|v| value_as_i64(&v)).filter(|n| *n > 0).map(|n| n as usize)
                        .ok_or_else(|| RuntimeError::TypeMismatch("Stride must be positive Int".into()))?;
                    let stepped: Vec<Value> = items.into_iter().step_by(n).collect();
                    current = if is_tuple_type { Value::Tuple(stepped) } else { Value::List(stepped) };
                }
                BracketOp::Mask(mask_expr) => {
                    let items = match current { Value::List(ref items) => items.clone(), Value::Tuple(ref items) => items.clone(), _ => return Err(RuntimeError::TypeMismatch("Mask requires list or tuple".into())) };
                    let mut filtered = Vec::new();
                    for item in items {
                        if eval_mask_condition(ctx, mask_expr, &item)? { filtered.push(item); }
                    }
                    current = if is_tuple_type { Value::Tuple(filtered) } else { Value::List(filtered) };
                }
            }
        }

        Ok(current)
    }
}

impl ExprCodegenLLVM for ListLiteralExpr {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        // Delegated to expr/collections.rs via emit_expr
        crate::backend::llvm::TypedRegister { name: "%list".into(), ty: crate::ast::Type::int() }
    }
}
impl ExprCodegenLLVM for MapLiteralExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%map".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for SetLiteralExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%set".into(), ty: Type::Void } } }
impl ExprCodegenLLVM for ListIndexExpr {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        crate::backend::llvm::TypedRegister { name: "%idx".into(), ty: crate::ast::Type::int() }
    }
}
impl ExprCodegenLLVM for SliceExpr {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        crate::backend::llvm::TypedRegister { name: "%slc".into(), ty: crate::ast::Type::int() }
    }
}
impl ExprCodegenLLVM for MultiSliceExpr {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        crate::backend::llvm::TypedRegister { name: "%mslc".into(), ty: crate::ast::Type::int() }
    }
}
impl ExprCodegenWebstack for ListLiteralExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for MapLiteralExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for SetLiteralExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for ListIndexExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for SliceExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for MultiSliceExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
