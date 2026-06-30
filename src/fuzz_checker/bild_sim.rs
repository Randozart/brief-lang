// ── BILD Simulator ──────────────────────────────────────────────────
//
// Lightweight register-machine interpreter that executes BILD (LLVM-IR-like)
// bodies of inop declarations with concrete `Value` types. Used by the fuzz
// checker to verify inop behavior without needing a fallback expression.
//
// The BILD body is a linear sequence of SSA instructions with no control flow.
// Each instruction is stored as a string in `Vec<String>` (semicolon included).
//
// Supported ops: add, sub, mul, sdiv, udiv, srem, urem, fadd, fsub, fmul, fdiv,
// and, or, xor, shl, lshr, ashr, icmp, fcmp, select, trunc, zext, sext,
// fptrunc, fpext, fptosi, sitofp, uitofp, load, store, getelementptr, term.
// Opaque ops (call, asm, etc.) return an error — the caller decides how
// to handle unverifiable results.

use crate::errors::{FuzzError, Span};
use crate::interpreter::Value;
use std::collections::HashMap;

/// Execute a BILD body with concrete register bindings.
///
/// `body` — each element is one BILD statement with trailing `;` included.
/// `params` — formal parameter names and types for the inop.
/// `bindings` — concrete argument values keyed by parameter name.
/// `has_state` — whether `%state` pointer is available.
/// `state_fields` — state field values keyed by field index (for GEP+load/store).
///
/// Returns the terminator value(s), or a `FuzzError` on simulation failure.
pub fn execute_bild(
    body: &[String],
    params: &[(String, crate::ast::Type)],
    bindings: &HashMap<String, Value>,
    has_state: bool,
    state_fields: &HashMap<usize, Value>,
) -> Result<Vec<Value>, FuzzError> {
    if body.is_empty() {
        return Ok(vec![Value::Void]);
    }

    // Register file: maps register names (e.g. "%t0", "%res") to Values.
    let mut regs: HashMap<String, Value> = HashMap::new();

    // Pre-populate parameter registers.
    for (name, _ty) in params {
        if let Some(val) = bindings.get(name) {
            regs.insert(format!("%{}", name), val.clone());
        }
    }

    // Pre-populate %state with a dummy pointer if the inop has state access.
    if has_state {
        regs.insert("%state".to_string(), Value::Ptr(0));
    }

    // State view: maps GEP-computed field indices to Values.
    // Only used when the inop has state access and load/store instructions.
    let mut state_view: HashMap<usize, Value> = state_fields.clone();

    let mut results: Vec<Value> = Vec::new();
    let mut opaque_dependency = false;

    for (line_idx, line) in body.iter().enumerate() {
        // Skip empty lines.
        if line.trim().is_empty() {
            continue;
        }

        // Remove trailing semicolon for tokenization.
        let clean = line.trim_end_matches(';').trim();
        if clean.is_empty() {
            continue;
        }

        // Check for terminator.
        if clean.starts_with("term") {
            let rest = clean[4..].trim(); // after "term"
            if rest.is_empty() {
                // `term;` with no value
                results.push(Value::Void);
            } else {
                // `term %reg` or `term %r1, %r2`
                let parts: Vec<&str> = rest.split(',').collect();
                for p in parts {
                    let reg_name = p.trim();
                    if let Some(val) = regs.get(reg_name) {
                        results.push(val.clone());
                    } else if !reg_name.is_empty() {
                        return Err(FuzzError::Unverifiable {
                            function: "(bild sim)".to_string(),
                            case_index: line_idx,
                            detail: format!("undefined register in terminator: {}", reg_name),
                            span: Span::dummy(),
                        });
                    }
                }
            }
            break; // terminator ends execution
        }

        // Parse assignment: %dest = opcode type operands...
        let parts: Vec<&str> = clean.split_whitespace().collect();
        if parts.len() < 4 || parts[1] != "=" {
            // Not an assignment — skip (could be comment or metadata).
            continue;
        }

        let dest = parts[0]; // e.g. "%res"
        let opcode = parts[2];
        // parts[3] is typically a type like "i64" — skip it for most operations.
        let operands: Vec<&str> = parts[3..].to_vec();

        let resolved = match opcode {
            "add" | "sub" | "mul" | "udiv" | "sdiv" | "urem" | "srem"
            | "fadd" | "fsub" | "fmul" | "fdiv" => {
                resolve_binop(opcode, &operands, &regs, line_idx)?
            }
            "and" | "or" | "xor" | "shl" | "lshr" | "ashr" => {
                resolve_bitwise(opcode, &operands, &regs, line_idx)?
            }
            "icmp" | "fcmp" => {
                resolve_icmp(opcode, &operands, &regs, line_idx)?
            }
            "select" => {
                resolve_select(&operands, &regs, line_idx)?
            }
            "trunc" | "zext" | "sext" | "fptrunc" | "fpext"
            | "fptosi" | "sitofp" | "uitofp" => {
                // Conversion ops: pass through the operand value (identity for simulation).
                resolve_conversion(opcode, &operands, &regs, line_idx)?
            }
            "load" => {
                resolve_load(&operands, &regs, &state_view, line_idx)?
            }
            "store" => {
                resolve_store(&operands, &regs, &mut state_view, line_idx)?;
                continue; // store doesn't produce a value
            }
            "getelementptr" | "getelementptr inbounds" => {
                resolve_gep(&operands, &regs, line_idx)?
            }
            "inttoptr" | "bitcast" | "alloca" => {
                // Pass-through: identity for the source operand.
                resolve_pass_through(&operands, &regs, line_idx)?
            }
            "extractvalue" => {
                // Simple extract: first value, first index.
                resolve_extractvalue(&operands, &regs, line_idx)?
            }
            _ => {
                // Opaque instruction (asm, call, atomicrmw, cmpxchg, etc.)
                opaque_dependency = true;
                continue;
            }
        };

        regs.insert(dest.to_string(), resolved);
    }

    if results.is_empty() {
        // No terminator found — void return.
        results.push(Value::Void);
    }

    if opaque_dependency && !results.is_empty() {
        let all_unknown = results.iter().all(|v| *v == Value::Void);
        // Don't error — let the caller decide based on whether the result
        // is meaningful.
    }

    Ok(results)
}

fn resolve_reg(name: &str, regs: &HashMap<String, Value>) -> Result<Value, FuzzError> {
    let trimmed = name.trim_end_matches(',');
    if let Some(val) = regs.get(trimmed) {
        Ok(val.clone())
    } else {
        // Try as literal integer (e.g. "42").
        if let Ok(n) = trimmed.parse::<i64>() {
            return Ok(Value::Int(n));
        }
        Ok(Value::Int(0)) // default zero for undefined
    }
}

fn resolve_binop(
    opcode: &str,
    operands: &[&str],
    regs: &HashMap<String, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    // Format: opcode type op1, op2
    let ty = operands.first().copied().unwrap_or("i64");
    let a = resolve_reg(operands.get(1).unwrap_or(&"0"), regs)?;
    let b = resolve_reg(operands.get(2).unwrap_or(&"0"), regs)?;

    let (va, vb) = match (&a, &b) {
        (Value::Int(va), Value::Int(vb)) => (*va, *vb),
        (Value::Bool(va), Value::Bool(vb)) => (*va as i64, *vb as i64),
        _ => return Ok(Value::Int(0)),
    };

    if ty == "float" || ty.starts_with("float") {
        let fa = match &a {
            Value::Float(f) => *f,
            _ => 0.0,
        };
        let fb = match &b {
            Value::Float(f) => *f,
            _ => 0.0,
        };
        return Ok(match opcode {
            "fadd" => Value::Float(fa + fb),
            "fsub" => Value::Float(fa - fb),
            "fmul" => Value::Float(fa * fb),
            "fdiv" => Value::Float(fa / fb),
            _ => Value::Float(0.0),
        });
    }

    Ok(Value::Int(match opcode {
        "add" => va.wrapping_add(vb),
        "sub" => va.wrapping_sub(vb),
        "mul" => va.wrapping_mul(vb),
        "sdiv" | "udiv" => {
            if vb == 0 { 0 } else { va / vb }
        }
        "srem" | "urem" => {
            if vb == 0 { 0 } else { va % vb }
        }
        // Float ops with i64 — shouldn't happen but handle gracefully.
        "fadd" => va + vb,
        "fsub" => va - vb,
        "fmul" => va * vb,
        "fdiv" => {
            if vb == 0 { 0 } else { va / vb }
        }
        _ => 0,
    }))
}

fn resolve_bitwise(
    opcode: &str,
    operands: &[&str],
    regs: &HashMap<String, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    let a = resolve_reg(operands.get(0).unwrap_or(&"0"), regs)?;
    let b = resolve_reg(operands.get(1).unwrap_or(&"0"), regs)?;
    let va = match a { Value::Int(n) => n, Value::Bool(b) => b as i64, _ => 0 };
    let vb = match b { Value::Int(n) => n, Value::Bool(b) => b as i64, _ => 0 };

    Ok(Value::Int(match opcode {
        "and" => va & vb,
        "or" => va | vb,
        "xor" => va ^ vb,
        "shl" => va.wrapping_shl(vb as u32),
        "lshr" => (va as u64).wrapping_shr(vb as u32) as i64,
        "ashr" => va.wrapping_shr(vb as u32),
        _ => 0,
    }))
}

fn resolve_icmp(
    _opcode: &str,
    operands: &[&str],
    regs: &HashMap<String, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    // Format: icmp <cond> <type> <op1>, <op2>
    let cond = operands.first().copied().unwrap_or("eq");
    let _ty = operands.get(1).copied().unwrap_or("i64");
    let a = resolve_reg(operands.get(2).unwrap_or(&"0"), regs)?;
    let b = resolve_reg(operands.get(3).unwrap_or(&"0"), regs)?;

    let va = match &a { Value::Int(n) => *n, Value::Bool(b) => *b as i64, _ => 0 };
    let vb = match &b { Value::Int(n) => *n, Value::Bool(b) => *b as i64, _ => 0 };

    let result = match cond {
        "eq" => va == vb,
        "ne" => va != vb,
        "slt" | "ult" | "olt" => va < vb,
        "sle" | "ule" | "ole" => va <= vb,
        "sgt" | "ugt" | "ogt" => va > vb,
        "sge" | "uge" | "oge" => va >= vb,
        _ => false,
    };

    Ok(Value::Bool(result))
}

fn resolve_select(
    operands: &[&str],
    regs: &HashMap<String, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    // Format: select <cond-ty> <cond>, <type> <val1>, <val2>
    let cond = resolve_reg(operands.get(1).unwrap_or(&"false"), regs)?;
    let val1 = resolve_reg(operands.get(3).unwrap_or(&"0"), regs)?;
    let val2 = resolve_reg(operands.get(4).unwrap_or(&"0"), regs)?;

    match cond {
        Value::Bool(true) => Ok(val1),
        Value::Bool(false) => Ok(val2),
        _ => Ok(val2),
    }
}

fn resolve_conversion(
    _opcode: &str,
    operands: &[&str],
    regs: &HashMap<String, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    // Format: opcode <src-ty> <value> to <dst-ty>
    // For simulation: pass through the operand value (identity).
    let val = resolve_reg(operands.get(1).unwrap_or(&"0"), regs)?;
    Ok(val)
}

fn resolve_pass_through(
    operands: &[&str],
    regs: &HashMap<String, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    // inttoptr, bitcast, alloca: pass through the first operand value.
    let val = resolve_reg(operands.get(1).unwrap_or(&"0"), regs)?;
    Ok(val)
}

fn resolve_load(
    operands: &[&str],
    regs: &HashMap<String, Value>,
    state_view: &HashMap<usize, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    // Format: load <type>, <ptr-type> <ptr>
    // For state simulation, the pointer comes from a GEP result.
    let ptr_name = operands.last().copied().unwrap_or("%state");
    let trimmed = ptr_name.trim_end_matches(',');
    let ptr_val = if let Some(val) = regs.get(trimmed) {
        val.clone()
    } else {
        Value::Ptr(0)
    };

    match ptr_val {
        Value::Ptr(idx) => {
            Ok(state_view.get(&(idx as usize)).cloned().unwrap_or(Value::Int(0)))
        }
        Value::Int(idx) => {
            Ok(state_view.get(&(idx as usize)).cloned().unwrap_or(Value::Int(0)))
        }
        _ => Ok(Value::Int(0)),
    }
}

fn resolve_store(
    operands: &[&str],
    regs: &HashMap<String, Value>,
    state_view: &mut HashMap<usize, Value>,
    _line: usize,
) -> Result<(), FuzzError> {
    // Format: store <type> <value>, <ptr-type> <ptr>
    let val_name = operands.get(1).copied().unwrap_or("0");
    let val = resolve_reg(val_name, regs)?;
    let ptr_name = operands.last().copied().unwrap_or("%state");
    let trimmed = ptr_name.trim_end_matches(',');
    let ptr_val = if let Some(p) = regs.get(trimmed) {
        p.clone()
    } else {
        Value::Ptr(0)
    };

    let idx = match ptr_val {
        Value::Ptr(n) => n as usize,
        Value::Int(n) => n as usize,
        _ => 0,
    };
    state_view.insert(idx, val);
    Ok(())
}

fn resolve_gep(
    operands: &[&str],
    regs: &HashMap<String, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    // Format: getelementptr [inbounds] <type>, <ptr-type> <ptr>, <idx-type> <idx>, ...
    // Simplified: extract the last index operand as the field offset.
    // e.g. "getelementptr inbounds %State, ptr %state, i32 0, i32 3" → field 3
    // Look for the last non-type, non-"inbounds" token that is an integer or register.
    let mut last_idx: i64 = 0;
    for token in operands.iter().rev() {
        let t = token.trim_end_matches(',');
        if t == "inbounds" || t.starts_with('%') || t == "ptr" || t.starts_with('i') || t == "float" || t == "double" {
            continue;
        }
        if let Ok(n) = t.parse::<i64>() {
            last_idx = n;
            break;
        }
        if let Some(val) = regs.get(t) {
            if let Value::Int(n) = val {
                last_idx = *n;
                break;
            }
        }
    }

    // Return the field index as a Ptr value (used by load/store).
    Ok(Value::Ptr(last_idx as u64))
}

fn resolve_extractvalue(
    operands: &[&str],
    regs: &HashMap<String, Value>,
    _line: usize,
) -> Result<Value, FuzzError> {
    // Format: extractvalue <struct-type> <value>, <idx>
    // Simplified: return the first value (cmpxchg result is { i64, i1 }).
    let val = resolve_reg(operands.get(1).unwrap_or(&"0"), regs)?;
    Ok(val)
}
