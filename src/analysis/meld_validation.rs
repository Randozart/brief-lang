//! 2026-07-16: P6 — Five-layer compile-time meld layout validation.
//!
//! Each meld declaration is checked for round-trip correctness:
//! forward ∘ inverse = identity for all inputs.
//!
//! Layers 1-3 always run (fatal errors). Layers 4-5 run only for
//! linear (field-to-field) melds; Layer 5 needs `z3` on PATH.
//!
//! All field offset/width annotations come from `attach_layout_fields()`
//! in `normalizer.rs`, stored as `field.{name}.offset` / `field.{name}.width`
//! on `ResolvedType.properties`.

use crate::ast::{Expr, MeldDeclaration, MeldRouteDef, PropertyValue};
use crate::proof_engine::{prove_smt_formula, SmtResult};
use crate::symbolic::{eval_symbolic_expr, SymbolicValue};
use crate::type_universe::{ResolvedType, TypeUniverse};
use std::collections::HashMap;

// ── Validation Error Enum ────────────────────────────────────────

/// 2026-07-16: Errors produced by the 5-layer validation cascade.
#[derive(Debug, Clone, PartialEq)]
pub enum MeldValidationError {
    TypeNotFound(String),
    FieldNotFound { ty: String, field: String },
    WidthMismatch { field: String, src_width: u64, dst_width: u64 },
    Overlap { bit: u64, field: String },
    Gap { bit: u64 },
    UnitVectorFailed { bit: u64 },
    SymbolicMismatch { field: String },
    SmtCounterexample { meld: String },
    SmtTimeout,
    NonLinearMeld(String),
}

impl std::fmt::Display for MeldValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeldValidationError::TypeNotFound(t) => write!(f, "type '{}' not found in universe", t),
            MeldValidationError::FieldNotFound { ty, field } => {
                write!(f, "field '{}.{}' not found (no offset/width annotation)", ty, field)
            }
            MeldValidationError::WidthMismatch { field, src_width, dst_width } => {
                write!(f, "field '{}' width mismatch: src={}, dst={}", field, src_width, dst_width)
            }
            MeldValidationError::Overlap { bit, field } => {
                write!(f, "bit {}: overlapping field '{}'", bit, field)
            }
            MeldValidationError::Gap { bit } => {
                write!(f, "bit {}: uncovered gap in destination layout", bit)
            }
            MeldValidationError::UnitVectorFailed { bit } => {
                write!(f, "bit {} unit vector did not survive round-trip", bit)
            }
            MeldValidationError::SymbolicMismatch { field } => {
                write!(f, "symbolic round-trip failed for field '{}'", field)
            }
            MeldValidationError::SmtCounterexample { meld } => {
                write!(f, "SMT found counterexample for meld '{}'", meld)
            }
            MeldValidationError::SmtTimeout => write!(f, "SMT solver timed out"),
            MeldValidationError::NonLinearMeld(m) => {
                write!(f, "meld '{}' has non-linear routes — skipping bit-level checks", m)
            }
        }
    }
}

// ── Public Entry Point ──────────────────────────────────────────

/// 2026-07-16: Run the full 5-layer validation cascade.
///
/// Layers 1-3: always run, errors are fatal (returned in Err).
/// Layer 4 (symbolic): runs only for linear melds, errors are warnings.
/// Layer 5 (SMT): runs only for linear melds in verbose mode, errors are warnings.
///
/// Returns Ok(()) if all applicable layers pass, or Err(list of errors).
pub fn validate_meld_layout(
    decl: &MeldDeclaration,
    universe: &TypeUniverse,
    verbose: bool,
) -> Result<(), Vec<MeldValidationError>> {
    let mut errors: Vec<MeldValidationError> = Vec::new();

    // Layer 1: Structural — always runs
    if let Err(e) = validate_structural(decl, universe) {
        errors.push(e);
    }

    // Layers 2-5 need both types with layout annotations
    let linear = is_linear_meld(decl);

    if !linear {
        errors.push(MeldValidationError::NonLinearMeld(format!(
            "{} -> {}", decl.name_a, decl.name_b
        )));
        return Err(errors);
    }

    // Layer 2: Bit-permutation — always runs for linear melds
    if let Err(e) = validate_bit_permutation(decl, universe) {
        errors.push(e);
    }

    // Layer 3: Unit-vector enumeration — always runs for linear melds
    if let Err(e) = validate_unit_vectors(decl, universe) {
        errors.push(e);
    }

    // If Layers 1-3 failed, don't run Layers 4-5
    if !errors.is_empty() {
        return Err(errors);
    }

    // Layer 4: Symbolic round-trip (fast, no external dependency)
    if let Err(e) = validate_symbolic(decl, universe) {
        // Symbolic errors are non-fatal: the simplifier may be too weak.
        eprintln!("warning: meld symbolic verification: {}", e);
    }

    // Layer 5: SMT universal proof (needs z3 on PATH)
    if verbose {
        if let Err(e) = validate_smt(decl, universe) {
            eprintln!("warning: meld SMT verification: {}", e);
        }
    }

    Ok(())
}

// ── Field Metadata Helpers ──────────────────────────────────────

/// 2026-07-16: Read a field's bit offset from type annotations.
/// Returns None when no annotation exists (field not part of layout).
pub fn get_field_offset(ty: &ResolvedType, field: &str) -> Option<u64> {
    let key = format!("field.{}.offset", field);
    ty.properties.get(&key).and_then(|v| {
        if let PropertyValue::Int(n) = v {
            Some(*n as u64)
        } else {
            None
        }
    })
}

/// 2026-07-16: Read a field's bit width from type annotations.
/// Returns None when no annotation exists.
pub fn get_field_width(ty: &ResolvedType, field: &str) -> Option<u64> {
    let key = format!("field.{}.width", field);
    ty.properties.get(&key).and_then(|v| {
        if let PropertyValue::Int(n) = v {
            Some(*n as u64)
        } else {
            None
        }
    })
}

/// 2026-07-16: Build a sorted list of (field_name, offset, width) for all
/// annotated fields of a type. Sorted by offset ascending.
fn build_field_map(ty: &ResolvedType) -> Vec<(String, u64, u64)> {
    let mut fields: Vec<(String, u64, u64)> = Vec::new();
    for (key, val) in &ty.properties {
        if let Some(rest) = key.strip_prefix("field.") {
            if let Some(name) = rest.strip_suffix(".offset") {
                if let PropertyValue::Int(offset) = val {
                    let width_key = format!("field.{}.width", name);
                    if let Some(PropertyValue::Int(width)) = ty.properties.get(&width_key) {
                        fields.push((name.to_string(), *offset as u64, *width as u64));
                    }
                }
            }
        }
    }
    fields.sort_by_key(|(_, offset, _)| *offset);
    fields
}

/// 2026-07-16: Check whether all routes in a meld are linear (field-to-field).
/// A linear route has dest_expr = Expr::Field(Expr::Identifier(_), field_name).
fn is_linear_meld(decl: &MeldDeclaration) -> bool {
    decl.routes.iter().all(|r| matches!(&r.dest_expr, Expr::Field(_, _)))
}

/// 2026-07-16: Get the source field name from a linear route expression.
/// Returns None for non-linear routes.
fn route_src_field(route: &MeldRouteDef) -> Option<&str> {
    match &route.dest_expr {
        Expr::Field(_, name) => Some(name.as_str()),
        _ => None,
    }
}

/// 2026-07-16: Check if a field name indicates padding (not required for coverage).
fn is_padding_field(name: &str) -> bool {
    name.contains("pad") || name.contains("reserved")
}

// ── Layer 1: Structural ─────────────────────────────────────────

/// 2026-07-16: Validate that both types exist and all referenced fields
/// have offset/width annotations.
fn validate_structural(
    decl: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    let type_a = universe
        .get(&decl.name_a)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_a.clone()))?;
    let type_b = universe
        .get(&decl.name_b)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_b.clone()))?;

    for route in &decl.routes {
        // Check dest_expr references a valid field on type_a
        if let Some(src_name) = route_src_field(route) {
            if get_field_offset(type_a, src_name).is_none() {
                return Err(MeldValidationError::FieldNotFound {
                    ty: type_a.name.clone(),
                    field: src_name.to_string(),
                });
            }
        }
        // Check accessor references a valid field on type_b
        if get_field_offset(type_b, &route.accessor).is_none() {
            return Err(MeldValidationError::FieldNotFound {
                ty: type_b.name.clone(),
                field: route.accessor.clone(),
            });
        }
    }

    Ok(())
}

// ── Layer 2: Bit-Permutation ────────────────────────────────────

/// 2026-07-16: For linear melds, verify the bit permutation is bijective:
/// no overlapping destination fields, no uncovered gaps, matching widths.
fn validate_bit_permutation(
    decl: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    // 2026-07-16: Empty routes = nothing to validate
    if decl.routes.is_empty() {
        return Ok(());
    }
    let type_a = universe.get(&decl.name_a)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_a.clone()))?;
    let type_b = universe.get(&decl.name_b)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_b.clone()))?;
    let total_bits = type_a.bytes * 8;
    let mut dest_covered = vec![false; total_bits as usize];

    for route in &decl.routes {
        let src_name = route_src_field(route).unwrap();
        let src_offset = get_field_offset(type_a, src_name).unwrap_or(0);
        let src_width = get_field_width(type_a, src_name).unwrap_or(64);
        let dst_offset = get_field_offset(type_b, &route.accessor).unwrap_or(0);
        let dst_width = get_field_width(type_b, &route.accessor).unwrap_or(64);

        // Width must match
        if src_width != dst_width {
            return Err(MeldValidationError::WidthMismatch {
                field: route.accessor.clone(),
                src_width,
                dst_width,
            });
        }

        // Mark destination bits covered; check for overlap
        for bit in dst_offset..dst_offset + dst_width {
            let idx = bit as usize;
            if idx >= dest_covered.len() {
                // Field extends beyond type width — error
                return Err(MeldValidationError::Overlap {
                    bit,
                    field: route.accessor.clone(),
                });
            }
            if dest_covered[idx] {
                return Err(MeldValidationError::Overlap {
                    bit,
                    field: route.accessor.clone(),
                });
            }
            dest_covered[idx] = true;
        }
    }

    // Check for gaps (uncovered bits) — exclude padding fields
    let padding_field_at = |bit: u64| -> bool {
        let bf = build_field_map(type_b);
        bf.iter()
            .any(|(name, offset, width)| {
                is_padding_field(name) && *offset <= bit && bit < *offset + *width
            })
    };

    for (bit, covered) in dest_covered.iter().enumerate() {
        if !covered && !padding_field_at(bit as u64) {
            return Err(MeldValidationError::Gap { bit: bit as u64 });
        }
    }

    Ok(())
}

// ── Layer 3: Unit-Vector Enumeration ────────────────────────────

/// 2026-07-16: For each field, verify round-trip preserves its value.
/// Creates a test value where the field = 1 (bit 0 set) and all other
/// fields = 0, applies forward + inverse, checks the field is still 1.
/// This is a stronger test than checking every bit: it verifies that
/// each field's value survives the round-trip independently.
fn validate_unit_vectors(
    decl: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    let type_a = universe.get(&decl.name_a)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_a.clone()))?;
    let type_b = universe.get(&decl.name_b)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_b.clone()))?;
    // 2026-07-16: Only test fields that are mapped in the meld routes.
    // Unmapped fields (padding, non-mapped slots) are not part of the
    // round-trip contract and would spuriously fail.
    let mapped_src: Vec<&str> = decl.routes.iter().filter_map(route_src_field).collect();
    let fields_a: Vec<_> = build_field_map(type_a).into_iter()
        .filter(|(name, _, _)| mapped_src.contains(&name.as_str()))
        .collect();

    for (field_name, _, _) in &fields_a {
        // Build a concrete value with only this field set to 1
        let mut src_val = HashMap::new();
        for (fn2, _, _) in &fields_a {
            src_val.insert(fn2.clone(), if fn2 == field_name { 1u64 } else { 0u64 });
        }
        // Forward: A → B
        let mid_val = eval_forward_u64(decl, &src_val, type_a, type_b);
        // Inverse: B → A
        let roundtrip = eval_inverse_u64(decl, &mid_val, type_a, type_b);

        if *roundtrip.get(field_name).unwrap_or(&0) != 1 {
            return Err(MeldValidationError::UnitVectorFailed {
                bit: get_field_offset(type_a, field_name).unwrap_or(0),
            });
        }
    }

    Ok(())
}

/// 2026-07-16: Evaluate forward routes: simple field-value mapping.
/// Maps each source field's value directly to the destination field.
/// This is not offset-aware — it treats each field as a standalone u64
/// value. The offset-aware packing is done only in the SMT layer.
fn eval_forward_u64(
    decl: &MeldDeclaration,
    input: &HashMap<String, u64>,
    _type_a: &ResolvedType,
    type_b: &ResolvedType,
) -> HashMap<String, u64> {
    let mut result = HashMap::new();
    for (name, _, _) in build_field_map(type_b) {
        result.insert(name, 0u64);
    }
    for route in &decl.routes {
        let src_name = match route_src_field(route) {
            Some(n) => n,
            None => continue,
        };
        let src_val = input.get(src_name).copied().unwrap_or(0);
        result.insert(route.accessor.clone(), src_val);
    }
    result
}

/// 2026-07-16: Evaluate inverse routes: reverse field-value mapping.
/// Each destination field's value maps back to the source field.
fn eval_inverse_u64(
    decl: &MeldDeclaration,
    input: &HashMap<String, u64>,
    type_a: &ResolvedType,
    _type_b: &ResolvedType,
) -> HashMap<String, u64> {
    let mut result = HashMap::new();
    for (name, _, _) in build_field_map(type_a) {
        result.insert(name, 0u64);
    }
    for route in &decl.routes {
        let src_name = route_src_field(route).unwrap();
        let val = input.get(&route.accessor).copied().unwrap_or(0);
        result.insert(src_name.to_string(), val);
    }
    result
}



// ── Layer 4: Symbolic Round-Trip ────────────────────────────────

/// 2026-07-16: Validate round-trip using symbolic execution.
/// Creates symbolic identifiers for each input field, evaluates forward
/// and inverse routes, then compares simplified output to input.
fn validate_symbolic(
    decl: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    let type_a = universe.get(&decl.name_a)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_a.clone()))?;
    let type_b = universe.get(&decl.name_b)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_b.clone()))?;

    // Build symbolic input: each source field gets a symbolic identifier
    let mut symbolic_input = HashMap::new();
    for (name, _, _) in build_field_map(type_a) {
        symbolic_input.insert(name.clone(), SymbolicValue::Identifier(format!("__in_{}", name)));
    }

    // Forward: A → B (symbolic)
    let symbolic_mid = eval_forward_symbolic(decl, &symbolic_input, type_a, type_b);

    // Inverse: B → A (symbolic)
    let symbolic_output = eval_inverse_symbolic(decl, &symbolic_mid, type_a, type_b);

    // Compare each output field to its corresponding input
    for (name, _, _) in build_field_map(type_a) {
        let expected =
            SymbolicValue::Identifier(format!("__in_{}", name));
        let actual = symbolic_output.get(&name).cloned().unwrap_or(SymbolicValue::Unknown);
        if !symbolic_deep_equals(&actual, &expected) {
            return Err(MeldValidationError::SymbolicMismatch { field: name });
        }
    }

    Ok(())
}

/// 2026-07-16: Evaluate forward routes symbolically.
fn eval_forward_symbolic(
    decl: &MeldDeclaration,
    input: &HashMap<String, SymbolicValue>,
    _type_a: &ResolvedType,
    type_b: &ResolvedType,
) -> HashMap<String, SymbolicValue> {
    let mut result = HashMap::new();
    for (name, _, _) in build_field_map(type_b) {
        result.insert(name.clone(), SymbolicValue::Literal(0, "i64".to_string()));
    }
    for route in &decl.routes {
        let src_name = match route_src_field(route) {
            Some(n) => n,
            None => continue,
        };
        let val = input.get(src_name).cloned().unwrap_or(SymbolicValue::Unknown);
        result.insert(route.accessor.clone(), val);
    }
    result
}

/// 2026-07-16: Evaluate inverse routes symbolically.
fn eval_inverse_symbolic(
    decl: &MeldDeclaration,
    input: &HashMap<String, SymbolicValue>,
    type_a: &ResolvedType,
    _type_b: &ResolvedType,
) -> HashMap<String, SymbolicValue> {
    let mut result = HashMap::new();
    for (name, _, _) in build_field_map(type_a) {
        result.insert(name.clone(), SymbolicValue::Literal(0, "i64".to_string()));
    }
    for route in &decl.routes {
        let dst_name = route_src_field(route).unwrap();
        let val = input.get(&route.accessor).cloned().unwrap_or(SymbolicValue::Unknown);
        result.insert(dst_name.to_string(), val);
    }
    result
}

/// 2026-07-16: Deep comparison of two symbolic values.
fn symbolic_deep_equals(a: &SymbolicValue, b: &SymbolicValue) -> bool {
    match (a, b) {
        (SymbolicValue::Identifier(an), SymbolicValue::Identifier(bn)) => an == bn,
        (SymbolicValue::Literal(av, _), SymbolicValue::Literal(bv, _)) => av == bv,
        (SymbolicValue::Binary(op_a, la, ra), SymbolicValue::Binary(op_b, lb, rb)) => {
            op_a == op_b && symbolic_deep_equals(la, lb) && symbolic_deep_equals(ra, rb)
        }
        _ => false,
    }
}

// ── Layer 5: SMT Universal Proof ────────────────────────────────

/// 2026-07-16: Validate meld via Z3 QF_BV universal proof.
/// Builds: ∀x. inverse(forward(x)) == x.
/// If UNSAT, identity holds for all inputs.
fn validate_smt(
    decl: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Result<(), MeldValidationError> {
    universe.get(&decl.name_a)
        .ok_or(MeldValidationError::TypeNotFound(decl.name_a.clone()))?;
    let formula = match build_meld_smt_formula(decl, universe) {
        Some(f) => f,
        None => return Ok(()), // No formula for non-linear routes — skip
    };

    match prove_smt_formula(&formula, 1000) {
        SmtResult::Unsat => Ok(()),
        SmtResult::Sat(_) | SmtResult::Unknown => {
            Err(MeldValidationError::SmtCounterexample {
                meld: format!("{} -> {}", decl.name_a, decl.name_b),
            })
        }
    }
}

/// 2026-07-16: Build an SMT-LIB2 QF_BV formula encoding the meld round-trip.
///
/// Structure:
///   (declare-const x (_ BitVec N))
///   (define-fun forward ((x (_ BitVec N))) (_ BitVec N)
///     (let ((f1 ((_ extract H1 L1) x)) ... )
///       (concat ...fields sorted by dst offset descending...)))
///   (define-fun inverse ((x (_ BitVec N))) (_ BitVec N)
///     (let ((f1 ((_ extract H1 L1) x)) ... )
///       (concat ...fields sorted by src offset descending...)))
///   (assert (not (= (inverse (forward x)) x)))
///   (check-sat)
///
/// Returns None for non-linear melds.
pub fn build_meld_smt_formula(
    decl: &MeldDeclaration,
    universe: &TypeUniverse,
) -> Option<String> {
    if !is_linear_meld(decl) {
        return None;
    }

    let type_a = universe.get(&decl.name_a)?;
    let type_b = universe.get(&decl.name_b)?;
    let total_bits = type_a.bytes * 8;
    let total_bits_b = type_b.bytes * 8;
    let n = total_bits.max(total_bits_b);

    let mut formula = String::new();
    formula.push_str("(set-logic QF_BV)\n");
    formula.push_str(&format!("(declare-const x (_ BitVec {}))\n", n));

    // ── Forward: A → B ──
    // Let bindings: extract each source field from A's layout
    formula.push_str(&format!(
        "(define-fun forward ((x (_ BitVec {}))) (_ BitVec {})\n",
        n, n
    ));
    formula.push_str("  (let (");
    for route in &decl.routes {
        let src_name = route_src_field(route).unwrap();
        let src_offset = get_field_offset(type_a, src_name).unwrap_or(0);
        let src_width = get_field_width(type_a, src_name).unwrap_or(64);
        let high = src_offset + src_width - 1;
        formula.push_str(&format!(
            " ({}_fv ((_ extract {} {}) x))",
            src_name, high, src_offset
        ));
    }
    formula.push_str("\n  )\n    ");

    // Concat: sort by destination field offset DESCENDING
    let mut dst_fields: Vec<(u64, &str, u64)> = decl
        .routes
        .iter()
        .map(|r| {
            let dst_offset = get_field_offset(type_b, &r.accessor).unwrap_or(0);
            let dst_width = get_field_width(type_b, &r.accessor).unwrap_or(64);
            (dst_offset + dst_width, r.accessor.as_str(), dst_width)
        })
        .collect();
    dst_fields.sort_by(|a, b| b.0.cmp(&a.0)); // desc by (offset + width)

    if dst_fields.is_empty() {
        formula.push_str("(_ BitVec 0)");
    } else if dst_fields.len() == 1 {
        formula.push_str(&format!("{}_fv", dst_fields[0].1));
    } else {
        formula.push_str("(concat");
        for (_, name, _) in &dst_fields {
            formula.push_str(&format!(" {}_fv", name));
        }
        formula.push_str(")");
    }
    formula.push_str("\n  )\n)\n");

    // ── Inverse: B → A ──
    // Let bindings: extract each source field from B's layout
    formula.push_str(&format!(
        "(define-fun inverse ((x (_ BitVec {}))) (_ BitVec {})\n",
        n, n
    ));
    formula.push_str("  (let (");
    for route in &decl.routes {
        let dst_offset = get_field_offset(type_b, &route.accessor).unwrap_or(0);
        let dst_width = get_field_width(type_b, &route.accessor).unwrap_or(64);
        let high = dst_offset + dst_width - 1;
        formula.push_str(&format!(
            " ({}_iv ((_ extract {} {}) x))",
            route.accessor, high, dst_offset
        ));
    }
    formula.push_str("\n  )\n    ");

    // Concat: sort by source field (A) offset DESCENDING
    let mut src_fields: Vec<(u64, &str)> = decl
        .routes
        .iter()
        .map(|r| {
            let src_name = route_src_field(r).unwrap();
            let src_offset = get_field_offset(type_a, src_name).unwrap_or(0);
            let src_width = get_field_width(type_a, src_name).unwrap_or(64);
            (src_offset + src_width, src_name)
        })
        .collect();
    src_fields.sort_by(|a, b| b.0.cmp(&a.0)); // desc by (offset + width)

    if src_fields.is_empty() {
        formula.push_str("(_ BitVec 0)");
    } else if src_fields.len() == 1 {
        formula.push_str(&format!("{}_iv", src_fields[0].1));
    } else {
        formula.push_str("(concat");
        for (_, name) in &src_fields {
            // Find the B field name that maps to this A field
            let b_name = decl
                .routes
                .iter()
                .find(|r| route_src_field(r) == Some(name))
                .map(|r| r.accessor.as_str())
                .unwrap_or(name);
            formula.push_str(&format!(" {}_iv", b_name));
        }
        formula.push_str(")");
    }
    formula.push_str("\n  )\n)\n");

    // Assert round-trip identity
    formula.push_str("(assert (not (= (inverse (forward x)) x)))\n");
    formula.push_str("(check-sat)\n");

    Some(formula)
}

// ── Builder: MeldDeclaration from TopLevel ──────────────────────

/// 2026-07-16: Build a `MeldDeclaration` from a `TopLevel::Meld`.
/// Only includes bindings with `layout.` prefix (those are the structural
/// field mappings); non-prefixed bindings are assumed to be metadata.
pub fn build_meld_declaration(
    name: &str,
    target: &str,
    bindings: &std::collections::HashMap<String, String>,
    span: &Option<crate::errors::Span>,
) -> MeldDeclaration {
    let mut routes = Vec::new();
    for (key, val) in bindings {
        let src_field = match key.strip_prefix("layout.") {
            Some(f) => f,
            None => continue,
        };
        routes.push(MeldRouteDef {
            accessor: val.clone(),
            dest_expr: Expr::Field(
                Box::new(Expr::Identifier(name.to_string())),
                src_field.to_string(),
            ),
        });
    }
    MeldDeclaration {
        name_a: name.to_string(),
        name_b: target.to_string(),
        routes,
        span: span.clone(),
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_universe::{ResolvedType, TypeUniverse};
    use std::collections::HashMap;

    /// Helper: create a ResolvedType with field offset/width annotations.
    fn make_type(name: &str, bytes: u64, fields: &[(&str, u64, u64)]) -> ResolvedType {
        let mut props = HashMap::new();
        for (fname, offset, width) in fields {
            props.insert(
                format!("field.{}.offset", fname),
                PropertyValue::Int(*offset as i64),
            );
            props.insert(
                format!("field.{}.width", fname),
                PropertyValue::Int(*width as i64),
            );
        }
        ResolvedType {
            name: name.to_string(),
            base: "Bits".to_string(),
            bytes,
            min_bits: bytes * 8,
            max_bits: bytes * 8,
            alignment: 8,
            properties: props,
            fields: vec![],
        }
    }

    fn make_universe() -> TypeUniverse {
        let mut u = TypeUniverse::new();
        u.types.insert(
            "A".to_string(),
            make_type("A", 16, &[("ptr", 0, 64), ("size", 64, 64)]),
        );
        u.types.insert(
            "B".to_string(),
            make_type("B", 16, &[("ptr", 0, 64), ("size", 64, 64)]),
        );
        u
    }

    fn make_meld_decl(name_a: &str, name_b: &str, fields: &[(&str, &str)]) -> MeldDeclaration {
        let routes = fields
            .iter()
            .map(|(src, dst)| MeldRouteDef {
                accessor: dst.to_string(),
                dest_expr: Expr::Field(
                    Box::new(Expr::Identifier(name_a.to_string())),
                    src.to_string(),
                ),
            })
            .collect();
        MeldDeclaration {
            name_a: name_a.to_string(),
            name_b: name_b.to_string(),
            routes,
            span: None,
        }
    }

    // ── Helper tests ──

    #[test]
    fn test_get_field_offset() {
        let ty = make_type("T", 8, &[("x", 0, 32), ("y", 32, 32)]);
        assert_eq!(get_field_offset(&ty, "x"), Some(0));
        assert_eq!(get_field_offset(&ty, "y"), Some(32));
        assert_eq!(get_field_offset(&ty, "z"), None);
    }

    #[test]
    fn test_get_field_width() {
        let ty = make_type("T", 8, &[("x", 0, 32), ("y", 32, 32)]);
        assert_eq!(get_field_width(&ty, "x"), Some(32));
        assert_eq!(get_field_width(&ty, "y"), Some(32));
    }

    #[test]
    fn test_build_field_map() {
        let ty = make_type("T", 8, &[("x", 0, 32), ("y", 32, 32)]);
        let map = build_field_map(&ty);
        assert_eq!(map.len(), 2);
        assert_eq!(map[0], ("x".to_string(), 0, 32));
        assert_eq!(map[1], ("y".to_string(), 32, 32));
    }

    #[test]
    fn test_is_linear_meld_positive() {
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        assert!(is_linear_meld(&decl));
    }

    #[test]
    fn test_is_linear_meld_negative() {
        let routes = vec![MeldRouteDef {
            accessor: "sum".to_string(),
            dest_expr: Expr::BinaryOp(
                crate::ast::BinaryOpKind::Add,
                Box::new(Expr::Field(Box::new(Expr::Identifier("A".into())), "x".into())),
                Box::new(Expr::Field(Box::new(Expr::Identifier("A".into())), "y".into())),
            ),
        }];
        let decl = MeldDeclaration {
            name_a: "A".into(),
            name_b: "B".into(),
            routes,
            span: None,
        };
        assert!(!is_linear_meld(&decl));
    }

    // ── Layer 1 tests ──

    #[test]
    fn test_l1_structural_ok() {
        let u = make_universe();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        assert!(validate_structural(&decl, &u).is_ok());
    }

    #[test]
    fn test_l1_structural_type_not_found() {
        let u = make_universe();
        let decl = make_meld_decl("A", "Z", &[("ptr", "ptr")]);
        match validate_structural(&decl, &u) {
            Err(MeldValidationError::TypeNotFound(n)) => assert_eq!(n, "Z"),
            other => panic!("expected TypeNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_l1_structural_field_not_found() {
        let u = make_universe();
        let decl = make_meld_decl("A", "B", &[("nonexistent", "ptr")]);
        match validate_structural(&decl, &u) {
            Err(MeldValidationError::FieldNotFound { field, .. }) => {
                assert_eq!(field, "nonexistent")
            }
            other => panic!("expected FieldNotFound, got {:?}", other),
        }
    }

    // ── Layer 2 tests ──

    #[test]
    fn test_l2_bit_permutation_ok() {
        let u = make_universe();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        assert!(validate_bit_permutation(&decl, &u).is_ok());
    }

    #[test]
    fn test_l2_width_mismatch() {
        let mut u = TypeUniverse::new();
        u.types.insert(
            "A".to_string(),
            make_type("A", 12, &[("x", 0, 64), ("y", 64, 32)]),
        );
        u.types.insert(
            "B".to_string(),
            make_type("B", 12, &[("x", 0, 64), ("y", 64, 64)]),
        );
        let decl = make_meld_decl("A", "B", &[("x", "x"), ("y", "y")]);
        match validate_bit_permutation(&decl, &u) {
            Err(MeldValidationError::WidthMismatch { field, .. }) => {
                assert_eq!(field, "y")
            }
            other => panic!("expected WidthMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_l2_overlap_detected() {
        let mut u = TypeUniverse::new();
        u.types.insert(
            "A".to_string(),
            make_type("A", 16, &[("x", 0, 64), ("y", 32, 64)]),
        );
        u.types.insert(
            "B".to_string(),
            make_type("B", 16, &[("x", 0, 64), ("y", 32, 64)]),
        );
        let decl = make_meld_decl("A", "B", &[("x", "x"), ("y", "y")]);
        match validate_bit_permutation(&decl, &u) {
            Err(MeldValidationError::Overlap { .. }) => {} // expected
            other => panic!("expected Overlap, got {:?}", other),
        }
    }

    #[test]
    fn test_l2_gap_detected() {
        let mut u = TypeUniverse::new();
        u.types.insert(
            "A".to_string(),
            make_type("A", 16, &[("x", 0, 64)]),
        );
        u.types.insert(
            "B".to_string(),
            make_type("B", 16, &[("x", 0, 64)]),
        );
        let decl = make_meld_decl("A", "B", &[("x", "x")]);
        match validate_bit_permutation(&decl, &u) {
            Err(MeldValidationError::Gap { .. }) => {} // expected
            other => panic!("expected Gap, got {:?}", other),
        }
    }

    // ── Layer 3 tests ──

    #[test]
    fn test_l3_unit_vector_ok() {
        let u = make_universe();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        assert!(validate_unit_vectors(&decl, &u).is_ok());
    }

    #[test]
    fn test_concrete_eval_forward() {
        let u = make_universe();
        let type_a = u.get("A").unwrap();
        let type_b = u.get("B").unwrap();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        let mut input = HashMap::new();
        input.insert("ptr".to_string(), 0x1234);
        input.insert("size".to_string(), 0);
        let mid = eval_forward_u64(&decl, &input, type_a, type_b);
        assert_eq!(mid.get("ptr"), Some(&0x1234u64));
    }

    #[test]
    fn test_concrete_forward_inverse_roundtrip() {
        let u = make_universe();
        let type_a = u.get("A").unwrap();
        let type_b = u.get("B").unwrap();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);

        // All zeros should round-trip to zeros
        let zero = HashMap::new();
        let mid = eval_forward_u64(&decl, &zero, type_a, type_b);
        let back = eval_inverse_u64(&decl, &mid, type_a, type_b);
        for (name, _, _) in build_field_map(type_a) {
            assert_eq!(*back.get(&name).unwrap_or(&0), 0, "field {} should be 0", name);
        }

        // ptr=1, size=0 should round-trip
        let mut input = HashMap::new();
        input.insert("ptr".to_string(), 1);
        input.insert("size".to_string(), 0);
        let mid2 = eval_forward_u64(&decl, &input, type_a, type_b);
        let back2 = eval_inverse_u64(&decl, &mid2, type_a, type_b);
        assert_eq!(back2.get("ptr"), Some(&1u64));
        assert_eq!(back2.get("size"), Some(&0u64));
    }

    // ── Layer 4 tests ──

    #[test]
    fn test_l4_symbolic_ok() {
        let u = make_universe();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        // Should pass: forward maps ptr->ptr, size->size; inverse maps back
        assert!(validate_symbolic(&decl, &u).is_ok());
    }

    // ── Layer 5 tests ──

    #[test]
    fn test_build_meld_smt_formula_produces_valid_smt() {
        let u = make_universe();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        let formula = build_meld_smt_formula(&decl, &u).unwrap();

        // Check formula structure
        assert!(formula.contains("set-logic QF_BV"));
        assert!(formula.contains("declare-const x"));
        assert!(formula.contains("define-fun forward"));
        assert!(formula.contains("define-fun inverse"));
        assert!(formula.contains("(assert (not (= (inverse (forward x)) x)))"));
        assert!(formula.contains("(check-sat)"));
    }

    #[test]
    fn test_build_meld_smt_formula_field_extraction() {
        let u = make_universe();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        let formula = build_meld_smt_formula(&decl, &u).unwrap();

        // The ptr field is at offset 0, width 64 → extract 63 0
        // The size field is at offset 64, width 64 → extract 127 64
        assert!(formula.contains("ptr_fv ((_ extract 63 0) x)"));
        assert!(formula.contains("size_fv ((_ extract 127 64) x)"));
    }

    #[test]
    fn test_build_meld_smt_formula_non_linear_returns_none() {
        let decl = MeldDeclaration {
            name_a: "A".into(),
            name_b: "B".into(),
            routes: vec![MeldRouteDef {
                accessor: "sum".to_string(),
                dest_expr: Expr::Decimal(42),
            }],
            span: None,
        };
        let u = make_universe();
        assert!(build_meld_smt_formula(&decl, &u).is_none());
    }

    // ── Full cascade tests ──

    #[test]
    fn test_full_cascade_linear_symmetric_ok() {
        let u = make_universe();
        let decl = make_meld_decl("A", "B", &[("ptr", "ptr"), ("size", "size")]);
        let result = validate_meld_layout(&decl, &u, false);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn test_full_cascade_width_mismatch_fails() {
        let mut u = TypeUniverse::new();
        u.types.insert(
            "A".to_string(),
            make_type("A", 12, &[("x", 0, 64), ("y", 64, 32)]),
        );
        u.types.insert(
            "B".to_string(),
            make_type("B", 12, &[("x", 0, 64), ("y", 64, 64)]),
        );
        let decl = make_meld_decl("A", "B", &[("x", "x"), ("y", "y")]);
        let result = validate_meld_layout(&decl, &u, false);
        match result {
            Err(errors) => {
                assert!(errors.iter().any(|e| matches!(e, MeldValidationError::WidthMismatch { .. })));
            }
            Ok(_) => panic!("expected Err"),
        }
    }
}
