// ── Expression Evaluation ──────────────────────────────────────────────
// 2026-07-12: Phase 3.1 — Flat dispatch, one arm per Expr variant.
// Call with # suffix dispatches to execute_intrinsic().
// Each complex arm is extracted into a named helper for flat code.

use crate::ast::*;
use crate::errors::RuntimeError;
use crate::interpreter::{
    Atom, bool_to_bits, execute_intrinsic, f64_to_bits, i64_to_bits, zero_bits, Value, VirtualHeap,
};
use std::collections::HashMap;
use std::sync::Arc;

/// 2026-08-06 (fix): the interpreter's scoped bindings + the function registry,
/// bundled so eval helpers stay under the Praetor 6-param gate now that
/// user-function dispatch (root-cause fix) threads the registry through.
pub struct EvalScope<'a> {
    pub bindings: &'a mut HashMap<String, Value>,
    pub functions: &'a HashMap<String, crate::interpreter::FunctionDef>,
}

/// Evaluate an expression to a Value.
/// Flat dispatch: one match arm per Expr variant.
pub fn eval_expr(
    expr: &Expr,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,

) -> Result<Value, RuntimeError> {
    match expr {
        // ── Literals ────────────────────────────────────────────
        Expr::Decimal(n) => Ok(i64_to_bits(*n)),
        Expr::TaggedLiteral(n, _) => Ok(i64_to_bits(*n)),
        Expr::Float(f) => Ok(f64_to_bits(*f)),
        // 2026-08-01 (audit): first-class Bool/Char values — the codegen
        // dispatches Print# by protocol category, so the interpreter must carry
        // char/bool-ness to print characters / true-false. as_i64/as_f64
        // promote both (C-style), so arithmetic/comparisons are unchanged.
        Expr::Bool(b) => Ok(Value::Atom(Atom::Bool(*b))),
        Expr::BeginProgram => Ok(Value::Atom(Atom::Bool(true))),
        Expr::Char(c) => Ok(Value::Atom(Atom::Char(*c))),
        Expr::Quoted(bytes) | Expr::TaggedQuotedLiteral(bytes, _) => Ok(Value::bits(bytes.clone())),

        // ── References ──────────────────────────────────────────
        Expr::Identifier(name) => bindings
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedVariable { name: name.clone() }),

        // ── Calls ───────────────────────────────────────────────
        Expr::Call(name, args, _) => eval_call(name, args, heap, bindings, functions),

        // ── Binary operators ─────────────────────────────────────
        Expr::BinaryOp(kind, lhs, rhs) => eval_binary_op(kind, lhs, rhs, heap, &mut EvalScope { bindings: &mut *bindings, functions: functions }),

        // ── Unary operators ──────────────────────────────────────
        Expr::UnaryOp(kind, expr) => eval_unary_op(kind, expr, heap, bindings, functions),

        // ── Block ────────────────────────────────────────────────
        Expr::Block(stmts) => eval_block(stmts, heap, bindings, functions),

        // ── If ───────────────────────────────────────────────────
        Expr::If(cond, then, else_) => eval_if(cond, then, else_, heap, &mut EvalScope { bindings: &mut *bindings, functions: functions }),

        // ── Tuple ────────────────────────────────────────────────
        Expr::Tuple(exprs) => {
            let values: Result<Vec<Value>, _> =
                exprs.iter().map(|e| eval_expr(e, heap, bindings, functions)).collect();
            Ok(Value::product(values?))
        }

        // ── List ─────────────────────────────────────────────────
        Expr::List(exprs) => {
            let values: Result<Vec<Value>, _> =
                exprs.iter().map(|e| eval_expr(e, heap, bindings, functions)).collect();
            Ok(Value::product(values?))
        }

        // ── Field access ─────────────────────────────────────────
        Expr::Field(obj, name) => eval_field(obj, name, heap, bindings, functions),

        // ── Index ────────────────────────────────────────────────
        Expr::Index(obj, index) => eval_index(obj, index, heap, bindings, functions),

        // ── Cast ─────────────────────────────────────────────────
        // 2026-08-01 (audit): the codegen emits a real conversion when the
        // protocol category changes (Bool→Int yields 1/0, Char→Int the code
        // point, Int→Char builds a character), so the interpreter must convert
        // the VALUE, not just reinterpret bits — otherwise `Print#((b as Int))`
        // would print "true" here but "1" in the backend. The target category
        // is resolved through the casting graph (Cast.# universe properties,
        // the same source as codegen) — never by type name. Custom types
        // resolve to "Bit" (identity reinterpretation, matching codegen's
        // fallback).
        Expr::Cast(expr, ty) => {
            let v = eval_expr(expr, heap, bindings, functions)?;
            match target_protocol_category(ty).as_str() {
                "Int" => match v {
                    Value::Atom(Atom::Bool(b)) => Ok(Value::Atom(Atom::Int(if b { 1 } else { 0 }))),
                    Value::Atom(Atom::Char(c)) => Ok(Value::Atom(Atom::Int(c as i64))),
                    _ => Ok(v),
                },
                "Float" => match v {
                    Value::Atom(Atom::Bool(b)) => Ok(Value::Atom(Atom::Float(if b { 1.0 } else { 0.0 }))),
                    Value::Atom(Atom::Char(c)) => Ok(Value::Atom(Atom::Float(c as i64 as f64))),
                    _ => Ok(v),
                },
                "Char" => match v {
                    Value::Atom(Atom::Int(n)) => Ok(Value::Atom(Atom::Char(char::from_u32(n as u32).unwrap_or('?')))),
                    _ => Ok(v),
                },
                _ => Ok(v),
            }
        }

        // ── IsType ───────────────────────────────────────────────
        Expr::IsType(expr, ty) => {
            let val = eval_expr(expr, heap, bindings, functions)?;
            crate::interpreter::casts::eval_is_type(&val, ty)
        }

        // ── Within ───────────────────────────────────────────────
        Expr::Within(expr, _scope) => eval_expr(expr, heap, bindings, functions),

        // ── Match ────────────────────────────────────────────────
        Expr::Match(scrutinee, arms) => eval_match(scrutinee, arms, heap, bindings, functions),

        // ── Lambda ───────────────────────────────────────────────
        // 2026-08-06 (Slice E): capture the current bindings as the closure
        // environment. Application binds params into a fresh env seeded from
        // this snapshot (see eval_call), so captures are by-value and
        // re-entrant.
        Expr::Lambda(params, body) => Ok(Value::Closure {
            params: Arc::new(params.clone()),
            body: Arc::new((**body).clone()),
            env: Arc::new(bindings.clone()),
        }),

        // ── Derivation block ─────────────────────────────────────
        // 2026-08-06 (Slice D): a derivation block is a meta-declaration, not
        // a runtime value; codegen emits `add 0, 0` (void) for it, so the
        // interpreter returns Void. Derivation example verification runs
        // through derive::verify_derivation_assertions, not this path.
        Expr::DerivationBlock(_) => Ok(Value::Void),

        // ── Struct literal ───────────────────────────────────────
        Expr::StructLiteral { type_name, fields } => {
            let _ = type_name;
            let mut names = Vec::with_capacity(fields.len());
            let values: Result<Vec<Value>, _> = fields
                .iter()
                .map(|(name, expr)| {
                    names.push(name.clone());
                    eval_expr(expr, heap, bindings, functions)
                })
                .collect();
            Ok(Value::named_product(values?, names))
        }

        // ── Dereference ──────────────────────────────────────────
        // 2026-07-18: Evaluate inner, expect Value::Ref(wrapped), return *wrapped.
        Expr::Deref(inner) => {
            let val = eval_expr(inner, heap, bindings, functions)?;
            match val {
                Value::Ref(wrapped) => Ok((*wrapped).clone()),
                other => Ok(other),
            }
        }
        // ── Address-of ───────────────────────────────────────────
        // 2026-07-18: Wrap the inner value in Value::Ref to represent a pointer.
        Expr::Consume(inner) => eval_expr(inner, heap, bindings, functions),
        Expr::AddrOf(inner) => {
            let val = eval_expr(inner, heap, bindings, functions)?;
            Ok(Value::Ref(Box::new(val)))
        }

        // ── Field / reflection / method ─────────────────────────
        Expr::Reflect(recv, name, kind) => eval_reflect(recv, name, *kind, heap, &mut EvalScope { bindings: &mut *bindings, functions: functions }),
        Expr::MethodCall(recv, name, args, _) => eval_method_call(recv, name, args, heap, &mut EvalScope { bindings: &mut *bindings, functions: functions }),

        // ── Formatting annotation ────────────────────────────────
        Expr::FormattingAnnotation(_) => Ok(Value::Void),

        // 2026-07-19: Plugin-intercept calls must be resolved by Front plugins
        // before evaluation. The compiler's build path rewrites the lowercase
        // macros (`print!`, `println!`, `get_env!`, `get_env_int!`) at the
        // Parsed stage; this native path keeps direct interpreter use correct
        // (rule #4: the interpreter is the reference) and reports a rename
        // hint for the deprecated PascalCase names.
        Expr::PluginIntercept { name, args, .. } => eval_intercept(name, args, heap, bindings, functions),
        Expr::Exists(_) => { unreachable!("fn? only in stage eval") },
        Expr::Slice { array, start, end, stride } => eval_slice(
            array,
            SliceBounds {
                start: start.as_deref(),
                end: end.as_deref(),
                stride: stride.as_deref(),
            },
            heap,
            bindings,
            functions,
        ),
        // 2026-08-07 (Phase 7): an iterable range value — consumed by
        // `foreach` (SPEC §11.4).
        Expr::Range { start, end, inclusive } => {
            let s = eval_expr(start, heap, bindings, functions)?
                .as_i64()
                .ok_or_else(|| RuntimeError::TypeError {
                    expected: "an integer range bound".into(),
                    found: "non-integer range bound".into(),
                })?;
            let e = eval_expr(end, heap, bindings, functions)?
                .as_i64()
                .ok_or_else(|| RuntimeError::TypeError {
                    expected: "an integer range bound".into(),
                    found: "non-integer range bound".into(),
                })?;
            Ok(Value::Range { start: s, end: e, inclusive: *inclusive })
        }

    }
}

/// Evaluate a function/intrinsic call.
fn eval_call(
    name: &str,
    args: &[Expr],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    let evaluated: Vec<Value> = args
        .iter()
        .map(|a| eval_expr(a, heap, bindings, functions))
        .collect::<Result<Vec<_>, _>>()?;

    if name.ends_with('#') {
        execute_intrinsic(name, &evaluated, heap)
    } else {
        // 2026-08-06 (Slice E): a closure binding applies — params bind into a
        // fresh env seeded from the captured snapshot, body evaluates in it.
        // A non-closure binding is returned as-is (the simplified Call model);
        // an absent name is an error.
        match bindings.get(name) {
            Some(Value::Closure { params, body, env }) => {
                if params.len() != evaluated.len() {
                    return Err(RuntimeError::TypeError {
                        expected: format!("{} arguments", params.len()),
                        found: format!("{} arguments", evaluated.len()),
                    });
                }
                let mut local: HashMap<String, Value> = (**env).clone();
                for (p, v) in params.iter().zip(evaluated.into_iter()) {
                    local.insert(p.clone(), v);
                }
                eval_expr(body, heap, &mut local, functions)
            }
            Some(v) => Ok(v.clone()),
            // 2026-08-06 (fix): a user-defined function (defn/txn) applies with
            // DYNAMIC scoping — its body reads the CALLER's state, so the local
            // env is seeded from the caller's bindings (not a captured
            // snapshot). A `term <value>` inside the body is the return.
            None => match functions.get(name) {
                Some(fn_def) => {
                    if fn_def.parameters.len() != evaluated.len() {
                        return Err(RuntimeError::TypeError {
                            expected: format!("{} arguments", fn_def.parameters.len()),
                            found: format!("{} arguments", evaluated.len()),
                        });
                    }
                    let mut local: HashMap<String, Value> = bindings.clone();
                    for (p, v) in fn_def.parameters.iter().zip(evaluated.into_iter()) {
                        local.insert(p.clone(), v);
                    }
                    let block = Expr::Block(fn_def.body.clone());
                    match eval_expr(&block, heap, &mut local, functions) {
                        Err(RuntimeError::TermReturn(v)) => Ok(v),
                        other => other,
                    }
                }
                None => Err(RuntimeError::UndefinedFunction(name.into())),
            },
        }
    }
}

/// 2026-08-06 (Slice B): Index a value. A `Product` is indexed by element
/// position; raw `Bits` by byte offset (String/Data content). Out-of-bounds
/// or non-indexable values are errors — no placeholder, no silent `zero_bits`.
fn eval_index(
    obj: &Expr,
    index: &Expr,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    let obj_val = eval_expr(obj, heap, bindings, functions)?;
    let idx_val = eval_expr(index, heap, bindings, functions)?;
    // 2026-08-07 (Phase 7): a Bool-vector index is a MASK — `data[mask]`
    // selects the bytes at the true positions (SPEC §16.5). Otherwise the
    // scalar integer-index path below applies.
    if let Some(mask) = bool_mask_from_value(&idx_val) {
        return eval_masked_index(obj_val, &mask);
    }
    let idx = idx_val
        .as_i64()
        .ok_or_else(|| RuntimeError::TypeError {
            expected: "an integer index".into(),
            found: "non-integer index".into(),
        })?;
    if idx < 0 {
        return Err(RuntimeError::TypeError {
            expected: "a non-negative index".into(),
            found: format!("negative index {}", idx),
        });
    }
    let idx = idx as usize;
    match obj_val {
        Value::Product { fields, .. } => fields
            .get(idx)
            .cloned()
            .ok_or_else(|| index_oob(idx, fields.len())),
        Value::Bits(bytes) => bytes.get(idx).copied().map(|b| Value::bits(vec![b])).ok_or_else(|| index_oob(idx, bytes.len())),
        other => Err(RuntimeError::TypeError {
            expected: "an indexable value (list, tuple, or bits)".into(),
            found: format!("{}", describe_value(&other)),
        }),
    }
}

/// 2026-08-07 (Phase 7): a Bool-vector value (a product of Bool atoms) used
/// as a mask index. Returns the mask bits, or None if the value is not a
/// Bool vector.
fn bool_mask_from_value(v: &Value) -> Option<Vec<bool>> {
    let Value::Product { fields, .. } = v else {
        return None;
    };
    let mut bits = Vec::with_capacity(fields.len());
    for f in fields {
        match f {
            Value::Atom(Atom::Bool(b)) => bits.push(*b),
            _ => return None,
        }
    }
    Some(bits)
}

/// 2026-08-07 (Phase 7): masked select — the elements at the true mask
/// positions, in ascending order. Over a `Product` (an Int/Bool vector) the
/// selected FIELDS form a new product; over byte data the selected bytes form
/// a new Bits value. A mask longer than the source truncates (the mask
/// governs), matching the runtime helpers.
fn eval_masked_index(obj_val: Value, mask: &[bool]) -> Result<Value, RuntimeError> {
    match obj_val {
        Value::Bits(bytes) => {
            let selected: Vec<u8> = bytes
                .iter()
                .enumerate()
                .filter(|(i, _)| mask.get(*i).copied().unwrap_or(false))
                .map(|(_, b)| *b)
                .collect();
            Ok(Value::bits(selected))
        }
        Value::Product { fields, names } => {
            let selected: Vec<Value> = fields
                .iter()
                .enumerate()
                .filter(|(i, _)| mask.get(*i).copied().unwrap_or(false))
                .map(|(_, f)| f.clone())
                .collect();
            let selected_names = names.map(|ns| {
                Arc::new(
                    ns.iter()
                        .enumerate()
                        .filter(|(i, _)| mask.get(*i).copied().unwrap_or(false))
                        .map(|(_, n)| n.clone())
                        .collect(),
                )
            });
            Ok(Value::Product { fields: selected, names: selected_names })
        }
        other => Err(RuntimeError::TypeError {
            expected: "byte data (Data) or a vector (list of elements)".into(),
            found: format!("{}", describe_value(&other)),
        }),
    }
}

/// A user-facing out-of-bounds index error (house style: what's wrong + fix).
fn index_oob(index: usize, len: usize) -> RuntimeError {
    RuntimeError::TypeError {
        expected: format!("an index in 0..{}", len),
        found: format!("index {} (length {})", index, len),
    }
}

/// Short value description for error messages (not Debug — stay terse).
fn describe_value(v: &Value) -> String {
    match v {
        Value::Atom(Atom::Int(n)) => format!("integer {}", n),
        Value::Atom(Atom::Float(f)) => format!("float {}", f),
        Value::Atom(Atom::Bool(b)) => format!("boolean {}", b),
        Value::Atom(Atom::Char(c)) => format!("character '{}'", c),
        Value::Bits(_) => "raw bits".into(),
        Value::Product { .. } => "product".into(),
        Value::Void => "void".into(),
        Value::Ref(_) => "reference".into(),
        Value::Closure { .. } => "closure".into(),
        Value::Sum { .. } => "sum".into(),
        Value::Range { .. } => "range".into(),
    }
}

/// 2026-08-06 (Slice D): field access on a struct value. The receiver must be
/// a named product (built by a struct literal); the field index is resolved
/// from the value's declared field-name map — no type registry involved.
fn eval_field(
    obj: &Expr,
    name: &str,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    let recv = eval_expr(obj, heap, bindings, functions)?;
    match recv {
        Value::Product { fields, names } => match names.as_ref().and_then(|ns| ns.iter().position(|n| n == name)) {
            Some(i) => fields.get(i).cloned().ok_or_else(|| field_oob(name, fields.len())),
            None => Err(RuntimeError::TypeError {
                expected: format!("a field named '{}'", name),
                found: describe_value(&Value::Product { fields, names }),
            }),
        },
        other => Err(RuntimeError::TypeError {
            expected: "a struct value".into(),
            found: describe_value(&other),
        }),
    }
}

/// A user-facing field-index error (house style: what's wrong + fix).
fn field_oob(name: &str, len: usize) -> RuntimeError {
    RuntimeError::TypeError {
        expected: format!("a field in the declared struct ({} fields)", len),
        found: format!("field '{}'", name),
    }
}

/// 2026-08-06 (Slice D): a method call. The full method-body dispatch needs
/// the typechecker's member registry (codegen uses obj_members), which the
/// dynamic interpreter does not carry. Evaluate the receiver and args for
/// side effects, then: an intrinsic (`name#`) dispatches with recv+args;
/// otherwise the name is looked up as a binding (mirroring the Call
/// simplification). No match returns Void rather than faking success.
fn eval_method_call(
    recv: &Expr,
    name: &str,
    args: &[Expr],
    heap: &mut VirtualHeap,
    scope: &mut EvalScope,
) -> Result<Value, RuntimeError> {
    let recv_val = eval_expr(recv, heap, scope.bindings, scope.functions)?;
    let arg_vals: Result<Vec<Value>, _> =
        args.iter().map(|a| eval_expr(a, heap, scope.bindings, scope.functions)).collect();
    let arg_vals = arg_vals?;
    if name.ends_with('#') {
        let mut all = vec![recv_val];
        all.extend(arg_vals);
        return execute_intrinsic(name, &all, heap);
    }
    Ok(scope.bindings.get(name).cloned().unwrap_or(Value::Void))
}

/// The three optional slice bounds (`array[start:end:stride]`), borrowed.
struct SliceBounds<'a> {
    start: Option<&'a Expr>,
    end: Option<&'a Expr>,
    stride: Option<&'a Expr>,
}

/// 2026-08-06 (Slice F): `array[start:end:stride]` — Python-style slicing
/// (SPEC §16.5) over raw `Bits` (string/bytes content) and `Product`
/// (list/tuple). Bounds follow the CPython algorithm: negative indices wrap,
/// out-of-range bounds clamp, defaults are start=0/end=len/stride=1, and a
/// negative stride walks the sequence in reverse. Stride 0 is an error.
fn eval_slice(
    array: &Expr,
    bounds: SliceBounds<'_>,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    let arr = eval_expr(array, heap, bindings, functions)?;
    let start = eval_opt_int(bounds.start, heap, bindings, functions)?;
    let end = eval_opt_int(bounds.end, heap, bindings, functions)?;
    let stride = match bounds.stride {
        Some(s) => eval_expr(s, heap, bindings, functions)?
            .as_i64()
            .ok_or_else(|| RuntimeError::TypeError {
                expected: "an integer slice stride".into(),
                found: "non-integer stride".into(),
            })?,
        None => 1,
    };
    match arr {
        Value::Bits(bytes) => {
            let sel = slice_indices(bytes.len(), start, end, stride)?;
            Ok(Value::bits(sel.into_iter().map(|i| bytes[i]).collect()))
        }
        Value::Product { fields, names } => {
            let sel = slice_indices(fields.len(), start, end, stride)?;
            let sliced: Vec<Value> = sel.iter().map(|&i| fields[i].clone()).collect();
            let sliced_names = names.map(|ns| {
                Arc::new(
                    ns.iter()
                        .enumerate()
                        .filter(|(i, _)| sel.contains(i))
                        .map(|(_, n)| n.clone())
                        .collect(),
                )
            });
            Ok(Value::Product { fields: sliced, names: sliced_names })
        }
        other => Err(RuntimeError::TypeError {
            expected: "a sliceable value (string or list)".into(),
            found: describe_value(&other),
        }),
    }
}

/// Evaluate an optional slice bound expression to an optional integer.
fn eval_opt_int(
    opt: Option<&Expr>,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Option<i64>, RuntimeError> {
    match opt {
        Some(e) => eval_expr(e, heap, bindings, functions)?
            .as_i64()
            .map(Some)
            .ok_or_else(|| RuntimeError::TypeError {
                expected: "an integer slice bound".into(),
                found: "non-integer bound".into(),
            }),
        None => Ok(None),
    }
}

/// CPython `slice.indices` normalization: map (start, stop, step) to the
/// concrete index list. Negative bounds wrap by length; positive-step
/// clamps into [0, len], negative-step into [-1, len-1]; step 0 is an error.
fn slice_indices(
    length: usize,
    start: Option<i64>,
    stop: Option<i64>,
    step: i64,
) -> Result<Vec<usize>, RuntimeError> {
    if step == 0 {
        return Err(RuntimeError::TypeError {
            expected: "a non-zero slice stride".into(),
            found: "stride 0".into(),
        });
    }
    let len = length as i64;
    if step > 0 {
        let lo = start.map(|s| wrap_bound(s, len)).unwrap_or(0).clamp(0, len);
        let hi = stop.map(|s| wrap_bound(s, len)).unwrap_or(len).clamp(0, len);
        let mut out = Vec::new();
        let mut i = lo;
        while i < hi {
            out.push(i as usize);
            i += step;
        }
        Ok(out)
    } else {
        let lo = start
            .map(|s| wrap_bound(s, len))
            .unwrap_or(len - 1)
            .clamp(-1, len - 1);
        let hi = stop
            .map(|s| wrap_bound(s, len))
            .unwrap_or(-1)
            .clamp(-1, len - 1);
        let mut out = Vec::new();
        let mut i = lo;
        while i > hi {
            out.push(i as usize);
            i += step;
        }
        Ok(out)
    }
}

/// Negative slice bounds wrap by the sequence length (Python convention).
fn wrap_bound(v: i64, len: i64) -> i64 {
    if v < 0 { v + len } else { v }
}

/// 2026-08-06 (Slice G): value-side reflection (typechecker's D1 table is the
/// authority on kind/target validity; this is the value computation).
/// Runtime targets: `Len` (String char count, product/sum field count),
/// `Absolute` (Int/Float abs). Compile-time targets: `Size`/`Bytes`
/// (field count or byte length), `Type` (the value's category string, or the
/// sum-variant name for a Sum). Unhandled targets return Void.
fn eval_reflect(
    recv: &Expr,
    name: &str,
    kind: ReflectKind,
    heap: &mut VirtualHeap,
    scope: &mut EvalScope,
) -> Result<Value, RuntimeError> {
    let val = eval_expr(recv, heap, scope.bindings, scope.functions)?;
    let ct = matches!(kind, ReflectKind::CompileTime);
    match (name, ct) {
        // 2026-08-01 (B3): String `Len` = UTF8 character count (not bytes).
        // A String is Value::Bits(bytes) or a heap handle Int atom.
        ("Len", false) => match val {
            Value::Bits(_) => {
                let bytes = val.string_bytes(heap).unwrap_or_default();
                let chars = bytes
                    .iter()
                    .filter(|b| (**b & 0xC0) != 0x80)
                    .count();
                Ok(Value::int(chars as i64))
            }
            (Value::Product { fields, .. } | Value::Sum { payload: fields, .. }) => {
                Ok(Value::int(fields.len() as i64))
            }
            _ => Ok(Value::Void),
        },
        ("Absolute", false) => match val {
            Value::Atom(Atom::Int(n)) => Ok(Value::int(n.wrapping_abs())),
            Value::Atom(Atom::Float(f)) => Ok(Value::float(f.abs())),
            _ => Ok(Value::Void),
        },
        ("Size", true) | ("Bytes", true) => match val {
            Value::Bits(bytes) => Ok(Value::int(bytes.len() as i64)),
            (Value::Product { fields, .. } | Value::Sum { payload: fields, .. }) => {
                Ok(Value::int(fields.len() as i64))
            }
            _ => Ok(Value::Void),
        },
        // `Type` (compile-time) is a frozen descriptor: the protocol category
        // code (must match the codegen's type_category_code, rule #4).
        ("Type", true) => Ok(Value::int(reflect_type_code(&val))),
        _ => Ok(Value::Void),
    }
}

/// 2026-08-06 (Phase 8): the semantic category code for `Type` reflection —
/// must match backend/llvm/emit_expr.rs type_category_code.
/// Codes: Int=0, Float=1, Bool=2, Char=3, Bits=4, Product=5, Sum=6,
/// Ref=7, Closure=8, Void=9.
fn reflect_type_code(v: &Value) -> i64 {
    match v {
        Value::Atom(Atom::Int(_)) => 0,
        Value::Atom(Atom::Float(_)) => 1,
        Value::Atom(Atom::Bool(_)) => 2,
        Value::Atom(Atom::Char(_)) => 3,
        Value::Bits(_) => 4,
        Value::Product { .. } => 5,
        Value::Sum { .. } => 6,
        Value::Ref(_) => 7,
        Value::Closure { .. } => 8,
        Value::Range { .. } => 10,
        Value::Void => 9,
    }
}

/// 2026-08-06 (Slice C): Evaluate a match expression. The scrutinee is
/// matched against each arm in order: a pattern match with the arm's bindings
/// in scope, then a `when` guard if present. The first arm whose pattern
/// matches AND guard passes wins; its body runs with the pattern bindings.
/// No arm matching is a non-exhaustive match error.
fn eval_match(
    scrutinee: &Expr,
    arms: &[MatchArm],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    let val = eval_expr(scrutinee, heap, bindings, functions)?;
    for arm in arms {
        let mut arm_bindings = bindings.clone();
        if pattern_match(&arm.pattern, &val, &mut arm_bindings) {
            if let Some(guard) = &arm.guard {
                let gv = eval_expr(guard, heap, &mut arm_bindings, functions)?;
                if !gv.is_true() {
                    continue;
                }
            }
            return eval_expr(&arm.body, heap, &mut arm_bindings, functions);
        }
    }
    Err(RuntimeError::NonExhaustiveMatch(describe_value(&val)))
}

/// Match a pattern against a value, inserting pattern bindings into
/// `bindings`. A failed match may leave partial bindings — callers use a
/// scratch binding map per arm. `EnumVariant` matches the derive CEGIS
/// `Sum { name, payload }` carrier; `Tuple` matches `Product`.
pub fn pattern_match(
    pat: &Pattern,
    val: &Value,
    bindings: &mut HashMap<String, Value>,
) -> bool {
    match pat {
        Pattern::Wildcard => true,
        Pattern::Binding(name) => {
            bindings.insert(name.clone(), val.clone());
            true
        }
        Pattern::Literal(lit) => match literal_pattern_value(lit) {
            Some(lv) => &lv == val,
            None => false,
        },
        // 2026-08-06: `start..end` is a half-open range (SPEC §16.4) — matches
        // n in [start, end). Integer ranges only; float/non-integer values
        // fail to match.
        Pattern::Range(start, end) => {
            let s = match literal_pattern_value(start).and_then(|v| v.as_i64()) {
                Some(n) => n,
                None => return false,
            };
            let e = match literal_pattern_value(end).and_then(|v| v.as_i64()) {
                Some(n) => n,
                None => return false,
            };
            val.as_i64().is_some_and(|n| s <= n && n < e)
        }
        // 2026-08-06 (Phase 7): inclusive `a..=b` — n in [start, end].
        Pattern::RangeInclusive(start, end) => {
            let s = match literal_pattern_value(start).and_then(|v| v.as_i64()) {
                Some(n) => n,
                None => return false,
            };
            let e = match literal_pattern_value(end).and_then(|v| v.as_i64()) {
                Some(n) => n,
                None => return false,
            };
            val.as_i64().is_some_and(|n| s <= n && n <= e)
        }
        Pattern::Tuple(pats) => {
            let items = match val {
                Value::Product { fields, .. } => fields,
                _ => return false,
            };
            items.len() == pats.len()
                && pats
                    .iter()
                    .zip(items.iter())
                    .all(|(p, item)| pattern_match(p, item, bindings))
        }
        Pattern::EnumVariant(name, subpats) => match val {
            Value::Sum { name: cname, payload: fields } => {
                cname == name
                    && fields.len() == subpats.len()
                    && subpats
                        .iter()
                        .zip(fields.iter())
                        .all(|(p, field)| pattern_match(p, field, bindings))
            }
            _ => false,
        },
    }
}

/// The value a literal pattern denotes — no evaluation, only literals.
/// Non-literal expressions in a pattern position fail to match.
fn literal_pattern_value(lit: &Expr) -> Option<Value> {
    match lit {
        Expr::Decimal(n) => Some(Value::int(*n)),
        Expr::TaggedLiteral(n, _) => Some(Value::int(*n)),
        Expr::Float(f) => Some(Value::float(*f)),
        Expr::Bool(b) => Some(Value::bool(*b)),
        Expr::Char(c) => Some(Value::char(*c)),
        Expr::Quoted(bytes) | Expr::TaggedQuotedLiteral(bytes, _) => Some(Value::bits(bytes.clone())),
        _ => None,
    }
}

/// 2026-08-01: Native evaluation for plugin intercepts — the reference
/// implementation of the lowercase macros. The build path rewrites these
/// to stdlib/intrinsic calls at the Parsed stage; this path keeps the
/// interpreter correct when it runs the raw AST. Format strings parse
/// identically to codegen via crate::plugin::print_plugin::parse_format.
fn eval_intercept(
    name: &str,
    args: &[Expr],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    match name {
        "print" | "println" => eval_print_macro(name, args, heap, bindings, functions),
        "get_env" => {
            let key = eval_string_arg(args, heap, bindings, functions)?;
            let val = std::env::var(&key).unwrap_or_default();
            Ok(Value::bits(val.into_bytes()))
        }
        "get_env_int" => {
            let key = eval_string_arg(args, heap, bindings, functions)?;
            let val = std::env::var(&key).unwrap_or_default();
            Ok(i64_to_bits(val.parse::<i64>().unwrap_or(0)))
        }
        // Deprecated PascalCase names — rejected by the typechecker with a
        // rename hint in the build path; mirrored here for interpreter parity.
        "Print" | "PrintLn" => Err(RuntimeError::UnsupportedIntrinsic(format!(
            "'{}!' is deprecated — use the lowercase 'print!' / 'println!' macros",
            name
        ))),
        "GetEnvInt" | "GetEnv" | "GetEnvOrDefault" => Err(RuntimeError::UnsupportedIntrinsic(
            format!(
                "'{}!' is deprecated — use the lowercase 'get_env_int!' / 'get_env!' macros",
                name
            ),
        )),
        _ => Err(RuntimeError::UnsupportedIntrinsic(format!(
            "plugin-intercept {}",
            name
        ))),
    }
}

/// Evaluate the print!/println! macro against runtime values.
fn eval_print_macro(
    name: &str,
    args: &[Expr],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    let is_println = name == "println";
    if args.is_empty() {
        if is_println {
            // 2026-08-01 (audit): the newline is a Char literal printed through
            // the generic Print# path (line termination is the macro's job).
            print_value(&Value::Atom(Atom::Char('\n')), heap)?;
            return Ok(Value::Void);
        }
        return Err(RuntimeError::UnsupportedIntrinsic(
            "print! requires a value or a format-string argument".to_string(),
        ));
    }

    if let Expr::Quoted(fmt) = &args[0] {
        let parts = crate::plugin::print_plugin::parse_format(fmt)
            .map_err(|msg| RuntimeError::UnsupportedIntrinsic(format!("{msg}")))?;
        let value_args = &args[1..];
        let mut next = 0usize;
        for part in &parts {
            match part {
                crate::plugin::print_plugin::FmtPart::Literal(seg) => {
                    print!("{}", String::from_utf8_lossy(seg));
                }
                crate::plugin::print_plugin::FmtPart::Next => {
                    let value = eval_expr(&value_args[next], heap, bindings, functions)?;
                    print_value(&value, heap)?;
                    next += 1;
                }
                crate::plugin::print_plugin::FmtPart::Position(n) => {
                    let value = eval_expr(&value_args[*n], heap, bindings, functions)?;
                    print_value(&value, heap)?;
                }
            }
        }
    } else {
        let value = eval_expr(&args[0], heap, bindings, functions)?;
        print_value(&value, heap)?;
    }

    if is_println {
        print_value(&Value::Atom(Atom::Char('\n')), heap)?;
    }
    Ok(Value::Void)
}

/// Print a runtime value by its representation: Float, integer, character,
/// boolean, or string bits — mirroring the generic `Print#` dispatch in
/// codegen. Bool prints true/false (its natural representation; an explicit
/// cast to Int is what yields 1/0), matching `__print_bool`.
fn print_value(value: &Value, heap: &mut VirtualHeap) -> Result<(), RuntimeError> {
    match value {
        Value::Atom(Atom::Float(f)) => {
            print!("{}", f);
        }
        Value::Atom(Atom::Int(n)) => {
            print!("{}", n);
        }
        Value::Atom(Atom::Bool(b)) => {
            print!("{}", if *b { "true" } else { "false" });
        }
        Value::Atom(Atom::Char(c)) => {
            print!("{}", c);
        }
        Value::Bits(bytes) => {
            execute_intrinsic("Print#", &[Value::bits(bytes.clone())], heap)?;
        }
        _ => {}
    }
    Ok(())
}

/// Resolve a cast target type's protocol category through the casting graph
/// (Cast.# universe properties) — the same source the LLVM backend uses, so
/// the interpreter's value conversion matches codegen. Never type-name
/// matching. The lazily-built default universe covers the bootstrap
/// primitives; custom types resolve to "Bit" (identity reinterpretation).
fn target_protocol_category(ty: &Type) -> String {
    use std::sync::OnceLock;
    static CTX: OnceLock<(
        crate::casting::graph::CastingGraph,
        crate::type_universe::TypeUniverse,
    )> = OnceLock::new();
    let (graph, universe) = CTX.get_or_init(|| {
        (
            crate::casting::graph::CastingGraph::new(),
            crate::type_universe::TypeUniverse::new(),
        )
    });
    graph.type_to_protocol(universe, ty).0
}

/// Evaluate args[0] and decode it as a string (Bits) for env-var keys.
fn eval_string_arg(
    args: &[Expr],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<String, RuntimeError> {
    let value = eval_expr(&args[0], heap, bindings, functions)?;
    match value {
        Value::Bits(bytes) => Ok(String::from_utf8_lossy(&bytes).to_string()),
        other => Err(RuntimeError::TypeError {
            expected: "String".into(),
            found: format!("{other:?}"),
        }),
    }
}

/// Evaluate a binary operation.
fn eval_binary_op(
    kind: &BinaryOpKind,
    lhs: &Expr,
    rhs: &Expr,
    heap: &mut VirtualHeap,
    scope: &mut EvalScope,
) -> Result<Value, RuntimeError> {
    let lv = eval_expr(lhs, heap, scope.bindings, scope.functions)?;
    let rv = eval_expr(rhs, heap, scope.bindings, scope.functions)?;

    match kind {
        BinaryOpKind::Add => {
            // 2026-08-03: `+` is string concat for #String/#Data operands (the
            // Concat op). The backend routes +-on-strings to concat via the
            // string_concat rewrite; the interpreter must match (rule 4).
            if let (Some(a), Some(b)) = (lv.string_bytes(heap), rv.string_bytes(heap)) {
                let mut out = a;
                out.extend_from_slice(&b);
                return Ok(Value::bits(out));
            }
            // Simplified: try arithmetic, fall back to intrinsic
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(i64_to_bits(a.wrapping_add(b))),
                _ => execute_intrinsic("Add#", &[lv, rv], heap),
            }
        }
        BinaryOpKind::Concat => {
            // 2026-08-03: `++`/Concat — string concatenation (interpreter
            // reference for the backend's Concat emitter).
            if let (Some(a), Some(b)) = (lv.string_bytes(heap), rv.string_bytes(heap)) {
                let mut out = a;
                out.extend_from_slice(&b);
                return Ok(Value::bits(out));
            }
            execute_intrinsic("Concat#", &[lv, rv], heap)
        }
        BinaryOpKind::Sub => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(i64_to_bits(a.wrapping_sub(b))),
                _ => execute_intrinsic("Sub#", &[lv, rv], heap),
            }
        }
        BinaryOpKind::Mul => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(i64_to_bits(a.wrapping_mul(b))),
                _ => execute_intrinsic("Mul#", &[lv, rv], heap),
            }
        }
        BinaryOpKind::Eq => {
            // 2026-08-01 (B1): content equality — two Strings are equal when
            // their payload bytes match, not when they live at the same
            // address. string_bytes() derefs both representations (raw Bits
            // and heap handles); when BOTH operands are Strings, compare
            // content. Otherwise fall through to numeric equality (ints,
            // floats, bools). This is the interpreter-first half of B1 (rule
            // #4); the LLVM backend mirrors it in emit_expr.rs.
            match (lv.string_bytes(heap), rv.string_bytes(heap)) {
                (Some(a), Some(b)) => Ok(Value::Atom(Atom::Bool(a == b))),
                _ => Ok(Value::Atom(Atom::Bool(lv.as_i64() == rv.as_i64()))),
            }
        }
        BinaryOpKind::Lt => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(Value::Atom(Atom::Bool(a < b))),
                _ => Ok(Value::Atom(Atom::Bool(false))),
            }
        }
        BinaryOpKind::Gt => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(Value::Atom(Atom::Bool(a > b))),
                _ => Ok(Value::Atom(Atom::Bool(false))),
            }
        }
        BinaryOpKind::Le => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(Value::Atom(Atom::Bool(a <= b))),
                _ => Ok(Value::Atom(Atom::Bool(false))),
            }
        }
        BinaryOpKind::Ge => {
            let la = lv.as_i64();
            let ra = rv.as_i64();
            match (la, ra) {
                (Some(a), Some(b)) => Ok(Value::Atom(Atom::Bool(a >= b))),
                _ => Ok(Value::Atom(Atom::Bool(false))),
            }
        }
        BinaryOpKind::Neq => {
            // 2026-08-01 (B1): content inequality — mirrors the Eq arm above
            // (deref both Strings, compare payload bytes).
            match (lv.string_bytes(heap), rv.string_bytes(heap)) {
                (Some(a), Some(b)) => Ok(Value::Atom(Atom::Bool(a != b))),
                _ => Ok(Value::Atom(Atom::Bool(lv.as_i64() != rv.as_i64()))),
            }
        }
        BinaryOpKind::And => {
            let lb = lv.is_true();
            let rb = rv.is_true();
            Ok(Value::Atom(Atom::Bool(lb && rb)))
        }
        BinaryOpKind::Or => {
            let lb = lv.is_true();
            let rb = rv.is_true();
            Ok(Value::Atom(Atom::Bool(lb || rb)))
        }
        BinaryOpKind::BitAnd | BinaryOpKind::BitOr | BinaryOpKind::BitXor => {
            // 2026-08-01 (B1): #String bitwise defaults — operate on content
            // bytes and return a NEW string of the same length (interpreter
            // half of B1; the backend mirrors it with briv_str_band/bor/bxor).
            // When both operands deref as strings, apply the byte-wise op.
            match (lv.string_bytes(heap), rv.string_bytes(heap)) {
                (Some(a), Some(b)) => {
                    if a.len() != b.len() {
                        return Err(RuntimeError::TypeError {
                            expected: "String bitwise operands of equal length".into(),
                            found: format!("{} vs {} bytes", a.len(), b.len()),
                        });
                    }
                    let result: Vec<u8> = a.iter().zip(b.iter()).map(|(x, y)| match kind {
                        BinaryOpKind::BitAnd => x & y,
                        BinaryOpKind::BitOr => x | y,
                        _ => x ^ y,
                    }).collect();
                    Ok(Value::bits(result))
                }
                _ => {
                    // 2026-08-03: Non-string operands (Int/Ptr) fall through to
                    // the integer bitwise intrinsics (BitAnd#/BitOr#/BitXor#),
                    // restoring the pre-B1 path that B1's string-default match
                    // arm shadowed (ba1d02b4) — it returned bool_to_bits(false)
                    // for every non-string operand, silently zeroing Int & Int.
                    // The backends emit real integer bitwise ops (LLVM and/or/xor);
                    // the interpreter is the reference and must match.
                    execute_intrinsic(&format!("{:?}#", kind), &[lv, rv], heap)
                }
            }
        }
        _ => {
            // Pass through unknown operators as intrinsic calls
            let op_name = format!("{:?}#", kind);
            execute_intrinsic(&op_name, &[lv, rv], heap)
        }
    }
}

/// Evaluate a unary operation.
fn eval_unary_op(
    kind: &UnaryOpKind,
    expr: &Expr,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    let val = eval_expr(expr, heap, bindings, functions)?;
    match kind {
        UnaryOpKind::Neg => {
            let n = val.as_i64().unwrap_or(0);
            Ok(i64_to_bits(n.wrapping_neg()))
        }
        UnaryOpKind::Not => {
            let b = val.is_true();
            Ok(bool_to_bits(!b))
        }
        UnaryOpKind::BitNot => {
            // 2026-08-01 (B1): #String unary bitwise default — complement each
            // content byte, same length (interpreter half; the backend emits
            // briv_str_bnot).
            if let Some(bytes) = val.string_bytes(heap) {
                return Ok(Value::bits(bytes.iter().map(|b| !b).collect()));
            }
            let n = val.as_i64().unwrap_or(0);
            Ok(i64_to_bits(!n))
        }
    }
}

/// Evaluate a block of statements.
fn eval_block(
    stmts: &[Statement],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    let mut result = Value::Void;
    for stmt in stmts {
        result = eval_statement(stmt, heap, bindings, functions)?;
    }
    Ok(result)
}

/// Evaluate an if expression.
fn eval_if(
    cond: &Expr,
    then: &Expr,
    else_: &Option<Box<Expr>>,
    heap: &mut VirtualHeap,
    scope: &mut EvalScope,
) -> Result<Value, RuntimeError> {
    let cv = eval_expr(cond, heap, scope.bindings, scope.functions)?;
    if cv.is_true() {
        eval_expr(then, heap, scope.bindings, scope.functions)
    } else if let Some(else_) = else_ {
        eval_expr(else_, heap, scope.bindings, scope.functions)
    } else {
        Ok(Value::Void)
    }
}

/// Evaluate a statement.
pub fn eval_statement(
    stmt: &Statement,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
    functions: &HashMap<String, crate::interpreter::FunctionDef>,
) -> Result<Value, RuntimeError> {
    match stmt {
        Statement::Let { name, expr, .. } => {
            if let Some(expr) = expr {
                let val = eval_expr(expr, heap, bindings, functions)?;
                bindings.insert(name.clone(), val);
            }
            Ok(Value::Void)
        }
        Statement::Assign(lhs, rhs) => {
            let val = eval_expr(rhs, heap, bindings, functions)?;
            if let Expr::Identifier(name) = lhs {
                bindings.insert(name.clone(), val);
            }
            Ok(Value::Void)
        }
        Statement::FreeHint(name) => {
            // 2026-08-01 (Phase 5): `free x;` — the local is dead; a later read errors.
            bindings.remove(name);
            Ok(Value::Void)
        }
        Statement::KeepHint(_) => Ok(Value::Void),
        Statement::ArrowAssign { target, value, consume } => {
            let val = eval_expr(value, heap, bindings, functions)?;
            if let Some(t) = target.as_ref() {
                if let Expr::Identifier(name) = t.as_ref() {
                    bindings.insert(name.clone(), val);
                }
            }
            if *consume {
                // 2026-08-01 (Phase 3): a consumed value's local is dead after
                // the statement — remove it so a later read errors.
                if let Expr::Identifier(name) = value.as_ref() {
                    bindings.remove(name);
                }
            }
            Ok(Value::Void)
        }
        Statement::Expression(expr) => eval_expr(expr, heap, bindings, functions),
        Statement::Term(val) => {
            match val {
                Some(val) => {
                    // 2026-07-28: Term with value signals early return.
                    let result = eval_expr(val, heap, bindings, functions)?;
                    Err(RuntimeError::TermReturn(result))
                }
                None => {
                    // Term without value is a convergence checkpoint — continue.
                    Ok(Value::Void)
                }
            }
        }
        Statement::Guarded(cond, body) => {
            let cv = eval_expr(cond, heap, bindings, functions)?;
            if cv.is_true() {
                let mut result = Value::Void;
                for stmt in body {
                    result = eval_statement(stmt, heap, bindings, functions)?;
                }
                Ok(result)
            } else {
                Ok(Value::Void)
            }
        }
        Statement::Gate(cond) => {
            // 2026-07-26: Convergence gate — evaluate condition.
            // If false, the caller (txn runner) is expected to retry the body.
            // The compile-time analysis must prove this eventually converges.
            eval_expr(cond, heap, bindings, functions)?;
            Ok(Value::Void)
        }
        Statement::If(cond, then, else_) => {
            let cv = eval_expr(cond, heap, bindings, functions)?;
            if cv.is_true() {
                let mut result = Value::Void;
                for stmt in then {
                    result = eval_statement(stmt, heap, bindings, functions)?;
                }
                Ok(result)
            } else {
                let mut result = Value::Void;
                for stmt in else_ {
                    result = eval_statement(stmt, heap, bindings, functions)?;
                }
                Ok(result)
            }
        }
        Statement::Block(stmts) => {
            let mut result = Value::Void;
            for stmt in stmts {
                result = eval_statement(stmt, heap, bindings, functions)?;
            }
            Ok(result)
        }
        Statement::EndProgram(val) => {
            // 2026-08-06 (endprogram plan): process boundary — a distinct
            // signal from TermReturn (which only ends the transaction). The
            // reactor stops on ProgramExit.
            match val {
                Some(val) => {
                    let result = eval_expr(val, heap, bindings, functions)?;
                    Err(RuntimeError::ProgramExit(result))
                }
                None => Err(RuntimeError::ProgramExit(Value::Void)),
            }
        }
        Statement::Rollback(_) => Ok(Value::Void),
        Statement::MetadataAssignment(_, _) => Ok(Value::Void),
        Statement::InlineAsm { .. } | Statement::InlineDefn(_) | Statement::InlineTxn(_) => Ok(Value::Void),
        Statement::SyncBlock(body) => {
            let mut result = Value::Void;
            for stmt in body {
                result = eval_statement(stmt, heap, bindings, functions)?;
            }
            Ok(result)
        }
        // 2026-08-07 (Phase 7): `foreach(item in list)` — the sole iteration
        // keyword (SPEC §11.4). The iterable is an integer range (`0..=n`) or
        // a collection (a Product for lists/vectors, or Bits for Data).
        Statement::Foreach { item, list, body } => {
            let iterable = eval_expr(list, heap, bindings, functions)?;
            let mut result = Value::Void;
            match iterable {
                Value::Range { start, end, inclusive } => {
                    let last = if inclusive { end } else { end - 1 };
                    if start <= last {
                        for cur in start..=last {
                            bindings.insert(item.clone(), Value::Atom(Atom::Int(cur)));
                            for stmt in body {
                                result = eval_statement(stmt, heap, bindings, functions)?;
                            }
                        }
                    }
                }
                Value::Product { fields, .. } => {
                    for f in &fields {
                        bindings.insert(item.clone(), f.clone());
                        for stmt in body {
                            result = eval_statement(stmt, heap, bindings, functions)?;
                        }
                    }
                }
                Value::Bits(bytes) => {
                    for b in &bytes {
                        bindings.insert(item.clone(), Value::bits(vec![*b]));
                        for stmt in body {
                            result = eval_statement(stmt, heap, bindings, functions)?;
                        }
                    }
                }
                other => {
                    return Err(RuntimeError::TypeError {
                        expected: "an iterable (a range, list, or byte data)".into(),
                        found: format!("{}", describe_value(&other)),
                    });
                }
            }
            Ok(result)
        }
        Statement::TrgBinding { .. } => Ok(Value::Void),
        Statement::Match { .. } => unreachable!("match only in $defn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval1(expr: &Expr) -> Value {
        eval_expr(
            expr,
            &mut VirtualHeap::new(),
            &mut HashMap::new(),
            &HashMap::new(),
        )
        .unwrap()
    }

    /// Evaluate expecting an error — the error-testing twin of `eval1`.
    fn eval1_err(expr: &Expr) -> String {
        eval_expr(
            expr,
            &mut VirtualHeap::new(),
            &mut HashMap::new(),
            &HashMap::new(),
        )
        .err()
        .unwrap_or_else(|| panic!("expected error for {expr:?}"))
        .to_string()
    }

    // 2026-08-01 (audit): #Char/#Bool are first-class values — literals
    // produce Value::Atom(Atom::Char)/Value::Atom(Atom::Bool), and casts convert across categories
    // (mirroring codegen, so Print# prints the same thing on both backends).

    #[test]
    fn test_char_literal_is_char_value() {
        assert_eq!(eval1(&Expr::Char('A')), Value::Atom(Atom::Char('A')));
        assert_eq!(eval1(&Expr::Char('A')).as_i64(), Some(65));
    }

    #[test]
    fn test_bool_literal_is_bool_value() {
        assert_eq!(eval1(&Expr::Bool(true)), Value::Atom(Atom::Bool(true)));
        assert_eq!(eval1(&Expr::Bool(true)).as_i64(), Some(1));
        assert!(eval1(&Expr::Bool(false)).is_true() == false);
    }

    #[test]
    fn test_cast_bool_to_int_is_0_or_1() {
        assert_eq!(
            eval1(&Expr::Cast(Box::new(Expr::Bool(true)), Type::int())),
            Value::Atom(Atom::Int(1))
        );
        assert_eq!(
            eval1(&Expr::Cast(Box::new(Expr::Bool(false)), Type::int())),
            Value::Atom(Atom::Int(0))
        );
    }

    #[test]
    fn test_cast_char_to_int_is_code_point() {
        assert_eq!(
            eval1(&Expr::Cast(Box::new(Expr::Char('A')), Type::int())),
            Value::Atom(Atom::Int(65))
        );
    }

    #[test]
    fn test_cast_int_to_char_builds_character() {
        assert_eq!(
            eval1(&Expr::Cast(Box::new(Expr::Decimal(65)), Type::char_())),
            Value::Atom(Atom::Char('A'))
        );
    }

    #[test]
    fn test_char_promotes_in_arithmetic() {
        let add = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Char('A')),
            Box::new(Expr::Decimal(1)),
        );
        assert_eq!(eval1(&add).as_i64(), Some(66));
    }

    #[test]
    fn test_plus_strings_concat() {
        // 2026-08-03: `+` is string concat for #String operands.
        let expr = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Quoted(b"foo".to_vec())),
            Box::new(Expr::Quoted(b"bar".to_vec())),
        );
        let result = eval1(&expr);
        assert_eq!(result.string_bytes(&VirtualHeap::new()), Some(b"foobar".to_vec()));
    }

    #[test]
    fn test_plus_ints_still_add() {
        // Numeric + stays arithmetic.
        let expr = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Decimal(20)),
            Box::new(Expr::Decimal(22)),
        );
        assert_eq!(eval1(&expr).as_i64(), Some(42));
    }

    #[test]
    fn test_comparison_returns_bool_value() {
        let eq = Expr::BinaryOp(
            BinaryOpKind::Eq,
            Box::new(Expr::Decimal(42)),
            Box::new(Expr::Decimal(42)),
        );
        assert_eq!(eval1(&eq), Value::Atom(Atom::Bool(true)));
        assert!(eval1(&eq).is_true());
    }

    // 2026-08-06 (Slice B): unnamed products, indexing, and membership.

    #[test]
    fn test_tuple_evaluates_to_product() {
        let t = Expr::Tuple(vec![Expr::Decimal(1), Expr::Decimal(2), Expr::Decimal(3)]);
        assert_eq!(
            eval1(&t),
            Value::product(vec![Value::int(1), Value::int(2), Value::int(3)])
        );
    }

    #[test]
    fn test_list_evaluates_to_product() {
        let l = Expr::List(vec![Expr::Bool(true), Expr::Decimal(7)]);
        assert_eq!(
            eval1(&l),
            Value::product(vec![Value::bool(true), Value::int(7)])
        );
    }

    #[test]
    fn test_index_product_element() {
        let l = Expr::List(vec![Expr::Decimal(10), Expr::Decimal(20), Expr::Decimal(30)]);
        let idx = Expr::Index(Box::new(l), Box::new(Expr::Decimal(1)));
        assert_eq!(eval1(&idx).as_i64(), Some(20));
    }

    #[test]
    fn test_index_product_out_of_bounds_errors() {
        let l = Expr::List(vec![Expr::Decimal(10)]);
        let idx = Expr::Index(Box::new(l), Box::new(Expr::Decimal(5)));
        let err = eval1_err(&idx);
        assert!(err.contains("index"), "got: {err}");
    }

    #[test]
    fn test_index_negative_errors() {
        let l = Expr::List(vec![Expr::Decimal(10)]);
        let idx = Expr::Index(Box::new(l), Box::new(Expr::Decimal(-1)));
        assert!(eval1_err(&idx).contains("negative"));
    }

    #[test]
    fn test_index_bits_byte() {
        let s = Expr::Quoted(b"abc".to_vec());
        let idx = Expr::Index(Box::new(s), Box::new(Expr::Decimal(1)));
        assert_eq!(eval1(&idx), Value::bits(vec![b'b']));
    }

    #[test]
    fn test_foreach_range_inclusive_accumulates() {
        // 2026-08-07 (Phase 7): `foreach(i in 0..=5) acc = acc + i` (SPEC
        // §11.4 counted iteration) accumulates 0+1+2+3+4+5 = 15.
        let mut heap = VirtualHeap::new();
        let mut bindings: HashMap<String, Value> = HashMap::new();
        bindings.insert("acc".to_string(), Value::Atom(Atom::Int(0)));
        let foreach = Statement::Foreach {
            item: "i".to_string(),
            list: Box::new(Expr::Range {
                start: Box::new(Expr::Decimal(0)),
                end: Box::new(Expr::Decimal(5)),
                inclusive: true,
            }),
            body: vec![Statement::Assign(
                Expr::Identifier("acc".to_string()),
                Expr::BinaryOp(
                    BinaryOpKind::Add,
                    Box::new(Expr::Identifier("acc".to_string())),
                    Box::new(Expr::Identifier("i".to_string())),
                ),
            )],
        };
        eval_statement(&foreach, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(bindings.get("acc").and_then(|v| v.as_i64()), Some(15));
    }

    #[test]
    fn test_foreach_range_exclusive_stops_short() {
        // `0..5` excludes the end — 0+1+2+3+4 = 10.
        let mut heap = VirtualHeap::new();
        let mut bindings: HashMap<String, Value> = HashMap::new();
        bindings.insert("acc".to_string(), Value::Atom(Atom::Int(0)));
        let foreach = Statement::Foreach {
            item: "i".to_string(),
            list: Box::new(Expr::Range {
                start: Box::new(Expr::Decimal(0)),
                end: Box::new(Expr::Decimal(5)),
                inclusive: false,
            }),
            body: vec![Statement::Assign(
                Expr::Identifier("acc".to_string()),
                Expr::BinaryOp(
                    BinaryOpKind::Add,
                    Box::new(Expr::Identifier("acc".to_string())),
                    Box::new(Expr::Identifier("i".to_string())),
                ),
            )],
        };
        eval_statement(&foreach, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(bindings.get("acc").and_then(|v| v.as_i64()), Some(10));
    }

    #[test]
    fn test_foreach_over_empty_range_skips() {
        let mut heap = VirtualHeap::new();
        let mut bindings: HashMap<String, Value> = HashMap::new();
        bindings.insert("acc".to_string(), Value::Atom(Atom::Int(7)));
        let foreach = Statement::Foreach {
            item: "i".to_string(),
            list: Box::new(Expr::Range {
                start: Box::new(Expr::Decimal(5)),
                end: Box::new(Expr::Decimal(0)),
                inclusive: false,
            }),
            body: vec![Statement::Assign(
                Expr::Identifier("acc".to_string()),
                Expr::Decimal(0),
            )],
        };
        eval_statement(&foreach, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(bindings.get("acc").and_then(|v| v.as_i64()), Some(7));
    }

    #[test]
    fn test_foreach_over_product_iterates_elements() {
        // A collection iterable (a list) binds each element in turn — the
        // reference for `foreach(item in items)`.
        let mut heap = VirtualHeap::new();
        let mut bindings: HashMap<String, Value> = HashMap::new();
        bindings.insert("acc".to_string(), Value::Atom(Atom::Int(0)));
        let foreach = Statement::Foreach {
            item: "x".to_string(),
            list: Box::new(Expr::List(vec![Expr::Decimal(1), Expr::Decimal(2), Expr::Decimal(3)])),
            body: vec![Statement::Assign(
                Expr::Identifier("acc".to_string()),
                Expr::BinaryOp(
                    BinaryOpKind::Add,
                    Box::new(Expr::Identifier("acc".to_string())),
                    Box::new(Expr::Identifier("x".to_string())),
                ),
            )],
        };
        eval_statement(&foreach, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(bindings.get("acc").and_then(|v| v.as_i64()), Some(6));
    }

    #[test]
    fn test_index_non_indexable_errors() {
        let idx = Expr::Index(Box::new(Expr::Decimal(42)), Box::new(Expr::Decimal(0)));
        assert!(eval1_err(&idx).contains("indexable"));
    }

    #[test]
    fn test_mask_index_bits_selects_true_positions() {
        // 2026-08-07 (Phase 7): `data[mask]` — a Bool-vector index selects
        // the bytes at the true positions (SPEC §16.5).
        let data = Expr::Quoted(b"\x01\x02\x03\x04\x05".to_vec());
        let mask = Expr::List(vec![
            Expr::Bool(true), Expr::Bool(false), Expr::Bool(true),
            Expr::Bool(false), Expr::Bool(true),
        ]);
        let idx = Expr::Index(Box::new(data), Box::new(mask));
        assert_eq!(eval1(&idx), Value::bits(vec![0x01, 0x03, 0x05]));
    }

    #[test]
    fn test_mask_index_all_false_yields_empty() {
        let data = Expr::Quoted(b"abc".to_vec());
        let mask = Expr::List(vec![Expr::Bool(false), Expr::Bool(false), Expr::Bool(false)]);
        let idx = Expr::Index(Box::new(data), Box::new(mask));
        assert_eq!(eval1(&idx), Value::bits(vec![]));
    }

    #[test]
    fn test_mask_index_longer_than_data_truncates() {
        // A mask longer than the data truncates (the mask governs), matching
        // the runtime helper.
        let data = Expr::Quoted(b"ab".to_vec());
        let mask = Expr::List(vec![Expr::Bool(true), Expr::Bool(false), Expr::Bool(true)]);
        let idx = Expr::Index(Box::new(data), Box::new(mask));
        assert_eq!(eval1(&idx), Value::bits(vec![b'a']));
    }

    #[test]
    fn test_mask_index_non_bits_source_errors() {
        // The mask path requires a byte-buffer source, mirroring the
        // typechecker/codegen boundary (Data only).
        let data = Expr::Decimal(42);
        let mask = Expr::List(vec![Expr::Bool(true)]);
        let idx = Expr::Index(Box::new(data), Box::new(mask));
        assert!(eval1_err(&idx).contains("byte data"), "got: {}", eval1_err(&idx));
    }

    #[test]
    fn test_mask_index_mixed_mask_is_not_a_mask() {
        // A list with a non-Bool element is NOT a mask — it falls to the
        // scalar path and errors on the non-integer index.
        let data = Expr::Quoted(b"abc".to_vec());
        let mask = Expr::List(vec![Expr::Bool(true), Expr::Decimal(7)]);
        let idx = Expr::Index(Box::new(data), Box::new(mask));
        assert!(eval1_err(&idx).contains("integer index"));
    }

    #[test]
    fn test_mask_index_product_selects_fields() {
        // 2026-08-07 (Phase 7): a Boolean mask over a product (a typed
        // vector) yields a new product of the selected fields — the
        // interpreter reference for `Int[N][mask]` → `List<Int>`.
        let v = Expr::List(vec![Expr::Decimal(10), Expr::Decimal(20), Expr::Decimal(30)]);
        let mask = Expr::List(vec![Expr::Bool(true), Expr::Bool(false), Expr::Bool(true)]);
        let idx = Expr::Index(Box::new(v), Box::new(mask));
        match eval1(&idx) {
            Value::Product { fields, .. } => {
                let vals: Vec<i64> = fields.iter().map(|f| f.as_i64().unwrap()).collect();
                assert_eq!(vals, vec![10, 30]);
            }
            other => panic!("expected a product, got {:?}", other),
        }
    }

    #[test]
    fn test_is_type_int_atom_membership() {
        let e = Expr::IsType(Box::new(Expr::Decimal(42)), Type::int());
        assert_eq!(eval1(&e), Value::Atom(Atom::Bool(true)));
    }

    #[test]
    fn test_is_type_float_atom_rejects_int_type() {
        let e = Expr::IsType(Box::new(Expr::Float(1.5)), Type::int());
        assert_eq!(eval1(&e), Value::Atom(Atom::Bool(false)));
    }

    #[test]
    fn test_print_dispatches_bool_and_char() {
        // Print# is the convenience intrinsic: Bool prints true/false, Char
        // prints the character. Both must dispatch without error.
        let mut heap = VirtualHeap::new();
        let r = execute_intrinsic("Print#", &[Value::Atom(Atom::Bool(true))], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(0));
        let r = execute_intrinsic("Print#", &[Value::Atom(Atom::Char('A'))], &mut heap).unwrap();
        assert_eq!(r.as_i64(), Some(0));
    }

    // 2026-08-06 (Slice C): match with patterns, guards, exhaustiveness.

    fn arm(pattern: Pattern, guard: Option<Expr>, body: i64) -> MatchArm {
        MatchArm {
            pattern,
            guard,
            body: Box::new(Expr::Decimal(body)),
        }
    }

    #[test]
    fn test_match_literal_selects_arm() {
        let m = Expr::Match(
            Box::new(Expr::Decimal(0)),
            vec![
                arm(Pattern::Literal(Expr::Decimal(0)), None, 10),
                arm(Pattern::Wildcard, None, 99),
            ],
        );
        assert_eq!(eval1(&m).as_i64(), Some(10));
    }

    #[test]
    fn test_match_wildcard_fallback() {
        let m = Expr::Match(
            Box::new(Expr::Decimal(5)),
            vec![
                arm(Pattern::Literal(Expr::Decimal(0)), None, 10),
                arm(Pattern::Wildcard, None, 99),
            ],
        );
        assert_eq!(eval1(&m).as_i64(), Some(99));
    }

    #[test]
    fn test_match_guard_false_skips_arm() {
        let m = Expr::Match(
            Box::new(Expr::Decimal(5)),
            vec![
                arm(Pattern::Literal(Expr::Decimal(5)), Some(Expr::Bool(false)), 10),
                arm(Pattern::Wildcard, None, 99),
            ],
        );
        assert_eq!(eval1(&m).as_i64(), Some(99));
    }

    #[test]
    fn test_match_binding_in_body() {
        let m = Expr::Match(
            Box::new(Expr::Decimal(42)),
            vec![MatchArm {
                pattern: Pattern::Binding("x".into()),
                guard: None,
                body: Box::new(Expr::Identifier("x".into())),
            }],
        );
        assert_eq!(eval1(&m).as_i64(), Some(42));
    }

    #[test]
    fn test_match_binding_in_guard() {
        // `x when x > 5 => x` — the guard sees the pattern binding.
        let gt = Expr::BinaryOp(
            BinaryOpKind::Gt,
            Box::new(Expr::Identifier("x".into())),
            Box::new(Expr::Decimal(5)),
        );
        let m = Expr::Match(
            Box::new(Expr::Decimal(10)),
            vec![MatchArm {
                pattern: Pattern::Binding("x".into()),
                guard: Some(gt),
                body: Box::new(Expr::Identifier("x".into())),
            }],
        );
        assert_eq!(eval1(&m).as_i64(), Some(10));
    }

    #[test]
    fn test_match_binding_guard_fails_then_fallback() {
        // x when x > 5 fails for x = 3; wildcard picks up.
        let gt = Expr::BinaryOp(
            BinaryOpKind::Gt,
            Box::new(Expr::Identifier("x".into())),
            Box::new(Expr::Decimal(5)),
        );
        let m = Expr::Match(
            Box::new(Expr::Decimal(3)),
            vec![
                MatchArm {
                    pattern: Pattern::Binding("x".into()),
                    guard: Some(gt),
                    body: Box::new(Expr::Decimal(10)),
                },
                arm(Pattern::Wildcard, None, 99),
            ],
        );
        assert_eq!(eval1(&m).as_i64(), Some(99));
    }

    #[test]
    fn test_match_tuple_pattern_binds_fields() {
        let scrut = Expr::List(vec![Expr::Decimal(1), Expr::Decimal(2)]);
        let m = Expr::Match(
            Box::new(scrut),
            vec![MatchArm {
                pattern: Pattern::Tuple(vec![
                    Pattern::Literal(Expr::Decimal(1)),
                    Pattern::Binding("y".into()),
                ]),
                guard: None,
                body: Box::new(Expr::Identifier("y".into())),
            }],
        );
        assert_eq!(eval1(&m).as_i64(), Some(2));
    }

    #[test]
    fn test_match_range_half_open() {
        // 1..5 is [1, 5): 3 matches, 5 does not.
        let m = |n: i64| {
            Expr::Match(
                Box::new(Expr::Decimal(n)),
                vec![
                    arm(
                        Pattern::Range(Expr::Decimal(1), Expr::Decimal(5)),
                        None,
                        7,
                    ),
                    arm(Pattern::Wildcard, None, 0),
                ],
            )
        };
        assert_eq!(eval1(&m(3)).as_i64(), Some(7));
        assert_eq!(eval1(&m(1)).as_i64(), Some(7));
        assert_eq!(eval1(&m(5)).as_i64(), Some(0));
    }

    #[test]
    fn test_match_range_inclusive() {
        // 2026-08-06 (Phase 7): 1..=5 is [1, 5]: 3 and 5 match, 6 does not.
        let m = |n: i64| {
            Expr::Match(
                Box::new(Expr::Decimal(n)),
                vec![
                    arm(
                        Pattern::RangeInclusive(Expr::Decimal(1), Expr::Decimal(5)),
                        None,
                        7,
                    ),
                    arm(Pattern::Wildcard, None, 0),
                ],
            )
        };
        assert_eq!(eval1(&m(3)).as_i64(), Some(7));
        assert_eq!(eval1(&m(5)).as_i64(), Some(7));
        assert_eq!(eval1(&m(6)).as_i64(), Some(0));
    }

    #[test]
    fn test_match_enum_variant_over_sum() {
        let scrut = Value::Sum { name: "Foo".into(), payload: vec![Value::int(1), Value::int(2)] };
        let m = Expr::Match(
            Box::new(Expr::Identifier("scrut".into())),
            vec![MatchArm {
                pattern: Pattern::EnumVariant(
                    "Foo".into(),
                    vec![Pattern::Wildcard, Pattern::Literal(Expr::Decimal(2))],
                ),
                guard: None,
                body: Box::new(Expr::Decimal(88)),
            }],
        );
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert("scrut".into(), scrut);
        let r = eval_expr(&m, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(r.as_i64(), Some(88));
    }

    #[test]
    fn test_match_non_exhaustive_errors() {
        let m = Expr::Match(
            Box::new(Expr::Decimal(5)),
            vec![arm(Pattern::Literal(Expr::Decimal(0)), None, 10)],
        );
        let err = eval1_err(&m);
        assert!(err.contains("non-exhaustive"), "got: {err}");
    }

    #[test]
    fn test_match_empty_arms_errors() {
        let m = Expr::Match(Box::new(Expr::Decimal(1)), vec![]);
        assert!(eval1_err(&m).contains("non-exhaustive"));
    }

    // 2026-08-06 (Slice D): struct literals, field access, method calls.

    fn person() -> Expr {
        Expr::StructLiteral {
            type_name: "Person".into(),
            fields: vec![
                ("name".into(), Expr::Quoted(b"ada".to_vec())),
                ("age".into(), Expr::Decimal(36)),
            ],
        }
    }

    #[test]
    fn test_struct_literal_is_named_product() {
        let r = eval1(&person());
        match r {
            Value::Product { fields, names } => {
                assert_eq!(fields, vec![Value::bits(b"ada".to_vec()), Value::int(36)]);
                assert_eq!(
                    names.as_ref().map(|n| n.as_ref()),
                    Some(&vec!["name".to_string(), "age".to_string()])
                );
            }
            other => panic!("expected Product, got {other:?}"),
        }
    }

    #[test]
    fn test_field_access_on_struct_literal() {
        let f = Expr::Field(Box::new(person()), "age".into());
        assert_eq!(eval1(&f).as_i64(), Some(36));
    }

    #[test]
    fn test_field_access_on_binding() {
        let f = Expr::Field(Box::new(Expr::Identifier("p".into())), "age".into());
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert(
            "p".into(),
            Value::named_product(vec![Value::int(36)], vec!["age".into()]),
        );
        let r = eval_expr(&f, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(r.as_i64(), Some(36));
    }

    #[test]
    fn test_field_access_unknown_field_errors() {
        let f = Expr::Field(Box::new(person()), "height".into());
        let err = eval1_err(&f);
        assert!(err.contains("field"), "got: {err}");
    }

    #[test]
    fn test_field_access_non_struct_errors() {
        let f = Expr::Field(Box::new(Expr::Decimal(5)), "x".into());
        assert!(eval1_err(&f).contains("struct"));
    }

    #[test]
    fn test_method_call_intrinsic_dispatches_with_receiver() {
        let m = Expr::MethodCall(Box::new(Expr::Decimal(-7)), "Abs#".into(), vec![], None);
        assert_eq!(eval1(&m).as_i64(), Some(7));
    }

    #[test]
    fn test_method_call_binding_lookup() {
        let m = Expr::MethodCall(Box::new(Expr::Decimal(5)), "foo".into(), vec![], None);
        assert_eq!(eval1(&m), Value::Void);
        let m2 = Expr::MethodCall(Box::new(Expr::Decimal(5)), "seven".into(), vec![], None);
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert("seven".into(), Value::int(7));
        let r = eval_expr(&m2, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(r.as_i64(), Some(7));
    }

    // 2026-08-06 (Slice E): closures.

    fn inc_by_one() -> Expr {
        Expr::Lambda(
            vec!["x".into()],
            Box::new(Expr::BinaryOp(
                BinaryOpKind::Add,
                Box::new(Expr::Identifier("x".into())),
                Box::new(Expr::Decimal(1)),
            )),
        )
    }

    #[test]
    fn test_lambda_is_closure_value() {
        assert!(matches!(eval1(&inc_by_one()), Value::Closure { .. }));
    }

    #[test]
    fn test_closure_applies_via_call() {
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert("f".into(), eval1(&inc_by_one()));
        let call = Expr::Call("f".into(), vec![Expr::Decimal(41)], None);
        let r = eval_expr(&call, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(r.as_i64(), Some(42));
    }

    #[test]
    fn test_closure_captures_outer_binding() {
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert("k".into(), Value::int(5));
        // (x) => x + k  — k is captured from the enclosing scope.
        let lam = Expr::Lambda(
            vec!["x".into()],
            Box::new(Expr::BinaryOp(
                BinaryOpKind::Add,
                Box::new(Expr::Identifier("x".into())),
                Box::new(Expr::Identifier("k".into())),
            )),
        );
        let f = eval_expr(&lam, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        bindings.insert("f".into(), f);
        let call = Expr::Call("f".into(), vec![Expr::Decimal(1)], None);
        assert_eq!(eval_expr(&call, &mut heap, &mut bindings, &HashMap::new()).unwrap().as_i64(), Some(6));
    }

    #[test]
    fn test_closure_arity_mismatch_errors() {
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert("f".into(), eval1(&inc_by_one()));
        let call = Expr::Call("f".into(), vec![Expr::Decimal(1), Expr::Decimal(2)], None);
        let err = eval_expr(&call, &mut heap, &mut bindings, &HashMap::new()).err().unwrap().to_string();
        assert!(err.contains("arguments"), "got: {err}");
    }

    #[test]
    fn test_closure_call_unbound_name_errors() {
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        let call = Expr::Call("missing".into(), vec![], None);
        let err = eval_expr(&call, &mut heap, &mut bindings, &HashMap::new()).err().unwrap().to_string();
        assert!(err.contains("undefined function"), "got: {err}");
    }

    #[test]
    fn test_nested_closure_currying() {
        // (x) => (y) => x + y ; f(2)(3) = 5
        let inner = Expr::Lambda(
            vec!["y".into()],
            Box::new(Expr::BinaryOp(
                BinaryOpKind::Add,
                Box::new(Expr::Identifier("x".into())),
                Box::new(Expr::Identifier("y".into())),
            )),
        );
        let outer = Expr::Lambda(vec!["x".into()], Box::new(inner));
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert("f".into(), eval1(&outer));
        let call1 = Expr::Call("f".into(), vec![Expr::Decimal(2)], None);
        let g = eval_expr(&call1, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert!(matches!(g, Value::Closure { .. }));
        bindings.insert("g".into(), g);
        let call2 = Expr::Call("g".into(), vec![Expr::Decimal(3)], None);
        assert_eq!(eval_expr(&call2, &mut heap, &mut bindings, &HashMap::new()).unwrap().as_i64(), Some(5));
    }

    #[test]
    fn test_closure_reentrant_applications() {
        // Applying the same closure twice with different args must not leak
        // state between applications.
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert("f".into(), eval1(&inc_by_one()));
        let c1 = Expr::Call("f".into(), vec![Expr::Decimal(1)], None);
        assert_eq!(eval_expr(&c1, &mut heap, &mut bindings, &HashMap::new()).unwrap().as_i64(), Some(2));
        let c2 = Expr::Call("f".into(), vec![Expr::Decimal(100)], None);
        assert_eq!(eval_expr(&c2, &mut heap, &mut bindings, &HashMap::new()).unwrap().as_i64(), Some(101));
    }

    // 2026-08-06 (fix): user-defined functions apply through the registry.

    fn term_fn(name: &str, params: Vec<&str>, body: Expr) -> crate::interpreter::FunctionDef {
        crate::interpreter::FunctionDef {
            name: name.to_string(),
            parameters: params.into_iter().map(|p| p.to_string()).collect(),
            body: vec![Statement::Term(Some(body))],
        }
    }

    #[test]
    fn test_user_function_call_applies() {
        // `defn my_add(a, b) { term a + b; }` — a call applies it.
        let add = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("a".into())),
            Box::new(Expr::Identifier("b".into())),
        );
        let mut functions = HashMap::new();
        functions.insert("my_add".into(), term_fn("my_add", vec!["a", "b"], add));
        let call = Expr::Call("my_add".into(), vec![Expr::Decimal(2), Expr::Decimal(3)], None);
        let r = eval_expr(&call, &mut VirtualHeap::new(), &mut HashMap::new(), &functions).unwrap();
        assert_eq!(r.as_i64(), Some(5));
    }

    #[test]
    fn test_user_function_reads_caller_state_dynamically() {
        // The defn body reads `k` from the CALLER's bindings (dynamic scoping),
        // not a captured snapshot.
        let body = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x".into())),
            Box::new(Expr::Identifier("k".into())),
        );
        let mut functions = HashMap::new();
        functions.insert("add_k".into(), term_fn("add_k", vec!["x"], body));
        let call = Expr::Call("add_k".into(), vec![Expr::Decimal(5)], None);
        let mut bindings = HashMap::new();
        bindings.insert("k".into(), Value::int(10));
        let r = eval_expr(&call, &mut VirtualHeap::new(), &mut bindings, &functions).unwrap();
        assert_eq!(r.as_i64(), Some(15));
    }

    #[test]
    fn test_user_function_arity_mismatch_errors() {
        let mut functions = HashMap::new();
        functions.insert("f".into(), term_fn("f", vec!["x"], Expr::Identifier("x".into())));
        let call = Expr::Call("f".into(), vec![Expr::Decimal(1), Expr::Decimal(2)], None);
        let err = eval_expr(&call, &mut VirtualHeap::new(), &mut HashMap::new(), &functions)
            .err()
            .unwrap()
            .to_string();
        assert!(err.contains("arguments"), "got: {err}");
    }

    // 2026-08-06 (Slice F): slicing.

    fn slice_expr(array: Expr, start: Option<i64>, end: Option<i64>, stride: Option<i64>) -> Expr {
        let bound = |v: i64| Some(Box::new(Expr::Decimal(v)));
        Expr::Slice {
            array: Box::new(array),
            start: start.and_then(bound),
            end: end.and_then(bound),
            stride: stride.and_then(bound),
        }
    }

    fn abcdef() -> Expr {
        Expr::Quoted(b"abcdef".to_vec())
    }

    fn sliced_bytes(r: Value) -> Vec<u8> {
        match r {
            Value::Bits(b) => b,
            other => panic!("expected Bits, got {other:?}"),
        }
    }

    #[test]
    fn test_slice_string_range() {
        let r = eval1(&slice_expr(abcdef(), Some(1), Some(3), None));
        assert_eq!(sliced_bytes(r), b"bc".to_vec());
    }

    #[test]
    fn test_slice_string_open_bounds() {
        let r = eval1(&slice_expr(abcdef(), None, Some(3), None));
        assert_eq!(sliced_bytes(r), b"abc".to_vec());
        let r = eval1(&slice_expr(abcdef(), Some(3), None, None));
        assert_eq!(sliced_bytes(r), b"def".to_vec());
    }

    #[test]
    fn test_slice_string_positive_stride() {
        let r = eval1(&slice_expr(abcdef(), None, None, Some(2)));
        assert_eq!(sliced_bytes(r), b"ace".to_vec());
    }

    #[test]
    fn test_slice_string_reverse() {
        let r = eval1(&slice_expr(abcdef(), None, None, Some(-1)));
        assert_eq!(sliced_bytes(r), b"fedcba".to_vec());
    }

    #[test]
    fn test_slice_string_negative_index() {
        let r = eval1(&slice_expr(abcdef(), Some(-2), None, None));
        assert_eq!(sliced_bytes(r), b"ef".to_vec());
    }

    #[test]
    fn test_slice_string_bounds_clamp() {
        let r = eval1(&slice_expr(abcdef(), Some(0), Some(100), None));
        assert_eq!(sliced_bytes(r), b"abcdef".to_vec());
        let r = eval1(&slice_expr(abcdef(), Some(100), None, None));
        assert_eq!(sliced_bytes(r), b"".to_vec());
    }

    #[test]
    fn test_slice_list_product() {
        let l = Expr::List(vec![Expr::Decimal(10), Expr::Decimal(20), Expr::Decimal(30)]);
        let r = eval1(&slice_expr(l, Some(1), None, None));
        assert_eq!(r, Value::product(vec![Value::int(20), Value::int(30)]));
    }

    #[test]
    fn test_slice_stride_zero_errors() {
        let e = slice_expr(abcdef(), None, None, Some(0));
        let err = eval1_err(&e);
        assert!(err.contains("stride"), "got: {err}");
    }

    #[test]
    fn test_slice_non_sliceable_errors() {
        let e = slice_expr(Expr::Decimal(5), Some(0), Some(1), None);
        assert!(eval1_err(&e).contains("sliceable"));
    }

    // 2026-08-06 (Slice G): reflection value-side.

    #[test]
    fn test_reflect_len_on_product_is_field_count() {
        let r = Expr::Reflect(
            Box::new(person()),
            "Len".into(),
            ReflectKind::Runtime,
        );
        assert_eq!(eval1(&r).as_i64(), Some(2));
    }

    #[test]
    fn test_reflect_size_on_product_is_field_count() {
        let r = Expr::Reflect(
            Box::new(person()),
            "Size".into(),
            ReflectKind::CompileTime,
        );
        assert_eq!(eval1(&r).as_i64(), Some(2));
    }

    #[test]
    fn test_reflect_bytes_on_string_is_byte_length() {
        let r = Expr::Reflect(
            Box::new(Expr::Quoted(b"abc".to_vec())),
            "Bytes".into(),
            ReflectKind::CompileTime,
        );
        assert_eq!(eval1(&r).as_i64(), Some(3));
    }

    #[test]
    fn test_reflect_absolute_on_int() {
        let r = Expr::Reflect(
            Box::new(Expr::Decimal(-7)),
            "Absolute".into(),
            ReflectKind::Runtime,
        );
        assert_eq!(eval1(&r).as_i64(), Some(7));
    }

    #[test]
    fn test_reflect_type_on_sum_is_sum_category() {
        let r = Expr::Reflect(
            Box::new(Expr::Identifier("c".into())),
            "Type".into(),
            ReflectKind::CompileTime,
        );
        let mut heap = VirtualHeap::new();
        let mut bindings = HashMap::new();
        bindings.insert("c".into(), Value::sum("Some".into(), vec![Value::int(1)]));
        let out = eval_expr(&r, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(out.as_i64(), Some(6));
    }

    #[test]
    fn test_reflect_type_on_int_atom_is_category() {
        let r = Expr::Reflect(
            Box::new(Expr::Decimal(5)),
            "Type".into(),
            ReflectKind::CompileTime,
        );
        let out = eval1(&r);
        assert_eq!(out.as_i64(), Some(0));
    }

    #[test]
    fn test_reflect_unknown_target_is_void() {
        let r = Expr::Reflect(
            Box::new(Expr::Decimal(5)),
            "Bogus".into(),
            ReflectKind::Runtime,
        );
        assert_eq!(eval1(&r), Value::Void);
    }
}
