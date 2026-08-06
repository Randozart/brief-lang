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

/// Evaluate an expression to a Value.
/// Flat dispatch: one match arm per Expr variant.
pub fn eval_expr(
    expr: &Expr,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
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
        Expr::Char(c) => Ok(Value::Atom(Atom::Char(*c))),
        Expr::Quoted(bytes) | Expr::TaggedQuotedLiteral(bytes, _) => Ok(Value::bits(bytes.clone())),

        // ── References ──────────────────────────────────────────
        Expr::Identifier(name) => bindings
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedVariable { name: name.clone() }),

        // ── Calls ───────────────────────────────────────────────
        Expr::Call(name, args, _) => eval_call(name, args, heap, bindings),

        // ── Binary operators ─────────────────────────────────────
        Expr::BinaryOp(kind, lhs, rhs) => eval_binary_op(kind, lhs, rhs, heap, bindings),

        // ── Unary operators ──────────────────────────────────────
        Expr::UnaryOp(kind, expr) => eval_unary_op(kind, expr, heap, bindings),

        // ── Block ────────────────────────────────────────────────
        Expr::Block(stmts) => eval_block(stmts, heap, bindings),

        // ── If ───────────────────────────────────────────────────
        Expr::If(cond, then, else_) => eval_if(cond, then, else_, heap, bindings),

        // ── Tuple ────────────────────────────────────────────────
        Expr::Tuple(exprs) => {
            let values: Result<Vec<Value>, _> =
                exprs.iter().map(|e| eval_expr(e, heap, bindings)).collect();
            Ok(Value::Product(values?))
        }

        // ── List ─────────────────────────────────────────────────
        Expr::List(exprs) => {
            let values: Result<Vec<Value>, _> =
                exprs.iter().map(|e| eval_expr(e, heap, bindings)).collect();
            Ok(Value::Product(values?))
        }

        // ── Field access ─────────────────────────────────────────
        Expr::Field(obj, _name) => eval_expr(obj, heap, bindings),

        // ── Index ────────────────────────────────────────────────
        Expr::Index(obj, index) => eval_index(obj, index, heap, bindings),

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
            let v = eval_expr(expr, heap, bindings)?;
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
            let val = eval_expr(expr, heap, bindings)?;
            crate::interpreter::casts::eval_is_type(&val, ty)
        }

        // ── Within ───────────────────────────────────────────────
        Expr::Within(expr, _scope) => eval_expr(expr, heap, bindings),

        // ── Match ────────────────────────────────────────────────
        Expr::Match(_, arms) => {
            if let Some(first) = arms.first() {
                eval_expr(&first.body, heap, bindings)
            } else {
                Ok(Value::Void)
            }
        }

        // ── Lambda ───────────────────────────────────────────────
        Expr::Lambda(_, _) => Ok(Value::Void),

        // ── Derivation block ─────────────────────────────────────
        Expr::DerivationBlock(_) | Expr::StructLiteral { .. } => Ok(Value::Void),

        // ── Dereference ──────────────────────────────────────────
        // 2026-07-18: Evaluate inner, expect Value::Ref(wrapped), return *wrapped.
        Expr::Deref(inner) => {
            let val = eval_expr(inner, heap, bindings)?;
            match val {
                Value::Ref(wrapped) => Ok((*wrapped).clone()),
                other => Ok(other),
            }
        }
        // ── Address-of ───────────────────────────────────────────
        // 2026-07-18: Wrap the inner value in Value::Ref to represent a pointer.
        Expr::Consume(inner) => eval_expr(inner, heap, bindings),
        Expr::AddrOf(inner) => {
            let val = eval_expr(inner, heap, bindings)?;
            Ok(Value::Ref(Box::new(val)))
        }

        // ── Field / reflection / method ─────────────────────────
        Expr::Field(recv, name) => {
            let _ = (recv, name);
            Ok(Value::Void)
        }
        Expr::Reflect(recv, name, _kind) => {
            // 2026-08-01 (B3): reflection on String values — `Len` (Size prop)
            // = UTF8 character count, `Bytes` = byte length. A String is
            // `Value::Bits(bytes)` (direct) or a heap handle `Value::Atom(Atom::Int(addr))`
            // (`[len: i64][bytes]`). Mirrors the backend's briv_char_len /
            // header-read emission (rule #4: interpreter is the reference).
            let val = eval_expr(recv, heap, bindings)?;
            match name.as_str() {
                "Len" => {
                    let bytes = val
                        .string_bytes(heap)
                        .unwrap_or_default();
                    let chars = bytes
                        .iter()
                        .filter(|b| (**b & 0xC0) != 0x80)
                        .count();
                    Ok(i64_to_bits(chars as i64))
                }
                "Bytes" => {
                    let bytes = val
                        .string_bytes(heap)
                        .unwrap_or_default();
                    Ok(i64_to_bits(bytes.len() as i64))
                }
                _ => Ok(Value::Void),
            }
        }
        Expr::MethodCall(recv, _name, args, _) => {
            eval_expr(recv, heap, bindings)?;
            for a in args {
                eval_expr(a, heap, bindings)?;
            }
            Ok(Value::Void)
        }

        // ── Formatting annotation ────────────────────────────────
        Expr::FormattingAnnotation(_) => Ok(Value::Void),

        // 2026-07-19: Plugin-intercept calls must be resolved by Front plugins
        // before evaluation. The compiler's build path rewrites the lowercase
        // macros (`print!`, `println!`, `get_env!`, `get_env_int!`) at the
        // Parsed stage; this native path keeps direct interpreter use correct
        // (rule #4: the interpreter is the reference) and reports a rename
        // hint for the deprecated PascalCase names.
        Expr::PluginIntercept { name, args, .. } => eval_intercept(name, args, heap, bindings),
        Expr::Exists(_) => { unreachable!("fn? only in stage eval") },
            Expr::Slice { array, .. } => eval_expr(array, heap, bindings),

    }
}

/// Evaluate a function/intrinsic call.
fn eval_call(
    name: &str,
    args: &[Expr],
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let evaluated: Vec<Value> = args
        .iter()
        .map(|a| eval_expr(a, heap, bindings))
        .collect::<Result<Vec<_>, _>>()?;

    if name.ends_with('#') {
        execute_intrinsic(name, &evaluated, heap)
    } else {
        // User function call (simplified: looks up binding)
        bindings
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedVariable { name: name.into() })
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
) -> Result<Value, RuntimeError> {
    let obj_val = eval_expr(obj, heap, bindings)?;
    let idx = eval_expr(index, heap, bindings)?
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
        Value::Product(fields) => fields.get(idx).cloned().ok_or_else(|| index_oob(idx, fields.len())),
        Value::Bits(bytes) => bytes.get(idx).copied().map(|b| Value::bits(vec![b])).ok_or_else(|| index_oob(idx, bytes.len())),
        other => Err(RuntimeError::TypeError {
            expected: "an indexable value (list, tuple, or bits)".into(),
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
        Value::Product(_) => "product".into(),
        Value::Void => "void".into(),
        Value::Ref(_) => "reference".into(),
        Value::Constructor(_, _) => "constructor".into(),
        Value::List(_) => "list".into(),
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
) -> Result<Value, RuntimeError> {
    match name {
        "print" | "println" => eval_print_macro(name, args, heap, bindings),
        "get_env" => {
            let key = eval_string_arg(args, heap, bindings)?;
            let val = std::env::var(&key).unwrap_or_default();
            Ok(Value::bits(val.into_bytes()))
        }
        "get_env_int" => {
            let key = eval_string_arg(args, heap, bindings)?;
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
                    let value = eval_expr(&value_args[next], heap, bindings)?;
                    print_value(&value, heap)?;
                    next += 1;
                }
                crate::plugin::print_plugin::FmtPart::Position(n) => {
                    let value = eval_expr(&value_args[*n], heap, bindings)?;
                    print_value(&value, heap)?;
                }
            }
        }
    } else {
        let value = eval_expr(&args[0], heap, bindings)?;
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
) -> Result<String, RuntimeError> {
    let value = eval_expr(&args[0], heap, bindings)?;
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
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let lv = eval_expr(lhs, heap, bindings)?;
    let rv = eval_expr(rhs, heap, bindings)?;

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
) -> Result<Value, RuntimeError> {
    let val = eval_expr(expr, heap, bindings)?;
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
) -> Result<Value, RuntimeError> {
    let mut result = Value::Void;
    for stmt in stmts {
        result = eval_statement(stmt, heap, bindings)?;
    }
    Ok(result)
}

/// Evaluate an if expression.
fn eval_if(
    cond: &Expr,
    then: &Expr,
    else_: &Option<Box<Expr>>,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    let cv = eval_expr(cond, heap, bindings)?;
    if cv.is_true() {
        eval_expr(then, heap, bindings)
    } else if let Some(else_) = else_ {
        eval_expr(else_, heap, bindings)
    } else {
        Ok(Value::Void)
    }
}

/// Evaluate a statement.
pub fn eval_statement(
    stmt: &Statement,
    heap: &mut VirtualHeap,
    bindings: &mut HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match stmt {
        Statement::Let { name, expr, .. } => {
            if let Some(expr) = expr {
                let val = eval_expr(expr, heap, bindings)?;
                bindings.insert(name.clone(), val);
            }
            Ok(Value::Void)
        }
        Statement::Assign(lhs, rhs) => {
            let val = eval_expr(rhs, heap, bindings)?;
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
            let val = eval_expr(value, heap, bindings)?;
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
        Statement::Expression(expr) => eval_expr(expr, heap, bindings),
        Statement::Term(val) => {
            match val {
                Some(val) => {
                    // 2026-07-28: Term with value signals early return.
                    let result = eval_expr(val, heap, bindings)?;
                    Err(RuntimeError::TermReturn(result))
                }
                None => {
                    // Term without value is a convergence checkpoint — continue.
                    Ok(Value::Void)
                }
            }
        }
        Statement::Guarded(cond, body) => {
            let cv = eval_expr(cond, heap, bindings)?;
            if cv.is_true() {
                let mut result = Value::Void;
                for stmt in body {
                    result = eval_statement(stmt, heap, bindings)?;
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
            eval_expr(cond, heap, bindings)?;
            Ok(Value::Void)
        }
        Statement::If(cond, then, else_) => {
            let cv = eval_expr(cond, heap, bindings)?;
            if cv.is_true() {
                let mut result = Value::Void;
                for stmt in then {
                    result = eval_statement(stmt, heap, bindings)?;
                }
                Ok(result)
            } else {
                let mut result = Value::Void;
                for stmt in else_ {
                    result = eval_statement(stmt, heap, bindings)?;
                }
                Ok(result)
            }
        }
        Statement::Block(stmts) => {
            let mut result = Value::Void;
            for stmt in stmts {
                result = eval_statement(stmt, heap, bindings)?;
            }
            Ok(result)
        }
        Statement::ExitProgram(val) => {
            match val {
                Some(val) => {
                    let result = eval_expr(val, heap, bindings)?;
                    Err(RuntimeError::TermReturn(result))
                }
                None => {
                    Ok(Value::Void)
                }
            }
        }
        Statement::Rollback(_) => Ok(Value::Void),
        Statement::MetadataAssignment(_, _) => Ok(Value::Void),
        Statement::InlineAsm { .. } | Statement::InlineDefn(_) | Statement::InlineTxn(_) => Ok(Value::Void),
        Statement::SyncBlock(body) => {
            let mut result = Value::Void;
            for stmt in body {
                result = eval_statement(stmt, heap, bindings)?;
            }
            Ok(result)
        }
        Statement::Foreach { .. } => Ok(Value::Void),
        Statement::TrgBinding { .. } => Ok(Value::Void),
        Statement::Match { .. } => unreachable!("match only in $defn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval1(expr: &Expr) -> Value {
        eval_expr(expr, &mut VirtualHeap::new(), &mut HashMap::new()).unwrap()
    }

    /// Evaluate expecting an error — the error-testing twin of `eval1`.
    fn eval1_err(expr: &Expr) -> String {
        eval_expr(expr, &mut VirtualHeap::new(), &mut HashMap::new())
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
            Value::Product(vec![Value::int(1), Value::int(2), Value::int(3)])
        );
    }

    #[test]
    fn test_list_evaluates_to_product() {
        let l = Expr::List(vec![Expr::Bool(true), Expr::Decimal(7)]);
        assert_eq!(
            eval1(&l),
            Value::Product(vec![Value::bool(true), Value::int(7)])
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
    fn test_index_non_indexable_errors() {
        let idx = Expr::Index(Box::new(Expr::Decimal(42)), Box::new(Expr::Decimal(0)));
        assert!(eval1_err(&idx).contains("indexable"));
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
}
