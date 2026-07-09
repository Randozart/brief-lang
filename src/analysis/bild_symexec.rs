use crate::analysis::bild_asm;
use crate::ast::{Expr, InopDeclaration};
use std::collections::HashMap;

/// Symbolic expression for proving BILD body matches its fallback.
#[derive(Debug, Clone, PartialEq)]
pub enum SymExpr {
    Var(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Add(Box<SymExpr>, Box<SymExpr>),
    Sub(Box<SymExpr>, Box<SymExpr>),
    Mul(Box<SymExpr>, Box<SymExpr>),
    Div(Box<SymExpr>, Box<SymExpr>),
    Rem(Box<SymExpr>, Box<SymExpr>),
    FAdd(Box<SymExpr>, Box<SymExpr>),
    FSub(Box<SymExpr>, Box<SymExpr>),
    FMul(Box<SymExpr>, Box<SymExpr>),
    FDiv(Box<SymExpr>, Box<SymExpr>),
    BitAnd(Box<SymExpr>, Box<SymExpr>),
    BitOr(Box<SymExpr>, Box<SymExpr>),
    BitXor(Box<SymExpr>, Box<SymExpr>),
    Shl(Box<SymExpr>, Box<SymExpr>),
    LShr(Box<SymExpr>, Box<SymExpr>),
    AShr(Box<SymExpr>, Box<SymExpr>),
    Eq(Box<SymExpr>, Box<SymExpr>),
    Ne(Box<SymExpr>, Box<SymExpr>),
    Lt(Box<SymExpr>, Box<SymExpr>),
    Le(Box<SymExpr>, Box<SymExpr>),
    Gt(Box<SymExpr>, Box<SymExpr>),
    Ge(Box<SymExpr>, Box<SymExpr>),
    Select(Box<SymExpr>, Box<SymExpr>, Box<SymExpr>),
    Not(Box<SymExpr>),
    Neg(Box<SymExpr>),
    /// Opaque operation — cannot be symbolically executed.
    Opaque(String),
}

impl SymExpr {
    /// Structural equality with commutativity for add/mul/and/or/xor.
    pub fn structural_eq(&self, other: &SymExpr) -> bool {
        if self == other {
            return true;
        }
        match (self, other) {
            (SymExpr::Add(a1, b1), SymExpr::Add(a2, b2))
            | (SymExpr::Mul(a1, b1), SymExpr::Mul(a2, b2))
            | (SymExpr::BitAnd(a1, b1), SymExpr::BitAnd(a2, b2))
            | (SymExpr::BitOr(a1, b1), SymExpr::BitOr(a2, b2))
            | (SymExpr::BitXor(a1, b1), SymExpr::BitXor(a2, b2)) => {
                (a1 == a2 && b1 == b2) || (a1 == b2 && b1 == a2)
            }
            _ => false,
        }
    }
}

/// BILD symbolic execution error.
#[derive(Debug, Clone)]
pub enum SymExecError {
    UnsupportedOpcode(String, usize),
    ParseError(String, usize),
    MissingTerminator,
    NoFallback,
    OpaqueInstruction(String, usize),
}

impl SymExecError {
    pub fn to_string(&self) -> String {
        match self {
            SymExecError::UnsupportedOpcode(op, line) => {
                format!("unsupported BILD opcode '{}' at line {}", op, line)
            }
            SymExecError::ParseError(msg, line) => {
                format!("BILD parse error at line {}: {}", line, msg)
            }
            SymExecError::MissingTerminator => {
                "BILD body has no terminator".to_string()
            }
            SymExecError::NoFallback => {
                "inop has no fallback expression to verify against".to_string()
            }
            SymExecError::OpaqueInstruction(op, line) => {
                format!("opaque BILD instruction '{}' at line {} — contract cannot be verified", op, line)
            }
        }
    }
}

/// Result of symbolic execution.
#[derive(Debug)]
pub struct SymExecResult {
    pub bild_expr: Option<SymExpr>,
    pub fallback_expr: Option<SymExpr>,
    pub errors: Vec<SymExecError>,
    /// True if the BILD body contains opaque ops (load, store, call, asm).
    pub has_opaque_ops: bool,
}

/// Symbolically execute the BILD body and compare with the fallback expression.
pub fn verify_inop(inop: &InopDeclaration) -> SymExecResult {
    let mut errors = Vec::new();

    let bild_expr = match symexec_bild(inop) {
        Ok(Some(expr)) => Some(expr),
        Ok(None) => {
            errors.push(SymExecError::MissingTerminator);
            None
        }
        Err(e) => {
            errors.push(e);
            None
        }
    };

    let fallback_expr = inop.fallback.as_ref().and_then(|fb| {
        symexec_expr(fb)
    });

    let has_opaque_ops = inop.llvm_body.iter().enumerate().any(|(i, line)| {
        let trimmed = line.trim().trim_end_matches(';');
        // Check the RHS of any assignment for opaque instructions
        let rhs = if let Some(eq_pos) = trimmed.find(" = ") {
            trimmed[eq_pos + 3..].trim()
        } else {
            trimmed
        };
        rhs.starts_with("load ")
            || rhs.starts_with("store ")
            || rhs.starts_with("call ")
            || rhs.starts_with("alloca ")
            || rhs.starts_with("getelementptr ")
    });

    SymExecResult { bild_expr, fallback_expr, errors, has_opaque_ops }
}

/// Symbolically execute the BILD body.
/// Returns the symbolic expression for the terminator's return value.
fn symexec_bild(inop: &InopDeclaration) -> Result<Option<SymExpr>, SymExecError> {
    // Desugar asm target blocks before symbolic execution
    let body = bild_asm::desugar_asm_target(&inop.llvm_body);

    let mut regs: HashMap<String, SymExpr> = HashMap::new();

    // Parameters are symbolic variables
    for (name, _) in &inop.params {
        regs.insert(format!("%{}", name), SymExpr::Var(name.clone()));
    }

    let mut return_expr: Option<SymExpr> = None;

    for (i, line) in body.iter().enumerate() {
        let trimmed = line.trim().trim_end_matches(';');

        if trimmed.starts_with("term!") {
            // Side-effecting term — no return value
            return_expr = Some(SymExpr::Opaque("term!".to_string()));
            continue;
        }

        if trimmed.starts_with("term ") || trimmed == "term" {
            let after = trimmed.strip_prefix("term").map(|s| s.trim()).unwrap_or("");
            if !after.is_empty() {
                let parts: Vec<&str> = after.split(',').map(|s| s.trim()).collect();
                if parts.len() == 1 {
                    return_expr = Some(resolve_reg(parts[0], &regs));
                } else {
                    // Multi-output: return first value (conservative)
                    return_expr = Some(resolve_reg(parts[0], &regs));
                }
            }
            continue;
        }

        // Parse %reg = opcode ...
        if let Some(eq_pos) = trimmed.find(" = ") {
            let lhs = trimmed[..eq_pos].trim().to_string();
            let rhs = trimmed[eq_pos + 3..].trim();

            if lhs.starts_with('%') {
                // Treat call, asm, load, store, alloca, getelementptr as opaque
                if rhs.starts_with("call ") || rhs.starts_with("asm ")
                    || rhs.starts_with("load ") || rhs.starts_with("store ")
                    || rhs.starts_with("alloca ") || rhs.starts_with("getelementptr ")
                {
                    regs.insert(lhs, SymExpr::Opaque(rhs.to_string()));
                } else {
                    let sym = parse_bild_instruction(rhs, &regs, i)?;
                    regs.insert(lhs, sym);
                }
            }
        }
    }

    Ok(return_expr)
}

/// Parse a single BILD instruction into a SymExpr.
fn parse_bild_instruction(
    instr: &str,
    regs: &HashMap<String, SymExpr>,
    line: usize,
) -> Result<SymExpr, SymExecError> {
    let parts: Vec<&str> = instr.split_whitespace().collect();
    if parts.is_empty() {
        return Err(SymExecError::ParseError("empty instruction".to_string(), line));
    }

    let opcode = parts[0];
    match opcode {
        "add" | "sub" | "mul" | "udiv" | "sdiv" | "urem" | "srem"
        | "fadd" | "fsub" | "fmul" | "fdiv"
        | "and" | "or" | "xor" | "shl" | "lshr" | "ashr"
        | "icmp" | "fcmp" | "select" => {
            // Format: opcode type op1, op2  (or opcode type op1, op2, op3 for select)
            let mut parts_iter = instr.split_whitespace().skip(1);
            let _ty = parts_iter.next(); // skip type (i64, float, etc.)
            let mut operands: Vec<&str> = parts_iter.collect();
            // Remove trailing commas
            for op in operands.iter_mut() {
                *op = op.trim_end_matches(',');
            }

            match opcode {
                "add" | "fadd" => {
                    if operands.len() < 2 { return Err(SymExecError::ParseError("expected 2 operands".to_string(), line)); }
                    let a = resolve_reg(operands[0], regs);
                    let b = resolve_reg(operands[1], regs);
                    Ok(if opcode == "fadd" { SymExpr::FAdd(Box::new(a), Box::new(b)) }
                       else { SymExpr::Add(Box::new(a), Box::new(b)) })
                }
                "sub" | "fsub" => {
                    if operands.len() < 2 { return Err(SymExecError::ParseError("expected 2 operands".to_string(), line)); }
                    let a = resolve_reg(operands[0], regs);
                    let b = resolve_reg(operands[1], regs);
                    Ok(if opcode == "fsub" { SymExpr::FSub(Box::new(a), Box::new(b)) }
                       else { SymExpr::Sub(Box::new(a), Box::new(b)) })
                }
                "mul" | "fmul" => {
                    if operands.len() < 2 { return Err(SymExecError::ParseError("expected 2 operands".to_string(), line)); }
                    let a = resolve_reg(operands[0], regs);
                    let b = resolve_reg(operands[1], regs);
                    Ok(if opcode == "fmul" { SymExpr::FMul(Box::new(a), Box::new(b)) }
                       else { SymExpr::Mul(Box::new(a), Box::new(b)) })
                }
                "sdiv" | "udiv" | "fdiv" => {
                    if operands.len() < 2 { return Err(SymExecError::ParseError("expected 2 operands".to_string(), line)); }
                    let a = resolve_reg(operands[0], regs);
                    let b = resolve_reg(operands[1], regs);
                    Ok(if opcode == "fdiv" { SymExpr::FDiv(Box::new(a), Box::new(b)) }
                       else { SymExpr::Div(Box::new(a), Box::new(b)) })
                }
                "srem" | "urem" => {
                    if operands.len() < 2 { return Err(SymExecError::ParseError("expected 2 operands".to_string(), line)); }
                    let a = resolve_reg(operands[0], regs);
                    let b = resolve_reg(operands[1], regs);
                    Ok(SymExpr::Rem(Box::new(a), Box::new(b)))
                }
                "and" => Ok(binop(operands, regs, line, |a, b| SymExpr::BitAnd(a, b))?),
                "or" => Ok(binop(operands, regs, line, |a, b| SymExpr::BitOr(a, b))?),
                "xor" => Ok(binop(operands, regs, line, |a, b| SymExpr::BitXor(a, b))?),
                "shl" => Ok(binop(operands, regs, line, |a, b| SymExpr::Shl(a, b))?),
                "lshr" => Ok(binop(operands, regs, line, |a, b| SymExpr::LShr(a, b))?),
                "ashr" => Ok(binop(operands, regs, line, |a, b| SymExpr::AShr(a, b))?),
                "icmp" | "fcmp" => {
                    // Format: icmp <cond> <type> <op1>, <op2>
                    // Don't use the general skip(1) — icmp has a different layout
                    let mut icmp_parts = instr.split_whitespace();
                    let _icmp_op = icmp_parts.next(); // "icmp"
                    let cond = icmp_parts.next().unwrap_or("eq"); // "sgt"
                    let _icmp_ty = icmp_parts.next(); // "i64"
                    let mut icmp_ops: Vec<&str> = icmp_parts.collect();
                    let a = resolve_reg(icmp_ops.first().map(|s| s.trim_end_matches(',')).unwrap_or("0"), regs);
                    let b = resolve_reg(icmp_ops.get(1).copied().unwrap_or("0"), regs);
                    match cond {
                        "eq" => Ok(SymExpr::Eq(Box::new(a), Box::new(b))),
                        "ne" => Ok(SymExpr::Ne(Box::new(a), Box::new(b))),
                        "slt" | "ult" | "olt" => Ok(SymExpr::Lt(Box::new(a), Box::new(b))),
                        "sle" | "ule" | "ole" => Ok(SymExpr::Le(Box::new(a), Box::new(b))),
                        "sgt" | "ugt" | "ogt" => Ok(SymExpr::Gt(Box::new(a), Box::new(b))),
                        "sge" | "uge" | "oge" => Ok(SymExpr::Ge(Box::new(a), Box::new(b))),
                        _ => Err(SymExecError::ParseError(format!("unknown icmp condition '{}'", cond), line)),
                    }
                }
                "select" => {
                    // Format: select <cond-ty> <cond-val>, <type> <val1>, <val2>
                    let mut sel_parts = instr.split_whitespace();
                    let _sel_op = sel_parts.next(); // "select"
                    let _cond_ty = sel_parts.next(); // "i1"
                    let mut sel_ops: Vec<&str> = sel_parts.collect();
                    let c = resolve_reg(sel_ops.first().map(|s| s.trim_end_matches(',')).unwrap_or("0"), regs);
                    let _val_ty = sel_ops.get(1).copied(); // skip type
                    let a = resolve_reg(sel_ops.get(2).map(|s| s.trim_end_matches(',')).unwrap_or("0"), regs);
                    let b = resolve_reg(sel_ops.get(3).copied().unwrap_or("0"), regs);
                    Ok(SymExpr::Select(Box::new(c), Box::new(a), Box::new(b)))
                }
                _ => Ok(SymExpr::Opaque(instr.to_string())),
            }
        }
        "trunc" | "zext" | "sext" | "fptrunc" | "fpext" | "fptosi" | "sitofp" | "uitofp" => {
            // Conversion: opcode type value to type → identity in symbolic execution
            let parts: Vec<&str> = instr.split_whitespace().collect();
            if parts.len() < 3 {
                return Err(SymExecError::ParseError("expected 'opcode type value'".to_string(), line));
            }
            let val = parts[2].trim_end_matches(',');
            Ok(resolve_reg(val, regs))
        }
        _ => Ok(SymExpr::Opaque(instr.to_string())),
    }
}

fn binop(
    operands: Vec<&str>,
    regs: &HashMap<String, SymExpr>,
    line: usize,
    f: fn(Box<SymExpr>, Box<SymExpr>) -> SymExpr,
) -> Result<SymExpr, SymExecError> {
    if operands.len() < 2 {
        return Err(SymExecError::ParseError("expected 2 operands".to_string(), line));
    }
    let a = resolve_reg(operands[0], regs);
    let b = resolve_reg(operands[1], regs);
    Ok(f(Box::new(a), Box::new(b)))
}

/// Resolve a register name or literal to a SymExpr.
fn resolve_reg(name: &str, regs: &HashMap<String, SymExpr>) -> SymExpr {
    let name = name.trim();
    if name.starts_with('%') {
        regs.get(name).cloned().unwrap_or(SymExpr::Var(name[1..].to_string()))
    } else if let Ok(n) = name.parse::<i64>() {
        SymExpr::Int(n)
    } else if let Ok(f) = name.parse::<f64>() {
        SymExpr::Float(f)
    } else if name == "true" {
        SymExpr::Bool(true)
    } else if name == "false" {
        SymExpr::Bool(false)
    } else {
        SymExpr::Var(name.to_string())
    }
}

/// Symbolically evaluate a Brief expression.
pub fn symexec_expr(expr: &Expr) -> Option<SymExpr> {
    match expr {
        Expr::Integer(n) => Some(SymExpr::Int(*n)),
        Expr::Float(f) => Some(SymExpr::Float(*f)),
        Expr::Bool(b) => Some(SymExpr::Bool(*b)),
        Expr::Identifier(name) => Some(SymExpr::Var(name.clone())),
        Expr::Add(a, b) => Some(SymExpr::Add(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Sub(a, b) => Some(SymExpr::Sub(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Mul(a, b) => Some(SymExpr::Mul(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Div(a, b) => Some(SymExpr::Div(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Mod(a, b) => Some(SymExpr::Rem(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Eq(a, b) => Some(SymExpr::Eq(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Ne(a, b) => Some(SymExpr::Ne(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Lt(a, b) => Some(SymExpr::Lt(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Le(a, b) => Some(SymExpr::Le(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Gt(a, b) => Some(SymExpr::Gt(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Ge(a, b) => Some(SymExpr::Ge(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::And(a, b) => Some(SymExpr::BitAnd(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Or(a, b) => Some(SymExpr::BitOr(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Not(a) => Some(SymExpr::Not(Box::new(symexec_expr(a)?))),
        Expr::Neg(a) => Some(SymExpr::Neg(Box::new(symexec_expr(a)?))),
        Expr::BitAnd(a, b) => Some(SymExpr::BitAnd(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::BitOr(a, b) => Some(SymExpr::BitOr(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::BitXor(a, b) => Some(SymExpr::BitXor(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Shl(a, b) => Some(SymExpr::Shl(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Shr(a, b) => Some(SymExpr::AShr(Box::new(symexec_expr(a)?), Box::new(symexec_expr(b)?))),
        Expr::Block(_, last) => symexec_expr(last),
        _ => None,
    }
}

/// Compare BILD result with fallback result.
/// Returns true if they match structurally.
pub fn compare_bild_with_fallback(
    bild: &Option<SymExpr>,
    fallback: &Option<SymExpr>,
    inop_name: &str,
) -> Option<String> {
    match (bild, fallback) {
        (Some(b), Some(f)) => {
            if b.structural_eq(f) || *b == *f {
                None // Match
            } else {
                Some(format!(
                    "inop `{}`: BILD body produces {:?} but fallback produces {:?}",
                    inop_name, b, f
                ))
            }
        }
        (Some(_), None) => {
            Some(format!("inop `{}`: BILD body has a return value but no fallback to verify against", inop_name))
        }
        (None, Some(_)) => {
            Some(format!("inop `{}`: fallback exists but BILD body has no terminator", inop_name))
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Contract, InopDeclaration, Type};

    fn make_inop(name: &str, params: Vec<(&str, Type)>, body: Vec<&str>, fallback: Option<Expr>) -> InopDeclaration {
        InopDeclaration {
            name: name.to_string(),
            type_params: vec![],
            params: params.into_iter().map(|(n, t)| (n.to_string(), t)).collect(),
            outputs: vec![Type::Custom("Int".to_string())],
            contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
            llvm_body: body.into_iter().map(|s| s.to_string()).collect(),
            fallback,
            has_side_effects: false,
            has_state_access: false,
            section: None,
            llvm_body_spans: vec![],
            span: None,
        }
    }

    #[test]
    fn test_sadd_matches_fallback() {
        let inop = make_inop("sadd", vec![("a", Type::Custom("Int".to_string())), ("b", Type::Custom("Int".to_string()))], vec![
            "%res = add i64 %a, %b;",
            "term %res;",
        ], Some(Expr::Add(Box::new(Expr::Identifier("a".into())), Box::new(Expr::Identifier("b".into())))));
        let result = verify_inop(&inop);
        let msg = compare_bild_with_fallback(&result.bild_expr, &result.fallback_expr, "sadd");
        assert!(msg.is_none(), "sadd should match fallback: {:?}", msg);
    }

    #[test]
    fn test_mismatch_detected() {
        let inop = make_inop("bad", vec![("a", Type::Custom("Int".to_string()))], vec![
            "%res = add i64 %a, %a;",
            "term %res;",
        ], Some(Expr::Add(Box::new(Expr::Identifier("a".into())), Box::new(Expr::Integer(1)))));
        let result = verify_inop(&inop);
        let msg = compare_bild_with_fallback(&result.bild_expr, &result.fallback_expr, "bad");
        assert!(msg.is_some(), "mismatch should be detected");
        assert!(msg.unwrap().contains("produces"), "message should describe mismatch");
    }

    #[test]
    fn test_opaque_instruction_detected() {
        let inop = make_inop("loader", vec![("p", Type::Custom("Int".to_string()))], vec![
            "%res = load i64, i8* %p;",
            "term %res;",
        ], Some(Expr::Identifier("p".into())));
        let result = verify_inop(&inop);
        assert!(result.has_opaque_ops, "load should be detected as opaque");
    }

    #[test]
    fn test_missing_fallback() {
        let inop = make_inop("nofb", vec![("a", Type::Custom("Int".to_string()))], vec![
            "term %a;",
        ], None);
        let msg = compare_bild_with_fallback(&Some(SymExpr::Var("a".into())), &None, "nofb");
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("no fallback"));
    }

    #[test]
    fn test_sdiv_srem_matches() {
        let inop = make_inop("divmod", vec![("a", Type::Custom("Int".to_string())), ("b", Type::Custom("Int".to_string()))], vec![
            "%q = sdiv i64 %a, %b;",
            "%r = srem i64 %a, %b;",
            "term %q;",
        ], Some(Expr::Div(Box::new(Expr::Identifier("a".into())), Box::new(Expr::Identifier("b".into())))));
        let result = verify_inop(&inop);
        let msg = compare_bild_with_fallback(&result.bild_expr, &result.fallback_expr, "divmod");
        assert!(msg.is_none(), "divmod should match fallback: {:?}", msg);
    }

    #[test]
    fn test_icmp_select_matches() {
        let inop = make_inop("max", vec![("a", Type::Custom("Int".to_string())), ("b", Type::Custom("Int".to_string()))], vec![
            "%cmp = icmp sgt i64 %a, %b;",
            "%res = select i1 %cmp, i64 %a, i64 %b;",
            "term %res;",
        ], Some(Expr::Identifier("a".into())));  // simplified: just test BILD parses
        let result = verify_inop(&inop);
        assert!(result.errors.is_empty(), "no errors: {:?}", result.errors);
        assert!(matches!(result.bild_expr, Some(SymExpr::Select(..))));
    }
}
