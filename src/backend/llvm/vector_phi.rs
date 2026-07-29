// ── Vector Phi Groups ────────────────────────────────────────────
//
// 2026-07-29: Repurposed from SLP isomorphism into vector phi promotion.
// When N scalar fields share isomorphic expression trees, we promote them
// to a single <N x T> phi node. This reduces register pressure in hot loops
// from N scalar phis to 1 vector phi + extractelement/insertelement.
//
// Only fields with width >= 4 are promoted (below that, the scalar phi +
// extractelement overhead exceeds the register-pressure gain).
//
// The expression codegen per lane is IDENTICAL to the scalar case.
// No shufflevector, no hand-rolled SLP traversals. Only the phi storage
// mechanism changes — LLVM's optimizer handles the rest.

use crate::ast::{Expr, Statement, Type};
use crate::backend::llvm::context::FunctionContext;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// A group of fields promoted to a single vector phi node.
#[derive(Debug, Clone)]
pub(crate) struct VectorPhiGroup {
    /// Descriptive name for register naming (e.g., "bx").
    pub name: String,
    /// LLVM element type string (e.g., "float", "i64", "double").
    pub element_ty: String,
    /// Number of lanes (e.g., 4 for <4 x float>).
    pub width: usize,
    /// Field names in index order.
    pub fields: Vec<String>,
    /// The SSA register name for this group's phi (e.g., "%phi_bx").
    pub phi_reg: String,
    /// The SSA register name for this group's backedge (e.g., "%be_bx").
    pub backedge_reg: String,
}

/// Scan a transaction body for fields that can be grouped into vector phi nodes.
/// Uses isomorphism analysis from slp_isomorphism.rs.
///
/// A group is formed when:
/// 1. All fields have the same LLVM type (e.g., all float)
/// 2. All fields are unconditionally written every iteration (in write_set)
/// 3. The fields' assignment expressions are structurally isomorphic
///
/// Returns empty vec if no groups of size >= 4 are found.
pub(crate) fn detect_vector_groups(
    write_set: &HashSet<String>,
    body: &[Statement],
    field_index_map: &HashMap<String, usize>,
    field_types: &[String],
) -> Vec<VectorPhiGroup> {
    let candidates = crate::analysis::slp_isomorphism::analyze_body(body);
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut groups: Vec<VectorPhiGroup> = Vec::new();
    for c in &candidates {
        // All fields must be unconditionally written (in write_set).
        let all_in_write_set = c.fields.iter().all(|f| write_set.contains(f));
        if !all_in_write_set {
            continue;
        }

        // All fields must have the same LLVM type.
        let element_ty = match c.fields.first() {
            Some(first) => {
                let idx = match field_index_map.get(first.as_str()) {
                    Some(&i) => i,
                    None => continue,
                };
                field_types.get(idx).cloned().unwrap_or_else(|| "i64".to_string())
            }
            None => continue,
        };

        let all_same_type = c.fields.iter().all(|f| {
            field_index_map.get(f.as_str()).and_then(|idx| field_types.get(*idx))
                .map_or(false, |t| t == &element_ty)
        });
        if !all_same_type {
            continue;
        }

        groups.push(VectorPhiGroup {
            name: c.group_name.clone(),
            element_ty,
            width: c.width,
            fields: c.fields.clone(),
            phi_reg: String::new(),
            backedge_reg: String::new(),
        });
    }

    groups
}

/// Emit the pre-header insertelement chain and loop header phi nodes
/// for all vector groups.
///
/// For each group of <4 x float>:
///   %iv1 = insertelement <4 x float> undef, float %init_bx0, i32 0
///   %iv2 = insertelement <4 x float> %iv1, float %init_bx1, i32 1
///   ...
///   %phi_bx = phi <4 x float> [ %ivN, %entry ], [ %be_bx, %latch ]
///
/// Populates phi_reg and backedge_reg on each group.
/// Returns a map from field_name -> vector phi register name (for extractelement
/// lookup) and a map from field_name -> lane index within the vector.
pub(crate) fn emit_vector_header(
    fun: &mut FunctionContext,
    out: &mut String,
    groups: &mut [VectorPhiGroup],
    init_regs: &HashMap<String, String>,
    indent: &str,
) -> (HashMap<String, String>, HashMap<String, u32>) {
    let mut field_to_phi: HashMap<String, String> = HashMap::new();
    let mut field_to_lane: HashMap<String, u32> = HashMap::new();

    for g in groups.iter_mut() {
        let vec_ty = format!("<{} x {}>", g.width, g.element_ty);
        let mut prev_ins = String::new();

        for (lane_idx, field_name) in g.fields.iter().enumerate() {
            let init_val = init_regs.get(field_name.as_str())
                .cloned()
                .unwrap_or_else(|| "0".to_string());
            let ins_reg = fun.next_reg_with_prefix(&format!("iv_{}", g.name));
            if lane_idx == 0 {
                writeln!(out, "{}{} = insertelement {} undef, {} {}, i32 0",
                    indent, ins_reg, vec_ty, g.element_ty, init_val).ok();
            } else {
                writeln!(out, "{}{} = insertelement {} {}, {} {}, i32 {}",
                    indent, ins_reg, vec_ty, prev_ins, g.element_ty, init_val, lane_idx).ok();
            }
            prev_ins = ins_reg;
            field_to_lane.insert(field_name.clone(), lane_idx as u32);
        }

        let phi_reg = fun.next_reg_with_prefix(&format!("phi_{}", g.name));
        let phi_reg_name = phi_reg.clone();
        let be_reg = format!("%be_{}", g.name);
        writeln!(out, "{}{} = phi {} [ {}, %entry ], [ {}, %latch ]",
            indent, phi_reg_name, vec_ty, prev_ins, be_reg).ok();

        g.phi_reg = phi_reg_name.clone();
        g.backedge_reg = be_reg;

        for field_name in &g.fields {
            field_to_phi.insert(field_name.clone(), phi_reg_name.clone());
        }
    }

    (field_to_phi, field_to_lane)
}

/// When the body codegen encounters Identifier("bx0"):
/// - Look up "bx0" in the vector groups
/// - Emit: %lane = extractelement <4 x float> %phi_bx, i32 0
/// - Return the lane register name
///
/// Returns None if the field is not in any active vector group (caller
/// should fall back to scalar phi resolution).
pub(crate) fn emit_extractelement(
    fun: &mut FunctionContext,
    out: &mut String,
    field_name: &str,
    groups: &[VectorPhiGroup],
    field_to_phi: &HashMap<String, String>,
    field_to_lane: &HashMap<String, u32>,
    indent: &str,
) -> Option<String> {
    let phi_reg = field_to_phi.get(field_name)?;
    let lane_idx = field_to_lane.get(field_name)?;
    let g = groups.iter().find(|g| g.fields.contains(&field_name.to_string()))?;
    let vec_ty = format!("<{} x {}>", g.width, g.element_ty);
    let lane_reg = fun.next_reg_with_prefix(&format!("ex_{}", field_name));
    writeln!(out, "{}{} = extractelement {} {}, {} i32 {}",
        indent, lane_reg, vec_ty, phi_reg, g.element_ty, lane_idx).ok();
    Some(lane_reg)
}

/// Record a field update for the backedge. Called when the body codegen
/// processes Assign(Identifier("bx0"), val).
///
/// Stores the updated lane value. At the latch, all updated lanes are
/// assembled into the backedge vector via insertelement chain.
pub(crate) fn record_field_update(
    fun: &mut FunctionContext,
    field_name: &str,
    value_reg: &str,
    groups: &[VectorPhiGroup],
    field_to_lane: &HashMap<String, u32>,
) {
    let Some(g) = groups.iter().find(|g| g.fields.contains(&field_name.to_string())) else {
        return;
    };
    let Some(&lane_idx) = field_to_lane.get(field_name) else {
        return;
    };
    let key = format!("{}-{}", g.name, lane_idx);
    fun.vector_phi_current.insert(key, value_reg.to_string());
}

/// Emit the latch block's insertelement chain for all updated lanes.
/// For each group with updates:
///   %upd1 = insertelement <4 x float> %phi_bx, float %new_bx0, i32 0
///   ...
///   %be_bx = insertelement <4 x float> %updN, float %new_bx3, i32 3
///
/// The final insertelement result IS the backedge register.
pub(crate) fn emit_vector_backedge(
    fun: &mut FunctionContext,
    out: &mut String,
    groups: &[VectorPhiGroup],
    field_to_lane: &HashMap<String, u32>,
    indent: &str,
) {
    for g in groups {
        let vec_ty = format!("<{} x {}>", g.width, g.element_ty);
        let mut prev = g.phi_reg.clone();

        for (lane_idx, field_name) in g.fields.iter().enumerate() {
            let key = format!("{}-{}", g.name, lane_idx as u32);
            let value_reg = match fun.vector_phi_current.get(&key) {
                Some(r) => r.clone(),
                None => continue,
            };
            let ins_reg = fun.next_reg_with_prefix(&format!("up_{}", g.name));
            writeln!(out, "{}{} = insertelement {} {}, {} {}, i32 {}",
                indent, ins_reg, vec_ty, prev, g.element_ty, value_reg, lane_idx).ok();
            prev = ins_reg;
        }

        writeln!(out, "{}{} = bitcast {} {} to {}",
            indent, g.backedge_reg, vec_ty, prev, vec_ty).ok();
    }
}
