// ── Narrow Int Pass ──────────────────────────────────────────
// 2026-07-24: Value-range inference for Int types.
//
// Returns a per-binding map: fn_name → { "ret" → bits, "let_x" → bits }
// The LLVM backend reads this map when emitting function signatures
// and local variables, using the narrowed width instead of 64-bit.
//
// On WASM this eliminates BigInt (i64 → i32) when values fit in 32 bits.
// On all targets it produces tighter code.

use std::collections::HashMap;

use crate::ast::*;

/// Extended with arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntRange {
    pub min: i128,
    pub max: i128,
}

impl IntRange {
    pub const UNKNOWN: IntRange = IntRange { min: i128::MIN, max: i128::MAX };
    pub fn exactly(v: i128) -> Self { IntRange { min: v, max: v } }
    pub fn is_unknown(&self) -> bool { self.min == i128::MIN && self.max == i128::MAX }

    /// Smallest power-of-two bit width that fits this range (signed).
    pub fn bit_width(&self) -> Option<u64> {
        if self.is_unknown() { return None; }
        if self.min == 0 && self.max == 0 { return Some(1); }
        let unsigned_max = self.max.max(self.min.unsigned_abs() as i128) as u128;
        let bits = if unsigned_max == 0 { 0 } else { 128 - unsigned_max.leading_zeros() as u64 };
        let sign_bit = if self.min < 0 { 1 } else { 0 };
        Some((bits + sign_bit).max(1))
    }
}

fn infer_range(expr: &Expr, scope: &HashMap<String, IntRange>) -> IntRange {
    match expr {
        Expr::Decimal(n) => IntRange::exactly(*n as i128),
        Expr::Bool(_) => IntRange { min: 0, max: 1 },
        Expr::Identifier(name) => {
            scope.get(name).copied().unwrap_or(IntRange::UNKNOWN)
        }
        Expr::BinaryOp(kind, lhs, rhs) => {
            let l = infer_range(lhs.as_ref(), scope);
            let r = infer_range(rhs.as_ref(), scope);
            match kind {
                BinaryOpKind::Add => add_range(l, r),
                BinaryOpKind::Sub => sub_range(l, r),
                BinaryOpKind::Mul => mul_range(l, r),
                BinaryOpKind::And => and_range(l, r),
                BinaryOpKind::Or => or_range(l, r),
                BinaryOpKind::Shl => shl_range(l, r),
                BinaryOpKind::Shr => shr_range(l, r),
                _ => IntRange::UNKNOWN,
            }
        }
        Expr::UnaryOp(kind, inner) => {
            let val = infer_range(inner.as_ref(), scope);
            match kind {
                UnaryOpKind::Neg => IntRange { min: -val.max, max: -val.min },
                UnaryOpKind::Not => IntRange { min: 0, max: 1 },
                _ => IntRange::UNKNOWN,
            }
        }
        Expr::Exists(_) => IntRange { min: 0, max: 1 },
        _ => IntRange::UNKNOWN,
    }
}

fn add_range(a: IntRange, b: IntRange) -> IntRange {
    IntRange { min: a.min.saturating_add(b.min), max: a.max.saturating_add(b.max) }
}
fn sub_range(a: IntRange, b: IntRange) -> IntRange {
    IntRange { min: a.min.saturating_sub(b.max), max: a.max.saturating_sub(b.min) }
}
fn mul_range(a: IntRange, b: IntRange) -> IntRange {
    let products = [
        a.min.saturating_mul(b.min), a.min.saturating_mul(b.max),
        a.max.saturating_mul(b.min), a.max.saturating_mul(b.max),
    ];
    IntRange { min: *products.iter().min().unwrap(), max: *products.iter().max().unwrap() }
}
fn and_range(a: IntRange, b: IntRange) -> IntRange { IntRange { min: 0, max: a.max.min(b.max) } }
fn or_range(a: IntRange, b: IntRange) -> IntRange {
    let combined = (a.max | b.max) as u128;
    let pow2 = if combined == 0 { 0 } else { 1u128 << (128 - combined.leading_zeros()) };
    let max_val = pow2.saturating_sub(1) as i128;
    IntRange { min: 0, max: max_val.max(0) }
}
fn shl_range(a: IntRange, b: IntRange) -> IntRange {
    if b.min < 0 || b.max > 127 { return IntRange::UNKNOWN; }
    IntRange {
        min: a.min.wrapping_shl(b.min.min(127).max(0) as u32),
        max: a.max.wrapping_shl(b.max.min(127).max(0) as u32),
    }
}
fn shr_range(a: IntRange, b: IntRange) -> IntRange {
    if b.min < 0 || b.max > 127 { return IntRange::UNKNOWN; }
    IntRange {
        min: a.min.wrapping_shr(b.max.min(127).max(0) as u32),
        max: a.max.wrapping_shr(b.min.min(127).max(0) as u32),
    }
}

/// Infer ranges for all let-bindings and returns in a statement block.
/// Returns a map of binding_name → narrowed bit_width for bindings
/// that can be narrowed below 64 bits.
fn infer_ranges_in_body(body: &[Statement], scope: &mut HashMap<String, IntRange>) -> (Option<IntRange>, HashMap<String, u64>) {
    let mut return_range = None;
    let mut narrowed_bindings = HashMap::new();

    for stmt in body {
        match stmt {
            Statement::Let { name, expr, .. } => {
                if let Some(e) = expr {
                    let range = infer_range(e, scope);
                    scope.insert(name.clone(), range);
                    if let Some(bits) = range.bit_width() {
                        if bits < 64 {
                            narrowed_bindings.insert(format!("let_{}", name), bits);
                        }
                    }
                }
            }
            Statement::Assign(target, expr) => {
                if let Expr::Identifier(name) = target {
                    let range = infer_range(expr, scope);
                    scope.insert(name.clone(), range);
                    if let Some(bits) = range.bit_width() {
                        if bits < 64 {
                            narrowed_bindings.insert(format!("assign_{}", name), bits);
                        }
                    }
                }
            }
            Statement::Guarded(cond, inner) => {
                if let Expr::BinaryOp(BinaryOpKind::Lt, lhs, rhs) = cond {
                    if let Expr::Identifier(name) = lhs.as_ref() {
                        let bound = infer_range(rhs.as_ref(), scope);
                        if let Some(entry) = scope.get(name) {
                            let narrowed = IntRange { min: entry.min, max: entry.max.min(bound.max - 1) };
                            let mut inner_scope = scope.clone();
                            inner_scope.insert(name.clone(), narrowed);
                            let (ir, nb) = infer_ranges_in_body(inner, &mut inner_scope);
                            narrowed_bindings.extend(nb);
                            if ir.is_some() { return_range = ir; }
                            continue;
                        }
                    }
                }
                let (ir, nb) = infer_ranges_in_body(inner, scope);
                narrowed_bindings.extend(nb);
                if ir.is_some() { return_range = ir; }
            }
            Statement::Term(Some(expr)) => {
                return_range = Some(infer_range(expr, scope));
            }
            Statement::Term(None) => {
                return_range = Some(IntRange::UNKNOWN);
            }
            _ => {}
        }
    }
    (return_range, narrowed_bindings)
}

/// Extract parameter ranges from a contract precondition expression.
/// Handles `a < N && b < M` patterns — the common case for range constraints.
fn apply_contract_ranges(expr: &Expr, scope: &mut HashMap<String, IntRange>) {
    match expr {
        // Single constraint: x < N
        Expr::BinaryOp(BinaryOpKind::Lt, lhs, rhs) => {
            if let Expr::Identifier(name) = lhs.as_ref() {
                let bound = extract_constant(rhs.as_ref());
                if let Some(hi) = bound {
                    if let Some(entry) = scope.get(name) {
                        let narrowed = IntRange { min: entry.min, max: entry.max.min(hi - 1) };
                        scope.insert(name.clone(), narrowed);
                    }
                }
            }
        }
        // Conjunction: x < N && y < M && ...
        Expr::BinaryOp(BinaryOpKind::And, lhs, rhs) => {
            apply_contract_ranges(lhs.as_ref(), scope);
            apply_contract_ranges(rhs.as_ref(), scope);
        }
        _ => {}
    }
}

/// Extract a constant integer value from an expression, if possible.
fn extract_constant(expr: &Expr) -> Option<i128> {
    match expr {
        Expr::Decimal(n) => Some(*n as i128),
        _ => None,
    }
}

/// Narrow a single definition/function — returns per-binding narrowed widths.
fn narrow_body(name: &str, params: &[(String, Type)], body: &[Statement], contract: Option<&Contract>) -> HashMap<String, u64> {
    let mut scope = HashMap::new();
    for (pname, _ty) in params {
        scope.insert(pname.clone(), IntRange::UNKNOWN);
    }
    // Apply contract precondition ranges
    if let Some(c) = contract {
        apply_contract_ranges(&c.pre_condition, &mut scope);
    }
    let (return_range, mut bindings) = infer_ranges_in_body(body, &mut scope);
    if let Some(range) = return_range {
        if let Some(bits) = range.bit_width() {
            if bits < 64 {
                bindings.insert("ret".to_string(), bits);
            }
        }
    }
    // Narrow parameter types where contract range proves they fit in fewer bits
    for (i, (pname, _ty)) in params.iter().enumerate() {
        if let Some(range) = scope.get(pname) {
            if let Some(bits) = range.bit_width() {
                if bits < 64 {
                    bindings.insert(format!("param_{}", i), bits);
                }
            }
        }
    }
    bindings
}

fn process(d: &Definition, all: &mut HashMap<String, HashMap<String, u64>>) {
    let bindings = narrow_body(&d.name, &d.parameters, &d.body, Some(&d.contract));
    if !bindings.is_empty() {
        all.insert(d.name.clone(), bindings);
    }
}

fn process_txn(t: &Transaction, all: &mut HashMap<String, HashMap<String, u64>>) {
    let bindings = narrow_body(&t.name, &t.parameters, &t.body, Some(&t.contract));
    if !bindings.is_empty() {
        all.insert(t.name.clone(), bindings);
    }
}

/// Main entry point: walk all TopLevel items and infer Int width narrowing.
/// Returns a map: fn_name → { binding_name → narrowed_bits }
///
/// The LLVM backend reads this map when emitting function signatures
/// and local variable declarations. Narrowed bindings use smaller LLVM
/// integer types (e.g., i32 instead of i64), eliminating WASM BigInt.
pub fn narrow_types(items: &[TopLevel]) -> HashMap<String, HashMap<String, u64>> {
    let mut all_bindings = HashMap::new();
    for item in items {
        match item {
            TopLevel::Definition(d) => process(d, &mut all_bindings),
            TopLevel::Transaction(t) => process_txn(t, &mut all_bindings),
            TopLevel::Export(e) => {
                if let TopLevel::Definition(d) = e.inner.as_ref() {
                    process(d, &mut all_bindings);
                }
            }
            _ => {}
        }
    }
    all_bindings
}
