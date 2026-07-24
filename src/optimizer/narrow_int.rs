// ── Narrow Int Pass ──────────────────────────────────────────
// 2026-07-24: Value-range inference for Int types.
//
// After typechecking and normalizer, walks each function body tracking
// (min, max) ranges through expression trees. Where the range fits in
// a narrower width, updates ResolvedType.max_bits in the universe.
//
// On WASM this eliminates BigInt (i64 → i32) when values fit in 32 bits.
// On all targets it produces tighter code.

use std::collections::HashMap;

use crate::ast::*;
use crate::type_universe::{ResolvedType, TypeUniverse};

/// Extends IntRange with arithmetic.
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
    /// Returns None for unknown range.
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
        _ => IntRange::UNKNOWN,
    }
}

fn add_range(a: IntRange, b: IntRange) -> IntRange {
    IntRange {
        min: a.min.saturating_add(b.min),
        max: a.max.saturating_add(b.max),
    }
}

fn sub_range(a: IntRange, b: IntRange) -> IntRange {
    IntRange {
        min: a.min.saturating_sub(b.max),
        max: a.max.saturating_sub(b.min),
    }
}

fn mul_range(a: IntRange, b: IntRange) -> IntRange {
    let products = [
        a.min.saturating_mul(b.min),
        a.min.saturating_mul(b.max),
        a.max.saturating_mul(b.min),
        a.max.saturating_mul(b.max),
    ];
    IntRange {
        min: *products.iter().min().unwrap(),
        max: *products.iter().max().unwrap(),
    }
}

fn and_range(a: IntRange, b: IntRange) -> IntRange {
    IntRange { min: 0, max: a.max.min(b.max) }
}

fn or_range(a: IntRange, b: IntRange) -> IntRange {
    let combined = (a.max | b.max) as u128;
    let pow2 = if combined == 0 { 0 } else { 1u128 << (128 - combined.leading_zeros()) };
    let max_val = pow2.saturating_sub(1) as i128;
    IntRange { min: 0, max: max_val.max(0) }
}

fn shl_range(a: IntRange, b: IntRange) -> IntRange {
    if b.min < 0 || b.max > 127 { return IntRange::UNKNOWN; }
    let shift_min = b.min.min(127).max(0) as u32;
    let shift_max = b.max.min(127).max(0) as u32;
    IntRange {
        min: a.min.wrapping_shl(shift_min),
        max: a.max.wrapping_shl(shift_max),
    }
}

fn shr_range(a: IntRange, b: IntRange) -> IntRange {
    if b.min < 0 || b.max > 127 { return IntRange::UNKNOWN; }
    let shift_min = b.min.min(127).max(0) as u32;
    let shift_max = b.max.min(127).max(0) as u32;
    IntRange {
        min: a.min.wrapping_shr(shift_max),
        max: a.max.wrapping_shr(shift_min),
    }
}

/// Infer ranges for all let-bindings and returns in a statement block.
fn infer_ranges_in_body(body: &[Statement], scope: &mut HashMap<String, IntRange>) -> Option<IntRange> {
    let mut return_range = None;
    for stmt in body {
        match stmt {
            Statement::Let { name, expr, .. } => {
                if let Some(e) = expr {
                    let range = infer_range(e, scope);
                    scope.insert(name.clone(), range);
                }
            }
            Statement::Assign(target, expr) => {
                if let Expr::Identifier(name) = target {
                    let range = infer_range(expr, scope);
                    scope.insert(name.clone(), range);
                }
            }
            Statement::Guarded(cond, inner) => {
                // When guard may constrain variable ranges
                if let Expr::BinaryOp(BinaryOpKind::Lt, lhs, rhs) = cond {
                    if let Expr::Identifier(name) = lhs.as_ref() {
                        let bound = infer_range(rhs.as_ref(), scope);
                        if let Some(entry) = scope.get(name) {
                            let narrowed = IntRange {
                                min: entry.min,
                                max: entry.max.min(bound.max - 1),
                            };
                            let mut inner_scope = scope.clone();
                            inner_scope.insert(name.clone(), narrowed);
                            if let Some(r) = infer_ranges_in_body(inner, &mut inner_scope) {
                                return_range = Some(r);
                            }
                        }
                    }
                }
                // Default: evaluate body without narrowing
                if return_range.is_none() {
                    if let Some(r) = infer_ranges_in_body(inner, scope) {
                        return_range = Some(r);
                    }
                }
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
    return_range
}

/// Narrow Int types in a definition based on inferred value ranges.
fn narrow_definition(d: &Definition, universe: &mut TypeUniverse) {
    let mut scope = HashMap::new();
    // Initialize parameters as unknown
    for (name, _ty) in &d.parameters {
        scope.insert(name.clone(), IntRange::UNKNOWN);
    }
    if let Some(return_range) = infer_ranges_in_body(&d.body, &mut scope) {
        if let Some(bits) = return_range.bit_width() {
            if let Some(rt) = universe.types.get_mut(&d.name) {
                if rt.max_bits > bits && bits <= rt.max_bits {
                    rt.max_bits = bits;
                }
            }
        }
    }
}

/// Narrow Int types in a transaction based on inferred value ranges.
fn narrow_transaction(t: &Transaction, universe: &mut TypeUniverse) {
    let mut scope = HashMap::new();
    for (name, _ty) in &t.parameters {
        scope.insert(name.clone(), IntRange::UNKNOWN);
    }
    if let Some(return_range) = infer_ranges_in_body(&t.body, &mut scope) {
        if let Some(bits) = return_range.bit_width() {
            if let Some(rt) = universe.types.get_mut(&t.name) {
                if rt.max_bits > bits && bits <= rt.max_bits {
                    rt.max_bits = bits;
                }
            }
        }
    }
}

/// Main entry point: walk all TopLevel items and narrow Int types.
/// Call after normalizer, before codegen.
pub fn narrow_types(items: &mut [TopLevel], universe: &mut TypeUniverse) {
    for item in items.iter() {
        match item {
            TopLevel::Definition(d) => narrow_definition(d, universe),
            TopLevel::Transaction(t) => narrow_transaction(t, universe),
            _ => {}
        }
    }
}
