// ── Expression / Statement Evaluation ────────────────────────────
//
// This submodule owns eval_expr (the main expression dispatch),
// try_eval_fn_projection, and eval_subtype_projection.
// All follow max 2 nesting with guard clauses and extracted helpers.
//
// Extracted from the monolithic interpreter/mod.rs during Phase 4.

use super::intrinsics::{bits_to_f64, bits_to_i64, f64_to_bits, i64_to_bits, value_as_bool, value_as_f64, value_as_i64};
use super::{Interpreter, RuntimeError, Value};
use crate::ast::*;
use crate::features::arrow::{ArrowDiscardExpr, ArrowMutExpr, ArrowTransferExpr};
use crate::features::binary_op::{BinaryOpExpr, BinaryOpKind};
use crate::features::block::BlockExpr;
use crate::features::call::CallExpr;
use crate::features::collection::{ListLiteralExpr, MapLiteralExpr, SetLiteralExpr, ListIndexExpr, SliceExpr, MultiSliceExpr};
use crate::features::dbvl::DbvlTableExpr;
use crate::features::ellipsis::EllipsisExpr;
use crate::features::field::{FieldAccessExpr, StructInstanceExpr, ObjectLiteralExpr};
use crate::features::literal::LiteralExpr;
use crate::features::pattern::{PatternMatchExpr, MatchExpr};
use crate::features::projection::ProjectionExpr;
use crate::features::sigcall::SigCallExpr;
use crate::features::subtype::SubtypeProjectionExpr;
use crate::features::tuple::{TupleExpr, TupleDestructureExpr};
use crate::features::unary_op::{UnaryOpExpr, UnaryOpKind};
use crate::features::traits::{ExprDispatch, ExprEval};
use regex::Regex;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

impl Interpreter {
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            // Pattern B: delegate to feature struct
            Expr::Literal(lit) => lit.evaluate(self, &ExprDispatch),
            // Legacy scalar variants — keep inline until Phase 14 (variant removal)
            Expr::Decimal(v) => Ok(Value::Bits(i64_to_bits(*v))),
            Expr::IntegerSuffixed(v, _) => Ok(Value::Bits(i64_to_bits(*v))),
            Expr::Float(v) => Ok(Value::Bits(f64_to_bits(*v))),
            Expr::Float64(v) => Ok(Value::Bits(f64_to_bits(*v))),
            Expr::Quoted(v) => Ok(Value::Bits(v.clone())),
            Expr::RegexLiteral(v) => {
                match crate::analysis::dfa::compile_to_dfa(v) {
                    Ok(dfa) => Ok(Value::Regex(dfa)),
                    Err(e) => Err(RuntimeError::TypeMismatch(format!("Invalid regex: {}", e))),
                }
            }
            Expr::Char(v) => Ok(Value::Bits((*v as u32).to_le_bytes().to_vec())),
            Expr::Bool(v) => Ok(Value::Bits(vec![if *v { 1u8 } else { 0u8 }])),
            Expr::Term => self.state.get("term").cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable("term".to_string())),
            Expr::Identifier(name) => self.state.get(name).cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            Expr::AddrOf(inner) => {
                let val = self.eval_expr(inner)?;
                Ok(Value::Ref(Box::new(val)))
            },
            Expr::PriorState(name) => self.prior_state.get(name).cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
            // Legacy binary op variants — delegate through feature struct
            Expr::Add(l, r) => BinaryOpExpr::new(BinaryOpKind::Add, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Sub(l, r) => BinaryOpExpr::new(BinaryOpKind::Sub, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Mul(l, r) => BinaryOpExpr::new(BinaryOpKind::Mul, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Div(l, r) => BinaryOpExpr::new(BinaryOpKind::Div, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Mod(l, r) => BinaryOpExpr::new(BinaryOpKind::Mod, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Eq(l, r) => BinaryOpExpr::new(BinaryOpKind::Eq, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Ne(l, r) => BinaryOpExpr::new(BinaryOpKind::Ne, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Lt(l, r) => BinaryOpExpr::new(BinaryOpKind::Lt, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Le(l, r) => BinaryOpExpr::new(BinaryOpKind::Le, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Gt(l, r) => BinaryOpExpr::new(BinaryOpKind::Gt, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Ge(l, r) => BinaryOpExpr::new(BinaryOpKind::Ge, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::IsType(lit, target) => {
                let val = self.eval_expr(lit)?;
                self.eval_is_type(val, target)
            }
            Expr::FromCheck(le, ty) => {
                let val = self.eval_expr(le)?;
                self.eval_from_check(val, ty)
            }
            Expr::Like(l, r) => {
                let lv = self.eval_expr(l)?;
                let rv = self.eval_expr(r)?;
                self.eval_like(lv, rv)
            }
            Expr::Or(l, r) => BinaryOpExpr::new(BinaryOpKind::Or, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::And(l, r) => BinaryOpExpr::new(BinaryOpKind::And, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::BitAnd(l, r) => BinaryOpExpr::new(BinaryOpKind::BitAnd, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::BitOr(l, r) => BinaryOpExpr::new(BinaryOpKind::BitOr, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::BitXor(l, r) => BinaryOpExpr::new(BinaryOpKind::BitXor, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Shl(l, r) => BinaryOpExpr::new(BinaryOpKind::Shl, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            Expr::Shr(l, r) => BinaryOpExpr::new(BinaryOpKind::Shr, *l.clone(), *r.clone()).evaluate(self, &ExprDispatch),
            // Legacy unary op variants — delegate through feature struct
            Expr::Not(inner) => UnaryOpExpr::new(UnaryOpKind::Not, *inner.clone()).evaluate(self, &ExprDispatch),
            Expr::Neg(inner) => UnaryOpExpr::new(UnaryOpKind::Neg, *inner.clone()).evaluate(self, &ExprDispatch),
            Expr::BitNot(inner) => UnaryOpExpr::new(UnaryOpKind::BitNot, *inner.clone()).evaluate(self, &ExprDispatch),
            // Legacy Arrow variants — delegate through feature structs
            Expr::ArrowMut { dir, target, index, value, consume } => ArrowMutExpr { consume: *consume, 
                dir: dir.clone(), target: target.clone(), index: index.clone(), value: value.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::ArrowDiscard { target, index } => ArrowDiscardExpr {
                target: target.clone(), index: index.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::ArrowTransfer { dest, source, filter, consume } => ArrowTransferExpr { consume: *consume, 
                dest: dest.clone(), source: source.clone(), filter: filter.clone(),
            }.evaluate(self, &ExprDispatch),
            Expr::SigCall { modifier, expr } => SigCallExpr { modifier: modifier.clone(), expr: expr.clone() }.evaluate(self, &ExprDispatch),
            Expr::Ellipsis => EllipsisExpr.evaluate(self, &ExprDispatch),
            Expr::Call(name, args) => {
                // Check if this function has a default watchdog from frgn import
                if let Some((bound, unit, retries, fallback_opt)) = self.frgn_watchdogs.get(name) {
                    let fallback = fallback_opt.as_ref().cloned().unwrap_or(Expr::Decimal(0));
                    let within = Expr::Within {
                        body: Box::new(Expr::Call(name.clone(), args.clone())),
                        bound: *bound,
                        unit: unit.clone(),
                        retries: *retries,
                        fallback: Box::new(fallback),
                    };
                    self.eval_expr(&within)
                } else {
                    crate::features::call::CallExpr::new(name.clone(), args.clone()).evaluate(self, &ExprDispatch)
                }
            }
            Expr::CellCall(callee, args) => {
                let callee_name = match callee.as_ref() {
                    Expr::Identifier(name) => name.clone(),
                    other => return Err(RuntimeError::TypeMismatch(
                        format!("CellCall callee must be an identifier, got {:?}", other)
                    )),
                };
                let cell_def = self.cell_defs.get(&callee_name).ok_or_else(|| {
                    RuntimeError::TypeMismatch(format!("Cell '{}' not found", callee_name))
                })?.clone();
                let arg_values: Result<Vec<Value>, _> = args.iter().map(|a| self.eval_expr(a)).collect();
                self.call_cell(&cell_def, &arg_values?)
            }
            Expr::IntrinsicCall { intrinsic, args } => self.eval_intrinsic(intrinsic, args),
            // Legacy collection variants — delegate through feature structs
            Expr::ListLiteral(elements) =>
                ListLiteralExpr { elements: elements.clone() }.evaluate(self, &ExprDispatch),
            Expr::MapLiteral(entries) =>
                MapLiteralExpr { entries: entries.clone() }.evaluate(self, &ExprDispatch),
            Expr::SetLiteral(entries) =>
                SetLiteralExpr { entries: entries.clone() }.evaluate(self, &ExprDispatch),
            Expr::ListIndex(list_expr, index_expr) =>
                ListIndexExpr { list: list_expr.clone(), index: index_expr.clone() }.evaluate(self, &ExprDispatch),
            Expr::Projection { source, target } => {
                // Function metadata projections don't evaluate the source expression
                // (the source is a defn/inop/txn name, not a state variable)
                if let Some(result) = self.try_eval_fn_projection(source, target) {
                    result
                } else {
                    ProjectionExpr::new(*source.clone(), target.clone()).evaluate(self, &ExprDispatch)
                }
            }
            Expr::FieldAccess(obj_expr, field_name) =>
                FieldAccessExpr { obj: obj_expr.clone(), field: field_name.clone() }.evaluate(self, &ExprDispatch),
            Expr::StructInstance(typename, fields) =>
                StructInstanceExpr { typename: typename.clone(), fields: fields.clone() }.evaluate(self, &ExprDispatch),
            Expr::ObjectLiteral(fields) =>
                ObjectLiteralExpr { fields: fields.clone() }.evaluate(self, &ExprDispatch),
            Expr::PatternMatch { value, variant, fields } =>
                PatternMatchExpr { value: value.clone(), variant: variant.clone(), fields: fields.clone() }.evaluate(self, &ExprDispatch),
            Expr::Concat(l, r) => {
                let left = self.eval_expr(l)?;
                let right = self.eval_expr(r)?;
                match (left, right) {
                    (Value::List(mut a), Value::List(b)) => { a.extend(b); Ok(Value::List(a)) }
                    _ => Err(RuntimeError::TypeMismatch("list concat".into())),
                }
            }
            Expr::Slice { value, start, end, stride, mask } =>
                SliceExpr { value: value.clone(), start: start.clone(), end: end.clone(), stride: stride.clone(), mask: mask.clone() }.evaluate(self, &ExprDispatch),
            Expr::Block(stmts, last) =>
                BlockExpr { stmts: stmts.clone(), last: last.clone() }.evaluate(self, &ExprDispatch),
            Expr::Tuple(exprs) =>
                TupleExpr { exprs: exprs.clone() }.evaluate(self, &ExprDispatch),
            Expr::TupleDestructure(names, expr) =>
                TupleDestructureExpr { names: names.clone(), expr: expr.clone() }.evaluate(self, &ExprDispatch),
            Expr::MultiSlice { value, ops } =>
                MultiSliceExpr { value: value.clone(), ops: ops.clone() }.evaluate(self, &ExprDispatch),
            Expr::Cast(inner, target_ty) => {
                let v = self.eval_expr(inner)?;
                return self.eval_cast(v, target_ty);
            }
            Expr::SubtypeProjection { source, ops } =>
                SubtypeProjectionExpr { source: source.clone(), ops: ops.clone() }.evaluate(self, &ExprDispatch),
            Expr::DbvlTable { path, field_names, key_offsets, schema_name } =>
                DbvlTableExpr { path: path.clone(), field_names: field_names.clone(), key_offsets: key_offsets.clone(), schema_name: schema_name.clone() }.evaluate(self, &ExprDispatch),
            Expr::Match { value, arms } => {
                let match_arms: Vec<crate::features::pattern::MatchArm> = arms.iter().map(|a| crate::features::pattern::MatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.clone(),
                    body: a.body.clone(),
                }).collect();
                MatchExpr { value: value.clone(), arms: match_arms }.evaluate(self, &ExprDispatch)
            }
            // ── Pattern B routing ────────────────────────────────
            Expr::BinaryOp(bop) => bop.evaluate(self, &ExprDispatch),
            Expr::UnaryOp(uop) => uop.evaluate(self, &ExprDispatch),
            Expr::CallExpr(ce) => ce.evaluate(self, &ExprDispatch),
            // DEFERRED: Pattern B variants below are not yet evaluated.
            // They exist in the enum but the feature files have stub evaluate
            // methods. Old variants still handle all cases.
            Expr::ProjectionExpr(_) | Expr::CallExpr(_)
            | Expr::ListLiteralExpr(_) | Expr::MapLiteralExpr(_) | Expr::SetLiteralExpr(_)
            | Expr::SliceExpr(_) | Expr::MultiSliceExpr(_) | Expr::FieldAccessExpr(_)
            | Expr::StructInstanceExpr(_) | Expr::ObjectLiteralExpr(_)
            | Expr::TupleExpr(_) | Expr::TupleDestructureExpr(_) | Expr::EllipsisExpr(_)
            | Expr::ArrowMutExpr(_) | Expr::ArrowDiscardExpr(_) | Expr::ArrowTransferExpr(_)
            | Expr::PatternMatchExpr(_) | Expr::MatchExpr(_) | Expr::BlockExpr(_)
            | Expr::SigCallExpr(_) | Expr::SubtypeProjectionExpr(_) | Expr::DbvlTableExpr(_)
            | Expr::TypeRef(_) => {
                Err(RuntimeError::TypeMismatch("Pattern B variant not yet evaluated".into()))
            }
            // Template/macro nodes
            Expr::TemplateCall { name, .. } => {
                Err(RuntimeError::TypeMismatch(format!("macro not expanded: {}", name)))
            }
            Expr::MacroCall { name, .. } => {
                Err(RuntimeError::TypeMismatch(format!("macro not expanded: {}", name)))
            }
            Expr::Interpolate(_) | Expr::InterpolateExpr(_) => {
                unreachable!("should have been substituted")
            }
            // GPU shared memory — interpreter returns 0 (no GPU simulation)
            Expr::SharedMem(_) => Ok(Value::Bits(i64_to_bits(0))),
            Expr::QuoteBlock { statements, .. } => {
                Ok(Value::Block(statements.clone()))
            }
            // Pipe chains — desugared before this pass
            Expr::PipeChain(_) => unreachable!("PipeChain should have been desugared"),
            Expr::Deref(inner) => {
                let val = self.eval_expr(inner)?;
                match val {
                    Value::Ref(v) => Ok(*v),
                    _ => Err(RuntimeError::TypeMismatch("cannot dereference non-pointer".into())),
                }
            },
            Expr::Within { body, bound, unit: _, retries, fallback } => {
                let saved_counter = self.cycle_counter;
                let saved_budget = self.cycle_budget;
                let max_cycles = saved_counter + bound;
                let mut attempt = 0u64;
                let saved_state = self.state.clone();
                loop {
                    self.cycle_counter = saved_counter;
                    self.cycle_budget = max_cycles;
                    match self.eval_expr(body) {
                        Ok(val) => {
                            self.cycle_budget = saved_budget;
                            break Ok(val);
                        }
                        Err(RuntimeError::Timeout(_)) => {
                            attempt += 1;
                            if attempt > *retries {
                                self.state = saved_state;
                                self.cycle_budget = saved_budget;
                                self.cycle_counter = saved_counter;
                                break self.eval_expr(fallback);
                            }
                            self.state = saved_state.clone();
                        }
                        Err(e) => {
                            self.cycle_budget = saved_budget;
                            break Err(e);
                        }
                    }
                }
            }
            // 2026-07-11: Phase 5 — deferred literal not evaluable in interpreter
            Expr::DeferredLiteral { .. } => Ok(Value::Bits(i64_to_bits(0))),
        }
    }
    fn try_eval_fn_projection(&mut self, source: &Expr, target: &ProjectionTarget) -> Option<Result<Value, RuntimeError>> {
        let name = match source {
            Expr::Identifier(n) => n.clone(),
            _ => return None,
        };
        match target {
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
            | ProjectionTarget::FnSpan => {}
            _ => return None,
        }

        // Look up the declaration
        let meta = if let Some(defn) = self.definitions.get(&name) {
            Some(FnMeta {
                params: defn.parameters.iter().map(|(_, t)| t.clone()).collect(),
                outputs: defn.outputs.clone(),
                span: None, // Definition does not store span
                has_side_effects: false,
            })
        } else if let Some(inop) = self.inop_decls.get(&name) {
            Some(FnMeta {
                params: inop.params.iter().map(|(_, t)| t.clone()).collect(),
                outputs: inop.outputs.clone(),
                span: inop.span,
                has_side_effects: inop.has_side_effects,
            })
        } else if let Some(txn) = self.callable_txns.get(&name) {
            Some(FnMeta {
                params: txn.parameters.iter().map(|(_, t)| t.clone()).collect(),
                outputs: txn.outputs.clone(),
                span: txn.span,
                has_side_effects: !txn.is_reactive,
            })
        } else {
            None
        };

        let meta = match meta {
            Some(m) => m,
            None => return Some(Err(RuntimeError::UndefinedVariable(
                format!("cannot apply Fn projection to '{}': not a defined function, inop, or transaction", name)
            ))),
        };

        Some(match target {
            ProjectionTarget::Address => Ok(Value::Bits(i64_to_bits(0))),  // sentinel; real addr only in codegen
            ProjectionTarget::Name => Ok(Value::Bits(name.into_bytes())),
            ProjectionTarget::Params => {
                let s = meta.params.iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(Value::Bits(s.into_bytes()))
            }
            ProjectionTarget::Returns => {
                let s = meta.outputs.iter()
                    .map(|t| format!("{:?}", t))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(Value::Bits(s.into_bytes()))
            }
            ProjectionTarget::Arity => Ok(Value::Bits(i64_to_bits(meta.params.len() as i64))),
            ProjectionTarget::Loc => {
                let loc = match meta.span {
                    Some(s) => format!("{}:{}", s.line, s.column),
                    None => String::new(),
                };
                Ok(Value::Bits(loc.into_bytes()))
            }
            ProjectionTarget::Doc => Ok(Value::Bits(Vec::new())), // doc comments not stored yet
            ProjectionTarget::Hash => {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                name.hash(&mut hasher);
                Ok(Value::Bits(i64_to_bits(hasher.finish() as i64)))
            }
            ProjectionTarget::Contracts => Ok(Value::Bits(Vec::new())), // contracts not serialized yet
            ProjectionTarget::Module => Ok(Value::Bits(Vec::new())), // module tracking not implemented yet
            ProjectionTarget::IsPure => Ok(Value::Bits(vec![if (!meta.has_side_effects) { 1u8 } else { 0u8 }])),
            ProjectionTarget::FnSpan => {
                let (start, end) = match meta.span {
                    Some(s) => (s.start as i64, s.end as i64),
                    None => (0, 0),
                };
                Ok(Value::Tuple(vec![Value::Bits(i64_to_bits(start)), Value::Bits(i64_to_bits(end))]))
            }
            _ => unreachable!(),
        })
    }

    /// Evaluate a `<:` subtype projection: applies a sequence of ops to a source value.
    pub(crate) fn eval_subtype_projection(&mut self, mut source: Value, ops: &[crate::ast::SubtypeOp]) -> Result<Value, RuntimeError> {
        // 2026-07-11: Expr::String produces Value::Bits; check both representations.
        let source_str = match &source {
            Value::Bits(b) => String::from_utf8(b.clone()).ok(),
            _ => None,
        };
        if let Some(ref s) = source_str {
            for op in ops {
                if let crate::ast::SubtypeOp::Match(pattern_expr) = op {
                    let pattern_val = self.eval_expr(pattern_expr)?;
                    // 2026-07-11: Expr::String produces Value::Bits; convert both representations.
                    let pattern = match pattern_val {
                        Value::Bits(b) => String::from_utf8(b).map_err(|_| RuntimeError::TypeMismatch(
                            "Regex pattern must be a valid UTF-8 string".to_string()
                        ))?,
                        _ => return Err(RuntimeError::TypeMismatch(
                            "Regex pattern must be a string".to_string()
                        )),
                    };
                    let re = Regex::new(&pattern)
                        .map_err(|e| RuntimeError::TypeMismatch(format!("Invalid regex: {}", e)))?;
                    if let Some(caps) = re.captures(s) {
                        let group_count = caps.iter().len().saturating_sub(1);
                        if group_count == 0 {
                            return Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }]));
                        }
                        let mut groups = Vec::new();
                        for i in 1..caps.iter().len() {
                            if let Some(m) = caps.get(i) {
                                groups.push(Value::Bits(m.as_str().to_string().into_bytes()));
                            }
                        }
                        match groups.len() {
                            0 => return Ok(Value::Bits(vec![if true { 1u8 } else { 0u8 }])),
                            1 => return Ok(groups.into_iter().next().unwrap()),
                            _ => return Ok(Value::Tuple(groups)),
                        }
                    } else {
                        return Ok(Value::Bits(vec![0u8]));
                    }
                }
            }
            return Ok(Value::Bits(s.to_string().into()));
        }

        // Check for DbvlTable conversion to collection
        let source = match source {
            Value::DbvlTable(table_ref) => {
                // Check for indexed FILTER on key field
                if let Some(crate::ast::SubtypeOp::Filter(predicate)) = ops.first() {
                    if let Some(literal_key) = try_extract_key_eq(predicate, table_ref.schema_key_index.unwrap_or(0)) {
                        let results = self.resolve_dbvl_key(&table_ref, &literal_key)?;
                        let remaining_ops = &ops[1..];
                        if remaining_ops.is_empty() {
                            if results.len() == 1 {
                                return Ok(results.into_iter().next().unwrap());
                            }
                            return Ok(Value::List(results));
                        }
                        // Apply remaining ops to the resolved list by converting to Value::List
                        // and falling through to the collection processing code below
                        Value::List(results)
                    } else {
                        // Full materialization
                        let mut all_entries = Vec::new();
                        for key in table_ref.key_offsets.keys() {
                            if let Ok(mut results) = self.resolve_dbvl_key(&table_ref, key) {
                                all_entries.append(&mut results);
                            }
                        }
                        Value::List(all_entries)
                    }
                } else {
                    // Full materialization
                    let mut all_entries = Vec::new();
                    for key in table_ref.key_offsets.keys() {
                        if let Ok(mut results) = self.resolve_dbvl_key(&table_ref, key) {
                            all_entries.append(&mut results);
                        }
                    }
                    Value::List(all_entries)
                }
            }
            other => other,
        };

        // Collection projection — source must be a list
        let mut items: Vec<Value> = match source {
            Value::List(list) => list,
            Value::Tuple(tup) => tup,
            Value::HashMap(map) => map.into_values().collect(),
            Value::HashSet(set) => set.into_iter().map(|s| Value::Bits(s.into_bytes())).collect(),
            val => {
                return Err(RuntimeError::TypeMismatch(
                    format!("Subtype projection requires a collection or string, got {:?}", val)
                ));
            }
        };

        // Helper to compare two Values for ordering
        fn cmp_values(a: &Value, b: &Value) -> std::cmp::Ordering {
            match (a, b) {
                (Value::Bits(a_bits), Value::Bits(b_bits)) => {
                    let a_int = value_as_i64(a);
                    let b_int = value_as_i64(b);
                    match (a_int, b_int) {
                        (Some(ai), Some(bi)) => ai.cmp(&bi),
                        _ => {
                            let af = value_as_f64(a);
                            let bf = value_as_f64(b);
                            match (af, bf) {
                                (Some(af), Some(bf)) => af.partial_cmp(&bf).unwrap_or(std::cmp::Ordering::Equal),
                                _ => a_bits.cmp(b_bits),
                            }
                        }
                    }
                }
                _ => std::cmp::Ordering::Equal,
            }
        }

        // Helper to compare Values for equality (used in dedup/group)
        fn values_equal(a: &Value, b: &Value) -> bool {
            match (a, b) {
                (Value::Tuple(av), Value::Tuple(bv)) => av.len() == bv.len() && av.iter().zip(bv.iter()).all(|(x, y)| values_equal(x, y)),
                (a, b) => a == b,
            }
        }

        // Apply each non-terminal op in order
        let mut is_terminal = false;
        for op in ops {
            match op {
                crate::ast::SubtypeOp::Match(_) => {
                    return Err(RuntimeError::TypeMismatch("MATCH can only be used on String sources".into()));
                }
                crate::ast::SubtypeOp::Filter(predicate) => {
                    items = items.into_iter().filter(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        let result = self.eval_expr(predicate).unwrap_or(Value::Bits(vec![0u8]));
                        result == Value::Bits(vec![1u8])
                    }).collect();
                }
                crate::ast::SubtypeOp::Map(transform) => {
                    items = items.into_iter().map(|item| {
                        self.state.insert("_".to_string(), item);
                        self.eval_expr(transform).unwrap_or(Value::Bits(vec![0u8]))
                    }).collect();
                }
                crate::ast::SubtypeOp::Limit(n) => {
                    let take = (*n).min(items.len());
                    items = items.into_iter().take(take).collect();
                }
                crate::ast::SubtypeOp::Skip(n) => {
                    let skip = (*n).min(items.len());
                    items = items.into_iter().skip(skip).collect();
                }
                crate::ast::SubtypeOp::Unique => {
                    let mut seen = Vec::new();
                    items = items.into_iter().filter(|item| {
                        if seen.iter().any(|s: &Value| values_equal(s, item)) {
                            false
                        } else {
                            seen.push(item.clone());
                            true
                        }
                    }).collect();
                }
                crate::ast::SubtypeOp::Sort(key) => {
                    let keys: Vec<Value> = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        self.eval_expr(key).unwrap_or(Value::Bits(i64_to_bits(0)))
                    }).collect();
                    let mut indices: Vec<usize> = (0..items.len()).collect();
                    indices.sort_by(|&a, &b| cmp_values(&keys[a], &keys[b]));
                    items = indices.into_iter().map(|i| items[i].clone()).collect();
                }
                crate::ast::SubtypeOp::Join(other, key) => {
                    let other_val = self.eval_expr(other)?;
                    let other_list = match other_val {
                        Value::List(list) => list,
                        _ => return Err(RuntimeError::TypeMismatch("JOIN requires a List source".into())),
                    };
                    // Compute key for each source item
                    let item_keys: Vec<Value> = items.iter().map(|a| {
                        self.state.insert("_".to_string(), a.clone());
                        self.eval_expr(key).unwrap_or(Value::Bits(i64_to_bits(0)))
                    }).collect();
                    let other_keys: Vec<Value> = other_list.iter().map(|b| {
                        self.state.insert("_".to_string(), b.clone());
                        self.eval_expr(key).unwrap_or(Value::Bits(i64_to_bits(0)))
                    }).collect();

                    let mut result = Vec::new();
                    for (i, a) in items.iter().enumerate() {
                        for (j, b) in other_list.iter().enumerate() {
                            if values_equal(&item_keys[i], &other_keys[j]) {
                                result.push(Value::Tuple(vec![a.clone(), b.clone()]));
                            }
                        }
                    }
                    items = result;
                }
                crate::ast::SubtypeOp::Group(key) => {
                    let mut group_keys: Vec<Value> = Vec::new();
                    let mut group_items: Vec<Vec<Value>> = Vec::new();
                    for item in items {
                        self.state.insert("_".to_string(), item.clone());
                        let k = self.eval_expr(key).unwrap_or(Value::Bits(i64_to_bits(0)));
                        if let Some(pos) = group_keys.iter().position(|gk| values_equal(gk, &k)) {
                            group_items[pos].push(item);
                        } else {
                            group_keys.push(k);
                            group_items.push(vec![item]);
                        }
                    }
                    items = group_keys.into_iter().zip(group_items.into_iter())
                        .map(|(k, v)| Value::Tuple(vec![k, Value::List(v)]))
                        .collect();
                }
                crate::ast::SubtypeOp::Count => {
                    is_terminal = true;
                    items = vec![Value::Bits(i64_to_bits(items.len() as i64))];
                }
                crate::ast::SubtypeOp::Sum(expr) => {
                    is_terminal = true;
                    let total: i64 = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        match self.eval_expr(expr).unwrap_or(Value::Bits(i64_to_bits(0))) {
                            Value::Bits(b) => bits_to_i64(&Value::Bits(b)).unwrap_or(0),
                            _ => 0,
                        }
                    }).sum();
                    items = vec![Value::Bits(i64_to_bits(total))];
                }
                crate::ast::SubtypeOp::Avg(expr) => {
                    is_terminal = true;
                    let len = items.len();
                    if len == 0 {
                        items = vec![Value::Bits(i64_to_bits(0))];
                    } else {
                        let total: f64 = items.iter().map(|item| {
                            self.state.insert("_".to_string(), item.clone());
                            let v = self.eval_expr(expr).unwrap_or(Value::Bits(i64_to_bits(0)));
                            match &v {
                                Value::Bits(b) => {
                                    bits_to_i64(&v).map(|n| n as f64)
                                        .or_else(|_| bits_to_f64(&v))
                                        .unwrap_or(0.0)
                                }
                                _ => 0.0,
                            }
                        }).sum();
                        items = vec![Value::Bits(f64_to_bits(total / len as f64))];
                    }
                }
                crate::ast::SubtypeOp::Min(expr) => {
                    is_terminal = true;
                    let best = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        self.eval_expr(expr).unwrap_or(Value::Bits(i64_to_bits(i64::MAX)))
                    }).min_by(|a, b| cmp_values(a, b));
                    items = vec![best.unwrap_or(Value::Bits(i64_to_bits(0)))];
                }
                crate::ast::SubtypeOp::Max(expr) => {
                    is_terminal = true;
                    let best = items.iter().map(|item| {
                        self.state.insert("_".to_string(), item.clone());
                        self.eval_expr(expr).unwrap_or(Value::Bits(i64_to_bits(i64::MIN)))
                    }).max_by(|a, b| cmp_values(a, b));
                    items = vec![best.unwrap_or(Value::Bits(i64_to_bits(0)))];
                }
            }
        }

        if is_terminal {
            Ok(items.into_iter().next().unwrap_or(Value::Bits(i64_to_bits(0))))
        } else {
            Ok(Value::List(items))
        }
    }

    /// Resolve a key in a lazy-loaded DbvlTable.
    /// Checks cache first, then seeks + parses the line from the file.
    pub(crate) fn resolve_dbvl_key(&mut self, table: &DbvlTableInner, key: &str) -> Result<Vec<Value>, RuntimeError> {
        // Check cache
        if let Some(entry_cache) = self.dbvl_cache.get(&table.path) {
            if let Some(values) = entry_cache.get(key) {
                return Ok(values.clone());
            }
        }

        // Look up key in offset index
        let offsets = match table.key_offsets.get(key) {
            Some(offsets) => offsets.clone(),
            None => return Ok(vec![]), // key not found
        };

        // Read the file
        let content = std::fs::read_to_string(&table.path)
            .map_err(|e| RuntimeError::TypeMismatch(
                format!("Failed to read DBVL file '{}': {}", table.path, e)
            ))?;

        let mut results = Vec::new();
        for &offset in &offsets {
            // Extract line at byte offset
            let rest = &content[offset..];
            let line = rest.lines().next().unwrap_or("");

            // Parse CSV line into values
            let values = parse_csv_line(line);
            let mut field_map = HashMap::new();
            for (i, val) in values.iter().enumerate() {
                let field_name = table.field_names.get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("field_{}", i));
                field_map.insert(field_name, val.clone());
            }

            let entry = match &table.schema_name {
                Some(schema) => {
                    // Try to match field names to schema positions
                    let mut named_fields: Vec<(String, Value)> = Vec::new();
                    for (i, val) in values.iter().enumerate() {
                        if i < table.field_names.len() {
                            named_fields.push((table.field_names[i].clone(), val.clone()));
                        }
                    }
                    Value::Instance {
                        typename: schema.clone(),
                        fields: named_fields.into_iter().collect(),
                    }
                }
                None => Value::HashMap(field_map),
            };
            results.push(entry);
        }

        // Cache the result
        self.dbvl_cache
            .entry(table.path.clone())
            .or_default()
            .insert(key.to_string(), results.clone());

        Ok(results)
    }
}

/// Try to extract a literal key comparison from a FILTER predicate.
/// Detects patterns like `_.key_field == "literal"` or `_.field_0 == "literal"`.
fn try_extract_key_eq(expr: &crate::ast::Expr, key_index: usize) -> Option<String> {
    if let crate::ast::Expr::Eq(left, right) = expr {
        // Check if left side is `_.field_name` or `_.field_N`
        let is_key_field = match left.as_ref() {
            crate::ast::Expr::FieldAccess(obj, field) => {
                matches!(obj.as_ref(), crate::ast::Expr::Identifier(name) if name == "_")
                    && (field == &format!("field_{}", key_index) || field == "field_0")
            }
            _ => false,
        };
        if is_key_field {
            // Extract literal from right side
            match right.as_ref() {
                crate::ast::Expr::Quoted(s) => return Some(String::from_utf8_lossy(s).to_string()),
                crate::ast::Expr::Decimal(n) => return Some(n.to_string()),
                _ => {}
            }
        }
    }
    None
}

/// Parse a single CSV line into Values (lightweight, for lazy dbvl loading)
fn parse_csv_line(line: &str) -> Vec<Value> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                result.push(parse_csv_value(current.trim()));
                current = String::new();
            }
            c => {
                current.push(c);
            }
        }
    }
    result.push(parse_csv_value(current.trim()));

    result
}

/// Parse a single CSV field value into a Brief Value
fn parse_csv_value(s: &str) -> Value {
    // Try int first
    if let Ok(n) = s.parse::<i64>() {
        return Value::Bits(i64_to_bits(n));
    }
    // Try float
    if let Ok(f) = s.parse::<f64>() {
        return Value::Bits(f64_to_bits(f));
    }
    // Bool
    if s == "true" {
        return Value::Bits(vec![1u8]);
    }
    if s == "false" {
        return Value::Bits(vec![0u8]);
    }
    // Default: string
    Value::Bits(s.to_string().into_bytes())
}

